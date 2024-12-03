//! NOTE: For now only acceptable enums are ones with `repr(u8)`,
//! because those have stable discriminant offset and size.
use crate::{
    meta::Meta,
    object::RuntimeObject,
    types::{struct_type::StructField, EnumVariantQuery, MetaQuery, StructFieldQuery, Type},
    Visibility,
};
use intuicio_data::{is_copy, is_send, is_sync, type_hash::TypeHash, Finalize, Initialize};
use rustc_hash::FxHasher;
use std::{
    alloc::Layout,
    borrow::Cow,
    hash::{Hash, Hasher},
};

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
}

impl RuntimeEnumBuilder {
    pub fn new(name: impl ToString) -> Self {
        Self {
            meta: None,
            name: name.to_string(),
            module_name: None,
            visibility: Visibility::default(),
            type_hash: TypeHash::of::<RuntimeObject>(),
            type_name: std::any::type_name::<RuntimeObject>().to_owned(),
            variants: vec![],
            defaut_variant: None,
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

    pub fn variant(mut self, mut variant: EnumVariant) -> Self {
        variant.discriminant = self
            .variants
            .last()
            .map(|variant| variant.discriminant + 1)
            .unwrap_or(0);
        self.variants.push(variant);
        self
    }

    pub fn variant_with_discriminant(mut self, mut variant: EnumVariant, discriminant: u8) -> Self {
        variant.discriminant = discriminant;
        self.variants.push(variant);
        self
    }

    pub fn set_default_variant(mut self, discriminant: u8) -> Self {
        self.defaut_variant = Some(discriminant);
        self
    }

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
        Enum {
            meta: self.meta,
            name: self.name,
            module_name: self.module_name,
            visibility: self.visibility,
            type_hash: self.type_hash,
            type_name: self.type_name,
            variants: self.variants,
            default_variant: self.defaut_variant,
            layout: self.layout.pad_to_align(),
            initializer: Some(self.initializer),
            finalizer: self.finalizer,
            is_send,
            is_sync,
            is_copy,
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
        }
    }
}

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
}

impl NativeEnumBuilder {
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
            variants: vec![],
            defaut_variant: None,
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
            variants: vec![],
            defaut_variant: None,
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
            variants: vec![],
            defaut_variant: None,
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

    pub fn variant(mut self, mut variant: EnumVariant, discriminant: u8) -> Self {
        self.is_send = self.is_send && variant.is_send();
        self.is_sync = self.is_sync && variant.is_sync();
        self.is_copy = self.is_copy && variant.is_copy();
        variant.discriminant = discriminant;
        self.variants.push(variant);
        self
    }

    pub fn set_default_variant(mut self, discriminant: u8) -> Self {
        self.defaut_variant = Some(discriminant);
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
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct EnumVariant {
    pub meta: Option<Meta>,
    pub name: String,
    pub fields: Vec<StructField>,
    discriminant: u8,
}

impl EnumVariant {
    pub fn new(name: impl ToString) -> Self {
        Self {
            meta: None,
            name: name.to_string(),
            fields: vec![],
            discriminant: 0,
        }
    }

    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = Some(meta);
        self
    }

    pub fn with_field(mut self, field: StructField) -> Self {
        self.fields.push(field);
        self
    }

    pub fn with_field_with_offset(mut self, mut field: StructField, offset: usize) -> Self {
        field.offset = offset;
        self.fields.push(field);
        self
    }

    pub fn discriminant(&self) -> u8 {
        self.discriminant
    }

    pub fn is_send(&self) -> bool {
        self.fields.iter().all(|f| f.type_handle.is_send())
    }

    pub fn is_sync(&self) -> bool {
        self.fields.iter().all(|f| f.type_handle.is_sync())
    }

    pub fn is_copy(&self) -> bool {
        self.fields.iter().all(|f| f.type_handle.is_copy())
    }

    pub fn find_fields<'a>(
        &'a self,
        query: StructFieldQuery<'a>,
    ) -> impl Iterator<Item = &'a StructField> + 'a {
        self.fields
            .iter()
            .filter(move |field| query.is_valid(field))
    }

    pub fn find_field<'a>(&'a self, query: StructFieldQuery<'a>) -> Option<&'a StructField> {
        self.find_fields(query).next()
    }
}

#[derive(Debug)]
pub struct Enum {
    pub meta: Option<Meta>,
    pub name: String,
    pub module_name: Option<String>,
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
}

impl Enum {
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

    pub fn variants(&self) -> &[EnumVariant] {
        &self.variants
    }

    pub fn default_variant_discriminant(&self) -> Option<u8> {
        self.default_variant
    }

    pub fn default_variant(&self) -> Option<&EnumVariant> {
        let discriminant = self.default_variant?;
        self.variants
            .iter()
            .find(|variant| variant.discriminant == discriminant)
    }

    pub fn is_compatible(&self, other: &Self) -> bool {
        self.layout == other.layout && self.variants == other.variants
    }

    pub fn find_variants<'a>(
        &'a self,
        query: EnumVariantQuery<'a>,
    ) -> impl Iterator<Item = &'a EnumVariant> + 'a {
        self.variants
            .iter()
            .filter(move |variant| query.is_valid(variant))
    }

    pub fn find_variant<'a>(&'a self, query: EnumVariantQuery<'a>) -> Option<&'a EnumVariant> {
        self.find_variants(query).next()
    }

    /// # Safety
    pub unsafe fn find_variant_by_value<T: 'static>(&self, value: &T) -> Option<&EnumVariant> {
        if TypeHash::of::<T>() == self.type_hash {
            let discriminant = (value as *const T as *const u8).read();
            self.variants
                .iter()
                .find(|variant| variant.discriminant == discriminant)
        } else {
            None
        }
    }

    pub fn find_variant_by_discriminant(&self, discriminant: u8) -> Option<&EnumVariant> {
        self.variants
            .iter()
            .find(|variant| variant.discriminant == discriminant)
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

#[derive(Debug, Default, Clone, PartialEq, Hash)]
pub struct EnumQuery<'a> {
    pub name: Option<Cow<'a, str>>,
    pub module_name: Option<Cow<'a, str>>,
    pub type_hash: Option<TypeHash>,
    pub type_name: Option<Cow<'a, str>>,
    pub visibility: Option<Visibility>,
    pub variants: Cow<'a, [EnumVariantQuery<'a>]>,
    pub meta: Option<MetaQuery>,
}

impl<'a> EnumQuery<'a> {
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

    pub fn as_hash(&self) -> u64 {
        let mut hasher = FxHasher::default();
        self.hash(&mut hasher);
        hasher.finish()
    }

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
            #[allow(unused_mut)]
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
            #[allow(unused_mut)]
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
        #[allow(unused_mut)]
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
            #[allow(unused_mut)]
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
            #[allow(unused_mut)]
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
            #[allow(unused_mut)]
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
            #[allow(unused_mut)]
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
    use crate::{meta::Meta, object::*, registry::*, IntuicioEnum};
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
