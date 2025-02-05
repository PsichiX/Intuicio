use rustc_hash::FxHasher;
use std::hash::{Hash, Hasher};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeHash {
    hash: u64,
}

impl Default for TypeHash {
    fn default() -> Self {
        Self::INVALID
    }
}

impl TypeHash {
    pub const INVALID: Self = Self { hash: 0 };

    pub fn of<T: ?Sized>() -> Self {
        unsafe { Self::raw(std::any::type_name::<T>()) }
    }

    /// # Safety
    pub unsafe fn raw(name: &str) -> Self {
        let mut hasher = FxHasher::default();
        name.hash(&mut hasher);
        Self {
            hash: hasher.finish(),
        }
    }

    pub fn hash(&self) -> u64 {
        self.hash
    }
}

impl std::fmt::Display for TypeHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:X}", self.hash)
    }
}
