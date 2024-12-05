use crate::{
    archetype::ArchetypeEntityColumnAccess,
    bundle::Bundle,
    entity::Entity,
    world::{Relation, World, WorldError},
};
use intuicio_core::{context::Context, function::FunctionHandle, registry::Registry};
use intuicio_data::{
    data_stack::DataStackPack,
    lifetime::Lifetime,
    managed::{DynamicManaged, DynamicManagedRef},
};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

pub use intuicio_core::function::Function as ActorMessageFunction;

pub struct ActorTag;
pub struct ActorChild;
pub struct ActorParent;

#[derive(Debug, Default, Clone)]
pub struct ActorMessageListeners(HashMap<String, FunctionHandle>);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Actor(Entity);

impl Actor {
    pub fn spawn(
        world: &mut World,
        bundle: impl Bundle + Send + Sync + 'static,
    ) -> Result<Self, WorldError> {
        let entity = world.spawn((
            ActorTag,
            ActorMessageListeners::default(),
            Relation::<ActorChild>::default(),
            Relation::<ActorParent>::default(),
        ))?;
        world.insert(entity, bundle)?;
        Ok(Self(entity))
    }

    pub fn despawn(self, world: &mut World) -> Result<(), WorldError> {
        world.despawn(self.0)
    }

    pub fn exists(self, world: &World) -> bool {
        world.has_entity(self.0)
    }

    pub fn entity(self) -> Entity {
        self.0
    }

    pub fn component<const LOCKING: bool, T: Send + Sync + 'static>(
        self,
        world: &World,
    ) -> Result<ActorComponent<LOCKING, T>, WorldError> {
        Ok(ActorComponent(world.get::<LOCKING, T>(self.0, false)?))
    }

    pub fn component_mut<const LOCKING: bool, T: Send + Sync + 'static>(
        self,
        world: &World,
    ) -> Result<ActorComponentMut<LOCKING, T>, WorldError> {
        Ok(ActorComponentMut(world.get::<LOCKING, T>(self.0, true)?))
    }

    pub fn add_child<const LOCKING: bool>(
        self,
        world: &mut World,
        other: Self,
    ) -> Result<(), WorldError> {
        world.relate::<LOCKING, _>(ActorChild, self.0, other.0)?;
        world.relate::<LOCKING, _>(ActorParent, other.0, self.0)?;
        Ok(())
    }

    pub fn remove_child<const LOCKING: bool>(
        self,
        world: &mut World,
        other: Self,
    ) -> Result<(), WorldError> {
        world.unrelate::<LOCKING, ActorChild>(self.0, other.0)?;
        world.unrelate::<LOCKING, ActorParent>(other.0, self.0)?;
        Ok(())
    }

    pub fn children<const LOCKING: bool>(self, world: &World) -> impl Iterator<Item = Self> + '_ {
        world
            .relations_outgoing::<LOCKING, ActorChild>(self.0)
            .map(|(_, _, entity)| Self(entity))
    }

    pub fn parents<const LOCKING: bool>(self, world: &World) -> impl Iterator<Item = Self> + '_ {
        world
            .relations_outgoing::<LOCKING, ActorParent>(self.0)
            .map(|(_, _, entity)| Self(entity))
    }

    pub fn register_message_listener<const LOCKING: bool>(
        self,
        world: &World,
        id: impl ToString,
        function: ActorMessageFunction,
    ) -> Result<(), WorldError> {
        let mut listeners = self.component_mut::<LOCKING, ActorMessageListeners>(world)?;
        let listeners = listeners.deref_mut();
        listeners.0.insert(id.to_string(), function.into_handle());
        Ok(())
    }

    pub fn unregister_message_listener<const LOCKING: bool>(
        self,
        world: &World,
        id: &str,
    ) -> Result<(), WorldError> {
        let mut listeners = self.component_mut::<LOCKING, ActorMessageListeners>(world)?;
        let listeners = listeners.deref_mut();
        listeners.0.remove(id);
        Ok(())
    }

    pub fn message_listener<const LOCKING: bool>(
        self,
        world: &World,
        id: &str,
    ) -> Result<Option<FunctionHandle>, WorldError> {
        let listeners = self.component::<LOCKING, ActorMessageListeners>(world)?;
        let listeners = listeners.deref();
        Ok(listeners.0.get(id).cloned())
    }

    pub fn invoke_message<const LOCKING: bool>(
        self,
        world: &World,
        id: &str,
        context: &mut Context,
        registry: &Registry,
    ) -> Result<(), WorldError> {
        let listeners = self.component::<LOCKING, ActorMessageListeners>(world)?;
        let listeners = listeners.deref();
        if let Some(function) = listeners.0.get(id).cloned() {
            context.stack().push(DynamicManaged::new(self).unwrap());
            let lifetime = Lifetime::default();
            let value = DynamicManagedRef::new(world, lifetime.borrow().unwrap());
            context.stack().push(value);
            function.invoke(context, registry);
        }
        Ok(())
    }

    pub fn dispatch_message<const LOCKING: bool, O: DataStackPack, I: DataStackPack>(
        self,
        world: &World,
        id: &str,
        context: &mut Context,
        registry: &Registry,
        inputs: I,
    ) -> Result<Option<O>, WorldError> {
        let listeners = self.component::<LOCKING, ActorMessageListeners>(world)?;
        let listeners = listeners.deref();
        if let Some(function) = listeners.0.get(id).cloned() {
            inputs.stack_push_reversed(context.stack());
            context.stack().push(DynamicManaged::new(self).unwrap());
            let lifetime = Lifetime::default();
            let value = DynamicManagedRef::new(world, lifetime.borrow().unwrap());
            context.stack().push(value);
            function.invoke(context, registry);
            Ok(Some(O::stack_pop(context.stack())))
        } else {
            Ok(None)
        }
    }

    pub fn dispatch_message_hierarchy<const LOCKING: bool, I: DataStackPack + Clone>(
        self,
        world: &World,
        id: &str,
        context: &mut Context,
        registry: &Registry,
        inputs: I,
    ) -> Result<(), WorldError> {
        self.dispatch_message::<LOCKING, (), I>(world, id, context, registry, inputs.clone())?;
        for child in self.children::<LOCKING>(world) {
            child.dispatch_message_hierarchy::<LOCKING, I>(
                world,
                id,
                context,
                registry,
                inputs.clone(),
            )?;
        }
        Ok(())
    }
}

pub struct ActorComponent<'a, const LOCKING: bool, T: Send + Sync + 'static>(
    ArchetypeEntityColumnAccess<'a, LOCKING, T>,
);

impl<const LOCKING: bool, T: Send + Sync + 'static> Deref for ActorComponent<'_, LOCKING, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.read().unwrap()
    }
}

pub struct ActorComponentMut<'a, const LOCKING: bool, T: Send + Sync + 'static>(
    ArchetypeEntityColumnAccess<'a, LOCKING, T>,
);

impl<const LOCKING: bool, T: Send + Sync + 'static> Deref for ActorComponentMut<'_, LOCKING, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.read().unwrap()
    }
}

impl<const LOCKING: bool, T: Send + Sync + 'static> DerefMut for ActorComponentMut<'_, LOCKING, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.write().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use intuicio_core::prelude::*;
    use intuicio_derive::intuicio_function;

    fn is_async<T: Send + Sync>() {}

    struct Attack(usize);

    struct Lives(usize);

    #[intuicio_function(transformer = "DynamicManagedValueTransformer")]
    fn attack(world: &World, this: Actor, other: Actor) {
        let this_attack = this.component::<true, Attack>(world).unwrap();
        let mut other_lives = other.component_mut::<true, Lives>(world).unwrap();
        (*other_lives).0 = (*other_lives).0.saturating_sub((*this_attack).0);
    }

    #[test]
    fn test_actor() {
        is_async::<Actor>();

        let registry = Registry::default()
            .with_basic_types()
            .with_type(NativeStructBuilder::new_uninitialized::<DynamicManaged>().build())
            .with_type(NativeStructBuilder::new_uninitialized::<DynamicManagedRef>().build());
        let mut context = Context::new(10240, 10240);
        let mut world = World::default();

        let player = Actor::spawn(&mut world, ("player".to_owned(), Attack(2), Lives(1))).unwrap();
        player
            .register_message_listener::<true>(&world, "attack", attack::define_function(&registry))
            .unwrap();
        assert!(player.exists(&world));
        assert_eq!(
            player.component::<true, Attack>(&world).unwrap().deref().0,
            2
        );
        assert_eq!(
            player.component::<true, Lives>(&world).unwrap().deref().0,
            1
        );

        let enemy = Actor::spawn(&mut world, ("enemy".to_owned(), Attack(1), Lives(2))).unwrap();
        assert!(enemy.exists(&world));
        assert_eq!(
            enemy.component::<true, Attack>(&world).unwrap().deref().0,
            1
        );
        assert_eq!(enemy.component::<true, Lives>(&world).unwrap().deref().0, 2);

        player
            .dispatch_message::<true, (), _>(
                &world,
                "attack",
                &mut context,
                &registry,
                (DynamicManaged::new(enemy).unwrap(),),
            )
            .unwrap();
        assert_eq!(enemy.component::<true, Lives>(&world).unwrap().deref().0, 0);
    }
}