use rustc_hash::FxHasher;
use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
};

#[derive(Debug, Copy, Clone)]
pub struct TypeHash {
    hash: u64,
    #[cfg(feature = "typehash_debug_name")]
    name: Option<&'static str>,
}

impl Default for TypeHash {
    fn default() -> Self {
        Self::INVALID
    }
}

impl TypeHash {
    pub const INVALID: Self = Self {
        hash: 0,
        #[cfg(feature = "typehash_debug_name")]
        name: None,
    };

    /// # Safety
    pub unsafe fn raw(name: &str) -> Self {
        let mut hasher = FxHasher::default();
        name.hash(&mut hasher);
        Self {
            hash: hasher.finish(),
            #[cfg(feature = "typehash_debug_name")]
            name: None,
        }
    }

    /// # Safety
    pub unsafe fn raw_static(name: &'static str) -> Self {
        let mut hasher = FxHasher::default();
        name.hash(&mut hasher);
        Self {
            hash: hasher.finish(),
            #[cfg(feature = "typehash_debug_name")]
            name: Some(name),
        }
    }

    pub fn of<T: ?Sized>() -> Self {
        let name = std::any::type_name::<T>();
        let mut hasher = FxHasher::default();
        name.hash(&mut hasher);
        Self {
            hash: hasher.finish(),
            #[cfg(feature = "typehash_debug_name")]
            name: Some(name),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.hash != Self::INVALID.hash
    }

    pub fn hash(&self) -> u64 {
        self.hash
    }

    #[cfg(feature = "typehash_debug_name")]
    pub fn name(&self) -> Option<&'static str> {
        self.name
    }
}

impl PartialEq for TypeHash {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl Eq for TypeHash {}

impl PartialOrd for TypeHash {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TypeHash {
    fn cmp(&self, other: &Self) -> Ordering {
        self.hash.cmp(&other.hash)
    }
}

impl Hash for TypeHash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

impl std::fmt::Display for TypeHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[cfg(feature = "typehash_debug_name")]
        {
            if let Some(name) = self.name {
                return write!(f, "#{:X}: {}", self.hash, name);
            }
        }
        write!(f, "#{:X}", self.hash)
    }
}
