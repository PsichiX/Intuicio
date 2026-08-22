//! Raw pointers passed to and from scripts.
//!
//! **Experimental.** Nothing here tracks lifetimes or aliasing. A [`Ptr`] is a
//! bare pointer with a nicer shape, so a script can hold one long after the
//! value it points at is gone. Use the managed boxes of `intuicio-data` unless
//! direct pointer access is really what you need.
//!
//! [`PtrValueTransformer`] is what plugs this into a native function: `&T` and
//! `&mut T` arguments travel as [`Ptr<T>`], and owned values travel as
//! themselves.

use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use intuicio_core::{registry::Registry, transformer::ValueTransformer};

/// A pointer with the type thrown away.
pub type VoidPtr = Ptr<()>;

/// A raw pointer that can be stored in a script value.
///
/// `Copy`, and null by default. Dereferencing goes through [`Deref`], which
/// panics on a null pointer and checks nothing else.
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
    /// Returns `true` when this pointer is null.
    pub fn is_null(self) -> bool {
        self.pointer.is_null()
    }

    /// Returns the address as a shared raw pointer.
    pub fn to_ptr(self) -> *const T {
        self.pointer
    }

    /// Returns the address as a mutable raw pointer.
    pub fn to_ptr_mut(self) -> *mut T {
        self.pointer
    }

    /// Borrows the value, or returns [`None`] for a null pointer.
    ///
    /// # Safety
    ///
    /// The pointer must still name a live, initialized `T`, and nothing else
    /// may write to it while the borrow lives.
    pub unsafe fn as_ref(&self) -> Option<&T> {
        if self.is_null() {
            None
        } else {
            Some(unsafe { &*(self.pointer as *const T) })
        }
    }

    /// Borrows the value mutably, or returns [`None`] for a null pointer.
    ///
    /// # Safety
    ///
    /// The pointer must still name a live, initialized `T`, and nothing else
    /// may touch it while the borrow lives.
    pub unsafe fn as_ref_mut(&mut self) -> Option<&mut T> {
        if self.is_null() {
            None
        } else {
            Some(unsafe { &mut *self.pointer })
        }
    }

    /// Reads the same address as a `Ptr<U>`.
    ///
    /// # Safety
    ///
    /// Every later read or write goes through `U`, so the address must really
    /// hold a `U`.
    pub unsafe fn cast<U>(self) -> Ptr<U> {
        Ptr {
            pointer: self.pointer as *mut U,
        }
    }

    /// Takes the allocation back into a [`Box`].
    ///
    /// # Safety
    ///
    /// The pointer must come from [`Ptr::from_box`] and must not have been
    /// taken back already. The box frees the allocation on drop, so every other
    /// copy of this pointer goes stale.
    pub unsafe fn into_box(self) -> Box<T> {
        unsafe { Box::from_raw(self.pointer) }
    }

    /// Leaks a [`Box`] and keeps its address.
    ///
    /// # Safety
    ///
    /// The allocation leaks unless [`Ptr::into_box`] takes it back exactly
    /// once.
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

// Safety: a `Ptr` owns nothing and points at a value that lives elsewhere, so
// it can cross threads whenever that value can. Nothing checks that the value
// is still alive, which is the risk this whole crate takes.
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

/// Passes references to scripts as [`Ptr`], and owned values as themselves.
///
/// The script side gets no borrow checking at all. Reading through a stale
/// pointer is undefined, and a null one panics.
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
    use intuicio_core::{context::Context, define_native_struct};
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
        registry.add_type(define_native_struct! {
            registry => struct (Ptr<usize>) {}
        });
        let add = registry.add_function(add::define_function(&registry));
        let mut context = Context::new(10240, 10240);
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
