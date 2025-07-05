use crate::Reference;
use intuicio_core::{
    context::Context,
    function::{Function, FunctionBody, FunctionParameter, FunctionSignature},
    registry::Registry,
    types::{TypeHandle, struct_type::NativeStructBuilder},
    utils::object_pop_from_stack,
};
use intuicio_data::type_hash::TypeHash;
use intuicio_derive::intuicio_function;
use intuicio_ffi::FfiLibrary;
use intuicio_framework_dynamic::{Array, Integer, Real, Text, Type};
use std::{
    alloc::Layout,
    ffi::{
        CString, c_char, c_double, c_float, c_int, c_long, c_longlong, c_short, c_uchar, c_uint,
        c_ulong, c_ulonglong, c_ushort, c_void,
    },
    path::Path,
    ptr::null_mut,
    sync::Arc,
};

#[repr(transparent)]
#[allow(non_camel_case_types)]
struct c_string(pub *mut c_char);

impl Default for c_string {
    fn default() -> Self {
        Self(null_mut())
    }
}

#[repr(transparent)]
#[allow(non_camel_case_types)]
struct c_pointer(pub *const c_void);

impl Default for c_pointer {
    fn default() -> Self {
        Self(null_mut())
    }
}

enum DataType {
    Void,
    CShort,
    CInt,
    CLong,
    CLongLong,
    CUChar,
    CUShort,
    CUInt,
    CULong,
    CULongLong,
    CFloat,
    CDouble,
    CChar,
    CString,
    Pointer,
    Value(TypeHandle),
}

impl DataType {
    fn from_reference(reference: &Reference, allow_non_copy_owned: bool) -> Self {
        if reference.is_null() {
            Self::Void
        } else if let Some(text) = reference.read::<Text>() {
            match text.as_str() {
                "void" => Self::Void,
                "char" => Self::CChar,
                "short" => Self::CShort,
                "int" => Self::CInt,
                "long" => Self::CLong,
                "longlong" => Self::CLongLong,
                "uchar" => Self::CUChar,
                "ushort" => Self::CUShort,
                "uint" => Self::CUInt,
                "ulong" => Self::CULong,
                "ulonglong" => Self::CULongLong,
                "float" => Self::CFloat,
                "double" => Self::CDouble,
                "string" => Self::CString,
                "pointer" => Self::Pointer,
                name => panic!("Unsupported data type specifier: {name}"),
            }
        } else if let Some(type_) = reference.read::<Type>() {
            if allow_non_copy_owned || type_.handle().unwrap().is_copy() {
                Self::Value(type_.handle().unwrap().clone())
            } else {
                panic!(
                    "Owned value type `{}` is not copy!",
                    type_.handle().unwrap().name()
                );
            }
        } else {
            panic!("Data type specifier can be only Text or Type!");
        }
    }

    fn type_handle(&self, is_result: bool) -> Option<TypeHandle> {
        match self {
            DataType::Void => None,
            DataType::CShort => Some(
                NativeStructBuilder::new::<c_short>()
                    .build()
                    .into_type()
                    .into_handle(),
            ),
            DataType::CInt => Some(
                NativeStructBuilder::new::<c_int>()
                    .build()
                    .into_type()
                    .into_handle(),
            ),
            DataType::CLong => Some(
                NativeStructBuilder::new::<c_long>()
                    .build()
                    .into_type()
                    .into_handle(),
            ),
            DataType::CLongLong => Some(
                NativeStructBuilder::new::<c_longlong>()
                    .build()
                    .into_type()
                    .into_handle(),
            ),
            DataType::CUChar => Some(
                NativeStructBuilder::new::<c_uchar>()
                    .build()
                    .into_type()
                    .into_handle(),
            ),
            DataType::CUShort => Some(
                NativeStructBuilder::new::<c_ushort>()
                    .build()
                    .into_type()
                    .into_handle(),
            ),
            DataType::CUInt => Some(
                NativeStructBuilder::new::<c_uint>()
                    .build()
                    .into_type()
                    .into_handle(),
            ),
            DataType::CULong => Some(
                NativeStructBuilder::new::<c_ulong>()
                    .build()
                    .into_type()
                    .into_handle(),
            ),
            DataType::CULongLong => Some(
                NativeStructBuilder::new::<c_ulonglong>()
                    .build()
                    .into_type()
                    .into_handle(),
            ),
            DataType::CFloat => Some(
                NativeStructBuilder::new::<c_float>()
                    .build()
                    .into_type()
                    .into_handle(),
            ),
            DataType::CDouble => Some(
                NativeStructBuilder::new::<c_double>()
                    .build()
                    .into_type()
                    .into_handle(),
            ),
            DataType::CChar => Some(
                NativeStructBuilder::new::<c_char>()
                    .build()
                    .into_type()
                    .into_handle(),
            ),
            DataType::CString => Some(
                NativeStructBuilder::new::<c_string>()
                    .build()
                    .into_type()
                    .into_handle(),
            ),
            DataType::Pointer => {
                if is_result {
                    None
                } else {
                    Some(
                        NativeStructBuilder::new::<c_pointer>()
                            .build()
                            .into_type()
                            .into_handle(),
                    )
                }
            }
            DataType::Value(handle) => Some(handle.clone()),
        }
    }

    fn pop_from_stack(&self, context: &mut Context, registry: &Registry) -> Reference {
        if let Some(value) = context.stack().pop::<c_short>() {
            Reference::new_integer(value as _, registry)
        } else if let Some(value) = context.stack().pop::<c_int>() {
            Reference::new_integer(value as _, registry)
        } else if let Some(value) = context.stack().pop::<c_long>() {
            Reference::new_integer(value as _, registry)
        } else if let Some(value) = context.stack().pop::<c_longlong>() {
            Reference::new_integer(value as _, registry)
        } else if let Some(value) = context.stack().pop::<c_uchar>() {
            Reference::new_integer(value as _, registry)
        } else if let Some(value) = context.stack().pop::<c_ushort>() {
            Reference::new_integer(value as _, registry)
        } else if let Some(value) = context.stack().pop::<c_uint>() {
            Reference::new_integer(value as _, registry)
        } else if let Some(value) = context.stack().pop::<c_ulong>() {
            Reference::new_integer(value as _, registry)
        } else if let Some(value) = context.stack().pop::<c_ulonglong>() {
            Reference::new_integer(value as _, registry)
        } else if let Some(value) = context.stack().pop::<c_float>() {
            Reference::new_real(value as _, registry)
        } else if let Some(value) = context.stack().pop::<c_double>() {
            Reference::new_real(value as _, registry)
        } else if let Some(value) = context.stack().pop::<c_char>() {
            Reference::new_text((value as u8 as char).to_string(), registry)
        } else if let Some(pointer) = context.stack().pop::<c_string>() {
            let value = unsafe { CString::from_raw(pointer.0) };
            Reference::new_text(value.to_string_lossy().to_string(), registry)
        } else if context.stack().pop::<c_pointer>().is_some() {
            panic!("Cannot marshal pointers from stack!")
        } else {
            let object = object_pop_from_stack(context.stack(), registry).unwrap();
            Reference::new_raw(object)
        }
    }
}

enum DataValue {
    CShort(c_short),
    CInt(c_int),
    CLong(c_long),
    CLongLong(c_longlong),
    CUChar(c_uchar),
    CUShort(c_ushort),
    CUInt(c_uint),
    CULong(c_ulong),
    CULongLong(c_ulonglong),
    CFloat(c_float),
    CDouble(c_double),
    CChar(c_char),
    CString(CString),
    Pointer(Reference),
    Value(Layout, TypeHash, unsafe fn(*mut ()), Vec<u8>),
}

impl DataValue {
    fn from_reference(reference: Reference, type_: &DataType) -> Self {
        match type_ {
            DataType::Void => unreachable!(),
            DataType::CShort => Self::CShort(*reference.read::<Integer>().unwrap() as _),
            DataType::CInt => Self::CInt(*reference.read::<Integer>().unwrap() as _),
            DataType::CLong => Self::CLong(*reference.read::<Integer>().unwrap() as _),
            DataType::CLongLong => Self::CLongLong(*reference.read::<Integer>().unwrap() as _),
            DataType::CUChar => Self::CUChar(*reference.read::<Integer>().unwrap() as _),
            DataType::CUShort => Self::CUShort(*reference.read::<Integer>().unwrap() as _),
            DataType::CUInt => Self::CUInt(*reference.read::<Integer>().unwrap() as _),
            DataType::CULong => Self::CULong(*reference.read::<Integer>().unwrap() as _),
            DataType::CULongLong => Self::CULongLong(*reference.read::<Integer>().unwrap() as _),
            DataType::CFloat => Self::CFloat(*reference.read::<Real>().unwrap() as _),
            DataType::CDouble => Self::CDouble(*reference.read::<Real>().unwrap() as _),
            DataType::CChar => {
                let text = reference.read::<Text>().unwrap();
                if text.is_empty() {
                    panic!("Text cannot be empty!");
                } else {
                    Self::CChar(text.chars().nth(0).unwrap() as _)
                }
            }
            DataType::CString => {
                let text = reference.read::<Text>().unwrap();
                Self::CString(CString::new(text.as_str()).unwrap())
            }
            DataType::Pointer => Self::Pointer(reference),
            DataType::Value(type_) => {
                if type_.is_copy() {
                    unsafe {
                        let mut data = vec![0u8; type_.layout().size()];
                        data.as_mut_ptr()
                            .copy_from(reference.read_object().unwrap().as_ptr(), data.len());
                        Self::Value(*type_.layout(), type_.type_hash(), type_.finalizer(), data)
                    }
                } else {
                    unreachable!()
                }
            }
        }
    }

    fn push_to_stack(&self, context: &mut Context) {
        match self {
            Self::CShort(value) => {
                context.stack().push(*value);
            }
            Self::CInt(value) => {
                context.stack().push(*value);
            }
            Self::CLong(value) => {
                context.stack().push(*value);
            }
            Self::CLongLong(value) => {
                context.stack().push(*value);
            }
            Self::CUChar(value) => {
                context.stack().push(*value);
            }
            Self::CUShort(value) => {
                context.stack().push(*value);
            }
            Self::CUInt(value) => {
                context.stack().push(*value);
            }
            Self::CULong(value) => {
                context.stack().push(*value);
            }
            Self::CULongLong(value) => {
                context.stack().push(*value);
            }
            Self::CFloat(value) => {
                context.stack().push(*value);
            }
            Self::CDouble(value) => {
                context.stack().push(*value);
            }
            Self::CChar(value) => {
                context.stack().push(*value);
            }
            Self::CString(buffer) => {
                context
                    .stack()
                    .push(c_string(buffer.as_ptr().cast_mut() as *mut _));
            }
            Self::Pointer(reference) => {
                context.stack().push(unsafe {
                    c_pointer(reference.read_object().unwrap().as_ptr().cast_mut() as *mut _)
                });
            }
            Self::Value(layout, type_hash, finalizer, data) => unsafe {
                context
                    .stack()
                    .push_raw(*layout, *type_hash, *finalizer, data);
            },
        }
    }
}

#[intuicio_function(module_name = "ffi", use_registry)]
pub fn load(registry: &Registry, path: Reference) -> Reference {
    let path = path.read::<Text>().unwrap();
    let path = Path::new(path.as_str());
    if let Ok(lib) = FfiLibrary::new(path) {
        Reference::new(lib, registry)
    } else {
        Reference::null()
    }
}

#[intuicio_function(module_name = "ffi", use_registry)]
pub fn function(
    registry: &Registry,
    mut library: Reference,
    name: Reference,
    result: Reference,
    arguments: Reference,
) -> Reference {
    let mut library = library.write::<FfiLibrary>().unwrap();
    let name = name.read::<Text>().unwrap();
    let result_type = DataType::from_reference(&result, true);
    let argument_types = arguments
        .read::<Array>()
        .unwrap()
        .iter()
        .map(|argument| DataType::from_reference(argument, false))
        .collect::<Vec<_>>();
    let mut signature = FunctionSignature::new(name.as_str()).with_module_name(library.name());
    if let Some(type_) = result_type.type_handle(true) {
        signature
            .outputs
            .push(FunctionParameter::new("result", type_));
    }
    for (index, argument) in argument_types.iter().enumerate() {
        signature.inputs.push(FunctionParameter::new(
            format!("arg{index}"),
            argument.type_handle(false).unwrap(),
        ));
    }
    let ffi = library.function(signature.clone()).unwrap();
    let handle = Function::new(
        signature,
        FunctionBody::Closure(Arc::new(move |context, registry| unsafe {
            let values = argument_types
                .iter()
                .map(|type_| {
                    DataValue::from_reference(context.stack().pop::<Reference>().unwrap(), type_)
                })
                .collect::<Vec<_>>();
            for value in values.into_iter().rev() {
                value.push_to_stack(context);
            }
            ffi.call(context).expect("FFI call error");
            let result = if !matches!(result_type, DataType::Void) {
                result_type.pop_from_stack(context, registry)
            } else {
                Reference::null()
            };
            context.stack().push(result);
        })),
    )
    .into_handle();
    Reference::new(intuicio_framework_dynamic::Function::new(handle), registry)
}

pub fn install(registry: &mut Registry) {
    registry.add_type(NativeStructBuilder::new_uninitialized::<FfiLibrary>().build());
    registry.add_function(load::define_function(registry));
    registry.add_function(function::define_function(registry));
}
