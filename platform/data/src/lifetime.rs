use std::{
    future::poll_fn,
    ops::{Deref, DerefMut},
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::Poll,
};

#[derive(Default)]
struct LifetimeStateInner {
    locked: AtomicBool,
    readers: AtomicUsize,
    writer: AtomicUsize,
    read_access: AtomicUsize,
    write_access: AtomicBool,
    tag: AtomicUsize,
}

#[derive(Default, Clone)]
pub struct LifetimeState {
    inner: Arc<LifetimeStateInner>,
}

impl LifetimeState {
    pub fn can_read(&self) -> bool {
        self.inner.writer.load(Ordering::Acquire) == 0
    }

    pub fn can_write(&self, id: usize) -> bool {
        self.inner.writer.load(Ordering::Acquire) == id
            && self.inner.readers.load(Ordering::Acquire) == 0
    }

    pub fn writer_depth(&self) -> usize {
        self.inner.writer.load(Ordering::Acquire)
    }

    pub fn is_read_accessible(&self) -> bool {
        !self.inner.write_access.load(Ordering::Acquire)
    }

    pub fn is_write_accessible(&self) -> bool {
        !self.inner.write_access.load(Ordering::Acquire)
            && self.inner.read_access.load(Ordering::Acquire) == 0
    }

    pub fn is_in_use(&self) -> bool {
        self.inner.read_access.load(Ordering::Acquire) > 0
            || self.inner.write_access.load(Ordering::Acquire)
    }

    pub fn is_locked(&self) -> bool {
        self.inner.locked.load(Ordering::Acquire)
    }

    pub fn try_lock(&'_ self) -> Option<LifetimeStateAccess<'_>> {
        if !self.inner.locked.load(Ordering::Acquire) {
            self.inner.locked.store(true, Ordering::Release);
            Some(LifetimeStateAccess {
                state: self,
                unlock: true,
            })
        } else {
            None
        }
    }

    pub fn lock(&'_ self) -> LifetimeStateAccess<'_> {
        while self.inner.locked.load(Ordering::Acquire) {
            std::hint::spin_loop();
        }
        self.inner.locked.store(true, Ordering::Release);
        LifetimeStateAccess {
            state: self,
            unlock: true,
        }
    }

    /// # Safety
    pub unsafe fn lock_unchecked(&'_ self) -> LifetimeStateAccess<'_> {
        LifetimeStateAccess {
            state: self,
            unlock: true,
        }
    }

    /// # Safety
    pub unsafe fn update_tag(&self, tag: &Lifetime) {
        let tag = tag as *const Lifetime as usize;
        self.inner.tag.store(tag, Ordering::Release);
    }

    pub fn tag(&self) -> usize {
        self.inner.tag.load(Ordering::Acquire)
    }

    pub fn downgrade(&self) -> LifetimeWeakState {
        LifetimeWeakState {
            inner: Arc::downgrade(&self.inner),
            tag: self.inner.tag.load(Ordering::Acquire),
        }
    }
}

#[derive(Clone)]
pub struct LifetimeWeakState {
    inner: Weak<LifetimeStateInner>,
    tag: usize,
}

impl LifetimeWeakState {
    /// # Safety
    pub unsafe fn upgrade_unchecked(&self) -> Option<LifetimeState> {
        Some(LifetimeState {
            inner: self.inner.upgrade()?,
        })
    }

    pub fn upgrade(&self) -> Option<LifetimeState> {
        let inner = self.inner.upgrade()?;
        (inner.tag.load(Ordering::Acquire) == self.tag).then_some(LifetimeState { inner })
    }

    pub fn is_owned_by(&self, state: &LifetimeState) -> bool {
        Arc::downgrade(&state.inner).ptr_eq(&self.inner)
    }
}

pub struct LifetimeStateAccess<'a> {
    state: &'a LifetimeState,
    unlock: bool,
}

impl Drop for LifetimeStateAccess<'_> {
    fn drop(&mut self) {
        if self.unlock {
            self.state.inner.locked.store(false, Ordering::Release);
        }
    }
}

impl LifetimeStateAccess<'_> {
    pub fn state(&self) -> &LifetimeState {
        self.state
    }

    pub fn unlock(&mut self, value: bool) {
        self.unlock = value;
    }

    pub fn acquire_reader(&mut self) {
        let v = self.state.inner.readers.load(Ordering::Acquire) + 1;
        self.state.inner.readers.store(v, Ordering::Release);
    }

    pub fn release_reader(&mut self) {
        let v = self
            .state
            .inner
            .readers
            .load(Ordering::Acquire)
            .saturating_sub(1);
        self.state.inner.readers.store(v, Ordering::Release);
    }

    #[must_use]
    pub fn acquire_writer(&mut self) -> usize {
        let v = self.state.inner.writer.load(Ordering::Acquire) + 1;
        self.state.inner.writer.store(v, Ordering::Release);
        v
    }

    pub fn release_writer(&mut self, id: usize) {
        let v = self.state.inner.writer.load(Ordering::Acquire);
        if id <= v {
            self.state
                .inner
                .writer
                .store(id.saturating_sub(1), Ordering::Release);
        }
    }

    pub fn acquire_read_access(&mut self) {
        let v = self.state.inner.read_access.load(Ordering::Acquire) + 1;
        self.state.inner.read_access.store(v, Ordering::Release);
    }

    pub fn release_read_access(&mut self) {
        let v = self
            .state
            .inner
            .read_access
            .load(Ordering::Acquire)
            .saturating_sub(1);
        self.state.inner.read_access.store(v, Ordering::Release);
    }

    pub fn acquire_write_access(&mut self) {
        self.state.inner.write_access.store(true, Ordering::Release);
    }

    pub fn release_write_access(&mut self) {
        self.state
            .inner
            .write_access
            .store(false, Ordering::Release);
    }
}

#[derive(Default)]
pub struct Lifetime(LifetimeState);

impl Lifetime {
    pub fn state(&self) -> &LifetimeState {
        unsafe { self.0.update_tag(self) };
        &self.0
    }

    pub fn update_tag(&self) {
        unsafe { self.0.update_tag(self) };
    }

    pub fn tag(&self) -> usize {
        unsafe { self.0.update_tag(self) };
        self.0.tag()
    }

    pub fn borrow(&self) -> Option<LifetimeRef> {
        unsafe { self.0.update_tag(self) };
        self.0
            .try_lock()
            .filter(|access| access.state.can_read())
            .map(|mut access| {
                access.acquire_reader();
                LifetimeRef(self.0.downgrade())
            })
    }

    pub async fn borrow_async(&self) -> LifetimeRef {
        loop {
            if let Some(lifetime_ref) = self.borrow() {
                return lifetime_ref;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<LifetimeRef>::Pending
            })
            .await;
        }
    }

    pub fn borrow_mut(&self) -> Option<LifetimeRefMut> {
        unsafe { self.0.update_tag(self) };
        self.0
            .try_lock()
            .filter(|access| access.state.can_write(0))
            .map(|mut access| {
                let id = access.acquire_writer();
                LifetimeRefMut(self.0.downgrade(), id)
            })
    }

    pub async fn borrow_mut_async(&self) -> LifetimeRefMut {
        loop {
            if let Some(lifetime_ref_mut) = self.borrow_mut() {
                return lifetime_ref_mut;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<LifetimeRefMut>::Pending
            })
            .await;
        }
    }

    pub fn lazy(&self) -> LifetimeLazy {
        unsafe { self.0.update_tag(self) };
        LifetimeLazy(self.0.downgrade())
    }

    pub fn read<'a, T: ?Sized>(&'a self, data: &'a T) -> Option<ValueReadAccess<'a, T>> {
        unsafe { self.0.update_tag(self) };
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

    pub async fn read_async<'a, T: ?Sized>(&'a self, data: &'a T) -> ValueReadAccess<'a, T> {
        unsafe { self.read_ptr_async(data as *const T).await }
    }

    /// # Safety
    pub unsafe fn read_ptr<T: ?Sized>(&'_ self, data: *const T) -> Option<ValueReadAccess<'_, T>> {
        unsafe { self.0.update_tag(self) };
        self.0
            .try_lock()
            .filter(|access| access.state.is_read_accessible())
            .and_then(|mut access| {
                access.unlock = false;
                access.acquire_read_access();
                Some(ValueReadAccess {
                    lifetime: self.0.clone(),
                    data: unsafe { data.as_ref() }?,
                })
            })
    }

    /// # Safety
    pub async unsafe fn read_ptr_async<'a, T: ?Sized + 'a>(
        &'a self,
        data: *const T,
    ) -> ValueReadAccess<'a, T> {
        loop {
            if let Some(access) = unsafe { self.read_ptr(data) } {
                return access;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<ValueReadAccess<'a, T>>::Pending
            })
            .await;
        }
    }

    pub fn write<'a, T: ?Sized>(&'a self, data: &'a mut T) -> Option<ValueWriteAccess<'a, T>> {
        unsafe { self.0.update_tag(self) };
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

    pub async fn write_async<'a, T: ?Sized>(&'a self, data: &'a mut T) -> ValueWriteAccess<'a, T> {
        unsafe { self.write_ptr_async(data as *mut T).await }
    }

    /// # Safety
    pub unsafe fn write_ptr<T: ?Sized>(&'_ self, data: *mut T) -> Option<ValueWriteAccess<'_, T>> {
        unsafe { self.0.update_tag(self) };
        self.0
            .try_lock()
            .filter(|access| access.state.is_write_accessible())
            .and_then(|mut access| {
                access.unlock = false;
                access.acquire_write_access();
                Some(ValueWriteAccess {
                    lifetime: self.0.clone(),
                    data: unsafe { data.as_mut() }?,
                })
            })
    }

    /// # Safety
    pub async unsafe fn write_ptr_async<'a, T: ?Sized + 'a>(
        &'a self,
        data: *mut T,
    ) -> ValueWriteAccess<'a, T> {
        loop {
            if let Some(access) = unsafe { self.write_ptr(data) } {
                return access;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<ValueWriteAccess<'a, T>>::Pending
            })
            .await;
        }
    }

    pub fn try_read_lock(&self) -> Option<ReadLock> {
        unsafe { self.0.update_tag(self) };
        let mut access = self.0.lock();
        if !access.state.is_read_accessible() {
            return None;
        }
        access.acquire_read_access();
        Some(ReadLock {
            lifetime: self.0.clone(),
        })
    }

    pub fn read_lock(&self) -> ReadLock {
        unsafe { self.0.update_tag(self) };
        let mut access = self.0.lock();
        while !access.state.is_read_accessible() {
            std::hint::spin_loop();
        }
        access.acquire_read_access();
        ReadLock {
            lifetime: self.0.clone(),
        }
    }

    pub async fn read_lock_async(&self) -> ReadLock {
        loop {
            unsafe { self.0.update_tag(self) };
            let mut access = self.0.lock();
            if access.state.is_read_accessible() {
                access.acquire_read_access();
                return ReadLock {
                    lifetime: self.0.clone(),
                };
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<ReadLock>::Pending
            })
            .await;
        }
    }

    pub fn try_write_lock(&self) -> Option<WriteLock> {
        unsafe { self.0.update_tag(self) };
        let mut access = self.0.lock();
        if !access.state.is_write_accessible() {
            return None;
        }
        access.acquire_write_access();
        Some(WriteLock {
            lifetime: self.0.clone(),
        })
    }

    pub fn write_lock(&self) -> WriteLock {
        unsafe { self.0.update_tag(self) };
        let mut access = self.0.lock();
        while !access.state.is_write_accessible() {
            std::hint::spin_loop();
        }
        access.acquire_write_access();
        WriteLock {
            lifetime: self.0.clone(),
        }
    }

    pub async fn write_lock_async(&self) -> WriteLock {
        loop {
            unsafe { self.0.update_tag(self) };
            let mut access = self.0.lock();
            if access.state.is_write_accessible() {
                access.acquire_write_access();
                return WriteLock {
                    lifetime: self.0.clone(),
                };
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<WriteLock>::Pending
            })
            .await;
        }
    }

    pub async fn wait_for_read_access(&self) {
        loop {
            if self.state().is_read_accessible() {
                return;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<()>::Pending
            })
            .await;
        }
    }

    pub async fn wait_for_write_access(&self) {
        loop {
            if self.state().is_write_accessible() {
                return;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<()>::Pending
            })
            .await;
        }
    }
}

pub struct LifetimeRef(LifetimeWeakState);

impl Drop for LifetimeRef {
    fn drop(&mut self) {
        if let Some(owner) = unsafe { self.0.upgrade_unchecked() }
            && let Some(mut access) = owner.try_lock()
        {
            access.release_reader();
        }
    }
}

impl LifetimeRef {
    pub fn state(&self) -> &LifetimeWeakState {
        &self.0
    }

    pub fn tag(&self) -> usize {
        self.0.tag
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

    pub async fn borrow_async(&self) -> LifetimeRef {
        loop {
            if let Some(lifetime_ref) = self.borrow() {
                return lifetime_ref;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<LifetimeRef>::Pending
            })
            .await;
        }
    }

    pub fn read<'a, T: ?Sized>(&'a self, data: &'a T) -> Option<ValueReadAccess<'a, T>> {
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

    pub async fn read_async<'a, T: ?Sized>(&'a self, data: &'a T) -> ValueReadAccess<'a, T> {
        loop {
            if let Some(access) = self.read(data) {
                return access;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<ValueReadAccess<'a, T>>::Pending
            })
            .await;
        }
    }

    /// # Safety
    pub unsafe fn read_ptr<T: ?Sized>(&'_ self, data: *const T) -> Option<ValueReadAccess<'_, T>> {
        let state = self.0.upgrade()?;
        let mut access = state.try_lock()?;
        if access.state.is_read_accessible() {
            access.unlock = false;
            access.acquire_read_access();
            drop(access);
            Some(ValueReadAccess {
                lifetime: state,
                data: unsafe { data.as_ref() }?,
            })
        } else {
            None
        }
    }

    /// # Safety
    pub async unsafe fn read_ptr_async<'a, T: ?Sized + 'a>(
        &'a self,
        data: *const T,
    ) -> ValueReadAccess<'a, T> {
        loop {
            if let Some(access) = unsafe { self.read_ptr(data) } {
                return access;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<ValueReadAccess<'a, T>>::Pending
            })
            .await;
        }
    }

    pub fn try_read_lock(&self) -> Option<ReadLock> {
        let state = self.0.upgrade()?;
        let mut access = state.lock();
        if !access.state.is_read_accessible() {
            return None;
        }
        access.acquire_read_access();
        Some(ReadLock {
            lifetime: state.clone(),
        })
    }

    pub fn read_lock(&self) -> Option<ReadLock> {
        let state = self.0.upgrade()?;
        let mut access = state.lock();
        while !access.state.is_read_accessible() {
            std::hint::spin_loop();
        }
        access.acquire_read_access();
        Some(ReadLock {
            lifetime: state.clone(),
        })
    }

    pub async fn read_lock_async(&self) -> ReadLock {
        loop {
            if let Some(lock) = self.read_lock() {
                return lock;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<ReadLock>::Pending
            })
            .await;
        }
    }

    pub fn consume<T: ?Sized>(self, data: &'_ T) -> Result<ValueReadAccess<'_, T>, Self> {
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

    pub async fn wait_for_read_access(&self) {
        loop {
            let Some(state) = self.0.upgrade() else {
                return;
            };
            if state.is_read_accessible() {
                return;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<()>::Pending
            })
            .await;
        }
    }

    pub async fn wait_for_write_access(&self) {
        loop {
            let Some(state) = self.0.upgrade() else {
                return;
            };
            if state.is_read_accessible() {
                return;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<()>::Pending
            })
            .await;
        }
    }
}

pub struct LifetimeRefMut(LifetimeWeakState, usize);

impl Drop for LifetimeRefMut {
    fn drop(&mut self) {
        if let Some(state) = unsafe { self.0.upgrade_unchecked() }
            && let Some(mut access) = state.try_lock()
        {
            access.release_writer(self.1);
        }
    }
}

impl LifetimeRefMut {
    pub fn state(&self) -> &LifetimeWeakState {
        &self.0
    }

    pub fn tag(&self) -> usize {
        self.0.tag
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

    pub async fn borrow_async(&self) -> LifetimeRef {
        loop {
            if let Some(lifetime_ref) = self.borrow() {
                return lifetime_ref;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<LifetimeRef>::Pending
            })
            .await;
        }
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

    pub async fn borrow_mut_async(&self) -> LifetimeRefMut {
        loop {
            if let Some(lifetime_ref_mut) = self.borrow_mut() {
                return lifetime_ref_mut;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<LifetimeRefMut>::Pending
            })
            .await;
        }
    }

    pub fn read<'a, T: ?Sized>(&'a self, data: &'a T) -> Option<ValueReadAccess<'a, T>> {
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

    pub async fn read_async<'a, T: ?Sized>(&'a self, data: &'a T) -> ValueReadAccess<'a, T> {
        loop {
            if let Some(access) = self.read(data) {
                return access;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<ValueReadAccess<'a, T>>::Pending
            })
            .await;
        }
    }

    /// # Safety
    pub unsafe fn read_ptr<T: ?Sized>(&'_ self, data: *const T) -> Option<ValueReadAccess<'_, T>> {
        let state = self.0.upgrade()?;
        let mut access = state.try_lock()?;
        if access.state.is_read_accessible() {
            access.unlock = false;
            access.acquire_read_access();
            drop(access);
            Some(ValueReadAccess {
                lifetime: state,
                data: unsafe { data.as_ref() }?,
            })
        } else {
            None
        }
    }

    /// # Safety
    pub async unsafe fn read_ptr_async<'a, T: ?Sized + 'a>(
        &'a self,
        data: *const T,
    ) -> ValueReadAccess<'a, T> {
        loop {
            if let Some(access) = unsafe { self.read_ptr(data) } {
                return access;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<ValueReadAccess<'a, T>>::Pending
            })
            .await;
        }
    }

    pub fn write<'a, T: ?Sized>(&'a self, data: &'a mut T) -> Option<ValueWriteAccess<'a, T>> {
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

    pub async fn write_async<'a, T: ?Sized>(&'a self, data: &'a mut T) -> ValueWriteAccess<'a, T> {
        unsafe { self.write_ptr_async(data as *mut T).await }
    }

    /// # Safety
    pub unsafe fn write_ptr<T: ?Sized>(&'_ self, data: *mut T) -> Option<ValueWriteAccess<'_, T>> {
        let state = self.0.upgrade()?;
        let mut access = state.try_lock()?;
        if access.state.is_write_accessible() {
            access.unlock = false;
            access.acquire_write_access();
            drop(access);
            Some(ValueWriteAccess {
                lifetime: state,
                data: unsafe { data.as_mut() }?,
            })
        } else {
            None
        }
    }

    /// # Safety
    pub async unsafe fn write_ptr_async<'a, T: ?Sized + 'a>(
        &'a self,
        data: *mut T,
    ) -> ValueWriteAccess<'a, T> {
        loop {
            if let Some(access) = unsafe { self.write_ptr(data) } {
                return access;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<ValueWriteAccess<'a, T>>::Pending
            })
            .await;
        }
    }

    pub fn try_read_lock(&self) -> Option<ReadLock> {
        let state = self.0.upgrade()?;
        let mut access = state.lock();
        if !access.state.is_read_accessible() {
            return None;
        }
        access.acquire_read_access();
        Some(ReadLock {
            lifetime: state.clone(),
        })
    }

    pub fn read_lock(&self) -> Option<ReadLock> {
        let state = self.0.upgrade()?;
        let mut access = state.lock();
        while !access.state.is_read_accessible() {
            std::hint::spin_loop();
        }
        access.acquire_read_access();
        Some(ReadLock {
            lifetime: state.clone(),
        })
    }

    pub async fn read_lock_async(&self) -> ReadLock {
        loop {
            if let Some(lock) = self.read_lock() {
                return lock;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<ReadLock>::Pending
            })
            .await;
        }
    }

    pub fn try_write_lock(&self) -> Option<WriteLock> {
        let state = self.0.upgrade()?;
        let mut access = state.lock();
        if !access.state.is_write_accessible() {
            return None;
        }
        access.acquire_write_access();
        Some(WriteLock {
            lifetime: state.clone(),
        })
    }

    pub fn write_lock(&self) -> Option<WriteLock> {
        let state = self.0.upgrade()?;
        let mut access = state.lock();
        while !access.state.is_write_accessible() {
            std::hint::spin_loop();
        }
        access.acquire_write_access();
        Some(WriteLock {
            lifetime: state.clone(),
        })
    }

    pub async fn write_lock_async(&self) -> WriteLock {
        loop {
            if let Some(lock) = self.write_lock() {
                return lock;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<WriteLock>::Pending
            })
            .await;
        }
    }

    pub fn consume<T: ?Sized>(self, data: &'_ mut T) -> Result<ValueWriteAccess<'_, T>, Self> {
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

    pub async fn wait_for_read_access(&self) {
        loop {
            let Some(state) = self.0.upgrade() else {
                return;
            };
            if state.is_read_accessible() {
                return;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<()>::Pending
            })
            .await;
        }
    }

    pub async fn wait_for_write_access(&self) {
        loop {
            let Some(state) = self.0.upgrade() else {
                return;
            };
            if state.is_read_accessible() {
                return;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<()>::Pending
            })
            .await;
        }
    }
}

#[derive(Clone)]
pub struct LifetimeLazy(LifetimeWeakState);

impl LifetimeLazy {
    pub fn state(&self) -> &LifetimeWeakState {
        &self.0
    }

    pub fn tag(&self) -> usize {
        self.0.tag
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

    pub async fn borrow_async(&self) -> LifetimeRef {
        loop {
            if let Some(lifetime_ref) = self.borrow() {
                return lifetime_ref;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<LifetimeRef>::Pending
            })
            .await;
        }
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

    pub async fn borrow_mut_async(&self) -> LifetimeRefMut {
        loop {
            if let Some(lifetime_ref_mut) = self.borrow_mut() {
                return lifetime_ref_mut;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<LifetimeRefMut>::Pending
            })
            .await;
        }
    }

    pub fn read<'a, T: ?Sized>(&'a self, data: &'a T) -> Option<ValueReadAccess<'a, T>> {
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

    pub async fn read_async<'a, T: ?Sized>(&'a self, data: &'a T) -> ValueReadAccess<'a, T> {
        loop {
            if let Some(access) = self.read(data) {
                return access;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<ValueReadAccess<'a, T>>::Pending
            })
            .await;
        }
    }

    /// # Safety
    pub unsafe fn read_ptr<T: ?Sized>(&'_ self, data: *const T) -> Option<ValueReadAccess<'_, T>> {
        let state = self.0.upgrade()?;
        let mut access = state.try_lock()?;
        if access.state.is_read_accessible() {
            access.unlock = false;
            access.acquire_read_access();
            drop(access);
            Some(ValueReadAccess {
                lifetime: state,
                data: unsafe { data.as_ref() }?,
            })
        } else {
            None
        }
    }

    /// # Safety
    pub async unsafe fn read_ptr_async<'a, T: ?Sized + 'a>(
        &'a self,
        data: *const T,
    ) -> ValueReadAccess<'a, T> {
        loop {
            if let Some(access) = unsafe { self.read_ptr(data) } {
                return access;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<ValueReadAccess<'a, T>>::Pending
            })
            .await;
        }
    }

    pub fn write<'a, T: ?Sized>(&'a self, data: &'a mut T) -> Option<ValueWriteAccess<'a, T>> {
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

    pub async fn write_async<'a, T: ?Sized>(&'a self, data: &'a mut T) -> ValueWriteAccess<'a, T> {
        unsafe { self.write_ptr_async(data as *mut T).await }
    }

    /// # Safety
    pub unsafe fn write_ptr<T: ?Sized>(&'_ self, data: *mut T) -> Option<ValueWriteAccess<'_, T>> {
        let state = self.0.upgrade()?;
        let mut access = state.try_lock()?;
        if access.state.is_write_accessible() {
            access.unlock = false;
            access.acquire_write_access();
            drop(access);
            Some(ValueWriteAccess {
                lifetime: state,
                data: unsafe { data.as_mut() }?,
            })
        } else {
            None
        }
    }

    /// # Safety
    pub async unsafe fn write_ptr_async<'a, T: ?Sized + 'a>(
        &'a self,
        data: *mut T,
    ) -> ValueWriteAccess<'a, T> {
        loop {
            if let Some(access) = unsafe { self.write_ptr(data) } {
                return access;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<ValueWriteAccess<'a, T>>::Pending
            })
            .await;
        }
    }

    pub fn consume<T: ?Sized>(self, data: &'_ mut T) -> Result<ValueWriteAccess<'_, T>, Self> {
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

    pub async fn wait_for_read_access(&self) {
        loop {
            let Some(state) = self.0.upgrade() else {
                return;
            };
            if state.is_read_accessible() {
                return;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<()>::Pending
            })
            .await;
        }
    }

    pub async fn wait_for_write_access(&self) {
        loop {
            let Some(state) = self.0.upgrade() else {
                return;
            };
            if state.is_read_accessible() {
                return;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<()>::Pending
            })
            .await;
        }
    }
}

pub struct ValueReadAccess<'a, T: 'a + ?Sized> {
    lifetime: LifetimeState,
    data: &'a T,
}

impl<T: ?Sized> Drop for ValueReadAccess<'_, T> {
    fn drop(&mut self) {
        unsafe { self.lifetime.lock_unchecked().release_read_access() };
    }
}

impl<'a, T: ?Sized> ValueReadAccess<'a, T> {
    /// # Safety
    pub unsafe fn new_raw(data: &'a T, lifetime: LifetimeState) -> Self {
        Self { lifetime, data }
    }
}

impl<T: ?Sized> Deref for ValueReadAccess<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a, T: ?Sized> ValueReadAccess<'a, T> {
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

pub struct ValueWriteAccess<'a, T: 'a + ?Sized> {
    lifetime: LifetimeState,
    data: &'a mut T,
}

impl<T: ?Sized> Drop for ValueWriteAccess<'_, T> {
    fn drop(&mut self) {
        unsafe { self.lifetime.lock_unchecked().release_write_access() };
    }
}

impl<'a, T: ?Sized> ValueWriteAccess<'a, T> {
    /// # Safety
    pub unsafe fn new_raw(data: &'a mut T, lifetime: LifetimeState) -> Self {
        Self { lifetime, data }
    }
}

impl<T: ?Sized> Deref for ValueWriteAccess<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T: ?Sized> DerefMut for ValueWriteAccess<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

impl<'a, T: ?Sized> ValueWriteAccess<'a, T> {
    pub fn remap<U>(
        self,
        f: impl FnOnce(&mut T) -> Option<&mut U>,
    ) -> Result<ValueWriteAccess<'a, U>, Self> {
        if let Some(data) = f(unsafe { std::mem::transmute::<&mut T, &'a mut T>(&mut *self.data) })
        {
            Ok(ValueWriteAccess {
                lifetime: self.lifetime.clone(),
                data,
            })
        } else {
            Err(self)
        }
    }
}

pub struct ReadLock {
    lifetime: LifetimeState,
}

impl Drop for ReadLock {
    fn drop(&mut self) {
        unsafe { self.lifetime.lock_unchecked().release_read_access() };
    }
}

impl ReadLock {
    /// # Safety
    pub unsafe fn new_raw(lifetime: LifetimeState) -> Self {
        Self { lifetime }
    }

    pub fn using<R>(self, f: impl FnOnce() -> R) -> R {
        let result = f();
        drop(self);
        result
    }
}

pub struct WriteLock {
    lifetime: LifetimeState,
}

impl Drop for WriteLock {
    fn drop(&mut self) {
        unsafe { self.lifetime.lock_unchecked().release_write_access() };
    }
}

impl WriteLock {
    /// # Safety
    pub unsafe fn new_raw(lifetime: LifetimeState) -> Self {
        Self { lifetime }
    }

    pub fn using<R>(self, f: impl FnOnce() -> R) -> R {
        let result = f();
        drop(self);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::*;

    fn is_async<T: Send + Sync + ?Sized>() {
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

    #[test]
    fn test_lifetimes_move_invalidation() {
        let lifetime = Lifetime::default();
        let lifetime_ref = lifetime.borrow().unwrap();
        assert_eq!(lifetime_ref.tag(), lifetime.tag());
        assert!(lifetime_ref.exists());
        let lifetime_ref2 = lifetime_ref;
        assert_eq!(lifetime_ref2.tag(), lifetime.tag());
        assert!(lifetime_ref2.exists());
        let lifetime = Box::new(lifetime);
        assert_ne!(lifetime_ref2.tag(), lifetime.tag());
        assert!(!lifetime_ref2.exists());
        let lifetime = *lifetime;
        assert_ne!(lifetime_ref2.tag(), lifetime.tag());
        assert!(!lifetime_ref2.exists());
    }

    #[pollster::test]
    async fn test_lifetime_async() {
        let mut value = 42usize;
        let lifetime = Lifetime::default();
        assert_eq!(*lifetime.read_async(&value).await, 42);
        {
            let lifetime_ref = lifetime.borrow_async().await;
            {
                let access = lifetime_ref.read_async(&value).await;
                assert_eq!(*access, 42);
            }
        }
        {
            let lifetime_ref_mut = lifetime.borrow_mut_async().await;
            {
                let mut access = lifetime_ref_mut.write_async(&mut value).await;
                *access = 7;
                assert_eq!(*access, 7);
            }
            assert_eq!(*lifetime.read_async(&value).await, 7);
        }
        {
            let mut access = lifetime.write_async(&mut value).await;
            *access = 84;
        }
        {
            let access = lifetime.read_async(&value).await;
            assert_eq!(*access, 84);
        }
    }

    #[test]
    fn test_lifetime_locks() {
        let lifetime = Lifetime::default();
        assert!(lifetime.state().is_read_accessible());
        assert!(lifetime.state().is_write_accessible());

        let read_lock = lifetime.read_lock();
        assert!(lifetime.state().is_read_accessible());
        assert!(!lifetime.state().is_write_accessible());

        drop(read_lock);
        assert!(lifetime.state().is_read_accessible());
        assert!(lifetime.state().is_write_accessible());

        let read_lock = lifetime.read_lock();
        assert!(lifetime.state().is_read_accessible());
        assert!(!lifetime.state().is_write_accessible());

        let read_lock2 = lifetime.read_lock();
        assert!(lifetime.state().is_read_accessible());
        assert!(!lifetime.state().is_write_accessible());

        drop(read_lock);
        assert!(lifetime.state().is_read_accessible());
        assert!(!lifetime.state().is_write_accessible());

        drop(read_lock2);
        assert!(lifetime.state().is_read_accessible());
        assert!(lifetime.state().is_write_accessible());

        let write_lock = lifetime.write_lock();
        assert!(!lifetime.state().is_read_accessible());
        assert!(!lifetime.state().is_write_accessible());

        assert!(lifetime.try_read_lock().is_none());
        assert!(lifetime.try_write_lock().is_none());

        drop(write_lock);
        assert!(lifetime.state().is_read_accessible());
        assert!(lifetime.state().is_write_accessible());

        let data = ();
        let read_access = lifetime.read(&data).unwrap();
        assert!(lifetime.state().is_read_accessible());
        assert!(!lifetime.state().is_write_accessible());
        assert!(lifetime.state().is_locked());

        drop(read_access);
        assert!(lifetime.try_read_lock().is_some());
        assert!(lifetime.try_write_lock().is_some());
    }
}
