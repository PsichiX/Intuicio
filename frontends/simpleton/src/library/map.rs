use crate::{library::closure::Closure, Array, Function, Integer, Map, Reference, Text};
use intuicio_core::{context::Context, define_native_struct, registry::Registry, IntuicioStruct};
use intuicio_derive::{intuicio_function, intuicio_method, intuicio_methods, IntuicioStruct};

#[intuicio_function(module_name = "map", use_registry)]
pub fn new(registry: &Registry, capacity: Reference) -> Reference {
    Reference::new_map(
        Map::with_capacity(capacity.read::<Integer>().map(|v| *v as _).unwrap_or(0)),
        registry,
    )
}

#[intuicio_function(module_name = "map")]
pub fn reserve(mut map: Reference, additional: Reference) -> Reference {
    map.write::<Map>()
        .unwrap()
        .reserve(*additional.read::<Integer>().unwrap() as usize);
    Reference::null()
}

#[intuicio_function(module_name = "map", use_registry)]
pub fn size(registry: &Registry, map: Reference) -> Reference {
    Reference::new_integer(map.read::<Map>().unwrap().len() as Integer, registry)
}

#[intuicio_function(module_name = "map", use_registry)]
pub fn capacity(registry: &Registry, map: Reference) -> Reference {
    Reference::new_integer(map.read::<Map>().unwrap().capacity() as Integer, registry)
}

#[intuicio_function(module_name = "map")]
pub fn clear(mut map: Reference) -> Reference {
    map.write::<Map>().unwrap().clear();
    Reference::null()
}

#[intuicio_function(module_name = "map", use_registry)]
pub fn contains_key(registry: &Registry, mut map: Reference, key: Reference) -> Reference {
    let result = map
        .write::<Map>()
        .unwrap()
        .contains_key(key.read::<Text>().unwrap().as_str());
    Reference::new_boolean(result, registry)
}

#[intuicio_function(module_name = "map", use_registry)]
pub fn contains_value(registry: &Registry, mut map: Reference, value: Reference) -> Reference {
    let result = map
        .write::<Map>()
        .unwrap()
        .values()
        .any(|item| value.does_share_reference(item, true));
    Reference::new_boolean(result, registry)
}

#[intuicio_function(module_name = "map", use_registry)]
pub fn find_key(registry: &Registry, mut map: Reference, value: Reference) -> Reference {
    map.write::<Map>()
        .unwrap()
        .iter()
        .find(|(_, item)| value.does_share_reference(item, true))
        .map(|(key, _)| Reference::new_text(key.to_owned(), registry))
        .unwrap_or_default()
}

#[intuicio_function(module_name = "map")]
pub fn remove(mut map: Reference, key: Reference) -> Reference {
    map.write::<Map>()
        .unwrap()
        .remove(key.read::<Text>().unwrap().as_str())
        .unwrap_or_default()
}

#[intuicio_function(module_name = "map")]
pub fn set(mut map: Reference, key: Reference, value: Reference) -> Reference {
    map.write::<Map>()
        .unwrap()
        .insert(key.read::<Text>().unwrap().to_owned(), value)
        .unwrap_or_default()
}

#[intuicio_function(module_name = "map")]
pub fn get(mut map: Reference, key: Reference) -> Reference {
    map.write::<Map>()
        .unwrap()
        .get(key.read::<Text>().unwrap().as_str())
        .cloned()
        .unwrap_or_default()
}

#[intuicio_function(module_name = "map", use_registry)]
pub fn join(registry: &Registry, a: Reference, b: Reference) -> Reference {
    Reference::new_map(
        a.read::<Map>()
            .unwrap()
            .iter()
            .chain(b.read::<Map>().unwrap().iter())
            .map(|(k, v)| (k.to_owned(), v.clone()))
            .collect::<Map>(),
        registry,
    )
}

#[intuicio_function(module_name = "map", use_registry)]
pub fn zip(registry: &Registry, keys: Reference, values: Reference) -> Reference {
    Reference::new_map(
        keys.read::<Array>()
            .unwrap()
            .iter()
            .map(|key| key.read::<Text>().unwrap().to_owned())
            .zip(values.read::<Array>().unwrap().iter().cloned())
            .collect::<Map>(),
        registry,
    )
}

#[intuicio_function(module_name = "map", use_registry)]
pub fn keys(registry: &Registry, mut map: Reference) -> Reference {
    Reference::new_array(
        map.write::<Map>()
            .unwrap()
            .keys()
            .map(|key| Reference::new_text(key.to_owned(), registry))
            .collect::<Array>(),
        registry,
    )
}

#[intuicio_function(module_name = "map", use_registry)]
pub fn values(registry: &Registry, mut map: Reference) -> Reference {
    Reference::new_array(
        map.write::<Map>()
            .unwrap()
            .values()
            .cloned()
            .collect::<Array>(),
        registry,
    )
}

#[intuicio_function(module_name = "map", use_registry)]
pub fn iter(registry: &Registry, map: Reference) -> Reference {
    let keys = map.read::<Map>().unwrap().keys().cloned().collect();
    Reference::new(
        MapIter {
            map,
            index: 0,
            keys,
            next: Reference::new(
                Closure {
                    function: Function::by_name("next", "map_iter", registry).unwrap(),
                    captured: vec![],
                },
                registry,
            ),
        },
        registry,
    )
}

#[intuicio_function(module_name = "map", use_context, use_registry)]
pub fn collect(context: &mut Context, registry: &Registry, iterator: Reference) -> Reference {
    let mut result = Map::new();
    loop {
        let value = crate::library::iter::next(context, registry, iterator.clone());
        if value.is_null() {
            break;
        }
        if let Some(pair) = value.read::<Array>() {
            let key = pair.get(0).unwrap().read::<Text>().unwrap().to_owned();
            let value = pair.get(1).unwrap().clone();
            result.insert(key, value);
        } else if let Some(pair) = value.read::<Map>() {
            let key = pair.get("key").unwrap().read::<Text>().unwrap().to_owned();
            let value = pair.get("value").unwrap().clone();
            result.insert(key, value);
        } else if let Some(pair) = value.read_object() {
            let key = pair
                .read_field::<Reference>("key")
                .unwrap()
                .read::<Text>()
                .unwrap()
                .to_owned();
            let value = pair.read_field::<Reference>("value").unwrap().clone();
            result.insert(key, value);
        };
    }
    Reference::new_map(result, registry)
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "Pair", module_name = "map")]
pub struct Pair {
    pub key: Reference,
    pub value: Reference,
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "MapIter", module_name = "map_iter")]
pub struct MapIter {
    #[intuicio(ignore)]
    pub map: Reference,
    #[intuicio(ignore)]
    pub index: usize,
    #[intuicio(ignore)]
    pub keys: Vec<String>,
    pub next: Reference,
}

#[intuicio_methods(module_name = "map_iter")]
impl MapIter {
    #[intuicio_method(use_registry)]
    pub fn next(registry: &Registry, mut iterator: Reference) -> Reference {
        let mut iterator = iterator.write::<MapIter>().unwrap();
        let map = iterator.map.clone();
        let map = map.read::<Map>().unwrap();
        if iterator.index >= map.len() {
            return Reference::null();
        }
        let index = iterator.index;
        iterator.index += 1;
        let key = iterator.keys.get(index).unwrap();
        map.get(key)
            .map(|value| {
                Reference::new(
                    Pair {
                        key: Reference::new_text(key.to_owned(), registry),
                        value: value.clone(),
                    },
                    registry,
                )
            })
            .unwrap_or_default()
    }
}

pub fn install(registry: &mut Registry) {
    registry.add_struct(define_native_struct! {
        registry => mod map struct Map (Map) {}
    });
    registry.add_function(new::define_function(registry));
    registry.add_function(reserve::define_function(registry));
    registry.add_function(size::define_function(registry));
    registry.add_function(capacity::define_function(registry));
    registry.add_function(clear::define_function(registry));
    registry.add_function(contains_key::define_function(registry));
    registry.add_function(contains_value::define_function(registry));
    registry.add_function(find_key::define_function(registry));
    registry.add_function(remove::define_function(registry));
    registry.add_function(set::define_function(registry));
    registry.add_function(get::define_function(registry));
    registry.add_function(join::define_function(registry));
    registry.add_function(zip::define_function(registry));
    registry.add_function(keys::define_function(registry));
    registry.add_function(values::define_function(registry));
    registry.add_function(iter::define_function(registry));
    registry.add_function(collect::define_function(registry));
    registry.add_struct(Pair::define_struct(registry));
    registry.add_struct(MapIter::define_struct(registry));
    registry.add_function(MapIter::next__define_function(registry));
}

#[macro_export]
macro_rules! map {
    ($( $key:ident : $value:expr ),* $(,)?) => {{
        let mut result = $crate::Map::new();
        $(
            result.insert(stringify!($key).to_string(), $value);
        )*
        result
    }};
}
