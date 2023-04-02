use crate::{
    function::{Function, FunctionHandle, FunctionQuery},
    prelude::{NativeStructBuilder, StructQuery},
    struct_type::{Struct, StructHandle},
};
use std::sync::Arc;

pub type RegistryHandle = Arc<Registry>;

#[derive(Debug, Default)]
pub struct Registry {
    functions: Vec<FunctionHandle>,
    structs: Vec<StructHandle>,
}

impl Registry {
    pub fn with_basic_types(self) -> Self {
        self.with_struct(NativeStructBuilder::new::<()>().build())
            .with_struct(NativeStructBuilder::new::<bool>().build())
            .with_struct(NativeStructBuilder::new::<i8>().build())
            .with_struct(NativeStructBuilder::new::<i16>().build())
            .with_struct(NativeStructBuilder::new::<i32>().build())
            .with_struct(NativeStructBuilder::new::<i64>().build())
            .with_struct(NativeStructBuilder::new::<i128>().build())
            .with_struct(NativeStructBuilder::new::<isize>().build())
            .with_struct(NativeStructBuilder::new::<u8>().build())
            .with_struct(NativeStructBuilder::new::<u16>().build())
            .with_struct(NativeStructBuilder::new::<u32>().build())
            .with_struct(NativeStructBuilder::new::<u64>().build())
            .with_struct(NativeStructBuilder::new::<u128>().build())
            .with_struct(NativeStructBuilder::new::<usize>().build())
            .with_struct(NativeStructBuilder::new::<f32>().build())
            .with_struct(NativeStructBuilder::new::<f64>().build())
            .with_struct(NativeStructBuilder::new::<char>().build())
            .with_struct(NativeStructBuilder::new_named::<String>("String").build())
    }

    pub fn with_function(mut self, function: Function) -> Self {
        self.add_function(function);
        self
    }

    pub fn with_struct(mut self, struct_type: Struct) -> Self {
        self.add_struct(struct_type);
        self
    }

    pub fn add_function_handle(&mut self, function_handle: FunctionHandle) {
        if !self
            .functions
            .iter()
            .any(|handle| handle.signature() == function_handle.signature())
        {
            self.functions.push(function_handle);
        }
    }

    pub fn add_function(&mut self, function: Function) -> FunctionHandle {
        if let Some(handle) = self
            .functions
            .iter()
            .find(|handle| handle.signature() == function.signature())
        {
            handle.clone()
        } else {
            let handle = FunctionHandle::new(function);
            self.functions.push(handle.clone());
            handle
        }
    }

    pub fn remove_function(&mut self, function_handle: FunctionHandle) {
        if let Some(position) = self
            .functions
            .iter()
            .position(|handle| handle.signature() == function_handle.signature())
        {
            self.functions.remove(position);
        }
    }

    pub fn remove_functions(&mut self, query: FunctionQuery) {
        while let Some(position) = self
            .functions
            .iter()
            .position(|handle| query.is_valid(handle.signature()))
        {
            self.functions.swap_remove(position);
        }
    }

    pub fn functions(&self) -> impl Iterator<Item = &FunctionHandle> {
        self.functions.iter()
    }

    pub fn find_functions<'a>(
        &'a self,
        query: FunctionQuery<'a>,
    ) -> impl Iterator<Item = FunctionHandle> + '_ {
        self.functions
            .iter()
            .filter(move |handle| query.is_valid(handle.signature()))
            .cloned()
    }

    pub fn find_function<'a>(&'a self, query: FunctionQuery<'a>) -> Option<FunctionHandle> {
        self.find_functions(query).next()
    }

    pub fn add_struct_handle(&mut self, struct_handle: StructHandle) {
        if !self
            .structs
            .iter()
            .any(|handle| handle.as_ref() == struct_handle.as_ref())
        {
            self.structs.push(struct_handle);
        }
    }

    pub fn add_struct(&mut self, struct_type: Struct) -> StructHandle {
        if let Some(handle) = self
            .structs
            .iter()
            .find(|handle| handle.as_ref() == &struct_type)
        {
            handle.clone()
        } else {
            let handle = StructHandle::new(struct_type);
            self.structs.push(handle.clone());
            handle
        }
    }

    pub fn remove_struct(&mut self, struct_handle: StructHandle) {
        if let Some(position) = self
            .structs
            .iter()
            .position(|handle| handle == &struct_handle)
        {
            self.functions.remove(position);
        }
    }

    pub fn remove_structs(&mut self, query: StructQuery) {
        while let Some(position) = self
            .structs
            .iter()
            .position(|handle| query.is_valid(handle))
        {
            self.structs.swap_remove(position);
        }
    }

    pub fn structs(&self) -> impl Iterator<Item = &StructHandle> {
        self.structs.iter()
    }

    pub fn find_structs<'a>(
        &'a self,
        query: StructQuery<'a>,
    ) -> impl Iterator<Item = StructHandle> + '_ {
        self.structs
            .iter()
            .filter(move |handle| query.is_valid(handle))
            .cloned()
    }

    pub fn find_struct<'a>(&'a self, query: StructQuery<'a>) -> Option<StructHandle> {
        self.find_structs(query).next()
    }
}
