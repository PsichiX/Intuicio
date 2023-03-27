use crate::{Array, Boolean, Integer, Map, Real, Reference, Text};
use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};
use intuicio_core::{registry::Registry, IntuicioStruct};
use intuicio_derive::{intuicio_method, intuicio_methods, IntuicioStruct};
use std::io::{Cursor, Read, Write};

#[repr(u8)]
#[derive(Debug, Default, Copy, Clone)]
enum DataType {
    #[default]
    Null = 0,
    Boolean = 1,
    Integer8 = 2,
    Integer16 = 3,
    Integer32 = 4,
    Integer64 = 5,
    Real = 7,
    Text = 8,
    Array = 9,
    Map = 10,
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "Bytes", module_name = "bytes")]
pub struct Bytes {
    #[intuicio(ignore)]
    buffer: Cursor<Vec<u8>>,
}

#[intuicio_methods(module_name = "bytes")]
impl Bytes {
    #[intuicio_method(use_registry)]
    pub fn new(registry: &Registry) -> Reference {
        Reference::new(Bytes::default(), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn from(registry: &Registry, array: Reference) -> Reference {
        let buffer = array
            .read::<Array>()
            .unwrap()
            .iter()
            .map(|byte| *byte.read::<Integer>().unwrap() as u8)
            .collect();
        Reference::new(
            Bytes {
                buffer: Cursor::new(buffer),
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn into(registry: &Registry, bytes: Reference) -> Reference {
        let array = bytes
            .read::<Bytes>()
            .unwrap()
            .buffer
            .get_ref()
            .iter()
            .map(|byte| Reference::new_integer(*byte as Integer, registry))
            .collect();
        Reference::new_array(array, registry)
    }

    #[intuicio_method(use_registry)]
    pub fn size(registry: &Registry, bytes: Reference) -> Reference {
        let bytes = bytes.read::<Bytes>().unwrap();
        Reference::new_integer(bytes.buffer.get_ref().len() as Integer, registry)
    }

    #[intuicio_method(use_registry)]
    pub fn position(registry: &Registry, bytes: Reference) -> Reference {
        let bytes = bytes.read::<Bytes>().unwrap();
        Reference::new_integer(bytes.buffer.position() as Integer, registry)
    }

    #[intuicio_method()]
    pub fn set_position(mut bytes: Reference, position: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        bytes
            .buffer
            .set_position(*position.read::<Integer>().unwrap() as u64);
        Reference::null()
    }

    #[intuicio_method()]
    pub fn clear(mut bytes: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        bytes.buffer.set_position(0);
        bytes.buffer.get_mut().clear();
        Reference::null()
    }

    #[intuicio_method(use_registry)]
    pub fn read_boolean(registry: &Registry, bytes: Reference) -> Reference {
        Self::read_u8(registry, bytes)
    }

    #[intuicio_method(use_registry)]
    pub fn read_u8(registry: &Registry, mut bytes: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        bytes
            .buffer
            .read_u8()
            .map(|value| Reference::new_integer(value as Integer, registry))
            .unwrap_or_default()
    }

    #[intuicio_method(use_registry)]
    pub fn read_u16(registry: &Registry, mut bytes: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        bytes
            .buffer
            .read_u16::<NetworkEndian>()
            .map(|value| Reference::new_integer(value as Integer, registry))
            .unwrap_or_default()
    }

    #[intuicio_method(use_registry)]
    pub fn read_u32(registry: &Registry, mut bytes: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        bytes
            .buffer
            .read_u32::<NetworkEndian>()
            .map(|value| Reference::new_integer(value as Integer, registry))
            .unwrap_or_default()
    }

    #[intuicio_method(use_registry)]
    pub fn read_u64(registry: &Registry, mut bytes: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        bytes
            .buffer
            .read_u64::<NetworkEndian>()
            .map(|value| Reference::new_integer(value as Integer, registry))
            .unwrap_or_default()
    }

    #[intuicio_method(use_registry)]
    pub fn read_i8(registry: &Registry, mut bytes: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        bytes
            .buffer
            .read_i8()
            .map(|value| Reference::new_integer(value as Integer, registry))
            .unwrap_or_default()
    }

    #[intuicio_method(use_registry)]
    pub fn read_i16(registry: &Registry, mut bytes: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        bytes
            .buffer
            .read_i16::<NetworkEndian>()
            .map(|value| Reference::new_integer(value as Integer, registry))
            .unwrap_or_default()
    }

    #[intuicio_method(use_registry)]
    pub fn read_i32(registry: &Registry, mut bytes: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        bytes
            .buffer
            .read_i32::<NetworkEndian>()
            .map(|value| Reference::new_integer(value as Integer, registry))
            .unwrap_or_default()
    }

    #[intuicio_method(use_registry)]
    pub fn read_i64(registry: &Registry, mut bytes: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        bytes
            .buffer
            .read_i64::<NetworkEndian>()
            .map(|value| Reference::new_integer(value as Integer, registry))
            .unwrap_or_default()
    }

    #[intuicio_method(use_registry)]
    pub fn read_f32(registry: &Registry, mut bytes: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        bytes
            .buffer
            .read_f32::<NetworkEndian>()
            .map(|value| Reference::new_real(value as Real, registry))
            .unwrap_or_default()
    }

    #[intuicio_method(use_registry)]
    pub fn read_f64(registry: &Registry, mut bytes: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        bytes
            .buffer
            .read_f64::<NetworkEndian>()
            .map(|value| Reference::new_real(value as Real, registry))
            .unwrap_or_default()
    }

    #[intuicio_method(use_registry)]
    pub fn read_text(registry: &Registry, mut bytes: Reference, size: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let size = *size.read::<Integer>().unwrap() as usize;
        let mut result = vec![0; size];
        bytes
            .buffer
            .read_exact(&mut result)
            .map(|_| Reference::new_text(String::from_utf8_lossy(&result).to_string(), registry))
            .unwrap_or_default()
    }

    #[intuicio_method(use_registry)]
    pub fn read_bytes(registry: &Registry, mut bytes: Reference, size: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let size = *size.read::<Integer>().unwrap() as usize;
        let mut result = vec![0; size];
        bytes
            .buffer
            .read_exact(&mut result)
            .map(|_| {
                Reference::new_array(
                    result
                        .into_iter()
                        .map(|byte| Reference::new_integer(byte as Integer, registry))
                        .collect(),
                    registry,
                )
            })
            .unwrap_or_default()
    }

    #[intuicio_method(use_registry)]
    pub fn write_boolean(registry: &Registry, bytes: Reference, value: Reference) -> Reference {
        Self::write_u8(registry, bytes, value)
    }

    #[intuicio_method(use_registry)]
    pub fn write_u8(registry: &Registry, mut bytes: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let value = *value.read::<Integer>().unwrap() as u8;
        Reference::new_boolean(bytes.buffer.write_u8(value).is_ok(), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn write_u16(registry: &Registry, mut bytes: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let value = *value.read::<Integer>().unwrap() as u16;
        Reference::new_boolean(
            bytes.buffer.write_u16::<NetworkEndian>(value).is_ok(),
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn write_u32(registry: &Registry, mut bytes: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let value = *value.read::<Integer>().unwrap() as u32;
        Reference::new_boolean(
            bytes.buffer.write_u32::<NetworkEndian>(value).is_ok(),
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn write_u64(registry: &Registry, mut bytes: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let value = *value.read::<Integer>().unwrap() as u64;
        Reference::new_boolean(
            bytes.buffer.write_u64::<NetworkEndian>(value).is_ok(),
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn write_i8(registry: &Registry, mut bytes: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let value = *value.read::<Integer>().unwrap() as i8;
        Reference::new_boolean(bytes.buffer.write_i8(value).is_ok(), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn write_i16(registry: &Registry, mut bytes: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let value = *value.read::<Integer>().unwrap() as i16;
        Reference::new_boolean(
            bytes.buffer.write_i16::<NetworkEndian>(value).is_ok(),
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn write_i32(registry: &Registry, mut bytes: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let value = *value.read::<Integer>().unwrap() as i32;
        Reference::new_boolean(
            bytes.buffer.write_i32::<NetworkEndian>(value).is_ok(),
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn write_i64(registry: &Registry, mut bytes: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let value = *value.read::<Integer>().unwrap() as i64;
        Reference::new_boolean(
            bytes.buffer.write_i64::<NetworkEndian>(value).is_ok(),
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn write_f32(registry: &Registry, mut bytes: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let value = *value.read::<Real>().unwrap() as f32;
        Reference::new_boolean(
            bytes.buffer.write_f32::<NetworkEndian>(value).is_ok(),
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn write_f64(registry: &Registry, mut bytes: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let value = *value.read::<Real>().unwrap() as f64;
        Reference::new_boolean(
            bytes.buffer.write_f64::<NetworkEndian>(value).is_ok(),
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn write_text(registry: &Registry, mut bytes: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let buffer = value.read::<Text>().unwrap();
        if bytes.buffer.write_all(buffer.as_bytes()).is_ok() {
            Reference::new_integer(buffer.as_bytes().len() as Integer, registry)
        } else {
            Reference::new_integer(0, registry)
        }
    }

    #[intuicio_method(use_registry)]
    pub fn write_bytes(registry: &Registry, mut bytes: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let buffer = value
            .read::<Array>()
            .unwrap()
            .iter()
            .map(|byte| *byte.read::<Integer>().unwrap() as u8)
            .collect::<Vec<_>>();
        if bytes.buffer.write_all(&buffer).is_ok() {
            Reference::new_integer(buffer.len() as Integer, registry)
        } else {
            Reference::new_integer(0, registry)
        }
    }

    #[intuicio_method()]
    pub fn serialize(mut bytes: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        Self::write(&mut bytes.buffer, &value);
        Reference::null()
    }

    #[intuicio_method(use_registry)]
    pub fn deserialize(registry: &Registry, mut bytes: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        Self::read(registry, &mut bytes.buffer)
    }

    fn read_type(buffer: &mut Cursor<Vec<u8>>) -> DataType {
        let result = buffer.read_u8().unwrap();
        unsafe { std::mem::transmute(result) }
    }

    fn read(registry: &Registry, buffer: &mut Cursor<Vec<u8>>) -> Reference {
        match Self::read_type(buffer) {
            DataType::Null => Reference::null(),
            DataType::Boolean => Reference::new_boolean(buffer.read_u8().unwrap() != 0, registry),
            DataType::Integer8 => Reference::new_integer(buffer.read_i8().unwrap() as _, registry),
            DataType::Integer16 => {
                Reference::new_integer(buffer.read_i16::<NetworkEndian>().unwrap() as _, registry)
            }
            DataType::Integer32 => {
                Reference::new_integer(buffer.read_i32::<NetworkEndian>().unwrap() as _, registry)
            }
            DataType::Integer64 => {
                Reference::new_integer(buffer.read_i64::<NetworkEndian>().unwrap() as _, registry)
            }
            DataType::Real => {
                Reference::new_real(buffer.read_f64::<NetworkEndian>().unwrap() as _, registry)
            }
            DataType::Text => {
                let size = buffer.read_u32::<NetworkEndian>().unwrap() as usize;
                let mut bytes = vec![0; size];
                buffer.read_exact(&mut bytes).unwrap();
                Reference::new_text(String::from_utf8_lossy(&bytes).to_string(), registry)
            }
            DataType::Array => {
                let count = buffer.read_u32::<NetworkEndian>().unwrap() as usize;
                let mut result = Array::with_capacity(count);
                for _ in 0..count {
                    result.push(Self::read(registry, buffer));
                }
                Reference::new_array(result, registry)
            }
            DataType::Map => {
                let count = buffer.read_u32::<NetworkEndian>().unwrap() as usize;
                let mut result = Map::with_capacity(count);
                for _ in 0..count {
                    let size = buffer.read_u8().unwrap() as usize;
                    let mut bytes = vec![0; size];
                    buffer.read_exact(&mut bytes).unwrap();
                    result.insert(
                        String::from_utf8_lossy(&bytes).to_string(),
                        Self::read(registry, buffer),
                    );
                }
                Reference::new_map(result, registry)
            }
        }
    }

    fn write_type(buffer: &mut Cursor<Vec<u8>>, data_type: DataType) {
        buffer.write_u8(data_type as u8).unwrap();
    }

    fn write(buffer: &mut Cursor<Vec<u8>>, value: &Reference) {
        if value.is_null() {
            Self::write_type(buffer, DataType::Null);
        } else if let Some(value) = value.read::<Boolean>() {
            Self::write_type(buffer, DataType::Boolean);
            buffer.write_u8(*value as _).unwrap();
        } else if let Some(value) = value.read::<Integer>() {
            if *value & i8::MAX as i64 == *value {
                Self::write_type(buffer, DataType::Integer8);
                buffer.write_i8(*value as _).unwrap();
            } else if *value & i16::MAX as i64 == *value {
                Self::write_type(buffer, DataType::Integer16);
                buffer.write_i16::<NetworkEndian>(*value as _).unwrap();
            } else if *value & i32::MAX as i64 == *value {
                Self::write_type(buffer, DataType::Integer32);
                buffer.write_i32::<NetworkEndian>(*value as _).unwrap();
            } else {
                Self::write_type(buffer, DataType::Integer64);
                buffer.write_i64::<NetworkEndian>(*value).unwrap();
            }
        } else if let Some(value) = value.read::<Real>() {
            Self::write_type(buffer, DataType::Real);
            buffer.write_f64::<NetworkEndian>(*value).unwrap();
        } else if let Some(value) = value.read::<Text>() {
            Self::write_type(buffer, DataType::Text);
            let bytes = value.as_bytes();
            buffer.write_u32::<NetworkEndian>(bytes.len() as _).unwrap();
            buffer.write_all(bytes).unwrap();
        } else if let Some(value) = value.read::<Array>() {
            Self::write_type(buffer, DataType::Array);
            buffer.write_u32::<NetworkEndian>(value.len() as _).unwrap();
            for value in value.iter() {
                Self::write(buffer, value);
            }
        } else if let Some(value) = value.read::<Map>() {
            Self::write_type(buffer, DataType::Map);
            buffer.write_u32::<NetworkEndian>(value.len() as _).unwrap();
            for (key, value) in value.iter() {
                let bytes = key.as_bytes();
                buffer.write_u8(bytes.len() as _).unwrap();
                buffer.write_all(bytes).unwrap();
                Self::write(buffer, value);
            }
        }
    }
}

pub fn install(registry: &mut Registry) {
    registry.add_struct(Bytes::define_struct(registry));
    registry.add_function(Bytes::new__define_function(registry));
    registry.add_function(Bytes::from__define_function(registry));
    registry.add_function(Bytes::into__define_function(registry));
    registry.add_function(Bytes::size__define_function(registry));
    registry.add_function(Bytes::position__define_function(registry));
    registry.add_function(Bytes::set_position__define_function(registry));
    registry.add_function(Bytes::clear__define_function(registry));
    registry.add_function(Bytes::read_boolean__define_function(registry));
    registry.add_function(Bytes::read_u8__define_function(registry));
    registry.add_function(Bytes::read_u16__define_function(registry));
    registry.add_function(Bytes::read_u32__define_function(registry));
    registry.add_function(Bytes::read_u64__define_function(registry));
    registry.add_function(Bytes::read_i8__define_function(registry));
    registry.add_function(Bytes::read_i16__define_function(registry));
    registry.add_function(Bytes::read_i32__define_function(registry));
    registry.add_function(Bytes::read_i64__define_function(registry));
    registry.add_function(Bytes::read_f32__define_function(registry));
    registry.add_function(Bytes::read_f64__define_function(registry));
    registry.add_function(Bytes::read_text__define_function(registry));
    registry.add_function(Bytes::read_bytes__define_function(registry));
    registry.add_function(Bytes::write_boolean__define_function(registry));
    registry.add_function(Bytes::write_u8__define_function(registry));
    registry.add_function(Bytes::write_u16__define_function(registry));
    registry.add_function(Bytes::write_u32__define_function(registry));
    registry.add_function(Bytes::write_u64__define_function(registry));
    registry.add_function(Bytes::write_i8__define_function(registry));
    registry.add_function(Bytes::write_i16__define_function(registry));
    registry.add_function(Bytes::write_i32__define_function(registry));
    registry.add_function(Bytes::write_i64__define_function(registry));
    registry.add_function(Bytes::write_f32__define_function(registry));
    registry.add_function(Bytes::write_f64__define_function(registry));
    registry.add_function(Bytes::write_text__define_function(registry));
    registry.add_function(Bytes::write_bytes__define_function(registry));
    registry.add_function(Bytes::serialize__define_function(registry));
    registry.add_function(Bytes::deserialize__define_function(registry));
}
