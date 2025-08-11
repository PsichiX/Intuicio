#![allow(unpredictable_function_pointer_comparisons)]

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
use intuicio_data::type_hash::TypeHash;
use rustc_hash::FxHasher;
use std::{
    alloc::Layout,
    borrow::Cow,
    hash::{Hash, Hasher},
    sync::Arc,
};

pub type TypeHandle = Arc<Type>;
pub type MetaQuery = fn(&Meta) -> bool;

#[derive(Debug, PartialEq)]
pub enum Type {
    Struct(Struct),
    Enum(Enum),
}

impl Type {
    pub fn is_struct(&self) -> bool {
        matches!(self, Self::Struct(_))
    }

    pub fn is_enum(&self) -> bool {
        matches!(self, Self::Enum(_))
    }

    pub fn as_struct(&self) -> Option<&Struct> {
        if let Self::Struct(value) = self {
            Some(value)
        } else {
            None
        }
    }

    pub fn as_enum(&self) -> Option<&Enum> {
        if let Self::Enum(value) = self {
            Some(value)
        } else {
            None
        }
    }

    pub fn meta(&self) -> Option<&Meta> {
        match self {
            Self::Struct(value) => value.meta.as_ref(),
            Self::Enum(value) => value.meta.as_ref(),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Struct(value) => &value.name,
            Self::Enum(value) => &value.name,
        }
    }

    pub fn module_name(&self) -> Option<&str> {
        match self {
            Self::Struct(value) => value.module_name.as_deref(),
            Self::Enum(value) => value.module_name.as_deref(),
        }
    }

    pub fn visibility(&self) -> Visibility {
        match self {
            Self::Struct(value) => value.visibility,
            Self::Enum(value) => value.visibility,
        }
    }

    pub fn is_runtime(&self) -> bool {
        match self {
            Self::Struct(value) => value.is_runtime(),
            Self::Enum(value) => value.is_runtime(),
        }
    }

    pub fn is_native(&self) -> bool {
        match self {
            Self::Struct(value) => value.is_native(),
            Self::Enum(value) => value.is_native(),
        }
    }

    pub fn is_send(&self) -> bool {
        match self {
            Self::Struct(value) => value.is_send(),
            Self::Enum(value) => value.is_send(),
        }
    }

    pub fn is_sync(&self) -> bool {
        match self {
            Self::Struct(value) => value.is_sync(),
            Self::Enum(value) => value.is_sync(),
        }
    }

    pub fn is_copy(&self) -> bool {
        match self {
            Self::Struct(value) => value.is_copy(),
            Self::Enum(value) => value.is_copy(),
        }
    }

    pub fn can_initialize(&self) -> bool {
        match self {
            Self::Struct(value) => value.can_initialize(),
            Self::Enum(value) => value.can_initialize(),
        }
    }

    pub fn type_hash(&self) -> TypeHash {
        match self {
            Self::Struct(value) => value.type_hash(),
            Self::Enum(value) => value.type_hash(),
        }
    }

    pub fn type_name(&self) -> &str {
        match self {
            Self::Struct(value) => value.type_name(),
            Self::Enum(value) => value.type_name(),
        }
    }

    pub fn layout(&self) -> &Layout {
        match self {
            Self::Struct(value) => value.layout(),
            Self::Enum(value) => value.layout(),
        }
    }

    pub fn struct_fields(&self) -> Option<&[StructField]> {
        if let Self::Struct(value) = self {
            Some(value.fields())
        } else {
            None
        }
    }

    pub fn enum_variants(&self) -> Option<&[EnumVariant]> {
        if let Self::Enum(value) = self {
            Some(value.variants())
        } else {
            None
        }
    }

    pub fn is_compatible(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Struct(a), Self::Struct(b)) => a.is_compatible(b),
            (Self::Enum(a), Self::Enum(b)) => a.is_compatible(b),
            _ => false,
        }
    }

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

    pub fn find_struct_field<'a>(&'a self, query: StructFieldQuery<'a>) -> Option<&'a StructField> {
        if let Self::Struct(value) = self {
            value.find_field(query)
        } else {
            None
        }
    }

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

    pub fn find_enum_variant<'a>(&'a self, query: EnumVariantQuery<'a>) -> Option<&'a EnumVariant> {
        if let Self::Enum(value) = self {
            value.find_variant(query)
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn try_copy(&self, from: *const u8, to: *mut u8) -> bool {
        match self {
            Self::Struct(value) => unsafe { value.try_copy(from, to) },
            Self::Enum(value) => unsafe { value.try_copy(from, to) },
        }
    }

    /// # Safety
    pub unsafe fn find_enum_variant_by_value<T: 'static>(&self, value: &T) -> Option<&EnumVariant> {
        if let Self::Enum(enum_type) = self {
            unsafe { enum_type.find_variant_by_value(value) }
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn initialize(&self, pointer: *mut ()) -> bool {
        match self {
            Self::Struct(value) => unsafe { value.initialize(pointer) },
            Self::Enum(value) => unsafe { value.initialize(pointer) },
        }
    }

    /// # Safety
    pub unsafe fn finalize(&self, pointer: *mut ()) {
        match self {
            Self::Struct(value) => unsafe { value.finalize(pointer) },
            Self::Enum(value) => unsafe { value.finalize(pointer) },
        }
    }

    /// # Safety
    pub unsafe fn initializer(&self) -> Option<unsafe fn(*mut ())> {
        match self {
            Self::Struct(value) => unsafe { value.initializer() },
            Self::Enum(value) => unsafe { value.initializer() },
        }
    }

    /// # Safety
    pub unsafe fn finalizer(&self) -> unsafe fn(*mut ()) {
        match self {
            Self::Struct(value) => unsafe { value.finalizer() },
            Self::Enum(value) => unsafe { value.finalizer() },
        }
    }

    pub fn into_handle(self) -> TypeHandle {
        self.into()
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

#[derive(Debug, Default, Clone, PartialEq, Hash)]
pub struct StructFieldQuery<'a> {
    pub name: Option<Cow<'a, str>>,
    pub type_query: Option<TypeQuery<'a>>,
    pub visibility: Option<Visibility>,
    pub meta: Option<MetaQuery>,
}

impl StructFieldQuery<'_> {
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

#[derive(Debug, Default, Clone, PartialEq, Hash)]
pub struct EnumVariantQuery<'a> {
    pub name: Option<Cow<'a, str>>,
    pub fields: Cow<'a, [StructFieldQuery<'a>]>,
    pub meta: Option<MetaQuery>,
}

impl EnumVariantQuery<'_> {
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

#[derive(Debug, Default, Clone, PartialEq, Hash)]
pub enum TypeKindQuery<'a> {
    #[default]
    None,
    Struct {
        fields: Cow<'a, [StructFieldQuery<'a>]>,
    },
    Enum {
        variants: Cow<'a, [EnumVariantQuery<'a>]>,
    },
}

impl TypeKindQuery<'_> {
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

#[derive(Debug, Default, Clone, PartialEq, Hash)]
pub struct TypeQuery<'a> {
    pub name: Option<Cow<'a, str>>,
    pub module_name: Option<Cow<'a, str>>,
    pub type_hash: Option<TypeHash>,
    pub type_name: Option<Cow<'a, str>>,
    pub visibility: Option<Visibility>,
    pub kind: TypeKindQuery<'a>,
    pub meta: Option<MetaQuery>,
}

impl<'a> TypeQuery<'a> {
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

    pub fn as_hash(&self) -> u64 {
        let mut hasher = FxHasher::default();
        self.hash(&mut hasher);
        hasher.finish()
    }

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
