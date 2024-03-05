use crate::{is_copy, is_send, is_sync, meta::Meta, prelude::RuntimeObject, Visibility};
use intuicio_data::{type_hash::TypeHash, Finalize, Initialize};
use rustc_hash::FxHasher;
use std::{
    alloc::Layout,
    borrow::Cow,
    hash::{Hash, Hasher},
    sync::Arc,
};

pub type StructHandle = Arc<Struct>;
pub type StructMetaQuery = fn(&Meta) -> bool;

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
}

impl RuntimeStructBuilder {
    pub fn new(name: impl ToString) -> Self {
        Self {
            meta: None,
            name: name.to_string(),
            module_name: None,
            visibility: Visibility::default(),
            type_hash: TypeHash::of::<RuntimeObject>(),
            type_name: std::any::type_name::<RuntimeObject>().to_owned(),
            fields: vec![],
            layout: Layout::from_size_align(0, 1).unwrap(),
            initializer: RuntimeObject::initialize_raw,
            finalizer: RuntimeObject::finalize_raw,
        }
    }

    pub fn meta(mut self, meta: Meta) -> Self {
        self.meta = Some(meta);
        self
    }

    pub fn module_name(mut self, module_name: impl ToString) -> Self {
        self.module_name = Some(module_name.to_string());
        self
    }

    pub fn visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = visibility;
        self
    }

    pub fn field(mut self, mut field: StructField) -> Self {
        let (new_layout, offset) = self.layout.extend(field.struct_handle.layout).unwrap();
        self.layout = new_layout;
        field.offset = offset;
        self.fields.push(field);
        self
    }

    pub fn build(mut self) -> Struct {
        self.fields.sort_by(|a, b| a.offset.cmp(&b.offset));
        let is_send = self.fields.iter().all(|field| field.struct_handle.is_send);
        let is_sync = self.fields.iter().all(|field| field.struct_handle.is_sync);
        let is_copy = self.fields.iter().all(|field| field.struct_handle.is_copy);
        Struct {
            meta: self.meta,
            name: self.name,
            module_name: self.module_name,
            visibility: self.visibility,
            type_hash: self.type_hash,
            type_name: self.type_name,
            fields: self.fields,
            layout: self.layout.pad_to_align(),
            initializer: Some(self.initializer),
            finalizer: self.finalizer,
            is_send,
            is_sync,
            is_copy,
        }
    }

    pub fn build_handle(self) -> StructHandle {
        self.build().into()
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
        }
    }
}

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
}

impl NativeStructBuilder {
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
            is_send: is_send::<T>(),
            is_sync: is_sync::<T>(),
            is_copy: is_copy::<T>(),
        }
    }

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
            is_send: is_send::<T>(),
            is_sync: is_sync::<T>(),
            is_copy: is_copy::<T>(),
        }
    }

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
            is_send: is_send::<T>(),
            is_sync: is_sync::<T>(),
            is_copy: is_copy::<T>(),
        }
    }

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
            is_send: is_send::<T>(),
            is_sync: is_sync::<T>(),
            is_copy: is_copy::<T>(),
        }
    }

    pub fn meta(mut self, meta: Meta) -> Self {
        self.meta = Some(meta);
        self
    }

    pub fn module_name(mut self, module_name: impl ToString) -> Self {
        self.module_name = Some(module_name.to_string());
        self
    }

    pub fn visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = visibility;
        self
    }

    pub fn field(mut self, mut field: StructField, offset: usize) -> Self {
        field.offset = offset;
        self.is_send = self.is_send && field.struct_handle.is_send;
        self.is_sync = self.is_sync && field.struct_handle.is_sync;
        self.is_copy = self.is_copy && field.struct_handle.is_copy;
        self.fields.push(field);
        self
    }

    /// # Safety
    pub unsafe fn override_send(mut self, mode: bool) -> Self {
        self.is_send = mode;
        self
    }

    /// # Safety
    pub unsafe fn override_sync(mut self, mode: bool) -> Self {
        self.is_sync = mode;
        self
    }

    /// # Safety
    pub unsafe fn override_copy(mut self, mode: bool) -> Self {
        self.is_copy = mode;
        self
    }

    pub fn build(mut self) -> Struct {
        self.fields.sort_by(|a, b| a.offset.cmp(&b.offset));
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
        }
    }

    pub fn build_handle(self) -> StructHandle {
        self.build().into()
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
        }
    }
}

pub struct StructField {
    pub meta: Option<Meta>,
    pub name: String,
    pub visibility: Visibility,
    offset: usize,
    struct_handle: StructHandle,
}

impl std::fmt::Debug for StructField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StructField")
            .field("meta", &self.meta)
            .field("name", &self.name)
            .field("visibility", &self.visibility)
            .field("offset", &self.offset)
            .field("struct_handle", &self.struct_handle.name)
            .finish()
    }
}

impl StructField {
    pub fn new(name: impl ToString, struct_handle: StructHandle) -> Self {
        Self {
            meta: None,
            name: name.to_string(),
            visibility: Visibility::default(),
            offset: 0,
            struct_handle,
        }
    }

    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = Some(meta);
        self
    }

    pub fn with_visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = visibility;
        self
    }

    pub fn address_offset(&self) -> usize {
        self.offset
    }

    pub fn struct_handle(&self) -> &StructHandle {
        &self.struct_handle
    }
}

impl PartialEq for StructField {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.offset == other.offset
            && self.struct_handle == other.struct_handle
    }
}

#[derive(Debug)]
pub struct Struct {
    pub meta: Option<Meta>,
    pub name: String,
    pub module_name: Option<String>,
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
}

impl Struct {
    pub fn is_runtime(&self) -> bool {
        self.type_hash == TypeHash::of::<RuntimeObject>()
    }

    pub fn is_native(&self) -> bool {
        !self.is_runtime()
    }

    pub fn is_send(&self) -> bool {
        self.is_send
    }

    pub fn is_sync(&self) -> bool {
        self.is_sync
    }

    pub fn is_copy(&self) -> bool {
        self.is_copy
    }

    pub fn can_initialize(&self) -> bool {
        self.initializer.is_some()
    }

    pub fn type_hash(&self) -> TypeHash {
        self.type_hash
    }

    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn fields(&self) -> &[StructField] {
        &self.fields
    }

    pub fn is_compatible(&self, other: &Self) -> bool {
        self.layout == other.layout && self.fields == other.fields
    }

    pub fn find_fields<'a>(
        &'a self,
        query: StructFieldQuery<'a>,
    ) -> impl Iterator<Item = &StructField> + '_ {
        self.fields
            .iter()
            .filter(move |field| query.is_valid(field))
    }

    pub fn find_field<'a>(&'a self, query: StructFieldQuery<'a>) -> Option<&StructField> {
        self.find_fields(query).next()
    }

    /// # Safety
    pub unsafe fn try_copy(&self, from: *const u8, to: *mut u8) -> bool {
        if !self.is_send {
            return false;
        }
        let size = self.layout.size();
        if from < to.add(size) && from.add(size) > to {
            return false;
        }
        to.copy_from_nonoverlapping(from, size);
        true
    }

    /// # Safety
    pub unsafe fn initialize(&self, pointer: *mut ()) -> bool {
        if let Some(initializer) = self.initializer {
            (initializer)(pointer);
            true
        } else {
            false
        }
    }

    /// # Safety
    pub unsafe fn finalize(&self, pointer: *mut ()) {
        (self.finalizer)(pointer);
    }

    /// # Safety
    pub unsafe fn initializer(&self) -> Option<unsafe fn(*mut ())> {
        self.initializer
    }

    /// # Safety
    pub unsafe fn finalizer(&self) -> unsafe fn(*mut ()) {
        self.finalizer
    }
}

impl PartialEq for Struct {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.type_hash == other.type_hash
            && self.layout == other.layout
            && self.fields.len() == other.fields.len()
            && self
                .fields
                .iter()
                .zip(other.fields.iter())
                .all(|(a, b)| a == b)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Hash)]
pub struct StructFieldQuery<'a> {
    pub name: Option<Cow<'a, str>>,
    pub struct_query: Option<StructQuery<'a>>,
    pub visibility: Option<Visibility>,
}

impl<'a> StructFieldQuery<'a> {
    pub fn is_valid(&self, field: &StructField) -> bool {
        self.name
            .as_ref()
            .map(|name| name.as_ref() == field.name)
            .unwrap_or(true)
            && self
                .struct_query
                .as_ref()
                .map(|query| query.is_valid(&field.struct_handle))
                .unwrap_or(true)
            && self
                .visibility
                .map(|visibility| field.visibility.is_visible(visibility))
                .unwrap_or(true)
    }

    pub fn to_static(&self) -> StructFieldQuery<'static> {
        StructFieldQuery {
            name: self
                .name
                .as_ref()
                .map(|name| name.as_ref().to_owned().into()),
            struct_query: self.struct_query.as_ref().map(|query| query.to_static()),
            visibility: self.visibility,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Hash)]
pub struct StructQuery<'a> {
    pub name: Option<Cow<'a, str>>,
    pub module_name: Option<Cow<'a, str>>,
    pub type_hash: Option<TypeHash>,
    pub type_name: Option<Cow<'a, str>>,
    pub visibility: Option<Visibility>,
    pub fields: Cow<'a, [StructFieldQuery<'a>]>,
    pub meta: Option<StructMetaQuery>,
}

impl<'a> StructQuery<'a> {
    pub fn of_type_name<T: 'static>() -> Self {
        Self {
            type_name: Some(std::any::type_name::<T>().into()),
            ..Default::default()
        }
    }

    pub fn of<T: 'static>() -> Self {
        Self {
            type_hash: Some(TypeHash::of::<T>()),
            ..Default::default()
        }
    }

    pub fn of_named<T: 'static>(name: &'a str) -> Self {
        Self {
            name: Some(name.into()),
            type_hash: Some(TypeHash::of::<T>()),
            ..Default::default()
        }
    }

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

    pub fn as_hash(&self) -> u64 {
        let mut hasher = FxHasher::default();
        self.hash(&mut hasher);
        hasher.finish()
    }

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
    ) => {
        {
            #[allow(unused_mut)]
            let mut override_send = Option::<bool>::None;
            $(
                override_send = Some($override_send as bool);
            )?
            #[allow(unused_mut)]
            let mut override_sync = Option::<bool>::None;
            $(
                override_sync = Some($override_sync as bool);
            )?
            #[allow(unused_mut)]
            let mut override_copy = Option::<bool>::None;
            $(
                override_copy = Some($override_copy as bool);
            )?
            #[allow(unused_mut)]
            let mut name = std::any::type_name::<$type>().to_owned();
            $(
                name = stringify!($name).to_owned();
            )?
            #[allow(unused_mut)]
            let mut result = $crate::struct_type::NativeStructBuilder::new_named_uninitialized::<$type>(name);
            $(
                result = result.module_name(stringify!($module_name).to_owned());
            )?
            $(
                result = result.field(
                    $crate::struct_type::StructField::new(
                        stringify!($field_name),
                        $registry
                            .find_struct($crate::struct_type::StructQuery::of::<$field_type>())
                            .unwrap(),
                    ),
                    $crate::__internal::offset_of!($type, $field_name),
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
        }
    };
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
    ) => {
        {
            #[allow(unused_mut)]
            let mut override_send = Option::<bool>::None;
            $(
                override_send = Some($override_send as bool);
            )?
            #[allow(unused_mut)]
            let mut override_sync = Option::<bool>::None;
            $(
                override_sync = Some($override_sync as bool);
            )?
            #[allow(unused_mut)]
            let mut override_copy = Option::<bool>::None;
            $(
                override_copy = Some($override_copy as bool);
            )?
            #[allow(unused_mut)]
            let mut name = std::any::type_name::<$type>().to_owned();
            $(
                name = stringify!($name).to_owned();
            )?
            #[allow(unused_mut)]
            let mut result = $crate::struct_type::NativeStructBuilder::new_named::<$type>(name);
            $(
                result = result.module_name(stringify!($module_name).to_owned());
            )?
            $(
                result = result.field(
                    $crate::struct_type::StructField::new(
                        stringify!($field_name),
                        $registry
                            .find_struct($crate::struct_type::StructQuery::of::<$field_type>())
                            .unwrap(),
                    ),
                    $crate::__internal::offset_of!($type, $field_name),
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
        }
    };
}

#[macro_export]
macro_rules! define_runtime_struct {
    (
        $registry:expr
        =>
        $(mod $module_name:ident)?
        struct $name:ident {
            $( $field_name:ident : $field_type:ty ),*
        }
    ) => {
        {
            let mut result = $crate::struct_type::RuntimeStructBuilder::new(stringify!($name));
            $(
                result.module_name = Some(stringify!($module_name).to_owned());
            )?
            $(
                result = result.field(
                    $crate::struct_type::StructField::new(
                        stringify!($field_name),
                        $registry
                            .find_struct($crate::struct_type::StructQuery::of::<$field_type>())
                            .unwrap(),
                    )
                );
            )*
            result.build()
        }
    };
}

#[cfg(test)]
mod tests {
    use crate as intuicio_core;
    use crate::object::Object;
    use crate::{meta::*, registry::*, IntuicioStruct};
    use intuicio_derive::*;

    #[derive(IntuicioStruct, Default)]
    #[intuicio(meta = "foo")]
    pub struct Bar {}

    #[intuicio_methods()]
    impl Bar {
        #[intuicio_method(meta = "foo")]
        fn method_meta() {}
    }

    #[test]
    fn test_struct_type() {
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
        struct Foo {
            a: bool,
            b: usize,
        }

        let mut registry = Registry::default().with_basic_types();
        let struct_type = registry.add_struct(define_native_struct! {
            registry => struct (Foo) {
                a: bool,
                b: usize
            }
        });
        assert!(struct_type.is_send());
        assert!(struct_type.is_sync());
        assert!(struct_type.is_copy());
        assert_eq!(struct_type.type_name(), std::any::type_name::<Foo>());
        assert_eq!(struct_type.fields()[0].name, "b");
        assert_eq!(struct_type.fields()[0].address_offset(), 0);
        assert_eq!(struct_type.fields()[1].name, "a");
        assert_eq!(struct_type.fields()[1].address_offset(), 8);

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
