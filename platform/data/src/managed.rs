use crate::lifetime::{Lifetime, LifetimeRef, LifetimeRefMut, ValueReadAccess, ValueWriteAccess};
use std::ptr::NonNull;

#[derive(Default)]
pub struct Managed<T> {
    lifetime: Lifetime,
    data: T,
}

impl<T> Managed<T> {
    pub fn new(data: T) -> Self {
        Self {
            lifetime: Default::default(),
            data,
        }
    }

    pub fn renew(mut self) -> Self {
        self.lifetime = Lifetime::default();
        self
    }

    pub fn lifetime(&self) -> &Lifetime {
        &self.lifetime
    }

    pub fn read(&self) -> Option<ValueReadAccess<T>> {
        self.lifetime.read(&self.data)
    }

    pub fn write(&mut self) -> Option<ValueWriteAccess<T>> {
        self.lifetime.write(&mut self.data)
    }

    pub fn consume(self) -> Result<T, Self> {
        if self.lifetime.state().is_in_use() {
            Err(self)
        } else {
            Ok(self.data)
        }
    }

    pub fn borrow(&self) -> Option<ManagedRef<T>> {
        Some(ManagedRef::new(&self.data, self.lifetime.borrow()?))
    }

    pub fn borrow_mut(&mut self) -> Option<ManagedRefMut<T>> {
        Some(ManagedRefMut::new(
            &mut self.data,
            self.lifetime.borrow_mut()?,
        ))
    }
}

pub struct ManagedRef<T> {
    lifetime: LifetimeRef,
    data: NonNull<T>,
}

unsafe impl<T> Send for ManagedRef<T> where T: Send {}

impl<T> ManagedRef<T> {
    pub fn new(data: &T, lifetime: LifetimeRef) -> Self {
        Self {
            lifetime,
            data: unsafe { NonNull::new_unchecked(data as *const T as *mut T) },
        }
    }

    /// # Safety
    pub unsafe fn new_raw(data: *const T, lifetime: LifetimeRef) -> Self {
        Self {
            lifetime,
            data: NonNull::new_unchecked(data as *mut _),
        }
    }

    pub fn lifetime(&self) -> &LifetimeRef {
        &self.lifetime
    }

    pub fn borrow(&self) -> Option<ManagedRef<T>> {
        Some(ManagedRef {
            lifetime: self.lifetime.borrow()?,
            data: self.data,
        })
    }

    pub fn read(&self) -> Option<ValueReadAccess<T>> {
        self.lifetime.read(unsafe { self.data.as_ref() })
    }

    /// # Safety
    pub unsafe fn as_ptr(&self) -> Option<*const T> {
        if self.lifetime.exists() {
            Some(self.data.as_ptr())
        } else {
            None
        }
    }
}

pub struct ManagedRefMut<T> {
    lifetime: LifetimeRefMut,
    data: NonNull<T>,
}
unsafe impl<T> Send for ManagedRefMut<T> where T: Send {}

impl<T> ManagedRefMut<T> {
    pub fn new(data: &mut T, lifetime: LifetimeRefMut) -> Self {
        Self {
            lifetime,
            data: unsafe { NonNull::new_unchecked(data as *mut T) },
        }
    }

    /// # Safety
    pub unsafe fn new_raw(data: *mut T, lifetime: LifetimeRefMut) -> Self {
        Self {
            lifetime,
            data: NonNull::new_unchecked(data),
        }
    }

    pub fn lifetime(&self) -> &LifetimeRefMut {
        &self.lifetime
    }

    pub fn borrow(&self) -> Option<ManagedRef<T>> {
        Some(ManagedRef {
            lifetime: self.lifetime.borrow()?,
            data: self.data,
        })
    }

    pub fn borrow_mut(&self) -> Option<ManagedRefMut<T>> {
        Some(ManagedRefMut {
            lifetime: self.lifetime.borrow_mut()?,
            data: self.data,
        })
    }

    pub fn read(&self) -> Option<ValueReadAccess<T>> {
        self.lifetime.read(unsafe { self.data.as_ref() })
    }

    pub fn write(&mut self) -> Option<ValueWriteAccess<T>> {
        self.lifetime.write(unsafe { self.data.as_mut() })
    }

    /// # Safety
    pub unsafe fn as_ptr(&self) -> Option<*const T> {
        if self.lifetime.exists() {
            Some(self.data.as_ptr())
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn as_mut_ptr(&mut self) -> Option<*mut T> {
        if self.lifetime.exists() {
            Some(self.data.as_ptr())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::managed::{Managed, ManagedRef, ManagedRefMut};

    fn is_async<T: Send + 'static>() {
        println!("{} is send!", std::any::type_name::<T>());
    }

    #[test]
    fn test_managed() {
        is_async::<Managed<()>>();
        is_async::<ManagedRef<()>>();
        is_async::<ManagedRefMut<()>>();

        let mut value = Managed::new(42);
        let value_ref = value.borrow().unwrap();
        assert!(value.borrow().is_some());
        assert!(value.borrow_mut().is_none());
        drop(value_ref);
        assert!(value.borrow().is_some());
        assert!(value.borrow_mut().is_some());
        *value.write().unwrap() = 40;
        assert_eq!(*value.read().unwrap(), 40);
        *value.borrow_mut().unwrap().write().unwrap() = 2;
        assert_eq!(*value.read().unwrap(), 2);
        let value_ref = value.borrow().unwrap();
        drop(value);
        assert!(value_ref.read().is_none());
    }
}
