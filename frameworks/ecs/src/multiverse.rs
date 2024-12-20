use crate::{
    archetype::{
        ArchetypeColumnInfo, ArchetypeDynamicEntityColumnAccess, ArchetypeEntityColumnAccess,
    },
    entity::Entity,
    query::{
        DynamicQueryFilter, DynamicQueryItem, DynamicQueryIter, TypedQueryFetch, TypedQueryIter,
    },
    world::{World, WorldError},
    Component,
};
use intuicio_data::type_hash::TypeHash;
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    ops::{Deref, DerefMut},
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Multity {
    One([Entity; 1]),
    Two([Entity; 2]),
    More(Vec<Entity>),
}

impl Multity {
    pub fn new(entity: Entity) -> Self {
        Self::One([entity])
    }

    pub fn with(mut self, entity: Entity) -> Self {
        self.push(entity);
        self
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        match self {
            Self::One(_) => 1,
            Self::Two(_) => 2,
            Self::More(items) => items.len(),
        }
    }

    pub fn root(&self) -> Entity {
        match self {
            Self::One([entity]) => *entity,
            Self::Two([entity, _]) => *entity,
            Self::More(items) => *items.first().unwrap(),
        }
    }

    pub fn entity(&self) -> Entity {
        match self {
            Self::One([entity]) => *entity,
            Self::Two([_, entity]) => *entity,
            Self::More(items) => *items.last().unwrap(),
        }
    }

    pub fn parent(&self) -> Option<Self> {
        let mut result = self.clone();
        if result.pop().is_some() {
            Some(result)
        } else {
            None
        }
    }

    pub fn push(&mut self, entity: Entity) {
        *self = match std::mem::replace(self, Self::new(Default::default())) {
            Self::One([a]) => Self::Two([a, entity]),
            Self::Two([a, b]) => Self::More(vec![a, b, entity]),
            Self::More(mut items) => {
                items.push(entity);
                Self::More(items)
            }
        }
    }

    pub fn pop(&mut self) -> Option<Entity> {
        match std::mem::replace(self, Self::new(Default::default())) {
            Self::One([a]) => {
                *self = Self::One([a]);
                None
            }
            Self::Two([a, b]) => {
                *self = Self::One([a]);
                Some(b)
            }
            Self::More(mut items) => {
                let result = items.pop()?;
                match items.len() {
                    2 => {
                        *self = Self::Two([items[0], items[1]]);
                    }
                    1 => {
                        *self = Self::One([items[0]]);
                    }
                    _ => {
                        *self = Self::More(items);
                    }
                }
                Some(result)
            }
        }
    }

    pub fn prepend(&mut self, other: impl IntoIterator<Item = Entity>) {
        *self = Self::from_iter(other.into_iter().chain(self.iter()));
    }

    pub fn append(&mut self, other: impl IntoIterator<Item = Entity>) {
        for entity in other.into_iter() {
            self.push(entity);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        match self {
            Self::One(items) => items.as_slice().iter().copied(),
            Self::Two(items) => items.as_slice().iter().copied(),
            Self::More(items) => items.as_slice().iter().copied(),
        }
    }

    pub fn into_inner(self) -> Vec<Entity> {
        self.iter().collect()
    }
}

impl FromIterator<Entity> for Multity {
    fn from_iter<T: IntoIterator<Item = Entity>>(iter: T) -> Self {
        let mut iter = iter.into_iter();
        let entity = iter.next().unwrap();
        let mut result = Self::new(entity);
        for entity in iter {
            result.push(entity);
        }
        result
    }
}

pub struct ArchetypeMultityColumnAccess<'a, const LOCKING: bool, T: Component> {
    _worlds: Vec<ArchetypeEntityColumnAccess<'a, LOCKING, World>>,
    entity: ArchetypeEntityColumnAccess<'a, LOCKING, T>,
}

impl<const LOCKING: bool, T: Component> ArchetypeMultityColumnAccess<'_, LOCKING, T> {
    #[inline]
    pub fn info(&self) -> &ArchetypeColumnInfo {
        self.entity.info()
    }

    #[inline]
    pub fn is_unique(&self) -> bool {
        self.entity.is_unique()
    }

    /// # Safety
    #[inline]
    pub unsafe fn data(&self) -> *mut u8 {
        self.entity.data()
    }

    pub fn read(&self) -> Option<&T> {
        self.entity.read()
    }

    pub fn write(&mut self) -> Option<&mut T> {
        self.entity.write()
    }
}

pub struct ArchetypeDynamicMultityColumnAccess<'a, const LOCKING: bool> {
    _worlds: Vec<ArchetypeEntityColumnAccess<'a, LOCKING, World>>,
    entity: ArchetypeDynamicEntityColumnAccess<'a, LOCKING>,
}

impl<const LOCKING: bool> ArchetypeDynamicMultityColumnAccess<'_, LOCKING> {
    #[inline]
    pub fn info(&self) -> &ArchetypeColumnInfo {
        self.entity.info()
    }

    #[inline]
    pub fn is_unique(&self) -> bool {
        self.entity.is_unique()
    }

    /// # Safety
    #[inline]
    pub unsafe fn data(&self) -> *mut u8 {
        self.entity.data()
    }

    pub fn read<T: Component>(&self) -> Option<&T> {
        self.entity.read::<T>()
    }

    pub fn write<T: Component>(&mut self) -> Option<&mut T> {
        self.entity.write::<T>()
    }
}

pub struct HyperComponentRef<'a, const LOCKING: bool, T: Send + Sync + 'static> {
    inner: ArchetypeMultityColumnAccess<'a, LOCKING, T>,
}

impl<const LOCKING: bool, T: Send + Sync + 'static> Deref for HyperComponentRef<'_, LOCKING, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.read().unwrap()
    }
}

pub struct HyperComponentRefMut<'a, const LOCKING: bool, T: Send + Sync + 'static> {
    inner: ArchetypeMultityColumnAccess<'a, LOCKING, T>,
}

impl<const LOCKING: bool, T: Send + Sync + 'static> Deref for HyperComponentRefMut<'_, LOCKING, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.read().unwrap()
    }
}

impl<const LOCKING: bool, T: Send + Sync + 'static> DerefMut
    for HyperComponentRefMut<'_, LOCKING, T>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.write().unwrap()
    }
}

pub struct MultiverseTypedQueryIter<'a, const LOCKING: bool, Fetch: TypedQueryFetch<'a, LOCKING>> {
    queries: VecDeque<TypedQueryIter<'a, LOCKING, Fetch>>,
}

impl<'a, const LOCKING: bool, Fetch: TypedQueryFetch<'a, LOCKING>> Default
    for MultiverseTypedQueryIter<'a, LOCKING, Fetch>
{
    fn default() -> Self {
        Self {
            queries: Default::default(),
        }
    }
}

impl<'a, const LOCKING: bool, Fetch: TypedQueryFetch<'a, LOCKING>>
    MultiverseTypedQueryIter<'a, LOCKING, Fetch>
{
    pub fn new(world: &'a World) -> Self {
        let mut result = Self::default();
        result.include(world);
        result
    }

    pub fn include(&mut self, world: &'a World) {
        self.queries.push_back(world.query::<'a, LOCKING, Fetch>());
        for world in world.query::<'a, LOCKING, &World>() {
            self.include(world);
        }
    }
}

impl<'a, const LOCKING: bool, Fetch: TypedQueryFetch<'a, LOCKING>> Iterator
    for MultiverseTypedQueryIter<'a, LOCKING, Fetch>
{
    type Item = Fetch::Value;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(query) = self.queries.back_mut() {
                if let Some(result) = query.next() {
                    return Some(result);
                }
                self.queries.pop_back();
                continue;
            }
            break;
        }
        None
    }
}

pub struct MultiverseDynamicQueryIter<'a, const LOCKING: bool> {
    queries: VecDeque<DynamicQueryIter<'a, LOCKING>>,
}

impl<const LOCKING: bool> Default for MultiverseDynamicQueryIter<'_, LOCKING> {
    fn default() -> Self {
        Self {
            queries: Default::default(),
        }
    }
}

impl<'a, const LOCKING: bool> MultiverseDynamicQueryIter<'a, LOCKING> {
    pub fn new(filter: &DynamicQueryFilter, world: &'a World) -> Self {
        let mut result = Self::default();
        result.include(filter, world);
        result
    }

    pub fn include(&mut self, filter: &DynamicQueryFilter, world: &'a World) {
        self.queries
            .push_back(world.dynamic_query::<LOCKING>(filter));
        for world in world.query::<'a, LOCKING, &World>() {
            self.include(filter, world);
        }
    }
}

impl<'a, const LOCKING: bool> Iterator for MultiverseDynamicQueryIter<'a, LOCKING> {
    type Item = DynamicQueryItem<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(query) = self.queries.back_mut() {
                if let Some(result) = query.next() {
                    return Some(result);
                }
                self.queries.pop_back();
                continue;
            }
            break;
        }
        None
    }
}

pub struct MultiverseMultityTypedQueryIter<
    'a,
    const LOCKING: bool,
    Fetch: TypedQueryFetch<'a, LOCKING>,
> {
    /// [(parent world?, query)]
    #[allow(clippy::type_complexity)]
    queries: VecDeque<(
        Option<Multity>,
        TypedQueryIter<'a, LOCKING, (Entity, Fetch)>,
    )>,
}

impl<'a, const LOCKING: bool, Fetch: TypedQueryFetch<'a, LOCKING>> Default
    for MultiverseMultityTypedQueryIter<'a, LOCKING, Fetch>
{
    fn default() -> Self {
        Self {
            queries: Default::default(),
        }
    }
}

impl<'a, const LOCKING: bool, Fetch: TypedQueryFetch<'a, LOCKING>>
    MultiverseMultityTypedQueryIter<'a, LOCKING, Fetch>
{
    pub fn new(world: &'a World) -> Self {
        let mut result = Self::default();
        result.include(world, None);
        result
    }

    fn include(&mut self, world: &'a World, parent: Option<Multity>) {
        self.queries.push_back((
            parent.clone(),
            world.query::<'a, LOCKING, (Entity, Fetch)>(),
        ));
        for (entity, world) in world.query::<'a, LOCKING, (Entity, &World)>() {
            let parent = if let Some(parent) = parent.as_ref() {
                parent.clone().with(entity)
            } else {
                Multity::new(entity)
            };
            self.include(world, Some(parent));
        }
    }
}

impl<'a, const LOCKING: bool, Fetch: TypedQueryFetch<'a, LOCKING>> Iterator
    for MultiverseMultityTypedQueryIter<'a, LOCKING, Fetch>
{
    type Item = (Multity, Fetch::Value);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some((parent, query)) = self.queries.back_mut() {
                if let Some((entity, result)) = query.next() {
                    let multity = if let Some(parent) = parent.as_ref() {
                        parent.clone().with(entity)
                    } else {
                        Multity::new(entity)
                    };
                    return Some((multity, result));
                }
                self.queries.pop_back();
                continue;
            }
            break;
        }
        None
    }
}

pub struct MultiverseMultityDynamicQueryIter<'a, const LOCKING: bool> {
    /// [(parent world?, query)]
    queries: VecDeque<(Option<Multity>, DynamicQueryIter<'a, LOCKING>)>,
}

impl<const LOCKING: bool> Default for MultiverseMultityDynamicQueryIter<'_, LOCKING> {
    fn default() -> Self {
        Self {
            queries: Default::default(),
        }
    }
}

impl<'a, const LOCKING: bool> MultiverseMultityDynamicQueryIter<'a, LOCKING> {
    pub fn new(filter: &DynamicQueryFilter, world: &'a World) -> Self {
        let mut result = Self::default();
        result.include(filter, world, None);
        result
    }

    fn include(&mut self, filter: &DynamicQueryFilter, world: &'a World, parent: Option<Multity>) {
        self.queries
            .push_back((parent.clone(), world.dynamic_query::<LOCKING>(filter)));
        for (entity, world) in world.query::<'a, LOCKING, (Entity, &World)>() {
            let parent = if let Some(parent) = parent.as_ref() {
                parent.clone().with(entity)
            } else {
                Multity::new(entity)
            };
            self.include(filter, world, Some(parent));
        }
    }
}

impl<'a, const LOCKING: bool> Iterator for MultiverseMultityDynamicQueryIter<'a, LOCKING> {
    type Item = (Multity, DynamicQueryItem<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some((parent, query)) = self.queries.back_mut() {
                if let Some(result) = query.next() {
                    let multity = if let Some(parent) = parent.as_ref() {
                        parent.clone().with(result.entity())
                    } else {
                        Multity::new(result.entity())
                    };
                    return Some((multity, result));
                }
                self.queries.pop_back();
                continue;
            }
            break;
        }
        None
    }
}

pub struct Multiverse<'a> {
    pub world: &'a World,
}

impl<'a> Multiverse<'a> {
    pub fn new(world: &'a World) -> Self {
        Self { world }
    }

    pub fn component<const LOCKING: bool, T: Component>(
        &self,
        multity: Multity,
    ) -> Result<HyperComponentRef<LOCKING, T>, WorldError> {
        Ok(HyperComponentRef {
            inner: self.get::<LOCKING, T>(multity, false)?,
        })
    }

    pub fn component_mut<const LOCKING: bool, T: Component>(
        &self,
        multity: Multity,
    ) -> Result<HyperComponentRefMut<LOCKING, T>, WorldError> {
        Ok(HyperComponentRefMut {
            inner: self.get::<LOCKING, T>(multity, true)?,
        })
    }

    pub fn get<const LOCKING: bool, T: Component>(
        &self,
        multity: Multity,
        unique: bool,
    ) -> Result<ArchetypeMultityColumnAccess<'a, LOCKING, T>, WorldError> {
        let mut worlds =
            Vec::<ArchetypeEntityColumnAccess<LOCKING, World>>::with_capacity(multity.len());
        let mut iter = multity.iter().peekable();
        while let Some(entity) = iter.next() {
            if iter.peek().is_none() {
                let entity = if let Some(access) = worlds.last() {
                    let world =
                        unsafe { std::mem::transmute::<&World, &'a World>(access.read().unwrap()) };
                    world.get::<LOCKING, T>(entity, unique)?
                } else {
                    self.world.get::<LOCKING, T>(entity, unique)?
                };
                return Ok(ArchetypeMultityColumnAccess {
                    _worlds: worlds,
                    entity,
                });
            }
            let world = if let Some(access) = worlds.last() {
                let world =
                    unsafe { std::mem::transmute::<&World, &'a World>(access.read().unwrap()) };
                world.get::<LOCKING, World>(entity, unique)?
            } else {
                self.world.get::<LOCKING, World>(entity, unique)?
            };
            worlds.push(world);
        }
        unreachable!()
    }

    pub fn dynamic_get<const LOCKING: bool>(
        &self,
        type_hash: TypeHash,
        multity: Multity,
        unique: bool,
    ) -> Result<ArchetypeDynamicMultityColumnAccess<LOCKING>, WorldError> {
        let mut worlds =
            Vec::<ArchetypeEntityColumnAccess<LOCKING, World>>::with_capacity(multity.len());
        let mut iter = multity.iter().peekable();
        while let Some(entity) = iter.next() {
            if iter.peek().is_none() {
                let entity = if let Some(access) = worlds.last() {
                    let world =
                        unsafe { std::mem::transmute::<&World, &'a World>(access.read().unwrap()) };
                    world.dynamic_get::<LOCKING>(type_hash, entity, unique)?
                } else {
                    self.world
                        .dynamic_get::<LOCKING>(type_hash, entity, unique)?
                };
                return Ok(ArchetypeDynamicMultityColumnAccess {
                    _worlds: worlds,
                    entity,
                });
            }
            let world = if let Some(access) = worlds.last() {
                let world =
                    unsafe { std::mem::transmute::<&World, &'a World>(access.read().unwrap()) };
                world.get::<LOCKING, World>(entity, unique)?
            } else {
                self.world.get::<LOCKING, World>(entity, unique)?
            };
            worlds.push(world);
        }
        unreachable!()
    }

    pub fn query<'b, const LOCKING: bool, Fetch: TypedQueryFetch<'b, LOCKING>>(
        &'b self,
    ) -> MultiverseTypedQueryIter<'b, LOCKING, Fetch> {
        MultiverseTypedQueryIter::new(self.world)
    }

    pub fn multity_query<'b, const LOCKING: bool, Fetch: TypedQueryFetch<'b, LOCKING>>(
        &'b self,
    ) -> MultiverseMultityTypedQueryIter<'b, LOCKING, Fetch> {
        MultiverseMultityTypedQueryIter::new(self.world)
    }

    pub fn dynamic_query<'b, const LOCKING: bool>(
        &'b self,
        filter: &DynamicQueryFilter,
    ) -> MultiverseDynamicQueryIter<'a, LOCKING> {
        MultiverseDynamicQueryIter::new(filter, self.world)
    }

    pub fn dynamic_multity_query<'b, const LOCKING: bool>(
        &'b self,
        filter: &DynamicQueryFilter,
    ) -> MultiverseMultityDynamicQueryIter<'a, LOCKING> {
        MultiverseMultityDynamicQueryIter::new(filter, self.world)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{query::Include, world::World};

    #[test]
    fn test_multiverse() {
        let mut world1 = World::default();
        let a = world1.spawn((1usize,)).unwrap();
        let b = world1.spawn((2usize,)).unwrap();
        let c = world1.spawn((3usize,)).unwrap();

        let mut world2 = World::default();
        let d = world2.spawn((4usize,)).unwrap();
        let e = world2.spawn((5usize,)).unwrap();
        let f = world2.spawn((world1,)).unwrap();

        let mut world3 = World::default();
        let g = world3.spawn((6usize,)).unwrap();
        let h = world3.spawn((world2,)).unwrap();

        let mut world = World::default();
        let i = world.spawn((world3,)).unwrap();

        assert_eq!(
            Multiverse::new(&world)
                .query::<true, &usize>()
                .copied()
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5, 6]
        );

        assert_eq!(
            Multiverse::new(&world)
                .dynamic_query::<true>(&DynamicQueryFilter::default().read::<usize>())
                .map(|item| *item.read::<usize>().unwrap().read::<usize>().unwrap())
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5, 6]
        );

        let multity = Multity::from_iter([i, h, f, b]);
        *Multiverse::new(&world)
            .component_mut::<true, usize>(multity.clone())
            .unwrap() = 10;
        assert_eq!(
            *Multiverse::new(&world)
                .component::<true, usize>(multity.clone())
                .unwrap(),
            10
        );

        assert_eq!(
            Multiverse::new(&world)
                .query::<true, &usize>()
                .copied()
                .collect::<Vec<_>>(),
            vec![1, 10, 3, 4, 5, 6]
        );

        assert_eq!(
            Multiverse::new(&world)
                .multity_query::<true, Include<usize>>()
                .map(|(multity, _)| multity)
                .collect::<Vec<_>>(),
            vec![
                Multity::from_iter([i, h, f, a]),
                Multity::from_iter([i, h, f, b]),
                Multity::from_iter([i, h, f, c]),
                Multity::from_iter([i, h, d]),
                Multity::from_iter([i, h, e]),
                Multity::from_iter([i, g]),
            ]
        );

        assert_eq!(
            Multiverse::new(&world)
                .dynamic_multity_query::<true>(&DynamicQueryFilter::default().include::<usize>())
                .map(|(multity, _)| multity)
                .collect::<Vec<_>>(),
            vec![
                Multity::from_iter([i, h, f, a]),
                Multity::from_iter([i, h, f, b]),
                Multity::from_iter([i, h, f, c]),
                Multity::from_iter([i, h, d]),
                Multity::from_iter([i, h, e]),
                Multity::from_iter([i, g]),
            ]
        );
    }
}
