//! Cheap runtime type identity.
//!
//! See [`TypeHash`].
use rustc_hash::FxHasher;
use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
};

/// Runtime identity of a type, stored as a hash of its full type name.
///
/// Used instead of [`std::any::TypeId`] because it can also be built for
/// types that only exist on the script side, from their name alone (see
/// [`TypeHash::raw`]). Comparison, ordering and hashing all use the hash
/// only, never the name.
///
/// ```
/// # use intuicio_data::type_hash::TypeHash;
/// assert_eq!(TypeHash::of::<i32>(), TypeHash::of::<i32>());
/// assert_ne!(TypeHash::of::<i32>(), TypeHash::of::<f32>());
/// ```
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
    /// Hash that no real type maps to, used as a null value.
    ///
    /// This is also what [`TypeHash::default`] returns.
    pub const INVALID: Self = Self {
        hash: 0,
        #[cfg(feature = "typehash_debug_name")]
        name: None,
    };

    /// Builds a hash from a type name given at runtime.
    ///
    /// # Safety
    ///
    /// The name must be the full, qualified name of the type, the same name
    /// [`TypeHash::of`] builds its hash from. A different name makes two views
    /// of one type compare as different types. Type-erased containers use that
    /// comparison to decide if a cast is safe.
    pub unsafe fn raw(name: &str) -> Self {
        let mut hasher = FxHasher::default();
        name.hash(&mut hasher);
        Self {
            hash: hasher.finish(),
            #[cfg(feature = "typehash_debug_name")]
            name: None,
        }
    }

    /// Same as [`TypeHash::raw`], but keeps the name for diagnostics when the
    /// `typehash_debug_name` feature is on.
    ///
    /// # Safety
    ///
    /// Same as [`TypeHash::raw`].
    pub unsafe fn raw_static(name: &'static str) -> Self {
        let mut hasher = FxHasher::default();
        name.hash(&mut hasher);
        Self {
            hash: hasher.finish(),
            #[cfg(feature = "typehash_debug_name")]
            name: Some(name),
        }
    }

    /// Builds the hash of a type that exists only on the script side, from its
    /// qualified name.
    ///
    /// The name is salted with NUL bytes, which [`std::any::type_name`] never
    /// produces. So a runtime type never gets the same hash as a Rust type, and
    /// a runtime value never passes the check that reads it as a Rust type.
    /// Reflection stays the only way to read such a value.
    ///
    /// This is safe, unlike [`TypeHash::raw`], because the hash it builds
    /// cannot match a Rust type.
    ///
    /// ```
    /// # use intuicio_data::type_hash::TypeHash;
    /// assert_eq!(
    ///     TypeHash::of_runtime("game::Player"),
    ///     TypeHash::of_runtime("game::Player")
    /// );
    /// assert_ne!(
    ///     TypeHash::of_runtime("game::Player"),
    ///     TypeHash::of_runtime("game::Vector")
    /// );
    /// assert_ne!(TypeHash::of_runtime("i32"), TypeHash::of::<i32>());
    /// ```
    pub fn of_runtime(qualified_name: &str) -> Self {
        const SALT: &str = "\0intuicio::runtime\0";
        let mut hasher = FxHasher::default();
        SALT.hash(&mut hasher);
        qualified_name.hash(&mut hasher);
        Self {
            hash: hasher.finish(),
            #[cfg(feature = "typehash_debug_name")]
            name: None,
        }
    }

    /// Builds the hash of a Rust type known at compile time.
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

    /// Returns `false` only for [`TypeHash::INVALID`].
    pub fn is_valid(&self) -> bool {
        self.hash != Self::INVALID.hash
    }

    /// Returns the raw hash value.
    pub fn hash(&self) -> u64 {
        self.hash
    }

    /// Returns the type name this hash was built from, when it is known.
    ///
    /// Only available with the `typehash_debug_name` feature.
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
