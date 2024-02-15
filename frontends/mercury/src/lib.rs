use std::ops::{Deref, DerefMut};

pub type VoidPtr = Ptr<()>;

// Experiments with highly unsafe pointer access.
// A.k.a. what could go wrong when trying to emulate direct pointer access.
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

#[cfg(test)]
mod tests {
    use super::*;
    use intuicio_core::prelude::*;

    #[test]
    fn test_async() {
        fn is_async<T: Send + Sync>() {}

        is_async::<Ptr<usize>>();
        is_async::<Ptr<Ptr<usize>>>();
    }

    #[test]
    fn test_raw_pointers() {
        let mut registry = Registry::default().with_basic_types();
        registry.add_struct(define_native_struct! {
            registry => struct (Ptr<usize>) {}
        });
        let add = registry.add_function(define_function! {
            registry => mod intrinsics fn add(a: Ptr<usize>, b: Ptr<usize>) -> (result: usize) {
                (*a + *b,)
            }
        });
        let mut context = Context::new(1024, 1024, 1024);
        let a = 40usize;
        let b = 2usize;
        context.stack().push(Ptr::from(&b));
        context.stack().push(Ptr::from(&a));
        add.invoke(&mut context, &registry);
        assert_eq!(context.stack().pop::<usize>().unwrap(), 42);
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
