use crate::{
    bundle::{Bundle, BundleColumns},
    entity::Entity,
    world::World,
};
use std::{
    marker::PhantomData,
    sync::{Arc, Mutex},
};

pub trait Command: Send + Sync + 'static {
    fn execute(self, world: &mut World);
}

#[derive(Default)]
pub struct CommandBuffer {
    #[allow(clippy::type_complexity)]
    commands: Vec<Box<dyn FnOnce(&mut World) + Send + Sync>>,
}

impl CommandBuffer {
    pub fn schedule(&mut self, command: impl FnOnce(&mut World) + Send + Sync + 'static) {
        self.commands.push(Box::new(command));
    }

    pub fn command(&mut self, command: impl Command) {
        self.schedule(|world| command.execute(world));
    }

    pub fn commands(&mut self, mut buffer: CommandBuffer) {
        self.schedule(move |world| {
            buffer.execute(world);
        });
    }

    pub fn execute(&mut self, world: &mut World) {
        for command in std::mem::take(&mut self.commands) {
            (command)(world);
        }
    }

    pub fn clear(&mut self) {
        self.commands.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }
}

#[derive(Default, Clone)]
pub struct SharedCommandBuffer {
    inner: Arc<Mutex<CommandBuffer>>,
}

impl SharedCommandBuffer {
    pub fn with<R>(&mut self, f: impl FnOnce(&mut CommandBuffer) -> R) -> Option<R> {
        if let Ok(mut buffer) = self.inner.lock() {
            Some(f(&mut buffer))
        } else {
            None
        }
    }

    pub fn try_with<R>(&mut self, f: impl FnOnce(&mut CommandBuffer) -> R) -> Option<R> {
        if let Ok(mut buffer) = self.inner.try_lock() {
            Some(f(&mut buffer))
        } else {
            None
        }
    }
}

pub struct SpawnCommand<T: Bundle + Send + Sync + 'static> {
    bundle: T,
}

impl<T: Bundle + Send + Sync + 'static> SpawnCommand<T> {
    pub fn new(bundle: T) -> Self {
        Self { bundle }
    }
}

impl<T: Bundle + Send + Sync + 'static> Command for SpawnCommand<T> {
    fn execute(self, world: &mut World) {
        world.spawn(self.bundle).unwrap();
    }
}

pub struct DespawnCommand {
    entity: Entity,
}

impl DespawnCommand {
    pub fn new(entity: Entity) -> Self {
        Self { entity }
    }
}

impl Command for DespawnCommand {
    fn execute(self, world: &mut World) {
        world.despawn(self.entity).unwrap();
    }
}

pub struct InsertCommand<T: Bundle + Send + Sync + 'static> {
    entity: Entity,
    bundle: T,
}

impl<T: Bundle + Send + Sync + 'static> InsertCommand<T> {
    pub fn new(entity: Entity, bundle: T) -> Self {
        Self { entity, bundle }
    }
}

impl<T: Bundle + Send + Sync + 'static> Command for InsertCommand<T> {
    fn execute(self, world: &mut World) {
        world.insert(self.entity, self.bundle).unwrap();
    }
}

pub struct RemoveCommand<T: BundleColumns> {
    entity: Entity,
    _phantom: PhantomData<fn() -> T>,
}

impl<T: Bundle + Send + Sync + 'static> RemoveCommand<T> {
    pub fn new(entity: Entity) -> Self {
        Self {
            entity,
            _phantom: PhantomData,
        }
    }
}

impl<T: Bundle + Send + Sync + 'static> Command for RemoveCommand<T> {
    fn execute(self, world: &mut World) {
        world.remove::<T>(self.entity).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;

    #[test]
    fn test_async() {
        fn is_async<T: Send + Sync>() {}

        is_async::<CommandBuffer>();
        is_async::<SharedCommandBuffer>();
    }

    #[test]
    fn test_command_buffer() {
        let mut world = World::default();
        let mut buffer = CommandBuffer::default();
        assert!(world.is_empty());

        buffer.command(SpawnCommand::new((1u8, 2u16, 3u32)));
        buffer.execute(&mut world);
        assert_eq!(world.len(), 1);

        let entity = world.entities().next().unwrap();
        buffer.command(DespawnCommand::new(entity));
        buffer.execute(&mut world);
        assert!(world.is_empty());
    }
}
