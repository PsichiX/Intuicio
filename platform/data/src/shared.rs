use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

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
    pub fn new(data: T) -> Self {
        Self {
            data: Rc::new(RefCell::new(data)),
        }
    }

    pub fn try_consume(self) -> Result<T, Self> {
        match Rc::try_unwrap(self.data) {
            Ok(data) => Ok(data.into_inner()),
            Err(data) => Err(Self { data }),
        }
    }

    pub fn read(&self) -> Option<Ref<T>> {
        self.data.try_borrow().ok()
    }

    pub fn write(&self) -> Option<RefMut<T>> {
        self.data.try_borrow_mut().ok()
    }

    pub fn swap(&self, data: T) -> Option<T> {
        let mut value = self.data.try_borrow_mut().ok()?;
        Some(std::mem::replace(&mut value, data))
    }

    pub fn references_count(&self) -> usize {
        Rc::strong_count(&self.data)
    }

    pub fn does_share_reference(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.data, &other.data)
    }
}

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
    pub fn new(data: T) -> Self {
        Self {
            data: Arc::new(RwLock::new(data)),
        }
    }

    pub fn try_consume(self) -> Result<T, Self> {
        match Arc::try_unwrap(self.data) {
            Ok(data) => Ok(data.into_inner().unwrap()),
            Err(data) => Err(Self { data }),
        }
    }

    pub fn read(&self) -> Option<RwLockReadGuard<T>> {
        self.data.read().ok()
    }

    pub fn write(&self) -> Option<RwLockWriteGuard<T>> {
        self.data.write().ok()
    }

    pub fn swap(&self, data: T) -> Option<T> {
        let mut value = self.data.write().ok()?;
        Some(std::mem::replace(&mut value, data))
    }

    pub fn references_count(&self) -> usize {
        Arc::strong_count(&self.data)
    }

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
