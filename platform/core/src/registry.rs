use crate::{
    function::{Function, FunctionHandle, FunctionQuery},
    prelude::{NativeStructBuilder, StructQuery},
    struct_type::{Struct, StructHandle},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

pub type RegistryHandle = Arc<Registry>;

#[derive(Debug, Default)]
pub struct Registry {
    functions: Vec<FunctionHandle>,
    structs: Vec<StructHandle>,
    pub index_capacity: usize,
    pub use_indexing_threshold: usize,
    functions_index: RwLock<BTreeMap<u64, FunctionHandle>>,
    structs_index: RwLock<BTreeMap<u64, StructHandle>>,
}

impl Clone for Registry {
    fn clone(&self) -> Self {
        Self {
            functions: self.functions.clone(),
            structs: self.structs.clone(),
            index_capacity: self.index_capacity,
            use_indexing_threshold: self.use_indexing_threshold,
            functions_index: RwLock::new(
                self.functions_index
                    .read()
                    .ok()
                    .map(|items| items.clone())
                    .unwrap_or_default(),
            ),
            structs_index: RwLock::new(
                self.structs_index
                    .read()
                    .ok()
                    .map(|items| items.clone())
                    .unwrap_or_default(),
            ),
        }
    }
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

    pub fn with_index_capacity(mut self, capacity: usize) -> Self {
        self.index_capacity = capacity;
        self
    }

    pub fn with_max_index_capacity(mut self) -> Self {
        self.index_capacity = usize::MAX;
        self
    }

    pub fn with_use_indexing_threshold(mut self, threshold: usize) -> Self {
        self.use_indexing_threshold = threshold;
        self
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
        if self.index_capacity == 0 || self.functions.len() < self.use_indexing_threshold {
            self.find_functions(query).next()
        } else if let Ok(mut index) = self.functions_index.try_write() {
            let hash = query.as_hash();
            if let Some(found) = index.get(&hash) {
                Some(found.clone())
            } else if let Some(found) = self.find_functions(query).next() {
                for _ in 0..(index.len().saturating_sub(self.index_capacity)) {
                    if let Some(hash) = index.keys().next().copied() {
                        index.remove(&hash);
                    }
                }
                index.insert(hash, found.clone());
                Some(found)
            } else {
                None
            }
        } else {
            self.find_functions(query).next()
        }
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
        if self.index_capacity == 0 || self.structs.len() < self.use_indexing_threshold {
            self.find_structs(query).next()
        } else if let Ok(mut index) = self.structs_index.try_write() {
            let hash = query.as_hash();
            if let Some(found) = index.get(&hash) {
                Some(found.clone())
            } else if let Some(found) = self.find_structs(query).next() {
                for _ in 0..(index.len().saturating_sub(self.index_capacity)) {
                    if let Some(hash) = index.keys().next().copied() {
                        index.remove(&hash);
                    }
                }
                index.insert(hash, found.clone());
                Some(found)
            } else {
                None
            }
        } else {
            self.find_structs(query).next()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_async() {
        fn is_async<T: Send + Sync>() {}

        is_async::<Registry>();
    }
}
