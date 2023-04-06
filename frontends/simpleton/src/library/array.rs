use crate::{library::closure::Closure, Array, Boolean, Function, Integer, Reference};
use intuicio_core::{context::Context, define_native_struct, registry::Registry, IntuicioStruct};
use intuicio_derive::{intuicio_function, intuicio_method, intuicio_methods, IntuicioStruct};

#[intuicio_function(module_name = "array", use_registry)]
pub fn new(registry: &Registry, capacity: Reference) -> Reference {
    Reference::new_array(
        Array::with_capacity(capacity.read::<Integer>().map(|v| *v as _).unwrap_or(0)),
        registry,
    )
}

#[intuicio_function(module_name = "array")]
pub fn reserve(mut array: Reference, additional: Reference) -> Reference {
    array
        .write::<Array>()
        .unwrap()
        .reserve(*additional.read::<Integer>().unwrap() as usize);
    Reference::null()
}

#[intuicio_function(module_name = "array", use_registry)]
pub fn size(registry: &Registry, array: Reference) -> Reference {
    Reference::new_integer(array.read::<Array>().unwrap().len() as Integer, registry)
}

#[intuicio_function(module_name = "array", use_registry)]
pub fn capacity(registry: &Registry, array: Reference) -> Reference {
    Reference::new_integer(
        array.read::<Array>().unwrap().capacity() as Integer,
        registry,
    )
}

#[intuicio_function(module_name = "array")]
pub fn clear(mut array: Reference) -> Reference {
    array.write::<Array>().unwrap().clear();
    Reference::null()
}

#[intuicio_function(module_name = "array", use_registry)]
pub fn contains(registry: &Registry, map: Reference, value: Reference) -> Reference {
    let result = map
        .read::<Array>()
        .unwrap()
        .iter()
        .any(|item| value.does_share_reference(item, true));
    Reference::new_boolean(result, registry)
}

#[intuicio_function(module_name = "array", use_registry)]
pub fn find(
    registry: &Registry,
    array: Reference,
    value: Reference,
    reverse: Reference,
) -> Reference {
    if *reverse.read::<Boolean>().unwrap() {
        array
            .read::<Array>()
            .unwrap()
            .iter()
            .rev()
            .position(|item| value.does_share_reference(item, true))
            .map(|index| Reference::new_integer(index as Integer, registry))
            .unwrap_or_default()
    } else {
        array
            .read::<Array>()
            .unwrap()
            .iter()
            .position(|item| value.does_share_reference(item, true))
            .map(|index| Reference::new_integer(index as Integer, registry))
            .unwrap_or_default()
    }
}

#[intuicio_function(module_name = "array")]
pub fn push(mut array: Reference, value: Reference) -> Reference {
    array.write::<Array>().unwrap().push(value);
    Reference::null()
}

#[intuicio_function(module_name = "array")]
pub fn insert(mut array: Reference, index: Reference, value: Reference) -> Reference {
    array
        .write::<Array>()
        .unwrap()
        .insert(*index.read::<Integer>().unwrap() as _, value);
    Reference::null()
}

#[intuicio_function(module_name = "array")]
pub fn pop(mut array: Reference) -> Reference {
    array.write::<Array>().unwrap().pop().unwrap_or_default()
}

#[intuicio_function(module_name = "array")]
pub fn remove(mut array: Reference, index: Reference) -> Reference {
    array
        .write::<Array>()
        .unwrap()
        .remove(*index.read::<Integer>().unwrap() as _)
}

#[intuicio_function(module_name = "array")]
pub fn set(mut array: Reference, index: Reference, value: Reference) -> Reference {
    array
        .write::<Array>()
        .unwrap()
        .get_mut(*index.read::<Integer>().unwrap() as usize)
        .map(|item| std::mem::replace(item, value))
        .unwrap_or_default()
}

#[intuicio_function(module_name = "array")]
pub fn get(array: Reference, index: Reference) -> Reference {
    array
        .read::<Array>()
        .unwrap()
        .get(*index.read::<Integer>().unwrap() as usize)
        .cloned()
        .unwrap_or_default()
}

#[intuicio_function(module_name = "array", use_registry)]
pub fn slice(
    registry: &Registry,
    array: Reference,
    index: Reference,
    count: Reference,
) -> Reference {
    let index = *index.read::<Integer>().unwrap() as usize;
    let count = *count.read::<Integer>().unwrap() as usize;
    Reference::new_array(
        array.read::<Array>().unwrap()[index..(index + count)].to_vec(),
        registry,
    )
}

#[intuicio_function(module_name = "array", use_registry)]
pub fn join(registry: &Registry, a: Reference, b: Reference) -> Reference {
    Reference::new_array(
        a.read::<Array>()
            .unwrap()
            .iter()
            .chain(b.read::<Array>().unwrap().iter())
            .cloned()
            .collect::<Array>(),
        registry,
    )
}

#[intuicio_function(module_name = "array", use_registry)]
pub fn iter(registry: &Registry, array: Reference, reversed: Reference) -> Reference {
    Reference::new(
        ArrayIter {
            array,
            index: 0,
            reversed: *reversed.read::<Boolean>().unwrap(),
            next: Reference::new(
                Closure {
                    function: Function::by_name("next", "array_iter", registry).unwrap(),
                    captured: vec![],
                },
                registry,
            ),
        },
        registry,
    )
}

#[intuicio_function(module_name = "array", use_context, use_registry)]
pub fn collect(context: &mut Context, registry: &Registry, iterator: Reference) -> Reference {
    let mut result = Array::new();
    loop {
        let value = crate::library::iter::next(context, registry, iterator.clone());
        if value.is_null() {
            break;
        }
        result.push(value);
    }
    Reference::new_array(result, registry)
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "ArrayIter", module_name = "array_iter")]
pub struct ArrayIter {
    #[intuicio(ignore)]
    pub array: Reference,
    #[intuicio(ignore)]
    pub index: usize,
    #[intuicio(ignore)]
    pub reversed: bool,
    pub next: Reference,
}

#[intuicio_methods(module_name = "array_iter")]
impl ArrayIter {
    #[intuicio_method()]
    pub fn next(mut iterator: Reference) -> Reference {
        let mut iterator = iterator.write::<ArrayIter>().unwrap();
        let array = iterator.array.clone();
        let array = array.read::<Array>().unwrap();
        if iterator.index >= array.len() {
            return Reference::null();
        }
        let index = if iterator.reversed {
            array.len() - iterator.index - 1
        } else {
            iterator.index
        };
        iterator.index += 1;
        array.get(index).cloned().unwrap_or_default()
    }
}

pub fn install(registry: &mut Registry) {
    registry.add_struct(define_native_struct! {
        registry => mod array struct Array (Array) {}
    });
    registry.add_function(new::define_function(registry));
    registry.add_function(reserve::define_function(registry));
    registry.add_function(size::define_function(registry));
    registry.add_function(capacity::define_function(registry));
    registry.add_function(clear::define_function(registry));
    registry.add_function(contains::define_function(registry));
    registry.add_function(find::define_function(registry));
    registry.add_function(push::define_function(registry));
    registry.add_function(insert::define_function(registry));
    registry.add_function(pop::define_function(registry));
    registry.add_function(remove::define_function(registry));
    registry.add_function(set::define_function(registry));
    registry.add_function(get::define_function(registry));
    registry.add_function(slice::define_function(registry));
    registry.add_function(join::define_function(registry));
    registry.add_function(iter::define_function(registry));
    registry.add_function(collect::define_function(registry));
    registry.add_struct(ArrayIter::define_struct(registry));
    registry.add_function(ArrayIter::next__define_function(registry));
}
