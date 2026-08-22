//! The building blocks every Intuicio scripting solution is made of.
//!
//! Intuicio is not a scripting language. It is a set of pieces to build one
//! from. This crate holds the pieces that every part of such a build agrees
//! on.
//!
//! # The pipeline
//!
//! ```text
//! source (text, node graph, anything)
//!   |  frontend
//!   v
//! script data  ->  backend  ->  Function in a Registry
//!                                     ^
//!                                     |  Host calls it in a Context
//! ```
//!
//! - A **frontend** turns some input into [script data](script). It can be a
//!   parser, a node graph editor, or anything else that produces the same
//!   data.
//! - A **backend** turns script data into a callable [`function::Function`]. A
//!   virtual machine is the obvious one, a transpiler to Rust is another.
//! - The **host** is the native side: Rust functions and types registered in a
//!   [`registry::Registry`], callable from scripts and calling back into them.
//!
//! # Why script and native calls look the same
//!
//! Every function, native or scripted, has the same shape:
//! `fn(&mut Context, &Registry)`. It pops its arguments off the context stack
//! and pushes its results back. Neither side can tell which kind it is calling,
//! so a program can mix frontends and backends freely.
//!
//! # Where to start
//!
//! - [`registry`] - where every type and function is declared.
//! - [`context`] - the stack and registers a call runs on.
//! - [`host`] - the convenient way to call into all of it.
//! - [`script`] - the data a frontend produces and a backend consumes.
//! - [`types`] - runtime descriptions of structs and enums.
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

/// Re-export used by the `define_native_struct!` macro to find field offsets.
pub use memoffset::offset_of as __internal__offset_of__;

/// Returns the byte offset of a field inside one variant of a `repr(u8)` enum.
///
/// Used by `define_native_enum!`. Only sound for `repr(u8)` enums, whose
/// discriminant sits at offset zero.
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

/// A search filter for a field that the target may or may not have.
///
/// `Option` cannot say "must be absent". A query built from `Option` therefore
/// cannot ask for a function with no type handle, which is how a free function
/// is told apart from a method. A lookup by name would then also match every
/// method of that name.
///
/// Use this where the target field is an `Option`, and plain `Option` where it
/// is not.
///
/// ```
/// # use intuicio_core::Filter;
/// let ignore = Filter::<u32>::default();
/// assert!(ignore.is_valid(None::<&u32>, |_, _| true));
/// assert!(ignore.is_valid(Some(&1), |_, _| true));
///
/// assert!(Filter::<u32>::Absent.is_valid(None::<&u32>, |_, _| true));
/// assert!(!Filter::<u32>::Absent.is_valid(Some(&1), |_, _| true));
///
/// assert!(Filter::Matching(1).is_valid(Some(&1), |query, value| query == value));
/// assert!(!Filter::Matching(1).is_valid(None, |query, value| query == value));
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Filter<T> {
    /// Matches whether the target has one or not.
    #[default]
    Ignore,
    /// Matches only when the target has none.
    Absent,
    /// Matches only when the target has one that satisfies this.
    Matching(T),
}

impl<T> Filter<T> {
    /// Applies the filter to a target field, using `matches` to compare.
    pub fn is_valid<U>(&self, value: Option<&U>, matches: impl FnOnce(&T, &U) -> bool) -> bool {
        match self {
            Self::Ignore => true,
            Self::Absent => value.is_none(),
            Self::Matching(query) => value.map(|value| matches(query, value)).unwrap_or(false),
        }
    }

    /// Whether this filter says nothing, which is the default.
    ///
    /// Useful as a serde `skip_serializing_if`, so a query only writes the
    /// filters it actually sets.
    pub fn is_ignore(&self) -> bool {
        matches!(self, Self::Ignore)
    }

    /// Whether this filter demands the target has none.
    pub fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }

    /// Returns what is being matched against, if anything.
    pub fn matching(&self) -> Option<&T> {
        match self {
            Self::Matching(query) => Some(query),
            _ => None,
        }
    }

    /// Rebuilds the filter with a mapped payload, keeping the same setting.
    ///
    /// The named lifetime lets the result borrow from `self`, so an owned filter
    /// can become a borrowing one.
    pub fn map<'a, U>(&'a self, f: impl FnOnce(&'a T) -> U) -> Filter<U> {
        match self {
            Self::Ignore => Filter::Ignore,
            Self::Absent => Filter::Absent,
            Self::Matching(query) => Filter::Matching(f(query)),
        }
    }
}

impl<T> From<T> for Filter<T> {
    fn from(value: T) -> Self {
        Self::Matching(value)
    }
}

/// `None` becomes [`Filter::Ignore`], not [`Filter::Absent`], because that is
/// what `None` meant when these fields were `Option`.
impl<T> From<Option<T>> for Filter<T> {
    fn from(value: Option<T>) -> Self {
        match value {
            Some(value) => Self::Matching(value),
            None => Self::Ignore,
        }
    }
}

/// How far a type, function or field can be seen from.
///
/// Ordered from narrowest to widest, so a wider visibility satisfies a
/// narrower requirement. See [`Visibility::is_visible`].
#[derive(
    Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum Visibility {
    /// Visible only inside the type that declares it.
    Private,
    /// Visible inside the declaring module.
    Module,
    #[default]
    /// Visible everywhere. The default.
    Public,
}

impl Visibility {
    /// Returns `true` when this visibility is at least as wide as `scope`.
    ///
    /// ```
    /// # use intuicio_core::Visibility;
    /// assert!(Visibility::Public.is_visible(Visibility::Module));
    /// assert!(!Visibility::Private.is_visible(Visibility::Module));
    /// ```
    pub fn is_visible(self, scope: Self) -> bool {
        self >= scope
    }

    /// Returns `true` for [`Visibility::Public`].
    pub fn is_public(&self) -> bool {
        *self == Visibility::Public
    }

    /// Returns `true` for [`Visibility::Module`].
    pub fn is_module(&self) -> bool {
        *self == Visibility::Module
    }

    /// Returns `true` for [`Visibility::Private`].
    pub fn is_private(&self) -> bool {
        *self == Visibility::Private
    }
}

/// A Rust struct that can describe itself to a registry.
///
/// Implemented by the `IntuicioStruct` derive macro.
pub trait IntuicioStruct {
    /// Builds the runtime description of this struct.
    ///
    /// `registry` is needed to look up the types of the fields, so every field
    /// type has to be registered first.
    fn define_struct(registry: &Registry) -> Struct;
}

/// A Rust enum that can describe itself to a registry.
///
/// Implemented by the `IntuicioEnum` derive macro.
pub trait IntuicioEnum {
    /// Builds the runtime description of this enum.
    ///
    /// `registry` is needed to look up the types of the variant fields, so every
    /// field type has to be registered first.
    fn define_enum(registry: &Registry) -> Enum;
}

/// Semantic version of a crate, used to check that plugins match their host.
///
/// `repr(C)`, because it crosses the plugin ABI boundary.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(C)]
pub struct IntuicioVersion {
    major: usize,
    minor: usize,
    patch: usize,
}

impl IntuicioVersion {
    /// Builds a version.
    pub fn new(major: usize, minor: usize, patch: usize) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Returns the major number.
    pub fn major(&self) -> usize {
        self.major
    }

    /// Returns the minor number.
    pub fn minor(&self) -> usize {
        self.minor
    }

    /// Returns the patch number.
    pub fn patch(&self) -> usize {
        self.patch
    }

    /// Returns `true` when major and minor match, ignoring the patch number.
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

/// Builds an [`IntuicioVersion`] from the `CARGO_PKG_VERSION_*` variables of
/// the calling crate.
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

/// Returns the version of this crate, for plugins to check against.
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
