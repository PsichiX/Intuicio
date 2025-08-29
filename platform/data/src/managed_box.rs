#![allow(clippy::arc_with_non_send_sync)]

use crate::{
    Finalize,
    lifetime::{
        Lifetime, LifetimeLazy, LifetimeRef, LifetimeRefMut, ValueReadAccess, ValueWriteAccess,
    },
    managed::{
        DynamicManaged, DynamicManagedLazy, DynamicManagedRef, DynamicManagedRefMut, Managed,
        ManagedLazy, ManagedRef, ManagedRefMut,
    },
    type_hash::TypeHash,
};
use std::{alloc::Layout, cell::UnsafeCell, sync::Arc};

pub struct ManagedBox<T> {
    inner: Arc<UnsafeCell<Managed<T>>>,
}

unsafe impl<T> Send for ManagedBox<T> {}
unsafe impl<T> Sync for ManagedBox<T> {}

impl<T: Default> Default for ManagedBox<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> ManagedBox<T> {
    pub fn new(value: T) -> Self
    where
        T: Finalize,
    {
        Self {
            inner: Arc::new(UnsafeCell::new(Managed::new(value))),
        }
    }

    pub fn new_raw(data: T, lifetime: Lifetime) -> Self {
        Self {
            inner: Arc::new(UnsafeCell::new(Managed::new_raw(data, lifetime))),
        }
    }

    pub fn into_dynamic(self) -> Option<DynamicManagedBox> {
        Arc::try_unwrap(self.inner).ok().and_then(|inner| {
            inner
                .into_inner()
                .into_dynamic()
                .map(|result| DynamicManagedBox {
                    inner: Arc::new(UnsafeCell::new(result)),
                })
        })
    }

    pub fn instances_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }

    pub fn does_share_reference(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    pub fn type_hash(&self) -> TypeHash {
        TypeHash::of::<T>()
    }

    pub fn lifetime_borrow(&self) -> Option<LifetimeRef> {
        unsafe { (&*self.inner.get()).lifetime().borrow() }
    }

    pub fn lifetime_borrow_mut(&self) -> Option<LifetimeRefMut> {
        unsafe { (&*self.inner.get()).lifetime().borrow_mut() }
    }

    pub fn lifetime_lazy(&self) -> LifetimeLazy {
        unsafe { (&*self.inner.get()).lifetime().lazy() }
    }

    pub fn read(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        unsafe { (&*self.inner.get()).read() }
    }

    pub async fn read_async(&'_ self) -> ValueReadAccess<'_, T> {
        unsafe { (&*self.inner.get()).read_async().await }
    }

    pub fn write(&'_ mut self) -> Option<ValueWriteAccess<'_, T>> {
        unsafe { (&mut *self.inner.get()).write() }
    }

    pub async fn write_async(&'_ mut self) -> ValueWriteAccess<'_, T> {
        unsafe { (&mut *self.inner.get()).write_async().await }
    }

    pub fn borrow(&self) -> Option<ManagedRef<T>> {
        unsafe { (&*self.inner.get()).borrow() }
    }

    pub async fn borrow_async(&self) -> ManagedRef<T> {
        unsafe { (&*self.inner.get()).borrow_async().await }
    }

    pub fn borrow_mut(&mut self) -> Option<ManagedRefMut<T>> {
        unsafe { (&mut *self.inner.get()).borrow_mut() }
    }

    pub async fn borrow_mut_async(&mut self) -> ManagedRefMut<T> {
        unsafe { (&mut *self.inner.get()).borrow_mut_async().await }
    }

    pub fn lazy(&mut self) -> ManagedLazy<T> {
        unsafe { (&mut *self.inner.get()).lazy() }
    }

    /// # Safety
    pub unsafe fn lazy_immutable(&self) -> ManagedLazy<T> {
        unsafe { (&*self.inner.get()).lazy_immutable() }
    }

    /// # Safety
    pub unsafe fn as_ptr(&self) -> *const T {
        unsafe { (&*self.inner.get()).as_ptr() }
    }

    /// # Safety
    pub unsafe fn as_mut_ptr(&mut self) -> *mut T {
        unsafe { (&mut *self.inner.get()).as_mut_ptr() }
    }
}

impl<T> Clone for ManagedBox<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

pub struct DynamicManagedBox {
    inner: Arc<UnsafeCell<DynamicManaged>>,
}

unsafe impl Send for DynamicManagedBox {}
unsafe impl Sync for DynamicManagedBox {}

impl DynamicManagedBox {
    pub fn new<T: Finalize>(data: T) -> Result<Self, T> {
        Ok(Self {
            inner: Arc::new(UnsafeCell::new(DynamicManaged::new(data)?)),
        })
    }

    pub fn new_raw(
        type_hash: TypeHash,
        lifetime: Lifetime,
        memory: *mut u8,
        layout: Layout,
        finalizer: unsafe fn(*mut ()),
    ) -> Option<Self> {
        Some(Self {
            inner: Arc::new(UnsafeCell::new(DynamicManaged::new_raw(
                type_hash, lifetime, memory, layout, finalizer,
            )?)),
        })
    }

    pub fn new_uninitialized(
        type_hash: TypeHash,
        layout: Layout,
        finalizer: unsafe fn(*mut ()),
    ) -> Self {
        Self {
            inner: Arc::new(UnsafeCell::new(DynamicManaged::new_uninitialized(
                type_hash,
                layout.pad_to_align(),
                finalizer,
            ))),
        }
    }

    pub fn into_typed<T>(self) -> Result<ManagedBox<T>, Self> {
        match Arc::try_unwrap(self.inner) {
            Ok(inner) => match inner.into_inner().into_typed() {
                Ok(result) => Ok(ManagedBox {
                    inner: Arc::new(UnsafeCell::new(result)),
                }),
                Err(dynamic) => Err(Self {
                    inner: Arc::new(UnsafeCell::new(dynamic)),
                }),
            },
            Err(result) => Err(Self { inner: result }),
        }
    }

    pub fn instances_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }

    pub fn does_share_reference(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    pub fn type_hash(&self) -> TypeHash {
        unsafe { *(&*self.inner.get()).type_hash() }
    }

    pub fn lifetime_borrow(&self) -> Option<LifetimeRef> {
        unsafe { (&*self.inner.get()).lifetime().borrow() }
    }

    pub fn lifetime_borrow_mut(&self) -> Option<LifetimeRefMut> {
        unsafe { (&*self.inner.get()).lifetime().borrow_mut() }
    }

    pub fn lifetime_lazy(&self) -> LifetimeLazy {
        unsafe { (&*self.inner.get()).lifetime().lazy() }
    }

    pub fn is<T>(&self) -> bool {
        unsafe { (&*self.inner.get()).is::<T>() }
    }

    pub fn borrow(&self) -> Option<DynamicManagedRef> {
        unsafe { (&*self.inner.get()).borrow() }
    }

    pub async fn borrow_async(&self) -> DynamicManagedRef {
        unsafe { (&*self.inner.get()).borrow_async().await }
    }

    pub fn borrow_mut(&mut self) -> Option<DynamicManagedRefMut> {
        unsafe { (&mut *self.inner.get()).borrow_mut() }
    }

    pub async fn borrow_mut_async(&mut self) -> DynamicManagedRefMut {
        unsafe { (&mut *self.inner.get()).borrow_mut_async().await }
    }

    pub fn lazy(&self) -> DynamicManagedLazy {
        unsafe { (&*self.inner.get()).lazy() }
    }

    pub fn read<T>(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        unsafe { (&*self.inner.get()).read() }
    }

    pub async fn read_async<'a, T: 'a>(&'a self) -> Option<ValueReadAccess<'a, T>> {
        unsafe { (&*self.inner.get()).read_async().await }
    }

    pub fn write<T>(&'_ mut self) -> Option<ValueWriteAccess<'_, T>> {
        unsafe { (&mut *self.inner.get()).write() }
    }

    pub async fn write_async<'a, T: 'a>(&'a mut self) -> Option<ValueWriteAccess<'a, T>> {
        unsafe { (&mut *self.inner.get()).write_async().await }
    }

    /// # Safety
    pub unsafe fn memory(&self) -> &[u8] {
        unsafe { (&*self.inner.get()).memory() }
    }

    /// # Safety
    pub unsafe fn memory_mut(&mut self) -> &mut [u8] {
        unsafe { (&mut *self.inner.get()).memory_mut() }
    }

    /// # Safety
    pub unsafe fn as_ptr<T>(&self) -> Option<*const T> {
        unsafe { (&*self.inner.get()).as_ptr() }
    }

    /// # Safety
    pub unsafe fn as_mut_ptr<T>(&mut self) -> Option<*mut T> {
        unsafe { (&mut *self.inner.get()).as_mut_ptr() }
    }

    /// # Safety
    pub unsafe fn as_ptr_raw(&self) -> *const u8 {
        unsafe { (&*self.inner.get()).as_ptr_raw() }
    }

    /// # Safety
    pub unsafe fn as_mut_ptr_raw(&mut self) -> *mut u8 {
        unsafe { (&mut *self.inner.get()).as_mut_ptr_raw() }
    }
}

impl Clone for DynamicManagedBox {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_async<T: Send + Sync>() {}

    #[test]
    fn test_managed_box() {
        is_async::<ManagedBox<i32>>();

        let a = ManagedBox::new(42usize);
        assert_eq!(*a.read().unwrap(), 42);
        assert_eq!(a.instances_count(), 1);
        let mut b = a.clone();
        assert_eq!(a.instances_count(), 2);
        assert_eq!(b.instances_count(), 2);
        assert!(a.does_share_reference(&b));
        assert_eq!(*b.read().unwrap(), 42);
        *b.write().unwrap() = 10;
        assert_eq!(*a.read().unwrap(), 10);
        assert_eq!(*b.read().unwrap(), 10);
        drop(a);
        assert_eq!(b.instances_count(), 1);
        drop(b);
    }

    #[test]
    fn test_dynamic_managed_box() {
        is_async::<DynamicManagedBox>();

        let a = DynamicManagedBox::new(42usize).ok().unwrap();
        assert!(a.is::<usize>());
        assert_eq!(*a.read::<usize>().unwrap(), 42);
        assert_eq!(a.instances_count(), 1);
        let mut b = a.clone();
        assert!(b.is::<usize>());
        assert_eq!(a.instances_count(), 2);
        assert_eq!(b.instances_count(), 2);
        assert!(a.does_share_reference(&b));
        assert_eq!(*b.read::<usize>().unwrap(), 42);
        *b.write::<usize>().unwrap() = 10;
        assert_eq!(*a.read::<usize>().unwrap(), 10);
        assert_eq!(*b.read::<usize>().unwrap(), 10);
        drop(a);
        assert_eq!(b.instances_count(), 1);
        drop(b);
    }

    #[test]
    fn test_managed_box_borrows() {
        let v = ManagedBox::new(42usize);
        let r = v.borrow().unwrap();
        drop(v);
        assert!(r.read().is_none());
    }

    #[test]
    fn test_fuzz_managed_box() {
        let builders = [
            || DynamicManagedBox::new(1u8).ok().unwrap(),
            || DynamicManagedBox::new(2u16).ok().unwrap(),
            || DynamicManagedBox::new(3u32).ok().unwrap(),
            || DynamicManagedBox::new(4u64).ok().unwrap(),
            || DynamicManagedBox::new(5u128).ok().unwrap(),
            || DynamicManagedBox::new([42u8; 1000]).ok().unwrap(),
            || DynamicManagedBox::new([42u8; 10000]).ok().unwrap(),
            || DynamicManagedBox::new([42u8; 100000]).ok().unwrap(),
        ];
        let mut boxes = std::array::from_fn::<_, 50, _>(|_| None);
        for index in 0..100 {
            let source = index % builders.len();
            let target = index % boxes.len();
            boxes[target] = Some((builders[source])());
        }
    }
}
