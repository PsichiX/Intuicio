use crate::{Array, Boolean, Function, Integer, Map, Real, Reference, Text, Type};
use intuicio_core::registry::Registry;
use intuicio_derive::intuicio_function;
use std::fmt::Write;

#[intuicio_function(module_name = "debug")]
pub fn assert(status: Reference, message: Reference) -> Reference {
    if !*status.read::<Boolean>().unwrap() {
        panic!("{}", message.read::<Text>().unwrap().as_str());
    }
    Reference::null()
}

fn debug_indent(result: &mut dyn Write, indent: usize) {
    for _ in 0..(indent * 2) {
        result.write_char(' ').unwrap()
    }
}

fn debug_impl(value: &Reference, result: &mut dyn Write, indent: &mut Option<usize>) {
    if value.is_null() {
        write!(result, "null").unwrap();
    } else if let Some(value) = value.read::<Boolean>() {
        write!(result, "{}", *value).unwrap();
    } else if let Some(value) = value.read::<Integer>() {
        write!(result, "{}", *value).unwrap();
    } else if let Some(value) = value.read::<Real>() {
        write!(result, "{}", *value).unwrap();
    } else if let Some(value) = value.read::<Text>() {
        write!(result, r#""{}""#, value.as_str()).unwrap();
    } else if let Some(value) = value.read::<Array>() {
        write!(result, "[").unwrap();
        if !value.is_empty() {
            if let Some(indent) = indent {
                *indent += 1;
                writeln!(result).unwrap();
                debug_indent(result, *indent);
            }
            for (index, value) in value.iter().enumerate() {
                if index > 0 {
                    write!(result, ", ").unwrap();
                    if let Some(indent) = indent {
                        writeln!(result).unwrap();
                        debug_indent(result, *indent);
                    }
                }
                debug_impl(value, result, indent);
            }
            if let Some(indent) = indent {
                *indent -= 1;
                writeln!(result).unwrap();
                debug_indent(result, *indent);
            }
        }
        write!(result, "]").unwrap();
    } else if let Some(value) = value.read::<Map>() {
        write!(result, "{{").unwrap();
        if !value.is_empty() {
            if let Some(indent) = indent {
                *indent += 1;
                writeln!(result).unwrap();
                debug_indent(result, *indent);
            }
            for (index, (key, value)) in value.iter().enumerate() {
                if index > 0 {
                    write!(result, ", ").unwrap();
                    if let Some(indent) = indent {
                        writeln!(result).unwrap();
                        debug_indent(result, *indent);
                    }
                }
                write!(result, "{}: ", key).unwrap();
                debug_impl(value, result, indent);
            }
            if let Some(indent) = indent {
                *indent -= 1;
                writeln!(result).unwrap();
                debug_indent(result, *indent);
            }
        }
        write!(result, "}}").unwrap();
    } else if let Some(value) = value.read::<Type>() {
        write!(
            result,
            "<{}::{}>",
            value.handle().unwrap().module_name.as_deref().unwrap_or(""),
            value.handle().unwrap().name
        )
        .unwrap();
    } else if let Some(value) = value.read::<Function>() {
        let signature = value.handle().unwrap().signature();
        write!(
            result,
            "<{}::{}(",
            signature.module_name.as_deref().unwrap_or(""),
            signature.name,
        )
        .unwrap();
        for (index, parameter) in signature.inputs.iter().enumerate() {
            if index > 0 {
                write!(result, ", ").unwrap();
            }
            write!(result, "{}", parameter.name).unwrap();
        }
        write!(result, ")>").unwrap();
    } else if let Some(value) = value.read_object() {
        write!(
            result,
            "<{}::{}> {{",
            value.struct_handle().module_name.as_deref().unwrap_or(""),
            value.struct_handle().name
        )
        .unwrap();
        if !value.struct_handle().fields().is_empty() {
            if let Some(indent) = indent {
                *indent += 1;
                writeln!(result).unwrap();
                debug_indent(result, *indent);
            }
            for (index, field) in value.struct_handle().fields().iter().enumerate() {
                if let Some(value) = value.read_field::<Reference>(&field.name) {
                    if index > 0 {
                        write!(result, ", ").unwrap();
                        if let Some(indent) = indent {
                            writeln!(result).unwrap();
                            debug_indent(result, *indent);
                        }
                    }
                    write!(result, "{}: ", field.name).unwrap();
                    debug_impl(value, result, indent);
                } else {
                    if index > 0 {
                        write!(result, ", ").unwrap();
                    }
                    write!(result, "<?>").unwrap();
                }
            }
            if let Some(indent) = indent {
                *indent -= 1;
                writeln!(result).unwrap();
                debug_indent(result, *indent);
            }
        }
        write!(result, "}}").unwrap();
    } else {
        write!(result, "<?>").unwrap();
    }
}

#[intuicio_function(module_name = "debug", use_registry)]
pub fn debug(registry: &Registry, value: Reference, pretty: Reference) -> Reference {
    let mut result = String::new();
    let indent = Some(0);
    debug_impl(
        &value,
        &mut result,
        &mut if *pretty.read::<Boolean>().unwrap() {
            indent
        } else {
            None
        },
    );
    Reference::new_text(result, registry)
}

pub fn install(registry: &mut Registry) {
    registry.add_function(assert::define_function(registry));
    registry.add_function(debug::define_function(registry));
}
