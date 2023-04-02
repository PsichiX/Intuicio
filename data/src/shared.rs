use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
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
    }
}
