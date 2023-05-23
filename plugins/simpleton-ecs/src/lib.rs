use bitvec::vec::BitVec;
use intuicio_core::{core_version, registry::Registry, IntuicioStruct, IntuicioVersion};
use intuicio_derive::{intuicio_method, intuicio_methods, IntuicioStruct};
use intuicio_frontend_simpleton::{
    library::closure::Closure, Array, Function, Integer, Reference, Type,
};
use std::collections::{HashMap, HashSet};

struct Bucket {
    types: Vec<Type>,
    entitity_components: Vec<(Integer, Vec<Reference>)>,
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "World", module_name = "world")]
pub struct World {
    #[intuicio(ignore)]
    component_table: Vec<Type>,
    #[intuicio(ignore)]
    buckets: HashMap<BitVec, Bucket>,
    #[intuicio(ignore)]
    entity_generator: Integer,
    #[intuicio(ignore)]
    resources: Vec<Reference>,
    #[intuicio(ignore)]
    to_despawn: HashSet<Integer>,
    #[intuicio(ignore)]
    to_add: HashMap<Integer, Vec<Reference>>,
    #[intuicio(ignore)]
    to_remove: HashMap<Integer, Vec<Type>>,
    #[intuicio(ignore)]
    to_clear: bool,
}

#[intuicio_methods(module_name = "world")]
impl World {
    #[allow(clippy::new_ret_no_self)]
    #[intuicio_method(use_registry)]
    pub fn new(registry: &Registry) -> Reference {
        Reference::new(World::default(), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn spawn(registry: &Registry, mut world: Reference) -> Reference {
        let mut world = world.write::<World>().expect("`world` is not a World!");
        let entity = world.entity_generator;
        world.entity_generator = world.entity_generator.wrapping_add(1);
        Reference::new_integer(entity, registry)
    }

    #[intuicio_method()]
    pub fn despawn(mut world: Reference, entity: Reference) -> Reference {
        let mut world = world.write::<World>().expect("`world` is not a World!");
        let entity = entity
            .read::<Integer>()
            .expect("`entity` is not an Integer!");
        world.to_despawn.insert(*entity);
        Reference::null()
    }

    #[intuicio_method()]
    pub fn add(mut world: Reference, entity: Reference, component: Reference) -> Reference {
        let mut world = world.write::<World>().expect("`world` is not a World!");
        let entity = entity
            .read::<Integer>()
            .expect("`entity` is not an Integer!");
        world
            .to_add
            .entry(*entity)
            .or_default()
            .push(component.clone());
        component
    }

    #[intuicio_method()]
    pub fn add_bundle(mut world: Reference, entity: Reference, components: Reference) -> Reference {
        let mut world = world.write::<World>().expect("`world` is not a World!");
        let entity = entity
            .read::<Integer>()
            .expect("`entity` is not an Integer!");
        let target = world.to_add.entry(*entity).or_default();
        target.extend(
            components
                .read::<Array>()
                .expect("`components` is not an Array!")
                .to_owned(),
        );
        components
    }

    #[intuicio_method()]
    pub fn remove(mut world: Reference, entity: Reference, component_type: Reference) -> Reference {
        let mut world = world.write::<World>().expect("`world` is not a World!");
        let entity = entity
            .read::<Integer>()
            .expect("`entity` is not an Integer!");
        let component_type = component_type
            .read::<Type>()
            .expect("`component_type` is not a Type!");
        world
            .to_remove
            .entry(*entity)
            .or_default()
            .push(component_type.to_owned());
        Reference::null()
    }

    #[intuicio_method()]
    pub fn clear(mut world: Reference) -> Reference {
        let mut world = world.write::<World>().expect("`world` is not a World!");
        world.to_clear = true;
        world.resources.clear();
        Reference::null()
    }

    #[intuicio_method(use_registry)]
    pub fn entities(registry: &Registry, world: Reference) -> Reference {
        let world = world.read::<World>().expect("`world` is not a World!");
        Reference::new_array(
            world
                .buckets
                .values()
                .flat_map(|bucket| {
                    bucket
                        .entitity_components
                        .iter()
                        .map(|(entity, _)| Reference::new_integer(*entity, registry))
                })
                .collect(),
            registry,
        )
    }

    #[intuicio_method()]
    pub fn get(world: Reference, entity: Reference, component_type: Reference) -> Reference {
        let world = world.read::<World>().expect("`world` is not a World!");
        let entity = entity
            .read::<Integer>()
            .expect("`entity` is not an Integer!");
        let component_type = component_type
            .read::<Type>()
            .expect("`component_type` is not a Type!");
        for bucket in world.buckets.values() {
            if let Some(components) = bucket
                .entitity_components
                .iter()
                .find(|(e, _)| *entity == *e)
            {
                if let Some(index) = bucket
                    .types
                    .iter()
                    .position(|ty| component_type.is_same_as(ty))
                {
                    return components.1[index].clone();
                }
            }
        }
        Reference::null()
    }

    #[intuicio_method(use_registry)]
    pub fn query(registry: &Registry, world: Reference, component_types: Reference) -> Reference {
        let world_ref = world.clone();
        let world = world.read::<World>().expect("`world` is not a World!");
        let component_types = component_types
            .read::<Array>()
            .expect("`component_types` is not an Array!");
        let types = component_types
            .iter()
            .filter_map(|item| item.read::<Type>())
            .map(|ty| ty.to_owned())
            .collect::<Vec<_>>();
        let archetype = world.archetype(types.iter().map(|ty| ty.to_owned()));
        let buckets = world
            .buckets
            .keys()
            .filter(|bucket_archetype| Self::archetype_contains(bucket_archetype, &archetype))
            .cloned()
            .collect::<Vec<_>>();
        Reference::new(
            IterQuery {
                types,
                buckets,
                current_bucket: 0,
                current_entity: 0,
                world: world_ref,
                next: Reference::new(
                    Closure {
                        function: Function::by_name("next", "query", registry).unwrap(),
                        captured: vec![],
                    },
                    registry,
                ),
            },
            registry,
        )
    }

    #[intuicio_method()]
    pub fn maintain(mut world: Reference) -> Reference {
        let mut world = world.write::<World>().expect("`world` is not a World!");
        if world.to_clear {
            world.buckets.clear();
        } else {
            for entity in std::mem::take(&mut world.to_despawn) {
                world.take(entity);
            }
            for (entity, component_types) in std::mem::take(&mut world.to_remove) {
                if let Some(components) = world.take(entity) {
                    let items = world.to_add.entry(entity).or_default();
                    items.extend(components.into_iter().filter(|component| {
                        component
                            .type_of()
                            .map(|ty| !component_types.iter().any(|cty| ty.is_same_as(cty)))
                            .unwrap_or_default()
                    }));
                }
            }
        }
        for (entity, components) in std::mem::take(&mut world.to_add) {
            for component in &components {
                let component_type = component.type_of().unwrap();
                if !world
                    .component_table
                    .iter()
                    .any(|ty| component_type.is_same_as(ty))
                {
                    world.component_table.push(component_type);
                }
            }
            let original = world.take(entity).unwrap_or_default();
            let archetype = world.archetype(
                original
                    .iter()
                    .chain(components.iter())
                    .filter_map(|component| component.type_of()),
            );
            let bucket = if let Some(bucket) = world.buckets.get_mut(&archetype) {
                bucket
            } else {
                let types = world
                    .component_table
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| {
                        archetype
                            .get(*index)
                            .map(|value| *value)
                            .unwrap_or_default()
                    })
                    .map(|(_, ty)| ty.to_owned())
                    .collect();
                world.buckets.insert(
                    archetype.to_owned(),
                    Bucket {
                        types,
                        entitity_components: Default::default(),
                    },
                );
                world.buckets.get_mut(&archetype).unwrap()
            };
            let bucket_components = if let Some(bucket_components) = bucket
                .entitity_components
                .iter_mut()
                .find(|(e, _)| entity == *e)
            {
                &mut bucket_components.1
            } else {
                let count = bucket.types.len();
                let index = bucket.entitity_components.len();
                bucket
                    .entitity_components
                    .push((entity, vec![Reference::null(); count]));
                &mut bucket.entitity_components[index].1
            };
            for component in original.into_iter().chain(components.into_iter()) {
                if let Some(component_type) = component.type_of() {
                    if let Some(index) = bucket
                        .types
                        .iter()
                        .position(|ty| component_type.is_same_as(ty))
                    {
                        bucket_components[index] = component;
                    }
                }
            }
        }
        let to_delete = world
            .buckets
            .iter()
            .filter(|(_, bucket)| bucket.types.is_empty() || bucket.entitity_components.is_empty())
            .map(|(archetype, _)| archetype.to_owned())
            .collect::<Vec<_>>();
        for archetype in to_delete {
            world.buckets.remove(&archetype);
        }
        Reference::null()
    }

    #[intuicio_method()]
    pub fn add_resource(mut world: Reference, resource: Reference) -> Reference {
        let mut world = world.write::<World>().expect("`world` is not a World!");
        let resource_type = resource.type_of().unwrap();
        if let Some(res) = world
            .resources
            .iter_mut()
            .find(|res| res.type_of().unwrap().is_same_as(&resource_type))
        {
            *res = resource.clone();
        } else {
            world.resources.push(resource.clone());
        }
        resource
    }

    #[intuicio_method()]
    pub fn remove_resource(mut world: Reference, resource_type: Reference) -> Reference {
        let mut world = world.write::<World>().expect("`world` is not a World!");
        let resource_type = resource_type
            .read::<Type>()
            .expect("`resource_type` is not a Type!");
        if let Some(index) = world
            .resources
            .iter()
            .position(|res| res.type_of().unwrap().is_same_as(&resource_type))
        {
            return world.resources.swap_remove(index);
        }
        Reference::null()
    }

    #[intuicio_method()]
    pub fn resource(world: Reference, resource_type: Reference) -> Reference {
        let world = world.read::<World>().expect("`world` is not a World!");
        let resource_type = resource_type
            .read::<Type>()
            .expect("`resource_type` is not a Type!");
        if let Some(resource) = world
            .resources
            .iter()
            .find(|res| res.type_of().unwrap().is_same_as(&resource_type))
        {
            return resource.clone();
        }
        Reference::null()
    }

    #[intuicio_method(use_registry)]
    pub fn resources(
        registry: &Registry,
        world: Reference,
        resource_types: Reference,
    ) -> Reference {
        let world = world.read::<World>().expect("`world` is not a World!");
        let resource_types = resource_types
            .read::<Array>()
            .expect("`resource_types` is not an Array!");
        Reference::new_array(
            resource_types
                .iter()
                .map(|resource_type| {
                    let resource_type = resource_type
                        .read::<Type>()
                        .expect("`resource_types` item is not a Type!");
                    world
                        .resources
                        .iter()
                        .find(|resource| resource.type_of().unwrap().is_same_as(&resource_type))
                        .cloned()
                        .unwrap_or_default()
                })
                .collect(),
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn snapshot(registry: &Registry, world: Reference) -> Reference {
        let world = world.read::<World>().expect("`world` is not a World!");
        Reference::new_array(
            world
                .buckets
                .values()
                .map(|bucket| {
                    let mut result = HashMap::with_capacity(2);
                    result.insert(
                        "types".to_owned(),
                        Reference::new_array(
                            bucket
                                .types
                                .iter()
                                .map(|ty| Reference::new_type(ty.to_owned(), registry))
                                .collect(),
                            registry,
                        ),
                    );
                    result.insert(
                        "bucket".to_owned(),
                        Reference::new_array(
                            bucket
                                .entitity_components
                                .iter()
                                .map(|(entity, components)| {
                                    let mut result = HashMap::with_capacity(2);
                                    result.insert(
                                        "entity".to_owned(),
                                        Reference::new_integer(*entity, registry),
                                    );
                                    result.insert(
                                        "components".to_owned(),
                                        Reference::new_array(components.to_owned(), registry),
                                    );
                                    Reference::new_map(result, registry)
                                })
                                .collect(),
                            registry,
                        ),
                    );
                    Reference::new_map(result, registry)
                })
                .collect(),
            registry,
        )
    }

    fn take(&mut self, entity: Integer) -> Option<Vec<Reference>> {
        for bucket in self.buckets.values_mut() {
            if let Some(index) = bucket
                .entitity_components
                .iter()
                .position(|(e, _)| entity == *e)
            {
                return Some(bucket.entitity_components.swap_remove(index).1);
            }
        }
        None
    }

    fn archetype(&self, component_types: impl Iterator<Item = Type>) -> BitVec {
        let mut result = BitVec::repeat(false, self.component_table.len());
        for component_type in component_types {
            if let Some(index) = self
                .component_table
                .iter()
                .position(|ty| component_type.is_same_as(ty))
            {
                result.set(index, true);
            }
        }
        result
    }

    fn archetype_contains(bucket: &BitVec, subset: &BitVec) -> bool {
        for (bucket, subset) in bucket.iter().zip(subset.iter()) {
            if *subset && !*bucket {
                return false;
            }
        }
        true
    }
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "IterQuery", module_name = "query")]
pub struct IterQuery {
    #[intuicio(ignore)]
    pub types: Vec<Type>,
    #[intuicio(ignore)]
    pub buckets: Vec<BitVec>,
    #[intuicio(ignore)]
    pub current_bucket: usize,
    #[intuicio(ignore)]
    pub current_entity: usize,
    #[intuicio(ignore)]
    pub world: Reference,
    pub next: Reference,
}

#[intuicio_methods(module_name = "query")]
impl IterQuery {
    #[intuicio_method(use_registry)]
    pub fn next(registry: &Registry, mut iterator: Reference) -> Reference {
        let mut iterator = iterator.write::<IterQuery>().unwrap();
        let world = iterator.world.clone();
        let world = world.read::<World>().unwrap();
        while let Some(bucket) = iterator.buckets.get(iterator.current_bucket) {
            if let Some(bucket) = world.buckets.get(bucket) {
                'entity: while let Some((entity, components)) =
                    bucket.entitity_components.get(iterator.current_entity)
                {
                    iterator.current_entity += 1;
                    let mut result = Vec::with_capacity(1 + iterator.types.len());
                    result.push(Reference::new_integer(*entity, registry));
                    for ty in &iterator.types {
                        if let Some(index) = bucket.types.iter().position(|cty| cty.is_same_as(ty))
                        {
                            result.push(components[index].clone());
                        } else {
                            continue 'entity;
                        }
                    }
                    return Reference::new_array(result, registry);
                }
                iterator.current_entity = 0;
            }
            iterator.current_bucket += 1;
        }
        Reference::null()
    }
}

#[no_mangle]
pub extern "C" fn version() -> IntuicioVersion {
    core_version()
}

#[no_mangle]
pub extern "C" fn install(registry: &mut Registry) {
    registry.add_struct(World::define_struct(registry));
    registry.add_struct(IterQuery::define_struct(registry));
    registry.add_function(World::new__define_function(registry));
    registry.add_function(World::spawn__define_function(registry));
    registry.add_function(World::despawn__define_function(registry));
    registry.add_function(World::add__define_function(registry));
    registry.add_function(World::add_bundle__define_function(registry));
    registry.add_function(World::remove__define_function(registry));
    registry.add_function(World::clear__define_function(registry));
    registry.add_function(World::entities__define_function(registry));
    registry.add_function(World::get__define_function(registry));
    registry.add_function(World::query__define_function(registry));
    registry.add_function(World::maintain__define_function(registry));
    registry.add_function(World::add_resource__define_function(registry));
    registry.add_function(World::remove_resource__define_function(registry));
    registry.add_function(World::resource__define_function(registry));
    registry.add_function(World::resources__define_function(registry));
    registry.add_function(World::snapshot__define_function(registry));
    registry.add_function(IterQuery::next__define_function(registry));
}
