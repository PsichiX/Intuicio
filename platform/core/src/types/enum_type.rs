#![allow(unpredictable_function_pointer_comparisons)]

//! Runtime description of an enum.
//!
//! [`Enum`] is the enum counterpart of [`crate::types::struct_type::Struct`],
//! built by [`NativeEnumBuilder`] for a real Rust enum or [`RuntimeEnumBuilder`]
//! for one a script invented.
//!
//! A value is one `u8` discriminant at offset zero, followed by the fields of
//! whichever variant it holds. So only `repr(u8)` Rust enums can be described.
//! No other representation says where the discriminant lives.
use crate::{
    Visibility,
    meta::Meta,
    object::RuntimeObject,
    types::{
        EnumVariantQuery, MetaQuery, StructFieldQuery, Type,
        struct_type::{StructField, qualified_name},
    },
};
use intuicio_data::{Finalize, Initialize, type_hash::TypeHash};
use rustc_hash::FxHasher;
use std::{
    alloc::Layout,
    borrow::Cow,
    hash::{Hash, Hasher},
};

/// Builds an [`Enum`] that no Rust type backs.
///
/// Each variant is laid out after the discriminant, and the whole enum takes
/// the size and alignment of its largest variant.
pub struct RuntimeEnumBuilder {
    meta: Option<Meta>,
    name: String,
    module_name: Option<String>,
    visibility: Visibility,
    type_hash: TypeHash,
    type_name: String,
    variants: Vec<EnumVariant>,
    defaut_variant: Option<u8>,
    layout: Layout,
    initializer: unsafe fn(*mut ()),
    finalizer: unsafe fn(*mut ()),
    is_runtime: bool,
}

impl RuntimeEnumBuilder {
    /// Starts an enum with no variants.
    ///
    /// The type hash is settled in [`build`](Self::build), not here, because it
    /// comes from the module and the name, and the module is set afterwards.
    pub fn new(name: impl ToString) -> Self {
        Self {
            meta: None,
            name: name.to_string(),
            module_name: None,
            visibility: Visibility::default(),
            type_hash: TypeHash::INVALID,
            type_name: std::any::type_name::<RuntimeObject>().to_owned(),
            variants: vec![],
            defaut_variant: None,
            layout: Layout::from_size_align(0, 1).unwrap(),
            initializer: RuntimeObject::initialize_raw,
            finalizer: RuntimeObject::finalize_raw,
            is_runtime: true,
        }
    }

    /// Attaches metadata.
    pub fn meta(mut self, meta: Meta) -> Self {
        self.meta = Some(meta);
        self
    }

    /// Attaches metadata when there is any.
    pub fn maybe_meta(mut self, meta: Option<Meta>) -> Self {
        self.meta = meta;
        self
    }

    /// Sets the owning module.
    pub fn module_name(mut self, module_name: impl ToString) -> Self {
        self.module_name = Some(module_name.to_string());
        self
    }

    /// Sets visibility.
    pub fn visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = visibility;
        self
    }

    /// Appends a variant, numbering it after the ones added so far.
    pub fn variant(mut self, mut variant: EnumVariant) -> Self {
        variant.discriminant = self
            .variants
            .last()
            .map(|variant| variant.discriminant + 1)
            .unwrap_or(0);
        self.variants.push(variant);
        self
    }

    /// Appends a variant with a discriminant you choose.
    pub fn variant_with_discriminant(mut self, mut variant: EnumVariant, discriminant: u8) -> Self {
        variant.discriminant = discriminant;
        self.variants.push(variant);
        self
    }

    /// Picks which variant a default value holds.
    ///
    /// Without one, a default value cannot be created.
    pub fn set_default_variant(mut self, discriminant: u8) -> Self {
        self.defaut_variant = Some(discriminant);
        self
    }

    /// Lays every variant out and produces the type.
    ///
    /// `Send`, `Sync` and `Copy` are inferred: the enum has each of them only
    /// when every field of every variant does.
    ///
    /// # Panics
    ///
    /// Panics when a variant cannot be laid out, which means its size
    /// overflowed.
    pub fn build(mut self) -> Enum {
        self.variants.sort_by_key(|a| a.discriminant);
        self.layout = Layout::new::<u8>();
        for variant in &mut self.variants {
            let mut layout = Layout::new::<u8>();
            for field in &mut variant.fields {
                let (new_layout, offset) = layout.extend(*field.type_handle.layout()).unwrap();
                layout = new_layout;
                field.offset = offset;
            }
            self.layout = Layout::from_size_align(
                self.layout.size().max(layout.size()),
                self.layout.align().max(layout.align()),
            )
            .unwrap();
        }
        let mut is_send = true;
        let mut is_sync = true;
        let mut is_copy = true;
        for variant in &mut self.variants {
            variant.fields.sort_by_key(|a| a.offset);
            is_send = is_send
                && variant
                    .fields
                    .iter()
                    .all(|field| field.type_handle.is_send());
            is_sync = is_sync
                && variant
                    .fields
                    .iter()
                    .all(|field| field.type_handle.is_sync());
            is_copy = is_copy
                && variant
                    .fields
                    .iter()
                    .all(|field| field.type_handle.is_copy());
        }
        let type_hash = if self.is_runtime {
            TypeHash::of_runtime(&qualified_name(self.module_name.as_deref(), &self.name))
        } else {
            self.type_hash
        };
        Enum {
            meta: self.meta,
            name: self.name,
            module_name: self.module_name,
            visibility: self.visibility,
            type_hash,
            type_name: self.type_name,
            variants: self.variants,
            default_variant: self.defaut_variant,
            layout: self.layout.pad_to_align(),
            initializer: Some(self.initializer),
            finalizer: self.finalizer,
            is_send,
            is_sync,
            is_copy,
            is_runtime: self.is_runtime,
        }
    }
}

impl From<Enum> for RuntimeEnumBuilder {
    fn from(value: Enum) -> Self {
        Self {
            meta: value.meta,
            name: value.name,
            module_name: value.module_name,
            visibility: value.visibility,
            type_hash: value.type_hash,
            type_name: value.type_name,
            variants: value.variants,
            defaut_variant: value.default_variant,
            layout: value.layout,
            initializer: value.initializer.unwrap_or(RuntimeObject::initialize_raw),
            finalizer: value.finalizer,
            is_runtime: value.is_runtime,
        }
    }
}

/// Builds an [`Enum`] describing a real Rust type.
///
/// The layout comes from the compiler, so discriminants and field offsets
/// have to be passed in rather than computed. The `define_native_enum!` macro
/// and the `IntuicioEnum` derive both do that for you.
///
/// Only `repr(u8)` enums can be described. See the [module docs](self).
#[derive(Debug)]
pub struct NativeEnumBuilder {
    meta: Option<Meta>,
    name: String,
    module_name: Option<String>,
    visibility: Visibility,
    type_hash: TypeHash,
    type_name: String,
    variants: Vec<EnumVariant>,
    defaut_variant: Option<u8>,
    layout: Layout,
    initializer: Option<unsafe fn(*mut ())>,
    finalizer: unsafe fn(*mut ()),
    is_send: bool,
    is_sync: bool,
    is_copy: bool,
    is_runtime: bool,
}

impl NativeEnumBuilder {
    /// Describes `T`, named after its full Rust type name.
    pub fn new<T: Initialize + Finalize + 'static>() -> Self {
        Self {
            meta: None,
            name: std::any::type_name::<T>().to_owned(),
            module_name: None,
            visibility: Visibility::default(),
            type_hash: TypeHash::of::<T>(),
            type_name: std::any::type_name::<T>().to_owned(),
            variants: vec![],
            defaut_variant: None,
            layout: Layout::new::<T>().pad_to_align(),
            initializer: Some(T::initialize_raw),
            finalizer: T::finalize_raw,
            is_send: false,
            is_sync: false,
            is_copy: false,
            is_runtime: false,
        }
    }

    /// Describes `T` under a name of your choosing.
    pub fn new_named<T: Initialize + Finalize + 'static>(name: impl ToString) -> Self {
        Self {
            meta: None,
            name: name.to_string(),
            module_name: None,
            visibility: Visibility::default(),
            type_hash: TypeHash::of::<T>(),
            type_name: std::any::type_name::<T>().to_owned(),
            variants: vec![],
            defaut_variant: None,
            layout: Layout::new::<T>().pad_to_align(),
            initializer: Some(T::initialize_raw),
            finalizer: T::finalize_raw,
            is_send: false,
            is_sync: false,
            is_copy: false,
            is_runtime: false,
        }
    }

    /// Describes `T` without a way to create a default value.
    pub fn new_uninitialized<T: Finalize + 'static>() -> Self {
        Self {
            meta: None,
            name: std::any::type_name::<T>().to_owned(),
            module_name: None,
            visibility: Visibility::default(),
            type_hash: TypeHash::of::<T>(),
            type_name: std::any::type_name::<T>().to_owned(),
            variants: vec![],
            defaut_variant: None,
            layout: Layout::new::<T>().pad_to_align(),
            initializer: None,
            finalizer: T::finalize_raw,
            is_send: false,
            is_sync: false,
            is_copy: false,
            is_runtime: false,
        }
    }

    /// [`NativeEnumBuilder::new_uninitialized`] under a name of your choosing.
    pub fn new_named_uninitialized<T: Finalize + 'static>(name: impl ToString) -> Self {
        Self {
            meta: None,
            name: name.to_string(),
            module_name: None,
            visibility: Visibility::default(),
            type_hash: TypeHash::of::<T>(),
            type_name: std::any::type_name::<T>().to_owned(),
            variants: vec![],
            defaut_variant: None,
            layout: Layout::new::<T>().pad_to_align(),
            initializer: None,
            finalizer: T::finalize_raw,
            is_send: false,
            is_sync: false,
            is_copy: false,
            is_runtime: false,
        }
    }

    /// Attaches metadata.
    pub fn meta(mut self, meta: Meta) -> Self {
        self.meta = Some(meta);
        self
    }

    /// Attaches metadata when there is any.
    pub fn maybe_meta(mut self, meta: Option<Meta>) -> Self {
        self.meta = meta;
        self
    }

    /// Sets the owning module.
    pub fn module_name(mut self, module_name: impl ToString) -> Self {
        self.module_name = Some(module_name.to_string());
        self
    }

    /// Sets visibility.
    pub fn visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = visibility;
        self
    }

    /// Declares a variant with the discriminant the Rust enum gives it.
    ///
    /// Field offsets inside the variant come from
    /// [`EnumVariant::with_field_with_offset`]. Get them from
    /// `__internal__offset_of_enum__!`, never by hand.
    pub fn variant(mut self, mut variant: EnumVariant, discriminant: u8) -> Self {
        self.is_send = self.is_send && variant.is_send();
        self.is_sync = self.is_sync && variant.is_sync();
        self.is_copy = self.is_copy && variant.is_copy();
        variant.discriminant = discriminant;
        self.variants.push(variant);
        self
    }

    /// Picks which variant a default value holds.
    pub fn set_default_variant(mut self, discriminant: u8) -> Self {
        self.defaut_variant = Some(discriminant);
        self
    }

    /// Declares that values of this type may move between threads.
    ///
    /// # Safety
    ///
    /// Nothing verifies the claim. Saying `true` for a type that is not `Send`
    /// lets scripts move it across threads and cause data races.
    pub unsafe fn override_send(mut self, mode: bool) -> Self {
        self.is_send = mode;
        self
    }

    /// Declares that values of this type may be shared between threads.
    ///
    /// # Safety
    ///
    /// Nothing verifies the claim. Saying `true` for a type that is not `Sync`
    /// lets scripts share it across threads and cause data races.
    pub unsafe fn override_sync(mut self, mode: bool) -> Self {
        self.is_sync = mode;
        self
    }

    /// Declares that values of this type may be duplicated by copying bytes.
    ///
    /// # Safety
    ///
    /// Nothing verifies the claim. Saying `true` for a type that owns a
    /// resource duplicates the owner and leads to a double free.
    pub unsafe fn override_copy(mut self, mode: bool) -> Self {
        self.is_copy = mode;
        self
    }

    /// Produces the type.
    pub fn build(mut self) -> Enum {
        self.variants.sort_by_key(|a| a.discriminant);
        for variant in &mut self.variants {
            variant.fields.sort_by_key(|a| a.offset);
        }
        Enum {
            meta: self.meta,
            name: self.name,
            module_name: self.module_name,
            visibility: self.visibility,
            type_hash: self.type_hash,
            type_name: self.type_name,
            variants: self.variants,
            default_variant: self.defaut_variant,
            layout: self.layout,
            initializer: self.initializer,
            finalizer: self.finalizer,
            is_send: self.is_send,
            is_sync: self.is_sync,
            is_copy: self.is_copy,
            is_runtime: self.is_runtime,
        }
    }
}

impl From<Enum> for NativeEnumBuilder {
    fn from(value: Enum) -> Self {
        Self {
            meta: value.meta,
            name: value.name,
            module_name: value.module_name,
            visibility: value.visibility,
            type_hash: value.type_hash,
            type_name: value.type_name,
            variants: value.variants,
            defaut_variant: value.default_variant,
            layout: value.layout,
            initializer: value.initializer,
            finalizer: value.finalizer,
            is_send: value.is_send,
            is_sync: value.is_sync,
            is_copy: value.is_copy,
            is_runtime: value.is_runtime,
        }
    }
}

/// One variant of an [`Enum`]: a name, a discriminant and fields.
#[derive(Debug, PartialEq)]
pub struct EnumVariant {
    /// Metadata attached to this variant.
    pub meta: Option<Meta>,
    /// Variant name.
    pub name: String,
    /// Fields this variant carries, ordered by offset once built.
    pub fields: Vec<StructField>,
    discriminant: u8,
}

impl EnumVariant {
    /// Builds an empty variant with discriminant zero, to be numbered by a
    /// builder.
    pub fn new(name: impl ToString) -> Self {
        Self {
            meta: None,
            name: name.to_string(),
            fields: vec![],
            discriminant: 0,
        }
    }

    /// Attaches metadata, builder style.
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = Some(meta);
        self
    }

    /// Appends a field whose offset a [`RuntimeEnumBuilder`] will decide.
    pub fn with_field(mut self, field: StructField) -> Self {
        self.fields.push(field);
        self
    }

    /// Appends a field at a known byte offset, for a native enum.
    pub fn with_field_with_offset(mut self, mut field: StructField, offset: usize) -> Self {
        field.offset = offset;
        self.fields.push(field);
        self
    }

    /// Returns the discriminant that selects this variant.
    pub fn discriminant(&self) -> u8 {
        self.discriminant
    }

    /// Returns `true` when every field may move between threads.
    pub fn is_send(&self) -> bool {
        self.fields.iter().all(|f| f.type_handle.is_send())
    }

    /// Returns `true` when every field may be shared between threads.
    pub fn is_sync(&self) -> bool {
        self.fields.iter().all(|f| f.type_handle.is_sync())
    }

    /// Returns `true` when every field may be duplicated by copying bytes.
    pub fn is_copy(&self) -> bool {
        self.fields.iter().all(|f| f.type_handle.is_copy())
    }

    /// Iterates the fields a query matches.
    pub fn find_fields<'a>(
        &'a self,
        query: StructFieldQuery<'a>,
    ) -> impl Iterator<Item = &'a StructField> + 'a {
        self.fields
            .iter()
            .filter(move |field| query.is_valid(field))
    }

    /// Returns the first field a query matches.
    pub fn find_field<'a>(&'a self, query: StructFieldQuery<'a>) -> Option<&'a StructField> {
        self.find_fields(query).next()
    }
}

/// Runtime description of an enum.
///
/// Build one with [`NativeEnumBuilder`] or [`RuntimeEnumBuilder`].
#[derive(Debug)]
pub struct Enum {
    /// Metadata attached to this type.
    pub meta: Option<Meta>,
    /// Name this type is registered under.
    pub name: String,
    /// Module this type belongs to.
    pub module_name: Option<String>,
    /// How widely this type is visible.
    pub visibility: Visibility,
    type_hash: TypeHash,
    type_name: String,
    variants: Vec<EnumVariant>,
    default_variant: Option<u8>,
    layout: Layout,
    initializer: Option<unsafe fn(*mut ())>,
    finalizer: unsafe fn(*mut ()),
    is_send: bool,
    is_sync: bool,
    is_copy: bool,
    is_runtime: bool,
}

impl Enum {
    /// Returns `true` for an enum with no Rust counterpart.
    ///
    /// Recorded when the type is built, not read off `type_hash`. See
    /// [`crate::types::struct_type::Struct::is_runtime`] for why.
    pub fn is_runtime(&self) -> bool {
        self.is_runtime
    }

    /// Returns `true` for an enum describing a real Rust type.
    pub fn is_native(&self) -> bool {
        !self.is_runtime()
    }

    /// Returns `true` when values may move between threads.
    pub fn is_send(&self) -> bool {
        self.is_send
    }

    /// Returns `true` when values may be shared between threads.
    pub fn is_sync(&self) -> bool {
        self.is_sync
    }

    /// Returns `true` when values may be duplicated by copying bytes.
    pub fn is_copy(&self) -> bool {
        self.is_copy
    }

    /// Returns `true` when a default value can be created, which needs both an
    /// initializer and a default variant.
    pub fn can_initialize(&self) -> bool {
        self.initializer.is_some()
    }

    /// Returns the runtime identity of this type.
    pub fn type_hash(&self) -> TypeHash {
        self.type_hash
    }

    /// Returns the full Rust type name.
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Returns the memory layout of a value, sized for the largest variant.
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Returns the variants, ordered by discriminant.
    pub fn variants(&self) -> &[EnumVariant] {
        &self.variants
    }

    /// Returns the discriminant a default value holds.
    pub fn default_variant_discriminant(&self) -> Option<u8> {
        self.default_variant
    }

    /// Returns the variant a default value holds.
    pub fn default_variant(&self) -> Option<&EnumVariant> {
        let discriminant = self.default_variant?;
        self.variants
            .iter()
            .find(|variant| variant.discriminant == discriminant)
    }

    /// Returns `true` when both enums have the same layout and variants,
    /// whatever they are called.
    pub fn is_compatible(&self, other: &Self) -> bool {
        self.layout == other.layout && self.variants == other.variants
    }

    /// Iterates the variants a query matches.
    pub fn find_variants<'a>(
        &'a self,
        query: EnumVariantQuery<'a>,
    ) -> impl Iterator<Item = &'a EnumVariant> + 'a {
        self.variants
            .iter()
            .filter(move |variant| query.is_valid(variant))
    }

    /// Returns the first variant a query matches.
    pub fn find_variant<'a>(&'a self, query: EnumVariantQuery<'a>) -> Option<&'a EnumVariant> {
        self.find_variants(query).next()
    }

    /// Finds the variant a Rust enum value currently holds.
    ///
    /// # Safety
    ///
    /// `value` must be a `repr(u8)` enum that this type describes, otherwise
    /// the byte read as a discriminant means nothing.
    pub unsafe fn find_variant_by_value<T: 'static>(&self, value: &T) -> Option<&EnumVariant> {
        if TypeHash::of::<T>() == self.type_hash {
            let discriminant = unsafe { (value as *const T as *const u8).read() };
            self.variants
                .iter()
                .find(|variant| variant.discriminant == discriminant)
        } else {
            None
        }
    }

    /// Returns the variant with the given discriminant.
    pub fn find_variant_by_discriminant(&self, discriminant: u8) -> Option<&EnumVariant> {
        self.variants
            .iter()
            .find(|variant| variant.discriminant == discriminant)
    }

    /// Duplicates a value by copying its bytes.
    ///
    /// Returns `false` when the type is not marked `Send`, or when source and
    /// target overlap.
    ///
    /// # Safety
    ///
    /// `from` must point at an initialized value of this type and `to` at
    /// writable memory of its layout that does not already hold a value.
    pub unsafe fn try_copy(&self, from: *const u8, to: *mut u8) -> bool {
        if !self.is_send {
            return false;
        }
        let size = self.layout.size();
        if from < unsafe { to.add(size) } && unsafe { from.add(size) } > to {
            return false;
        }
        unsafe { to.copy_from_nonoverlapping(from, size) };
        true
    }

    /// Writes a default value into already allocated memory, setting the default
    /// variant's discriminant.
    ///
    /// Returns `false` when the type has no initializer.
    ///
    /// # Safety
    ///
    /// `pointer` must be writable memory of this type's layout, not already
    /// holding a value.
    pub unsafe fn initialize(&self, pointer: *mut ()) -> bool {
        if let Some(initializer) = self.initializer {
            unsafe { (initializer)(pointer) };
            true
        } else {
            false
        }
    }

    /// Drops the value at `pointer` in place.
    ///
    /// # Safety
    ///
    /// `pointer` must point at an initialized value of this type that nothing
    /// reads afterwards.
    pub unsafe fn finalize(&self, pointer: *mut ()) {
        unsafe { (self.finalizer)(pointer) };
    }

    /// Returns the raw initializer function, or [`None`] when there is none.
    ///
    /// # Safety
    ///
    /// Same conditions as [`Enum::initialize`] apply to every call of it.
    pub unsafe fn initializer(&self) -> Option<unsafe fn(*mut ())> {
        self.initializer
    }

    /// Returns the raw drop function.
    ///
    /// # Safety
    ///
    /// Same conditions as [`Enum::finalize`] apply to every call of it.
    pub unsafe fn finalizer(&self) -> unsafe fn(*mut ()) {
        self.finalizer
    }

    /// Wraps this enum in a [`Type`].
    pub fn into_type(self) -> Type {
        self.into()
    }
}

impl PartialEq for Enum {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.type_hash == other.type_hash
            && self.layout == other.layout
            && self.variants == other.variants
    }
}

/// Search filter for enums.
///
/// The enum-only counterpart of [`crate::types::TypeQuery`].
#[derive(Debug, Default, Clone, PartialEq, Hash)]
pub struct EnumQuery<'a> {
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
    /// Filters matched against the leading variants.
    pub variants: Cow<'a, [EnumVariantQuery<'a>]>,
    /// Predicate the type metadata must satisfy.
    pub meta: Option<MetaQuery>,
}

impl<'a> EnumQuery<'a> {
    /// Matches by full Rust type name.
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

    /// Returns `true` when `enum_type` satisfies every set field.
    pub fn is_valid(&self, enum_type: &Enum) -> bool {
        self.name
            .as_ref()
            .map(|name| name.as_ref() == enum_type.name)
            .unwrap_or(true)
            && self
                .module_name
                .as_ref()
                .map(|name| {
                    enum_type
                        .module_name
                        .as_ref()
                        .map(|module_name| name.as_ref() == module_name)
                        .unwrap_or(false)
                })
                .unwrap_or(true)
            && self
                .type_hash
                .map(|type_hash| enum_type.type_hash == type_hash)
                .unwrap_or(true)
            && self
                .type_name
                .as_ref()
                .map(|type_name| enum_type.type_name == type_name.as_ref())
                .unwrap_or(true)
            && self
                .visibility
                .map(|visibility| enum_type.visibility.is_visible(visibility))
                .unwrap_or(true)
            && self
                .variants
                .iter()
                .zip(enum_type.variants.iter())
                .all(|(query, field)| query.is_valid(field))
            && self
                .meta
                .as_ref()
                .map(|query| enum_type.meta.as_ref().map(query).unwrap_or(false))
                .unwrap_or(true)
    }

    /// Hashes the query.
    pub fn as_hash(&self) -> u64 {
        let mut hasher = FxHasher::default();
        self.hash(&mut hasher);
        hasher.finish()
    }

    /// Copies borrowed names into owned ones.
    pub fn to_static(&self) -> EnumQuery<'static> {
        EnumQuery {
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
            variants: self
                .variants
                .as_ref()
                .iter()
                .map(|query| query.to_static())
                .collect(),
            meta: self.meta,
        }
    }
}

/// Builds an [`Enum`] describing a `repr(u8)` Rust enum, filling in
/// discriminants and field offsets.
///
/// ```ignore
/// define_native_enum! {
///     registry => mod lib enum Foo (Foo) {
///         A = 0,
///         B(usize) = 1,
///         C { a: u32 } = 2
///     }
/// }
/// ```
///
/// Add `[uninitialized]` for a type with no default value, and the
/// `[override_send = ...]` family to assert thread and copy properties the
/// builder cannot infer. Those are unsafe claims, see
/// [`NativeEnumBuilder::override_send`].
#[macro_export]
macro_rules! define_native_enum {
    (
        $registry:expr
        =>
        $(mod $module_name:ident)?
        enum $($name:ident)? ($type:tt) {
            $( $variant:tt )*
        }
        [uninitialized]
        $( [override_send = $override_send:literal] )?
        $( [override_sync = $override_sync:literal] )?
        $( [override_copy = $override_copy:literal] )?
    ) => {{
        #[allow(unused)]
        let mut override_send = Option::<bool>::None;
        $(
            override_send = Some($override_send as bool);
        )?
        #[allow(unused)]
        let mut override_sync = Option::<bool>::None;
        $(
            override_sync = Some($override_sync as bool);
        )?
        #[allow(unused)]
        let mut override_copy = Option::<bool>::None;
        $(
            override_copy = Some($override_copy as bool);
        )?
        #[allow(unused)]
        let mut name = std::any::type_name::<$type>().to_owned();
        $(
            name = stringify!($name).to_owned();
        )?
        #[allow(unused)]
        let mut result = $crate::types::enum_type::NativeEnumBuilder::new_named_uninitialized::<$type>(name);
        $(
            result = result.module_name(stringify!($module_name).to_owned());
        )?
        $( $crate::define_native_enum! { @variant $registry => result => $type => $variant } )*
        if let Some(mode) = override_send {
            result = unsafe { result.override_send(mode) };
        }
        if let Some(mode) = override_sync {
            result = unsafe { result.override_sync(mode) };
        }
        if let Some(mode) = override_copy {
            result = unsafe { result.override_copy(mode) };
        }
        result.build()
    }};
    (
        $registry:expr
        =>
        $(mod $module_name:ident)?
        enum $($name:ident)? ($type:tt) {
            $( $variant:tt )*
        }
        $( [override_send = $override_send:literal] )?
        $( [override_sync = $override_sync:literal] )?
        $( [override_copy = $override_copy:literal] )?
    ) => {{
        #[allow(unused)]
        let mut override_send = Option::<bool>::None;
        $(
            override_send = Some($override_send as bool);
        )?
        #[allow(unused)]
        let mut override_sync = Option::<bool>::None;
        $(
            override_sync = Some($override_sync as bool);
        )?
        #[allow(unused)]
        let mut override_copy = Option::<bool>::None;
        $(
            override_copy = Some($override_copy as bool);
        )?
        #[allow(unused)]
        let mut name = std::any::type_name::<$type>().to_owned();
        $(
            name = stringify!($name).to_owned();
        )?
        #[allow(unused)]
        let mut result = $crate::types::enum_type::NativeEnumBuilder::new_named::<$type>(name);
        $(
            result = result.module_name(stringify!($module_name).to_owned());
        )?
        $( $crate::define_native_enum! { @variant $registry => result => $type => $variant } )*
        if let Some(mode) = override_send {
            result = unsafe { result.override_send(mode) };
        }
        if let Some(mode) = override_sync {
            result = unsafe { result.override_sync(mode) };
        }
        if let Some(mode) = override_copy {
            result = unsafe { result.override_copy(mode) };
        }
        result.build()
    }};
    (@fields_tuple $registry:expr => $variant:expr => $type:tt => $name:ident => {
        $current_field_name:ident : $current_field_type:ty $( , $rest_field_name:ident : $rest_field_type:ty )*
    } => { $($field_name:ident),* } => $discriminant:literal) => {
        $variant = $variant.with_field_with_offset(
            $crate::types::struct_type::StructField::new(
                stringify!($current_field_name),
                $registry
                    .find_type($crate::types::TypeQuery::of::<$current_field_type>())
                    .unwrap(),
            ),
            $crate::__internal__offset_of_enum__!(
                $type :: $name [$($field_name),*] => $current_field_name => $discriminant
            ),
        );
        $crate::define_native_enum! { @fields_tuple $registry => $variant => $type => $name => {
            $( $rest_field_name : $rest_field_type ),*
        } => { $( $field_name ),* } => $discriminant }
    };
    (@fields_tuple $registry:expr => $variant:expr => $type:tt => $name:ident => {} => { $($field_name:ident),* } => $discriminant:literal) => {};
    (@variant $registry:expr => $result:expr => $type:tt => {
        $name:ident ( $( $field_name:ident : $field_type:ty ),* ) = $discriminant:literal
    }) => {
        $result = {
            #[allow(unused)]
            let mut variant = $crate::types::enum_type::EnumVariant::new(stringify!($name));
            $crate::define_native_enum! { @fields_tuple $registry => variant => $type => $name => {
                $( $field_name : $field_type ),*
            } => { $( $field_name ),* } => $discriminant }
            $result.variant(variant, $discriminant)
        };
    };
    (@variant $registry:expr => $result:expr => $type:tt => {
        $name:ident { $( $field_name:ident : $field_type:ty ),* } = $discriminant:literal
    }) => {
        $result = {
            #[allow(unused)]
            let mut variant = $crate::types::enum_type::EnumVariant::new(stringify!($name));
            $(
                variant = variant.with_field_with_offset(
                    $crate::types::struct_type::StructField::new(
                        stringify!($field_name),
                        $registry
                            .find_type($crate::types::TypeQuery::of::<$field_type>())
                            .unwrap(),
                    ),
                    $crate::__internal__offset_of_enum__!(
                        $type :: $name { $field_name } => $discriminant
                    ),
                );
            )*
            $result.variant(variant, $discriminant)
        };
    };
    (@variant $registry:expr => $result:expr => $type:tt => {
        $name:ident = $discriminant:literal
    }) => {
        $result = {
            let variant = $crate::types::enum_type::EnumVariant::new(stringify!($name));
            $result.variant(variant, $discriminant)
        };
    };
}

/// Builds an [`Enum`] that no Rust type backs, laying its variants out.
///
/// ```ignore
/// define_runtime_enum! {
///     registry => mod lib enum Foo {
///         A,
///         B { a: i32 }
///     }
/// }
/// ```
#[macro_export]
macro_rules! define_runtime_enum {
    (
        $registry:expr
        =>
        $(mod $module_name:ident)?
        enum $name:ident {
            $( $variant:tt )*
        }
    ) => {{
        #[allow(unused)]
        let mut result = $crate::types::enum_type::RuntimeEnumBuilder::new(stringify!($name));
        $(
            result = result.module_name(stringify!($module_name).to_owned());
        )?
        $( $crate::define_runtime_enum! { @variant $registry => result => $variant } )?
        result.build()
    }};
    (@variant $registry:expr => $result:expr => {
        $name:ident $( ( $( $field_name:ident : $field_type:ty ),+ ) )? = $discriminant:literal
    }) => {
        $result = {
            #[allow(unused)]
            let mut variant = $crate::types::enum_type::EnumVariant::new(stringify!($name));
            $(
                $(
                    variant = variant.with_field(
                        $crate::types::struct_type::StructField::new(
                            stringify!($field_name),
                            $registry
                                .find_type($crate::types::TypeQuery::of::<$field_type>())
                                .unwrap(),
                        ),
                    );
                )*
            )?
            $result.variant_with_discriminant(variant, $discriminant)
        };
    };
    (@variant $registry:expr => $result:expr => {
        $name:ident $( ( $( $field_name:ident : $field_type:ty ),+ ) )?
    }) => {
        $result = {
            #[allow(unused)]
            let mut variant = $crate::types::enum_type::EnumVariant::new(stringify!($name));
            $(
                $(
                    variant = variant.with_field(
                        $crate::types::struct_type::StructField::new(
                            stringify!($field_name),
                            $registry
                                .find_type($crate::types::TypeQuery::of::<$field_type>())
                                .unwrap(),
                        ),
                    );
                )*
            )?
            $result.variant(variant)
        };
    };
    (@variant $registry:expr => $result:expr => {
        $name:ident $( { $( $field_name:ident : $field_type:ty ),+ } )? = $discriminant:literal
    }) => {
        $result = {
            #[allow(unused)]
            let mut variant = $crate::types::enum_type::EnumVariant::new(stringify!($name));
            $(
                $(
                    variant = variant.with_field(
                        $crate::types::struct_type::StructField::new(
                            stringify!($field_name),
                            $registry
                                .find_type($crate::types::TypeQuery::of::<$field_type>())
                                .unwrap(),
                        ),
                    );
                )*
            )?
            $result.variant_with_discriminant(variant, $discriminant)
        };
    };
    (@variant $registry:expr => $result:expr => {
        $name:ident $( { $( $field_name:ident : $field_type:ty ),+ } )?
    }) => {
        $result = {
            #[allow(unused)]
            let mut variant = $crate::types::enum_type::EnumVariant::new(stringify!($name));
            $(
                $(
                    variant = variant.with_field(
                        $crate::types::struct_type::StructField::new(
                            stringify!($field_name),
                            $registry
                                .find_type($crate::types::TypeQuery::of::<$field_type>())
                                .unwrap(),
                        ),
                    );
                )*
            )?
            $result.variant(variant)
        };
    };
}

#[cfg(test)]
mod test {
    use crate::{self as intuicio_core};
    use crate::{IntuicioEnum, meta::Meta, object::*, registry::*};
    use intuicio_derive::*;

    #[derive(IntuicioEnum, Default)]
    #[intuicio(meta = "foo")]
    #[repr(u8)]
    #[allow(dead_code)]
    pub enum Bar {
        #[default]
        A,
        B(u8) = 10,
        C(u16, u32) = 3,
        D {
            a: u32,
            b: u16,
        },
    }

    #[intuicio_methods()]
    impl Bar {
        #[intuicio_method(meta = "foo")]
        fn method_meta() {}
    }

    #[derive(IntuicioEnum, Default)]
    #[repr(u8)]
    #[allow(dead_code)]
    pub enum Zap {
        A,
        #[intuicio(ignore)]
        B,
        #[intuicio(default)]
        #[default]
        C,
    }

    #[test]
    fn test_enum_derive_ignore_and_default() {
        let registry = Registry::default().with_basic_types();
        let enum_type = Zap::define_enum(&registry);
        assert_eq!(enum_type.variants().len(), 2);
        assert_eq!(enum_type.variants()[0].name, "A");
        assert_eq!(enum_type.variants()[0].discriminant(), 0);
        assert_eq!(enum_type.variants()[1].name, "C");
        assert_eq!(enum_type.variants()[1].discriminant(), 2);
        assert_eq!(enum_type.default_variant_discriminant(), Some(2));
    }

    #[test]
    fn test_enum_type() {
        #[repr(u8)]
        #[allow(dead_code)]
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
        enum Foo {
            #[default]
            A,
            B(usize),
            C(u64, u32),
            D {
                a: u16,
                b: u8,
            },
        }

        let mut registry = Registry::default().with_basic_types();
        let a = define_native_enum! {
            registry => enum (Foo) {
                {A = 0}
                {B(a: usize) = 1}
                {C(a: u64, b: u32) = 2}
                {D { a: u16, b: u8 } = 3}
            }
            [override_send = true]
            [override_sync = true]
            [override_copy = true]
        };
        let b = define_runtime_enum! {
            registry => enum Foo {
                {A = 0}
                {B(a: usize) = 1}
                {C(a: u64, b: u32)}
                {D { a: u16, b: u8 }}
            }
        };
        assert!(a.is_compatible(&b));
        let enum_type = registry.add_type(a);
        assert!(enum_type.is_send());
        assert!(enum_type.is_sync());
        assert!(enum_type.is_copy());
        assert!(enum_type.is_enum());
        assert_eq!(enum_type.type_name(), std::any::type_name::<Foo>());
        assert_eq!(enum_type.as_enum().unwrap().variants().len(), 4);
        assert_eq!(enum_type.as_enum().unwrap().variants()[0].name, "A");
        assert_eq!(enum_type.as_enum().unwrap().variants()[0].fields.len(), 0);
        assert_eq!(enum_type.as_enum().unwrap().variants()[1].name, "B");
        assert_eq!(enum_type.as_enum().unwrap().variants()[1].fields.len(), 1);
        assert_eq!(
            enum_type.as_enum().unwrap().variants()[1].fields[0].name,
            "a"
        );
        assert_eq!(
            enum_type.as_enum().unwrap().variants()[1].fields[0].address_offset(),
            8
        );
        assert_eq!(enum_type.as_enum().unwrap().variants()[2].name, "C");
        assert_eq!(enum_type.as_enum().unwrap().variants()[2].fields.len(), 2);
        assert_eq!(
            enum_type.as_enum().unwrap().variants()[2].fields[0].name,
            "a"
        );
        assert_eq!(
            enum_type.as_enum().unwrap().variants()[2].fields[0].address_offset(),
            8
        );
        assert_eq!(
            enum_type.as_enum().unwrap().variants()[2].fields[1].name,
            "b"
        );
        assert_eq!(
            enum_type.as_enum().unwrap().variants()[2].fields[1].address_offset(),
            16
        );
        assert_eq!(enum_type.as_enum().unwrap().variants()[3].name, "D");
        assert_eq!(enum_type.as_enum().unwrap().variants()[3].fields.len(), 2);
        assert_eq!(
            enum_type.as_enum().unwrap().variants()[3].fields[0].name,
            "a"
        );
        assert_eq!(
            enum_type.as_enum().unwrap().variants()[3].fields[0].address_offset(),
            2
        );
        assert_eq!(
            enum_type.as_enum().unwrap().variants()[3].fields[1].name,
            "b"
        );
        assert_eq!(
            enum_type.as_enum().unwrap().variants()[3].fields[1].address_offset(),
            4
        );

        let source = Foo::D { a: 10, b: 42 };
        let mut target = Object::new(enum_type.clone());
        assert!(unsafe { !enum_type.try_copy(target.as_ptr(), target.as_mut_ptr()) });
        assert_ne!(&source, target.read::<Foo>().unwrap());
        assert!(unsafe {
            enum_type.try_copy(&source as *const Foo as *const u8, target.as_mut_ptr())
        });
        assert_eq!(&source, target.read::<Foo>().unwrap());

        assert_eq!(
            Bar::define_enum(&registry).meta,
            Some(Meta::Identifier("foo".to_owned()))
        );
    }
}
