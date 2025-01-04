use crate::{
    bundle::{Bundle, BundleChain},
    entity::Entity,
    prelude::QuickPlugin,
    query::Exclude,
    systems::{System, SystemContext, SystemObject},
    universe::Universe,
    world::Relation,
    Component,
};
use std::{
    collections::{HashSet, VecDeque},
    error::Error,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SystemPriority(pub usize);
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SystemOrder(pub usize);
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SystemGroupChild;
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SystemDependsOn;

pub struct GraphScheduler<const LOCKING: bool> {
    // TODO: named and unnamed thread pool for parallel systems execution.
}

impl<const LOCKING: bool> Default for GraphScheduler<LOCKING> {
    fn default() -> Self {
        Self {}
    }
}

impl<const LOCKING: bool> GraphScheduler<LOCKING> {
    pub fn run(&mut self, universe: &mut Universe) -> Result<(), Box<dyn Error>> {
        let mut visited = HashSet::with_capacity(universe.systems.len());
        Self::validate_no_cycles(
            universe,
            universe
                .systems
                .query::<LOCKING, (Entity, Exclude<Relation<SystemGroupChild>>)>()
                .map(|(entity, _)| entity)
                .collect(),
            &mut visited,
        )?;
        visited.clear();
        let mut queue = universe
            .systems
            .query::<LOCKING, (Entity, Exclude<Relation<SystemGroupChild>>)>()
            .map(|(entity, _)| entity)
            .collect::<VecDeque<_>>();
        while let Some(entity) = queue.pop_front() {
            Self::run_node(universe, entity, &mut visited, &mut queue)?;
        }
        universe.clear_changes();
        universe.execute_commands::<LOCKING>();
        universe.maintain_plugins();
        Ok(())
    }

    fn validate_no_cycles(
        universe: &Universe,
        entities: Vec<Entity>,
        visited: &mut HashSet<Entity>,
    ) -> Result<(), Box<dyn Error>> {
        for entity in entities {
            if visited.contains(&entity) {
                return Err(
                    format!("Found systems graph cycle for system entity: {}", entity).into(),
                );
            }
            visited.insert(entity);
            Self::validate_no_cycles(
                universe,
                universe
                    .systems
                    .relations_outgoing::<LOCKING, SystemGroupChild>(entity)
                    .map(|(_, _, entity)| entity)
                    .collect(),
                visited,
            )?;
        }
        Ok(())
    }

    fn run_node(
        universe: &Universe,
        entity: Entity,
        visited: &mut HashSet<Entity>,
        queue: &mut VecDeque<Entity>,
    ) -> Result<bool, Box<dyn Error>> {
        if visited.contains(&entity) {
            return Ok(true);
        }
        if universe
            .systems
            .relations_outgoing::<LOCKING, SystemDependsOn>(entity)
            .any(|(_, _, other)| !visited.contains(&other))
        {
            queue.push_back(entity);
            return Ok(false);
        }
        if let Ok(system) = universe.systems.component::<LOCKING, SystemObject>(entity) {
            if system.should_run(SystemContext::new(universe, entity)) {
                system.run(SystemContext::new(universe, entity))?;
            }
        }
        visited.insert(entity);
        Self::run_group(
            universe,
            universe
                .systems
                .relations_outgoing::<LOCKING, SystemGroupChild>(entity)
                .map(|(_, _, entity)| entity),
            visited,
            queue,
        )?;
        Ok(true)
    }

    fn run_group(
        universe: &Universe,
        entities: impl Iterator<Item = Entity>,
        visited: &mut HashSet<Entity>,
        queue: &mut VecDeque<Entity>,
    ) -> Result<(), Box<dyn Error>> {
        let mut selected = entities
            .map(|entity| {
                let priority = universe
                    .systems
                    .component::<LOCKING, SystemPriority>(entity)
                    .ok()
                    .map(|priority| *priority)
                    .unwrap_or_default();
                let order = universe
                    .systems
                    .component::<LOCKING, SystemOrder>(entity)
                    .ok()
                    .map(|order| *order)
                    .unwrap_or_default();
                (entity, priority, order)
            })
            .collect::<Vec<_>>();
        selected.sort_by(|(_, priority_a, order_a), (_, priority_b, order_b)| {
            priority_a
                .cmp(priority_b)
                .reverse()
                .then(order_a.cmp(order_b))
        });
        for (entity, _, _) in selected {
            Self::run_node(universe, entity, visited, queue)?;
        }
        Ok(())
    }
}

pub struct GraphSchedulerQuickPlugin<const LOCKING: bool, Tag: Send + Sync> {
    plugin: QuickPlugin<Tag>,
    order: usize,
}

impl<const LOCKING: bool, Tag: Send + Sync> Default for GraphSchedulerQuickPlugin<LOCKING, Tag> {
    fn default() -> Self {
        Self {
            plugin: Default::default(),
            order: 0,
        }
    }
}

impl<const LOCKING: bool, Tag: Send + Sync> GraphSchedulerQuickPlugin<LOCKING, Tag> {
    pub fn new(plugin: QuickPlugin<Tag>) -> Self {
        Self { plugin, order: 0 }
    }

    pub fn commit(self) -> QuickPlugin<Tag> {
        self.plugin
    }

    pub fn quick(mut self, f: impl FnOnce(QuickPlugin<Tag>) -> QuickPlugin<Tag>) -> Self {
        self.plugin = f(self.plugin);
        self
    }

    pub fn group<ID: Component + Clone + PartialEq, L: Bundle + Send + Sync + 'static>(
        mut self,
        id: ID,
        locals: L,
        f: impl FnOnce(GraphSchedulerGroup<LOCKING, ID, Tag>) -> GraphSchedulerGroup<LOCKING, ID, Tag>,
    ) -> Self {
        self.plugin = self
            .plugin
            .system_meta(BundleChain((id.clone(), SystemOrder(self.order)), locals));
        self.plugin = f(GraphSchedulerGroup {
            id,
            plugin: self.plugin,
            order: 0,
        })
        .plugin;
        self.order += 1;
        self
    }

    pub fn system<ID: Component>(
        mut self,
        system: impl System,
        id: ID,
        locals: impl Bundle + Send + Sync + 'static,
    ) -> Self {
        self.plugin = self
            .plugin
            .system(system, BundleChain((id, SystemOrder(self.order)), locals));
        self.order += 1;
        self
    }

    pub fn resource<T: Component>(mut self, resource: T) -> Self {
        self.plugin = self.plugin.resource(resource);
        self
    }
}

pub struct GraphSchedulerGroup<
    const LOCKING: bool,
    ID: Component + Clone + PartialEq,
    Tag: Send + Sync,
> {
    id: ID,
    plugin: QuickPlugin<Tag>,
    order: usize,
}

impl<const LOCKING: bool, ID: Component + Clone + PartialEq, Tag: Send + Sync>
    GraphSchedulerGroup<LOCKING, ID, Tag>
{
    pub fn quick(mut self, f: impl FnOnce(QuickPlugin<Tag>) -> QuickPlugin<Tag>) -> Self {
        self.plugin = f(self.plugin);
        self
    }

    pub fn group<L: Bundle + Send + Sync + 'static>(
        mut self,
        id: ID,
        locals: L,
        f: impl FnOnce(Self) -> Self,
    ) -> Self {
        self.plugin = self
            .plugin
            .system_meta(BundleChain((id.clone(), SystemOrder(self.order)), locals));
        self.plugin = f(GraphSchedulerGroup {
            id: id.clone(),
            plugin: self.plugin,
            order: 0,
        })
        .plugin;
        self.plugin =
            self.plugin
                .system_relation::<LOCKING, _, _>(self.id.clone(), SystemGroupChild, id);
        self.order += 1;
        self
    }

    pub fn system(
        mut self,
        system: impl System,
        id: ID,
        locals: impl Bundle + Send + Sync + 'static,
    ) -> Self {
        self.plugin = self.plugin.system(
            system,
            BundleChain((id.clone(), SystemOrder(self.order)), locals),
        );
        self.plugin =
            self.plugin
                .system_relation::<LOCKING, _, _>(self.id.clone(), SystemGroupChild, id);
        self.order += 1;
        self
    }
}
