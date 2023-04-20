pub mod context;
pub mod function;
pub mod host;
pub mod meta;
pub mod nativizer;
pub mod object;
pub mod registry;
pub mod script;
pub mod struct_type;

pub mod prelude {
    pub use crate::{
        context::*, function::*, host::*, nativizer::*, object::*, registry::*, script::*,
        struct_type::*, IntuicioStruct, Visibility,
    };
    pub use crate::{
        define_function, define_native_struct, define_runtime_struct, function_signature,
    };
}

pub mod __internal {
    pub use memoffset::offset_of;
}

use crate::{registry::Registry, struct_type::Struct};
use serde::{Deserialize, Serialize};
use std::{cell::Cell, marker::PhantomData};

#[derive(
    Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum Visibility {
    Private,
    Module,
    #[default]
    Public,
}

impl Visibility {
    pub fn is_visible(self, scope: Self) -> bool {
        self >= scope
    }

    pub fn is_public(&self) -> bool {
        *self == Visibility::Public
    }

    pub fn is_module(&self) -> bool {
        *self == Visibility::Module
    }

    pub fn is_private(&self) -> bool {
        *self == Visibility::Private
    }
}

pub trait IntuicioStruct {
    fn define_struct(registry: &Registry) -> Struct;
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct IntuicioVersion {
    major: usize,
    minor: usize,
    patch: usize,
}

impl IntuicioVersion {
    pub fn new(major: usize, minor: usize, patch: usize) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub fn major(&self) -> usize {
        self.major
    }

    pub fn minor(&self) -> usize {
        self.minor
    }

    pub fn patch(&self) -> usize {
        self.patch
    }

    pub fn is_compatible(&self, other: &Self) -> bool {
        self.major == other.major && self.minor == other.minor
    }
}

impl std::fmt::Display for IntuicioVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl std::fmt::Debug for IntuicioVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntuicioVersion")
            .field("major", &self.major)
            .field("minor", &self.minor)
            .field("patch", &self.patch)
            .finish()
    }
}

#[macro_export]
macro_rules! crate_version {
    () => {{
        let major = env!("CARGO_PKG_VERSION_MAJOR", "0")
            .parse::<usize>()
            .unwrap();
        let minor = env!("CARGO_PKG_VERSION_MINOR", "0")
            .parse::<usize>()
            .unwrap();
        let patch = env!("CARGO_PKG_VERSION_PATCH", "0")
            .parse::<usize>()
            .unwrap();
        $crate::IntuicioVersion::new(major, minor, patch)
    }};
}

pub fn core_version() -> IntuicioVersion {
    crate_version!()
}

struct IsSend<'a, T> {
    is_send: &'a Cell<bool>,
    _marker: PhantomData<T>,
}

impl<T> Clone for IsSend<'_, T> {
    fn clone(&self) -> Self {
        self.is_send.set(false);
        IsSend {
            is_send: self.is_send,
            _marker: PhantomData,
        }
    }
}

impl<T: Send> Copy for IsSend<'_, T> {}

pub fn is_send<T>() -> bool {
    let is_send = Cell::new(true);
    let _ = [IsSend::<T> {
        is_send: &is_send,
        _marker: PhantomData,
    }]
    .clone();
    is_send.get()
}

struct IsSync<'a, T> {
    is_sync: &'a Cell<bool>,
    _marker: PhantomData<T>,
}

impl<T> Clone for IsSync<'_, T> {
    fn clone(&self) -> Self {
        self.is_sync.set(false);
        IsSync {
            is_sync: self.is_sync,
            _marker: PhantomData,
        }
    }
}

impl<T: Sync> Copy for IsSync<'_, T> {}

pub fn is_sync<T>() -> bool {
    let is_sync = Cell::new(true);
    let _ = [IsSync::<T> {
        is_sync: &is_sync,
        _marker: PhantomData,
    }]
    .clone();
    is_sync.get()
}

#[cfg(test)]
mod tests {
    use crate::Visibility;

    #[test]
    fn test_visibility() {
        assert_eq!(Visibility::Private.is_visible(Visibility::Private), true);
        assert_eq!(Visibility::Private.is_visible(Visibility::Module), false);
        assert_eq!(Visibility::Private.is_visible(Visibility::Public), false);
        assert_eq!(Visibility::Module.is_visible(Visibility::Private), true);
        assert_eq!(Visibility::Module.is_visible(Visibility::Module), true);
        assert_eq!(Visibility::Module.is_visible(Visibility::Public), false);
        assert_eq!(Visibility::Public.is_visible(Visibility::Private), true);
        assert_eq!(Visibility::Public.is_visible(Visibility::Module), true);
        assert_eq!(Visibility::Public.is_visible(Visibility::Public), true);
    }
}
