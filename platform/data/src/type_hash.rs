use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeHash {
    hash: u64,
}

impl TypeHash {
    pub fn of<T: ?Sized>() -> Self {
        unsafe { Self::raw(std::any::type_name::<T>()) }
    }

    /// # Safety
    pub unsafe fn raw(name: &str) -> Self {
        let mut hasher = DefaultHasher::default();
        name.hash(&mut hasher);
        Self {
            hash: hasher.finish(),
        }
    }

    pub fn hash(&self) -> u64 {
        self.hash
    }
}
