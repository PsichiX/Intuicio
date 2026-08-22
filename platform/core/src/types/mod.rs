#![allow(unpredictable_function_pointer_comparisons)]

//! Runtime descriptions of structs and enums.
//!
//! A [`Type`] is what the platform knows about a data type at runtime: its
//! name, its memory layout, where its fields sit, and how to create and
//! destroy a value of it. Scripts use that to build and take apart values of
//! types they were never compiled against.
//!
//! # Native and runtime types
//!
//! A **native** type describes a real Rust type, so its layout comes from the
//! compiler and values of it can be moved to and from Rust directly. Build one
//! with [`struct_type::NativeStructBuilder`] or
//! [`enum_type::NativeEnumBuilder`], usually through the derive macros.
//!
//! A **runtime** type is invented by a script and has no Rust counterpart. Its
//! layout is computed from its fields when it is built, and its values are
//! handled field by field through [`crate::object::Object`]. Build one with
//! [`struct_type::RuntimeStructBuilder`] or [`enum_type::RuntimeEnumBuilder`].
//!
//! # Finding types
//!
//! [`TypeQuery`] filters types in a registry. It can match on name, module,
//! Rust type identity, visibility, metadata, and even on the shape of the
//! fields, which lets a script ask for structural rather than nominal matches.
pub mod enum_type;
pub mod struct_type;

use crate::{
    Visibility,
    meta::Meta,
    types::{
        enum_type::{Enum, EnumVariant},
        struct_type::{Struct, StructField},
    },
};
use intuicio_data::{Destructor, Finalizer, type_hash::TypeHash};
use rustc_hash::FxHasher;
use std::{
    alloc::Layout,
    borrow::Cow,
    hash::{Hash, Hasher},
    sync::Arc,
};

/// Shared type description, as a registry holds it.
pub type TypeHandle = Arc<Type>;
/// Predicate over metadata, used inside queries.
pub type MetaQuery = fn(&Meta) -> bool;

/// A struct or an enum, described at runtime.
///
/// Most methods forward to whichever kind is inside, so code that does not
/// care can work on [`Type`] alone.
#[derive(Debug, PartialEq)]
pub enum Type {
    /// A struct type.
    Struct(Struct),
    /// An enum type.
    Enum(Enum),
}

impl Type {
    /// Returns `true` for a struct.
    pub fn is_struct(&self) -> bool {
        matches!(self, Self::Struct(_))
    }

    /// Returns `true` for an enum.
    pub fn is_enum(&self) -> bool {
        matches!(self, Self::Enum(_))
    }

    /// Returns the struct, or [`None`] for an enum.
    pub fn as_struct(&self) -> Option<&Struct> {
        if let Self::Struct(value) = self {
            Some(value)
        } else {
            None
        }
    }

    /// Returns the enum, or [`None`] for a struct.
    pub fn as_enum(&self) -> Option<&Enum> {
        if let Self::Enum(value) = self {
            Some(value)
        } else {
            None
        }
    }

    /// Returns the metadata attached to this type.
    pub fn meta(&self) -> Option<&Meta> {
        match self {
            Self::Struct(value) => value.meta.as_ref(),
            Self::Enum(value) => value.meta.as_ref(),
        }
    }

    /// Returns the name this type is registered under.
    pub fn name(&self) -> &str {
        match self {
            Self::Struct(value) => &value.name,
            Self::Enum(value) => &value.name,
        }
    }

    /// Returns the module this type belongs to.
    pub fn module_name(&self) -> Option<&str> {
        match self {
            Self::Struct(value) => value.module_name.as_deref(),
            Self::Enum(value) => value.module_name.as_deref(),
        }
    }

    /// Returns how widely this type is visible.
    pub fn visibility(&self) -> Visibility {
        match self {
            Self::Struct(value) => value.visibility,
            Self::Enum(value) => value.visibility,
        }
    }

    /// Returns `true` for a type invented by a script, with no Rust counterpart.
    pub fn is_runtime(&self) -> bool {
        match self {
            Self::Struct(value) => value.is_runtime(),
            Self::Enum(value) => value.is_runtime(),
        }
    }

    /// Returns `true` for a type that describes a real Rust type.
    pub fn is_native(&self) -> bool {
        match self {
            Self::Struct(value) => value.is_native(),
            Self::Enum(value) => value.is_native(),
        }
    }

    /// Returns `true` when values of this type may move between threads.
    pub fn is_send(&self) -> bool {
        match self {
            Self::Struct(value) => value.is_send(),
            Self::Enum(value) => value.is_send(),
        }
    }

    /// Returns `true` when values of this type may be shared between threads.
    pub fn is_sync(&self) -> bool {
        match self {
            Self::Struct(value) => value.is_sync(),
            Self::Enum(value) => value.is_sync(),
        }
    }

    /// Returns `true` when values of this type may be duplicated by copying
    /// their bytes.
    pub fn is_copy(&self) -> bool {
        match self {
            Self::Struct(value) => value.is_copy(),
            Self::Enum(value) => value.is_copy(),
        }
    }

    /// Returns `true` when a default value of this type can be created.
    pub fn can_initialize(&self) -> bool {
        match self {
            Self::Struct(value) => value.can_initialize(),
            Self::Enum(value) => value.can_initialize(),
        }
    }

    /// Returns the runtime identity of this type.
    pub fn type_hash(&self) -> TypeHash {
        match self {
            Self::Struct(value) => value.type_hash(),
            Self::Enum(value) => value.type_hash(),
        }
    }

    /// Returns the full Rust type name, which for a runtime type is the name of
    /// the placeholder [`crate::object::RuntimeObject`].
    pub fn type_name(&self) -> &str {
        match self {
            Self::Struct(value) => value.type_name(),
            Self::Enum(value) => value.type_name(),
        }
    }

    /// Returns the memory layout of a value of this type.
    pub fn layout(&self) -> &Layout {
        match self {
            Self::Struct(value) => value.layout(),
            Self::Enum(value) => value.layout(),
        }
    }

    /// Returns the fields, or [`None`] for an enum.
    pub fn struct_fields(&self) -> Option<&[StructField]> {
        if let Self::Struct(value) = self {
            Some(value.fields())
        } else {
            None
        }
    }

    /// Returns the variants, or [`None`] for a struct.
    pub fn enum_variants(&self) -> Option<&[EnumVariant]> {
        if let Self::Enum(value) = self {
            Some(value.variants())
        } else {
            None
        }
    }

    /// Returns `true` when both types have the same layout and shape, whatever
    /// they are called.
    pub fn is_compatible(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Struct(a), Self::Struct(b)) => a.is_compatible(b),
            (Self::Enum(a), Self::Enum(b)) => a.is_compatible(b),
            _ => false,
        }
    }

    /// Iterates the fields a query matches.
    pub fn find_struct_fields<'a>(
        &'a self,
        query: StructFieldQuery<'a>,
    ) -> Option<impl Iterator<Item = &'a StructField> + 'a> {
        if let Self::Struct(value) = self {
            Some(value.find_fields(query))
        } else {
            None
        }
    }

    /// Returns the first field a query matches.
    pub fn find_struct_field<'a>(&'a self, query: StructFieldQuery<'a>) -> Option<&'a StructField> {
        if let Self::Struct(value) = self {
            value.find_field(query)
        } else {
            None
        }
    }

    /// Iterates the variants a query matches.
    pub fn find_enum_variants<'a>(
        &'a self,
        query: EnumVariantQuery<'a>,
    ) -> Option<impl Iterator<Item = &'a EnumVariant> + 'a> {
        if let Self::Enum(value) = self {
            Some(value.find_variants(query))
        } else {
            None
        }
    }

    /// Returns the first variant a query matches.
    pub fn find_enum_variant<'a>(&'a self, query: EnumVariantQuery<'a>) -> Option<&'a EnumVariant> {
        if let Self::Enum(value) = self {
            value.find_variant(query)
        } else {
            None
        }
    }

    /// Duplicates a value by copying its bytes, when the type allows it.
    ///
    /// Returns `false` without copying for types that are not plain data.
    ///
    /// # Safety
    ///
    /// `from` must point at an initialized value of this type and `to` at
    /// writable memory of its layout. The two must not overlap, which is
    /// checked, and `to` must not already hold a value, which is not.
    pub unsafe fn try_copy(&self, from: *const u8, to: *mut u8) -> bool {
        match self {
            Self::Struct(value) => unsafe { value.try_copy(from, to) },
            Self::Enum(value) => unsafe { value.try_copy(from, to) },
        }
    }

    /// Finds the variant a Rust enum value currently holds.
    ///
    /// # Safety
    ///
    /// `value` must be a `repr(u8)` enum that this type describes, otherwise
    /// the byte read as a discriminant means nothing.
    pub unsafe fn find_enum_variant_by_value<T: 'static>(&self, value: &T) -> Option<&EnumVariant> {
        if let Self::Enum(enum_type) = self {
            unsafe { enum_type.find_variant_by_value(value) }
        } else {
            None
        }
    }

    /// Writes a default value into already allocated memory.
    ///
    /// Returns `false` when the type has no way to create one.
    ///
    /// # Safety
    ///
    /// `pointer` must be writable memory of this type's layout, not already
    /// holding a value.
    pub unsafe fn initialize(&self, pointer: *mut ()) -> bool {
        match self {
            Self::Struct(value) => unsafe { value.initialize(pointer) },
            Self::Enum(value) => unsafe { value.initialize(pointer) },
        }
    }

    /// Drops the value at `pointer` in place.
    ///
    /// **A runtime type is taken apart field by field.** It has no Rust
    /// destructor to call: its fields are managed boxes that each own an
    /// allocation, and this walk is the only thing that frees them. A native
    /// type is dropped by the destructor the Rust compiler wrote for it.
    ///
    /// The walk recurses through [`Type::finalize`] on each field's own type,
    /// so a runtime type holding another runtime type is taken apart all the
    /// way down.
    ///
    /// # Safety
    ///
    /// `pointer` must point at an initialized value of this type that nothing
    /// reads afterwards.
    pub unsafe fn finalize(&self, pointer: *mut ()) {
        if !self.is_runtime() {
            match self {
                Self::Struct(value) => unsafe { value.finalize(pointer) },
                Self::Enum(value) => unsafe { value.finalize(pointer) },
            }
            return;
        }
        let memory = pointer.cast::<u8>();
        match self {
            Self::Struct(value) => {
                for field in value.fields() {
                    unsafe {
                        field
                            .type_handle()
                            .finalize(memory.add(field.address_offset()).cast::<()>())
                    };
                }
            }
            Self::Enum(value) => {
                let discriminant = unsafe { memory.read() };
                if let Some(variant) = value.find_variant_by_discriminant(discriminant) {
                    for field in &variant.fields {
                        unsafe {
                            field
                                .type_handle()
                                .finalize(memory.add(field.address_offset()).cast::<()>())
                        };
                    }
                }
            }
        }
    }

    /// Returns the raw initializer function, or [`None`] when there is none.
    ///
    /// # Safety
    ///
    /// Same conditions as [`Type::initialize`] apply to every call of it.
    pub unsafe fn initializer(&self) -> Option<unsafe fn(*mut ())> {
        match self {
            Self::Struct(value) => unsafe { value.initializer() },
            Self::Enum(value) => unsafe { value.initializer() },
        }
    }

    /// How a value of this type is destroyed, in a form that can be kept next to
    /// the value.
    ///
    /// **This is what an owning box should ask for.** A native type answers with
    /// its Rust destructor, a plain function pointer. A runtime type answers
    /// with a handle to itself, because its destructor is the field walk in
    /// [`Type::finalize`], and a function pointer cannot carry the field list.
    /// The returned [`Finalizer`] keeps the type alive for as long as a value of
    /// it exists.
    ///
    /// Takes `&Arc<Self>` rather than `&self`, because the runtime case hands
    /// out a share of the handle.
    pub fn finalizer(self: &Arc<Self>) -> Finalizer {
        if self.is_runtime() {
            return Finalizer::Runtime(self.clone() as Arc<dyn Destructor>);
        }
        // Safety: reading the pointer out of a native type is not the unsafe
        // part - calling it is, and that is `Finalizer::finalize`, which is
        // itself unsafe.
        let function = match &**self {
            Self::Struct(value) => unsafe { value.finalizer() },
            Self::Enum(value) => unsafe { value.finalizer() },
        };
        Finalizer::Native(function)
    }

    /// Wraps this type in a shared handle.
    pub fn into_handle(self) -> TypeHandle {
        self.into()
    }
}

/// A runtime type destroys its values by walking its own fields.
///
/// This is why a [`Finalizer`] can hold an object instead of a bare function
/// pointer. The walk needs the field list, and a function pointer cannot carry
/// one. Only a runtime type gets this. A native type answers
/// [`Type::finalizer`] with its Rust destructor.
impl Destructor for Type {
    unsafe fn destroy(&self, pointer: *mut ()) {
        unsafe { self.finalize(pointer) };
    }
}

impl From<Struct> for Type {
    fn from(value: Struct) -> Self {
        Self::Struct(value)
    }
}

impl From<Enum> for Type {
    fn from(value: Enum) -> Self {
        Self::Enum(value)
    }
}

/// Search filter for a struct field. An empty filter matches anything.
#[derive(Debug, Default, Clone, PartialEq, Hash)]
pub struct StructFieldQuery<'a> {
    /// Required field name.
    pub name: Option<Cow<'a, str>>,
    /// Filter on the field type.
    pub type_query: Option<TypeQuery<'a>>,
    /// Required visibility.
    pub visibility: Option<Visibility>,
    /// Predicate the field metadata must satisfy.
    pub meta: Option<MetaQuery>,
}

impl StructFieldQuery<'_> {
    /// Returns `true` when `field` satisfies every set filter.
    pub fn is_valid(&self, field: &StructField) -> bool {
        self.name
            .as_ref()
            .map(|name| name.as_ref() == field.name)
            .unwrap_or(true)
            && self
                .type_query
                .as_ref()
                .map(|query| query.is_valid(&field.type_handle))
                .unwrap_or(true)
            && self
                .visibility
                .map(|visibility| field.visibility.is_visible(visibility))
                .unwrap_or(true)
            && self
                .meta
                .as_ref()
                .map(|query| field.meta.as_ref().map(query).unwrap_or(false))
                .unwrap_or(true)
    }

    /// Copies borrowed names into owned ones.
    pub fn to_static(&self) -> StructFieldQuery<'static> {
        StructFieldQuery {
            name: self
                .name
                .as_ref()
                .map(|name| name.as_ref().to_owned().into()),
            type_query: self.type_query.as_ref().map(|query| query.to_static()),
            visibility: self.visibility,
            meta: self.meta,
        }
    }
}

/// Search filter for an enum variant.
///
/// Field filters are matched pairwise from the front, so listing fewer than
/// the variant has still matches.
#[derive(Debug, Default, Clone, PartialEq, Hash)]
pub struct EnumVariantQuery<'a> {
    /// Required variant name.
    pub name: Option<Cow<'a, str>>,
    /// Filters matched against the leading fields of the variant.
    pub fields: Cow<'a, [StructFieldQuery<'a>]>,
    /// Predicate the variant metadata must satisfy.
    pub meta: Option<MetaQuery>,
}

impl EnumVariantQuery<'_> {
    /// Returns `true` when `variant` satisfies every set filter.
    pub fn is_valid(&self, variant: &EnumVariant) -> bool {
        self.name
            .as_ref()
            .map(|name| name.as_ref() == variant.name)
            .unwrap_or(true)
            && self
                .fields
                .iter()
                .zip(variant.fields.iter())
                .all(|(query, field)| query.is_valid(field))
            && self
                .meta
                .as_ref()
                .map(|query| variant.meta.as_ref().map(query).unwrap_or(false))
                .unwrap_or(true)
    }

    /// Copies borrowed names into owned ones.
    pub fn to_static(&self) -> EnumVariantQuery<'static> {
        EnumVariantQuery {
            name: self
                .name
                .as_ref()
                .map(|name| name.as_ref().to_owned().into()),
            fields: self
                .fields
                .as_ref()
                .iter()
                .map(|query| query.to_static())
                .collect(),
            meta: self.meta,
        }
    }
}

/// Filter on the shape of a type, for matching structurally rather than by
/// name.
#[derive(Debug, Default, Clone, PartialEq, Hash)]
pub enum TypeKindQuery<'a> {
    #[default]
    /// Matches any type. The default.
    None,
    /// Matches a struct whose leading fields satisfy the filters.
    Struct {
        fields: Cow<'a, [StructFieldQuery<'a>]>,
    },
    /// Matches an enum whose leading variants satisfy the filters.
    Enum {
        variants: Cow<'a, [EnumVariantQuery<'a>]>,
    },
}

impl TypeKindQuery<'_> {
    /// Returns `true` when `type_` has the required shape.
    pub fn is_valid(&self, type_: &Type) -> bool {
        match (self, type_) {
            (Self::None, _) => true,
            (Self::Struct { fields }, Type::Struct(type_)) => fields
                .iter()
                .zip(type_.fields().iter())
                .all(|(query, field)| query.is_valid(field)),
            (Self::Struct { .. }, _) => false,
            (Self::Enum { variants }, Type::Enum(type_)) => variants
                .iter()
                .zip(type_.variants().iter())
                .all(|(query, variant)| query.is_valid(variant)),
            (Self::Enum { .. }, _) => false,
        }
    }

    /// Copies borrowed names into owned ones.
    pub fn to_static(&self) -> TypeKindQuery<'static> {
        match self {
            Self::None => TypeKindQuery::None,
            Self::Struct { fields } => TypeKindQuery::Struct {
                fields: fields
                    .as_ref()
                    .iter()
                    .map(|query| query.to_static())
                    .collect(),
            },
            Self::Enum { variants } => TypeKindQuery::Enum {
                variants: variants
                    .as_ref()
                    .iter()
                    .map(|query| query.to_static())
                    .collect(),
            },
        }
    }
}

/// Search filter for types in a [`crate::registry::Registry`].
///
/// Every field is optional and an empty query matches everything.
///
/// ```
/// # use intuicio_core::{registry::Registry, types::TypeQuery};
/// let registry = Registry::default().with_basic_types();
/// let found = registry.find_type(TypeQuery::of::<i32>()).unwrap();
/// assert_eq!(found.type_name(), "i32");
/// ```
#[derive(Debug, Default, Clone, PartialEq, Hash)]
pub struct TypeQuery<'a> {
    /// Required registered name.
    pub name: Option<Cow<'a, str>>,
    /// Required module.
    pub module_name: Option<Cow<'a, str>>,
    /// Required runtime type identity.
    pub type_hash: Option<TypeHash>,
    /// Required full Rust type name.
    pub type_name: Option<Cow<'a, str>>,
    /// Required visibility.
    pub visibility: Option<Visibility>,
    /// Filter on the shape of the type.
    pub kind: TypeKindQuery<'a>,
    /// Predicate the type metadata must satisfy.
    pub meta: Option<MetaQuery>,
}

impl<'a> TypeQuery<'a> {
    /// Matches by full Rust type name.
    ///
    /// Unlike [`TypeQuery::of`], this also matches a type registered from
    /// another build of the same crate.
    pub fn of_type_name<T: 'static>() -> Self {
        Self {
            type_name: Some(std::any::type_name::<T>().into()),
            ..Default::default()
        }
    }

    /// Matches the Rust type `T` by its identity.
    pub fn of<T: 'static>() -> Self {
        Self {
            type_hash: Some(TypeHash::of::<T>()),
            ..Default::default()
        }
    }

    /// Matches the Rust type `T` registered under a given name.
    pub fn of_named<T: 'static>(name: &'a str) -> Self {
        Self {
            name: Some(name.into()),
            type_hash: Some(TypeHash::of::<T>()),
            ..Default::default()
        }
    }

    /// Returns `true` when `type_` satisfies every set field.
    pub fn is_valid(&self, type_: &Type) -> bool {
        self.name
            .as_ref()
            .map(|name| name.as_ref() == type_.name())
            .unwrap_or(true)
            && self
                .module_name
                .as_ref()
                .map(|name| {
                    type_
                        .module_name()
                        .as_ref()
                        .map(|module_name| name.as_ref() == *module_name)
                        .unwrap_or(false)
                })
                .unwrap_or(true)
            && self
                .type_hash
                .map(|type_hash| type_.type_hash() == type_hash)
                .unwrap_or(true)
            && self
                .type_name
                .as_ref()
                .map(|type_name| type_.type_name() == type_name.as_ref())
                .unwrap_or(true)
            && self
                .visibility
                .map(|visibility| type_.visibility().is_visible(visibility))
                .unwrap_or(true)
            && self.kind.is_valid(type_)
            && self
                .meta
                .as_ref()
                .map(|query| {
                    type_
                        .meta()
                        .as_ref()
                        .map(|meta| query(meta))
                        .unwrap_or(false)
                })
                .unwrap_or(true)
    }

    /// Hashes the query, which is the key the registry caches results under.
    pub fn as_hash(&self) -> u64 {
        let mut hasher = FxHasher::default();
        self.hash(&mut hasher);
        hasher.finish()
    }

    /// Copies borrowed names into owned ones, so the query can outlive them.
    pub fn to_static(&self) -> TypeQuery<'static> {
        TypeQuery {
            name: self
                .name
                .as_ref()
                .map(|name| name.as_ref().to_owned().into()),
            module_name: self
                .module_name
                .as_ref()
                .map(|name| name.as_ref().to_owned().into()),
            type_hash: self.type_hash,
            type_name: self
                .type_name
                .as_ref()
                .map(|name| name.as_ref().to_owned().into()),
            visibility: self.visibility,
            kind: self.kind.to_static(),
            meta: self.meta,
        }
    }
}
