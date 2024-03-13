use crate::{Array, Boolean, Function, Integer, Map, Real, Reference, Text, Type};
use intuicio_core::{context::Context, define_native_struct, object::Object, registry::Registry};
use intuicio_derive::intuicio_function;
use std::collections::HashSet;

#[intuicio_function(module_name = "reflect", use_registry)]
pub fn find_type_by_name(
    registry: &Registry,
    name: Reference,
    module_name: Reference,
) -> Reference {
    Reference::new_type(
        Type::by_name(
            name.read::<Text>().unwrap().as_str(),
            module_name.read::<Text>().unwrap().as_str(),
            registry,
        )
        .unwrap(),
        registry,
    )
}

#[intuicio_function(module_name = "reflect", use_registry)]
pub fn type_of(registry: &Registry, value: Reference) -> Reference {
    Reference::new_type(value.type_of().unwrap(), registry)
}

#[intuicio_function(module_name = "reflect", use_registry)]
pub fn type_name(registry: &Registry, ty: Reference) -> Reference {
    Reference::new_text(
        ty.read::<Type>()
            .unwrap()
            .handle()
            .unwrap()
            .name()
            .to_owned(),
        registry,
    )
}

#[intuicio_function(module_name = "reflect", use_registry)]
pub fn type_fields(registry: &Registry, ty: Reference) -> Reference {
    Reference::new_array(
        ty.read::<Type>()
            .unwrap()
            .handle()
            .unwrap()
            .struct_fields()
            .unwrap()
            .iter()
            .map(|field| Reference::new_text(field.name.to_owned(), registry))
            .collect::<Array>(),
        registry,
    )
}

#[intuicio_function(module_name = "reflect", use_registry)]
pub fn type_byte_size(registry: &Registry, ty: Reference) -> Reference {
    Reference::new_integer(
        ty.read::<Type>().unwrap().handle().unwrap().layout().size() as Integer,
        registry,
    )
}

#[intuicio_function(module_name = "reflect")]
pub fn get_field(object: Reference, name: Reference) -> Reference {
    object
        .read_object()
        .unwrap()
        .read_field::<Reference>(name.read::<Text>().unwrap().as_str())
        .unwrap()
        .clone()
}

#[intuicio_function(module_name = "reflect")]
pub fn set_field(mut object: Reference, name: Reference, value: Reference) -> Reference {
    *object
        .write_object()
        .unwrap()
        .write_field::<Reference>(name.read::<Text>().unwrap().as_str())
        .unwrap() = value;
    Reference::null()
}

#[intuicio_function(module_name = "reflect")]
pub fn new(ty: Reference, properties: Reference) -> Reference {
    let type_ = ty.read::<Type>().unwrap().handle().unwrap().clone();
    let mut result = Object::new(type_);
    for (key, value) in properties.read::<Map>().unwrap().iter() {
        *result.write_field::<Reference>(key).unwrap() = value.clone();
    }
    Reference::new_raw(result)
}

#[intuicio_function(module_name = "reflect")]
pub fn pack(mut object: Reference, properties: Reference) -> Reference {
    let mut object = object.write_object().unwrap();
    for (key, value) in properties.read::<Map>().unwrap().iter() {
        *object.write_field::<Reference>(key).unwrap() = value.clone();
    }
    Reference::null()
}

#[intuicio_function(module_name = "reflect", use_registry)]
pub fn unpack(registry: &Registry, object: Reference) -> Reference {
    let object = object.read_object().unwrap();
    let result = object
        .type_handle()
        .struct_fields()
        .unwrap()
        .iter()
        .map(|field| {
            (
                field.name.to_owned(),
                object.read_field::<Reference>(&field.name).unwrap().clone(),
            )
        })
        .collect();
    Reference::new_map(result, registry)
}

#[intuicio_function(module_name = "reflect", use_registry)]
pub fn find_function_by_name(
    registry: &Registry,
    name: Reference,
    module_name: Reference,
) -> Reference {
    Reference::new_function(
        Function::by_name(
            name.read::<Text>().unwrap().as_str(),
            module_name.read::<Text>().unwrap().as_str(),
            registry,
        )
        .unwrap(),
        registry,
    )
}

#[intuicio_function(module_name = "reflect", use_context, use_registry)]
pub fn call(
    context: &mut Context,
    registry: &Registry,
    function: Reference,
    arguments: Reference,
) -> Reference {
    for argument in arguments.read::<Array>().unwrap().iter().rev() {
        context.stack().push(argument.clone());
    }
    function
        .read::<Function>()
        .unwrap()
        .handle()
        .unwrap()
        .invoke(context, registry);
    context.stack().pop::<Reference>().unwrap_or_default()
}

#[intuicio_function(module_name = "reflect", use_registry)]
pub fn function_name(registry: &Registry, value: Reference) -> Reference {
    Reference::new_text(
        value
            .read::<Function>()
            .unwrap()
            .handle()
            .unwrap()
            .signature()
            .name
            .to_owned(),
        registry,
    )
}

#[intuicio_function(module_name = "reflect", use_registry)]
pub fn function_arguments(registry: &Registry, value: Reference) -> Reference {
    Reference::new_array(
        value
            .read::<Function>()
            .unwrap()
            .handle()
            .unwrap()
            .signature()
            .inputs
            .iter()
            .map(|argument| Reference::new_text(argument.name.to_owned(), registry))
            .collect::<Array>(),
        registry,
    )
}

#[intuicio_function(module_name = "reflect")]
pub fn select(mut value: Reference, path: Reference) -> Reference {
    let path = path.read::<Text>().unwrap();
    if path.is_empty() {
        return value;
    }
    for part in path.split('/') {
        if value.is_null() {
            return value;
        }
        let found = if let Some(array) = value.read::<Array>() {
            if let Ok(index) = part.parse::<usize>() {
                array[index].clone()
            } else {
                return Reference::null();
            }
        } else if let Some(map) = value.read::<Map>() {
            if let Some(item) = map.get(part) {
                item.clone()
            } else {
                return Reference::null();
            }
        } else if let Some(item) = value.read_object().unwrap().read_field::<Reference>(part) {
            item.clone()
        } else {
            return Reference::null();
        };
        value = found;
    }
    value
}

#[intuicio_function(module_name = "reflect")]
pub fn pass_or(value: Reference, default: Reference) -> Reference {
    if value.is_null() {
        default
    } else {
        value
    }
}

#[intuicio_function(module_name = "reflect", use_registry)]
pub fn is_null(registry: &Registry, value: Reference) -> Reference {
    Reference::new_boolean(value.is_null(), registry)
}

#[intuicio_function(module_name = "reflect", use_registry)]
pub fn is_valid(registry: &Registry, value: Reference) -> Reference {
    Reference::new_boolean(!value.is_null(), registry)
}

#[intuicio_function(module_name = "reflect", use_registry)]
pub fn is_being_written(registry: &Registry, mut value: Reference) -> Reference {
    Reference::new_boolean(value.is_being_written(), registry)
}

#[intuicio_function(module_name = "reflect", use_registry)]
pub fn references_count(registry: &Registry, value: Reference) -> Reference {
    Reference::new_integer(value.references_count() as Integer - 1, registry)
}

#[intuicio_function(module_name = "reflect", use_registry)]
pub fn does_share_reference(registry: &Registry, a: Reference, b: Reference) -> Reference {
    Reference::new_boolean(a.does_share_reference(&b, false), registry)
}

pub fn are_same_impl(a: &Reference, b: &Reference) -> bool {
    if a.is_null() && b.is_null() {
        true
    } else if let (Some(a), Some(b)) = (a.read::<Boolean>(), b.read::<Boolean>()) {
        *a == *b
    } else if let (Some(a), Some(b)) = (a.read::<Integer>(), b.read::<Integer>()) {
        *a == *b
    } else if let (Some(a), Some(b)) = (a.read::<Real>(), b.read::<Real>()) {
        *a == *b
    } else if let (Some(a), Some(b)) = (a.read::<Text>(), b.read::<Text>()) {
        *a == *b
    } else if let (Some(a), Some(b)) = (a.read::<Array>(), b.read::<Array>()) {
        a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| are_same_impl(a, b))
    } else if let (Some(a), Some(b)) = (a.read::<Map>(), b.read::<Map>()) {
        a.len() == b.len()
            && a.keys().collect::<HashSet<_>>() == b.keys().collect::<HashSet<_>>()
            && a.iter().all(|(k, v)| are_same_impl(v, b.get(k).unwrap()))
    } else if let (Some(a), Some(b)) = (a.read::<Type>(), b.read::<Type>()) {
        a.is_same_as(&b)
    } else if let (Some(a), Some(b)) = (a.read::<Function>(), b.read::<Function>()) {
        a.is_same_as(&b)
    } else {
        let a = a.read_object().unwrap();
        let b = b.read_object().unwrap();
        let keys = a
            .type_handle()
            .struct_fields()
            .unwrap()
            .iter()
            .map(|field| &field.name)
            .chain(
                b.type_handle()
                    .struct_fields()
                    .unwrap()
                    .iter()
                    .map(|field| &field.name),
            )
            .collect::<HashSet<_>>();
        if keys.is_empty() {
            false
        } else {
            keys.into_iter()
                .map(|key| {
                    (
                        a.read_field::<Reference>(key),
                        b.read_field::<Reference>(key),
                    )
                })
                .all(|(a, b)| a.is_some() && b.is_some() && are_same_impl(a.unwrap(), b.unwrap()))
        }
    }
}

#[intuicio_function(module_name = "reflect", use_registry)]
pub fn are_same(registry: &Registry, a: Reference, b: Reference) -> Reference {
    Reference::new_boolean(are_same_impl(&a, &b), registry)
}

#[intuicio_function(module_name = "reflect", use_registry)]
pub fn to_boolean(registry: &Registry, value: Reference) -> Reference {
    if value.type_of().unwrap().is::<Boolean>() {
        return value;
    }
    if let Some(value) = value.read::<Integer>() {
        return Reference::new_boolean(*value != 0, registry);
    }
    if let Some(value) = value.read::<Real>() {
        return Reference::new_boolean(*value != 0.0, registry);
    }
    if let Some(value) = value.read::<Text>() {
        if let Ok(value) = value.parse::<Boolean>() {
            return Reference::new_boolean(value, registry);
        }
    }
    if let Some(value) = value.read::<Array>() {
        return Reference::new_boolean(!value.is_empty(), registry);
    }
    if let Some(value) = value.read::<Map>() {
        return Reference::new_boolean(!value.is_empty(), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "reflect", use_registry)]
pub fn to_integer(registry: &Registry, value: Reference) -> Reference {
    if value.type_of().unwrap().is::<Integer>() {
        return value;
    }
    if let Some(value) = value.read::<Boolean>() {
        return Reference::new_integer(if *value { 1 } else { 0 }, registry);
    }
    if let Some(value) = value.read::<Real>() {
        return Reference::new_integer(*value as Integer, registry);
    }
    if let Some(value) = value.read::<Text>() {
        if let Ok(value) = value.parse::<Integer>() {
            return Reference::new_integer(value, registry);
        }
    }
    Reference::null()
}

#[intuicio_function(module_name = "reflect", use_registry)]
pub fn to_real(registry: &Registry, value: Reference) -> Reference {
    if value.type_of().unwrap().is::<Real>() {
        return value;
    }
    if let Some(value) = value.read::<Boolean>() {
        return Reference::new_real(if *value { 1.0 } else { 0.0 }, registry);
    }
    if let Some(value) = value.read::<Integer>() {
        return Reference::new_real(*value as Real, registry);
    }
    if let Some(value) = value.read::<Text>() {
        if let Ok(value) = value.parse::<Real>() {
            return Reference::new_real(value, registry);
        }
    }
    Reference::null()
}

#[intuicio_function(module_name = "reflect", use_registry)]
pub fn to_text(registry: &Registry, value: Reference) -> Reference {
    if value.type_of().unwrap().is::<Text>() {
        return value;
    }
    if let Some(value) = value.read::<Boolean>() {
        return Reference::new_text(value.to_string(), registry);
    }
    if let Some(value) = value.read::<Integer>() {
        return Reference::new_text(value.to_string(), registry);
    }
    if let Some(value) = value.read::<Real>() {
        return Reference::new_text(value.to_string(), registry);
    }
    if let Some(value) = value.read::<Array>() {
        let mut result = "[".to_owned();
        for (index, value) in value.iter().enumerate() {
            if index > 0 {
                result.push_str(", ");
            }
            result.push_str(value.read::<Text>().unwrap().as_str());
        }
        result.push(']');
        return Reference::new_text(result, registry);
    }
    if let Some(value) = value.read::<Map>() {
        let mut result = "{".to_owned();
        for (index, (key, value)) in value.iter().enumerate() {
            if index > 0 {
                result.push_str(", ");
            }
            result.push_str(key.as_str());
            result.push_str(": ");
            result.push_str(value.read::<Text>().unwrap().as_str());
        }
        result.push('}');
        return Reference::new_text(result, registry);
    }
    if let Some(value) = value.read::<Type>() {
        let handle = value.handle().unwrap();
        let mut result = "struct ".to_owned();
        if let Some(name) = handle.module_name() {
            result.push_str(name);
            result.push_str("::");
        }
        result.push_str(handle.name());
        result.push_str(" {");
        for (index, field) in handle.struct_fields().unwrap().iter().enumerate() {
            if index > 0 {
                result.push_str(", ");
            }
            result.push_str(&field.name);
        }
        result.push('}');
        return Reference::new_text(result, registry);
    }
    if let Some(value) = value.read::<Function>() {
        let handle = value.handle().unwrap();
        let mut result = "func ".to_owned();
        if let Some(name) = handle.signature().module_name.as_ref() {
            result.push_str(name.as_str());
            result.push_str("::");
        }
        result.push_str(&handle.signature().name);
        result.push_str(" {");
        for (index, argument) in handle.signature().inputs.iter().enumerate() {
            if index > 0 {
                result.push_str(", ");
            }
            result.push_str(&argument.name);
        }
        result.push('}');
        return Reference::new_text(result, registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "reflect", use_context, use_registry)]
pub fn stack_size(context: &mut Context, registry: &Registry) -> Reference {
    Reference::new_integer(context.stack().position() as Integer, registry)
}

#[intuicio_function(module_name = "reflect", use_context, use_registry)]
pub fn registers_size(context: &mut Context, registry: &Registry) -> Reference {
    Reference::new_integer(context.registers().position() as Integer, registry)
}

pub fn install(registry: &mut Registry) {
    registry.add_type(define_native_struct! {
        registry => mod reflect struct Reference (Reference) {}
        [override_send = true]
    });
    registry.add_type(define_native_struct! {
        registry => mod reflect struct Type (Type) {}
    });
    registry.add_type(define_native_struct! {
        registry => mod reflect struct Function (Function) {}
    });
    registry.add_function(find_type_by_name::define_function(registry));
    registry.add_function(type_of::define_function(registry));
    registry.add_function(type_name::define_function(registry));
    registry.add_function(type_fields::define_function(registry));
    registry.add_function(type_byte_size::define_function(registry));
    registry.add_function(get_field::define_function(registry));
    registry.add_function(set_field::define_function(registry));
    registry.add_function(new::define_function(registry));
    registry.add_function(pack::define_function(registry));
    registry.add_function(unpack::define_function(registry));
    registry.add_function(find_function_by_name::define_function(registry));
    registry.add_function(call::define_function(registry));
    registry.add_function(function_name::define_function(registry));
    registry.add_function(function_arguments::define_function(registry));
    registry.add_function(select::define_function(registry));
    registry.add_function(pass_or::define_function(registry));
    registry.add_function(is_null::define_function(registry));
    registry.add_function(is_valid::define_function(registry));
    registry.add_function(is_being_written::define_function(registry));
    registry.add_function(references_count::define_function(registry));
    registry.add_function(does_share_reference::define_function(registry));
    registry.add_function(are_same::define_function(registry));
    registry.add_function(to_boolean::define_function(registry));
    registry.add_function(to_integer::define_function(registry));
    registry.add_function(to_real::define_function(registry));
    registry.add_function(to_text::define_function(registry));
    registry.add_function(stack_size::define_function(registry));
    registry.add_function(registers_size::define_function(registry));
}
