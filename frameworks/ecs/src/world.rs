use crate::{
    archetype::{
        Archetype, ArchetypeColumnInfo, ArchetypeDynamicEntityColumnAccess,
        ArchetypeEntityColumnAccess, ArchetypeEntityRowAccess, ArchetypeError,
    },
    bundle::{Bundle, BundleColumns},
    entity::Entity,
    query::{DynamicQueryFilter, DynamicQueryIter, TypedQueryFetch, TypedQueryIter},
    Component,
};
use intuicio_data::type_hash::TypeHash;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    error::Error,
    sync::{Arc, RwLock, RwLockReadGuard},
};

#[derive(Debug)]
pub enum WorldError {
    Archetype(ArchetypeError),
    ReachedEntityIdCapacity,
    ReachedArchetypeIdCapacity,
    EntityDoesNotExists { entity: Entity },
    ArchetypeDoesNotExists { id: u32 },
    DuplicateMutableArchetypeAccess { id: u32 },
    EmptyColumnSet,
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

    fn get_mut_many<const N: usize>(
        &mut self,
        id: [u32; N],
    ) -> Result<[&mut Archetype; N], WorldError> {
        for (index_a, a) in id.iter().copied().enumerate() {
            for (index_b, b) in id.iter().copied().enumerate() {
                if index_a != index_b && a == b {
                    return Err(WorldError::DuplicateMutableArchetypeAccess { id: a });
                }
            }
            if let Some(archetype) = self.table.get(a as usize) {
                if archetype.is_none() {
                    return Err(WorldError::ArchetypeDoesNotExists { id: a });
                }
            }
        }
        Ok(std::array::from_fn(|index| unsafe {
            &mut *(self.table[id[index] as usize].as_mut().unwrap() as *mut Archetype)
        }))
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

#[derive(Default)]
struct RelationsGraph {
    graph: Vec<(TypeHash, Entity, Entity)>,
}

impl RelationsGraph {
    fn connect(&mut self, type_hash: TypeHash, from: Entity, to: Entity) {
        if from != to
            && !self
                .graph
                .iter()
                .any(|(ty, f, t)| *ty == type_hash && *f == from && *t == to)
        {
            if self.graph.len() == self.graph.capacity() {
                self.graph.reserve_exact(self.graph.capacity());
            }
            self.graph.push((type_hash, from, to));
        }
    }

    fn disconnect(&mut self, type_hash: TypeHash, from: Entity, to: Entity) {
        if let Some(index) = self
            .graph
            .iter()
            .position(|(ty, f, t)| *ty == type_hash && *f == from && *t == to)
        {
            self.graph.swap_remove(index);
        }
    }

    fn disconnect_from(&mut self, type_hash: TypeHash, from: Entity) {
        self.graph
            .retain(|(ty, f, _)| *ty != type_hash || *f != from);
    }

    fn disconnect_to(&mut self, type_hash: TypeHash, to: Entity) {
        self.graph.retain(|(ty, _, t)| *ty != type_hash || *t != to);
    }

    fn disconnect_all(&mut self, type_hash: TypeHash, entity: Entity) {
        self.graph
            .retain(|(ty, f, t)| *ty != type_hash || (*f != entity && *t != entity));
    }

    fn disconnect_any(&mut self, entity: Entity) {
        self.graph.retain(|(_, f, t)| *f != entity && *t != entity);
    }

    fn connections(
        &self,
        type_hash: TypeHash,
        entity: Entity,
    ) -> impl Iterator<Item = Entity> + '_ {
        self.graph.iter().filter_map(move |(ty, f, t)| {
            if *ty == type_hash {
                if *f == entity {
                    Some(*t)
                } else if *t == entity {
                    Some(*f)
                } else {
                    None
                }
            } else {
                None
            }
        })
    }

    fn outgoing(&self, type_hash: TypeHash, entity: Entity) -> impl Iterator<Item = Entity> + '_ {
        self.graph.iter().filter_map(move |(ty, f, t)| {
            if *ty == type_hash && *f == entity {
                Some(*t)
            } else {
                None
            }
        })
    }

    fn incoming(&self, type_hash: TypeHash, entity: Entity) -> impl Iterator<Item = Entity> + '_ {
        self.graph.iter().filter_map(move |(ty, f, t)| {
            if *ty == type_hash && *t == entity {
                Some(*f)
            } else {
                None
            }
        })
    }
}

pub struct RelationsTraverseIter<'a> {
    relations: &'a RelationsGraph,
    type_hash: TypeHash,
    incoming: bool,
    stack: VecDeque<Entity>,
    visited: HashSet<Entity>,
}

impl<'a> Iterator for RelationsTraverseIter<'a> {
    type Item = Entity;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(entity) = self.stack.pop_front() {
            if self.visited.contains(&entity) {
                continue;
            }
            self.visited.insert(entity);
            if self.incoming {
                for entity in self.relations.incoming(self.type_hash, entity) {
                    if self.stack.len() == self.stack.capacity() {
                        self.stack.reserve_exact(self.stack.capacity());
                    }
                    self.stack.push_back(entity);
                }
            } else {
                for entity in self.relations.outgoing(self.type_hash, entity) {
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

#[derive(Default)]
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
}

pub struct World {
    pub new_archetype_capacity: usize,
    entities: EntityMap,
    archetypes: ArchetypeMap,
    relations: RelationsGraph,
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
            relations: Default::default(),
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
    pub fn archetypes(&self) -> impl Iterator<Item = &Archetype> {
        self.archetypes.iter()
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
                self.relations.disconnect_any(entity);
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
            let [old_archetype, new_archetype] = self.archetypes.get_mut_many([old_id, new_id])?;
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
            let [old_archetype, new_archetype] = self.archetypes.get_mut_many([old_id, new_id])?;
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
            self.relations.disconnect_any(entity);
        }
        self.removed
            .table
            .entry(entity)
            .or_default()
            .extend(bundle_types);
        Ok(())
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

    pub fn relate<T>(&mut self, from: Entity, to: Entity) {
        self.relate_raw(TypeHash::of::<T>(), from, to);
    }

    pub fn relate_raw(&mut self, type_hash: TypeHash, from: Entity, to: Entity) {
        self.relations.connect(type_hash, from, to);
    }

    pub fn unrelate<T>(&mut self, from: Entity, to: Entity) {
        self.unrelate_raw(TypeHash::of::<T>(), from, to);
    }

    pub fn unrelate_raw(&mut self, type_hash: TypeHash, from: Entity, to: Entity) {
        self.relations.disconnect(type_hash, from, to);
    }

    pub fn unrelate_from<T>(&mut self, from: Entity) {
        self.unrelate_from_raw(TypeHash::of::<T>(), from);
    }

    pub fn unrelate_from_raw(&mut self, type_hash: TypeHash, from: Entity) {
        self.relations.disconnect_from(type_hash, from);
    }

    pub fn unrelate_to<T>(&mut self, to: Entity) {
        self.unrelate_to_raw(TypeHash::of::<T>(), to);
    }

    pub fn unrelate_to_raw(&mut self, type_hash: TypeHash, to: Entity) {
        self.relations.disconnect_to(type_hash, to);
    }

    pub fn unrelate_all<T>(&mut self, entity: Entity) {
        self.unrelate_all_raw(TypeHash::of::<T>(), entity);
    }

    pub fn unrelate_all_raw(&mut self, type_hash: TypeHash, entity: Entity) {
        self.relations.disconnect_all(type_hash, entity);
    }

    pub fn unrelate_any(&mut self, entity: Entity) {
        self.relations.disconnect_any(entity);
    }

    pub fn relations<T>(&self, entity: Entity) -> impl Iterator<Item = Entity> + '_ {
        self.relations_raw(TypeHash::of::<T>(), entity)
    }

    pub fn relations_raw(
        &self,
        type_hash: TypeHash,
        entity: Entity,
    ) -> impl Iterator<Item = Entity> + '_ {
        self.relations.connections(type_hash, entity)
    }

    pub fn outgoing_relations<T>(&self, entity: Entity) -> impl Iterator<Item = Entity> + '_ {
        self.outgoing_relations_raw(TypeHash::of::<T>(), entity)
    }

    pub fn outgoing_relations_raw(
        &self,
        type_hash: TypeHash,
        entity: Entity,
    ) -> impl Iterator<Item = Entity> + '_ {
        self.relations.outgoing(type_hash, entity)
    }

    pub fn incoming_relations<T>(&self, entity: Entity) -> impl Iterator<Item = Entity> + '_ {
        self.incoming_relations_raw(TypeHash::of::<T>(), entity)
    }

    pub fn incoming_relations_raw(
        &self,
        type_hash: TypeHash,
        entity: Entity,
    ) -> impl Iterator<Item = Entity> + '_ {
        self.relations.incoming(type_hash, entity)
    }

    pub fn traverse_outgoing<T>(
        &self,
        entities: impl IntoIterator<Item = Entity>,
    ) -> RelationsTraverseIter {
        self.traverse_outgoing_raw(TypeHash::of::<T>(), entities)
    }

    pub fn traverse_outgoing_raw(
        &self,
        type_hash: TypeHash,
        entities: impl IntoIterator<Item = Entity>,
    ) -> RelationsTraverseIter {
        RelationsTraverseIter {
            relations: &self.relations,
            type_hash,
            incoming: false,
            stack: entities.into_iter().collect(),
            visited: Default::default(),
        }
    }

    pub fn traverse_incoming<T>(
        &self,
        entities: impl IntoIterator<Item = Entity>,
    ) -> RelationsTraverseIter {
        self.traverse_incoming_raw(TypeHash::of::<T>(), entities)
    }

    pub fn traverse_incoming_raw(
        &self,
        type_hash: TypeHash,
        entities: impl IntoIterator<Item = Entity>,
    ) -> RelationsTraverseIter {
        RelationsTraverseIter {
            relations: &self.relations,
            type_hash,
            incoming: true,
            stack: entities.into_iter().collect(),
            visited: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::{Exclude, Include, Update};
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
    fn test_updating_queries() {
        let mut world = World::default();
        for index in 0..10usize {
            world.spawn((index,)).unwrap();
        }
        for mut v in world.query::<true, Update<usize>>() {
            *v.write_notified(&world) *= 2;
        }
        for (entity, v) in world.query::<true, (Entity, &usize)>() {
            assert_eq!(entity.id() as usize * 2, *v);
        }
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
        world.relate::<Parent>(b, a);
        world.relate::<Parent>(c, a);
        world.relate::<Parent>(d, c);

        assert_eq!(
            world.incoming_relations::<Parent>(a).collect::<Vec<_>>(),
            vec![b, c]
        );
        assert_eq!(
            world.incoming_relations::<Parent>(b).collect::<Vec<_>>(),
            vec![]
        );
        assert_eq!(
            world.incoming_relations::<Parent>(c).collect::<Vec<_>>(),
            vec![d]
        );
        assert_eq!(
            world.incoming_relations::<Parent>(d).collect::<Vec<_>>(),
            vec![]
        );

        assert_eq!(
            world.outgoing_relations::<Parent>(a).collect::<Vec<_>>(),
            vec![]
        );
        assert_eq!(
            world.outgoing_relations::<Parent>(b).collect::<Vec<_>>(),
            vec![a]
        );
        assert_eq!(
            world.outgoing_relations::<Parent>(c).collect::<Vec<_>>(),
            vec![a]
        );
        assert_eq!(
            world.outgoing_relations::<Parent>(d).collect::<Vec<_>>(),
            vec![c]
        );

        assert_eq!(
            world.traverse_incoming::<Parent>([a]).collect::<Vec<_>>(),
            vec![a, b, c, d]
        );

        for (entity, _) in world.query::<true, (Entity, Include<Root>)>() {
            for other in world.incoming_relations::<Parent>(entity) {
                let mut v = world.get::<true, bool>(other, true).unwrap();
                let v = v.write().unwrap();
                *v = !*v;
            }
        }

        assert!(!*world.get::<true, bool>(a, false).unwrap().read().unwrap());
        assert!(*world.get::<true, bool>(b, false).unwrap().read().unwrap());
        assert!(*world.get::<true, bool>(c, false).unwrap().read().unwrap());
        assert!(!*world.get::<true, bool>(d, false).unwrap().read().unwrap());
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
