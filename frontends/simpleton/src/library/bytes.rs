use crate::{Array, Boolean, Integer, Map, Real, Reference, Text};
use byteorder::{NativeEndian, NetworkEndian, ReadBytesExt, WriteBytesExt};
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
    #[intuicio(ignore)]
    native_endian: bool,
}

#[intuicio_methods(module_name = "bytes")]
impl Bytes {
    pub fn new_raw(bytes: Vec<u8>) -> Self {
        Self {
            buffer: Cursor::new(bytes),
            native_endian: false,
        }
    }

    pub fn get_ref(&self) -> &[u8] {
        self.buffer.get_ref().as_slice()
    }

    pub fn get_mut(&mut self) -> &mut [u8] {
        self.buffer.get_mut().as_mut_slice()
    }

    #[allow(clippy::new_ret_no_self)]
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
        Reference::new(Self::new_raw(buffer), registry)
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

    #[intuicio_method(use_registry)]
    pub fn native_endian(registry: &Registry, bytes: Reference) -> Reference {
        let bytes = bytes.read::<Bytes>().unwrap();
        Reference::new_boolean(bytes.native_endian, registry)
    }

    #[intuicio_method()]
    pub fn set_native_endian(mut bytes: Reference, mode: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        bytes.native_endian = *mode.read::<Boolean>().unwrap();
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
    pub fn get_bit(registry: &Registry, bytes: Reference, index: Reference) -> Reference {
        let bytes = bytes.read::<Bytes>().unwrap();
        let index = *index.read::<Integer>().unwrap() as usize;
        let offset = index % std::mem::size_of::<u8>();
        let index = index / std::mem::size_of::<u8>();
        let result = bytes.buffer.get_ref()[index] & (1 << offset);
        Reference::new_boolean(result != 0, registry)
    }

    #[intuicio_method(use_registry)]
    pub fn set_bit(mut bytes: Reference, index: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let index = *index.read::<Integer>().unwrap() as usize;
        let value = *value.read::<Boolean>().unwrap();
        let offset = index % std::mem::size_of::<u8>();
        let index = index / std::mem::size_of::<u8>();
        let byte = &mut bytes.buffer.get_mut()[index];
        let enabled = (*byte & (1 << offset)) != 0;
        if enabled != value {
            *byte ^= 1 << offset;
        }
        Reference::null()
    }

    #[intuicio_method(use_registry)]
    pub fn get_integer(registry: &Registry, bytes: Reference, index: Reference) -> Reference {
        let bytes = bytes.read::<Bytes>().unwrap();
        let index = *index.read::<Integer>().unwrap() as usize;
        let buffer = unsafe { bytes.get_ref().align_to::<Integer>().1 };
        Reference::new_integer(buffer[index], registry)
    }

    #[intuicio_method(use_registry)]
    pub fn set_integer(mut bytes: Reference, index: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let index = *index.read::<Integer>().unwrap() as usize;
        let value = *value.read::<Integer>().unwrap();
        let buffer = unsafe { bytes.get_mut().align_to_mut::<Integer>().1 };
        buffer[index] = value;
        Reference::null()
    }

    #[intuicio_method(use_registry)]
    pub fn get_real(registry: &Registry, bytes: Reference, index: Reference) -> Reference {
        let bytes = bytes.read::<Bytes>().unwrap();
        let index = *index.read::<Integer>().unwrap() as usize;
        let buffer = unsafe { bytes.get_ref().align_to::<Real>().1 };
        Reference::new_real(buffer[index], registry)
    }

    #[intuicio_method(use_registry)]
    pub fn set_real(mut bytes: Reference, index: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let index = *index.read::<Integer>().unwrap() as usize;
        let value = *value.read::<Real>().unwrap();
        let buffer = unsafe { bytes.get_mut().align_to_mut::<Real>().1 };
        buffer[index] = value;
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
        let result = if bytes.native_endian {
            bytes.buffer.read_u16::<NativeEndian>()
        } else {
            bytes.buffer.read_u16::<NetworkEndian>()
        };
        result
            .map(|value| Reference::new_integer(value as Integer, registry))
            .unwrap_or_default()
    }

    #[intuicio_method(use_registry)]
    pub fn read_u32(registry: &Registry, mut bytes: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let result = if bytes.native_endian {
            bytes.buffer.read_u32::<NativeEndian>()
        } else {
            bytes.buffer.read_u32::<NetworkEndian>()
        };
        result
            .map(|value| Reference::new_integer(value as Integer, registry))
            .unwrap_or_default()
    }

    #[intuicio_method(use_registry)]
    pub fn read_u64(registry: &Registry, mut bytes: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let result = if bytes.native_endian {
            bytes.buffer.read_u64::<NativeEndian>()
        } else {
            bytes.buffer.read_u64::<NetworkEndian>()
        };
        result
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
        let result = if bytes.native_endian {
            bytes.buffer.read_i16::<NativeEndian>()
        } else {
            bytes.buffer.read_i16::<NetworkEndian>()
        };
        result
            .map(|value| Reference::new_integer(value as Integer, registry))
            .unwrap_or_default()
    }

    #[intuicio_method(use_registry)]
    pub fn read_i32(registry: &Registry, mut bytes: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let result = if bytes.native_endian {
            bytes.buffer.read_i32::<NativeEndian>()
        } else {
            bytes.buffer.read_i32::<NetworkEndian>()
        };
        result
            .map(|value| Reference::new_integer(value as Integer, registry))
            .unwrap_or_default()
    }

    #[intuicio_method(use_registry)]
    pub fn read_i64(registry: &Registry, mut bytes: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let result = if bytes.native_endian {
            bytes.buffer.read_i64::<NativeEndian>()
        } else {
            bytes.buffer.read_i64::<NetworkEndian>()
        };
        result
            .map(|value| Reference::new_integer(value as Integer, registry))
            .unwrap_or_default()
    }

    #[intuicio_method(use_registry)]
    pub fn read_f32(registry: &Registry, mut bytes: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let result = if bytes.native_endian {
            bytes.buffer.read_f32::<NativeEndian>()
        } else {
            bytes.buffer.read_f32::<NetworkEndian>()
        };
        result
            .map(|value| Reference::new_real(value as Real, registry))
            .unwrap_or_default()
    }

    #[intuicio_method(use_registry)]
    pub fn read_f64(registry: &Registry, mut bytes: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let result = if bytes.native_endian {
            bytes.buffer.read_f64::<NativeEndian>()
        } else {
            bytes.buffer.read_f64::<NetworkEndian>()
        };
        result
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
            .map(|_| Reference::new(Self::new_raw(result), registry))
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
            if bytes.native_endian {
                bytes.buffer.write_u16::<NativeEndian>(value).is_ok()
            } else {
                bytes.buffer.write_u16::<NetworkEndian>(value).is_ok()
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn write_u32(registry: &Registry, mut bytes: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let value = *value.read::<Integer>().unwrap() as u32;
        Reference::new_boolean(
            if bytes.native_endian {
                bytes.buffer.write_u32::<NativeEndian>(value).is_ok()
            } else {
                bytes.buffer.write_u32::<NetworkEndian>(value).is_ok()
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn write_u64(registry: &Registry, mut bytes: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let value = *value.read::<Integer>().unwrap() as u64;
        Reference::new_boolean(
            if bytes.native_endian {
                bytes.buffer.write_u64::<NativeEndian>(value).is_ok()
            } else {
                bytes.buffer.write_u64::<NetworkEndian>(value).is_ok()
            },
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
            if bytes.native_endian {
                bytes.buffer.write_i16::<NativeEndian>(value).is_ok()
            } else {
                bytes.buffer.write_i16::<NetworkEndian>(value).is_ok()
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn write_i32(registry: &Registry, mut bytes: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let value = *value.read::<Integer>().unwrap() as i32;
        Reference::new_boolean(
            if bytes.native_endian {
                bytes.buffer.write_i32::<NativeEndian>(value).is_ok()
            } else {
                bytes.buffer.write_i32::<NetworkEndian>(value).is_ok()
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn write_i64(registry: &Registry, mut bytes: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let value = *value.read::<Integer>().unwrap();
        Reference::new_boolean(
            if bytes.native_endian {
                bytes.buffer.write_i64::<NativeEndian>(value).is_ok()
            } else {
                bytes.buffer.write_i64::<NetworkEndian>(value).is_ok()
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn write_f32(registry: &Registry, mut bytes: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let value = *value.read::<Real>().unwrap() as f32;
        Reference::new_boolean(
            if bytes.native_endian {
                bytes.buffer.write_f32::<NativeEndian>(value).is_ok()
            } else {
                bytes.buffer.write_f32::<NetworkEndian>(value).is_ok()
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn write_f64(registry: &Registry, mut bytes: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        let value = *value.read::<Real>().unwrap();
        Reference::new_boolean(
            if bytes.native_endian {
                bytes.buffer.write_f64::<NativeEndian>(value).is_ok()
            } else {
                bytes.buffer.write_f64::<NetworkEndian>(value).is_ok()
            },
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
        let value = value.read::<Bytes>().unwrap();
        if bytes.buffer.write_all(value.buffer.get_ref()).is_ok() {
            Reference::new_integer(value.buffer.get_ref().len() as Integer, registry)
        } else {
            Reference::new_integer(0, registry)
        }
    }

    #[intuicio_method()]
    pub fn serialize(mut bytes: Reference, value: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        Self::write(bytes.native_endian, &mut bytes.buffer, &value);
        Reference::null()
    }

    #[intuicio_method(use_registry)]
    pub fn deserialize(registry: &Registry, mut bytes: Reference) -> Reference {
        let mut bytes = bytes.write::<Bytes>().unwrap();
        Self::read(registry, bytes.native_endian, &mut bytes.buffer)
    }

    fn read_type(buffer: &mut Cursor<Vec<u8>>) -> DataType {
        let result = buffer.read_u8().unwrap();
        unsafe { std::mem::transmute(result) }
    }

    fn read(registry: &Registry, native_endian: bool, buffer: &mut Cursor<Vec<u8>>) -> Reference {
        match Self::read_type(buffer) {
            DataType::Null => Reference::null(),
            DataType::Boolean => Reference::new_boolean(buffer.read_u8().unwrap() != 0, registry),
            DataType::Integer8 => Reference::new_integer(buffer.read_i8().unwrap() as _, registry),
            DataType::Integer16 => Reference::new_integer(
                if native_endian {
                    buffer.read_i16::<NativeEndian>().unwrap()
                } else {
                    buffer.read_i16::<NetworkEndian>().unwrap()
                } as _,
                registry,
            ),
            DataType::Integer32 => Reference::new_integer(
                if native_endian {
                    buffer.read_i32::<NativeEndian>().unwrap()
                } else {
                    buffer.read_i32::<NetworkEndian>().unwrap()
                } as _,
                registry,
            ),
            DataType::Integer64 => Reference::new_integer(
                if native_endian {
                    buffer.read_i64::<NativeEndian>().unwrap()
                } else {
                    buffer.read_i64::<NetworkEndian>().unwrap()
                } as _,
                registry,
            ),
            DataType::Real => Reference::new_real(
                if native_endian {
                    buffer.read_f64::<NativeEndian>().unwrap()
                } else {
                    buffer.read_f64::<NetworkEndian>().unwrap()
                } as _,
                registry,
            ),
            DataType::Text => {
                let size = if native_endian {
                    buffer.read_u32::<NativeEndian>().unwrap()
                } else {
                    buffer.read_u32::<NetworkEndian>().unwrap()
                } as usize;
                let mut bytes = vec![0; size];
                buffer.read_exact(&mut bytes).unwrap();
                Reference::new_text(String::from_utf8_lossy(&bytes).to_string(), registry)
            }
            DataType::Array => {
                let count = if native_endian {
                    buffer.read_u32::<NativeEndian>().unwrap()
                } else {
                    buffer.read_u32::<NetworkEndian>().unwrap()
                } as usize;
                let mut result = Array::with_capacity(count);
                for _ in 0..count {
                    result.push(Self::read(registry, native_endian, buffer));
                }
                Reference::new_array(result, registry)
            }
            DataType::Map => {
                let count = if native_endian {
                    buffer.read_u32::<NativeEndian>().unwrap()
                } else {
                    buffer.read_u32::<NetworkEndian>().unwrap()
                } as usize;
                let mut result = Map::with_capacity(count);
                for _ in 0..count {
                    let size = buffer.read_u8().unwrap() as usize;
                    let mut bytes = vec![0; size];
                    buffer.read_exact(&mut bytes).unwrap();
                    result.insert(
                        String::from_utf8_lossy(&bytes).to_string(),
                        Self::read(registry, native_endian, buffer),
                    );
                }
                Reference::new_map(result, registry)
            }
        }
    }

    fn write_type(buffer: &mut Cursor<Vec<u8>>, data_type: DataType) {
        buffer.write_u8(data_type as u8).unwrap();
    }

    fn write(native_endian: bool, buffer: &mut Cursor<Vec<u8>>, value: &Reference) {
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
                if native_endian {
                    buffer.write_i16::<NativeEndian>(*value as _).unwrap();
                } else {
                    buffer.write_i16::<NetworkEndian>(*value as _).unwrap();
                }
            } else if *value & i32::MAX as i64 == *value {
                Self::write_type(buffer, DataType::Integer32);
                if native_endian {
                    buffer.write_i32::<NativeEndian>(*value as _).unwrap();
                } else {
                    buffer.write_i32::<NetworkEndian>(*value as _).unwrap();
                }
            } else {
                Self::write_type(buffer, DataType::Integer64);
                if native_endian {
                    buffer.write_i64::<NativeEndian>(*value as _).unwrap();
                } else {
                    buffer.write_i64::<NetworkEndian>(*value as _).unwrap();
                }
            }
        } else if let Some(value) = value.read::<Real>() {
            Self::write_type(buffer, DataType::Real);
            if native_endian {
                buffer.write_f64::<NativeEndian>(*value as _).unwrap();
            } else {
                buffer.write_f64::<NetworkEndian>(*value as _).unwrap();
            }
        } else if let Some(value) = value.read::<Text>() {
            Self::write_type(buffer, DataType::Text);
            let bytes = value.as_bytes();
            if native_endian {
                buffer.write_u32::<NativeEndian>(bytes.len() as _).unwrap();
            } else {
                buffer.write_u32::<NetworkEndian>(bytes.len() as _).unwrap();
            }
            buffer.write_all(bytes).unwrap();
        } else if let Some(value) = value.read::<Array>() {
            Self::write_type(buffer, DataType::Array);
            if native_endian {
                buffer.write_u32::<NativeEndian>(value.len() as _).unwrap();
            } else {
                buffer.write_u32::<NetworkEndian>(value.len() as _).unwrap();
            }
            for value in value.iter() {
                Self::write(native_endian, buffer, value);
            }
        } else if let Some(value) = value.read::<Map>() {
            Self::write_type(buffer, DataType::Map);
            if native_endian {
                buffer.write_u32::<NativeEndian>(value.len() as _).unwrap();
            } else {
                buffer.write_u32::<NetworkEndian>(value.len() as _).unwrap();
            }
            for (key, value) in value.iter() {
                let bytes = key.as_bytes();
                buffer.write_u8(bytes.len() as _).unwrap();
                buffer.write_all(bytes).unwrap();
                Self::write(native_endian, buffer, value);
            }
        }
    }
}

pub fn install(registry: &mut Registry) {
    registry.add_type(Bytes::define_struct(registry));
    registry.add_function(Bytes::new__define_function(registry));
    registry.add_function(Bytes::from__define_function(registry));
    registry.add_function(Bytes::into__define_function(registry));
    registry.add_function(Bytes::size__define_function(registry));
    registry.add_function(Bytes::position__define_function(registry));
    registry.add_function(Bytes::set_position__define_function(registry));
    registry.add_function(Bytes::native_endian__define_function(registry));
    registry.add_function(Bytes::set_native_endian__define_function(registry));
    registry.add_function(Bytes::clear__define_function(registry));
    registry.add_function(Bytes::get_bit__define_function(registry));
    registry.add_function(Bytes::set_bit__define_function(registry));
    registry.add_function(Bytes::get_integer__define_function(registry));
    registry.add_function(Bytes::set_integer__define_function(registry));
    registry.add_function(Bytes::get_real__define_function(registry));
    registry.add_function(Bytes::set_real__define_function(registry));
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
