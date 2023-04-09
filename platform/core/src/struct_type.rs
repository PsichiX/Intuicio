use crate::{prelude::RuntimeObject, Visibility};
use intuicio_data::{type_hash::TypeHash, Finalize, Initialize};
use std::{alloc::Layout, borrow::Cow, sync::Arc};

pub type StructHandle = Arc<Struct>;

pub struct RuntimeStructBuilder {
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
        Struct {
            name: self.name,
            module_name: self.module_name,
            visibility: self.visibility,
            type_hash: self.type_hash,
            type_name: self.type_name,
            fields: self.fields,
            layout: self.layout.pad_to_align(),
            initializer: self.initializer,
            finalizer: self.finalizer,
        }
    }

    pub fn build_handle(self) -> StructHandle {
        self.build().into()
    }
}

impl From<Struct> for RuntimeStructBuilder {
    fn from(value: Struct) -> Self {
        Self {
            name: value.name,
            module_name: value.module_name,
            visibility: value.visibility,
            type_hash: value.type_hash,
            type_name: value.type_name,
            fields: value.fields,
            layout: value.layout,
            initializer: value.initializer,
            finalizer: value.finalizer,
        }
    }
}

pub struct NativeStructBuilder {
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

impl NativeStructBuilder {
    pub fn new<T: Initialize + Finalize + 'static>() -> Self {
        Self {
            name: std::any::type_name::<T>().to_owned(),
            module_name: None,
            visibility: Visibility::default(),
            type_hash: TypeHash::of::<T>(),
            type_name: std::any::type_name::<T>().to_owned(),
            fields: vec![],
            layout: Layout::new::<T>(),
            initializer: T::initialize_raw,
            finalizer: T::finalize_raw,
        }
    }

    pub fn new_named<T: Initialize + Finalize + 'static>(name: impl ToString) -> Self {
        Self {
            name: name.to_string(),
            module_name: None,
            visibility: Visibility::default(),
            type_hash: TypeHash::of::<T>(),
            type_name: std::any::type_name::<T>().to_owned(),
            fields: vec![],
            layout: Layout::new::<T>(),
            initializer: RuntimeObject::initialize_raw,
            finalizer: RuntimeObject::finalize_raw,
        }
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
        self.fields.push(field);
        self
    }

    pub fn build(mut self) -> Struct {
        self.fields.sort_by(|a, b| a.offset.cmp(&b.offset));
        Struct {
            name: self.name,
            module_name: self.module_name,
            visibility: self.visibility,
            type_hash: self.type_hash,
            type_name: self.type_name,
            fields: self.fields,
            layout: self.layout,
            initializer: self.initializer,
            finalizer: self.finalizer,
        }
    }

    pub fn build_handle(self) -> StructHandle {
        self.build().into()
    }
}

impl From<Struct> for NativeStructBuilder {
    fn from(value: Struct) -> Self {
        Self {
            name: value.name,
            module_name: value.module_name,
            visibility: value.visibility,
            type_hash: value.type_hash,
            type_name: value.type_name,
            fields: value.fields,
            layout: value.layout,
            initializer: value.initializer,
            finalizer: value.finalizer,
        }
    }
}

pub struct StructField {
    pub name: String,
    pub visibility: Visibility,
    offset: usize,
    struct_handle: StructHandle,
}

impl std::fmt::Debug for StructField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StructField")
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
            name: name.to_string(),
            visibility: Visibility::default(),
            offset: 0,
            struct_handle,
        }
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
    pub name: String,
    pub module_name: Option<String>,
    pub visibility: Visibility,
    type_hash: TypeHash,
    type_name: String,
    fields: Vec<StructField>,
    layout: Layout,
    pub(crate) initializer: unsafe fn(*mut ()),
    pub(crate) finalizer: unsafe fn(*mut ()),
}

impl Struct {
    pub fn is_runtime(&self) -> bool {
        self.type_hash == TypeHash::of::<RuntimeObject>()
    }

    pub fn is_native(&self) -> bool {
        !self.is_runtime()
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
    pub unsafe fn initialize(&self, pointer: *mut ()) {
        (self.initializer)(pointer);
    }

    /// # Safety
    pub unsafe fn finalize(&self, pointer: *mut ()) {
        (self.finalizer)(pointer);
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

#[derive(Debug, Default, Clone)]
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
}

#[derive(Debug, Default, Clone)]
pub struct StructQuery<'a> {
    pub name: Option<Cow<'a, str>>,
    pub module_name: Option<Cow<'a, str>>,
    pub type_hash: Option<TypeHash>,
    pub type_name: Option<Cow<'a, str>>,
    pub visibility: Option<Visibility>,
    pub fields: Cow<'a, [StructFieldQuery<'a>]>,
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
    ) => {
        {
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
    use crate::registry::Registry;

    #[test]
    fn test_struct_type() {
        #[derive(Default)]
        struct Foo {
            a: bool,
            b: usize,
        }

        let registry = Registry::default().with_basic_types();
        let struct_type = define_native_struct! {
            registry => struct (Foo) {
                a: bool,
                b: usize
            }
        };
        assert_eq!(struct_type.type_name(), std::any::type_name::<Foo>());
        assert_eq!(struct_type.fields()[0].name, "b");
        assert_eq!(struct_type.fields()[0].address_offset(), 0);
        assert_eq!(struct_type.fields()[1].name, "a");
        assert_eq!(struct_type.fields()[1].address_offset(), 8);
    }
}
