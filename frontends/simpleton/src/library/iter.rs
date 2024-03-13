use crate::{library::closure::Closure, Array, Boolean, Function, Integer, Map, Reference};
use intuicio_core::{context::Context, registry::Registry, IntuicioStruct};
use intuicio_derive::{intuicio_function, intuicio_method, intuicio_methods, IntuicioStruct};

const GENERATOR: &str = "next";

#[intuicio_function(module_name = "iter", use_context, use_registry)]
pub fn next(context: &mut Context, registry: &Registry, iterator: Reference) -> Reference {
    let iter = iterator.clone();
    if let Some(closure) = iterator.read::<Closure>() {
        closure.invoke(context, registry, &[iter])
    } else if let Some(map) = iterator.read::<Map>() {
        map.get(GENERATOR)
            .unwrap()
            .read::<Closure>()
            .unwrap()
            .invoke(context, registry, &[iter])
    } else {
        let closure = {
            let iterator = iterator.read_object().unwrap();
            iterator.read_field::<Reference>(GENERATOR).unwrap().clone()
        };
        let closure = closure.read::<Closure>().unwrap();
        closure.invoke(context, registry, &[iter])
    }
}

pub fn build_impl(
    context: &mut Context,
    registry: &Registry,
    previous: Option<Reference>,
    next: &[Reference],
) -> Reference {
    if next.is_empty() {
        return previous.unwrap_or_default();
    }
    let current = &next[0];
    if let Some(current) = current.read::<Array>() {
        let function = current[0].read::<Function>().unwrap();
        for argument in current[1..].iter().rev() {
            context.stack().push(argument.to_owned());
        }
        if let Some(previous) = previous {
            context.stack().push(previous);
        }
        function.handle().unwrap().invoke(context, registry);
        let previous = context.stack().pop::<Reference>().unwrap();
        build_impl(context, registry, Some(previous), &next[1..])
    } else {
        build_impl(context, registry, Some(current.clone()), &next[1..])
    }
}

#[intuicio_function(module_name = "iter", use_context, use_registry)]
pub fn build(context: &mut Context, registry: &Registry, iterators: Reference) -> Reference {
    let iterators = iterators.read::<Array>().unwrap();
    build_impl(context, registry, None, &iterators)
}

#[intuicio_function(module_name = "iter", use_registry)]
pub fn enumerate(registry: &Registry, iterator: Reference) -> Reference {
    Reference::new(
        IterEnumerate {
            iterator,
            index: 0,
            next: Reference::new(
                Closure {
                    function: Function::by_name("next", "iter_enumerate", registry).unwrap(),
                    captured: vec![],
                },
                registry,
            ),
        },
        registry,
    )
}

#[intuicio_function(module_name = "iter", use_registry)]
pub fn walk(registry: &Registry, start: Reference, steps: Reference) -> Reference {
    Reference::new(
        IterWalk {
            current: *start.read::<Integer>().unwrap(),
            steps: *steps.read::<Integer>().unwrap(),
            next: Reference::new(
                Closure {
                    function: Function::by_name("next", "iter_walk", registry).unwrap(),
                    captured: vec![],
                },
                registry,
            ),
        },
        registry,
    )
}

#[intuicio_function(module_name = "iter", use_registry)]
pub fn range(registry: &Registry, from: Reference, to: Reference) -> Reference {
    let from = *from.read::<Integer>().unwrap();
    let to = *to.read::<Integer>().unwrap();
    Reference::new(
        IterWalk {
            current: from,
            steps: to - from,
            next: Reference::new(
                Closure {
                    function: Function::by_name("next", "iter_walk", registry).unwrap(),
                    captured: vec![],
                },
                registry,
            ),
        },
        registry,
    )
}

#[intuicio_function(module_name = "iter", use_registry)]
pub fn range_inclusive(registry: &Registry, from: Reference, to: Reference) -> Reference {
    let from = *from.read::<Integer>().unwrap();
    let to = *to.read::<Integer>().unwrap();
    let steps = to - from;
    Reference::new(
        IterWalk {
            current: from,
            steps: steps + steps.signum(),
            next: Reference::new(
                Closure {
                    function: Function::by_name("next", "iter_walk", registry).unwrap(),
                    captured: vec![],
                },
                registry,
            ),
        },
        registry,
    )
}

#[intuicio_function(module_name = "iter", use_registry)]
pub fn filter(registry: &Registry, iterator: Reference, closure: Reference) -> Reference {
    Reference::new(
        IterFilter {
            iterator,
            closure,
            next: Reference::new(
                Closure {
                    function: Function::by_name("next", "iter_filter", registry).unwrap(),
                    captured: vec![],
                },
                registry,
            ),
        },
        registry,
    )
}

#[intuicio_function(module_name = "iter", use_registry)]
pub fn map(registry: &Registry, iterator: Reference, closure: Reference) -> Reference {
    Reference::new(
        IterMap {
            iterator,
            closure,
            next: Reference::new(
                Closure {
                    function: Function::by_name("next", "iter_map", registry).unwrap(),
                    captured: vec![],
                },
                registry,
            ),
        },
        registry,
    )
}

#[intuicio_function(module_name = "iter", use_registry)]
pub fn filter_map(registry: &Registry, iterator: Reference, closure: Reference) -> Reference {
    Reference::new(
        IterFilterMap {
            iterator,
            closure,
            next: Reference::new(
                Closure {
                    function: Function::by_name("next", "iter_filter_map", registry).unwrap(),
                    captured: vec![],
                },
                registry,
            ),
        },
        registry,
    )
}

#[intuicio_function(module_name = "iter", use_registry)]
pub fn flatten(registry: &Registry, iterator: Reference) -> Reference {
    Reference::new(
        IterFlatten {
            iterator,
            current: Reference::null(),
            next: Reference::new(
                Closure {
                    function: Function::by_name("next", "iter_flatten", registry).unwrap(),
                    captured: vec![],
                },
                registry,
            ),
        },
        registry,
    )
}

#[intuicio_function(module_name = "iter", use_registry)]
pub fn chain(registry: &Registry, iterators: Reference) -> Reference {
    Reference::new(
        IterChain {
            iterators: iterators.read::<Array>().unwrap().clone(),
            current: 0,
            next: Reference::new(
                Closure {
                    function: Function::by_name("next", "iter_chain", registry).unwrap(),
                    captured: vec![],
                },
                registry,
            ),
        },
        registry,
    )
}

#[intuicio_function(module_name = "iter", use_registry)]
pub fn zip(registry: &Registry, iterators: Reference) -> Reference {
    Reference::new(
        IterZip {
            iterators: iterators.read::<Array>().unwrap().clone(),
            next: Reference::new(
                Closure {
                    function: Function::by_name("next", "iter_zip", registry).unwrap(),
                    captured: vec![],
                },
                registry,
            ),
        },
        registry,
    )
}

#[intuicio_function(module_name = "iter", use_registry)]
pub fn chunks(registry: &Registry, iterator: Reference, count: Reference) -> Reference {
    Reference::new(
        IterChunks {
            iterator,
            count,
            next: Reference::new(
                Closure {
                    function: Function::by_name("next", "iter_chunks", registry).unwrap(),
                    captured: vec![],
                },
                registry,
            ),
        },
        registry,
    )
}

#[intuicio_function(module_name = "iter", use_context, use_registry)]
pub fn fold(
    context: &mut Context,
    registry: &Registry,
    iterator: Reference,
    mut start: Reference,
    closure: Reference,
) -> Reference {
    loop {
        let value = crate::library::iter::next(context, registry, iterator.clone());
        if value.is_null() {
            return start;
        }
        let closure = closure.read::<Closure>().unwrap();
        start = closure.invoke(context, registry, &[start.clone(), value]);
    }
}

#[intuicio_function(module_name = "iter", use_context, use_registry)]
pub fn find(
    context: &mut Context,
    registry: &Registry,
    iterator: Reference,
    closure: Reference,
) -> Reference {
    loop {
        let value = crate::library::iter::next(context, registry, iterator.clone());
        if value.is_null() {
            return value;
        }
        let closure = closure.read::<Closure>().unwrap();
        let result = closure.invoke(context, registry, &[value.clone()]);
        if *result.read::<Boolean>().unwrap() {
            return value;
        }
    }
}

#[intuicio_function(module_name = "iter", use_context, use_registry)]
pub fn find_map(
    context: &mut Context,
    registry: &Registry,
    iterator: Reference,
    closure: Reference,
) -> Reference {
    loop {
        let value = crate::library::iter::next(context, registry, iterator.clone());
        if value.is_null() {
            return value;
        }
        let closure = closure.read::<Closure>().unwrap();
        let result = closure.invoke(context, registry, &[value]);
        if !result.is_null() {
            return result;
        }
    }
}

#[intuicio_function(module_name = "iter", use_context, use_registry)]
pub fn position(
    context: &mut Context,
    registry: &Registry,
    iterator: Reference,
    closure: Reference,
) -> Reference {
    let mut index = 0;
    loop {
        let value = crate::library::iter::next(context, registry, iterator.clone());
        if value.is_null() {
            return value;
        }
        let closure = closure.read::<Closure>().unwrap();
        let result = closure.invoke(context, registry, &[value]);
        if *result.read::<Boolean>().unwrap() {
            return Reference::new_integer(index, registry);
        }
        index += 1;
    }
}

#[intuicio_function(module_name = "iter", use_context, use_registry)]
pub fn any(
    context: &mut Context,
    registry: &Registry,
    iterator: Reference,
    closure: Reference,
) -> Reference {
    loop {
        let value = crate::library::iter::next(context, registry, iterator.clone());
        if value.is_null() {
            return Reference::new_boolean(false, registry);
        }
        let closure = closure.read::<Closure>().unwrap();
        let result = closure.invoke(context, registry, &[value]);
        if *result.read::<Boolean>().unwrap() {
            return Reference::new_boolean(true, registry);
        }
    }
}

#[intuicio_function(module_name = "iter", use_context, use_registry)]
pub fn all(
    context: &mut Context,
    registry: &Registry,
    iterator: Reference,
    closure: Reference,
) -> Reference {
    loop {
        let value = crate::library::iter::next(context, registry, iterator.clone());
        if value.is_null() {
            return Reference::new_boolean(true, registry);
        }
        let closure = closure.read::<Closure>().unwrap();
        let result = closure.invoke(context, registry, &[value]);
        if !*result.read::<Boolean>().unwrap() {
            return Reference::new_boolean(false, registry);
        }
    }
}

#[intuicio_function(module_name = "iter", use_context, use_registry)]
pub fn count(context: &mut Context, registry: &Registry, iterator: Reference) -> Reference {
    let mut result = 0;
    loop {
        let value = crate::library::iter::next(context, registry, iterator.clone());
        if value.is_null() {
            return Reference::new_integer(result, registry);
        }
        result += 1;
    }
}

#[intuicio_function(module_name = "iter", use_context, use_registry)]
pub fn compared_by(
    context: &mut Context,
    registry: &Registry,
    iterator: Reference,
    closure: Reference,
) -> Reference {
    let mut found = crate::library::iter::next(context, registry, iterator.clone());
    if found.is_null() {
        return Reference::null();
    }
    loop {
        let value = crate::library::iter::next(context, registry, iterator.clone());
        if value.is_null() {
            return found;
        }
        let closure = closure.read::<Closure>().unwrap();
        let result = closure.invoke(context, registry, &[value.clone(), found.clone()]);
        if *result.read::<Boolean>().unwrap() {
            found = value;
        }
    }
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "IterWalk", module_name = "iter_walk")]
pub struct IterWalk {
    #[intuicio(ignore)]
    pub current: Integer,
    #[intuicio(ignore)]
    pub steps: Integer,
    pub next: Reference,
}

#[intuicio_methods(module_name = "iter_walk")]
impl IterWalk {
    #[intuicio_method(use_registry)]
    pub fn next(registry: &Registry, mut iterator: Reference) -> Reference {
        let mut iterator = iterator.write::<IterWalk>().unwrap();
        if iterator.steps.abs() == 0 {
            return Reference::null();
        }
        let value = iterator.current;
        let step = iterator.steps.signum();
        iterator.current += step;
        iterator.steps -= step;
        Reference::new_integer(value, registry)
    }
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "Enumeration", module_name = "iter")]
pub struct Enumeration {
    pub index: Reference,
    pub value: Reference,
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "IterEnumerate", module_name = "iter_enumerate")]
pub struct IterEnumerate {
    #[intuicio(ignore)]
    pub iterator: Reference,
    #[intuicio(ignore)]
    pub index: usize,
    pub next: Reference,
}

#[intuicio_methods(module_name = "iter_enumerate")]
impl IterEnumerate {
    #[intuicio_method(use_context, use_registry)]
    pub fn next(context: &mut Context, registry: &Registry, mut iterator: Reference) -> Reference {
        let mut iterator = iterator.write::<IterEnumerate>().unwrap();
        let value = next(context, registry, iterator.iterator.clone());
        if value.is_null() {
            return Reference::null();
        }
        let index = iterator.index;
        iterator.index += 1;
        Reference::new(
            Enumeration {
                index: Reference::new_integer(index as Integer, registry),
                value,
            },
            registry,
        )
    }
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "IterFilter", module_name = "iter_filter")]
pub struct IterFilter {
    #[intuicio(ignore)]
    pub iterator: Reference,
    #[intuicio(ignore)]
    pub closure: Reference,
    pub next: Reference,
}

#[intuicio_methods(module_name = "iter_filter")]
impl IterFilter {
    #[intuicio_method(use_context, use_registry)]
    pub fn next(context: &mut Context, registry: &Registry, iterator: Reference) -> Reference {
        let iterator = iterator.read::<IterFilter>().unwrap();
        loop {
            let value = next(context, registry, iterator.iterator.clone());
            if value.is_null() {
                return value;
            }
            let closure = iterator.closure.read::<Closure>().unwrap();
            let result = closure.invoke(context, registry, &[value.clone()]);
            if *result.read::<Boolean>().unwrap() {
                return value;
            }
        }
    }
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "IterMap", module_name = "iter_map")]
pub struct IterMap {
    #[intuicio(ignore)]
    pub iterator: Reference,
    #[intuicio(ignore)]
    pub closure: Reference,
    pub next: Reference,
}

#[intuicio_methods(module_name = "iter_map")]
impl IterMap {
    #[intuicio_method(use_context, use_registry)]
    pub fn next(context: &mut Context, registry: &Registry, iterator: Reference) -> Reference {
        let iterator = iterator.read::<IterMap>().unwrap();
        let value = next(context, registry, iterator.iterator.clone());
        if value.is_null() {
            value
        } else {
            let closure = iterator.closure.read::<Closure>().unwrap();
            closure.invoke(context, registry, &[value])
        }
    }
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "IterMap", module_name = "iter_filter_map")]
pub struct IterFilterMap {
    #[intuicio(ignore)]
    pub iterator: Reference,
    #[intuicio(ignore)]
    pub closure: Reference,
    pub next: Reference,
}

#[intuicio_methods(module_name = "iter_filter_map")]
impl IterFilterMap {
    #[intuicio_method(use_context, use_registry)]
    pub fn next(context: &mut Context, registry: &Registry, iterator: Reference) -> Reference {
        let iterator = iterator.read::<IterFilterMap>().unwrap();
        loop {
            let value = next(context, registry, iterator.iterator.clone());
            if value.is_null() {
                return value;
            }
            let closure = iterator.closure.read::<Closure>().unwrap();
            let result = closure.invoke(context, registry, &[value]);
            if !result.is_null() {
                return result;
            }
        }
    }
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "IterFlatten", module_name = "iter_flatten")]
pub struct IterFlatten {
    #[intuicio(ignore)]
    pub iterator: Reference,
    #[intuicio(ignore)]
    pub current: Reference,
    pub next: Reference,
}

#[intuicio_methods(module_name = "iter_flatten")]
impl IterFlatten {
    #[intuicio_method(use_context, use_registry)]
    pub fn next(context: &mut Context, registry: &Registry, mut iterator: Reference) -> Reference {
        let mut iterator = iterator.write::<IterFlatten>().unwrap();
        loop {
            if iterator.current.is_null() {
                iterator.current = next(context, registry, iterator.iterator.clone());
                if iterator.current.is_null() {
                    return Reference::null();
                }
            }
            let value = next(context, registry, iterator.current.clone());
            if value.is_null() {
                iterator.current = Reference::null()
            } else {
                return value;
            }
        }
    }
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "IterChain", module_name = "iter_chain")]
pub struct IterChain {
    #[intuicio(ignore)]
    pub iterators: Array,
    #[intuicio(ignore)]
    pub current: usize,
    pub next: Reference,
}

#[intuicio_methods(module_name = "iter_chain")]
impl IterChain {
    #[intuicio_method(use_context, use_registry)]
    pub fn next(context: &mut Context, registry: &Registry, mut iterator: Reference) -> Reference {
        let mut iterator = iterator.write::<IterChain>().unwrap();
        while let Some(current) = iterator.iterators.get(iterator.current) {
            let value = next(context, registry, current.clone());
            if value.is_null() {
                iterator.current += 1;
            } else {
                return value;
            }
        }
        Reference::null()
    }
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "IterZip", module_name = "iter_zip")]
pub struct IterZip {
    #[intuicio(ignore)]
    pub iterators: Array,
    pub next: Reference,
}

#[intuicio_methods(module_name = "iter_zip")]
impl IterZip {
    #[intuicio_method(use_context, use_registry)]
    pub fn next(context: &mut Context, registry: &Registry, iterator: Reference) -> Reference {
        let iterator = iterator.read::<IterZip>().unwrap();
        let result = iterator
            .iterators
            .iter()
            .map(|iterator| next(context, registry, iterator.clone()))
            .collect::<Array>();
        if result.iter().any(|value| value.is_null()) {
            Reference::null()
        } else {
            Reference::new_array(result, registry)
        }
    }
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "IterChunks", module_name = "iter_chunks")]
pub struct IterChunks {
    #[intuicio(ignore)]
    pub iterator: Reference,
    #[intuicio(ignore)]
    pub count: Reference,
    pub next: Reference,
}

#[intuicio_methods(module_name = "iter_chunks")]
impl IterChunks {
    #[intuicio_method(use_context, use_registry)]
    pub fn next(context: &mut Context, registry: &Registry, iterator: Reference) -> Reference {
        let iterator = iterator.read::<IterChunks>().unwrap();
        let count = *iterator.count.read::<Integer>().unwrap() as usize;
        if count == 0 {
            return Reference::null();
        }
        let mut result = Vec::with_capacity(count);
        for _ in 0..count {
            let value = next(context, registry, iterator.iterator.clone());
            if value.is_null() {
                return Reference::null();
            }
            result.push(value);
        }
        Reference::new_array(result, registry)
    }
}

pub fn install(registry: &mut Registry) {
    registry.add_function(next::define_function(registry));
    registry.add_function(build::define_function(registry));
    registry.add_function(walk::define_function(registry));
    registry.add_function(range::define_function(registry));
    registry.add_function(range_inclusive::define_function(registry));
    registry.add_function(enumerate::define_function(registry));
    registry.add_function(filter::define_function(registry));
    registry.add_function(map::define_function(registry));
    registry.add_function(filter_map::define_function(registry));
    registry.add_function(flatten::define_function(registry));
    registry.add_function(chain::define_function(registry));
    registry.add_function(zip::define_function(registry));
    registry.add_function(chunks::define_function(registry));
    registry.add_function(fold::define_function(registry));
    registry.add_function(find::define_function(registry));
    registry.add_function(find_map::define_function(registry));
    registry.add_function(position::define_function(registry));
    registry.add_function(any::define_function(registry));
    registry.add_function(all::define_function(registry));
    registry.add_function(count::define_function(registry));
    registry.add_function(compared_by::define_function(registry));
    registry.add_type(IterWalk::define_struct(registry));
    registry.add_function(IterWalk::next__define_function(registry));
    registry.add_type(Enumeration::define_struct(registry));
    registry.add_type(IterEnumerate::define_struct(registry));
    registry.add_function(IterEnumerate::next__define_function(registry));
    registry.add_type(IterFilter::define_struct(registry));
    registry.add_function(IterFilter::next__define_function(registry));
    registry.add_type(IterMap::define_struct(registry));
    registry.add_function(IterMap::next__define_function(registry));
    registry.add_type(IterFilterMap::define_struct(registry));
    registry.add_function(IterFilterMap::next__define_function(registry));
    registry.add_type(IterFlatten::define_struct(registry));
    registry.add_function(IterFlatten::next__define_function(registry));
    registry.add_type(IterChain::define_struct(registry));
    registry.add_function(IterChain::next__define_function(registry));
    registry.add_type(IterZip::define_struct(registry));
    registry.add_function(IterZip::next__define_function(registry));
    registry.add_type(IterChunks::define_struct(registry));
    registry.add_function(IterChunks::next__define_function(registry));
}
