use crate::{
    archetype::{
        Archetype, ArchetypeColumnInfo, ArchetypeDynamicEntityColumnAccess,
        ArchetypeEntityColumnAccess, ArchetypeEntityRowAccess, ArchetypeError,
    },
    bundle::{Bundle, BundleColumns},
    entity::Entity,
    processor::{WorldProcessor, WorldProcessorEntityMapping},
    query::{DynamicQueryFilter, DynamicQueryIter, TypedQueryFetch, TypedQueryIter},
    Component, ComponentRef, ComponentRefMut,
};
use intuicio_core::{registry::Registry, types::struct_type::NativeStructBuilder};
use intuicio_data::type_hash::TypeHash;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    error::Error,
    marker::PhantomData,
    sync::{Arc, RwLock, RwLockReadGuard},
};

#[derive(Debug, PartialEq, Eq)]
pub enum WorldError {
    Archetype(ArchetypeError),
    ReachedEntityIdCapacity,
    ReachedArchetypeIdCapacity,
    EntityDoesNotExists { entity: Entity },
    ArchetypeDoesNotExists { id: u32 },
    DuplicateMutableArchetypeAccess { id: u32 },
    EmptyColumnSet,
}

impl WorldError {
    pub fn allow<T>(
        input: Result<T, Self>,
        items: impl IntoIterator<Item = Self>,
        ok: T,
    ) -> Result<T, Self> {
        match input {
            Err(error) => {
                if items.into_iter().any(|item| error == item) {
                    Ok(ok)
                } else {
                    Err(error)
                }
            }
            result => result,
        }
    }
}

impl Error for WorldError {}

impl From<ArchetypeError> for WorldError {
    fn from(value: ArchetypeError) -> Self {
        Self::Archetype(value)
    }
}

impl std::fmt::Display for WorldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Archetype(archetype) => write!(f, "World archetype: {}", archetype),
            Self::ReachedEntityIdCapacity => write!(f, "Reached entity id capacity"),
            Self::ReachedArchetypeIdCapacity => write!(f, "Reached archetype id capacity"),
            Self::EntityDoesNotExists { entity } => {
                write!(f, "Entity does not exists: {}", entity)
            }
            Self::ArchetypeDoesNotExists { id } => {
                write!(f, "Archetype does not exists: {}", id)
            }
            Self::DuplicateMutableArchetypeAccess { id } => {
                write!(f, "Trying to access mutably same archetype twice: {}", id)
            }
            Self::EmptyColumnSet => {
                write!(f, "Trying to perform change on empty column set")
            }
        }
    }
}

#[derive(Default)]
struct EntityMap {
    id_generator: u32,
    /// index is entity id, value is pair of generation and optional archetype id.
    table: Vec<(u32, Option<u32>)>,
    reusable: Vec<Entity>,
    size: usize,
}

impl EntityMap {
    fn is_empty(&self) -> bool {
        self.size == 0
    }

    fn len(&self) -> usize {
        self.size
    }

    fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.table
            .iter()
            .enumerate()
            .filter_map(|(id, (generation, archetype))| {
                if archetype.is_some() {
                    Some(unsafe { Entity::new_unchecked(id as u32, *generation) })
                } else {
                    None
                }
            })
    }

    fn clear(&mut self) {
        self.id_generator = 0;
        self.table.clear();
        self.reusable.clear();
        self.size = 0;
    }

    fn acquire(&mut self) -> Result<(Entity, &mut Option<u32>), WorldError> {
        if let Some(mut entity) = self.reusable.pop() {
            let (generation, archetype) = &mut self.table[entity.id() as usize];
            entity = entity.bump_generation();
            *generation = entity.generation();
            self.size += 1;
            return Ok((entity, archetype));
        }
        if self.id_generator == u32::MAX {
            Err(WorldError::ReachedEntityIdCapacity)
        } else {
            let id = self.id_generator;
            self.id_generator += 1;
            while self.table.len() < self.id_generator as usize {
                if self.table.len() == self.table.capacity() {
                    self.table.reserve_exact(self.table.capacity());
                }
                self.table.push((0, None));
            }
            let (_, archetype) = &mut self.table[id as usize];
            self.size += 1;
            Ok((Entity::new(id, 0).unwrap(), archetype))
        }
    }

    fn release(&mut self, entity: Entity) -> Result<u32, WorldError> {
        if let Some((generation, archetype)) = self.table.get_mut(entity.id() as usize) {
            if entity.generation() == *generation {
                if let Some(archetype) = archetype.take() {
                    self.reusable.push(entity);
                    self.size -= 1;
                    Ok(archetype)
                } else {
                    Err(WorldError::EntityDoesNotExists { entity })
                }
            } else {
                Err(WorldError::EntityDoesNotExists { entity })
            }
        } else {
            Err(WorldError::EntityDoesNotExists { entity })
        }
    }

    fn get(&self, entity: Entity) -> Result<u32, WorldError> {
        if let Some((generation, archetype)) = self.table.get(entity.id() as usize) {
            if entity.generation() == *generation {
                if let Some(archetype) = *archetype {
                    Ok(archetype)
                } else {
                    Err(WorldError::EntityDoesNotExists { entity })
                }
            } else {
                Err(WorldError::EntityDoesNotExists { entity })
            }
        } else {
            Err(WorldError::EntityDoesNotExists { entity })
        }
    }

    fn set(&mut self, entity: Entity, archetype_id: u32) -> Result<(), WorldError> {
        if let Some((generation, archetype)) = self.table.get_mut(entity.id() as usize) {
            if entity.generation() == *generation {
                if let Some(archetype) = archetype.as_mut() {
                    *archetype = archetype_id;
                    Ok(())
                } else {
                    Err(WorldError::EntityDoesNotExists { entity })
                }
            } else {
                Err(WorldError::EntityDoesNotExists { entity })
            }
        } else {
            Err(WorldError::EntityDoesNotExists { entity })
        }
    }
}

#[derive(Default)]
struct ArchetypeMap {
    id_generator: u32,
    /// index is archetype id, value is optional archetype.
    table: Vec<Option<Archetype>>,
    reusable: Vec<u32>,
}

impl ArchetypeMap {
    fn iter(&self) -> impl Iterator<Item = &Archetype> + '_ {
        self.table.iter().filter_map(|archetype| archetype.as_ref())
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &mut Archetype> + '_ {
        self.table
            .iter_mut()
            .filter_map(|archetype| archetype.as_mut())
    }

    fn clear(&mut self) {
        self.id_generator = 0;
        self.table.clear();
        self.reusable.clear();
    }

    fn acquire(&mut self) -> Result<(u32, &mut Option<Archetype>), WorldError> {
        if let Some(id) = self.reusable.pop() {
            let archetype = &mut self.table[id as usize];
            return Ok((id, archetype));
        }
        if self.id_generator == u32::MAX {
            Err(WorldError::ReachedArchetypeIdCapacity)
        } else {
            let id = self.id_generator;
            self.id_generator += 1;
            while self.table.len() < self.id_generator as usize {
                if self.table.len() == self.table.capacity() {
                    self.table.reserve_exact(self.table.capacity());
                }
                self.table.push(None);
            }
            let archetype = &mut self.table[id as usize];
            Ok((id, archetype))
        }
    }

    fn get(&self, id: u32) -> Result<&Archetype, WorldError> {
        if let Some(archetype) = self
            .table
            .get(id as usize)
            .and_then(|archetype| archetype.as_ref())
        {
            Ok(archetype)
        } else {
            Err(WorldError::ArchetypeDoesNotExists { id })
        }
    }

    fn get_mut(&mut self, id: u32) -> Result<&mut Archetype, WorldError> {
        if let Some(archetype) = self
            .table
            .get_mut(id as usize)
            .and_then(|archetype| archetype.as_mut())
        {
            Ok(archetype)
        } else {
            Err(WorldError::ArchetypeDoesNotExists { id })
        }
    }

    fn get_mut_two(&mut self, [a, b]: [u32; 2]) -> Result<[&mut Archetype; 2], WorldError> {
        if a == b {
            return Err(WorldError::DuplicateMutableArchetypeAccess { id: a });
        }
        if let Some(archetype) = self.table.get(a as usize) {
            if archetype.is_none() {
                return Err(WorldError::ArchetypeDoesNotExists { id: a });
            }
        } else {
            return Err(WorldError::ArchetypeDoesNotExists { id: a });
        }
        if let Some(archetype) = self.table.get(b as usize) {
            if archetype.is_none() {
                return Err(WorldError::ArchetypeDoesNotExists { id: b });
            }
        } else {
            return Err(WorldError::ArchetypeDoesNotExists { id: b });
        }
        if a < b {
            let (left, right) = self.table.split_at_mut(b as usize);
            Ok([
                left[a as usize].as_mut().unwrap(),
                right[0].as_mut().unwrap(),
            ])
        } else {
            let (right, left) = self.table.split_at_mut(a as usize);
            Ok([
                left[0].as_mut().unwrap(),
                right[b as usize].as_mut().unwrap(),
            ])
        }
    }

    fn find_by_columns_exact(&self, columns: &[ArchetypeColumnInfo]) -> Option<u32> {
        for (id, archetype) in self.table.iter().enumerate() {
            if let Some(archetype) = archetype.as_ref() {
                if archetype.has_columns_exact(columns) {
                    return Some(id as u32);
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum RelationConnections<T: Component> {
    Zero([(T, Entity); 0]),
    One([(T, Entity); 1]),
    More(Vec<(T, Entity)>),
}

impl<T: Component> Default for RelationConnections<T> {
    fn default() -> Self {
        Self::Zero(Default::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Relation<T: Component> {
    connections: RelationConnections<T>,
}

impl<T: Component> Default for Relation<T> {
    fn default() -> Self {
        Self {
            connections: Default::default(),
        }
    }
}

impl<T: Component> Relation<T> {
    pub fn install_to_registry(registry: &mut Registry) {
        registry.add_type(NativeStructBuilder::new::<Self>().build());
    }

    pub fn register_to_processor(processor: &mut WorldProcessor) {
        processor.register_entity_remapping::<Self>(|relation, mapping| {
            let iter = match &mut relation.connections {
                RelationConnections::Zero(a) => a.iter_mut(),
                RelationConnections::One(a) => a.iter_mut(),
                RelationConnections::More(vec) => vec.iter_mut(),
            };
            for (_, entity) in iter {
                *entity = mapping.remap(*entity);
            }
        });
        processor.register_entity_inspector::<Self>(|relation| {
            relation.iter().map(|(_, entity)| entity).collect()
        });
        processor.register_formatter::<Self>(|relation, fmt| {
            fmt.debug_struct("Relation")
                .field(
                    "entities",
                    &relation
                        .iter()
                        .map(|(_, entity)| entity)
                        .collect::<Vec<_>>(),
                )
                .finish_non_exhaustive()
        });
    }

    pub fn register_to_processor_debug(processor: &mut WorldProcessor)
    where
        T: std::fmt::Debug,
    {
        processor.register_debug_formatter::<Self>();
    }

    pub fn new(payload: T, entity: Entity) -> Self {
        Self::default().with(payload, entity)
    }

    pub fn with(mut self, payload: T, entity: Entity) -> Self {
        self.add(payload, entity);
        self
    }

    pub fn type_hash(&self) -> TypeHash {
        TypeHash::of::<T>()
    }

    pub fn is_empty(&self) -> bool {
        match &self.connections {
            RelationConnections::Zero(_) => true,
            RelationConnections::One(_) => false,
            RelationConnections::More(vec) => vec.is_empty(),
        }
    }

    pub fn add(&mut self, payload: T, entity: Entity) {
        self.connections = match std::mem::take(&mut self.connections) {
            RelationConnections::Zero(_) => RelationConnections::One([(payload, entity)]),
            RelationConnections::One([a]) => RelationConnections::More(vec![a, (payload, entity)]),
            RelationConnections::More(mut vec) => {
                if let Some(index) = vec.iter().position(|item| item.1 == entity) {
                    vec[index].0 = payload;
                } else {
                    vec.push((payload, entity));
                }
                RelationConnections::More(vec)
            }
        };
    }

    pub fn remove(&mut self, entity: Entity) {
        self.connections = match std::mem::take(&mut self.connections) {
            RelationConnections::Zero(a) => RelationConnections::Zero(a),
            RelationConnections::One([a]) => {
                if a.1 == entity {
                    RelationConnections::Zero([])
                } else {
                    RelationConnections::One([a])
                }
            }
            RelationConnections::More(mut vec) => {
                if let Some(index) = vec.iter().position(|a| a.1 == entity) {
                    vec.swap_remove(index);
                }
                if vec.len() == 1 {
                    RelationConnections::One([vec.remove(0)])
                } else if vec.is_empty() {
                    RelationConnections::Zero([])
                } else {
                    RelationConnections::More(vec)
                }
            }
        }
    }

    pub fn has(&self, entity: Entity) -> bool {
        match &self.connections {
            RelationConnections::Zero(_) => false,
            RelationConnections::One([a]) => a.1 == entity,
            RelationConnections::More(vec) => vec.iter().any(|(_, e)| *e == entity),
        }
    }

    pub fn payload(&self, entity: Entity) -> Option<&T> {
        match &self.connections {
            RelationConnections::Zero(_) => None,
            RelationConnections::One([a]) => {
                if a.1 == entity {
                    Some(&a.0)
                } else {
                    None
                }
            }
            RelationConnections::More(vec) => {
                vec.iter()
                    .find_map(|(p, e)| if *e == entity { Some(p) } else { None })
            }
        }
    }

    pub fn payload_mut(&mut self, entity: Entity) -> Option<&mut T> {
        match &mut self.connections {
            RelationConnections::Zero(_) => None,
            RelationConnections::One([a]) => {
                if a.1 == entity {
                    Some(&mut a.0)
                } else {
                    None
                }
            }
            RelationConnections::More(vec) => {
                vec.iter_mut()
                    .find_map(|(p, e)| if *e == entity { Some(p) } else { None })
            }
        }
    }

    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        match &self.connections {
            RelationConnections::Zero(a) => a.iter(),
            RelationConnections::One(a) => a.iter(),
            RelationConnections::More(vec) => vec.iter(),
        }
        .map(|(_, e)| *e)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&T, Entity)> {
        match &self.connections {
            RelationConnections::Zero(a) => a.iter(),
            RelationConnections::One(a) => a.iter(),
            RelationConnections::More(vec) => vec.iter(),
        }
        .map(|(p, e)| (p, *e))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&mut T, Entity)> {
        match &mut self.connections {
            RelationConnections::Zero(a) => a.iter_mut(),
            RelationConnections::One(a) => a.iter_mut(),
            RelationConnections::More(vec) => vec.iter_mut(),
        }
        .map(|(p, e)| (p, *e))
    }
}

pub struct RelationsTraverseIter<'a, const LOCKING: bool, T: Component> {
    world: &'a World,
    incoming: bool,
    stack: VecDeque<Entity>,
    visited: HashSet<Entity>,
    _phantom: PhantomData<fn() -> T>,
}

impl<const LOCKING: bool, T: Component> Iterator for RelationsTraverseIter<'_, LOCKING, T> {
    type Item = Entity;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(entity) = self.stack.pop_front() {
            if self.visited.contains(&entity) {
                continue;
            }
            self.visited.insert(entity);
            if self.incoming {
                for (entity, _, _) in self.world.relations_incomming::<LOCKING, T>(entity) {
                    if self.stack.len() == self.stack.capacity() {
                        self.stack.reserve_exact(self.stack.capacity());
                    }
                    self.stack.push_back(entity);
                }
            } else {
                for (_, _, entity) in self.world.relations_outgoing::<LOCKING, T>(entity) {
                    if self.stack.len() == self.stack.capacity() {
                        self.stack.reserve_exact(self.stack.capacity());
                    }
                    self.stack.push_back(entity);
                }
            }
            return Some(entity);
        }
        None
    }
}

#[derive(Default, Clone)]
pub struct WorldChanges {
    table: HashMap<Entity, Vec<TypeHash>>,
}

impl WorldChanges {
    pub fn clear(&mut self) {
        self.table.clear();
    }

    pub fn has_entity(&self, entity: Entity) -> bool {
        self.table.contains_key(&entity)
    }

    pub fn has_entity_component<T>(&self, entity: Entity) -> bool {
        self.has_entity_component_raw(entity, TypeHash::of::<T>())
    }

    pub fn has_entity_component_raw(&self, entity: Entity, type_hash: TypeHash) -> bool {
        self.table
            .get(&entity)
            .map(|components| components.contains(&type_hash))
            .unwrap_or_default()
    }

    pub fn has_component<T>(&self) -> bool {
        self.has_component_raw(TypeHash::of::<T>())
    }

    pub fn has_component_raw(&self, type_hash: TypeHash) -> bool {
        self.table
            .values()
            .any(|components| components.contains(&type_hash))
    }

    pub fn iter(&self) -> impl Iterator<Item = (Entity, &[TypeHash])> {
        self.table
            .iter()
            .map(|(entity, components)| (*entity, components.as_slice()))
    }

    pub fn iter_of<T>(&self) -> impl Iterator<Item = Entity> + '_ {
        self.iter_of_raw(TypeHash::of::<T>())
    }

    pub fn iter_of_raw(&self, type_hash: TypeHash) -> impl Iterator<Item = Entity> + '_ {
        self.table
            .iter()
            .filter(move |(_, components)| components.contains(&type_hash))
            .map(|(entity, _)| *entity)
    }
}

pub struct World {
    pub new_archetype_capacity: usize,
    entities: EntityMap,
    archetypes: ArchetypeMap,
    added: WorldChanges,
    removed: WorldChanges,
    updated: Arc<RwLock<WorldChanges>>,
}

impl Default for World {
    fn default() -> Self {
        World {
            new_archetype_capacity: 128,
            entities: Default::default(),
            archetypes: Default::default(),
            added: Default::default(),
            removed: Default::default(),
            updated: Default::default(),
        }
    }
}

impl World {
    #[inline]
    pub fn with_new_archetype_capacity(mut self, value: usize) -> Self {
        self.new_archetype_capacity = value;
        self
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    #[inline]
    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entities.iter()
    }

    #[inline]
    pub(crate) fn entity_archetype_id(&self, entity: Entity) -> Result<u32, WorldError> {
        self.entities.get(entity)
    }

    #[inline]
    pub fn archetypes(&self) -> impl Iterator<Item = &Archetype> {
        self.archetypes.iter()
    }

    #[inline]
    pub fn archetypes_mut(&mut self) -> impl Iterator<Item = &mut Archetype> {
        self.archetypes.iter_mut()
    }

    #[inline]
    pub(crate) fn archetype_by_id(&self, id: u32) -> Result<&Archetype, WorldError> {
        self.archetypes.get(id)
    }

    pub fn added(&self) -> &WorldChanges {
        &self.added
    }

    pub fn removed(&self) -> &WorldChanges {
        &self.removed
    }

    pub fn updated(&self) -> Option<RwLockReadGuard<WorldChanges>> {
        self.updated.try_read().ok()
    }

    pub fn entity_did_changed(&self, entity: Entity) -> bool {
        self.added.has_entity(entity)
            || self.removed.has_entity(entity)
            || self
                .updated
                .try_read()
                .ok()
                .map(|updated| updated.has_entity(entity))
                .unwrap_or_default()
    }

    pub fn component_did_changed<T>(&self) -> bool {
        self.component_did_changed_raw(TypeHash::of::<T>())
    }

    pub fn component_did_changed_raw(&self, type_hash: TypeHash) -> bool {
        self.added.has_component_raw(type_hash)
            || self.removed.has_component_raw(type_hash)
            || self
                .updated
                .try_read()
                .ok()
                .map(|updated| updated.has_component_raw(type_hash))
                .unwrap_or_default()
    }

    pub fn entity_component_did_changed<T>(&self, entity: Entity) -> bool {
        self.entity_component_did_changed_raw(entity, TypeHash::of::<T>())
    }

    pub fn entity_component_did_changed_raw(&self, entity: Entity, type_hash: TypeHash) -> bool {
        self.added.has_entity_component_raw(entity, type_hash)
            || self.removed.has_entity_component_raw(entity, type_hash)
            || self
                .updated
                .try_read()
                .ok()
                .map(|updated| updated.has_entity_component_raw(entity, type_hash))
                .unwrap_or_default()
    }

    pub fn update<T>(&self, entity: Entity) {
        self.update_raw(entity, TypeHash::of::<T>());
    }

    pub fn update_raw(&self, entity: Entity, type_hash: TypeHash) {
        if let Ok(mut updated) = self.updated.try_write() {
            let components = updated.table.entry(entity).or_default();
            if !components.contains(&type_hash) {
                components.push(type_hash);
            }
        }
    }

    pub fn clear_changes(&mut self) {
        self.added.clear();
        self.removed.clear();
        if let Ok(mut updated) = self.updated.try_write() {
            updated.clear();
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.clear_changes();
        self.archetypes.clear();
        self.entities.clear();
    }

    pub fn spawn(&mut self, bundle: impl Bundle) -> Result<Entity, WorldError> {
        let bundle_columns = bundle.columns();
        if bundle_columns.is_empty() {
            return Err(WorldError::EmptyColumnSet);
        }
        let bundle_types = bundle_columns
            .iter()
            .map(|column| column.type_hash())
            .collect::<Vec<_>>();
        let (entity, id) = self.entities.acquire()?;
        let id = if let Some(archetype_id) = self.archetypes.find_by_columns_exact(&bundle_columns)
        {
            *id = Some(archetype_id);
            archetype_id
        } else {
            let (archetype_id, archetype_slot) = match self.archetypes.acquire() {
                Ok(result) => result,
                Err(error) => {
                    self.entities.release(entity)?;
                    return Err(error);
                }
            };
            let archetype = match Archetype::new(bundle_columns, self.new_archetype_capacity) {
                Ok(result) => result,
                Err(error) => {
                    self.entities.release(entity)?;
                    return Err(error.into());
                }
            };
            *archetype_slot = Some(archetype);
            *id = Some(archetype_id);
            archetype_id
        };
        let archetype = match self.archetypes.get_mut(id) {
            Ok(result) => result,
            Err(error) => {
                self.entities.release(entity)?;
                return Err(error);
            }
        };
        match archetype.insert(entity, bundle) {
            Ok(_) => {
                self.added
                    .table
                    .entry(entity)
                    .or_default()
                    .extend(bundle_types);
                Ok(entity)
            }
            Err(error) => {
                self.entities.release(entity)?;
                Err(error.into())
            }
        }
    }

    /// # Safety
    pub unsafe fn spawn_uninitialized<T: BundleColumns>(
        &mut self,
    ) -> Result<(Entity, ArchetypeEntityRowAccess), WorldError> {
        self.spawn_uninitialized_raw(T::columns_static())
    }

    /// # Safety
    pub unsafe fn spawn_uninitialized_raw(
        &mut self,
        columns: Vec<ArchetypeColumnInfo>,
    ) -> Result<(Entity, ArchetypeEntityRowAccess), WorldError> {
        if columns.is_empty() {
            return Err(WorldError::EmptyColumnSet);
        }
        let bundle_types = columns
            .iter()
            .map(|column| column.type_hash())
            .collect::<Vec<_>>();
        let (entity, id) = self.entities.acquire()?;
        let id = if let Some(archetype_id) = self.archetypes.find_by_columns_exact(&columns) {
            *id = Some(archetype_id);
            archetype_id
        } else {
            let (archetype_id, archetype_slot) = match self.archetypes.acquire() {
                Ok(result) => result,
                Err(error) => {
                    self.entities.release(entity)?;
                    return Err(error);
                }
            };
            let archetype = match Archetype::new(columns, self.new_archetype_capacity) {
                Ok(result) => result,
                Err(error) => {
                    self.entities.release(entity)?;
                    return Err(error.into());
                }
            };
            *archetype_slot = Some(archetype);
            *id = Some(archetype_id);
            archetype_id
        };
        let archetype = match self.archetypes.get_mut(id) {
            Ok(result) => result,
            Err(error) => {
                self.entities.release(entity)?;
                return Err(error);
            }
        };
        match archetype.add(entity) {
            Ok(result) => {
                self.added
                    .table
                    .entry(entity)
                    .or_default()
                    .extend(bundle_types);
                Ok((entity, result))
            }
            Err(error) => {
                self.entities.release(entity)?;
                Err(error.into())
            }
        }
    }

    pub fn despawn(&mut self, entity: Entity) -> Result<(), WorldError> {
        let id = self.entities.release(entity)?;
        let archetype = self.archetypes.get_mut(id).unwrap();
        match archetype.remove(entity) {
            Ok(_) => {
                self.removed
                    .table
                    .entry(entity)
                    .or_default()
                    .extend(archetype.columns().map(|column| column.type_hash()));
                Ok(())
            }
            Err(error) => {
                self.entities.acquire()?;
                Err(error.into())
            }
        }
    }

    /// # Safety
    pub unsafe fn despawn_uninitialized(&mut self, entity: Entity) -> Result<(), WorldError> {
        let id = self.entities.release(entity)?;
        let archetype = self.archetypes.get_mut(id).unwrap();
        match archetype.remove_uninitialized(entity) {
            Ok(_) => {
                self.removed
                    .table
                    .entry(entity)
                    .or_default()
                    .extend(archetype.columns().map(|column| column.type_hash()));
                Ok(())
            }
            Err(error) => {
                self.entities.acquire()?;
                Err(error.into())
            }
        }
    }

    pub fn insert(&mut self, entity: Entity, bundle: impl Bundle) -> Result<(), WorldError> {
        let bundle_columns = bundle.columns();
        if bundle_columns.is_empty() {
            return Err(WorldError::EmptyColumnSet);
        }
        let bundle_types = bundle_columns
            .iter()
            .map(|column| column.type_hash())
            .collect::<Vec<_>>();
        let old_id = self.entities.get(entity)?;
        let mut new_columns = self
            .archetypes
            .get_mut(old_id)?
            .columns()
            .cloned()
            .collect::<Vec<_>>();
        for column in bundle_columns {
            if !new_columns
                .iter()
                .any(|c| c.type_hash() == column.type_hash())
            {
                new_columns.push(column);
            }
        }
        if let Some(new_id) = self.archetypes.find_by_columns_exact(&new_columns) {
            if new_id == old_id {
                return Ok(());
            }
            let [old_archetype, new_archetype] = self.archetypes.get_mut_two([old_id, new_id])?;
            let access = old_archetype.transfer(new_archetype, entity)?;
            bundle.initialize_into(&access);
            self.entities.set(entity, new_id)?;
        } else {
            let mut archetype = Archetype::new(new_columns, self.new_archetype_capacity)?;
            let access = self
                .archetypes
                .get_mut(old_id)
                .unwrap()
                .transfer(&mut archetype, entity)?;
            bundle.initialize_into(&access);
            drop(access);
            let (new_id, archetype_slot) = self.archetypes.acquire()?;
            *archetype_slot = Some(archetype);
            self.entities.set(entity, new_id)?;
        }
        self.added
            .table
            .entry(entity)
            .or_default()
            .extend(bundle_types);
        Ok(())
    }

    pub fn remove<T: BundleColumns>(&mut self, entity: Entity) -> Result<(), WorldError> {
        self.remove_raw(entity, T::columns_static())
    }

    pub fn remove_raw(
        &mut self,
        entity: Entity,
        columns: Vec<ArchetypeColumnInfo>,
    ) -> Result<(), WorldError> {
        if columns.is_empty() {
            return Err(WorldError::EmptyColumnSet);
        }
        let bundle_types = columns
            .iter()
            .map(|column| column.type_hash())
            .collect::<Vec<_>>();
        let old_id = self.entities.get(entity)?;
        let mut new_columns = self
            .archetypes
            .get_mut(old_id)?
            .columns()
            .cloned()
            .collect::<Vec<_>>();
        let despawn = new_columns.is_empty();
        for column in columns {
            if let Some(index) = new_columns
                .iter()
                .position(|c| c.type_hash() == column.type_hash())
            {
                new_columns.swap_remove(index);
            }
        }
        if let Some(new_id) = self.archetypes.find_by_columns_exact(&new_columns) {
            if new_id == old_id {
                return Ok(());
            }
            let [old_archetype, new_archetype] = self.archetypes.get_mut_two([old_id, new_id])?;
            old_archetype.transfer(new_archetype, entity)?;
            self.entities.set(entity, new_id)?;
        } else {
            let mut archetype = Archetype::new(new_columns, self.new_archetype_capacity)?;
            self.archetypes
                .get_mut(old_id)
                .unwrap()
                .transfer(&mut archetype, entity)?;
            let (new_id, archetype_slot) = self.archetypes.acquire()?;
            *archetype_slot = Some(archetype);
            self.entities.set(entity, new_id)?;
        }
        if despawn {
            let _ = self.entities.release(entity);
        }
        self.removed
            .table
            .entry(entity)
            .or_default()
            .extend(bundle_types);
        Ok(())
    }

    pub fn merge<const LOCKING: bool>(
        &mut self,
        mut other: Self,
        processor: &WorldProcessor,
    ) -> Result<(), WorldError> {
        let mut mappings = HashMap::<_, _>::with_capacity(other.len());
        let mut archetype_offsets = Vec::with_capacity(other.archetypes().count());
        for archetype_from in other.archetypes_mut() {
            let columns = archetype_from.columns().cloned().collect::<Vec<_>>();
            let archetype_id =
                if let Some(archetype_id) = self.archetypes.find_by_columns_exact(&columns) {
                    archetype_id
                } else {
                    let (archetype_id, archetype_slot) = self.archetypes.acquire()?;
                    let archetype = Archetype::new(columns.clone(), self.new_archetype_capacity)?;
                    *archetype_slot = Some(archetype);
                    archetype_id
                };
            let archetype = self.archetypes.get_mut(archetype_id)?;
            let offset = archetype.len();
            let entities_from = archetype_from.entities().iter().collect::<Vec<_>>();
            for entity_from in entities_from {
                let (entity, access) = unsafe { self.spawn_uninitialized_raw(columns.clone())? };
                let access_from = match archetype_from.row::<LOCKING>(entity_from) {
                    Ok(access_from) => access_from,
                    Err(error) => {
                        drop(access);
                        unsafe { self.despawn_uninitialized(entity)? };
                        return Err(error.into());
                    }
                };
                for column in &columns {
                    unsafe {
                        let data = access.data(column.type_hash()).unwrap();
                        let data_from = access_from.data(column.type_hash()).unwrap();
                        data.copy_from(data_from, column.layout().size());
                    }
                }
                mappings.insert(entity_from, entity);
            }
            archetype_offsets.push((columns, offset));
            unsafe { archetype_from.clear_uninitialized() };
        }
        for (columns, offset) in archetype_offsets {
            if let Some(id) = self.archetypes.find_by_columns_exact(&columns) {
                let archetype = self.archetype_by_id(id)?;
                for column in archetype.columns() {
                    let access = archetype.dynamic_column::<LOCKING>(column.type_hash(), true)?;
                    for index in offset..archetype.len() {
                        unsafe {
                            processor.remap_entities_raw(
                                column.type_hash(),
                                access.data(index)?,
                                WorldProcessorEntityMapping::new(&mappings),
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn has_entity(&self, entity: Entity) -> bool {
        self.entities.get(entity).is_ok()
    }

    pub fn has_entity_component<T: Component>(&self, entity: Entity) -> bool {
        self.has_entity_component_raw(entity, TypeHash::of::<T>())
    }

    pub fn has_entity_component_raw(&self, entity: Entity, component: TypeHash) -> bool {
        self.entities
            .get(entity)
            .and_then(|index| self.archetypes.get(index))
            .map(|archetype| archetype.has_type(component))
            .unwrap_or_default()
    }

    pub fn find_by<const LOCKING: bool, T: Component + PartialEq>(
        &self,
        data: &T,
    ) -> Option<Entity> {
        for (entity, component) in self.query::<LOCKING, (Entity, &T)>() {
            if component == data {
                return Some(entity);
            }
        }
        None
    }

    pub fn component<const LOCKING: bool, T: Component>(
        &self,
        entity: Entity,
    ) -> Result<ComponentRef<LOCKING, T>, WorldError> {
        Ok(ComponentRef {
            inner: self.get::<LOCKING, T>(entity, false)?,
        })
    }

    pub fn component_mut<const LOCKING: bool, T: Component>(
        &self,
        entity: Entity,
    ) -> Result<ComponentRefMut<LOCKING, T>, WorldError> {
        Ok(ComponentRefMut {
            inner: self.get::<LOCKING, T>(entity, true)?,
        })
    }

    pub fn get<const LOCKING: bool, T: Component>(
        &self,
        entity: Entity,
        unique: bool,
    ) -> Result<ArchetypeEntityColumnAccess<LOCKING, T>, WorldError> {
        Ok(self
            .archetypes
            .get(self.entities.get(entity)?)?
            .entity::<LOCKING, T>(entity, unique)?)
    }

    pub fn dynamic_get<const LOCKING: bool>(
        &self,
        type_hash: TypeHash,
        entity: Entity,
        unique: bool,
    ) -> Result<ArchetypeDynamicEntityColumnAccess<LOCKING>, WorldError> {
        Ok(self
            .archetypes
            .get(self.entities.get(entity)?)?
            .dynamic_entity::<LOCKING>(type_hash, entity, unique)?)
    }

    pub fn row<const LOCKING: bool>(
        &self,
        entity: Entity,
    ) -> Result<ArchetypeEntityRowAccess, WorldError> {
        Ok(self
            .archetypes
            .get(self.entities.get(entity)?)?
            .row::<LOCKING>(entity)?)
    }

    pub fn query<'a, const LOCKING: bool, Fetch: TypedQueryFetch<'a, LOCKING>>(
        &'a self,
    ) -> TypedQueryIter<'a, LOCKING, Fetch> {
        TypedQueryIter::new(self)
    }

    pub fn dynamic_query<'a, const LOCKING: bool>(
        &'a self,
        filter: &DynamicQueryFilter,
    ) -> DynamicQueryIter<'a, LOCKING> {
        DynamicQueryIter::new(filter, self)
    }

    pub fn relate<const LOCKING: bool, T: Component>(
        &mut self,
        payload: T,
        from: Entity,
        to: Entity,
    ) -> Result<(), WorldError> {
        if let Ok(mut relation) = self.get::<LOCKING, Relation<T>>(from, true) {
            if let Some(relation) = relation.write() {
                relation.add(payload, to);
            }
            return Ok(());
        }
        self.insert(from, (Relation::<T>::default().with(payload, to),))
    }

    pub fn unrelate<const LOCKING: bool, T: Component>(
        &mut self,
        from: Entity,
        to: Entity,
    ) -> Result<(), WorldError> {
        let remove = if let Ok(mut relation) = self.get::<LOCKING, Relation<T>>(from, true) {
            if let Some(relation) = relation.write() {
                relation.remove(to);
                relation.is_empty()
            } else {
                false
            }
        } else {
            false
        };
        if remove {
            self.remove::<(Relation<T>,)>(from)?;
        }
        Ok(())
    }

    pub fn unrelate_any<const LOCKING: bool, T: Component>(
        &mut self,
        entity: Entity,
    ) -> Result<(), WorldError> {
        let to_remove = self
            .query::<LOCKING, (Entity, &mut Relation<T>)>()
            .filter_map(|(e, relation)| {
                relation.remove(entity);
                if relation.is_empty() {
                    Some(e)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        for entity in to_remove {
            self.remove::<(Relation<T>,)>(entity)?;
        }
        Ok(())
    }

    pub fn has_relation<const LOCKING: bool, T: Component>(
        &self,
        from: Entity,
        to: Entity,
    ) -> bool {
        self.get::<LOCKING, Relation<T>>(from, false)
            .ok()
            .and_then(|relation| Some(relation.read()?.has(to)))
            .unwrap_or_default()
    }

    pub fn relations<const LOCKING: bool, T: Component>(
        &self,
    ) -> impl Iterator<Item = (Entity, &T, Entity)> + '_ {
        self.query::<LOCKING, (Entity, &Relation<T>)>()
            .flat_map(|(from, relation)| {
                relation
                    .iter()
                    .map(move |(payload, to)| (from, payload, to))
            })
    }

    pub fn relations_mut<const LOCKING: bool, T: Component>(
        &self,
    ) -> impl Iterator<Item = (Entity, &mut T, Entity)> + '_ {
        self.query::<LOCKING, (Entity, &mut Relation<T>)>()
            .flat_map(|(from, relation)| {
                relation
                    .iter_mut()
                    .map(move |(payload, to)| (from, payload, to))
            })
    }

    pub fn relations_outgoing<const LOCKING: bool, T: Component>(
        &self,
        from: Entity,
    ) -> impl Iterator<Item = (Entity, &T, Entity)> + '_ {
        self.query::<LOCKING, (Entity, &Relation<T>)>()
            .filter(move |(entity, _)| *entity == from)
            .flat_map(|(from, relation)| {
                relation
                    .iter()
                    .map(move |(payload, to)| (from, payload, to))
            })
    }

    pub fn relations_outgoing_mut<const LOCKING: bool, T: Component>(
        &self,
        from: Entity,
    ) -> impl Iterator<Item = (Entity, &mut T, Entity)> + '_ {
        self.query::<LOCKING, (Entity, &mut Relation<T>)>()
            .filter(move |(entity, _)| *entity == from)
            .flat_map(|(from, relation)| {
                relation
                    .iter_mut()
                    .map(move |(payload, to)| (from, payload, to))
            })
    }

    pub fn relations_incomming<const LOCKING: bool, T: Component>(
        &self,
        to: Entity,
    ) -> impl Iterator<Item = (Entity, &T, Entity)> + '_ {
        self.query::<LOCKING, (Entity, &Relation<T>)>()
            .flat_map(move |(from, relation)| {
                relation
                    .iter()
                    .filter(move |(_, entity)| *entity == to)
                    .map(move |(payload, to)| (from, payload, to))
            })
    }

    pub fn relations_incomming_mut<const LOCKING: bool, T: Component>(
        &self,
        to: Entity,
    ) -> impl Iterator<Item = (Entity, &mut T, Entity)> + '_ {
        self.query::<LOCKING, (Entity, &mut Relation<T>)>()
            .flat_map(move |(from, relation)| {
                relation
                    .iter_mut()
                    .filter(move |(_, entity)| *entity == to)
                    .map(move |(payload, to)| (from, payload, to))
            })
    }

    pub fn traverse_outgoing<const LOCKING: bool, T: Component>(
        &self,
        entities: impl IntoIterator<Item = Entity>,
    ) -> RelationsTraverseIter<LOCKING, T> {
        RelationsTraverseIter {
            world: self,
            incoming: false,
            stack: entities.into_iter().collect(),
            visited: Default::default(),
            _phantom: Default::default(),
        }
    }

    pub fn traverse_incoming<const LOCKING: bool, T: Component>(
        &self,
        entities: impl IntoIterator<Item = Entity>,
    ) -> RelationsTraverseIter<LOCKING, T> {
        RelationsTraverseIter {
            world: self,
            incoming: true,
            stack: entities.into_iter().collect(),
            visited: Default::default(),
            _phantom: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        commands::{CommandBuffer, DespawnCommand},
        query::{Exclude, Include, Update},
    };
    use std::{
        sync::{Arc, RwLock},
        thread::spawn,
        time::{Duration, Instant},
    };

    #[test]
    fn test_world_changes() {
        let mut world = World::default();
        assert!(world.is_empty());
        assert!(world.spawn(()).is_err());

        let (entity, row) = unsafe { world.spawn_uninitialized::<(u8, u16, u32)>().unwrap() };
        assert_eq!(entity, Entity::new(0, 0).unwrap());
        unsafe { row.initialize(1u8).unwrap() };
        unsafe { row.initialize(2u16).unwrap() };
        unsafe { row.initialize(3u32).unwrap() };
        assert_eq!(*row.read::<u8>().unwrap(), 1);
        assert_eq!(*row.read::<u16>().unwrap(), 2);
        assert_eq!(*row.read::<u32>().unwrap(), 3);
        drop(row);
        world.despawn(entity).unwrap();
        assert!(world.is_empty());

        let entity = world.spawn((1u8, 2u16, 3u32)).unwrap();
        assert_eq!(entity, Entity::new(0, 1).unwrap());
        assert_eq!(
            *world
                .get::<true, u8>(entity, false)
                .unwrap()
                .read()
                .unwrap(),
            1
        );
        assert_eq!(
            *world
                .get::<true, u16>(entity, false)
                .unwrap()
                .read()
                .unwrap(),
            2
        );
        assert_eq!(
            *world
                .get::<true, u32>(entity, false)
                .unwrap()
                .read()
                .unwrap(),
            3
        );
        assert!(world.get::<true, u64>(entity, false).is_err());
        assert_eq!(world.len(), 1);

        world.insert(entity, (4u64,)).unwrap();
        assert_eq!(
            *world
                .get::<true, u8>(entity, false)
                .unwrap()
                .read()
                .unwrap(),
            1
        );
        assert_eq!(
            *world
                .get::<true, u16>(entity, false)
                .unwrap()
                .read()
                .unwrap(),
            2
        );
        assert_eq!(
            *world
                .get::<true, u32>(entity, false)
                .unwrap()
                .read()
                .unwrap(),
            3
        );
        assert_eq!(
            *world
                .get::<true, u64>(entity, false)
                .unwrap()
                .read()
                .unwrap(),
            4
        );

        world.remove::<(u8,)>(entity).unwrap();
        assert!(world.get::<true, u8>(entity, false).is_err());
        assert_eq!(
            *world
                .get::<true, u16>(entity, false)
                .unwrap()
                .read()
                .unwrap(),
            2
        );
        assert_eq!(
            *world
                .get::<true, u32>(entity, false)
                .unwrap()
                .read()
                .unwrap(),
            3
        );
        assert_eq!(
            *world
                .get::<true, u64>(entity, false)
                .unwrap()
                .read()
                .unwrap(),
            4
        );

        world.clear();
        assert!(world.is_empty());
    }

    #[test]
    fn test_world_query() {
        const N: usize = if cfg!(miri) { 10 } else { 1000 };

        let mut world = World::default().with_new_archetype_capacity(N);

        for index in 0..N {
            world.spawn((index as u8,)).unwrap();
        }
        for index in N..(N * 2) {
            world.spawn((index as u8, index as u16)).unwrap();
        }
        for index in (N * 2)..(N * 3) {
            world.spawn((index as u16,)).unwrap();
        }

        for (index, v) in world.query::<true, &u8>().enumerate() {
            assert_eq!(*v, index as u8);
        }

        for (index, item) in world
            .dynamic_query::<true>(&DynamicQueryFilter::default().read::<u8>())
            .enumerate()
        {
            let v = item.read::<u8>().unwrap().read::<u8>().unwrap();
            assert_eq!(*v, index as u8);
        }

        for (index, v) in world.query::<true, &u16>().enumerate() {
            assert_eq!(*v, (index + N) as u16);
        }

        for (index, item) in world
            .dynamic_query::<true>(&DynamicQueryFilter::default().read::<u16>())
            .enumerate()
        {
            let v = item.read::<u16>().unwrap().read::<u16>().unwrap();
            assert_eq!(*v, (index + N) as u16);
        }

        for (index, (entity, a, b)) in world.query::<true, (Entity, &u8, &u16)>().enumerate() {
            assert!(entity.is_valid());
            assert_eq!(*a, (index + N) as u8);
            assert_eq!(*b, (index + N) as u16);
        }

        for (index, item) in world
            .dynamic_query::<true>(&DynamicQueryFilter::default().read::<u8>().read::<u16>())
            .enumerate()
        {
            let a = item.read::<u8>().unwrap().read::<u8>().unwrap();
            let b = item.read::<u16>().unwrap().read::<u16>().unwrap();
            assert!(item.entity().is_valid());
            assert_eq!(*a, (index + N) as u8);
            assert_eq!(*b, (index + N) as u16);
        }

        for (index, (a, b)) in world.query::<true, (&u8, Option<&u16>)>().enumerate() {
            assert_eq!(*a, index as u8);
            if let Some(b) = b {
                assert_eq!(*b, index as u16);
            }
        }

        for (entity, _, _) in world.query::<true, (Entity, &u8, Include<u16>)>() {
            assert!((entity.id() as usize) >= N);
            assert!((entity.id() as usize) < N * 2);
        }

        for item in world
            .dynamic_query::<true>(&DynamicQueryFilter::default().read::<u8>().include::<u16>())
        {
            assert!((item.entity().id() as usize) >= N);
            assert!((item.entity().id() as usize) < N * 2);
        }

        for (entity, _, _) in world.query::<true, (Entity, &u8, Exclude<u16>)>() {
            assert!((entity.id() as usize) < N);
        }

        for item in world
            .dynamic_query::<true>(&DynamicQueryFilter::default().read::<u8>().exclude::<u16>())
        {
            assert!((item.entity().id() as usize) < N);
        }
    }

    #[test]
    fn test_change_detection() {
        let mut world = World::default();

        for index in 0..10usize {
            world.spawn((index,)).unwrap();
        }
        let mut list = world.added().iter_of::<usize>().collect::<Vec<_>>();
        list.sort();
        assert_eq!(
            list,
            (0..10)
                .map(|index| Entity::new(index, 0).unwrap())
                .collect::<Vec<_>>()
        );

        for mut v in world.query::<true, Update<usize>>() {
            *v.write_notified(&world) *= 2;
        }
        for (entity, v) in world.query::<true, (Entity, &usize)>() {
            assert_eq!(entity.id() as usize * 2, *v);
        }
        let mut list = world
            .updated()
            .unwrap()
            .iter_of::<usize>()
            .collect::<Vec<_>>();
        list.sort();
        assert_eq!(
            list,
            (0..10)
                .map(|index| Entity::new(index, 0).unwrap())
                .collect::<Vec<_>>()
        );

        let mut commands = CommandBuffer::default();
        for entity in world.entities() {
            commands.command(DespawnCommand::new(entity));
        }
        commands.execute(&mut world);
        let mut list = world.removed().iter_of::<usize>().collect::<Vec<_>>();
        list.sort();
        assert_eq!(
            list,
            (0..10)
                .map(|index| Entity::new(index, 0).unwrap())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_zst_components() {
        #[derive(Debug, PartialEq, Eq)]
        struct Foo;

        #[derive(Debug, PartialEq, Eq)]
        struct Bar(bool);

        let mut world = World::default();
        world.spawn((Foo,)).unwrap();
        assert_eq!(world.query::<true, &Foo>().count(), 1);
        for v in world.query::<true, &Foo>() {
            assert_eq!(v, &Foo);
        }
        world.spawn((Bar(true),)).unwrap();
        assert_eq!(world.query::<true, &Bar>().count(), 1);
        for v in world.query::<true, &Bar>() {
            assert_eq!(v, &Bar(true));
        }
        world.spawn((Foo, Bar(false))).unwrap();
        assert_eq!(world.query::<true, &Foo>().count(), 2);
        assert_eq!(world.query::<true, &Bar>().count(), 2);
        assert_eq!(world.query::<true, (&Bar, &Foo)>().count(), 1);
        for (a, b) in world.query::<true, (&Bar, &Foo)>() {
            assert_eq!(a, &Bar(false));
            assert_eq!(b, &Foo);
        }
    }

    #[test]
    fn test_world_relations() {
        struct Parent;
        struct Root;

        let mut world = World::default();
        let a = world.spawn((0u8, false, Root)).unwrap();
        let b = world.spawn((1u8, false)).unwrap();
        let c = world.spawn((2u8, false)).unwrap();
        let d = world.spawn((3u8, false)).unwrap();
        world.relate::<true, _>(Parent, b, a).unwrap();
        world.relate::<true, _>(Parent, c, a).unwrap();
        world.relate::<true, _>(Parent, d, c).unwrap();

        assert_eq!(
            world
                .relations_incomming::<true, Parent>(a)
                .map(|(entity, _, _)| entity)
                .collect::<Vec<_>>(),
            vec![b, c]
        );
        assert_eq!(
            world
                .relations_incomming::<true, Parent>(b)
                .map(|(entity, _, _)| entity)
                .collect::<Vec<_>>(),
            vec![]
        );
        assert_eq!(
            world
                .relations_incomming::<true, Parent>(c)
                .map(|(entity, _, _)| entity)
                .collect::<Vec<_>>(),
            vec![d]
        );
        assert_eq!(
            world
                .relations_incomming::<true, Parent>(d)
                .map(|(entity, _, _)| entity)
                .collect::<Vec<_>>(),
            vec![]
        );

        assert_eq!(
            world
                .relations_outgoing::<true, Parent>(a)
                .map(|(_, _, entity)| entity)
                .collect::<Vec<_>>(),
            vec![]
        );
        assert_eq!(
            world
                .relations_outgoing::<true, Parent>(b)
                .map(|(_, _, entity)| entity)
                .collect::<Vec<_>>(),
            vec![a]
        );
        assert_eq!(
            world
                .relations_outgoing::<true, Parent>(c)
                .map(|(_, _, entity)| entity)
                .collect::<Vec<_>>(),
            vec![a]
        );
        assert_eq!(
            world
                .relations_outgoing::<true, Parent>(d)
                .map(|(_, _, entity)| entity)
                .collect::<Vec<_>>(),
            vec![c]
        );

        assert_eq!(
            world
                .traverse_incoming::<true, Parent>([a])
                .collect::<Vec<_>>(),
            vec![a, b, c, d]
        );

        for (entity, _) in world.query::<true, (Entity, Include<Root>)>() {
            for (other, _, _) in world.relations_incomming::<true, Parent>(entity) {
                let mut v = world.get::<true, bool>(other, true).unwrap();
                let v = v.write().unwrap();
                *v = !*v;
            }
        }

        assert!(!*world.get::<true, bool>(a, false).unwrap().read().unwrap());
        assert!(*world.get::<true, bool>(b, false).unwrap().read().unwrap());
        assert!(*world.get::<true, bool>(c, false).unwrap().read().unwrap());
        assert!(!*world.get::<true, bool>(d, false).unwrap().read().unwrap());

        world.unrelate::<true, Parent>(b, a).unwrap();
        world.unrelate::<true, Parent>(c, a).unwrap();
        world.unrelate::<true, Parent>(d, c).unwrap();
        assert!(world.query::<true, &Relation<Parent>>().count() == 0);
    }

    #[test]
    fn test_world_async() {
        const N: usize = if cfg!(miri) { 10 } else { 1000 };

        fn is_async<T: Send + Sync>() {}

        is_async::<World>();

        let world = Arc::new(RwLock::new(World::default().with_new_archetype_capacity(N)));
        let world2 = world.clone();

        {
            let mut world = world.write().unwrap();
            for index in 0..N {
                world.spawn((index as u8, index as u16)).unwrap();
            }
        }

        let handle = spawn(move || {
            let timer = Instant::now();
            while timer.elapsed() < Duration::from_secs(1) {
                let world = world2.read().unwrap();
                for v in world.query::<true, &mut u16>() {
                    *v = v.wrapping_add(1);
                }
            }
        });

        let timer = Instant::now();
        while timer.elapsed() < Duration::from_secs(1) {
            let world = world.read().unwrap();
            for v in world.query::<true, &mut u8>() {
                *v = v.wrapping_add(1);
            }
        }

        let _ = handle.join();
    }
}
