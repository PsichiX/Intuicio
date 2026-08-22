//! The catalogue of everything scripts and host can reach.
//!
//! See [`Registry`].
use crate::{
    function::{Function, FunctionHandle, FunctionQuery},
    object::Object,
    types::{Type, TypeHandle, TypeQuery, struct_type::NativeStructBuilder},
};
use intuicio_data::managed::{
    DynamicManaged, DynamicManagedLazy, DynamicManagedRef, DynamicManagedRefMut,
    gc::DynamicManagedGc,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

/// Shared registry, as a [`crate::host::Host`] holds it.
pub type RegistryHandle = Arc<Registry>;

/// Everything both sides of a scripting solution know about: types and
/// functions.
///
/// Nothing can be called or constructed unless it is in here first, so a
/// registry is filled during setup and then treated as read only: a function
/// call takes `&Registry`, which means the registry cannot change while any
/// script is running.
///
/// # Lookups
///
/// Items are found by query rather than by key, since a script may know only
/// part of what it is looking for. See [`FunctionQuery`] and [`TypeQuery`].
///
/// Queries scan linearly by default. Two fields turn on a cache of resolved
/// queries: [`Registry::index_capacity`] sets how many results to remember,
/// and [`Registry::use_indexing_threshold`] how many items must be registered
/// before the cache is worth using.
///
/// ```
/// # use intuicio_core::{registry::Registry, types::TypeQuery};
/// let registry = Registry::default().with_basic_types();
/// assert!(registry.find_type(TypeQuery::of::<i32>()).is_some());
/// ```
#[derive(Debug, Default)]
pub struct Registry {
    functions: Vec<FunctionHandle>,
    types: Vec<TypeHandle>,
    /// How many resolved queries to keep cached. `0` disables the cache.
    pub index_capacity: usize,
    /// How many registered items are needed before the cache is used at all.
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
    /// Registers the Rust primitives and [`String`].
    ///
    /// Almost every setup wants these, since function signatures are built from
    /// types looked up in the registry.
    pub fn with_basic_types(self) -> Self {
        unsafe {
            self.with_type(
                NativeStructBuilder::new::<()>()
                    .override_send(true)
                    .override_sync(true)
                    .override_copy(true)
                    .build(),
            )
            .with_type(
                NativeStructBuilder::new::<bool>()
                    .override_send(true)
                    .override_sync(true)
                    .override_copy(true)
                    .build(),
            )
            .with_type(
                NativeStructBuilder::new::<i8>()
                    .override_send(true)
                    .override_sync(true)
                    .override_copy(true)
                    .build(),
            )
            .with_type(
                NativeStructBuilder::new::<i16>()
                    .override_send(true)
                    .override_sync(true)
                    .override_copy(true)
                    .build(),
            )
            .with_type(
                NativeStructBuilder::new::<i32>()
                    .override_send(true)
                    .override_sync(true)
                    .override_copy(true)
                    .build(),
            )
            .with_type(
                NativeStructBuilder::new::<i64>()
                    .override_send(true)
                    .override_sync(true)
                    .override_copy(true)
                    .build(),
            )
            .with_type(
                NativeStructBuilder::new::<i128>()
                    .override_send(true)
                    .override_sync(true)
                    .override_copy(true)
                    .build(),
            )
            .with_type(
                NativeStructBuilder::new::<isize>()
                    .override_send(true)
                    .override_sync(true)
                    .override_copy(true)
                    .build(),
            )
            .with_type(
                NativeStructBuilder::new::<u8>()
                    .override_send(true)
                    .override_sync(true)
                    .override_copy(true)
                    .build(),
            )
            .with_type(
                NativeStructBuilder::new::<u16>()
                    .override_send(true)
                    .override_sync(true)
                    .override_copy(true)
                    .build(),
            )
            .with_type(
                NativeStructBuilder::new::<u32>()
                    .override_send(true)
                    .override_sync(true)
                    .override_copy(true)
                    .build(),
            )
            .with_type(
                NativeStructBuilder::new::<u64>()
                    .override_send(true)
                    .override_sync(true)
                    .override_copy(true)
                    .build(),
            )
            .with_type(
                NativeStructBuilder::new::<u128>()
                    .override_send(true)
                    .override_sync(true)
                    .override_copy(true)
                    .build(),
            )
            .with_type(
                NativeStructBuilder::new::<usize>()
                    .override_send(true)
                    .override_sync(true)
                    .override_copy(true)
                    .build(),
            )
            .with_type(
                NativeStructBuilder::new::<f32>()
                    .override_send(true)
                    .override_sync(true)
                    .override_copy(true)
                    .build(),
            )
            .with_type(
                NativeStructBuilder::new::<f64>()
                    .override_send(true)
                    .override_sync(true)
                    .override_copy(true)
                    .build(),
            )
            .with_type(
                NativeStructBuilder::new::<char>()
                    .override_send(true)
                    .override_sync(true)
                    .override_copy(true)
                    .build(),
            )
            .with_type(
                NativeStructBuilder::new_named::<String>("String")
                    .override_send(true)
                    .override_sync(true)
                    .build(),
            )
        }
    }

    /// Registers the type-erased value boxes, so scripts can hold values whose
    /// type they do not know.
    pub fn with_erased_types(self) -> Self {
        unsafe {
            self.with_type(
                NativeStructBuilder::new_named_uninitialized::<DynamicManaged>("DynamicManaged")
                    .override_send(true)
                    .override_sync(true)
                    .build(),
            )
            .with_type(
                NativeStructBuilder::new_named_uninitialized::<DynamicManagedLazy>(
                    "DynamicManagedLazy",
                )
                .override_send(true)
                .override_sync(true)
                .build(),
            )
            .with_type(
                NativeStructBuilder::new_named_uninitialized::<DynamicManagedRef>(
                    "DynamicManagedRef",
                )
                .override_send(true)
                .override_sync(true)
                .build(),
            )
            .with_type(
                NativeStructBuilder::new_named_uninitialized::<DynamicManagedRefMut>(
                    "DynamicManagedRefMut",
                )
                .override_send(true)
                .override_sync(true)
                .build(),
            )
            .with_type(
                NativeStructBuilder::new_named_uninitialized::<DynamicManagedGc>(
                    "DynamicManagedGc",
                )
                .override_send(true)
                .override_sync(true)
                .build(),
            )
            .with_type(
                NativeStructBuilder::new_named_uninitialized::<Object>("Object")
                    .override_send(true)
                    .override_sync(true)
                    .build(),
            )
        }
    }

    /// Sets [`Registry::index_capacity`], builder style.
    pub fn with_index_capacity(mut self, capacity: usize) -> Self {
        self.index_capacity = capacity;
        self
    }

    /// Caches every resolved query, never evicting.
    pub fn with_max_index_capacity(mut self) -> Self {
        self.index_capacity = usize::MAX;
        self
    }

    /// Sets [`Registry::use_indexing_threshold`], builder style.
    pub fn with_use_indexing_threshold(mut self, threshold: usize) -> Self {
        self.use_indexing_threshold = threshold;
        self
    }

    /// Runs a setup closure, builder style. Handy for grouping a library's
    /// registrations into one function.
    pub fn with_install(mut self, f: impl FnOnce(&mut Self)) -> Self {
        self.install(f);
        self
    }

    /// Adds a function, builder style.
    pub fn with_function(mut self, function: Function) -> Self {
        self.add_function(function);
        self
    }

    /// Adds a type, builder style.
    pub fn with_type(mut self, type_: impl Into<Type>) -> Self {
        self.add_type(type_);
        self
    }

    /// Runs a setup closure.
    pub fn install(&mut self, f: impl FnOnce(&mut Self)) {
        f(self);
    }

    /// Adds an already shared function, unless one with the same signature is
    /// registered.
    pub fn add_function_handle(&mut self, function_handle: FunctionHandle) {
        if !self
            .functions
            .iter()
            .any(|handle| handle.signature() == function_handle.signature())
        {
            self.functions.push(function_handle);
        }
    }

    /// Adds a function and returns its handle.
    ///
    /// When a function with the same signature is already registered, that one
    /// is returned and the new one is dropped.
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

    /// Removes the function with the same signature as `function_handle`.
    pub fn remove_function(&mut self, function_handle: FunctionHandle) {
        if let Some(position) = self
            .functions
            .iter()
            .position(|handle| handle.signature() == function_handle.signature())
        {
            self.functions.swap_remove(position);
        }
    }

    /// Removes every function the query matches.
    pub fn remove_functions(&mut self, query: FunctionQuery) {
        while let Some(position) = self
            .functions
            .iter()
            .position(|handle| query.is_valid(handle.signature()))
        {
            self.functions.swap_remove(position);
        }
    }

    /// Iterates every registered function.
    pub fn functions(&self) -> impl Iterator<Item = &FunctionHandle> {
        self.functions.iter()
    }

    /// Iterates the functions a query matches, in registration order.
    pub fn find_functions<'a>(
        &'a self,
        query: FunctionQuery<'a>,
    ) -> impl Iterator<Item = FunctionHandle> + 'a {
        self.functions
            .iter()
            .filter(move |handle| query.is_valid(handle.signature()))
            .cloned()
    }

    /// Returns the first function a query matches.
    ///
    /// Goes through the query cache when it is enabled and worth using.
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

    /// Adds an already shared type, unless an equal one is registered.
    pub fn add_type_handle(&mut self, type_handle: TypeHandle) {
        if !self
            .types
            .iter()
            .any(|handle| handle.as_ref() == type_handle.as_ref())
        {
            self.types.push(type_handle);
        }
    }

    /// Adds a type and returns its handle.
    ///
    /// When an equal type is already registered, that one is returned and the
    /// new one is dropped.
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

    /// Removes one type.
    pub fn remove_type(&mut self, type_handle: TypeHandle) {
        if let Some(position) = self.types.iter().position(|handle| handle == &type_handle) {
            self.types.swap_remove(position);
        }
    }

    /// Removes every type the query matches.
    pub fn remove_types(&mut self, query: TypeQuery) {
        while let Some(position) = self.types.iter().position(|handle| query.is_valid(handle)) {
            self.types.swap_remove(position);
        }
    }

    /// Iterates every registered type.
    pub fn types(&self) -> impl Iterator<Item = &TypeHandle> {
        self.types.iter()
    }

    /// Iterates the types a query matches, in registration order.
    pub fn find_types<'a>(&'a self, query: TypeQuery<'a>) -> impl Iterator<Item = TypeHandle> + 'a {
        self.types
            .iter()
            .filter(move |handle| query.is_valid(handle))
            .cloned()
    }

    /// Returns the first type a query matches.
    ///
    /// Goes through the query cache when it is enabled and worth using.
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
        is_async::<String>();
        is_async::<DynamicManaged>();
        is_async::<DynamicManagedLazy>();
        is_async::<DynamicManagedRef>();
        is_async::<DynamicManagedRefMut>();
        is_async::<DynamicManagedGc>();
    }
}
