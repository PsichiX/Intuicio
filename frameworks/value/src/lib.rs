use intuicio_data::{
    is_copy,
    lifetime::{ValueReadAccess, ValueWriteAccess},
    managed_box::DynamicManagedBox,
    type_hash::TypeHash,
};
use std::{cmp::Ordering, collections::BTreeMap};

const SIZE: usize = std::mem::size_of::<DynamicManagedBox>();

#[derive(Default)]
enum ValueContent {
    #[default]
    Null,
    Object(DynamicManagedBox),
    Primitive {
        type_hash: TypeHash,
        data: [u8; SIZE],
    },
    String(String),
    Array(Vec<Value>),
    Map(BTreeMap<Value, Value>),
}

impl ValueContent {
    fn type_hash(&self) -> Option<TypeHash> {
        match self {
            Self::Null => None,
            Self::Object(data) => data.type_hash(),
            Self::Primitive { type_hash, .. } => Some(*type_hash),
            Self::String(_) => Some(TypeHash::of::<String>()),
            Self::Array(_) => Some(TypeHash::of::<Vec<Value>>()),
            Self::Map(_) => Some(TypeHash::of::<BTreeMap<Value, Value>>()),
        }
    }

    fn order(&self) -> u8 {
        match self {
            Self::Null => 0,
            Self::Object(_) => 1,
            Self::Primitive { .. } => 2,
            Self::String(_) => 3,
            Self::Array(_) => 4,
            Self::Map(_) => 5,
        }
    }
}

impl Clone for ValueContent {
    fn clone(&self) -> Self {
        match self {
            Self::Null => Self::Null,
            Self::Object(value) => Self::Object(value.clone()),
            Self::Primitive { type_hash, data } => Self::Primitive {
                type_hash: *type_hash,
                data: *data,
            },
            Self::String(value) => Self::String(value.clone()),
            Self::Array(value) => Self::Array(value.clone()),
            Self::Map(value) => Self::Map(value.clone()),
        }
    }
}

impl PartialEq for ValueContent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Object(me), Self::Object(other)) => unsafe { me.memory() == other.memory() },
            (
                Self::Primitive {
                    type_hash: my_type_hash,
                    data: my_data,
                },
                Self::Primitive {
                    type_hash: other_type_hash,
                    data: other_data,
                },
            ) => my_type_hash == other_type_hash && my_data == other_data,
            (Self::String(me), Self::String(other)) => me == other,
            (Self::Array(me), Self::Array(other)) => me == other,
            (Self::Map(me), Self::Map(other)) => me == other,
            _ => false,
        }
    }
}

impl Eq for ValueContent {}

impl PartialOrd for ValueContent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ValueContent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.order()
            .cmp(&other.order())
            .then_with(|| match (self, other) {
                (Self::Null, Self::Null) => Ordering::Equal,
                (Self::Object(me), Self::Object(other)) => unsafe {
                    me.memory().as_slice().cmp(other.memory().as_slice())
                },
                (
                    Self::Primitive {
                        type_hash: my_type_hash,
                        data: my_data,
                    },
                    Self::Primitive {
                        type_hash: other_type_hash,
                        data: other_data,
                    },
                ) => my_type_hash
                    .cmp(other_type_hash)
                    .then_with(|| my_data.cmp(other_data)),
                (Self::String(me), Self::String(other)) => me.cmp(other),
                (Self::Array(me), Self::Array(other)) => me.cmp(other),
                (Self::Map(me), Self::Map(other)) => me.cmp(other),
                _ => unreachable!(),
            })
    }
}

#[derive(Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Value {
    inner: ValueContent,
}

impl Value {
    pub fn type_hash(&self) -> Option<TypeHash> {
        self.inner.type_hash()
    }

    pub fn null() -> Self {
        Self {
            inner: ValueContent::Null,
        }
    }

    pub fn primitive_or_object<T>(value: T) -> Self {
        if is_copy::<T>() && std::mem::size_of::<T>() <= SIZE {
            let mut data = [0; SIZE];
            unsafe {
                data.as_mut_ptr().cast::<T>().write(value);
            }
            Self {
                inner: ValueContent::Primitive {
                    type_hash: TypeHash::of::<T>(),
                    data,
                },
            }
        } else {
            Self {
                inner: ValueContent::Object(DynamicManagedBox::new(value)),
            }
        }
    }

    pub fn object_raw(value: DynamicManagedBox) -> Self {
        Self {
            inner: ValueContent::Object(value),
        }
    }

    pub fn string(value: impl ToString) -> Self {
        Self {
            inner: ValueContent::String(value.to_string()),
        }
    }

    pub fn array(values: impl IntoIterator<Item = Value>) -> Self {
        Self {
            inner: ValueContent::Array(values.into_iter().collect()),
        }
    }

    pub fn array_empty() -> Self {
        Self {
            inner: ValueContent::Array(Default::default()),
        }
    }

    pub fn map(values: impl IntoIterator<Item = (Value, Value)>) -> Self {
        Self {
            inner: ValueContent::Map(values.into_iter().collect()),
        }
    }

    pub fn map_empty() -> Self {
        Self {
            inner: ValueContent::Map(Default::default()),
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self.inner, ValueContent::Null)
    }

    pub fn is<T>(&self) -> bool {
        self.type_hash()
            .map(|type_hash| type_hash == TypeHash::of::<T>())
            .unwrap_or_default()
    }

    pub fn as_object<T>(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        if let ValueContent::Object(value) = &self.inner {
            value.read::<T>()
        } else {
            None
        }
    }

    pub fn as_object_mut<T>(&'_ mut self) -> Option<ValueWriteAccess<'_, T>> {
        if let ValueContent::Object(value) = &mut self.inner {
            value.write::<T>()
        } else {
            None
        }
    }

    pub fn as_primitive<T: Copy>(&self) -> Option<T> {
        if let ValueContent::Primitive { type_hash, data } = &self.inner {
            if *type_hash == TypeHash::of::<T>() {
                unsafe { Some(data.as_ptr().cast::<T>().read()) }
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn as_string(&self) -> Option<&str> {
        if let ValueContent::String(content) = &self.inner {
            Some(content.as_str())
        } else {
            None
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Value>> {
        if let ValueContent::Array(content) = &self.inner {
            Some(content)
        } else {
            None
        }
    }

    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Value>> {
        if let ValueContent::Array(content) = &mut self.inner {
            Some(content)
        } else {
            None
        }
    }

    pub fn as_map(&self) -> Option<&BTreeMap<Value, Value>> {
        if let ValueContent::Map(content) = &self.inner {
            Some(content)
        } else {
            None
        }
    }

    pub fn as_map_mut(&mut self) -> Option<&mut BTreeMap<Value, Value>> {
        if let ValueContent::Map(content) = &mut self.inner {
            Some(content)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value() {
        assert_eq!(std::mem::size_of::<Value>(), 48);
        assert_eq!(SIZE, 32);
        let a = Value::primitive_or_object(42u8);
        let b = Value::primitive_or_object(10u16);
        let c = Value::primitive_or_object(4.2f32);
        let d = Value::array([a.clone(), b.clone(), c.clone()]);
        let mut e = Value::primitive_or_object([42u64; 10000]);
        let k1 = Value::string("foo");
        let k2 = Value::string("bar");
        let f = Value::map([(k1.clone(), d.clone()), (k2.clone(), e.clone())]);
        let g = Value::null();
        assert_eq!(a.as_primitive::<u8>().unwrap(), 42);
        assert_eq!(b.as_primitive::<u16>().unwrap(), 10);
        assert_eq!(c.as_primitive::<f32>().unwrap(), 4.2);
        e.as_object_mut::<[u64; 10000]>().unwrap()[0] = 10;
        assert_eq!(e.as_object::<[u64; 10000]>().unwrap()[0], 10);
        assert!(f.as_map().unwrap()[&k1] == d);
        assert!(f.as_map().unwrap()[&k1].as_array().unwrap()[0] == a);
        assert!(f.as_map().unwrap()[&k1].as_array().unwrap()[1] == b);
        assert!(f.as_map().unwrap()[&k1].as_array().unwrap()[2] == c);
        assert!(f.as_map().unwrap()[&k2] == e);
        assert!(g.is_null());
        drop(f);
    }
}
