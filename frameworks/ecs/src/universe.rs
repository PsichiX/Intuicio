use crate::{
    bundle::Bundle,
    commands::CommandBuffer,
    entity::Entity,
    processor::WorldProcessor,
    query::{Lookup, Query, TypedLookupFetch, TypedQueryFetch},
    resources::Resources,
    systems::{System, SystemContext, Systems},
    world::World,
    Component, ComponentRef, ComponentRefMut,
};
use intuicio_core::{context::Context, registry::Registry};
use intuicio_data::type_hash::TypeHash;
use intuicio_framework_serde::SerializationRegistry;
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    marker::PhantomData,
    sync::Mutex,
};

pub trait UniverseFetch<'a> {
    type Value;

    fn fetch(universe: &'a Universe, system: Entity) -> Result<Self::Value, Box<dyn Error>>;
}

impl<'a> UniverseFetch<'a> for &'a World {
    type Value = &'a World;

    fn fetch(universe: &'a Universe, _: Entity) -> Result<Self::Value, Box<dyn Error>> {
        Ok(&universe.simulation)
    }
}

pub struct Res<const LOCKING: bool, T>(PhantomData<fn() -> T>);

impl<'a, const LOCKING: bool, T: Component> UniverseFetch<'a> for Res<LOCKING, &'a T> {
    type Value = ComponentRef<'a, LOCKING, T>;

    fn fetch(universe: &'a Universe, _: Entity) -> Result<Self::Value, Box<dyn Error>> {
        universe.resources.get()
    }
}

impl<'a, const LOCKING: bool, T: Component> UniverseFetch<'a> for Res<LOCKING, &'a mut T> {
    type Value = ComponentRefMut<'a, LOCKING, T>;

    fn fetch(universe: &'a Universe, _: Entity) -> Result<Self::Value, Box<dyn Error>> {
        universe.resources.get_mut()
    }
}

pub struct Local<const LOCKING: bool, T>(PhantomData<fn() -> T>);

impl<'a, const LOCKING: bool, T: Component> UniverseFetch<'a> for Local<LOCKING, &'a T> {
    type Value = ComponentRef<'a, LOCKING, T>;

    fn fetch(universe: &'a Universe, system: Entity) -> Result<Self::Value, Box<dyn Error>> {
        Ok(universe.systems.component(system)?)
    }
}

impl<'a, const LOCKING: bool, T: Component> UniverseFetch<'a> for Local<LOCKING, &'a mut T> {
    type Value = ComponentRefMut<'a, LOCKING, T>;

    fn fetch(universe: &'a Universe, system: Entity) -> Result<Self::Value, Box<dyn Error>> {
        Ok(universe.systems.component_mut(system)?)
    }
}

impl<'a, const LOCKING: bool, Fetch: TypedQueryFetch<'a, LOCKING>> UniverseFetch<'a>
    for Query<'a, LOCKING, Fetch>
{
    type Value = Query<'a, LOCKING, Fetch>;

    fn fetch(_: &Universe, _: Entity) -> Result<Self::Value, Box<dyn Error>> {
        Ok(Query::<LOCKING, Fetch>::default())
    }
}

impl<'a, const LOCKING: bool, Fetch: TypedLookupFetch<'a, LOCKING>> UniverseFetch<'a>
    for Lookup<'a, LOCKING, Fetch>
{
    type Value = Lookup<'a, LOCKING, Fetch>;

    fn fetch(_: &Universe, _: Entity) -> Result<Self::Value, Box<dyn Error>> {
        Ok(Lookup::<LOCKING, Fetch>::default())
    }
}

macro_rules! impl_universe_fetch_tuple {
    ($($type:ident),+) => {
        impl<'a, $($type: UniverseFetch<'a>),+> UniverseFetch<'a> for ($($type,)+) {
            type Value = ($($type::Value,)+);

            fn fetch(universe: &'a Universe, entity: Entity) -> Result<Self::Value, Box<dyn Error>> {
                Ok(($($type::fetch(universe, entity)?,)+))
            }
        }
    };
}

impl_universe_fetch_tuple!(A);
impl_universe_fetch_tuple!(A, B);
impl_universe_fetch_tuple!(A, B, C);
impl_universe_fetch_tuple!(A, B, C, D);
impl_universe_fetch_tuple!(A, B, C, D, E);
impl_universe_fetch_tuple!(A, B, C, D, E, F);
impl_universe_fetch_tuple!(A, B, C, D, E, F, G);
impl_universe_fetch_tuple!(A, B, C, D, E, F, G, H);
impl_universe_fetch_tuple!(A, B, C, D, E, F, G, H, I);
impl_universe_fetch_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_universe_fetch_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_universe_fetch_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_universe_fetch_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_universe_fetch_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_universe_fetch_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_universe_fetch_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);

pub trait UniverseCondition {
    fn evaluate(context: SystemContext) -> bool;
}

pub struct ResourceDidChanged<T: Component>(PhantomData<fn() -> T>);

impl<T: Component> UniverseCondition for ResourceDidChanged<T> {
    fn evaluate(context: SystemContext) -> bool {
        context.universe.resources.did_changed::<T>()
    }
}

pub struct ResourceAdded<T: Component>(PhantomData<fn() -> T>);

impl<T: Component> UniverseCondition for ResourceAdded<T> {
    fn evaluate(context: SystemContext) -> bool {
        context.universe.resources.added().has_component::<T>()
    }
}

pub struct ResourceRemoved<T: Component>(PhantomData<fn() -> T>);

impl<T: Component> UniverseCondition for ResourceRemoved<T> {
    fn evaluate(context: SystemContext) -> bool {
        context.universe.resources.removed().has_component::<T>()
    }
}

pub struct ResourceUpdated<T: Component>(PhantomData<fn() -> T>);

impl<T: Component> UniverseCondition for ResourceUpdated<T> {
    fn evaluate(context: SystemContext) -> bool {
        context
            .universe
            .resources
            .updated()
            .map(|changes| changes.has_component::<T>())
            .unwrap_or_default()
    }
}

pub struct ComponentDidChanged<T: Component>(PhantomData<fn() -> T>);

impl<T: Component> UniverseCondition for ComponentDidChanged<T> {
    fn evaluate(context: SystemContext) -> bool {
        context.universe.simulation.component_did_changed::<T>()
    }
}

pub struct ComponentAdded<T: Component>(PhantomData<fn() -> T>);

impl<T: Component> UniverseCondition for ComponentAdded<T> {
    fn evaluate(context: SystemContext) -> bool {
        context.universe.simulation.added().has_component::<T>()
    }
}

pub struct ComponentRemoved<T: Component>(PhantomData<fn() -> T>);

impl<T: Component> UniverseCondition for ComponentRemoved<T> {
    fn evaluate(context: SystemContext) -> bool {
        context.universe.simulation.removed().has_component::<T>()
    }
}

pub struct ComponentUpdated<T: Component>(PhantomData<fn() -> T>);

impl<T: Component> UniverseCondition for ComponentUpdated<T> {
    fn evaluate(context: SystemContext) -> bool {
        context
            .universe
            .simulation
            .updated()
            .map(|changes| changes.has_component::<T>())
            .unwrap_or_default()
    }
}

pub struct SystemLocalDidChanged<T: Component>(PhantomData<fn() -> T>);

impl<T: Component> UniverseCondition for SystemLocalDidChanged<T> {
    fn evaluate(context: SystemContext) -> bool {
        context
            .universe
            .systems
            .entity_component_did_changed::<T>(context.entity())
    }
}

pub struct SystemLocalAdded<T: Component>(PhantomData<fn() -> T>);

impl<T: Component> UniverseCondition for SystemLocalAdded<T> {
    fn evaluate(context: SystemContext) -> bool {
        context
            .universe
            .systems
            .added()
            .has_entity_component::<T>(context.entity())
    }
}

pub struct SystemLocalRemoved<T: Component>(PhantomData<fn() -> T>);

impl<T: Component> UniverseCondition for SystemLocalRemoved<T> {
    fn evaluate(context: SystemContext) -> bool {
        context
            .universe
            .systems
            .removed()
            .has_entity_component::<T>(context.entity())
    }
}

pub struct SystemLocalUpdated<T: Component>(PhantomData<fn() -> T>);

impl<T: Component> UniverseCondition for SystemLocalUpdated<T> {
    fn evaluate(context: SystemContext) -> bool {
        context
            .universe
            .systems
            .updated()
            .map(|changes| changes.has_entity_component::<T>(context.entity()))
            .unwrap_or_default()
    }
}

macro_rules! impl_universe_condition_tuple {
    ($($type:ident),+) => {
        impl<$($type: UniverseCondition),+> UniverseCondition for ($($type,)+) {
            fn evaluate(context: SystemContext) -> bool {
                $($type::evaluate(context))&&+
            }
        }
    };
}

impl_universe_condition_tuple!(A);
impl_universe_condition_tuple!(A, B);
impl_universe_condition_tuple!(A, B, C);
impl_universe_condition_tuple!(A, B, C, D);
impl_universe_condition_tuple!(A, B, C, D, E);
impl_universe_condition_tuple!(A, B, C, D, E, F);
impl_universe_condition_tuple!(A, B, C, D, E, F, G);
impl_universe_condition_tuple!(A, B, C, D, E, F, G, H);
impl_universe_condition_tuple!(A, B, C, D, E, F, G, H, I);
impl_universe_condition_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_universe_condition_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_universe_condition_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_universe_condition_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_universe_condition_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_universe_condition_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_universe_condition_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);

#[derive(Default)]
pub struct Universe {
    pub simulation: World,
    pub systems: Systems,
    pub resources: Resources,
    plugins: HashMap<TypeHash, Box<dyn Plugin>>,
    plugins_to_register: Mutex<HashMap<TypeHash, Box<dyn Plugin>>>,
    plugins_to_unregister: Mutex<HashSet<TypeHash>>,
}

impl Drop for Universe {
    fn drop(&mut self) {
        self.clear_plugins();
    }
}

impl Universe {
    pub fn new(simulation: World) -> Self {
        Self {
            simulation,
            resources: Default::default(),
            systems: Default::default(),
            plugins: Default::default(),
            plugins_to_register: Default::default(),
            plugins_to_unregister: Default::default(),
        }
    }

    pub fn with_basics(self, stack_capacity: usize, registers_capacity: usize) -> Self {
        struct BasicsPlugin;
        self.with_plugin(
            QuickPlugin::<BasicsPlugin>::default()
                .resource(CommandBuffer::default())
                .resource(Registry::default())
                .resource(Context::new(stack_capacity, registers_capacity))
                .resource(WorldProcessor::default())
                .resource(SerializationRegistry::default()),
        )
    }

    pub fn clear_changes(&mut self) {
        self.simulation.clear_changes();
        self.resources.clear_changes();
        self.systems.clear_changes();
    }

    pub fn execute_commands<const LOCKING: bool>(&mut self) {
        for commands in self.resources.query::<LOCKING, &mut CommandBuffer>() {
            commands.execute(&mut self.simulation);
        }
        for commands in self.systems.query::<LOCKING, &mut CommandBuffer>() {
            commands.execute(&mut self.simulation);
        }
    }

    pub fn with_plugin<T: Plugin + 'static>(mut self, plugin: T) -> Self {
        self.add_plugin(plugin);
        self.maintain_plugins();
        self
    }

    pub fn add_plugin<T: Plugin + 'static>(&self, plugin: T) {
        if let Ok(mut to_register) = self.plugins_to_register.lock() {
            to_register.insert(TypeHash::of::<T>(), Box::new(plugin));
        }
    }

    pub fn remove_plugin<T: Plugin + 'static>(&self) {
        if let Ok(mut to_unregister) = self.plugins_to_unregister.lock() {
            to_unregister.insert(TypeHash::of::<T>());
        }
    }

    pub fn remove_plugin_raw(&self, type_hash: TypeHash) {
        if let Ok(mut to_unregister) = self.plugins_to_unregister.lock() {
            to_unregister.insert(type_hash);
        }
    }

    pub fn clear_plugins(&mut self) {
        for (_, mut plugin) in std::mem::take(&mut self.plugins) {
            plugin.on_unregister(&mut self.simulation, &mut self.systems, &mut self.resources);
        }
    }

    pub fn maintain_plugins(&mut self) {
        if let Ok(mut to_unregister) = self.plugins_to_unregister.try_lock() {
            if !to_unregister.is_empty() {
                for type_hash in to_unregister.drain() {
                    if let Some(mut plugin) = self.plugins.remove(&type_hash) {
                        plugin.on_unregister(
                            &mut self.simulation,
                            &mut self.systems,
                            &mut self.resources,
                        );
                    }
                }
            }
        }
        if let Ok(mut to_register) = self.plugins_to_register.try_lock() {
            if !to_register.is_empty() {
                for (type_hash, mut plugin) in std::mem::take(&mut *to_register).drain() {
                    if let Some(mut plugin) = self.plugins.remove(&type_hash) {
                        plugin.on_unregister(
                            &mut self.simulation,
                            &mut self.systems,
                            &mut self.resources,
                        );
                    }
                    if plugin
                        .dependencies()
                        .into_iter()
                        .all(|type_hash| self.plugins.contains_key(&type_hash))
                    {
                        plugin.on_register(
                            &mut self.simulation,
                            &mut self.systems,
                            &mut self.resources,
                        );
                        self.plugins.insert(type_hash, plugin);
                    } else {
                        to_register.insert(type_hash, plugin);
                    }
                }
            }
        }
    }
}

pub trait Plugin: Send + Sync {
    fn on_register(
        &mut self,
        simulation: &mut World,
        systems: &mut Systems,
        resources: &mut Resources,
    );
    fn on_unregister(
        &mut self,
        simulation: &mut World,
        systems: &mut Systems,
        resources: &mut Resources,
    );

    fn dependencies(&self) -> Vec<TypeHash> {
        vec![]
    }
}

#[derive(Default)]
pub struct PluginsPackage {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginsPackage {
    pub fn plugin(mut self, plugin: impl Plugin + 'static) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }
}

impl Plugin for PluginsPackage {
    fn on_register(
        &mut self,
        simulation: &mut World,
        systems: &mut Systems,
        resources: &mut Resources,
    ) {
        for plugin in &mut self.plugins {
            plugin.on_register(simulation, systems, resources);
        }
    }

    fn on_unregister(
        &mut self,
        simulation: &mut World,
        systems: &mut Systems,
        resources: &mut Resources,
    ) {
        for plugin in &mut self.plugins {
            plugin.on_unregister(simulation, systems, resources);
        }
    }
}

pub struct QuickPlugin<Tag: Send + Sync> {
    #[allow(clippy::type_complexity)]
    simulation_register:
        Vec<Box<dyn FnOnce(&mut World) -> Box<dyn FnOnce(&mut World) + Send + Sync> + Send + Sync>>,
    #[allow(clippy::type_complexity)]
    simulation_unregister: Vec<Box<dyn FnOnce(&mut World) + Send + Sync>>,
    #[allow(clippy::type_complexity)]
    systems_register: Vec<
        Box<dyn FnOnce(&mut Systems) -> Box<dyn FnOnce(&mut Systems) + Send + Sync> + Send + Sync>,
    >,
    #[allow(clippy::type_complexity)]
    systems_unregister: Vec<Box<dyn FnOnce(&mut Systems) + Send + Sync>>,
    #[allow(clippy::type_complexity)]
    resources_register: Vec<
        Box<
            dyn FnOnce(&mut Resources) -> Box<dyn FnOnce(&mut Resources) + Send + Sync>
                + Send
                + Sync,
        >,
    >,
    #[allow(clippy::type_complexity)]
    resources_unregister: Vec<Box<dyn FnOnce(&mut Resources) + Send + Sync>>,
    _phantom: PhantomData<fn() -> Tag>,
}

impl<Tag: Send + Sync> Default for QuickPlugin<Tag> {
    fn default() -> Self {
        Self {
            simulation_register: Default::default(),
            simulation_unregister: Default::default(),
            systems_register: Default::default(),
            systems_unregister: Default::default(),
            resources_register: Default::default(),
            resources_unregister: Default::default(),
            _phantom: Default::default(),
        }
    }
}

impl<Tag: Send + Sync> QuickPlugin<Tag> {
    pub fn entity(mut self, bundle: impl Bundle + Send + Sync + 'static) -> Self {
        self.simulation_register.push(Box::new(|world| {
            let entity = world.spawn(bundle).unwrap();
            Box::new(move |world| {
                let _ = world.despawn(entity);
            })
        }));
        self
    }

    pub fn entity_relation<const LOCKING: bool, R: Component, E: Component + PartialEq>(
        mut self,
        from: E,
        payload: R,
        to: E,
    ) -> Self {
        self.simulation_register.push(Box::new(move |world| {
            let from = world.find_by::<LOCKING, E>(&from).unwrap();
            let to = world.find_by::<LOCKING, E>(&to).unwrap();
            world.relate::<LOCKING, R>(payload, from, to).unwrap();
            Box::new(move |world| {
                let _ = world.unrelate::<LOCKING, R>(from, to);
            })
        }));
        self
    }

    pub fn system(
        mut self,
        system: impl System,
        locals: impl Bundle + Send + Sync + 'static,
    ) -> Self {
        self.systems_register.push(Box::new(|systems| {
            let entity = systems.add(system, locals).unwrap();
            Box::new(move |systems| {
                let _ = systems.despawn(entity);
            })
        }));
        self
    }

    pub fn system_meta(mut self, locals: impl Bundle + Send + Sync + 'static) -> Self {
        self.systems_register.push(Box::new(|systems| {
            let entity = systems.spawn(locals).unwrap();
            Box::new(move |systems| {
                let _ = systems.despawn(entity);
            })
        }));
        self
    }

    pub fn system_relation<const LOCKING: bool, R: Component, E: Component + PartialEq>(
        mut self,
        from: E,
        payload: R,
        to: E,
    ) -> Self {
        self.systems_register.push(Box::new(move |systems| {
            let from = systems.find_by::<LOCKING, E>(&from).unwrap();
            let to = systems.find_by::<LOCKING, E>(&to).unwrap();
            systems.relate::<LOCKING, R>(payload, from, to).unwrap();
            Box::new(move |systems| {
                let _ = systems.unrelate::<LOCKING, R>(from, to);
            })
        }));
        self
    }

    pub fn resource<T: Component>(mut self, resource: T) -> Self {
        self.resources_register.push(Box::new(|resources| {
            resources.add((resource,)).unwrap();
            Box::new(|resources| {
                let _ = resources.remove::<(T,)>();
            })
        }));
        self
    }
}

impl<T: Send + Sync> Plugin for QuickPlugin<T> {
    fn on_register(
        &mut self,
        simulation: &mut World,
        systems: &mut Systems,
        resources: &mut Resources,
    ) {
        for execute in self.simulation_register.drain(..) {
            self.simulation_unregister.push(execute(simulation));
        }
        for execute in self.systems_register.drain(..) {
            self.systems_unregister.push(execute(systems));
        }
        for execute in self.resources_register.drain(..) {
            self.resources_unregister.push(execute(resources));
        }
    }

    fn on_unregister(
        &mut self,
        simulation: &mut World,
        systems: &mut Systems,
        resources: &mut Resources,
    ) {
        for execute in self.simulation_unregister.drain(..) {
            execute(simulation);
        }
        for execute in self.systems_unregister.drain(..) {
            execute(systems);
        }
        for execute in self.resources_unregister.drain(..) {
            execute(resources);
        }
    }
}
