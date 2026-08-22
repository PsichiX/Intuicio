//! Runtime borrow checking for values Rust cannot track statically.
//!
//! Script values live in type-erased storage, so the compiler cannot prove
//! that a script does not alias them. A [`Lifetime`] is a small piece of
//! shared state that answers that question at runtime instead: it hands out
//! handles, and refuses to hand out a conflicting one.
//!
//! # Two independent axes
//!
//! A lifetime tracks borrows and accesses separately.
//!
//! **Borrows** are long lived claims, the runtime analogue of `&` and `&mut`:
//!
//! - [`Lifetime::borrow`] gives a [`LifetimeRef`]. Many can coexist, and they
//!   block mutable borrows.
//! - [`Lifetime::borrow_mut`] gives a [`LifetimeRefMut`]. It needs no readers
//!   and no other writer. Writers nest: an existing [`LifetimeRefMut`] can
//!   reborrow itself mutably, which raises the writer depth.
//! - [`Lifetime::lazy`] gives a [`LifetimeLazy`], which claims nothing and
//!   only checks conditions when it is actually used.
//!
//! **Accesses** are short lived guards over the data itself, taken from any of
//! the handles above:
//!
//! - `read` returns a [`ValueReadAccess`]. Many can coexist.
//! - `write` returns a [`ValueWriteAccess`]. It excludes every other access.
//! - [`ReadLock`] and [`WriteLock`] are the same guards without the data,
//!   for holding a claim across code that does not touch the value.
//!
//! Every method has a non-blocking form returning [`None`], a spinning form,
//! and an `_async` form that yields to the executor while it waits.
//!
//! # Dangling handles
//!
//! Handles hold a weak reference plus a tag, so they detect an owner that was
//! dropped or reset with [`Lifetime::invalidate`], and report it by returning
//! [`None`] rather than by dangling.
//!
//! ```
//! # use intuicio_data::lifetime::Lifetime;
//! let mut value = 0usize;
//! let lifetime = Lifetime::default();
//! *lifetime.write(&mut value).unwrap() = 42;
//! let borrow = lifetime.borrow().unwrap();
//! // a reader is out, so nobody can borrow mutably
//! assert!(lifetime.borrow_mut().is_none());
//! assert_eq!(*borrow.read(&value).unwrap(), 42);
//!
//! // read guards share, so several can be out at the same time
//! let first = lifetime.read(&value).unwrap();
//! let second = lifetime.read(&value).unwrap();
//! assert_eq!((*first, *second), (42, 42));
//! ```
use std::{
    future::poll_fn,
    ops::{Deref, DerefMut},
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::Poll,
};

/// Counters shared by one [`Lifetime`] and all handles derived from it.
///
/// `locked` is a spin lock guarding the rest, so that a check and the update
/// that follows it cannot be interleaved with another thread.
#[derive(Default)]
struct LifetimeStateInner {
    locked: AtomicBool,
    readers: AtomicUsize,
    writer: AtomicUsize,
    read_access: AtomicUsize,
    write_access: AtomicBool,
    tag: AtomicUsize,
}

/// Cloneable strong handle to the counters behind a [`Lifetime`].
///
/// Mostly an implementation detail of this module. Keeping one alive keeps
/// the counters alive, but it does not itself claim a borrow or an access.
#[derive(Default, Clone)]
pub struct LifetimeState {
    inner: Arc<LifetimeStateInner>,
}

impl LifetimeState {
    /// Returns `true` when no mutable borrow is out.
    pub fn can_read(&self) -> bool {
        self.inner.writer.load(Ordering::Acquire) == 0
    }

    /// Returns `true` when there are no readers and `id` is the current writer
    /// depth.
    ///
    /// Pass `0` to ask for a top level mutable borrow, or the depth of an
    /// existing [`LifetimeRefMut`] to ask for a nested reborrow.
    pub fn can_write(&self, id: usize) -> bool {
        self.inner.writer.load(Ordering::Acquire) == id
            && self.inner.readers.load(Ordering::Acquire) == 0
    }

    /// Returns how many [`LifetimeRef`] handles are out.
    pub fn readers_count(&self) -> usize {
        self.inner.readers.load(Ordering::Acquire)
    }

    /// Returns how deeply mutable borrows are nested, `0` when none is out.
    pub fn writer_depth(&self) -> usize {
        self.inner.writer.load(Ordering::Acquire)
    }

    /// Returns `true` when no write access guard is live.
    pub fn is_read_accessible(&self) -> bool {
        !self.inner.write_access.load(Ordering::Acquire)
    }

    /// Returns `true` when no access guard of any kind is live.
    pub fn is_write_accessible(&self) -> bool {
        !self.inner.write_access.load(Ordering::Acquire)
            && self.inner.read_access.load(Ordering::Acquire) == 0
    }

    /// Returns `true` while any access guard is live.
    pub fn is_in_use(&self) -> bool {
        self.inner.read_access.load(Ordering::Acquire) > 0
            || self.inner.write_access.load(Ordering::Acquire)
    }

    /// Returns `true` while another thread holds the internal spin lock.
    pub fn is_locked(&self) -> bool {
        self.inner.locked.load(Ordering::Acquire)
    }

    /// Takes the internal spin lock, or returns [`None`] when it is taken.
    pub fn try_lock(&'_ self) -> Option<LifetimeStateAccess<'_>> {
        if self
            .inner
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(LifetimeStateAccess {
                state: self,
                unlock: true,
            })
        } else {
            None
        }
    }

    /// Takes the internal spin lock, spinning until it is free.
    pub fn lock(&'_ self) -> LifetimeStateAccess<'_> {
        while self
            .inner
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            std::hint::spin_loop();
        }
        LifetimeStateAccess {
            state: self,
            unlock: true,
        }
    }

    /// Builds a lock guard without taking the lock.
    ///
    /// # Safety
    ///
    /// No other thread must update the counters while this guard lives. If one
    /// does, the read-modify-write pairs in [`LifetimeStateAccess`] can lose an
    /// update.
    pub unsafe fn lock_unchecked(&'_ self) -> LifetimeStateAccess<'_> {
        LifetimeStateAccess {
            state: self,
            unlock: true,
        }
    }

    /// Stamps the address of the owning [`Lifetime`] as the current tag.
    ///
    /// # Safety
    ///
    /// `tag` must be the [`Lifetime`] that owns this state. Passing another one
    /// makes stale handles look valid again.
    pub unsafe fn update_tag(&self, tag: &Lifetime) {
        let tag = tag as *const Lifetime as usize;
        self.inner.tag.store(tag, Ordering::Release);
    }

    /// Clears the tag, so every handle taken so far stops upgrading.
    ///
    /// # Safety
    ///
    /// Callers holding handles will see them go dead. Use through
    /// [`Lifetime::invalidate`] rather than directly.
    pub unsafe fn invalidate_tag(&self) {
        self.inner.tag.store(0, Ordering::Release);
    }

    /// Returns the current tag, or `0` when the state was invalidated.
    pub fn tag(&self) -> usize {
        self.inner.tag.load(Ordering::Acquire)
    }

    /// Takes a weak handle that remembers the current tag.
    pub fn downgrade(&self) -> LifetimeWeakState {
        LifetimeWeakState {
            inner: Arc::downgrade(&self.inner),
            tag: self.inner.tag.load(Ordering::Acquire),
        }
    }
}

/// Weak handle to a [`LifetimeState`], paired with the tag it was taken at.
///
/// [`LifetimeWeakState::upgrade`] fails once the owner is gone or the tag
/// changed, which is how [`LifetimeRef`], [`LifetimeRefMut`] and
/// [`LifetimeLazy`] notice they went stale.
#[derive(Clone)]
pub struct LifetimeWeakState {
    inner: Weak<LifetimeStateInner>,
    tag: usize,
}

impl LifetimeWeakState {
    /// Upgrades without checking the tag, so it succeeds even for a lifetime
    /// that was invalidated and reused.
    ///
    /// # Safety
    ///
    /// The counters are always valid to touch, but the value the lifetime used
    /// to guard may be a different one by now. Only use this to release
    /// counters that this handle itself acquired.
    pub unsafe fn upgrade_unchecked(&self) -> Option<LifetimeState> {
        Some(LifetimeState {
            inner: self.inner.upgrade()?,
        })
    }

    /// Upgrades to a strong handle, or returns [`None`] when the owner is gone
    /// or was invalidated.
    pub fn upgrade(&self) -> Option<LifetimeState> {
        let inner = self.inner.upgrade()?;
        (inner.tag.load(Ordering::Acquire) == self.tag).then_some(LifetimeState { inner })
    }

    /// Returns `true` when this handle points at `state`.
    pub fn is_owned_by(&self, state: &LifetimeState) -> bool {
        Arc::downgrade(&state.inner).ptr_eq(&self.inner)
    }
}

/// Guard over the internal spin lock of a [`LifetimeState`], through which
/// the borrow and access counters are updated.
///
/// Releases the lock on drop. Hold it only long enough to check a condition
/// and update the counters: while it is taken, every other handle to the same
/// lifetime is refused or spinning.
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
    /// Returns the locked state.
    pub fn state(&self) -> &LifetimeState {
        self.state
    }

    /// Chooses whether dropping this guard releases the spin lock.
    ///
    /// With `false` the lock stays taken until something releases it through
    /// [`LifetimeState::lock_unchecked`]. A held lock blocks every other handle
    /// to the same lifetime.
    pub fn unlock(&mut self, value: bool) {
        self.unlock = value;
    }

    /// Counts one more shared borrow.
    pub fn acquire_reader(&mut self) {
        let v = self.state.inner.readers.load(Ordering::Acquire) + 1;
        self.state.inner.readers.store(v, Ordering::Release);
    }

    /// Counts one shared borrow less, saturating at zero.
    pub fn release_reader(&mut self) {
        let v = self
            .state
            .inner
            .readers
            .load(Ordering::Acquire)
            .saturating_sub(1);
        self.state.inner.readers.store(v, Ordering::Release);
    }

    /// Counts one more nested mutable borrow and returns its new depth.
    ///
    /// The depth has to be given back to [`LifetimeStateAccess::release_writer`].
    #[must_use]
    pub fn acquire_writer(&mut self) -> usize {
        let v = self.state.inner.writer.load(Ordering::Acquire) + 1;
        self.state.inner.writer.store(v, Ordering::Release);
        v
    }

    /// Drops the mutable borrow at depth `id`, along with anything nested
    /// inside it. Does nothing when `id` is deeper than the current depth.
    pub fn release_writer(&mut self, id: usize) {
        let v = self.state.inner.writer.load(Ordering::Acquire);
        if id <= v {
            self.state
                .inner
                .writer
                .store(id.saturating_sub(1), Ordering::Release);
        }
    }

    /// Counts one more live read guard.
    pub fn acquire_read_access(&mut self) {
        let v = self.state.inner.read_access.load(Ordering::Acquire) + 1;
        self.state.inner.read_access.store(v, Ordering::Release);
    }

    /// Counts one live read guard less, saturating at zero.
    pub fn release_read_access(&mut self) {
        let v = self
            .state
            .inner
            .read_access
            .load(Ordering::Acquire)
            .saturating_sub(1);
        self.state.inner.read_access.store(v, Ordering::Release);
    }

    /// Marks a write guard as live, excluding every other access.
    pub fn acquire_write_access(&mut self) {
        self.state.inner.write_access.store(true, Ordering::Release);
    }

    /// Marks the write guard as gone.
    pub fn release_write_access(&mut self) {
        self.state
            .inner
            .write_access
            .store(false, Ordering::Release);
    }
}

/// Owner of a runtime borrow state, kept next to the value it describes.
///
/// Dropping it, or calling [`Lifetime::invalidate`], kills every handle
/// taken from it. See the [module docs](self) for the borrow and access
/// model.
#[derive(Default)]
pub struct Lifetime(LifetimeState);

impl Lifetime {
    /// Kills every handle taken so far and starts over with fresh counters.
    ///
    /// Call this when the value behind the lifetime is replaced, so that old
    /// handles cannot reach the new value.
    pub fn invalidate(&mut self) {
        unsafe { self.0.invalidate_tag() };
        self.0 = Default::default();
    }

    /// Returns the shared state, refreshing the tag first.
    pub fn state(&self) -> &LifetimeState {
        unsafe { self.0.update_tag(self) };
        &self.0
    }

    /// Re-stamps the tag with the current address of this lifetime.
    ///
    /// Needed after the lifetime was moved in memory, so handles taken before
    /// the move keep upgrading.
    pub fn update_tag(&self) {
        unsafe { self.0.update_tag(self) };
    }

    /// Returns the current tag.
    pub fn tag(&self) -> usize {
        unsafe { self.0.update_tag(self) };
        self.0.tag()
    }

    /// Takes a shared borrow, or returns [`None`] when a mutable borrow is out.
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

    /// [`Lifetime::borrow`], awaiting until it succeeds.
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

    /// Takes a mutable borrow, or returns [`None`] when any other borrow is out.
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

    /// [`Lifetime::borrow_mut`], awaiting until it succeeds.
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

    /// Takes a handle that claims no borrow and is checked only when used.
    pub fn lazy(&self) -> LifetimeLazy {
        unsafe { self.0.update_tag(self) };
        LifetimeLazy(self.0.downgrade())
    }

    /// Guards `data` for reading, or returns [`None`] while a write guard is
    /// live.
    ///
    /// The caller has to pass the data that this lifetime guards. The lifetime
    /// only tracks the permission.
    pub fn read<'a, T: ?Sized>(&'a self, data: &'a T) -> Option<ValueReadAccess<'a, T>> {
        unsafe { self.0.update_tag(self) };
        self.0
            .try_lock()
            .filter(|access| access.state.is_read_accessible())
            .map(|mut access| {
                access.acquire_read_access();
                ValueReadAccess {
                    lifetime: self.0.clone(),
                    data,
                }
            })
    }

    /// [`Lifetime::read`], awaiting until it succeeds.
    pub async fn read_async<'a, T: ?Sized>(&'a self, data: &'a T) -> ValueReadAccess<'a, T> {
        unsafe { self.read_ptr_async(data as *const T).await }
    }

    /// [`Lifetime::read`] over a raw pointer.
    ///
    /// # Safety
    ///
    /// `data` must stay valid and aligned for as long as the returned guard
    /// lives. A null pointer yields [`None`].
    pub unsafe fn read_ptr<T: ?Sized>(&'_ self, data: *const T) -> Option<ValueReadAccess<'_, T>> {
        let data = unsafe { data.as_ref() }?;
        unsafe { self.0.update_tag(self) };
        self.0
            .try_lock()
            .filter(|access| access.state.is_read_accessible())
            .map(|mut access| {
                access.acquire_read_access();
                ValueReadAccess {
                    lifetime: self.0.clone(),
                    data,
                }
            })
    }

    /// [`Lifetime::read_ptr`], awaiting until it succeeds.
    ///
    /// # Safety
    ///
    /// Same as [`Lifetime::read_ptr`], and `data` must also stay valid across
    /// every await point.
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

    /// Guards `data` for writing, or returns [`None`] while any other access
    /// guard is live.
    pub fn write<'a, T: ?Sized>(&'a self, data: &'a mut T) -> Option<ValueWriteAccess<'a, T>> {
        unsafe { self.0.update_tag(self) };
        self.0
            .try_lock()
            .filter(|access| access.state.is_write_accessible())
            .map(|mut access| {
                access.acquire_write_access();
                ValueWriteAccess {
                    lifetime: self.0.clone(),
                    data,
                }
            })
    }

    /// [`Lifetime::write`], awaiting until it succeeds.
    pub async fn write_async<'a, T: ?Sized>(&'a self, data: &'a mut T) -> ValueWriteAccess<'a, T> {
        unsafe { self.write_ptr_async(data as *mut T).await }
    }

    /// [`Lifetime::write`] over a raw pointer.
    ///
    /// # Safety
    ///
    /// `data` must stay valid, aligned and unaliased for as long as the
    /// returned guard lives. A null pointer yields [`None`].
    pub unsafe fn write_ptr<T: ?Sized>(&'_ self, data: *mut T) -> Option<ValueWriteAccess<'_, T>> {
        let data = unsafe { data.as_mut() }?;
        unsafe { self.0.update_tag(self) };
        self.0
            .try_lock()
            .filter(|access| access.state.is_write_accessible())
            .map(|mut access| {
                access.acquire_write_access();
                ValueWriteAccess {
                    lifetime: self.0.clone(),
                    data,
                }
            })
    }

    /// [`Lifetime::write_ptr`], awaiting until it succeeds.
    ///
    /// # Safety
    ///
    /// Same as [`Lifetime::write_ptr`], and `data` must also stay valid across
    /// every await point.
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

    /// Claims read access without holding any data, or returns [`None`] when a
    /// write guard is live.
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

    /// [`Lifetime::try_read_lock`], spinning until it succeeds.
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

    /// [`Lifetime::try_read_lock`], awaiting until it succeeds.
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

    /// Claims write access without holding any data, or returns [`None`] when
    /// any access guard is live.
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

    /// [`Lifetime::try_write_lock`], spinning until it succeeds.
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

    /// [`Lifetime::try_write_lock`], awaiting until it succeeds.
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

    /// Awaits until reading would be allowed, without claiming anything.
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

    /// Awaits until writing would be allowed, without claiming anything.
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

/// Shared borrow of a [`Lifetime`], the runtime analogue of `&T`.
///
/// Many can coexist and they keep mutable borrows out. Releases its claim
/// on drop.
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
    /// Returns the weak state this borrow points at.
    pub fn state(&self) -> &LifetimeWeakState {
        &self.0
    }

    /// Returns the tag the owner had when this borrow was taken.
    pub fn tag(&self) -> usize {
        self.0.tag
    }

    /// Returns `true` while the owning [`Lifetime`] is alive and valid.
    pub fn exists(&self) -> bool {
        self.0.upgrade().is_some()
    }

    /// Returns `true` when another shared borrow could be taken.
    pub fn can_read(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.can_read())
            .unwrap_or(false)
    }

    /// Returns `true` when no write guard is live.
    pub fn is_read_accessible(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.is_read_accessible())
            .unwrap_or(false)
    }

    /// Returns `true` while any access guard is live.
    pub fn is_in_use(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.is_in_use())
            .unwrap_or(false)
    }

    /// Returns `true` when this borrow came from `other`.
    pub fn is_owned_by(&self, other: &Lifetime) -> bool {
        self.0.is_owned_by(&other.0)
    }

    /// Takes another shared borrow of the same lifetime.
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

    /// [`LifetimeRef::borrow`], awaiting until it succeeds.
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

    /// Takes a lazy handle to the same lifetime.
    pub fn lazy(&self) -> LifetimeLazy {
        LifetimeLazy(self.0.clone())
    }

    /// Guards `data` for reading, or returns [`None`] while a write guard is
    /// live or the owner is gone.
    pub fn read<'a, T: ?Sized>(&'a self, data: &'a T) -> Option<ValueReadAccess<'a, T>> {
        let state = self.0.upgrade()?;
        let mut access = state.try_lock()?;
        if access.state.is_read_accessible() {
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

    /// [`LifetimeRef::read`], awaiting until it succeeds.
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

    /// [`LifetimeRef::read`] over a raw pointer.
    ///
    /// # Safety
    ///
    /// `data` must stay valid and aligned for as long as the returned guard
    /// lives. A null pointer yields [`None`].
    pub unsafe fn read_ptr<T: ?Sized>(&'_ self, data: *const T) -> Option<ValueReadAccess<'_, T>> {
        let data = unsafe { data.as_ref() }?;
        let state = self.0.upgrade()?;
        let mut access = state.try_lock()?;
        if access.state.is_read_accessible() {
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

    /// [`LifetimeRef::read_ptr`], awaiting until it succeeds.
    ///
    /// # Safety
    ///
    /// Same as [`LifetimeRef::read_ptr`], and `data` must also stay valid
    /// across every await point.
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

    /// Claims read access without holding any data.
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

    /// [`LifetimeRef::try_read_lock`], spinning until it succeeds. Returns
    /// [`None`] when the owner is gone.
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

    /// [`LifetimeRef::read_lock`], awaiting until it succeeds.
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

    /// Turns this borrow into a read guard over `data` that lives as long as
    /// the borrow would have.
    ///
    /// Gives the borrow back unchanged when a write guard is live.
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

    /// Awaits until reading would be allowed, or the owner is gone.
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

    /// Awaits until writing would be allowed, or the owner is gone.
    pub async fn wait_for_write_access(&self) {
        loop {
            let Some(state) = self.0.upgrade() else {
                return;
            };
            if state.is_write_accessible() {
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

/// Mutable borrow of a [`Lifetime`], the runtime analogue of `&mut T`.
///
/// Excludes every other top level borrow, but can reborrow itself through
/// [`LifetimeRefMut::borrow_mut`], which nests one level deeper. Releases
/// its level, and everything nested under it, on drop.
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
    /// Returns the weak state this borrow points at.
    pub fn state(&self) -> &LifetimeWeakState {
        &self.0
    }

    /// Returns the tag the owner had when this borrow was taken.
    pub fn tag(&self) -> usize {
        self.0.tag
    }

    /// Returns how deeply this mutable borrow is nested, starting at `1`.
    pub fn depth(&self) -> usize {
        self.1
    }

    /// Returns `true` while the owning [`Lifetime`] is alive and valid.
    pub fn exists(&self) -> bool {
        self.0.upgrade().is_some()
    }

    /// Returns `true` when a shared borrow could be taken.
    pub fn can_read(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.can_read())
            .unwrap_or(false)
    }

    /// Returns `true` when this borrow could be reborrowed mutably.
    pub fn can_write(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.can_write(self.1))
            .unwrap_or(false)
    }

    /// Returns `true` when no write guard is live.
    pub fn is_read_accessible(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.is_read_accessible())
            .unwrap_or(false)
    }

    /// Returns `true` when no access guard of any kind is live.
    pub fn is_write_accessible(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.is_write_accessible())
            .unwrap_or(false)
    }

    /// Returns `true` while any access guard is live.
    pub fn is_in_use(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.is_in_use())
            .unwrap_or(false)
    }

    /// Returns `true` when this borrow came from `other`.
    pub fn is_owned_by(&self, other: &Lifetime) -> bool {
        self.0.is_owned_by(&other.0)
    }

    /// Takes a shared borrow, which only succeeds once this mutable borrow is
    /// not the innermost one.
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

    /// [`LifetimeRefMut::borrow`], awaiting until it succeeds.
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

    /// Reborrows mutably one level deeper, or returns [`None`] when this is not
    /// the innermost mutable borrow.
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

    /// [`LifetimeRefMut::borrow_mut`], awaiting until it succeeds.
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

    /// Takes a lazy handle to the same lifetime.
    pub fn lazy(&self) -> LifetimeLazy {
        LifetimeLazy(self.0.clone())
    }

    /// Guards `data` for reading, or returns [`None`] while a write guard is
    /// live or the owner is gone.
    pub fn read<'a, T: ?Sized>(&'a self, data: &'a T) -> Option<ValueReadAccess<'a, T>> {
        let state = self.0.upgrade()?;
        let mut access = state.try_lock()?;
        if access.state.is_read_accessible() {
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

    /// [`LifetimeRefMut::read`], awaiting until it succeeds.
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

    /// [`LifetimeRefMut::read`] over a raw pointer.
    ///
    /// # Safety
    ///
    /// `data` must stay valid and aligned for as long as the returned guard
    /// lives. A null pointer yields [`None`].
    pub unsafe fn read_ptr<T: ?Sized>(&'_ self, data: *const T) -> Option<ValueReadAccess<'_, T>> {
        let data = unsafe { data.as_ref() }?;
        let state = self.0.upgrade()?;
        let mut access = state.try_lock()?;
        if access.state.is_read_accessible() {
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

    /// [`LifetimeRefMut::read_ptr`], awaiting until it succeeds.
    ///
    /// # Safety
    ///
    /// Same as [`LifetimeRefMut::read_ptr`], and `data` must also stay valid
    /// across every await point.
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

    /// Guards `data` for writing, or returns [`None`] while any other access
    /// guard is live or the owner is gone.
    pub fn write<'a, T: ?Sized>(&'a self, data: &'a mut T) -> Option<ValueWriteAccess<'a, T>> {
        let state = self.0.upgrade()?;
        let mut access = state.try_lock()?;
        if access.state.is_write_accessible() {
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

    /// [`LifetimeRefMut::write`], awaiting until it succeeds.
    pub async fn write_async<'a, T: ?Sized>(&'a self, data: &'a mut T) -> ValueWriteAccess<'a, T> {
        unsafe { self.write_ptr_async(data as *mut T).await }
    }

    /// [`LifetimeRefMut::write`] over a raw pointer.
    ///
    /// # Safety
    ///
    /// `data` must stay valid, aligned and unaliased for as long as the
    /// returned guard lives. A null pointer yields [`None`].
    pub unsafe fn write_ptr<T: ?Sized>(&'_ self, data: *mut T) -> Option<ValueWriteAccess<'_, T>> {
        let data = unsafe { data.as_mut() }?;
        let state = self.0.upgrade()?;
        let mut access = state.try_lock()?;
        if access.state.is_write_accessible() {
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

    /// [`LifetimeRefMut::write_ptr`], awaiting until it succeeds.
    ///
    /// # Safety
    ///
    /// Same as [`LifetimeRefMut::write_ptr`], and `data` must also stay valid
    /// across every await point.
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

    /// Claims read access without holding any data.
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

    /// [`LifetimeRefMut::try_read_lock`], spinning until it succeeds. Returns
    /// [`None`] when the owner is gone.
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

    /// [`LifetimeRefMut::read_lock`], awaiting until it succeeds.
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

    /// Claims write access without holding any data.
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

    /// [`LifetimeRefMut::try_write_lock`], spinning until it succeeds. Returns
    /// [`None`] when the owner is gone.
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

    /// [`LifetimeRefMut::write_lock`], awaiting until it succeeds.
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

    /// Turns this borrow into a write guard over `data` that lives as long as
    /// the borrow would have.
    ///
    /// Gives the borrow back unchanged when another access guard is live.
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

    /// Awaits until reading would be allowed, or the owner is gone.
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

    /// Awaits until writing would be allowed, or the owner is gone.
    pub async fn wait_for_write_access(&self) {
        loop {
            let Some(state) = self.0.upgrade() else {
                return;
            };
            if state.is_write_accessible() {
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

/// Handle that claims nothing until it is used.
///
/// Unlike [`LifetimeRef`] and [`LifetimeRefMut`], holding one blocks
/// nobody, and it can be cloned freely. Each call checks the conditions
/// again, so it is the right handle for a value that is looked up now and
/// touched later, such as a script variable.
#[derive(Clone)]
pub struct LifetimeLazy(LifetimeWeakState);

impl LifetimeLazy {
    /// Returns the weak state this handle points at.
    pub fn state(&self) -> &LifetimeWeakState {
        &self.0
    }

    /// Returns the tag the owner had when this handle was taken.
    pub fn tag(&self) -> usize {
        self.0.tag
    }

    /// Returns `true` while the owning [`Lifetime`] is alive and valid.
    pub fn exists(&self) -> bool {
        self.0.upgrade().is_some()
    }

    /// Returns `true` when no write guard is live.
    pub fn is_read_accessible(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.is_read_accessible())
            .unwrap_or(false)
    }

    /// Returns `true` when no access guard of any kind is live.
    pub fn is_write_accessible(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.is_write_accessible())
            .unwrap_or(false)
    }

    /// Returns `true` while any access guard is live.
    pub fn is_in_use(&self) -> bool {
        self.0
            .upgrade()
            .map(|state| state.is_in_use())
            .unwrap_or(false)
    }

    /// Returns `true` when this handle came from `other`.
    pub fn is_owned_by(&self, other: &Lifetime) -> bool {
        self.0.is_owned_by(&other.0)
    }

    /// Upgrades to a real shared borrow.
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

    /// [`LifetimeLazy::borrow`], awaiting until it succeeds.
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

    /// Upgrades to a real top level mutable borrow, which needs no other
    /// borrow to be out.
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

    /// [`LifetimeLazy::borrow_mut`], awaiting until it succeeds.
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

    /// Guards `data` for reading, or returns [`None`] while a write guard is
    /// live or the owner is gone.
    pub fn read<'a, T: ?Sized>(&'a self, data: &'a T) -> Option<ValueReadAccess<'a, T>> {
        let state = self.0.upgrade()?;
        let mut access = state.try_lock()?;
        if access.state.is_read_accessible() {
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

    /// [`LifetimeLazy::read`], awaiting until it succeeds.
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

    /// [`LifetimeLazy::read`] over a raw pointer.
    ///
    /// # Safety
    ///
    /// `data` must stay valid and aligned for as long as the returned guard
    /// lives. A null pointer yields [`None`].
    pub unsafe fn read_ptr<T: ?Sized>(&'_ self, data: *const T) -> Option<ValueReadAccess<'_, T>> {
        let data = unsafe { data.as_ref() }?;
        let state = self.0.upgrade()?;
        let mut access = state.try_lock()?;
        if access.state.is_read_accessible() {
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

    /// [`LifetimeLazy::read_ptr`], awaiting until it succeeds.
    ///
    /// # Safety
    ///
    /// Same as [`LifetimeLazy::read_ptr`], and `data` must also stay valid
    /// across every await point.
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

    /// Guards `data` for writing, or returns [`None`] while any other access
    /// guard is live or the owner is gone.
    pub fn write<'a, T: ?Sized>(&'a self, data: &'a mut T) -> Option<ValueWriteAccess<'a, T>> {
        let state = self.0.upgrade()?;
        let mut access = state.try_lock()?;
        if access.state.is_write_accessible() {
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

    /// [`LifetimeLazy::write`], awaiting until it succeeds.
    pub async fn write_async<'a, T: ?Sized>(&'a self, data: &'a mut T) -> ValueWriteAccess<'a, T> {
        unsafe { self.write_ptr_async(data as *mut T).await }
    }

    /// [`LifetimeLazy::write`] over a raw pointer.
    ///
    /// # Safety
    ///
    /// `data` must stay valid, aligned and unaliased for as long as the
    /// returned guard lives. A null pointer yields [`None`].
    pub unsafe fn write_ptr<T: ?Sized>(&'_ self, data: *mut T) -> Option<ValueWriteAccess<'_, T>> {
        let data = unsafe { data.as_mut() }?;
        let state = self.0.upgrade()?;
        let mut access = state.try_lock()?;
        if access.state.is_write_accessible() {
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

    /// [`LifetimeLazy::write_ptr`], awaiting until it succeeds.
    ///
    /// # Safety
    ///
    /// Same as [`LifetimeLazy::write_ptr`], and `data` must also stay valid
    /// across every await point.
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

    /// Turns this handle into a write guard over `data`.
    ///
    /// Gives the handle back unchanged when another access guard is live.
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

    /// Awaits until reading would be allowed, or the owner is gone.
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

    /// Awaits until writing would be allowed, or the owner is gone.
    pub async fn wait_for_write_access(&self) {
        loop {
            let Some(state) = self.0.upgrade() else {
                return;
            };
            if state.is_write_accessible() {
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

/// Read guard over a value, obtained from a lifetime or one of its handles.
///
/// Derefs to the value and releases the read claim on drop.
pub struct ValueReadAccess<'a, T: 'a + ?Sized> {
    lifetime: LifetimeState,
    data: &'a T,
}

impl<T: ?Sized> Drop for ValueReadAccess<'_, T> {
    fn drop(&mut self) {
        self.lifetime.lock().release_read_access();
    }
}

impl<'a, T: ?Sized> ValueReadAccess<'a, T> {
    /// Builds a guard from parts, without going through the state checks.
    ///
    /// # Safety
    ///
    /// The read claim on `lifetime` must already be acquired, since dropping
    /// this guard releases one. `data` must be the value that `lifetime`
    /// guards.
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
    /// Narrows the guard down to a part of the value, for example one field.
    ///
    /// Gives the guard back unchanged when `f` returns [`None`].
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

/// Write guard over a value, obtained from a lifetime or one of its
/// handles.
///
/// Derefs to the value mutably and releases the write claim on drop.
/// While it is live no other access is allowed.
pub struct ValueWriteAccess<'a, T: 'a + ?Sized> {
    lifetime: LifetimeState,
    data: &'a mut T,
}

impl<T: ?Sized> Drop for ValueWriteAccess<'_, T> {
    fn drop(&mut self) {
        self.lifetime.lock().release_write_access();
    }
}

impl<'a, T: ?Sized> ValueWriteAccess<'a, T> {
    /// Builds a guard from parts, without going through the state checks.
    ///
    /// # Safety
    ///
    /// The write claim on `lifetime` must already be acquired, since dropping
    /// this guard releases one. `data` must be the value that `lifetime`
    /// guards, and must not be aliased.
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
    /// Narrows the guard down to a part of the value, for example one field.
    ///
    /// Gives the guard back unchanged when `f` returns [`None`].
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

/// Read claim held without a reference to the value.
///
/// Useful for keeping a value readable across code that does not touch it.
/// Releases the claim on drop.
pub struct ReadLock {
    lifetime: LifetimeState,
}

impl Drop for ReadLock {
    fn drop(&mut self) {
        self.lifetime.lock().release_read_access();
    }
}

impl ReadLock {
    /// Builds a lock from a state, without going through the state checks.
    ///
    /// # Safety
    ///
    /// The read claim on `lifetime` must already be acquired, since dropping
    /// this lock releases one.
    pub unsafe fn new_raw(lifetime: LifetimeState) -> Self {
        Self { lifetime }
    }

    /// Runs `f` while holding the lock, then releases it.
    pub fn using<R>(self, f: impl FnOnce() -> R) -> R {
        let result = f();
        drop(self);
        result
    }
}

/// Write claim held without a reference to the value.
///
/// Blocks every other access until dropped.
pub struct WriteLock {
    lifetime: LifetimeState,
}

impl Drop for WriteLock {
    fn drop(&mut self) {
        self.lifetime.lock().release_write_access();
    }
}

impl WriteLock {
    /// Builds a lock from a state, without going through the state checks.
    ///
    /// # Safety
    ///
    /// The write claim on `lifetime` must already be acquired, since dropping
    /// this lock releases one.
    pub unsafe fn new_raw(lifetime: LifetimeState) -> Self {
        Self { lifetime }
    }

    /// Runs `f` while holding the lock, then releases it.
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
                    assert!(lifetime_lazy.read(&42).is_some());
                    assert!(lifetime_lazy.write(&mut 42).is_none());
                }
                let lifetime_ref2 = lifetime_ref.borrow().unwrap();
                {
                    let access = lifetime_ref2.read(&value).unwrap();
                    assert_eq!(*access, 42);
                    assert!(lifetime_lazy.read(&42).is_some());
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
        // the spin lock guards the counter update, not the guard's lifetime
        assert!(!lifetime.state().is_locked());

        drop(read_access);
        assert!(lifetime.try_read_lock().is_some());
        assert!(lifetime.try_write_lock().is_some());
    }

    #[test]
    fn test_read_access_guards_coexist() {
        let mut value = 42usize;
        let lifetime = Lifetime::default();

        let first = lifetime.read(&value).unwrap();
        let second = lifetime.read(&value).unwrap();
        let third = lifetime.read(&value).unwrap();
        assert_eq!(*first, 42);
        assert_eq!(*second, 42);
        assert_eq!(*third, 42);

        // no guard holds the spin lock, so nothing below spins or fails early
        assert!(!lifetime.state().is_locked());
        assert!(lifetime.state().is_read_accessible());
        // three readers still keep every writer out
        assert!(!lifetime.state().is_write_accessible());
        assert!(lifetime.try_write_lock().is_none());
        let lock = lifetime.try_read_lock().unwrap();

        drop(lock);
        drop(third);
        drop(second);
        assert!(!lifetime.state().is_write_accessible());
        drop(first);
        assert!(lifetime.state().is_write_accessible());

        *lifetime.write(&mut value).unwrap() = 10;
        assert_eq!(value, 10);
    }

    #[test]
    fn test_write_access_guard_excludes_readers() {
        let mut value = 42usize;
        let lifetime = Lifetime::default();

        let guard = lifetime.write(&mut value).unwrap();
        assert!(!lifetime.state().is_locked());
        assert!(!lifetime.state().is_read_accessible());
        assert!(lifetime.try_read_lock().is_none());
        assert!(lifetime.lazy().read(&0).is_none());

        drop(guard);
        assert!(lifetime.state().is_read_accessible());
        assert!(lifetime.try_read_lock().is_some());
    }

    #[test]
    fn test_null_pointer_leaves_no_access_behind() {
        let lifetime = Lifetime::default();

        assert!(unsafe { lifetime.read_ptr(std::ptr::null::<usize>()) }.is_none());
        assert!(unsafe { lifetime.write_ptr(std::ptr::null_mut::<usize>()) }.is_none());

        // a refused guard must not leave its count raised
        assert!(lifetime.state().is_read_accessible());
        assert!(lifetime.state().is_write_accessible());
        assert!(lifetime.try_write_lock().is_some());
    }

    #[test]
    fn test_read_access_guards_across_threads() {
        let lifetime = Arc::new(Lifetime::default());
        let value = Arc::new(7usize);

        let threads = (0..8)
            .map(|_| {
                let lifetime = lifetime.clone();
                let value = value.clone();
                spawn(move || {
                    let mut taken = 0usize;
                    for _ in 0..1000 {
                        if let Some(access) = lifetime.read(value.as_ref()) {
                            assert_eq!(*access, 7);
                            taken += 1;
                        }
                    }
                    taken
                })
            })
            .collect::<Vec<_>>();
        let total = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .sum::<usize>();
        assert!(total > 0);

        // every guard is gone, so the counter has to be back at zero
        assert!(lifetime.state().is_write_accessible());
    }
}
