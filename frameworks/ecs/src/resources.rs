use crate::{
    archetype::ArchetypeColumnInfo,
    bundle::{Bundle, BundleColumns},
    query::{TypedQueryFetch, TypedQueryIter},
    world::{World, WorldChanges, WorldError},
    Component, ComponentRef, ComponentRefMut,
};
use intuicio_data::type_hash::TypeHash;
use std::{error::Error, sync::RwLockReadGuard};

#[derive(Default)]
pub struct Resources {
    world: World,
}

impl Resources {
    pub fn add(&mut self, bundle: impl Bundle) -> Result<(), Box<dyn Error>> {
        let entity = self.world.entities().next();
        if let Some(entity) = entity {
            WorldError::allow(
                self.world.insert(entity, bundle),
                [WorldError::EmptyColumnSet],
                (),
            )?;
        } else {
            self.world.spawn(bundle)?;
        }
        Ok(())
    }

    pub fn remove<T: BundleColumns>(&mut self) -> Result<(), Box<dyn Error>> {
        let entity = self.world.entities().next();
        if let Some(entity) = entity {
            self.world.remove::<T>(entity)?;
        }
        Ok(())
    }

    pub fn remove_raw(&mut self, columns: Vec<ArchetypeColumnInfo>) -> Result<(), Box<dyn Error>> {
        let entity = self.world.entities().next();
        if let Some(entity) = entity {
            self.world.remove_raw(entity, columns)?;
        }
        Ok(())
    }

    pub fn clear(&mut self) {
        self.world.clear();
    }

    pub fn clear_changes(&mut self) {
        self.world.clear_changes();
    }

    pub fn added(&self) -> &WorldChanges {
        self.world.added()
    }

    pub fn removed(&self) -> &WorldChanges {
        self.world.removed()
    }

    pub fn updated(&self) -> Option<RwLockReadGuard<'_, WorldChanges>> {
        self.world.updated()
    }

    pub fn did_changed<T: Component>(&self) -> bool {
        self.world.component_did_changed::<T>()
    }

    pub fn did_changed_raw(&self, type_hash: TypeHash) -> bool {
        self.world.component_did_changed_raw(type_hash)
    }

    pub fn get<const LOCKING: bool, T: Component>(
        &self,
    ) -> Result<ComponentRef<LOCKING, T>, Box<dyn Error>> {
        let entity = self.world.entities().next().unwrap_or_default();
        Ok(self.world.component(entity)?)
    }

    pub fn get_mut<const LOCKING: bool, T: Component>(
        &self,
    ) -> Result<ComponentRefMut<LOCKING, T>, Box<dyn Error>> {
        let entity = self.world.entities().next().unwrap_or_default();
        Ok(self.world.component_mut(entity)?)
    }

    pub fn query<'a, const LOCKING: bool, Fetch: TypedQueryFetch<'a, LOCKING>>(
        &'a self,
    ) -> TypedQueryIter<'a, LOCKING, Fetch> {
        self.world.query::<LOCKING, Fetch>()
    }
}
