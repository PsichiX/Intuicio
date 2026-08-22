//! Shared ownership wrappers with fallible, non-panicking access.
//!
//! [`Shared`] is single threaded, [`AsyncShared`] is the thread safe
//! counterpart. Both return [`None`] instead of panicking when the value is
//! already borrowed. A script runtime has to report such an error, not abort
//! the host.
use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

/// Single threaded shared value, a thin wrapper over `Rc<RefCell<T>>`.
///
/// Unlike [`RefCell`] directly, borrowing never panics: [`Shared::read`] and
/// [`Shared::write`] return [`None`] when the value is already borrowed the
/// other way.
#[derive(Default)]
pub struct Shared<T> {
    data: Rc<RefCell<T>>,
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
        }
    }
}

impl<T> Shared<T> {
    /// Wraps a value into a new shared cell with one reference.
    pub fn new(data: T) -> Self {
        Self {
            data: Rc::new(RefCell::new(data)),
        }
    }

    /// Unwraps the value when this is the last reference, otherwise gives the
    /// handle back untouched.
    pub fn try_consume(self) -> Result<T, Self> {
        match Rc::try_unwrap(self.data) {
            Ok(data) => Ok(data.into_inner()),
            Err(data) => Err(Self { data }),
        }
    }

    /// Borrows the value immutably, or returns [`None`] when it is already
    /// borrowed mutably.
    pub fn read(&'_ self) -> Option<Ref<'_, T>> {
        self.data.try_borrow().ok()
    }

    /// Borrows the value mutably, or returns [`None`] when it is already
    /// borrowed.
    pub fn write(&'_ self) -> Option<RefMut<'_, T>> {
        self.data.try_borrow_mut().ok()
    }

    /// Replaces the value and returns the old one, or [`None`] when it is
    /// already borrowed.
    pub fn swap(&self, data: T) -> Option<T> {
        let mut value = self.data.try_borrow_mut().ok()?;
        Some(std::mem::replace(&mut value, data))
    }

    /// Returns how many handles point at this value.
    pub fn references_count(&self) -> usize {
        Rc::strong_count(&self.data)
    }

    /// Returns `true` when both handles point at the same value.
    pub fn does_share_reference(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.data, &other.data)
    }
}

/// Thread safe shared value, a thin wrapper over `Arc<RwLock<T>>`.
///
/// The [`Shared`] counterpart for values that cross threads. Access methods
/// return [`None`] when the lock is poisoned or cannot be taken.
#[derive(Default)]
pub struct AsyncShared<T> {
    data: Arc<RwLock<T>>,
}

impl<T> Clone for AsyncShared<T> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
        }
    }
}

impl<T> AsyncShared<T> {
    /// Wraps a value into a new shared cell with one reference.
    pub fn new(data: T) -> Self {
        Self {
            data: Arc::new(RwLock::new(data)),
        }
    }

    /// Unwraps the value when this is the last reference, otherwise gives the
    /// handle back untouched.
    pub fn try_consume(self) -> Result<T, Self> {
        match Arc::try_unwrap(self.data) {
            Ok(data) => Ok(data.into_inner().unwrap()),
            Err(data) => Err(Self { data }),
        }
    }

    /// Takes a read lock, blocking until it is free, or returns [`None`] when
    /// the lock is poisoned.
    pub fn read(&'_ self) -> Option<RwLockReadGuard<'_, T>> {
        self.data.read().ok()
    }

    /// Takes a write lock, blocking until it is free, or returns [`None`] when
    /// the lock is poisoned.
    pub fn write(&'_ self) -> Option<RwLockWriteGuard<'_, T>> {
        self.data.write().ok()
    }

    /// Replaces the value and returns the old one, or [`None`] when the lock is
    /// poisoned.
    pub fn swap(&self, data: T) -> Option<T> {
        let mut value = self.data.write().ok()?;
        Some(std::mem::replace(&mut value, data))
    }

    /// Returns how many handles point at this value.
    pub fn references_count(&self) -> usize {
        Arc::strong_count(&self.data)
    }

    /// Returns `true` when both handles point at the same value.
    pub fn does_share_reference(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
    }
}

#[cfg(test)]
mod tests {
    use super::Shared;

    #[test]
    fn test_shared() {
        let a = Shared::new(42);
        assert_eq!(a.references_count(), 1);
        assert_eq!(*a.read().unwrap(), 42);
        let b = a.clone();
        assert_eq!(a.references_count(), 2);
        assert_eq!(b.references_count(), 2);
        assert_eq!(*b.read().unwrap(), 42);
        *b.write().unwrap() = 10;
        assert_eq!(*a.read().unwrap(), 10);
        assert_eq!(*b.read().unwrap(), 10);
        assert!(b.try_consume().is_err());
        assert_eq!(a.try_consume().ok().unwrap(), 10);
    }
}
