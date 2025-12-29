pub mod context;
pub mod function;
pub mod host;
pub mod meta;
pub mod object;
pub mod registry;
pub mod script;
pub mod transformer;
pub mod types;
pub mod utils;

pub use memoffset::offset_of as __internal__offset_of__;

/// Assumes `repr(u8)` enums only.
#[macro_export]
macro_rules! __internal__offset_of_enum__ {
    ($type:tt :: $variant:ident [ $( $field:ident ),* ] => $used_field:ident => $discriminant:literal) => {{
        let mut data = std::mem::MaybeUninit::<$type>::uninit();
        let ptr = data.as_mut_ptr().cast::<u8>();
        #[allow(clippy::macro_metavars_in_unsafe)]
        unsafe {
            ptr.write($discriminant);
            #[allow(unused)]
            match data.assume_init_ref() {
                $type::$variant( $( $field ),* ) => {
                    ($used_field as *const _ as *const u8).offset_from(ptr) as usize
                }
                _ => unreachable!(),
            }
        }
    }};
    ($type:tt :: $variant:ident ( $index:tt ) => $discriminant:literal) => {{
        let mut data = std::mem::MaybeUninit::<$type>::uninit();
        let ptr = data.as_mut_ptr().cast::<u8>();
        #[allow(clippy::macro_metavars_in_unsafe)]
        unsafe {
            ptr.write($discriminant);
            #[allow(unused)]
            match data.assume_init_ref() {
                $type::$variant {
                    $index: __value__, ..
                } => (__value__ as *const _ as *const u8).offset_from(ptr) as usize,
                _ => unreachable!(),
            }
        }
    }};
    ($type:tt :: $variant:ident { $field:ident } => $discriminant:literal) => {{
        let mut data = std::mem::MaybeUninit::<$type>::uninit();
        let ptr = data.as_mut_ptr().cast::<u8>();
        #[allow(clippy::macro_metavars_in_unsafe)]
        unsafe {
            ptr.write($discriminant);
            #[allow(unused)]
            match data.assume_init_ref() {
                $type::$variant { $field, .. } => {
                    ($field as *const _ as *const u8).offset_from(ptr) as usize
                }
                _ => unreachable!(),
            }
        }
    }};
}

use crate::{
    registry::Registry,
    types::{enum_type::Enum, struct_type::Struct},
};
use serde::{Deserialize, Serialize};

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

pub trait IntuicioEnum {
    fn define_enum(registry: &Registry) -> Enum;
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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
        let major = option_env!("CARGO_PKG_VERSION_MAJOR")
            .unwrap_or("0")
            .parse::<usize>()
            .unwrap();
        let minor = option_env!("CARGO_PKG_VERSION_MINOR")
            .unwrap_or("0")
            .parse::<usize>()
            .unwrap();
        let patch = option_env!("CARGO_PKG_VERSION_PATCH")
            .unwrap_or("0")
            .parse::<usize>()
            .unwrap();
        $crate::IntuicioVersion::new(major, minor, patch)
    }};
}

pub fn core_version() -> IntuicioVersion {
    crate_version!()
}

#[cfg(test)]
mod tests {
    use crate::Visibility;

    #[test]
    fn test_visibility() {
        assert!(Visibility::Private.is_visible(Visibility::Private));
        assert!(!Visibility::Private.is_visible(Visibility::Module));
        assert!(!Visibility::Private.is_visible(Visibility::Public));
        assert!(Visibility::Module.is_visible(Visibility::Private));
        assert!(Visibility::Module.is_visible(Visibility::Module));
        assert!(!Visibility::Module.is_visible(Visibility::Public));
        assert!(Visibility::Public.is_visible(Visibility::Private));
        assert!(Visibility::Public.is_visible(Visibility::Module));
        assert!(Visibility::Public.is_visible(Visibility::Public));
    }

    #[test]
    fn test_offset_of_enum() {
        #[allow(dead_code)]
        #[repr(u8)]
        enum Foo {
            A,
            B(usize),
            C(u8, u16),
            D { a: u32, b: u64 },
        }

        assert_eq!(__internal__offset_of_enum__!(Foo::B[v] => v => 1), 8);
        assert_eq!(__internal__offset_of_enum__!(Foo::B(0) => 1), 8);
        assert_eq!(__internal__offset_of_enum__!(Foo::C[a, b] => a => 2), 1);
        assert_eq!(__internal__offset_of_enum__!(Foo::C[a, b] => b => 2), 2);
        assert_eq!(__internal__offset_of_enum__!(Foo::C(0) => 2), 1);
        assert_eq!(__internal__offset_of_enum__!(Foo::C(1) => 2), 2);
        assert_eq!(__internal__offset_of_enum__!(Foo::D { a } => 3), 4);
        assert_eq!(__internal__offset_of_enum__!(Foo::D { b } => 3), 8);
    }
}
