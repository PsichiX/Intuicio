use std::{
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Weak,
    },
};

#[derive(Default, Clone)]
pub struct LifetimeState {
    locked: Arc<AtomicBool>,
    readers: Arc<AtomicUsize>,
    writer: Arc<AtomicUsize>,
    read_access: Arc<AtomicUsize>,
    write_access: Arc<AtomicBool>,
}

impl LifetimeState {
    pub fn can_read(&self) -> bool {
        self.writer.load(Ordering::Acquire) == 0
    }

    pub fn can_write(&self, id: usize) -> bool {
        self.writer.load(Ordering::Acquire) == id && self.readers.load(Ordering::Acquire) == 0
    }

    pub fn writer_depth(&self) -> usize {
        self.writer.load(Ordering::Acquire)
    }

    pub fn is_read_accessible(&self) -> bool {
        !self.write_access.load(Ordering::Acquire)
    }

    pub fn is_write_accessible(&self) -> bool {
        !self.write_access.load(Ordering::Acquire) && self.read_access.load(Ordering::Acquire) == 0
    }

    pub fn is_in_use(&self) -> bool {
        self.read_access.load(Ordering::Acquire) > 0 || self.write_access.load(Ordering::Acquire)
    }

    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Acquire)
    }

    pub fn try_lock(&self) -> Option<LifetimeStateAccess> {
        if !self.locked.swap(true, Ordering::AcqRel) {
            Some(LifetimeStateAccess {
                state: self,
                unlock: true,
            })
        } else {
            None
        }
    }

    pub fn lock(&self) -> LifetimeStateAccess {
        while self.locked.load(Ordering::Acquire) {}
        self.locked.store(true, Ordering::Release);
        LifetimeStateAccess {
            state: self,
            unlock: true,
        }
    }

    pub fn lock_unchecked(&self) -> LifetimeStateAccess {
        LifetimeStateAccess {
            state: self,
            unlock: true,
        }
    }

    pub fn downgrade(&self) -> LifetimeWeakState {
        LifetimeWeakState {
            locked: Arc::downgrade(&self.locked),
            readers: Arc::downgrade(&self.readers),
            writer: Arc::downgrade(&self.writer),
            read_access: Arc::downgrade(&self.read_access),
            write_access: Arc::downgrade(&self.write_access),
        }
    }
}

#[derive(Clone)]
pub struct LifetimeWeakState {
    locked: Weak<AtomicBool>,
    readers: Weak<AtomicUsize>,
    writer: Weak<AtomicUsize>,
    read_access: Weak<AtomicUsize>,
    write_access: Weak<AtomicBool>,
}

impl LifetimeWeakState {
    pub fn upgrade(&self) -> Option<LifetimeState> {
        Some(LifetimeState {
            locked: self.locked.upgrade()?,
            readers: self.readers.upgrade()?,
            writer: self.writer.upgrade()?,
            read_access: self.read_access.upgrade()?,
            write_access: self.write_access.upgrade()?,
        })
    }

    pub fn is_owned_by(&self, state: &LifetimeState) -> bool {
        Arc::downgrade(&state.locked).ptr_eq(&self.locked)
    }
}

pub struct LifetimeStateAccess<'a> {
    state: &'a LifetimeState,
    unlock: bool,
}

impl<'a> Drop for LifetimeStateAccess<'a> {
    fn drop(&mut self) {
        if self.unlock {
            self.state.locked.store(false, Ordering::Release);
        }
    }
}

impl<'a> LifetimeStateAccess<'a> {
    pub fn state(&self) -> &LifetimeState {
        self.state
    }

    pub fn unlock(&mut self, value: bool) {
        self.unlock = value;
    }

    pub fn acquire_reader(&mut self) {
        let v = self.state.readers.load(Ordering::Acquire) + 1;
        self.state.readers.store(v, Ordering::Release);
    }

    pub fn release_reader(&mut self) {
        let v = self.state.readers.load(Ordering::Acquire).saturating_sub(1);
        self.state.readers.store(v, Ordering::Release);
    }

    #[must_use]
    pub fn acquire_writer(&mut self) -> usize {
        let v = self.state.writer.load(Ordering::Acquire) + 1;
        self.state.writer.store(v, Ordering::Release);
        v
    }

    pub fn release_writer(&mut self, id: usize) {
        let v = self.state.writer.load(Ordering::Acquire);
        if id <= v {
            self.state
                .writer
                .store(id.saturating_sub(1), Ordering::Release);
        }
    }

    pub fn acquire_read_access(&mut self) {
        let v = self.state.read_access.load(Ordering::Acquire) + 1;
        self.state.read_access.store(v, Ordering::Release);
    }

    pub fn release_read_access(&mut self) {
        let v = self
            .state
            .read_access
            .load(Ordering::Acquire)
            .saturating_sub(1);
        self.state.read_access.store(v, Ordering::Release);
    }

    pub fn acquire_write_access(&mut self) {
        self.state.write_access.store(true, Ordering::Release);
    }

    pub fn release_write_access(&mut self) {
        self.state.write_access.store(false, Ordering::Release);
    }
}

#[derive(Default)]
pub struct Lifetime(LifetimeState);

impl Lifetime {
    pub fn state(&self) -> &LifetimeState {
        &self.0
    }

    pub fn borrow(&self) -> Option<LifetimeRef> {
        self.0
            .try_lock()
            .filter(|access| access.state.can_read())
            .map(|mut access| {
                access.acquire_reader();
                LifetimeRef(self.0.downgrade())
            })
    }

    pub fn borrow_mut(&self) -> Option<LifetimeRefMut> {
        self.0
            .try_lock()
            .filter(|access| access.state.can_write(0))
            .map(|mut access| {
                let id = access.acquire_writer();
                LifetimeRefMut(self.0.downgrade(), id)
            })
    }

    pub fn lazy(&self) -> LifetimeLazy {
        LifetimeLazy(self.0.downgrade())
    }

    pub fn read<'a, T>(&'a self, data: &'a T) -> Option<ValueReadAccess<'a, T>> {
        self.0
            .try_lock()
            .filter(|access| access.state.is_read_accessible())
            .map(|mut access| {
                access.unlock = false;
                access.acquire_read_access();
                ValueReadAccess {
                    lifetime: self.0.clone(),
                    data,
                }
            })
    }

    pub fn write<'a, T>(&'a self, data: &'a mut T) -> Option<ValueWriteAccess<'a, T>> {
        self.0
            .try_lock()
            .filter(|access| access.state.is_write_accessible())
            .map(|mut access| {
                access.unlock = false;
                access.acquire_write_access();
                ValueWriteAccess {
                    lifetime: self.0.clone(),
                    data,
                }
            })
    }
}

pub struct LifetimeRef(LifetimeWeakState);

impl Drop for LifetimeRef {
    fn drop(&mut self) {
        if let Some(owner) = self.0.upgrade() {
            if let Some(mut access) = owner.try_lock() {
                access.release_reader();
            }
        }
    }
}

impl LifetimeRef {
    pub fn state(&self) -> &LifetimeWeakState {
        &self.0
    }

    pub fn exists(&self) -> bool {
        self.0.upgrade().is_some()
    }

    pub fn can_read(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.can_read())
            .unwrap_or(false)
    }

    pub fn is_read_accessible(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.is_read_accessible())
            .unwrap_or(false)
    }

    pub fn is_in_use(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.is_in_use())
            .unwrap_or(false)
    }

    pub fn is_owned_by(&self, other: &Lifetime) -> bool {
        self.0.is_owned_by(&other.0)
    }

    pub fn borrow(&self) -> Option<LifetimeRef> {
        self.0
            .upgrade()?
            .try_lock()
            .filter(|access| access.state.can_read())
            .map(|mut access| {
                access.acquire_reader();
                LifetimeRef(self.0.clone())
            })
    }

    pub fn read<'a, T>(&'a self, data: &'a T) -> Option<ValueReadAccess<'a, T>> {
        let state = self.0.upgrade()?;
        let mut access = state.try_lock()?;
        if access.state.is_read_accessible() {
            access.unlock = false;
            access.acquire_read_access();
            drop(access);
            Some(ValueReadAccess {
                lifetime: state,
                data,
            })
        } else {
            None
        }
    }

    pub fn consume<T>(self, data: &T) -> Result<ValueReadAccess<T>, Self> {
        let state = match self.0.upgrade() {
            Some(state) => state,
            None => return Err(self),
        };
        let mut access = match state.try_lock() {
            Some(access) => access,
            None => return Err(self),
        };
        if access.state.is_read_accessible() {
            access.unlock = false;
            access.acquire_read_access();
            drop(access);
            Ok(ValueReadAccess {
                lifetime: state,
                data,
            })
        } else {
            Err(self)
        }
    }
}

pub struct LifetimeRefMut(LifetimeWeakState, usize);

impl Drop for LifetimeRefMut {
    fn drop(&mut self) {
        if let Some(state) = self.0.upgrade() {
            if let Some(mut access) = state.try_lock() {
                access.release_writer(self.1);
            }
        }
    }
}

impl LifetimeRefMut {
    pub fn state(&self) -> &LifetimeWeakState {
        &self.0
    }

    pub fn depth(&self) -> usize {
        self.1
    }

    pub fn exists(&self) -> bool {
        self.0.upgrade().is_some()
    }

    pub fn can_read(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.can_read())
            .unwrap_or(false)
    }

    pub fn can_write(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.can_write(self.1))
            .unwrap_or(false)
    }

    pub fn is_read_accessible(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.is_read_accessible())
            .unwrap_or(false)
    }

    pub fn is_write_accessible(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.is_write_accessible())
            .unwrap_or(false)
    }

    pub fn is_in_use(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.is_in_use())
            .unwrap_or(false)
    }

    pub fn is_owned_by(&self, other: &Lifetime) -> bool {
        self.0.is_owned_by(&other.0)
    }

    pub fn borrow(&self) -> Option<LifetimeRef> {
        self.0
            .upgrade()?
            .try_lock()
            .filter(|access| access.state.can_read())
            .map(|mut access| {
                access.acquire_reader();
                LifetimeRef(self.0.clone())
            })
    }

    pub fn borrow_mut(&self) -> Option<LifetimeRefMut> {
        self.0
            .upgrade()?
            .try_lock()
            .filter(|access| access.state.can_write(self.1))
            .map(|mut access| {
                let id = access.acquire_writer();
                LifetimeRefMut(self.0.clone(), id)
            })
    }

    pub fn read<'a, T>(&'a self, data: &'a T) -> Option<ValueReadAccess<'a, T>> {
        let state = self.0.upgrade()?;
        let mut access = state.try_lock()?;
        if access.state.is_read_accessible() {
            access.unlock = false;
            access.acquire_read_access();
            drop(access);
            Some(ValueReadAccess {
                lifetime: state,
                data,
            })
        } else {
            None
        }
    }

    pub fn write<'a, T>(&'a self, data: &'a mut T) -> Option<ValueWriteAccess<'a, T>> {
        let state = self.0.upgrade()?;
        let mut access = state.try_lock()?;
        if access.state.is_write_accessible() {
            access.unlock = false;
            access.acquire_write_access();
            drop(access);
            Some(ValueWriteAccess {
                lifetime: state,
                data,
            })
        } else {
            None
        }
    }

    pub fn consume<T>(self, data: &mut T) -> Result<ValueWriteAccess<T>, Self> {
        let state = match self.0.upgrade() {
            Some(state) => state,
            None => return Err(self),
        };
        let mut access = match state.try_lock() {
            Some(access) => access,
            None => return Err(self),
        };
        if access.state.is_write_accessible() {
            access.unlock = false;
            access.acquire_write_access();
            drop(access);
            Ok(ValueWriteAccess {
                lifetime: state,
                data,
            })
        } else {
            Err(self)
        }
    }
}

#[derive(Clone)]
pub struct LifetimeLazy(LifetimeWeakState);

impl LifetimeLazy {
    pub fn state(&self) -> &LifetimeWeakState {
        &self.0
    }

    pub fn exists(&self) -> bool {
        self.0.upgrade().is_some()
    }

    pub fn is_read_accessible(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.is_read_accessible())
            .unwrap_or(false)
    }

    pub fn is_write_accessible(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.is_write_accessible())
            .unwrap_or(false)
    }

    pub fn is_in_use(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.is_in_use())
            .unwrap_or(false)
    }

    pub fn is_owned_by(&self, other: &Lifetime) -> bool {
        self.0.is_owned_by(&other.0)
    }

    pub fn borrow(&self) -> Option<LifetimeRef> {
        self.0
            .upgrade()?
            .try_lock()
            .filter(|access| access.state.can_read())
            .map(|mut access| {
                access.acquire_reader();
                LifetimeRef(self.0.clone())
            })
    }

    pub fn borrow_mut(&self) -> Option<LifetimeRefMut> {
        self.0
            .upgrade()?
            .try_lock()
            .filter(|access| access.state.can_write(0))
            .map(|mut access| {
                let id = access.acquire_writer();
                LifetimeRefMut(self.0.clone(), id)
            })
    }

    pub fn read<'a, T>(&'a self, data: &'a T) -> Option<ValueReadAccess<'a, T>> {
        let state = self.0.upgrade()?;
        let mut access = state.try_lock()?;
        if access.state.is_read_accessible() {
            access.unlock = false;
            access.acquire_read_access();
            drop(access);
            Some(ValueReadAccess {
                lifetime: state,
                data,
            })
        } else {
            None
        }
    }

    pub fn write<'a, T>(&'a self, data: &'a mut T) -> Option<ValueWriteAccess<'a, T>> {
        let state = self.0.upgrade()?;
        let mut access = state.try_lock()?;
        if access.state.is_write_accessible() {
            access.unlock = false;
            access.acquire_write_access();
            drop(access);
            Some(ValueWriteAccess {
                lifetime: state,
                data,
            })
        } else {
            None
        }
    }

    pub fn consume<T>(self, data: &mut T) -> Result<ValueWriteAccess<T>, Self> {
        let state = match self.0.upgrade() {
            Some(state) => state,
            None => return Err(self),
        };
        let mut access = match state.try_lock() {
            Some(access) => access,
            None => return Err(self),
        };
        if access.state.is_write_accessible() {
            access.unlock = false;
            access.acquire_write_access();
            drop(access);
            Ok(ValueWriteAccess {
                lifetime: state,
                data,
            })
        } else {
            Err(self)
        }
    }
}

pub struct ValueReadAccess<'a, T: 'a> {
    lifetime: LifetimeState,
    data: &'a T,
}

impl<'a, T> Drop for ValueReadAccess<'a, T> {
    fn drop(&mut self) {
        self.lifetime.lock_unchecked().release_read_access();
    }
}

impl<'a, T> ValueReadAccess<'a, T> {
    /// # Safety
    pub unsafe fn new_raw(data: &'a T, lifetime: LifetimeState) -> Self {
        Self { lifetime, data }
    }
}

impl<'a, T> Deref for ValueReadAccess<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a, T> ValueReadAccess<'a, T> {
    pub fn remap<U>(
        self,
        f: impl FnOnce(&T) -> Option<&U>,
    ) -> Result<ValueReadAccess<'a, U>, Self> {
        if let Some(data) = f(self.data) {
            Ok(ValueReadAccess {
                lifetime: self.lifetime.clone(),
                data,
            })
        } else {
            Err(self)
        }
    }
}

pub struct ValueWriteAccess<'a, T: 'a> {
    lifetime: LifetimeState,
    data: &'a mut T,
}

impl<'a, T> Drop for ValueWriteAccess<'a, T> {
    fn drop(&mut self) {
        self.lifetime.lock_unchecked().release_write_access();
    }
}

impl<'a, T> ValueWriteAccess<'a, T> {
    /// # Safety
    pub unsafe fn new_raw(data: &'a mut T, lifetime: LifetimeState) -> Self {
        Self { lifetime, data }
    }
}

impl<'a, T> Deref for ValueWriteAccess<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a, T> DerefMut for ValueWriteAccess<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

impl<'a, T> ValueWriteAccess<'a, T> {
    pub fn remap<U>(
        self,
        f: impl FnOnce(&mut T) -> Option<&mut U>,
    ) -> Result<ValueWriteAccess<'a, U>, Self> {
        if let Some(data) = f(unsafe { std::mem::transmute(&mut *self.data) }) {
            Ok(ValueWriteAccess {
                lifetime: self.lifetime.clone(),
                data,
            })
        } else {
            Err(self)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::*;

    fn is_async<T: Send + Sync + 'static>() {
        println!("{} is async!", std::any::type_name::<T>());
    }

    #[test]
    fn test_lifetimes() {
        is_async::<Lifetime>();
        is_async::<LifetimeRef>();
        is_async::<LifetimeRefMut>();
        is_async::<LifetimeLazy>();

        let mut value = 0usize;
        let lifetime_ref = {
            let lifetime = Lifetime::default();
            assert!(lifetime.state().can_read());
            assert!(lifetime.state().can_write(0));
            assert!(lifetime.state().is_read_accessible());
            assert!(lifetime.state().is_write_accessible());
            let lifetime_lazy = lifetime.lazy();
            assert!(lifetime_lazy.read(&42).is_some());
            assert!(lifetime_lazy.write(&mut 42).is_some());
            {
                let access = lifetime.read(&value).unwrap();
                assert_eq!(*access, value);
            }
            {
                let mut access = lifetime.write(&mut value).unwrap();
                *access = 42;
                assert_eq!(*access, 42);
            }
            {
                let lifetime_ref = lifetime.borrow().unwrap();
                assert!(lifetime.state().can_read());
                assert!(!lifetime.state().can_write(0));
                assert!(lifetime_ref.exists());
                assert!(lifetime_ref.is_owned_by(&lifetime));
                assert!(lifetime.borrow().is_some());
                assert!(lifetime.borrow_mut().is_none());
                assert!(lifetime_lazy.read(&42).is_some());
                assert!(lifetime_lazy.write(&mut 42).is_some());
                {
                    let access = lifetime_ref.read(&value).unwrap();
                    assert_eq!(*access, 42);
                    assert!(lifetime_lazy.read(&42).is_none());
                    assert!(lifetime_lazy.write(&mut 42).is_none());
                }
                let lifetime_ref2 = lifetime_ref.borrow().unwrap();
                {
                    let access = lifetime_ref2.read(&value).unwrap();
                    assert_eq!(*access, 42);
                    assert!(lifetime_lazy.read(&42).is_none());
                    assert!(lifetime_lazy.write(&mut 42).is_none());
                }
            }
            {
                let lifetime_ref_mut = lifetime.borrow_mut().unwrap();
                assert_eq!(lifetime.state().writer_depth(), 1);
                assert!(!lifetime.state().can_read());
                assert!(!lifetime.state().can_write(0));
                assert!(lifetime_ref_mut.exists());
                assert!(lifetime_ref_mut.is_owned_by(&lifetime));
                assert!(lifetime.borrow().is_none());
                assert!(lifetime.borrow_mut().is_none());
                assert!(lifetime_lazy.read(&42).is_some());
                assert!(lifetime_lazy.write(&mut 42).is_some());
                {
                    let mut access = lifetime_ref_mut.write(&mut value).unwrap();
                    *access = 7;
                    assert_eq!(*access, 7);
                    assert!(lifetime_lazy.read(&42).is_none());
                    assert!(lifetime_lazy.write(&mut 42).is_none());
                }
                let lifetime_ref_mut2 = lifetime_ref_mut.borrow_mut().unwrap();
                assert!(lifetime_lazy.read(&42).is_some());
                assert!(lifetime_lazy.write(&mut 42).is_some());
                {
                    assert_eq!(lifetime.state().writer_depth(), 2);
                    assert!(lifetime.borrow().is_none());
                    assert!(lifetime_ref_mut.borrow().is_none());
                    assert!(lifetime.borrow_mut().is_none());
                    assert!(lifetime_ref_mut.borrow_mut().is_none());
                    let mut access = lifetime_ref_mut2.write(&mut value).unwrap();
                    *access = 42;
                    assert_eq!(*access, 42);
                    assert!(lifetime.read(&42).is_none());
                    assert!(lifetime_ref_mut.read(&42).is_none());
                    assert!(lifetime.write(&mut 42).is_none());
                    assert!(lifetime_ref_mut.write(&mut 42).is_none());
                    assert!(lifetime_lazy.read(&42).is_none());
                    assert!(lifetime_lazy.write(&mut 42).is_none());
                    assert!(lifetime_lazy.read(&42).is_none());
                    assert!(lifetime_lazy.write(&mut 42).is_none());
                }
            }
            assert_eq!(lifetime.state().writer_depth(), 0);
            lifetime.borrow().unwrap()
        };
        assert!(!lifetime_ref.exists());
        assert_eq!(value, 42);
    }

    #[test]
    fn test_lifetimes_multithread() {
        let lifetime = Lifetime::default();
        let lifetime_ref = lifetime.borrow().unwrap();
        assert!(lifetime_ref.exists());
        assert!(lifetime_ref.is_owned_by(&lifetime));
        drop(lifetime);
        assert!(!lifetime_ref.exists());
        let lifetime = Lifetime::default();
        let lifetime = spawn(move || {
            let value_ref = lifetime.borrow().unwrap();
            assert!(value_ref.exists());
            assert!(value_ref.is_owned_by(&lifetime));
            lifetime
        })
        .join()
        .unwrap();
        assert!(!lifetime_ref.exists());
        assert!(!lifetime_ref.is_owned_by(&lifetime));
    }
}
