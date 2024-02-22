//! Experiments with highly unsafe pointer access.
//! A.k.a. what could go wrong when trying to emulate direct pointer access in scripting.

use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use intuicio_core::{registry::Registry, transformer::ValueTransformer};

pub type VoidPtr = Ptr<()>;

#[repr(transparent)]
pub struct Ptr<T> {
    pointer: *mut T,
}

impl<T> Default for Ptr<T> {
    fn default() -> Self {
        Self {
            pointer: std::ptr::null_mut(),
        }
    }
}

impl<T> Ptr<T> {
    pub fn is_null(self) -> bool {
        self.pointer.is_null()
    }

    pub fn to_ptr(self) -> *const T {
        self.pointer
    }

    pub fn to_ptr_mut(self) -> *mut T {
        self.pointer
    }

    /// # Safety
    pub unsafe fn as_ref(&self) -> Option<&T> {
        if self.is_null() {
            None
        } else {
            Some(&*(self.pointer as *const T))
        }
    }

    /// # Safety
    pub unsafe fn as_ref_mut(&mut self) -> Option<&mut T> {
        if self.is_null() {
            None
        } else {
            Some(&mut *self.pointer)
        }
    }

    /// # Safety
    pub unsafe fn cast<U>(self) -> Ptr<U> {
        Ptr {
            pointer: self.pointer as *mut U,
        }
    }

    /// # Safety
    pub unsafe fn into_box(self) -> Box<T> {
        Box::from_raw(self.pointer)
    }

    /// # Safety
    pub unsafe fn from_box(value: Box<T>) -> Self {
        Self {
            pointer: Box::leak(value) as *mut T,
        }
    }
}

impl<T> From<*mut T> for Ptr<T> {
    fn from(value: *mut T) -> Self {
        Self { pointer: value }
    }
}

impl<T> From<*const T> for Ptr<T> {
    fn from(value: *const T) -> Self {
        Self {
            pointer: value as *mut T,
        }
    }
}

impl<T> From<&mut T> for Ptr<T> {
    fn from(value: &mut T) -> Self {
        Self {
            pointer: value as *mut T,
        }
    }
}

impl<T> From<&T> for Ptr<T> {
    fn from(value: &T) -> Self {
        Self {
            pointer: value as *const T as *mut T,
        }
    }
}

impl<T> From<Ptr<T>> for *const T {
    fn from(value: Ptr<T>) -> Self {
        value.pointer as *const T
    }
}

impl<T> From<Ptr<T>> for *mut T {
    fn from(value: Ptr<T>) -> Self {
        value.pointer
    }
}

impl<T> Deref for Ptr<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.as_ref().expect("Trying to dereference null pointer!") }
    }
}

impl<T> DerefMut for Ptr<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            self.as_ref_mut()
                .expect("Trying to dereference null pointer!")
        }
    }
}

impl<T> Copy for Ptr<T> {}

impl<T> Clone for Ptr<T> {
    fn clone(&self) -> Self {
        *self
    }
}

// NOTE: I know this is bad, don't kill me - again, it's for experiments only sake,
// some day it might disappear in favor of some smarter solution.
unsafe impl<T> Send for Ptr<T> where T: Send {}
unsafe impl<T> Sync for Ptr<T> where T: Sync {}

impl<T> std::fmt::Debug for Ptr<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.pointer)
    }
}

impl<T> std::fmt::Display for Ptr<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.pointer)
    }
}

pub struct PtrValueTransformer<T: Default + Clone + 'static>(PhantomData<fn() -> T>);

impl<T: Default + Clone + 'static> ValueTransformer for PtrValueTransformer<T> {
    type Type = T;
    type Borrow<'r> = &'r T;
    type BorrowMut<'r> = &'r mut T;
    type Dependency = ();
    type Owned = T;
    type Ref = Ptr<T>;
    type RefMut = Ptr<T>;

    fn from_owned(_: &Registry, value: Self::Type) -> Self::Owned {
        value
    }

    fn from_ref(_: &Registry, value: &Self::Type, _: Option<Self::Dependency>) -> Self::Ref {
        Ptr::from(value)
    }

    fn from_ref_mut(
        _: &Registry,
        value: &mut Self::Type,
        _: Option<Self::Dependency>,
    ) -> Self::RefMut {
        Ptr::from(value)
    }

    fn into_owned(value: Self::Owned) -> Self::Type {
        value
    }

    fn into_ref(value: &Self::Ref) -> Self::Borrow<'_> {
        unsafe { value.as_ref().unwrap() }
    }

    fn into_ref_mut(value: &mut Self::RefMut) -> Self::BorrowMut<'_> {
        unsafe { value.as_ref_mut().unwrap() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use intuicio_core::prelude::*;
    use intuicio_derive::intuicio_function;

    #[test]
    fn test_async() {
        fn is_async<T: Send + Sync>() {}

        is_async::<Ptr<usize>>();
        is_async::<Ptr<Ptr<usize>>>();
    }

    #[intuicio_function(transformer = "PtrValueTransformer")]
    fn add(a: &usize, b: &mut usize) -> usize {
        *a + *b
    }

    #[test]
    fn test_raw_pointer_on_stack() {
        let mut registry = Registry::default().with_basic_types();
        registry.add_struct(define_native_struct! {
            registry => struct (Ptr<usize>) {}
        });
        let add = registry.add_function(add::define_function(&registry));
        let mut context = Context::new(1024, 1024, 1024);
        let a = 40usize;
        let mut b = 2usize;
        let (r,) = add.call::<(usize,), _>(
            &mut context,
            &registry,
            (Ptr::from(&a), Ptr::from(&mut b)),
            true,
        );
        assert_eq!(r, 42);
    }

    #[test]
    fn test_allocation() {
        unsafe {
            let a = Box::new(42usize);
            let mut b = Ptr::from_box(a);
            *b.as_ref_mut().unwrap() = 10;
            let c = b.into_box();
            let d = *c;
            assert_eq!(d, 10);
        }
    }
}
