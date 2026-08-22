//! Runtime description of a struct.
//!
//! [`Struct`] is built by one of two builders and then registered:
//! [`NativeStructBuilder`] for a real Rust struct, whose layout the compiler
//! decides, and [`RuntimeStructBuilder`] for a struct a script invented, whose
//! layout is computed from the fields.
use crate::{
    Visibility,
    meta::Meta,
    object::RuntimeObject,
    types::{MetaQuery, StructFieldQuery, Type, TypeHandle},
};
use intuicio_data::{Finalize, Initialize, type_hash::TypeHash};
use rustc_hash::FxHasher;
use std::{
    alloc::Layout,
    borrow::Cow,
    hash::{Hash, Hasher},
};

/// Builds a [`Struct`] that no Rust type backs.
///
/// Field offsets and the overall layout are computed on [`build`](Self::build)
/// by laying the fields out in order. Values of the result are handled as
/// [`crate::object::Object`], field by field.
pub struct RuntimeStructBuilder {
    meta: Option<Meta>,
    name: String,
    module_name: Option<String>,
    visibility: Visibility,
    type_hash: TypeHash,
    type_name: String,
    fields: Vec<StructField>,
    layout: Layout,
    initializer: unsafe fn(*mut ()),
    finalizer: unsafe fn(*mut ()),
    is_runtime: bool,
}

impl RuntimeStructBuilder {
    /// Starts a struct with no fields.
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
            fields: vec![],
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

    /// Appends a field. Its offset is decided at build time.
    pub fn field(mut self, field: StructField) -> Self {
        self.fields.push(field);
        self
    }

    /// Lays the fields out and produces the type.
    ///
    /// `Send`, `Sync` and `Copy` are inferred: the struct has each of them only
    /// when every field does.
    ///
    /// # Panics
    ///
    /// Panics when the fields cannot be laid out, which means the total size
    /// overflowed.
    pub fn build(mut self) -> Struct {
        for field in &mut self.fields {
            let (new_layout, offset) = self.layout.extend(*field.type_handle.layout()).unwrap();
            self.layout = new_layout;
            field.offset = offset;
        }
        self.fields.sort_by_key(|a| a.offset);
        let is_send = self.fields.iter().all(|field| field.type_handle.is_send());
        let is_sync = self.fields.iter().all(|field| field.type_handle.is_sync());
        let is_copy = self.fields.iter().all(|field| field.type_handle.is_copy());
        let type_hash = if self.is_runtime {
            TypeHash::of_runtime(&qualified_name(self.module_name.as_deref(), &self.name))
        } else {
            self.type_hash
        };
        Struct {
            meta: self.meta,
            name: self.name,
            module_name: self.module_name,
            visibility: self.visibility,
            type_hash,
            type_name: self.type_name,
            fields: self.fields,
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

/// Joins a module and a name into the identity a runtime type hashes.
///
/// Two runtime types are the same type when they sit in the same module under
/// the same name, so that pair is what the hash has to come from.
pub(crate) fn qualified_name(module_name: Option<&str>, name: &str) -> String {
    match module_name {
        Some(module_name) => format!("{module_name}::{name}"),
        None => name.to_owned(),
    }
}

impl From<Struct> for RuntimeStructBuilder {
    fn from(value: Struct) -> Self {
        Self {
            meta: value.meta,
            name: value.name,
            module_name: value.module_name,
            visibility: value.visibility,
            type_hash: value.type_hash,
            type_name: value.type_name,
            fields: value.fields,
            layout: value.layout,
            initializer: value.initializer.unwrap_or(RuntimeObject::initialize_raw),
            finalizer: value.finalizer,
            is_runtime: value.is_runtime,
        }
    }
}

/// Builds a [`Struct`] describing a real Rust type.
///
/// The layout comes from the compiler, so field offsets have to be passed in
/// rather than computed. The `define_native_struct!` macro and the
/// `IntuicioStruct` derive both do that for you.
pub struct NativeStructBuilder {
    meta: Option<Meta>,
    name: String,
    module_name: Option<String>,
    visibility: Visibility,
    type_hash: TypeHash,
    type_name: String,
    fields: Vec<StructField>,
    layout: Layout,
    initializer: Option<unsafe fn(*mut ())>,
    finalizer: unsafe fn(*mut ()),
    is_send: bool,
    is_sync: bool,
    is_copy: bool,
    is_runtime: bool,
}

impl NativeStructBuilder {
    /// Describes `T`, named after its full Rust type name.
    pub fn new<T: Initialize + Finalize + 'static>() -> Self {
        Self {
            meta: None,
            name: std::any::type_name::<T>().to_owned(),
            module_name: None,
            visibility: Visibility::default(),
            type_hash: TypeHash::of::<T>(),
            type_name: std::any::type_name::<T>().to_owned(),
            fields: vec![],
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
            fields: vec![],
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
    ///
    /// Scripts can then hold and pass values of it, but not construct one.
    pub fn new_uninitialized<T: Finalize + 'static>() -> Self {
        Self {
            meta: None,
            name: std::any::type_name::<T>().to_owned(),
            module_name: None,
            visibility: Visibility::default(),
            type_hash: TypeHash::of::<T>(),
            type_name: std::any::type_name::<T>().to_owned(),
            fields: vec![],
            layout: Layout::new::<T>().pad_to_align(),
            initializer: None,
            finalizer: T::finalize_raw,
            is_send: false,
            is_sync: false,
            is_copy: false,
            is_runtime: false,
        }
    }

    /// [`NativeStructBuilder::new_uninitialized`] under a name of your choosing.
    pub fn new_named_uninitialized<T: Finalize + 'static>(name: impl ToString) -> Self {
        Self {
            meta: None,
            name: name.to_string(),
            module_name: None,
            visibility: Visibility::default(),
            type_hash: TypeHash::of::<T>(),
            type_name: std::any::type_name::<T>().to_owned(),
            fields: vec![],
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

    /// Declares a field at a byte offset inside the Rust type.
    ///
    /// Get the offset from `__internal__offset_of__!`, never by hand. Rust does
    /// not promise any particular field order.
    pub fn field(mut self, mut field: StructField, offset: usize) -> Self {
        field.offset = offset;
        self.is_send = self.is_send && field.type_handle.is_send();
        self.is_sync = self.is_sync && field.type_handle.is_sync();
        self.is_copy = self.is_copy && field.type_handle.is_copy();
        self.fields.push(field);
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
    pub fn build(mut self) -> Struct {
        self.fields.sort_by_key(|a| a.offset);
        Struct {
            meta: self.meta,
            name: self.name,
            module_name: self.module_name,
            visibility: self.visibility,
            type_hash: self.type_hash,
            type_name: self.type_name,
            fields: self.fields,
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

impl From<Struct> for NativeStructBuilder {
    fn from(value: Struct) -> Self {
        Self {
            meta: value.meta,
            name: value.name,
            module_name: value.module_name,
            visibility: value.visibility,
            type_hash: value.type_hash,
            type_name: value.type_name,
            fields: value.fields,
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

/// One field of a [`Struct`] or of an [`crate::types::enum_type::EnumVariant`].
pub struct StructField {
    /// Metadata attached to this field.
    pub meta: Option<Meta>,
    /// Field name.
    pub name: String,
    /// How widely this field is visible.
    pub visibility: Visibility,
    pub(crate) offset: usize,
    pub(crate) type_handle: TypeHandle,
}

impl std::fmt::Debug for StructField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StructField")
            .field("meta", &self.meta)
            .field("name", &self.name)
            .field("visibility", &self.visibility)
            .field("offset", &self.offset)
            .field("type_handle", &self.type_handle.name())
            .finish()
    }
}

impl StructField {
    /// Builds a field at offset zero, to be placed by a builder.
    pub fn new(name: impl ToString, type_handle: TypeHandle) -> Self {
        Self {
            meta: None,
            name: name.to_string(),
            visibility: Visibility::default(),
            offset: 0,
            type_handle,
        }
    }

    /// Attaches metadata, builder style.
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = Some(meta);
        self
    }

    /// Sets visibility, builder style.
    pub fn with_visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = visibility;
        self
    }

    /// Returns the byte offset of this field inside the value.
    pub fn address_offset(&self) -> usize {
        self.offset
    }

    /// Returns the type of this field.
    pub fn type_handle(&self) -> &TypeHandle {
        &self.type_handle
    }
}

impl PartialEq for StructField {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.offset == other.offset
            && self.type_handle == other.type_handle
    }
}

/// Runtime description of a struct.
///
/// Build one with [`NativeStructBuilder`] or [`RuntimeStructBuilder`].
#[derive(Debug)]
pub struct Struct {
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
    fields: Vec<StructField>,
    layout: Layout,
    initializer: Option<unsafe fn(*mut ())>,
    finalizer: unsafe fn(*mut ()),
    is_send: bool,
    is_sync: bool,
    is_copy: bool,
    is_runtime: bool,
}

impl Struct {
    /// Returns `true` for a struct with no Rust counterpart.
    ///
    /// This is recorded when the type is built, not read off `type_hash`. Every
    /// runtime type gets its own hash, so a value can say which one it is. What
    /// makes such a type non-native is that the hash comes from
    /// [`TypeHash::of_runtime`], which no Rust type can produce.
    pub fn is_runtime(&self) -> bool {
        self.is_runtime
    }

    /// Returns `true` for a struct describing a real Rust type.
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

    /// Returns `true` when a default value can be created.
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

    /// Returns the memory layout of a value.
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Returns the fields, ordered by offset.
    pub fn fields(&self) -> &[StructField] {
        &self.fields
    }

    /// Returns `true` when both structs have the same layout and fields,
    /// whatever they are called.
    pub fn is_compatible(&self, other: &Self) -> bool {
        self.layout == other.layout && self.fields == other.fields
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

    /// Writes a default value into already allocated memory.
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
    /// Same conditions as [`Struct::initialize`] apply to every call of it.
    pub unsafe fn initializer(&self) -> Option<unsafe fn(*mut ())> {
        self.initializer
    }

    /// Returns the raw drop function.
    ///
    /// # Safety
    ///
    /// Same conditions as [`Struct::finalize`] apply to every call of it.
    pub unsafe fn finalizer(&self) -> unsafe fn(*mut ()) {
        self.finalizer
    }

    /// Wraps this struct in a [`Type`].
    pub fn into_type(self) -> Type {
        self.into()
    }
}

impl PartialEq for Struct {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.type_hash == other.type_hash
            && self.layout == other.layout
            && self.fields == other.fields
    }
}

/// Search filter for structs.
///
/// The struct-only counterpart of [`crate::types::TypeQuery`].
#[derive(Debug, Default, Clone, PartialEq, Hash)]
pub struct StructQuery<'a> {
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
    /// Filters matched against the leading fields.
    pub fields: Cow<'a, [StructFieldQuery<'a>]>,
    /// Predicate the type metadata must satisfy.
    pub meta: Option<MetaQuery>,
}

impl<'a> StructQuery<'a> {
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

    /// Returns `true` when `struct_type` satisfies every set field.
    pub fn is_valid(&self, struct_type: &Struct) -> bool {
        self.name
            .as_ref()
            .map(|name| name.as_ref() == struct_type.name)
            .unwrap_or(true)
            && self
                .module_name
                .as_ref()
                .map(|name| {
                    struct_type
                        .module_name
                        .as_ref()
                        .map(|module_name| name.as_ref() == module_name)
                        .unwrap_or(false)
                })
                .unwrap_or(true)
            && self
                .type_hash
                .map(|type_hash| struct_type.type_hash == type_hash)
                .unwrap_or(true)
            && self
                .type_name
                .as_ref()
                .map(|type_name| struct_type.type_name == type_name.as_ref())
                .unwrap_or(true)
            && self
                .visibility
                .map(|visibility| struct_type.visibility.is_visible(visibility))
                .unwrap_or(true)
            && self
                .fields
                .iter()
                .zip(struct_type.fields.iter())
                .all(|(query, field)| query.is_valid(field))
            && self
                .meta
                .as_ref()
                .map(|query| struct_type.meta.as_ref().map(query).unwrap_or(false))
                .unwrap_or(true)
    }

    /// Hashes the query.
    pub fn as_hash(&self) -> u64 {
        let mut hasher = FxHasher::default();
        self.hash(&mut hasher);
        hasher.finish()
    }

    /// Copies borrowed names into owned ones.
    pub fn to_static(&self) -> StructQuery<'static> {
        StructQuery {
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

/// Builds a [`Struct`] describing a Rust type, filling in field offsets.
///
/// ```ignore
/// define_native_struct! {
///     registry => mod lib struct Foo (Foo) {
///         a: bool,
///         b: i32
///     }
/// }
/// ```
///
/// Add `[uninitialized]` for a type with no default value, and
/// `[override_send = true]`, `[override_sync = true]` or
/// `[override_copy = true]` to assert thread and copy properties the builder
/// cannot infer. Those overrides are unsafe claims. See
/// [`NativeStructBuilder::override_send`].
#[macro_export]
macro_rules! define_native_struct {
    (
        $registry:expr
        =>
        $(mod $module_name:ident)?
        struct $($name:ident)? ($type:ty) {
            $( $field_name:ident : $field_type:ty ),*
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
        let mut result = $crate::types::struct_type::NativeStructBuilder::new_named_uninitialized::<$type>(name);
        $(
            result = result.module_name(stringify!($module_name).to_owned());
        )?
        $(
            result = result.field(
                $crate::types::struct_type::StructField::new(
                    stringify!($field_name),
                    $registry
                        .find_type($crate::types::TypeQuery::of::<$field_type>())
                        .unwrap(),
                ),
                $crate::__internal__offset_of__!($type, $field_name),
            );
        )*
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
        struct $($name:ident)? ($type:ty) {
            $( $field_name:ident : $field_type:ty ),*
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
        let mut result = $crate::types::struct_type::NativeStructBuilder::new_named::<$type>(name);
        $(
            result = result.module_name(stringify!($module_name).to_owned());
        )?
        $(
            result = result.field(
                $crate::types::struct_type::StructField::new(
                    stringify!($field_name),
                    $registry
                        .find_type($crate::types::TypeQuery::of::<$field_type>())
                        .unwrap(),
                ),
                $crate::__internal__offset_of__!($type, $field_name),
            );
        )*
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
}

/// Builds a [`Struct`] that no Rust type backs, laying its fields out.
///
/// ```ignore
/// define_runtime_struct! {
///     registry => mod lib struct Foo {
///         a: bool,
///         b: i32
///     }
/// }
/// ```
#[macro_export]
macro_rules! define_runtime_struct {
    (
        $registry:expr
        =>
        $(mod $module_name:ident)?
        struct $name:ident {
            $( $field_name:ident : $field_type:ty ),*
        }
    ) => {{
        #[allow(unused)]
        let mut result = $crate::types::struct_type::RuntimeStructBuilder::new(stringify!($name));
        $(
            result.module_name = Some(stringify!($module_name).to_owned());
        )?
        $(
            result = result.field(
                $crate::types::struct_type::StructField::new(
                    stringify!($field_name),
                    $registry
                        .find_type($crate::types::TypeQuery::of::<$field_type>())
                        .unwrap(),
                )
            );
        )*
        result.build()
    }};
}

#[cfg(test)]
mod tests {
    #![allow(unused_attributes)]
    use crate as intuicio_core;
    use crate::{IntuicioStruct, meta::Meta, object::*, registry::*};
    use intuicio_data::type_hash::TypeHash;
    use intuicio_derive::*;

    #[derive(IntuicioStruct, Default)]
    #[intuicio(meta = "foo")]
    pub struct Bar {}

    #[intuicio_methods()]
    impl Bar {
        #[intuicio_method(meta = "foo", args_meta(_bar = "foo"))]
        fn method_meta(_bar: bool) {}
    }

    #[test]
    fn test_runtime_structs_have_their_own_type_hash() {
        use super::RuntimeStructBuilder;

        let a = RuntimeStructBuilder::new("Player")
            .module_name("game")
            .build();
        let b = RuntimeStructBuilder::new("Vector")
            .module_name("game")
            .build();
        let a_again = RuntimeStructBuilder::new("Player")
            .module_name("game")
            .build();
        let a_elsewhere = RuntimeStructBuilder::new("Player")
            .module_name("other")
            .build();

        // Two different script types must be tellable apart, otherwise a value
        // cannot say which one it is and reflection lands on the wrong type.
        assert_ne!(a.type_hash(), b.type_hash());
        // The same module and name is the same type, so building it twice has
        // to agree. `declare` then `define` relies on this.
        assert_eq!(a.type_hash(), a_again.type_hash());
        // The module is part of the identity.
        assert_ne!(a.type_hash(), a_elsewhere.type_hash());

        // Still not a Rust type, which is what keeps casting illegal.
        assert!(a.is_runtime());
        assert!(!a.is_native());
        assert_ne!(a.type_hash(), TypeHash::of::<RuntimeObject>());
    }

    #[test]
    fn test_native_structs_stay_native() {
        use super::NativeStructBuilder;

        let type_ = NativeStructBuilder::new::<bool>().build();

        assert!(type_.is_native());
        assert!(!type_.is_runtime());
        assert_eq!(type_.type_hash(), TypeHash::of::<bool>());
    }

    #[test]
    fn test_struct_type() {
        #[repr(C)]
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
        struct Foo {
            a: bool,
            b: usize,
        }

        let mut registry = Registry::default().with_basic_types();
        let a = define_native_struct! {
            registry => struct (Foo) {
                a: bool,
                b: usize
            }
            [override_send = true]
            [override_sync = true]
            [override_copy = true]
        };
        let b = define_runtime_struct! {
            registry => struct Foo {
                a: bool,
                b: usize
            }
        };
        assert!(a.is_compatible(&b));
        let struct_type = registry.add_type(a);
        assert!(struct_type.is_send());
        assert!(struct_type.is_sync());
        assert!(struct_type.is_copy());
        assert!(struct_type.is_struct());
        assert_eq!(struct_type.type_name(), std::any::type_name::<Foo>());
        assert_eq!(struct_type.as_struct().unwrap().fields().len(), 2);
        assert_eq!(struct_type.as_struct().unwrap().fields()[0].name, "a");
        assert_eq!(
            struct_type.as_struct().unwrap().fields()[0].address_offset(),
            0
        );
        assert_eq!(struct_type.as_struct().unwrap().fields()[1].name, "b");
        assert_eq!(
            struct_type.as_struct().unwrap().fields()[1].address_offset(),
            8
        );

        let source = Foo { a: true, b: 42 };
        let mut target = Object::new(struct_type.clone());
        assert!(unsafe { !struct_type.try_copy(target.as_ptr(), target.as_mut_ptr()) });
        assert_ne!(&source, target.read::<Foo>().unwrap());
        assert!(unsafe {
            struct_type.try_copy(&source as *const Foo as *const u8, target.as_mut_ptr())
        });
        assert_eq!(&source, target.read::<Foo>().unwrap());

        assert_eq!(
            Bar::define_struct(&registry).meta,
            Some(Meta::Identifier("foo".to_owned()))
        );
    }
}
