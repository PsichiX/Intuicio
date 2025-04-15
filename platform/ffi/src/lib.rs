use intuicio_core::{
    context::Context,
    function::{Function, FunctionBody, FunctionQuery, FunctionSignature},
    types::TypeHandle,
};
use libffi::raw::{
    FFI_TYPE_STRUCT, ffi_abi_FFI_DEFAULT_ABI, ffi_call, ffi_cif, ffi_prep_cif, ffi_type,
    ffi_type_void,
};
use libloading::Library;
use std::{
    error::Error,
    ffi::{OsString, c_void},
    path::Path,
    ptr::null_mut,
    str::FromStr,
    sync::Arc,
};

pub use libffi::low::CodePtr as FfiCodePtr;

pub type FfiFunctionHandle = Arc<FfiFunction>;

pub struct FfiLibrary {
    library: Library,
    functions: Vec<(FunctionSignature, FfiFunctionHandle)>,
    name: String,
}

impl FfiLibrary {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let mut path = path.as_ref().to_path_buf();
        if path.extension().is_none() {
            path.set_extension(std::env::consts::DLL_EXTENSION);
        }
        Ok(Self {
            library: unsafe { Library::new(path.as_os_str())? },
            functions: Default::default(),
            name: path.to_string_lossy().to_string(),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn function(
        &mut self,
        signature: FunctionSignature,
    ) -> Result<FfiFunctionHandle, Box<dyn Error>> {
        unsafe {
            let symbol = OsString::from_str(&signature.name)?;
            let symbol = self
                .library
                .get::<unsafe extern "C" fn()>(symbol.as_encoded_bytes())?;
            let Some(function) = symbol.try_as_raw_ptr() else {
                return Err(format!("Could not get pointer of function: `{}`", signature).into());
            };
            let handle = Arc::new(FfiFunction::from_function_signature(
                FfiCodePtr(function),
                &signature,
            ));
            for (s, h) in &mut self.functions {
                if s == &signature {
                    *h = handle.clone();
                    return Ok(handle);
                }
            }
            self.functions.push((signature, handle.clone()));
            Ok(handle)
        }
    }

    pub fn find(&self, query: FunctionQuery) -> Option<FfiFunctionHandle> {
        self.functions.iter().find_map(|(signature, handle)| {
            if query.is_valid(signature) {
                Some(handle.clone())
            } else {
                None
            }
        })
    }
}

#[derive(Debug, Clone)]
pub struct FfiFunction {
    function: FfiCodePtr,
    result: Option<TypeHandle>,
    arguments: Vec<TypeHandle>,
}

unsafe impl Send for FfiFunction {}
unsafe impl Sync for FfiFunction {}

impl FfiFunction {
    pub fn from_function_signature(function: FfiCodePtr, signature: &FunctionSignature) -> Self {
        FfiFunction {
            function,
            result: signature
                .outputs
                .iter()
                .find(|param| param.name == "result")
                .map(|param| param.type_handle.clone()),
            arguments: signature
                .inputs
                .iter()
                .map(|param| param.type_handle.clone())
                .collect(),
        }
    }

    pub fn build_function(function: FfiCodePtr, signature: FunctionSignature) -> Function {
        let ffi = Self::from_function_signature(function, &signature);
        Function::new(
            signature,
            FunctionBody::Closure(Arc::new(move |context, _| unsafe {
                ffi.call(context).expect("FFI call error");
            })),
        )
    }

    pub fn new(function: FfiCodePtr) -> Self {
        Self {
            function,
            result: Default::default(),
            arguments: Default::default(),
        }
    }

    pub fn with_result(mut self, type_: TypeHandle) -> Self {
        self.result(type_);
        self
    }

    pub fn with_argument(mut self, type_: TypeHandle) -> Self {
        self.argument(type_);
        self
    }

    pub fn result(&mut self, type_: TypeHandle) {
        self.result = Some(type_);
    }

    pub fn argument(&mut self, type_: TypeHandle) {
        self.arguments.push(type_);
    }

    /// # Safety
    pub unsafe fn call(&self, context: &mut Context) -> Result<(), Box<dyn Error>> {
        let mut arguments_data = self
            .arguments
            .iter()
            .map(|type_| {
                if let Some((_, type_hash, _, data)) = unsafe { context.stack().pop_raw() } {
                    if type_hash == type_.type_hash() {
                        Ok(data)
                    } else {
                        Err(
                            format!("Popped value from stack is not `{}` type!", type_.name())
                                .into(),
                        )
                    }
                } else {
                    Err(format!("Could not pop `{}` type value from stack!", type_.name()).into())
                }
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        let mut arguments = arguments_data
            .iter_mut()
            .map(|data| data.as_mut_ptr() as *mut c_void)
            .collect::<Vec<_>>();
        let mut types = Vec::with_capacity(self.arguments.len() + 1);
        types.push(
            self.result
                .as_ref()
                .map(Self::make_type)
                .unwrap_or(unsafe { ffi_type_void }),
        );
        for type_ in &self.arguments {
            types.push(Self::make_type(type_));
        }
        let return_type = &mut types[0] as *mut _;
        let mut argument_types = types[1..]
            .iter_mut()
            .map(|type_| type_ as *mut _)
            .collect::<Vec<_>>();
        let mut cif = ffi_cif::default();
        unsafe {
            ffi_prep_cif(
                &mut cif as *mut _,
                ffi_abi_FFI_DEFAULT_ABI,
                arguments_data.len() as _,
                return_type,
                argument_types.as_mut_ptr(),
            )
        };
        let mut result = vec![0u8; unsafe { return_type.as_ref() }.unwrap().size];
        unsafe {
            ffi_call(
                &mut cif as *mut _,
                Some(*self.function.as_safe_fun()),
                result.as_mut_ptr() as *mut _,
                arguments.as_mut_ptr(),
            )
        };
        if let Some(type_) = self.result.as_ref() {
            unsafe {
                context.stack().push_raw(
                    *type_.layout(),
                    type_.type_hash(),
                    type_.finalizer(),
                    &result,
                )
            };
        }
        Ok(())
    }

    fn make_type(type_: &TypeHandle) -> ffi_type {
        let layout = type_.layout();
        ffi_type {
            size: layout.size(),
            alignment: layout.align() as _,
            type_: FFI_TYPE_STRUCT as _,
            elements: null_mut(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use intuicio_core::prelude::*;

    extern "C" fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    extern "C" fn ensure_42(v: i32) {
        assert_eq!(v, 42);
    }

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_ffi_function() {
        is_async::<FfiFunction>();

        let registry = Registry::default().with_basic_types();
        let mut context = Context::new(10240, 10240);

        let i32_type = registry.find_type(TypeQuery::of::<i32>()).unwrap();
        let ffi_add = FfiFunction::new(FfiCodePtr(add as *mut _))
            .with_argument(i32_type.clone())
            .with_argument(i32_type.clone())
            .with_result(i32_type.clone());
        let ffi_ensure =
            FfiFunction::new(FfiCodePtr(ensure_42 as *mut _)).with_argument(i32_type.clone());
        context.stack().push(2i32);
        context.stack().push(40i32);
        unsafe {
            ffi_add.call(&mut context).unwrap();
            ffi_ensure.call(&mut context).unwrap();
        }

        let ffi_add = FfiFunction::from_function_signature(
            FfiCodePtr(add as *mut _),
            &function_signature!(&registry => fn add(a: i32, b: i32) -> (result: i32)),
        );
        let ffi_ensure = FfiFunction::from_function_signature(
            FfiCodePtr(ensure_42 as *mut _),
            &function_signature!(&registry => fn ensure_42(v: i32) -> ()),
        );
        context.stack().push(2i32);
        context.stack().push(40i32);
        unsafe {
            ffi_add.call(&mut context).unwrap();
            ffi_ensure.call(&mut context).unwrap();
        }

        let ffi_add = FfiFunction::build_function(
            FfiCodePtr(add as *mut _),
            function_signature!(&registry => fn add(a: i32, b: i32) -> (result: i32)),
        );
        let ffi_ensure = FfiFunction::build_function(
            FfiCodePtr(ensure_42 as *mut _),
            function_signature!(&registry => fn ensure_42(v: i32) -> ()),
        );
        context.stack().push(2i32);
        context.stack().push(40i32);
        ffi_add.invoke(&mut context, &registry);
        ffi_ensure.invoke(&mut context, &registry);
    }

    #[test]
    fn test_ffi_library() {
        is_async::<FfiLibrary>();

        let registry = Registry::default().with_basic_types();
        let mut context = Context::new(10240, 10240);
        let mut lib = FfiLibrary::new("../../target/debug/ffi").unwrap();
        let ffi_add = lib
            .function(function_signature!(&registry => fn add(a: i32, b: i32) -> (result: i32)))
            .unwrap();
        let ffi_ensure = lib
            .function(function_signature!(&registry => fn ensure_42(v: i32) -> ()))
            .unwrap();
        context.stack().push(2i32);
        context.stack().push(40i32);
        unsafe {
            ffi_add.call(&mut context).unwrap();
            ffi_ensure.call(&mut context).unwrap();
        }
    }
}
