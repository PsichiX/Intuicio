use intuicio_data::type_hash::TypeHash;
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
    serialize_from: Box<dyn Fn(*const u8) -> Result<Intermediate, Box<dyn Error>> + Send + Sync>,
    #[allow(clippy::type_complexity)]
    deserialize_to: Box<dyn Fn(*mut u8, &Intermediate) -> Result<(), Box<dyn Error>> + Send + Sync>,
}

#[derive(Default)]
pub struct SerializationRegistry {
    mapping: HashMap<TypeHash, Serializer>,
}

impl SerializationRegistry {
    pub fn with_basic_types(mut self) -> Self {
        self.register::<()>(
            |_| Ok(Intermediate::Unit),
            |_, value| {
                if matches!(value, Intermediate::Unit) {
                    Ok(())
                } else {
                    Err("Expected unit value".into())
                }
            },
        );
        self.register::<bool>(
            |data| Ok((*data).into()),
            |data, value| {
                if let Intermediate::Bool(value) = value {
                    *data = *value;
                    Ok(())
                } else {
                    Err("Expected bool value".into())
                }
            },
        );
        self.register::<i8>(
            |data| Ok((*data).into()),
            |data, value| {
                if let Intermediate::I8(value) = value {
                    *data = *value;
                    Ok(())
                } else {
                    Err("Expected i8 value".into())
                }
            },
        );
        self.register::<i16>(
            |data| Ok((*data).into()),
            |data, value| match value {
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
            |data| Ok((*data).into()),
            |data, value| match value {
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
            |data| Ok((*data).into()),
            |data, value| match value {
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
            |data| Ok((*data).into()),
            |data, value| match value {
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
            |data| Ok((*data).into()),
            |data, value| match value {
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
            |data| Ok((*data).into()),
            |data, value| {
                if let Intermediate::U8(value) = value {
                    *data = *value;
                    Ok(())
                } else {
                    Err("Expected u8 value".into())
                }
            },
        );
        self.register::<u16>(
            |data| Ok((*data).into()),
            |data, value| match value {
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
            |data| Ok((*data).into()),
            |data, value| match value {
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
            |data| Ok((*data).into()),
            |data, value| match value {
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
            |data| Ok((*data).into()),
            |data, value| match value {
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
            |data| Ok((*data).into()),
            |data, value| match value {
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
            |data| Ok((*data).into()),
            |data, value| match value {
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
            |data| Ok((*data).into()),
            |data, value| match value {
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
            |data| Ok((*data).into()),
            |data, value| match value {
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
            |data| Ok(data.as_str().into()),
            |data, value| {
                if let Intermediate::String(value) = value {
                    *data = value.to_owned();
                    Ok(())
                } else {
                    Err("Expected string value".into())
                }
            },
        );
        self
    }

    pub fn with_serde<T: Serialize + DeserializeOwned>(mut self) -> Self {
        self.register_serde::<T>();
        self
    }

    pub fn register_serde<T: Serialize + DeserializeOwned>(&mut self) {
        self.register::<T>(
            |data| Ok(serde_intermediate::to_intermediate(data)?),
            |data, value| {
                *data = serde_intermediate::from_intermediate(value)?;
                Ok(())
            },
        );
    }

    pub fn register<T>(
        &mut self,
        serialize_from: impl Fn(&T) -> Result<Intermediate, Box<dyn Error>> + Send + Sync + 'static,
        deserialize_to: impl Fn(&mut T, &Intermediate) -> Result<(), Box<dyn Error>>
        + Send
        + Sync
        + 'static,
    ) {
        unsafe {
            self.register_raw(
                TypeHash::of::<T>(),
                move |data| serialize_from(data.cast::<T>().as_ref().unwrap()),
                move |data, value| deserialize_to(data.cast::<T>().as_mut().unwrap(), value),
            );
        }
    }

    /// # Safety
    pub unsafe fn register_raw(
        &mut self,
        type_hash: TypeHash,
        serialize_from: impl Fn(*const u8) -> Result<Intermediate, Box<dyn Error>>
        + Send
        + Sync
        + 'static,
        deserialize_to: impl Fn(*mut u8, &Intermediate) -> Result<(), Box<dyn Error>>
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

    pub fn serialize_from<T>(&self, data: &T) -> Result<Intermediate, Box<dyn Error>> {
        unsafe { self.dynamic_serialize_from(TypeHash::of::<T>(), data as *const T as *const u8) }
    }

    /// # Safety
    pub unsafe fn dynamic_serialize_from(
        &self,
        type_hash: TypeHash,
        data: *const u8,
    ) -> Result<Intermediate, Box<dyn Error>> {
        if let Some(serializer) = self.mapping.get(&type_hash) {
            return (serializer.serialize_from)(data);
        }
        Err("Type not existent in serialization registry".into())
    }

    pub fn deserialize_to<T: Default>(&self, value: &Intermediate) -> Result<T, Box<dyn Error>> {
        let mut result = T::default();
        unsafe {
            self.dynamic_deserialize_to(
                TypeHash::of::<T>(),
                &mut result as *mut T as *mut u8,
                value,
            )?;
        }
        Ok(result)
    }

    /// # Safety
    pub unsafe fn dynamic_deserialize_to(
        &self,
        type_hash: TypeHash,
        data: *mut u8,
        value: &Intermediate,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(serializer) = self.mapping.get(&type_hash) {
            (serializer.deserialize_to)(data, value)?;
            return Ok(());
        }
        Err("Type not existent in serialization registry".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use intuicio_derive::{IntuicioEnum, IntuicioStruct};
    use serde::Deserialize;

    #[derive(IntuicioEnum, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    #[repr(u8)]
    enum Skill {
        #[default]
        Brain,
        Muscles(bool),
        Magic {
            power: i32,
        },
    }

    #[derive(IntuicioStruct, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    struct Person {
        name: String,
        age: usize,
        skill: Skill,
    }

    #[test]
    fn test_serialization() {
        let serialization = SerializationRegistry::default()
            .with_basic_types()
            .with_serde::<Skill>()
            .with_serde::<Person>();

        let person = Person {
            name: "Grumpy".to_owned(),
            age: 24,
            skill: Skill::Magic { power: 42 },
        };
        let serialized = serialization.serialize_from(&person).unwrap();
        let person2 = serialization.deserialize_to::<Person>(&serialized).unwrap();
        assert_eq!(person, person2);
    }
}
