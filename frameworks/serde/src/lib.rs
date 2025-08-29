use intuicio_core::{
    object::Object as CoreObject,
    registry::Registry,
    types::{
        EnumVariantQuery, Type, TypeHandle, TypeQuery, enum_type::Enum, struct_type::StructField,
    },
};
use intuicio_data::{
    Finalize, managed::DynamicManaged, managed_box::DynamicManagedBox, type_hash::TypeHash,
};
use serde::{Serialize, de::DeserializeOwned};
use std::{collections::HashMap, error::Error};

pub use serde_intermediate::{
    Intermediate, Object,
    de::intermediate::DeserializeMode,
    error::{Error as IntermediateError, Result as IntermediateResult},
    from_intermediate, from_intermediate_as, from_object, from_str, from_str_as, to_intermediate,
    to_object, to_string, to_string_compact, to_string_pretty,
};

struct Serializer {
    #[allow(clippy::type_complexity)]
    serialize_from: Box<
        dyn Fn(*const u8, &SerializationRegistry, &Registry) -> Result<Intermediate, Box<dyn Error>>
            + Send
            + Sync,
    >,
    #[allow(clippy::type_complexity)]
    deserialize_to: Box<
        dyn Fn(
                *mut u8,
                &Intermediate,
                &SerializationRegistry,
                bool,
                &Registry,
            ) -> Result<(), Box<dyn Error>>
            + Send
            + Sync,
    >,
}

#[derive(Default)]
pub struct SerializationRegistry {
    mapping: HashMap<TypeHash, Serializer>,
}

impl SerializationRegistry {
    pub fn with_basic_types(mut self) -> Self {
        self.register::<()>(
            |_, _, _| Ok(Intermediate::Unit),
            |_, value, _, _, _| {
                if matches!(value, Intermediate::Unit) {
                    Ok(())
                } else {
                    Err("Expected unit value".into())
                }
            },
        );
        self.register::<bool>(
            |data, _, _| Ok((*data).into()),
            |data, value, _, _, _| {
                if let Intermediate::Bool(value) = value {
                    *data = *value;
                    Ok(())
                } else {
                    Err("Expected bool value".into())
                }
            },
        );
        self.register::<i8>(
            |data, _, _| Ok((*data).into()),
            |data, value, _, _, _| {
                if let Intermediate::I8(value) = value {
                    *data = *value;
                    Ok(())
                } else {
                    Err("Expected i8 value".into())
                }
            },
        );
        self.register::<i16>(
            |data, _, _| Ok((*data).into()),
            |data, value, _, _, _| match value {
                Intermediate::I8(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::I16(value) => {
                    *data = *value;
                    Ok(())
                }
                _ => Err("Expected i16 value".into()),
            },
        );
        self.register::<i32>(
            |data, _, _| Ok((*data).into()),
            |data, value, _, _, _| match value {
                Intermediate::I8(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::I16(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::I32(value) => {
                    *data = *value;
                    Ok(())
                }
                _ => Err("Expected i32 value".into()),
            },
        );
        self.register::<i64>(
            |data, _, _| Ok((*data).into()),
            |data, value, _, _, _| match value {
                Intermediate::I8(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::I16(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::I32(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::I64(value) => {
                    *data = *value;
                    Ok(())
                }
                _ => Err("Expected i64 value".into()),
            },
        );
        self.register::<i128>(
            |data, _, _| Ok((*data).into()),
            |data, value, _, _, _| match value {
                Intermediate::I8(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::I16(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::I32(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::I64(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::I128(value) => {
                    *data = *value;
                    Ok(())
                }
                _ => Err("Expected i128 value".into()),
            },
        );
        self.register::<isize>(
            |data, _, _| Ok((*data).into()),
            |data, value, _, _, _| match value {
                Intermediate::I8(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::I16(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::I32(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::I64(value) => {
                    *data = *value as _;
                    Ok(())
                }
                _ => Err("Expected isize value".into()),
            },
        );
        self.register::<u8>(
            |data, _, _| Ok((*data).into()),
            |data, value, _, _, _| {
                if let Intermediate::U8(value) = value {
                    *data = *value;
                    Ok(())
                } else {
                    Err("Expected u8 value".into())
                }
            },
        );
        self.register::<u16>(
            |data, _, _| Ok((*data).into()),
            |data, value, _, _, _| match value {
                Intermediate::U8(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::U16(value) => {
                    *data = *value;
                    Ok(())
                }
                _ => Err("Expected u16 value".into()),
            },
        );
        self.register::<u32>(
            |data, _, _| Ok((*data).into()),
            |data, value, _, _, _| match value {
                Intermediate::U8(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::U16(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::U32(value) => {
                    *data = *value;
                    Ok(())
                }
                _ => Err("Expected u32 value".into()),
            },
        );
        self.register::<u64>(
            |data, _, _| Ok((*data).into()),
            |data, value, _, _, _| match value {
                Intermediate::U8(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::U16(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::U32(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::U64(value) => {
                    *data = *value;
                    Ok(())
                }
                _ => Err("Expected u64 value".into()),
            },
        );
        self.register::<u128>(
            |data, _, _| Ok((*data).into()),
            |data, value, _, _, _| match value {
                Intermediate::U8(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::U16(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::U32(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::U64(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::U128(value) => {
                    *data = *value;
                    Ok(())
                }
                _ => Err("Expected u128 value".into()),
            },
        );
        self.register::<usize>(
            |data, _, _| Ok((*data).into()),
            |data, value, _, _, _| match value {
                Intermediate::U8(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::U16(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::U32(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::U64(value) => {
                    *data = *value as _;
                    Ok(())
                }
                _ => Err("Expected usize value".into()),
            },
        );
        self.register::<f32>(
            |data, _, _| Ok((*data).into()),
            |data, value, _, _, _| match value {
                Intermediate::I8(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::I16(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::I32(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::U8(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::U16(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::U32(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::F32(value) => {
                    *data = *value;
                    Ok(())
                }
                _ => Err("Expected f32 value".into()),
            },
        );
        self.register::<f64>(
            |data, _, _| Ok((*data).into()),
            |data, value, _, _, _| match value {
                Intermediate::I8(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::I16(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::I32(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::I64(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::U8(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::U16(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::U32(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::U64(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::F32(value) => {
                    *data = *value as _;
                    Ok(())
                }
                Intermediate::F64(value) => {
                    *data = *value;
                    Ok(())
                }
                _ => Err("Expected f64 value".into()),
            },
        );
        self.register::<char>(
            |data, _, _| Ok((*data).into()),
            |data, value, _, _, _| match value {
                Intermediate::Char(value) => {
                    *data = *value;
                    Ok(())
                }
                Intermediate::String(value) => {
                    if let Some(value) = value.chars().next() {
                        *data = value;
                        Ok(())
                    } else {
                        Err("Expected char value (intermediate string is empty)".into())
                    }
                }
                _ => Err("Expected char value".into()),
            },
        );
        self.register::<String>(
            |data, _, _| Ok(data.as_str().into()),
            |data, value, _, initialized, _| {
                if let Intermediate::String(value) = value {
                    if initialized {
                        *data = value.to_owned();
                    } else {
                        unsafe { (data as *mut String).write_unaligned(value.to_owned()) };
                    }
                    Ok(())
                } else {
                    Err("Expected string value".into())
                }
            },
        );
        self
    }

    pub fn with_erased_types(mut self) -> Self {
        self.register::<DynamicManaged>(
            |data, serializer, registry| unsafe {
                let Some(type_handle) = registry.find_type(TypeQuery {
                    type_hash: Some(*data.type_hash()),
                    ..Default::default()
                }) else {
                    return Err(format!(
                        "Type of DynamicManaged object not found. Hash: {}",
                        data.type_hash()
                    )
                    .into());
                };
                let value = serializer
                    .dynamic_serialize_from(*data.type_hash(), data.as_ptr_raw(), registry)
                    .map_err(|error| {
                        format!(
                            "{}. Type: {}::{}",
                            error,
                            type_handle.module_name().unwrap_or(""),
                            type_handle.name()
                        )
                    })?;
                Ok(Intermediate::struct_type()
                    .field("type", type_handle.name())
                    .field(
                        "module",
                        Intermediate::Option(
                            type_handle
                                .module_name()
                                .map(|name| Box::new(Intermediate::String(name.to_owned()))),
                        ),
                    )
                    .field("value", value))
            },
            |data, value, serializer, initialized, registry| unsafe {
                let Intermediate::Struct(fields) = value else {
                    return Err("Expected struct value".into());
                };
                let Some(type_name) = fields
                    .iter()
                    .find(|(name, _)| name == "type")
                    .and_then(|(_, value)| value.as_str())
                else {
                    return Err("Type field not found".into());
                };
                let Some(module_name) = fields
                    .iter()
                    .find(|(name, _)| name == "module")
                    .map(|(_, value)| value.as_option().and_then(|value| value.as_str()))
                else {
                    return Err("Module field not found".into());
                };
                let Some(value) = fields
                    .iter()
                    .find(|(name, _)| name == "value")
                    .map(|(_, value)| value)
                else {
                    return Err("Value field not found".into());
                };
                let Some(type_handle) = registry.find_type(TypeQuery {
                    name: Some(type_name.into()),
                    module_name: module_name.map(|name| name.into()),
                    ..Default::default()
                }) else {
                    return Err(format!(
                        "Type not found: {}::{}",
                        module_name.unwrap_or(""),
                        type_name
                    )
                    .into());
                };
                if initialized {
                    DynamicManaged::finalize_raw(data as *mut DynamicManaged as *mut ());
                }
                (data as *mut DynamicManaged).write_unaligned(DynamicManaged::new_uninitialized(
                    type_handle.type_hash(),
                    *type_handle.layout(),
                    type_handle.finalizer(),
                ));
                serializer.dynamic_deserialize_to(
                    type_handle.type_hash(),
                    data.as_mut_ptr_raw(),
                    value,
                    false,
                    registry,
                )
            },
        );
        self.register::<DynamicManagedBox>(
            |data, serializer, registry| unsafe {
                let type_hash = data.type_hash();
                let ptr = data.as_ptr_raw();
                let Some(type_handle) = registry.find_type(TypeQuery {
                    type_hash: Some(type_hash),
                    ..Default::default()
                }) else {
                    return Err(format!(
                        "Type of DynamicManagedBox object not found. Hash: {type_hash}"
                    )
                    .into());
                };
                let value = serializer
                    .dynamic_serialize_from(type_hash, ptr, registry)
                    .map_err(|error| {
                        format!(
                            "{}. Type: {}::{}",
                            error,
                            type_handle.module_name().unwrap_or(""),
                            type_handle.name()
                        )
                    })?;
                Ok(Intermediate::struct_type()
                    .field("type", type_handle.name())
                    .field(
                        "module",
                        Intermediate::Option(
                            type_handle
                                .module_name()
                                .map(|name| Box::new(Intermediate::String(name.to_owned()))),
                        ),
                    )
                    .field("value", value))
            },
            |data, value, serializer, initialized, registry| unsafe {
                let Intermediate::Struct(fields) = value else {
                    return Err("Expected struct value".into());
                };
                let Some(type_name) = fields
                    .iter()
                    .find(|(name, _)| name == "type")
                    .and_then(|(_, value)| value.as_str())
                else {
                    return Err("Type field not found".into());
                };
                let Some(module_name) = fields
                    .iter()
                    .find(|(name, _)| name == "module")
                    .map(|(_, value)| value.as_option().and_then(|value| value.as_str()))
                else {
                    return Err("Module field not found".into());
                };
                let Some(value) = fields
                    .iter()
                    .find(|(name, _)| name == "value")
                    .map(|(_, value)| value)
                else {
                    return Err("Value field not found".into());
                };
                let Some(type_handle) = registry.find_type(TypeQuery {
                    name: Some(type_name.into()),
                    module_name: module_name.map(|name| name.into()),
                    ..Default::default()
                }) else {
                    return Err(format!(
                        "Type not found: {}::{}",
                        module_name.unwrap_or(""),
                        type_name
                    )
                    .into());
                };
                if initialized {
                    DynamicManagedBox::finalize_raw(data as *mut DynamicManagedBox as *mut ());
                }
                (data as *mut DynamicManagedBox).write_unaligned(
                    DynamicManagedBox::new_uninitialized(
                        type_handle.type_hash(),
                        *type_handle.layout(),
                        type_handle.finalizer(),
                    ),
                );
                serializer.dynamic_deserialize_to(
                    type_handle.type_hash(),
                    data.as_mut_ptr_raw(),
                    value,
                    false,
                    registry,
                )
            },
        );
        self.register::<CoreObject>(
            |data, serializer, registry| unsafe {
                let Some(type_handle) = registry.find_type(TypeQuery {
                    type_hash: Some(data.type_handle().type_hash()),
                    ..Default::default()
                }) else {
                    return Err(format!(
                        "Type of Object not found. Hash: {}",
                        data.type_handle().type_hash()
                    )
                    .into());
                };
                let value = serializer
                    .dynamic_serialize_from(data.type_handle().type_hash(), data.as_ptr(), registry)
                    .map_err(|error| {
                        format!(
                            "{}. Type: {}::{}",
                            error,
                            type_handle.module_name().unwrap_or(""),
                            type_handle.name()
                        )
                    })?;
                Ok(Intermediate::struct_type()
                    .field("type", type_handle.name())
                    .field(
                        "module",
                        Intermediate::Option(
                            type_handle
                                .module_name()
                                .map(|name| Box::new(Intermediate::String(name.to_owned()))),
                        ),
                    )
                    .field("value", value))
            },
            |data, value, serializer, initialized, registry| unsafe {
                let Intermediate::Struct(fields) = value else {
                    return Err("Expected struct value".into());
                };
                let Some(type_name) = fields
                    .iter()
                    .find(|(name, _)| name == "type")
                    .and_then(|(_, value)| value.as_str())
                else {
                    return Err("Type field not found".into());
                };
                let Some(module_name) = fields
                    .iter()
                    .find(|(name, _)| name == "module")
                    .map(|(_, value)| value.as_option().and_then(|value| value.as_str()))
                else {
                    return Err("Module field not found".into());
                };
                let Some(value) = fields
                    .iter()
                    .find(|(name, _)| name == "value")
                    .map(|(_, value)| value)
                else {
                    return Err("Value field not found".into());
                };
                let Some(type_handle) = registry.find_type(TypeQuery {
                    name: Some(type_name.into()),
                    module_name: module_name.map(|name| name.into()),
                    ..Default::default()
                }) else {
                    return Err(format!(
                        "Type not found: {}::{}",
                        module_name.unwrap_or(""),
                        type_name
                    )
                    .into());
                };
                if initialized {
                    CoreObject::finalize_raw(data as *mut CoreObject as *mut ());
                }
                (data as *mut CoreObject)
                    .write_unaligned(CoreObject::new_uninitialized(type_handle.clone()).unwrap());
                serializer.dynamic_deserialize_to(
                    type_handle.type_hash(),
                    data.as_mut_ptr(),
                    value,
                    false,
                    registry,
                )
            },
        );
        self
    }

    pub fn with_serde<T: Serialize + DeserializeOwned>(mut self) -> Self {
        self.register_serde::<T>();
        self
    }

    pub fn with_reflection(mut self, handle: TypeHandle) -> Self {
        self.register_reflection(handle);
        self
    }

    pub fn with<T>(
        mut self,
        serialize_from: impl Fn(&T, &Self, &Registry) -> Result<Intermediate, Box<dyn Error>>
        + Send
        + Sync
        + 'static,
        deserialize_to: impl Fn(
            &mut T,
            &Intermediate,
            &Self,
            bool,
            &Registry,
        ) -> Result<(), Box<dyn Error>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.register(serialize_from, deserialize_to);
        self
    }

    pub fn with_raw(
        mut self,
        type_hash: TypeHash,
        serialize_from: impl Fn(*const u8, &Self, &Registry) -> Result<Intermediate, Box<dyn Error>>
        + Send
        + Sync
        + 'static,
        deserialize_to: impl Fn(
            *mut u8,
            &Intermediate,
            &Self,
            bool,
            &Registry,
        ) -> Result<(), Box<dyn Error>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        unsafe { self.register_raw(type_hash, serialize_from, deserialize_to) }
        self
    }

    pub fn register_serde<T: Serialize + DeserializeOwned>(&mut self) {
        self.register::<T>(
            |data, _, _| Ok(serde_intermediate::to_intermediate(data)?),
            |data, value, _, initialized, _| {
                if initialized {
                    *data = serde_intermediate::from_intermediate(value)?;
                } else {
                    unsafe {
                        (data as *mut T)
                            .write_unaligned(serde_intermediate::from_intermediate(value)?)
                    };
                }
                Ok(())
            },
        );
    }

    pub fn register_reflection(&mut self, handle: TypeHandle) {
        let handle_ser = handle.clone();
        let handle_de = handle.clone();
        unsafe {
            self.register_raw(
                handle.type_hash(),
                move |data, serializer, registry| match &*handle_ser {
                    Type::Struct(type_) => {
                        let mut result = Intermediate::struct_type();
                        for field in type_.fields() {
                            let value = serializer.dynamic_serialize_from(
                                field.type_handle().type_hash(),
                                data.add(field.address_offset()),
                                registry,
                            )?;
                            result = result.field(field.name.as_str(), value);
                        }
                        Ok(result)
                    }
                    Type::Enum(type_) => {
                        let discriminant = data.read();
                        if let Some(variant) = type_.find_variant_by_discriminant(discriminant) {
                            let mut result = Intermediate::struct_variant(variant.name.as_str());
                            for field in &variant.fields {
                                let value = serializer.dynamic_serialize_from(
                                    field.type_handle().type_hash(),
                                    data.add(field.address_offset()),
                                    registry,
                                )?;
                                result = result.field(field.name.as_str(), value);
                            }
                            Ok(result)
                        } else {
                            Err(
                                format!("Enum variant with discriminant: {discriminant} not found")
                                    .into(),
                            )
                        }
                    }
                },
                move |data, value, serializer, initialized, registry| match &*handle_de {
                    Type::Struct(type_) => {
                        fn item<'a>(
                            value: &'a Intermediate,
                            name: &'a str,
                        ) -> Option<&'a Intermediate> {
                            match value {
                                Intermediate::Struct(value) => value
                                    .iter()
                                    .find_map(|(n, v)| if n == name { Some(v) } else { None }),
                                Intermediate::Map(value) => value.iter().find_map(|(key, v)| {
                                    if key.as_str().map(|key| key == name).unwrap_or_default() {
                                        Some(v)
                                    } else {
                                        None
                                    }
                                }),
                                _ => None,
                            }
                        }
                        for field in type_.fields() {
                            let data = data.add(field.address_offset());
                            if initialized {
                                field.type_handle().finalize(data.cast());
                            }
                            if let Some(value) = item(value, &field.name) {
                                serializer.dynamic_deserialize_to(
                                    field.type_handle().type_hash(),
                                    data,
                                    value,
                                    false,
                                    registry,
                                )?;
                            } else if !initialized {
                                field.type_handle().initialize(data.cast());
                            }
                        }
                        Ok(())
                    }
                    Type::Enum(type_) => {
                        fn discriminant_fields<'a>(
                            type_: &'a Enum,
                            name: &'a str,
                        ) -> Option<(u8, &'a [StructField])> {
                            type_
                                .find_variant(EnumVariantQuery {
                                    name: Some(name.into()),
                                    ..Default::default()
                                })
                                .map(|variant| (variant.discriminant(), variant.fields.as_slice()))
                        }
                        if initialized {
                            type_.finalize(data.cast());
                        }
                        match value {
                            Intermediate::UnitVariant(name) => {
                                if let Some((discriminant, _)) = discriminant_fields(type_, name) {
                                    data.write_unaligned(discriminant);
                                } else {
                                    return Err(format!("Enum variant: {name} not found").into());
                                }
                            }
                            Intermediate::NewTypeVariant(name, value) => {
                                if let Some((discriminant, fields)) =
                                    discriminant_fields(type_, name)
                                {
                                    let field = &fields[0];
                                    data.write_unaligned(discriminant);
                                    serializer.dynamic_deserialize_to(
                                        field.type_handle().type_hash(),
                                        data.add(field.address_offset()),
                                        value,
                                        false,
                                        registry,
                                    )?;
                                } else {
                                    return Err(format!("Enum variant: {name} not found").into());
                                }
                            }
                            Intermediate::TupleVariant(name, values) => {
                                if let Some((discriminant, fields)) =
                                    discriminant_fields(type_, name)
                                {
                                    data.write_unaligned(discriminant);
                                    for field in fields {
                                        let index = field
                                            .name
                                            .parse::<usize>()
                                            .map_err(|_| "Expected tuple field name")?;
                                        if let Some(value) = values.get(index) {
                                            serializer.dynamic_deserialize_to(
                                                field.type_handle().type_hash(),
                                                data.add(field.address_offset()),
                                                value,
                                                false,
                                                registry,
                                            )?;
                                        } else if !initialized {
                                            field.type_handle().initialize(
                                                data.add(field.address_offset()).cast(),
                                            );
                                        }
                                    }
                                } else {
                                    return Err(format!("Enum variant: {name} not found").into());
                                }
                            }
                            Intermediate::StructVariant(name, values) => {
                                if let Some((discriminant, fields)) =
                                    discriminant_fields(type_, name)
                                {
                                    data.write_unaligned(discriminant);
                                    for field in fields {
                                        if let Some((_, value)) = values
                                            .iter()
                                            .find(|(key, _)| key == field.name.as_str())
                                        {
                                            serializer.dynamic_deserialize_to(
                                                field.type_handle().type_hash(),
                                                data.add(field.address_offset()),
                                                value,
                                                false,
                                                registry,
                                            )?;
                                        } else if !initialized {
                                            field.type_handle().initialize(
                                                data.add(field.address_offset()).cast(),
                                            );
                                        }
                                    }
                                } else {
                                    return Err(format!("Enum variant: {name} not found").into());
                                }
                            }
                            _ => return Err("Expected enum variant".into()),
                        }
                        Ok(())
                    }
                },
            );
        }
    }

    pub fn register<T>(
        &mut self,
        serialize_from: impl Fn(&T, &Self, &Registry) -> Result<Intermediate, Box<dyn Error>>
        + Send
        + Sync
        + 'static,
        deserialize_to: impl Fn(
            &mut T,
            &Intermediate,
            &Self,
            bool,
            &Registry,
        ) -> Result<(), Box<dyn Error>>
        + Send
        + Sync
        + 'static,
    ) {
        let type_hash = TypeHash::of::<T>();
        unsafe {
            self.register_raw(
                type_hash,
                move |data, serializer, registry| {
                    serialize_from(data.cast::<T>().as_ref().unwrap(), serializer, registry)
                },
                move |data, value, serialzier, initialized, registry| {
                    deserialize_to(
                        data.cast::<T>().as_mut().unwrap(),
                        value,
                        serialzier,
                        initialized,
                        registry,
                    )
                },
            );
        }
    }

    /// # Safety
    pub unsafe fn register_raw(
        &mut self,
        type_hash: TypeHash,
        serialize_from: impl Fn(*const u8, &Self, &Registry) -> Result<Intermediate, Box<dyn Error>>
        + Send
        + Sync
        + 'static,
        deserialize_to: impl Fn(
            *mut u8,
            &Intermediate,
            &Self,
            bool,
            &Registry,
        ) -> Result<(), Box<dyn Error>>
        + Send
        + Sync
        + 'static,
    ) {
        self.mapping.insert(
            type_hash,
            Serializer {
                serialize_from: Box::new(serialize_from),
                deserialize_to: Box::new(deserialize_to),
            },
        );
    }

    pub fn unregister<T>(&mut self) {
        self.unregister_raw(TypeHash::of::<T>());
    }

    pub fn unregister_raw(&mut self, type_hash: TypeHash) {
        self.mapping.remove(&type_hash);
    }

    pub fn serialize_from<T>(
        &self,
        data: &T,
        registry: &Registry,
    ) -> Result<Intermediate, Box<dyn Error>> {
        unsafe {
            let type_hash = TypeHash::of::<T>();
            self.dynamic_serialize_from(type_hash, data as *const T as *const u8, registry)
                .map_err(|error| format!("{}. Type: {}", error, std::any::type_name::<T>()).into())
        }
    }

    /// # Safety
    pub unsafe fn dynamic_serialize_from(
        &self,
        type_hash: TypeHash,
        data: *const u8,
        registry: &Registry,
    ) -> Result<Intermediate, Box<dyn Error>> {
        if let Some(serializer) = self.mapping.get(&type_hash) {
            return (serializer.serialize_from)(data, self, registry);
        }
        Err("Type does not exist in serialization registry".into())
    }

    pub fn deserialize_to<T: Default>(
        &self,
        value: &Intermediate,
        registry: &Registry,
    ) -> Result<T, Box<dyn Error>> {
        let mut result = T::default();
        unsafe {
            self.dynamic_deserialize_to(
                TypeHash::of::<T>(),
                &mut result as *mut T as *mut u8,
                value,
                true,
                registry,
            )
            .map_err(|error| format!("{}. Type: {}", error, std::any::type_name::<T>()))?;
        }
        Ok(result)
    }

    pub fn deserialize_into<T>(
        &self,
        result: &mut T,
        value: &Intermediate,
        registry: &Registry,
    ) -> Result<(), Box<dyn Error>> {
        unsafe {
            self.dynamic_deserialize_to(
                TypeHash::of::<T>(),
                result as *mut T as *mut u8,
                value,
                true,
                registry,
            )
            .map_err(|error| format!("{}. Type: {}", error, std::any::type_name::<T>()))?;
        }
        Ok(())
    }

    /// # Safety
    pub unsafe fn dynamic_deserialize_to(
        &self,
        type_hash: TypeHash,
        data: *mut u8,
        value: &Intermediate,
        data_initialized: bool,
        registry: &Registry,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(serializer) = self.mapping.get(&type_hash) {
            (serializer.deserialize_to)(data, value, self, data_initialized, registry)?;
            return Ok(());
        }
        Err("Type not existent in serialization registry".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use intuicio_core::{IntuicioEnum, IntuicioStruct, registry::Registry};
    use intuicio_derive::{IntuicioEnum, IntuicioStruct};
    use serde::Deserialize;

    #[derive(IntuicioEnum, Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[repr(u8)]
    enum Skill {
        #[default]
        Brain,
        Muscles(bool),
        Magic {
            power: i32,
        },
    }

    #[derive(IntuicioStruct, Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct Person {
        name: String,
        age: usize,
        skill: Skill,
    }

    #[derive(IntuicioStruct)]
    struct Wrapper {
        object: DynamicManaged,
    }

    impl Default for Wrapper {
        fn default() -> Self {
            Self {
                object: DynamicManaged::new(()).unwrap(),
            }
        }
    }

    #[test]
    fn test_serde_serialization() {
        let registry = Registry::default().with_basic_types();
        let serialization = SerializationRegistry::default()
            .with_basic_types()
            .with_serde::<Skill>()
            .with_serde::<Person>();

        let data = Person {
            name: "Grumpy".to_owned(),
            age: 24,
            skill: Skill::Magic { power: 42 },
        };
        let serialized = serialization.serialize_from(&data, &registry).unwrap();
        let data2 = serialization
            .deserialize_to::<Person>(&serialized, &registry)
            .unwrap();
        assert_eq!(data, data2);
    }

    #[test]
    fn test_reflection_serialization() {
        let mut registry = Registry::default().with_basic_types();
        let skill_type = registry.add_type(Skill::define_enum(&registry));
        let person_type = registry.add_type(Person::define_struct(&registry));
        let serialization = SerializationRegistry::default()
            .with_basic_types()
            .with_reflection(skill_type)
            .with_reflection(person_type);

        let data = Person {
            name: "Grumpy".to_owned(),
            age: 24,
            skill: Skill::Magic { power: 42 },
        };
        let serialized = serialization.serialize_from(&data, &registry).unwrap();
        let data2 = serialization
            .deserialize_to::<Person>(&serialized, &registry)
            .unwrap();
        assert_eq!(data, data2);
    }

    #[test]
    fn test_type_erased_serialization() {
        let mut registry = Registry::default().with_basic_types().with_erased_types();
        registry.add_type(Skill::define_enum(&registry));
        registry.add_type(Person::define_struct(&registry));
        let wrapper_type = registry.add_type(Wrapper::define_struct(&registry));
        let serialization = SerializationRegistry::default()
            .with_basic_types()
            .with_serde::<Skill>()
            .with_serde::<Person>()
            .with_reflection(wrapper_type)
            .with_erased_types();

        let data = Wrapper {
            object: DynamicManaged::new(Person {
                name: "Grumpy".to_owned(),
                age: 24,
                skill: Skill::Magic { power: 42 },
            })
            .unwrap(),
        };
        let serialized = serialization.serialize_from(&data, &registry).unwrap();
        let data2 = serialization
            .deserialize_to::<Wrapper>(&serialized, &registry)
            .unwrap();
        let data = data.object.consume::<Person>().ok().unwrap();
        let data2 = data2.object.consume::<Person>().ok().unwrap();
        assert_eq!(data, data2);
    }
}
