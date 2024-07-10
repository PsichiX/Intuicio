use intuicio_data::type_hash::TypeHash;

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
use std::error::Error;

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

pub struct World {
    pub new_archetype_capacity: usize,
    entities: EntityMap,
    archetypes: ArchetypeMap,
}

impl Default for World {
    fn default() -> Self {
        World {
            new_archetype_capacity: 128,
            entities: Default::default(),
            archetypes: Default::default(),
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

    #[inline]
    pub fn clear(&mut self) {
        self.archetypes.clear();
        self.entities.clear();
    }

    pub fn spawn(&mut self, bundle: impl Bundle) -> Result<Entity, WorldError> {
        let bundle_columns = bundle.columns();
        if bundle_columns.is_empty() {
            return Err(WorldError::EmptyColumnSet);
        }
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
            Ok(_) => Ok(entity),
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
            Ok(result) => Ok((entity, result)),
            Err(error) => {
                self.entities.release(entity)?;
                Err(error.into())
            }
        }
    }

    pub fn despawn(&mut self, entity: Entity) -> Result<(), WorldError> {
        let id = self.entities.release(entity)?;
        match self.archetypes.get_mut(id).unwrap().remove(entity) {
            Ok(result) => Ok(result),
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
        let old_id = self.entities.get(entity)?;
        let mut new_columns = self
            .archetypes
            .get_mut(old_id)?
            .columns()
            .cloned()
            .collect::<Vec<_>>();
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::{Exclude, Include};

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

    // #[test]
    // fn test_world_async() {
    //     const N: usize = if cfg!(miri) { 10 } else { 1000 };

    //     fn is_async<T: Send + Sync>() {}

    //     is_async::<World>();

    //     let world = Arc::new(RwLock::new(World::default().with_new_archetype_capacity(N)));
    //     let world2 = world.clone();

    //     {
    //         let mut world = world.write().unwrap();
    //         for index in 0..N {
    //             world.spawn((index as u8, index as u16)).unwrap();
    //         }
    //     }

    //     let handle = spawn(move || {
    //         let timer = Instant::now();
    //         while timer.elapsed() < Duration::from_secs(1) {
    //             let world = world2.read().unwrap();
    //             for mut v in world.query::<true, &mut u16>() {
    //                 *v = v.wrapping_add(1);
    //             }
    //         }
    //     });

    //     let timer = Instant::now();
    //     while timer.elapsed() < Duration::from_secs(1) {
    //         let world = world.read().unwrap();
    //         for mut v in world.query::<true, &mut u8>() {
    //             *v = v.wrapping_add(1);
    //         }
    //     }

    //     let _ = handle.join();
    // }
}
