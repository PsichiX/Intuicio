use crate::{commands::CommandBuffer, entity::Entity, world::World, Component};
use intuicio_data::type_hash::TypeHash;
use std::collections::HashMap;

#[derive(Default)]
pub struct ChangeObserver {
    pub commands: CommandBuffer,
    #[allow(clippy::type_complexity)]
    on_added:
        HashMap<TypeHash, Vec<Box<dyn FnMut(&World, &mut CommandBuffer, Entity) + Send + Sync>>>,
    #[allow(clippy::type_complexity)]
    on_removed:
        HashMap<TypeHash, Vec<Box<dyn FnMut(&World, &mut CommandBuffer, Entity) + Send + Sync>>>,
    #[allow(clippy::type_complexity)]
    on_updated:
        HashMap<TypeHash, Vec<Box<dyn FnMut(&World, &mut CommandBuffer, Entity) + Send + Sync>>>,
}

impl ChangeObserver {
    pub fn on_added<T: Component>(
        &mut self,
        callback: impl FnMut(&World, &mut CommandBuffer, Entity) + Send + Sync + 'static,
    ) {
        self.on_added_raw(TypeHash::of::<T>(), callback);
    }

    pub fn on_added_raw(
        &mut self,
        type_hash: TypeHash,
        callback: impl FnMut(&World, &mut CommandBuffer, Entity) + Send + Sync + 'static,
    ) {
        self.on_added
            .entry(type_hash)
            .or_default()
            .push(Box::new(callback));
    }

    pub fn on_removed<T: Component>(
        &mut self,
        callback: impl FnMut(&World, &mut CommandBuffer, Entity) + Send + Sync + 'static,
    ) {
        self.on_removed_raw(TypeHash::of::<T>(), callback);
    }

    pub fn on_removed_raw(
        &mut self,
        type_hash: TypeHash,
        callback: impl FnMut(&World, &mut CommandBuffer, Entity) + Send + Sync + 'static,
    ) {
        self.on_removed
            .entry(type_hash)
            .or_default()
            .push(Box::new(callback));
    }

    pub fn on_updated<T: Component>(
        &mut self,
        callback: impl FnMut(&World, &mut CommandBuffer, Entity) + Send + Sync + 'static,
    ) {
        self.on_updated_raw(TypeHash::of::<T>(), callback);
    }

    pub fn on_updated_raw(
        &mut self,
        type_hash: TypeHash,
        callback: impl FnMut(&World, &mut CommandBuffer, Entity) + Send + Sync + 'static,
    ) {
        self.on_updated
            .entry(type_hash)
            .or_default()
            .push(Box::new(callback));
    }

    pub fn process(&mut self, world: &mut World) {
        for (entity, types) in world.added().iter() {
            for type_hash in types {
                if let Some(listeners) = self.on_added.get_mut(type_hash) {
                    for listener in listeners {
                        listener(world, &mut self.commands, entity);
                    }
                }
            }
        }
        if let Some(updated) = world.updated() {
            for (entity, types) in updated.iter() {
                for type_hash in types {
                    if let Some(listeners) = self.on_updated.get_mut(type_hash) {
                        for listener in listeners {
                            listener(world, &mut self.commands, entity);
                        }
                    }
                }
            }
        }
        for (entity, types) in world.removed().iter() {
            for type_hash in types {
                if let Some(listeners) = self.on_removed.get_mut(type_hash) {
                    for listener in listeners {
                        listener(world, &mut self.commands, entity);
                    }
                }
            }
        }
    }

    pub fn process_execute(&mut self, world: &mut World) {
        self.process(world);
        self.commands.execute(world);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{commands::DespawnCommand, world::World};
    use std::sync::{Arc, RwLock};

    #[test]
    fn test_async() {
        fn is_async<T: Send + Sync>() {}

        is_async::<ChangeObserver>();
    }

    #[test]
    fn test_change_observer() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Phase {
            None,
            Added,
            Updated,
            Removed,
        }

        let phase = Arc::new(RwLock::new(Phase::None));
        let phase1 = phase.clone();
        let phase2 = phase.clone();
        let phase3 = phase.clone();

        let mut observer = ChangeObserver::default();
        observer.on_added::<bool>(move |_, commands, entity| {
            let phase1 = phase1.clone();
            // normally you don't need to schedule this code, but it helps here in tests
            // to test for separate phases. Without it you go from None to Updated phase.
            commands.schedule(move |world| {
                let mut access = world.get::<true, bool>(entity, true).unwrap();
                let data = access.write().unwrap();
                *data = !*data;
                world.update::<bool>(entity);
                *phase1.write().unwrap() = Phase::Added;
            });
        });
        observer.on_updated::<bool>(move |_, commands, entity| {
            commands.command(DespawnCommand::new(entity));
            *phase2.write().unwrap() = Phase::Updated;
        });
        observer.on_removed::<bool>(move |_, _, _| {
            *phase3.write().unwrap() = Phase::Removed;
        });

        let mut world = World::default();
        let entity = world.spawn((false,)).unwrap();
        assert!(!*world
            .get::<true, bool>(entity, false)
            .unwrap()
            .read()
            .unwrap());
        assert_eq!(*phase.read().unwrap(), Phase::None);

        observer.process(&mut world);
        world.clear_changes();
        observer.commands.execute(&mut world);
        assert_eq!(*phase.read().unwrap(), Phase::Added);

        observer.process(&mut world);
        world.clear_changes();
        observer.commands.execute(&mut world);
        assert_eq!(*phase.read().unwrap(), Phase::Updated);

        observer.process(&mut world);
        world.clear_changes();
        observer.commands.execute(&mut world);
        assert_eq!(*phase.read().unwrap(), Phase::Removed);
    }
}
