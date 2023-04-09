use crate::{Array, Boolean, Integer, Reference, Text};
use intuicio_core::{define_native_struct, registry::Registry};
use intuicio_derive::intuicio_function;
use regex::{Captures, Regex};

use super::bytes::Bytes;

thread_local! {
    static FORMAT_REGEX: Regex = Regex::new(r#"\{(\d+)\}"#).unwrap();
}

#[intuicio_function(module_name = "text", use_registry)]
pub fn length(registry: &Registry, text: Reference) -> Reference {
    Reference::new_integer(text.read::<Text>().unwrap().len() as Integer, registry)
}

#[intuicio_function(module_name = "text", use_registry)]
pub fn character(registry: &Registry, text: Reference, index: Reference) -> Reference {
    let index = *index.read::<Integer>().unwrap() as usize;
    text.read::<Text>()
        .unwrap()
        .chars()
        .skip(index)
        .next()
        .map(|character| Reference::new_text(Text::from(character), registry))
        .unwrap_or_default()
}

#[intuicio_function(module_name = "text", use_registry)]
pub fn find(
    registry: &Registry,
    text: Reference,
    pattern: Reference,
    reverse: Reference,
) -> Reference {
    if *reverse.read::<Boolean>().unwrap() {
        text.read::<Text>()
            .unwrap()
            .rfind(pattern.read::<Text>().unwrap().as_str())
            .map(|index| Reference::new_integer(index as Integer, registry))
            .unwrap_or(Reference::new_integer(-1, registry))
    } else {
        text.read::<Text>()
            .unwrap()
            .find(pattern.read::<Text>().unwrap().as_str())
            .map(|index| Reference::new_integer(index as Integer, registry))
            .unwrap_or(Reference::new_integer(-1, registry))
    }
}

#[intuicio_function(module_name = "text", use_registry)]
pub fn slice(
    registry: &Registry,
    text: Reference,
    index: Reference,
    count: Reference,
) -> Reference {
    let index = *index.read::<Integer>().unwrap() as usize;
    let count = *count.read::<Integer>().unwrap() as usize;
    Reference::new_text(
        text.read::<Text>()
            .unwrap()
            .chars()
            .skip(index)
            .take(count)
            .collect::<Text>(),
        registry,
    )
}

#[intuicio_function(module_name = "text", use_registry)]
pub fn join(registry: &Registry, array: Reference, separator: Reference) -> Reference {
    Reference::new_text(
        array
            .read::<Array>()
            .unwrap()
            .iter()
            .map(|item| item.read::<Text>().unwrap().to_owned())
            .collect::<Vec<_>>()
            .join(separator.read::<Text>().unwrap().as_str()),
        registry,
    )
}

#[intuicio_function(module_name = "text", use_registry)]
pub fn combine(registry: &Registry, a: Reference, b: Reference) -> Reference {
    Reference::new_text(
        a.read::<Text>().unwrap().to_owned() + b.read::<Text>().unwrap().as_str(),
        registry,
    )
}

#[intuicio_function(module_name = "text", use_registry)]
pub fn format(registry: &Registry, template: Reference, arguments: Reference) -> Reference {
    let arguments = &*arguments.read::<Array>().unwrap();
    let result = FORMAT_REGEX.with(|regex| {
        regex
            .replace_all(
                template.read::<Text>().unwrap().as_str(),
                |captures: &Captures| {
                    let capture = &captures[1];
                    capture
                        .parse::<usize>()
                        .map(|index| arguments[index].read::<Text>().unwrap().to_owned())
                        .unwrap_or_else(|_| capture.to_owned())
                },
            )
            .as_ref()
            .to_owned()
    });
    Reference::new_text(result, registry)
}

#[intuicio_function(module_name = "text", use_registry)]
pub fn split(registry: &Registry, value: Reference, separator: Reference) -> Reference {
    let result = value
        .read::<Text>()
        .unwrap()
        .split(separator.read::<Text>().unwrap().as_str())
        .filter(|part| !part.is_empty())
        .map(|part| Reference::new_text(part.to_owned(), registry))
        .collect::<Array>();
    Reference::new_array(result, registry)
}

#[intuicio_function(module_name = "text", use_registry)]
pub fn to_bytes(registry: &Registry, value: Reference) -> Reference {
    let result = value.read::<Text>().unwrap().as_bytes().to_owned();
    Reference::new(Bytes::new_raw(result), registry)
}

#[intuicio_function(module_name = "text", use_registry)]
pub fn from_bytes(registry: &Registry, bytes: Reference) -> Reference {
    let result = bytes.read::<Bytes>().unwrap();
    Reference::new_text(
        String::from_utf8_lossy(result.get_ref()).to_string(),
        registry,
    )
}

#[intuicio_function(module_name = "text", use_registry)]
pub fn equals(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Text>(), b.read::<Text>()) {
        return Reference::new_boolean(*a == *b, registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "text", use_registry)]
pub fn not_equals(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Text>(), b.read::<Text>()) {
        return Reference::new_boolean(*a != *b, registry);
    }
    Reference::null()
}

pub fn install(registry: &mut Registry) {
    registry.add_struct(define_native_struct! {
        registry => mod text struct Text (Text) {}
    });
    registry.add_function(length::define_function(registry));
    registry.add_function(character::define_function(registry));
    registry.add_function(find::define_function(registry));
    registry.add_function(slice::define_function(registry));
    registry.add_function(join::define_function(registry));
    registry.add_function(combine::define_function(registry));
    registry.add_function(format::define_function(registry));
    registry.add_function(split::define_function(registry));
    registry.add_function(to_bytes::define_function(registry));
    registry.add_function(from_bytes::define_function(registry));
    registry.add_function(equals::define_function(registry));
    registry.add_function(not_equals::define_function(registry));
}
