pub mod actor;
pub mod archetype;
pub mod bundle;
pub mod commands;
pub mod entity;
pub mod multiverse;
pub mod observer;
pub mod prefab;
pub mod processor;
pub mod query;
pub mod resources;
pub mod scheduler;
pub mod systems;
pub mod universe;
pub mod world;

pub mod prelude {
    pub use crate::{
        commands::*, entity::*, query::*, resources::*, scheduler::*, systems::*, universe::*,
        world::*, Component, ComponentRef, ComponentRefMut,
    };
}

use crate::archetype::ArchetypeEntityColumnAccess;
use std::ops::{Deref, DerefMut};

pub trait Component: Send + Sync + 'static {}

impl<T: Send + Sync + 'static> Component for T {}

pub struct ComponentRef<'a, const LOCKING: bool, T: Send + Sync + 'static> {
    inner: ArchetypeEntityColumnAccess<'a, LOCKING, T>,
}

impl<const LOCKING: bool, T: Send + Sync + 'static> Deref for ComponentRef<'_, LOCKING, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.read().unwrap()
    }
}

pub struct ComponentRefMut<'a, const LOCKING: bool, T: Send + Sync + 'static> {
    inner: ArchetypeEntityColumnAccess<'a, LOCKING, T>,
}

impl<const LOCKING: bool, T: Send + Sync + 'static> Deref for ComponentRefMut<'_, LOCKING, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.read().unwrap()
    }
}

impl<const LOCKING: bool, T: Send + Sync + 'static> DerefMut for ComponentRefMut<'_, LOCKING, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.write().unwrap()
    }
}
