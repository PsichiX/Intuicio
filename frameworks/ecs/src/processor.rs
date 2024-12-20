use crate::{
    entity::Entity,
    world::{World, WorldError},
    Component,
};
use intuicio_data::type_hash::TypeHash;
use std::collections::HashMap;

#[derive(Default)]
pub struct WorldProcessor {
    #[allow(clippy::type_complexity)]
    remap_entities:
        HashMap<TypeHash, Box<dyn Fn(*mut u8, WorldProcessorEntityMapping) + Send + Sync>>,
    #[allow(clippy::type_complexity)]
    related_entities: HashMap<TypeHash, Box<dyn Fn(*const u8) -> Vec<Entity> + Send + Sync>>,
    #[allow(clippy::type_complexity)]
    format: HashMap<
        TypeHash,
        Box<dyn Fn(*const u8, &mut std::fmt::Formatter) -> std::fmt::Result + Send + Sync>,
    >,
}

impl WorldProcessor {
    pub fn register_entity_remapping<T: Component>(
        &mut self,
        f: impl Fn(&mut T, WorldProcessorEntityMapping) + Send + Sync + 'static,
    ) {
        self.register_entity_remapping_raw(TypeHash::of::<T>(), move |pointer, mapping| {
            f(unsafe { pointer.cast::<T>().as_mut().unwrap() }, mapping)
        });
    }

    pub fn register_entity_remapping_raw(
        &mut self,
        type_hash: TypeHash,
        f: impl Fn(*mut u8, WorldProcessorEntityMapping) + Send + Sync + 'static,
    ) {
        self.remap_entities.insert(type_hash, Box::new(f));
    }

    pub fn unregister_entity_remapping<T: Component>(&mut self) {
        self.unregister_entity_remapping_raw(TypeHash::of::<T>());
    }

    pub fn unregister_entity_remapping_raw(&mut self, type_hash: TypeHash) {
        self.remap_entities.remove(&type_hash);
    }

    pub fn remap_entities<T>(&self, data: &mut T, mappings: WorldProcessorEntityMapping) {
        unsafe {
            self.remap_entities_raw(TypeHash::of::<T>(), data as *mut T as *mut u8, mappings);
        }
    }

    /// # Safety
    pub unsafe fn remap_entities_raw(
        &self,
        type_hash: TypeHash,
        pointer: *mut u8,
        mappings: WorldProcessorEntityMapping,
    ) {
        if let Some(remapper) = self.remap_entities.get(&type_hash) {
            remapper(pointer, mappings);
        }
    }

    pub fn register_entity_inspector<T: Component>(
        &mut self,
        f: impl Fn(&T) -> Vec<Entity> + Send + Sync + 'static,
    ) {
        self.register_entity_inspector_raw(TypeHash::of::<T>(), move |pointer| {
            f(unsafe { pointer.cast::<T>().as_ref().unwrap() })
        });
    }

    pub fn register_entity_inspector_raw(
        &mut self,
        type_hash: TypeHash,
        f: impl Fn(*const u8) -> Vec<Entity> + Send + Sync + 'static,
    ) {
        self.related_entities.insert(type_hash, Box::new(f));
    }

    pub fn unregister_entity_inspector<T: Component>(&mut self) {
        self.unregister_entity_inspector_raw(TypeHash::of::<T>());
    }

    pub fn unregister_entity_inspector_raw(&mut self, type_hash: TypeHash) {
        self.related_entities.remove(&type_hash);
    }

    pub fn related_entities<T>(&self, data: &T) -> Vec<Entity> {
        unsafe { self.related_entities_raw(TypeHash::of::<T>(), data as *const T as *const u8) }
    }

    /// # Safety
    pub unsafe fn related_entities_raw(
        &self,
        type_hash: TypeHash,
        pointer: *const u8,
    ) -> Vec<Entity> {
        if let Some(inspector) = self.related_entities.get(&type_hash) {
            inspector(pointer)
        } else {
            Default::default()
        }
    }

    pub fn all_related_entities<const LOCKING: bool>(
        &self,
        world: &World,
        entities: impl IntoIterator<Item = Entity>,
        output: &mut Vec<Entity>,
    ) -> Result<(), WorldError> {
        let mut stack = entities.into_iter().collect::<Vec<_>>();
        while let Some(entity) = stack.pop() {
            if !output.contains(&entity) {
                output.push(entity);
                let row = world.row::<LOCKING>(entity)?;
                for type_hash in row.types() {
                    unsafe {
                        let data = row.data(type_hash)?;
                        stack.extend(self.related_entities_raw(type_hash, data));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn register_display_formatter<T: Component + std::fmt::Display>(&mut self) {
        self.register_formatter::<T>(|data, fmt| data.fmt(fmt));
    }

    pub fn register_debug_formatter<T: Component + std::fmt::Debug>(&mut self) {
        self.register_formatter::<T>(|data, fmt| data.fmt(fmt));
    }

    pub fn register_formatter<T: Component>(
        &mut self,
        f: impl Fn(&T, &mut std::fmt::Formatter) -> std::fmt::Result + Send + Sync + 'static,
    ) {
        self.register_formatter_raw(TypeHash::of::<T>(), move |pointer, fmt| {
            f(unsafe { pointer.cast::<T>().as_ref().unwrap() }, fmt)
        });
    }

    pub fn register_formatter_raw(
        &mut self,
        type_hash: TypeHash,
        f: impl Fn(*const u8, &mut std::fmt::Formatter) -> std::fmt::Result + Send + Sync + 'static,
    ) {
        self.format.insert(type_hash, Box::new(f));
    }

    pub fn unregister_formatter<T: Component>(&mut self) {
        self.unregister_formatter_raw(TypeHash::of::<T>());
    }

    pub fn unregister_formatter_raw(&mut self, type_hash: TypeHash) {
        self.format.remove(&type_hash);
    }

    pub fn format_component<'a, T: Component>(
        &'a self,
        data: &'a T,
    ) -> WorldProcessorComponentFormat<'a, T> {
        WorldProcessorComponentFormat {
            processor: self,
            data,
        }
    }

    /// # Safety
    pub unsafe fn format_component_raw(
        &self,
        type_hash: TypeHash,
        pointer: *const u8,
    ) -> WorldProcessorComponentFormatRaw<'_> {
        WorldProcessorComponentFormatRaw {
            processor: self,
            type_hash,
            pointer,
        }
    }

    pub fn format_world<'a, const LOCKING: bool>(
        &'a self,
        world: &'a World,
    ) -> WorldProcessorWorldFormat<'a, LOCKING> {
        WorldProcessorWorldFormat {
            processor: self,
            world,
        }
    }
}

pub struct WorldProcessorEntityMapping<'a> {
    mapping: &'a HashMap<Entity, Entity>,
}

impl<'a> WorldProcessorEntityMapping<'a> {
    pub fn new(mapping: &'a HashMap<Entity, Entity>) -> Self {
        Self { mapping }
    }

    pub fn remap(&self, entity: Entity) -> Entity {
        self.mapping.get(&entity).copied().unwrap_or_default()
    }
}

pub struct WorldProcessorComponentFormat<'a, T: Component> {
    processor: &'a WorldProcessor,
    data: &'a T,
}

impl<T: Component> WorldProcessorComponentFormat<'_, T> {
    pub fn format(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(formatter) = self.processor.format.get(&TypeHash::of::<T>()) {
            formatter(self.data as *const T as *const u8, fmt)
        } else {
            write!(fmt, "<MISSING>")
        }
    }
}

impl<T: Component> std::fmt::Debug for WorldProcessorComponentFormat<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.format(f)
    }
}

impl<T: Component> std::fmt::Display for WorldProcessorComponentFormat<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.format(f)
    }
}

pub struct WorldProcessorComponentFormatRaw<'a> {
    processor: &'a WorldProcessor,
    type_hash: TypeHash,
    pointer: *const u8,
}

impl WorldProcessorComponentFormatRaw<'_> {
    pub fn format(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(formatter) = self.processor.format.get(&self.type_hash) {
            formatter(self.pointer, fmt)
        } else {
            write!(fmt, "<MISSING>")
        }
    }
}

impl std::fmt::Debug for WorldProcessorComponentFormatRaw<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.format(f)
    }
}

impl std::fmt::Display for WorldProcessorComponentFormatRaw<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.format(f)
    }
}

pub struct WorldProcessorWorldFormat<'a, const LOCKING: bool> {
    processor: &'a WorldProcessor,
    world: &'a World,
}

impl<const LOCKING: bool> WorldProcessorWorldFormat<'_, LOCKING> {
    pub fn format(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_list()
            .entries(self.world.entities().map(|entity| {
                let access = self.world.row::<LOCKING>(entity).unwrap();
                WorldProcessorWorldRowFormat {
                    entity,
                    components: access
                        .types()
                        .map(|type_hash| WorldProcessorWorldColumnFormat {
                            processor: self.processor,
                            type_hash,
                            data: unsafe { access.data(type_hash).unwrap() },
                        })
                        .collect(),
                }
            }))
            .finish()
    }
}

impl<const LOCKING: bool> std::fmt::Debug for WorldProcessorWorldFormat<'_, LOCKING> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.format(f)
    }
}

impl<const LOCKING: bool> std::fmt::Display for WorldProcessorWorldFormat<'_, LOCKING> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.format(f)
    }
}

struct WorldProcessorWorldRowFormat<'a> {
    entity: Entity,
    components: Vec<WorldProcessorWorldColumnFormat<'a>>,
}

impl WorldProcessorWorldRowFormat<'_> {
    pub fn format(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_list()
            .entry(&self.entity)
            .entries(self.components.iter())
            .finish()
    }
}

impl std::fmt::Debug for WorldProcessorWorldRowFormat<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.format(f)
    }
}

impl std::fmt::Display for WorldProcessorWorldRowFormat<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.format(f)
    }
}

struct WorldProcessorWorldColumnFormat<'a> {
    processor: &'a WorldProcessor,
    type_hash: TypeHash,
    data: *const u8,
}

impl WorldProcessorWorldColumnFormat<'_> {
    pub fn format(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("Column")
            .field("type_hash", &self.type_hash)
            .field("component", unsafe {
                &self
                    .processor
                    .format_component_raw(self.type_hash, self.data)
            })
            .finish()
    }
}

impl std::fmt::Debug for WorldProcessorWorldColumnFormat<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.format(f)
    }
}

impl std::fmt::Display for WorldProcessorWorldColumnFormat<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.format(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{Relation, World};

    #[test]
    fn test_world_merge() {
        let mut world = World::default();
        world.spawn((10usize,)).unwrap();

        let mut world2 = World::default();
        let a = world2.spawn((42usize,)).unwrap();
        let b = world2.spawn((false, Relation::new((), a))).unwrap();
        world2.spawn((true, Relation::new((), b))).unwrap();

        let mut processor = WorldProcessor::default();
        Relation::<()>::register_to_processor(&mut processor);

        world.merge::<true>(world2, &processor).unwrap();
        let entities = world.entities().collect::<Vec<_>>();
        assert_eq!(entities.len(), 4);
        assert_eq!(*world.component::<true, usize>(entities[0]).unwrap(), 10);
        assert_eq!(*world.component::<true, usize>(entities[1]).unwrap(), 42);
        assert!(!*world.component::<true, bool>(entities[2]).unwrap());
        assert_eq!(
            *world
                .component::<true, Relation<()>>(entities[2])
                .unwrap()
                .iter()
                .map(|(_, entity)| entity)
                .collect::<Vec<_>>(),
            vec![entities[1]]
        );
        assert!(*world.component::<true, bool>(entities[3]).unwrap());
        assert_eq!(
            *world
                .component::<true, Relation<()>>(entities[3])
                .unwrap()
                .iter()
                .map(|(_, entity)| entity)
                .collect::<Vec<_>>(),
            vec![entities[2]]
        );
    }
}
