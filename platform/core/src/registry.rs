use crate::{
    function::{Function, FunctionHandle, FunctionQuery},
    object::Object,
    types::{Type, TypeHandle, TypeQuery, struct_type::NativeStructBuilder},
};
use intuicio_data::{
    managed::{DynamicManaged, DynamicManagedLazy, DynamicManagedRef, DynamicManagedRefMut},
    managed_gc::DynamicManagedGc,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

pub type RegistryHandle = Arc<Registry>;

#[derive(Debug, Default)]
pub struct Registry {
    functions: Vec<FunctionHandle>,
    types: Vec<TypeHandle>,
    pub index_capacity: usize,
    pub use_indexing_threshold: usize,
    functions_index: RwLock<BTreeMap<u64, FunctionHandle>>,
    types_index: RwLock<BTreeMap<u64, TypeHandle>>,
}

impl Clone for Registry {
    fn clone(&self) -> Self {
        Self {
            functions: self.functions.clone(),
            types: self.types.clone(),
            index_capacity: self.index_capacity,
            use_indexing_threshold: self.use_indexing_threshold,
            functions_index: RwLock::new(
                self.functions_index
                    .read()
                    .ok()
                    .map(|items| items.clone())
                    .unwrap_or_default(),
            ),
            types_index: RwLock::new(
                self.types_index
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
        self.with_type(NativeStructBuilder::new::<()>().build())
            .with_type(NativeStructBuilder::new::<bool>().build())
            .with_type(NativeStructBuilder::new::<i8>().build())
            .with_type(NativeStructBuilder::new::<i16>().build())
            .with_type(NativeStructBuilder::new::<i32>().build())
            .with_type(NativeStructBuilder::new::<i64>().build())
            .with_type(NativeStructBuilder::new::<i128>().build())
            .with_type(NativeStructBuilder::new::<isize>().build())
            .with_type(NativeStructBuilder::new::<u8>().build())
            .with_type(NativeStructBuilder::new::<u16>().build())
            .with_type(NativeStructBuilder::new::<u32>().build())
            .with_type(NativeStructBuilder::new::<u64>().build())
            .with_type(NativeStructBuilder::new::<u128>().build())
            .with_type(NativeStructBuilder::new::<usize>().build())
            .with_type(NativeStructBuilder::new::<f32>().build())
            .with_type(NativeStructBuilder::new::<f64>().build())
            .with_type(NativeStructBuilder::new::<char>().build())
            .with_type(NativeStructBuilder::new_named::<String>("String").build())
    }

    pub fn with_erased_types(self) -> Self {
        self.with_type(
            NativeStructBuilder::new_named_uninitialized::<DynamicManaged>("DynamicManaged")
                .build(),
        )
        .with_type(
            NativeStructBuilder::new_named_uninitialized::<DynamicManagedLazy>(
                "DynamicManagedLazy",
            )
            .build(),
        )
        .with_type(
            NativeStructBuilder::new_named_uninitialized::<DynamicManagedRef>("DynamicManagedRef")
                .build(),
        )
        .with_type(
            NativeStructBuilder::new_named_uninitialized::<DynamicManagedRefMut>(
                "DynamicManagedRefMut",
            )
            .build(),
        )
        .with_type(
            NativeStructBuilder::new_named_uninitialized::<DynamicManagedGc>("DynamicManagedGc")
                .build(),
        )
        .with_type(NativeStructBuilder::new_named_uninitialized::<Object>("Object").build())
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

    pub fn with_install(mut self, f: impl FnOnce(&mut Self)) -> Self {
        self.install(f);
        self
    }

    pub fn with_function(mut self, function: Function) -> Self {
        self.add_function(function);
        self
    }

    pub fn with_type(mut self, type_: impl Into<Type>) -> Self {
        self.add_type(type_);
        self
    }

    pub fn install(&mut self, f: impl FnOnce(&mut Self)) {
        f(self);
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
            self.functions.swap_remove(position);
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
    ) -> impl Iterator<Item = FunctionHandle> + 'a {
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

    pub fn add_type_handle(&mut self, type_handle: TypeHandle) {
        if !self
            .types
            .iter()
            .any(|handle| handle.as_ref() == type_handle.as_ref())
        {
            self.types.push(type_handle);
        }
    }

    pub fn add_type(&mut self, type_: impl Into<Type>) -> TypeHandle {
        let type_ = type_.into();
        if let Some(handle) = self.types.iter().find(|handle| handle.as_ref() == &type_) {
            handle.clone()
        } else {
            let handle = TypeHandle::new(type_);
            self.types.push(handle.clone());
            handle
        }
    }

    pub fn remove_type(&mut self, type_handle: TypeHandle) {
        if let Some(position) = self.types.iter().position(|handle| handle == &type_handle) {
            self.types.swap_remove(position);
        }
    }

    pub fn remove_types(&mut self, query: TypeQuery) {
        while let Some(position) = self.types.iter().position(|handle| query.is_valid(handle)) {
            self.types.swap_remove(position);
        }
    }

    pub fn types(&self) -> impl Iterator<Item = &TypeHandle> {
        self.types.iter()
    }

    pub fn find_types<'a>(&'a self, query: TypeQuery<'a>) -> impl Iterator<Item = TypeHandle> + 'a {
        self.types
            .iter()
            .filter(move |handle| query.is_valid(handle))
            .cloned()
    }

    pub fn find_type<'a>(&'a self, query: TypeQuery<'a>) -> Option<TypeHandle> {
        if self.index_capacity == 0 || self.types.len() < self.use_indexing_threshold {
            self.find_types(query).next()
        } else if let Ok(mut index) = self.types_index.try_write() {
            let hash = query.as_hash();
            if let Some(found) = index.get(&hash) {
                Some(found.clone())
            } else if let Some(found) = self.find_types(query).next() {
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
            self.find_types(query).next()
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
