pub mod gc;
pub mod value;

use crate::{
    Finalize,
    lifetime::{
        Lifetime, LifetimeLazy, LifetimeRef, LifetimeRefMut, ValueReadAccess, ValueWriteAccess,
    },
    managed::value::{DynamicManagedValue, ManagedValue},
    non_zero_alloc, non_zero_dealloc,
    type_hash::TypeHash,
};
use std::{alloc::Layout, mem::MaybeUninit};

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

    pub fn new_raw(data: T, lifetime: Lifetime) -> Self {
        Self { lifetime, data }
    }

    pub fn into_inner(self) -> (Lifetime, T) {
        (self.lifetime, self.data)
    }

    pub fn into_dynamic(self) -> Result<DynamicManaged, Self> {
        match DynamicManaged::new(self.data) {
            Ok(value) => Ok(value),
            Err(data) => Err(Managed {
                lifetime: self.lifetime,
                data,
            }),
        }
    }

    pub fn renew(mut self) -> Self {
        self.lifetime = Lifetime::default();
        self
    }

    pub fn lifetime(&self) -> &Lifetime {
        &self.lifetime
    }

    pub fn read(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        self.lifetime.read(&self.data)
    }

    pub fn write(&'_ mut self) -> Option<ValueWriteAccess<'_, T>> {
        self.lifetime.write(&mut self.data)
    }

    pub fn consume(self) -> Result<T, Self> {
        if self.lifetime.state().is_in_use() {
            Err(self)
        } else {
            Ok(self.data)
        }
    }

    pub fn move_into_ref(self, mut target: ManagedRefMut<T>) -> Result<(), Self> {
        *target.write().unwrap() = self.consume()?;
        Ok(())
    }

    pub fn move_into_lazy(self, target: ManagedLazy<T>) -> Result<(), Self> {
        *target.write().unwrap() = self.consume()?;
        Ok(())
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

    pub fn lazy(&mut self) -> ManagedLazy<T> {
        ManagedLazy::new(&mut self.data, self.lifetime.lazy())
    }

    /// # Safety
    pub unsafe fn lazy_immutable(&self) -> ManagedLazy<T> {
        unsafe {
            ManagedLazy::new_raw(&self.data as *const T as *mut T, self.lifetime.lazy()).unwrap()
        }
    }

    /// # Safety
    pub unsafe fn map<U>(self, f: impl FnOnce(T) -> U) -> Managed<U> {
        Managed {
            lifetime: Default::default(),
            data: f(self.data),
        }
    }

    /// # Safety
    pub unsafe fn try_map<U>(self, f: impl FnOnce(T) -> Option<U>) -> Option<Managed<U>> {
        f(self.data).map(|data| Managed {
            lifetime: Default::default(),
            data,
        })
    }

    /// # Safety
    pub unsafe fn as_ptr(&self) -> *const T {
        &self.data as _
    }

    /// # Safety
    pub unsafe fn as_mut_ptr(&mut self) -> *mut T {
        &mut self.data as _
    }
}

pub struct ManagedRef<T: ?Sized> {
    lifetime: LifetimeRef,
    data: *const T,
}

unsafe impl<T: ?Sized> Send for ManagedRef<T> where T: Send {}
unsafe impl<T: ?Sized> Sync for ManagedRef<T> where T: Sync {}

impl<T: ?Sized> ManagedRef<T> {
    pub fn new(data: &T, lifetime: LifetimeRef) -> Self {
        Self {
            lifetime,
            data: data as *const T,
        }
    }

    /// # Safety
    pub unsafe fn new_raw(data: *const T, lifetime: LifetimeRef) -> Option<Self> {
        if data.is_null() {
            None
        } else {
            Some(Self { lifetime, data })
        }
    }

    pub fn make(data: &T) -> (Self, Lifetime) {
        let result = Lifetime::default();
        (Self::new(data, result.borrow().unwrap()), result)
    }

    /// # Safety
    pub unsafe fn make_raw(data: *const T) -> Option<(Self, Lifetime)> {
        let result = Lifetime::default();
        Some((
            unsafe { Self::new_raw(data, result.borrow().unwrap()) }?,
            result,
        ))
    }

    pub fn into_inner(self) -> (LifetimeRef, *const T) {
        (self.lifetime, self.data)
    }

    pub fn into_dynamic(self) -> DynamicManagedRef {
        unsafe {
            DynamicManagedRef::new_raw(TypeHash::of::<T>(), self.lifetime, self.data as *const u8)
                .unwrap()
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

    /// # Safety
    pub unsafe fn lazy_immutable(&self) -> ManagedLazy<T> {
        ManagedLazy {
            lifetime: self.lifetime.lazy(),
            data: self.data as *mut T,
        }
    }

    pub fn read(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        unsafe { self.lifetime.read_ptr(self.data) }
    }

    /// # Safety
    pub unsafe fn map<U>(self, f: impl FnOnce(&T) -> &U) -> ManagedRef<U> {
        unsafe {
            let data = f(&*self.data);
            ManagedRef {
                lifetime: self.lifetime,
                data: data as *const U,
            }
        }
    }

    /// # Safety
    pub unsafe fn try_map<U>(self, f: impl FnOnce(&T) -> Option<&U>) -> Option<ManagedRef<U>> {
        unsafe {
            f(&*self.data).map(|data| ManagedRef {
                lifetime: self.lifetime,
                data: data as *const U,
            })
        }
    }

    /// # Safety
    pub unsafe fn as_ptr(&self) -> Option<*const T> {
        if self.lifetime.exists() {
            Some(self.data)
        } else {
            None
        }
    }
}

impl<T> TryFrom<ManagedValue<T>> for ManagedRef<T> {
    type Error = ();

    fn try_from(value: ManagedValue<T>) -> Result<Self, Self::Error> {
        match value {
            ManagedValue::Ref(value) => Ok(value),
            _ => Err(()),
        }
    }
}

pub struct ManagedRefMut<T: ?Sized> {
    lifetime: LifetimeRefMut,
    data: *mut T,
}

unsafe impl<T: ?Sized> Send for ManagedRefMut<T> where T: Send {}
unsafe impl<T: ?Sized> Sync for ManagedRefMut<T> where T: Sync {}

impl<T: ?Sized> ManagedRefMut<T> {
    pub fn new(data: &mut T, lifetime: LifetimeRefMut) -> Self {
        Self {
            lifetime,
            data: data as *mut T,
        }
    }

    /// # Safety
    pub unsafe fn new_raw(data: *mut T, lifetime: LifetimeRefMut) -> Option<Self> {
        if data.is_null() {
            None
        } else {
            Some(Self { lifetime, data })
        }
    }

    pub fn make(data: &mut T) -> (Self, Lifetime) {
        let result = Lifetime::default();
        (Self::new(data, result.borrow_mut().unwrap()), result)
    }

    /// # Safety
    pub unsafe fn make_raw(data: *mut T) -> Option<(Self, Lifetime)> {
        let result = Lifetime::default();
        Some((
            unsafe { Self::new_raw(data, result.borrow_mut().unwrap()) }?,
            result,
        ))
    }

    pub fn into_inner(self) -> (LifetimeRefMut, *mut T) {
        (self.lifetime, self.data)
    }

    pub fn into_dynamic(self) -> DynamicManagedRefMut {
        unsafe {
            DynamicManagedRefMut::new_raw(TypeHash::of::<T>(), self.lifetime, self.data as *mut u8)
                .unwrap()
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

    pub fn borrow_mut(&mut self) -> Option<ManagedRefMut<T>> {
        Some(ManagedRefMut {
            lifetime: self.lifetime.borrow_mut()?,
            data: self.data,
        })
    }

    pub fn lazy(&self) -> ManagedLazy<T> {
        ManagedLazy {
            lifetime: self.lifetime.lazy(),
            data: self.data,
        }
    }

    pub fn read(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        unsafe { self.lifetime.read_ptr(self.data) }
    }

    pub fn write(&'_ mut self) -> Option<ValueWriteAccess<'_, T>> {
        unsafe { self.lifetime.write_ptr(self.data) }
    }

    /// # Safety
    pub unsafe fn map<U>(self, f: impl FnOnce(&mut T) -> &mut U) -> ManagedRefMut<U> {
        unsafe {
            let data = f(&mut *self.data);
            ManagedRefMut {
                lifetime: self.lifetime,
                data: data as *mut U,
            }
        }
    }

    /// # Safety
    pub unsafe fn try_map<U>(
        self,
        f: impl FnOnce(&mut T) -> Option<&mut U>,
    ) -> Option<ManagedRefMut<U>> {
        unsafe {
            f(&mut *self.data).map(|data| ManagedRefMut {
                lifetime: self.lifetime,
                data: data as *mut U,
            })
        }
    }

    /// # Safety
    pub unsafe fn as_ptr(&self) -> Option<*const T> {
        if self.lifetime.exists() {
            Some(self.data)
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn as_mut_ptr(&mut self) -> Option<*mut T> {
        if self.lifetime.exists() {
            Some(self.data)
        } else {
            None
        }
    }
}

impl<T> TryFrom<ManagedValue<T>> for ManagedRefMut<T> {
    type Error = ();

    fn try_from(value: ManagedValue<T>) -> Result<Self, Self::Error> {
        match value {
            ManagedValue::RefMut(value) => Ok(value),
            _ => Err(()),
        }
    }
}

pub struct ManagedLazy<T: ?Sized> {
    lifetime: LifetimeLazy,
    data: *mut T,
}

unsafe impl<T: ?Sized> Send for ManagedLazy<T> where T: Send {}
unsafe impl<T: ?Sized> Sync for ManagedLazy<T> where T: Sync {}

impl<T: ?Sized> Clone for ManagedLazy<T> {
    fn clone(&self) -> Self {
        Self {
            lifetime: self.lifetime.clone(),
            data: self.data,
        }
    }
}

impl<T: ?Sized> ManagedLazy<T> {
    pub fn new(data: &mut T, lifetime: LifetimeLazy) -> Self {
        Self {
            lifetime,
            data: data as *mut T,
        }
    }

    /// # Safety
    pub unsafe fn new_raw(data: *mut T, lifetime: LifetimeLazy) -> Option<Self> {
        if data.is_null() {
            None
        } else {
            Some(Self { lifetime, data })
        }
    }

    pub fn make(data: &mut T) -> (Self, Lifetime) {
        let result = Lifetime::default();
        (Self::new(data, result.lazy()), result)
    }

    /// # Safety
    pub unsafe fn make_raw(data: *mut T) -> Option<(Self, Lifetime)> {
        let result = Lifetime::default();
        Some((unsafe { Self::new_raw(data, result.lazy()) }?, result))
    }

    pub fn into_inner(self) -> (LifetimeLazy, *mut T) {
        (self.lifetime, self.data)
    }

    pub fn into_dynamic(self) -> DynamicManagedLazy {
        unsafe {
            DynamicManagedLazy::new_raw(TypeHash::of::<T>(), self.lifetime, self.data as *mut u8)
                .unwrap()
        }
    }

    pub fn lifetime(&self) -> &LifetimeLazy {
        &self.lifetime
    }

    pub fn borrow(&self) -> Option<ManagedRef<T>> {
        Some(ManagedRef {
            lifetime: self.lifetime.borrow()?,
            data: self.data,
        })
    }

    pub fn borrow_mut(&mut self) -> Option<ManagedRefMut<T>> {
        Some(ManagedRefMut {
            lifetime: self.lifetime.borrow_mut()?,
            data: self.data,
        })
    }

    pub fn read(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        unsafe { self.lifetime.read_ptr(self.data) }
    }

    pub fn write(&'_ self) -> Option<ValueWriteAccess<'_, T>> {
        unsafe { self.lifetime.write_ptr(self.data) }
    }

    /// # Safety
    pub unsafe fn map<U>(self, f: impl FnOnce(&mut T) -> &mut U) -> ManagedLazy<U> {
        unsafe {
            let data = f(&mut *self.data);
            ManagedLazy {
                lifetime: self.lifetime,
                data: data as *mut U,
            }
        }
    }

    /// # Safety
    pub unsafe fn try_map<U>(
        self,
        f: impl FnOnce(&mut T) -> Option<&mut U>,
    ) -> Option<ManagedLazy<U>> {
        unsafe {
            f(&mut *self.data).map(|data| ManagedLazy {
                lifetime: self.lifetime,
                data: data as *mut U,
            })
        }
    }

    /// # Safety
    pub unsafe fn as_ptr(&self) -> Option<*const T> {
        if self.lifetime.exists() {
            Some(self.data)
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn as_mut_ptr(&self) -> Option<*mut T> {
        if self.lifetime.exists() {
            Some(self.data)
        } else {
            None
        }
    }
}

impl<T> TryFrom<ManagedValue<T>> for ManagedLazy<T> {
    type Error = ();

    fn try_from(value: ManagedValue<T>) -> Result<Self, Self::Error> {
        match value {
            ManagedValue::Lazy(value) => Ok(value),
            _ => Err(()),
        }
    }
}

pub struct DynamicManaged {
    type_hash: TypeHash,
    lifetime: Lifetime,
    memory: *mut u8,
    layout: Layout,
    finalizer: unsafe fn(*mut ()),
    drop: bool,
}

unsafe impl Send for DynamicManaged {}
unsafe impl Sync for DynamicManaged {}

impl Drop for DynamicManaged {
    fn drop(&mut self) {
        if self.drop {
            unsafe {
                if self.memory.is_null() {
                    return;
                }
                let data_pointer = self.memory.cast::<()>();
                (self.finalizer)(data_pointer);
                non_zero_dealloc(self.memory, self.layout);
                self.memory = std::ptr::null_mut();
            }
        }
    }
}

impl DynamicManaged {
    pub fn new<T: Finalize>(data: T) -> Result<Self, T> {
        let layout = Layout::new::<T>().pad_to_align();
        unsafe {
            let memory = non_zero_alloc(layout);
            if memory.is_null() {
                Err(data)
            } else {
                memory.cast::<T>().write(data);
                Ok(Self {
                    type_hash: TypeHash::of::<T>(),
                    lifetime: Default::default(),
                    memory,
                    layout,
                    finalizer: T::finalize_raw,
                    drop: true,
                })
            }
        }
    }

    pub fn new_raw(
        type_hash: TypeHash,
        lifetime: Lifetime,
        memory: *mut u8,
        layout: Layout,
        finalizer: unsafe fn(*mut ()),
    ) -> Option<Self> {
        if memory.is_null() {
            None
        } else {
            Some(Self {
                type_hash,
                lifetime,
                memory,
                layout,
                finalizer,
                drop: true,
            })
        }
    }

    pub fn new_uninitialized(
        type_hash: TypeHash,
        layout: Layout,
        finalizer: unsafe fn(*mut ()),
    ) -> Self {
        let layout = layout.pad_to_align();
        let memory = unsafe { non_zero_alloc(layout) };
        Self {
            type_hash,
            lifetime: Default::default(),
            memory,
            layout,
            finalizer,
            drop: true,
        }
    }

    /// # Safety
    pub unsafe fn from_bytes(
        type_hash: TypeHash,
        lifetime: Lifetime,
        bytes: Vec<u8>,
        layout: Layout,
        finalizer: unsafe fn(*mut ()),
    ) -> Self {
        let layout = layout.pad_to_align();
        let memory = unsafe { non_zero_alloc(layout) };
        unsafe { memory.copy_from(bytes.as_ptr(), bytes.len()) };
        Self {
            type_hash,
            lifetime,
            memory,
            layout,
            finalizer,
            drop: true,
        }
    }

    #[allow(clippy::type_complexity)]
    pub fn into_inner(mut self) -> (TypeHash, Lifetime, *mut u8, Layout, unsafe fn(*mut ())) {
        self.drop = false;
        (
            self.type_hash,
            std::mem::take(&mut self.lifetime),
            self.memory,
            self.layout,
            self.finalizer,
        )
    }

    pub fn into_typed<T>(self) -> Result<Managed<T>, Self> {
        Ok(Managed::new(self.consume()?))
    }

    pub fn renew(mut self) -> Self {
        self.lifetime = Lifetime::default();
        self
    }

    pub fn type_hash(&self) -> &TypeHash {
        &self.type_hash
    }

    pub fn lifetime(&self) -> &Lifetime {
        &self.lifetime
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn finalizer(&self) -> unsafe fn(*mut ()) {
        self.finalizer
    }

    /// # Safety
    pub unsafe fn memory(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.memory, self.layout.size()) }
    }

    /// # Safety
    pub unsafe fn memory_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.memory, self.layout.size()) }
    }

    pub fn is<T>(&self) -> bool {
        self.type_hash == TypeHash::of::<T>()
    }

    pub fn read<T>(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { self.lifetime.read_ptr(self.memory.cast::<T>()) }
        } else {
            None
        }
    }

    pub fn write<T>(&'_ mut self) -> Option<ValueWriteAccess<'_, T>> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { self.lifetime.write_ptr(self.memory.cast::<T>()) }
        } else {
            None
        }
    }

    pub fn consume<T>(mut self) -> Result<T, Self> {
        if self.type_hash == TypeHash::of::<T>() && !self.lifetime.state().is_in_use() {
            if self.memory.is_null() {
                return Err(self);
            }
            self.drop = false;
            let mut result = MaybeUninit::<T>::uninit();
            unsafe {
                result.as_mut_ptr().copy_from(self.memory.cast::<T>(), 1);
                non_zero_dealloc(self.memory, self.layout);
                self.memory = std::ptr::null_mut();
                Ok(result.assume_init())
            }
        } else {
            Err(self)
        }
    }

    pub fn move_into_ref(self, target: DynamicManagedRefMut) -> Result<(), Self> {
        if self.type_hash == target.type_hash && self.memory != target.data {
            if self.memory.is_null() {
                return Err(self);
            }
            let (_, _, memory, layout, _) = self.into_inner();
            unsafe {
                target.data.copy_from(memory, layout.size());
                non_zero_dealloc(memory, layout);
            }
            Ok(())
        } else {
            Err(self)
        }
    }

    pub fn move_into_lazy(self, target: DynamicManagedLazy) -> Result<(), Self> {
        if self.type_hash == target.type_hash && self.memory != target.data {
            if self.memory.is_null() {
                return Err(self);
            }
            let (_, _, memory, layout, _) = self.into_inner();
            unsafe {
                target.data.copy_from(memory, layout.size());
                non_zero_dealloc(memory, layout);
            }
            Ok(())
        } else {
            Err(self)
        }
    }

    pub fn borrow(&self) -> Option<DynamicManagedRef> {
        unsafe { DynamicManagedRef::new_raw(self.type_hash, self.lifetime.borrow()?, self.memory) }
    }

    pub fn borrow_mut(&mut self) -> Option<DynamicManagedRefMut> {
        unsafe {
            DynamicManagedRefMut::new_raw(self.type_hash, self.lifetime.borrow_mut()?, self.memory)
        }
    }

    pub fn lazy(&self) -> DynamicManagedLazy {
        unsafe {
            DynamicManagedLazy::new_raw(self.type_hash, self.lifetime.lazy(), self.memory).unwrap()
        }
    }

    /// # Safety
    pub unsafe fn map<T, U: Finalize>(self, f: impl FnOnce(T) -> U) -> Option<Self> {
        let data = self.consume::<T>().ok()?;
        let data = f(data);
        Self::new(data).ok()
    }

    /// # Safety
    pub unsafe fn try_map<T, U: Finalize>(self, f: impl FnOnce(T) -> Option<U>) -> Option<Self> {
        let data = self.consume::<T>().ok()?;
        let data = f(data)?;
        Self::new(data).ok()
    }

    /// # Safety
    pub unsafe fn as_ptr<T>(&self) -> Option<*const T> {
        if self.type_hash == TypeHash::of::<T>() && !self.lifetime.state().is_in_use() {
            Some(self.memory.cast::<T>())
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn as_mut_ptr<T>(&mut self) -> Option<*mut T> {
        if self.type_hash == TypeHash::of::<T>() && !self.lifetime.state().is_in_use() {
            Some(self.memory.cast::<T>())
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn as_ptr_raw(&self) -> *const u8 {
        self.memory
    }

    /// # Safety
    pub unsafe fn as_mut_ptr_raw(&mut self) -> *mut u8 {
        self.memory
    }
}

impl TryFrom<DynamicManagedValue> for DynamicManaged {
    type Error = ();

    fn try_from(value: DynamicManagedValue) -> Result<Self, Self::Error> {
        match value {
            DynamicManagedValue::Owned(value) => Ok(value),
            _ => Err(()),
        }
    }
}

pub struct DynamicManagedRef {
    type_hash: TypeHash,
    lifetime: LifetimeRef,
    data: *const u8,
}

unsafe impl Send for DynamicManagedRef {}
unsafe impl Sync for DynamicManagedRef {}

impl DynamicManagedRef {
    pub fn new<T: ?Sized>(data: &T, lifetime: LifetimeRef) -> Self {
        Self {
            type_hash: TypeHash::of::<T>(),
            lifetime,
            data: data as *const T as *const u8,
        }
    }

    /// # Safety
    pub unsafe fn new_raw(
        type_hash: TypeHash,
        lifetime: LifetimeRef,
        data: *const u8,
    ) -> Option<Self> {
        if data.is_null() {
            None
        } else {
            Some(Self {
                type_hash,
                lifetime,
                data,
            })
        }
    }

    pub fn make<T: ?Sized>(data: &T) -> (Self, Lifetime) {
        let result = Lifetime::default();
        (Self::new(data, result.borrow().unwrap()), result)
    }

    pub fn into_inner(self) -> (TypeHash, LifetimeRef, *const u8) {
        (self.type_hash, self.lifetime, self.data)
    }

    pub fn into_typed<T>(self) -> Result<ManagedRef<T>, Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { Ok(ManagedRef::new_raw(self.data.cast::<T>(), self.lifetime).unwrap()) }
        } else {
            Err(self)
        }
    }

    pub fn type_hash(&self) -> &TypeHash {
        &self.type_hash
    }

    pub fn lifetime(&self) -> &LifetimeRef {
        &self.lifetime
    }

    pub fn borrow(&self) -> Option<DynamicManagedRef> {
        Some(DynamicManagedRef {
            type_hash: self.type_hash,
            lifetime: self.lifetime.borrow()?,
            data: self.data,
        })
    }

    /// # Safety
    pub unsafe fn lazy_immutable(&self) -> DynamicManagedLazy {
        DynamicManagedLazy {
            type_hash: self.type_hash,
            lifetime: self.lifetime.lazy(),
            data: self.data as *mut u8,
        }
    }

    pub fn is<T>(&self) -> bool {
        self.type_hash == TypeHash::of::<T>()
    }

    pub fn read<T>(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { self.lifetime.read_ptr(self.data.cast::<T>()) }
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn map<T, U>(self, f: impl FnOnce(&T) -> &U) -> Option<Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe {
                let data = f(&*self.data.cast::<T>());
                Some(Self {
                    type_hash: TypeHash::of::<U>(),
                    lifetime: self.lifetime,
                    data: data as *const U as *const u8,
                })
            }
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn try_map<T, U>(self, f: impl FnOnce(&T) -> Option<&U>) -> Option<Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe {
                let data = f(&*self.data.cast::<T>())?;
                Some(Self {
                    type_hash: TypeHash::of::<U>(),
                    lifetime: self.lifetime,
                    data: data as *const U as *const u8,
                })
            }
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn as_ptr<T>(&self) -> Option<*const T> {
        if self.type_hash == TypeHash::of::<T>() && self.lifetime.exists() {
            Some(self.data.cast::<T>())
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn as_ptr_raw(&self) -> Option<*const u8> {
        if self.lifetime.exists() {
            Some(self.data)
        } else {
            None
        }
    }
}

impl TryFrom<DynamicManagedValue> for DynamicManagedRef {
    type Error = ();

    fn try_from(value: DynamicManagedValue) -> Result<Self, Self::Error> {
        match value {
            DynamicManagedValue::Ref(value) => Ok(value),
            _ => Err(()),
        }
    }
}

pub struct DynamicManagedRefMut {
    type_hash: TypeHash,
    lifetime: LifetimeRefMut,
    data: *mut u8,
}

unsafe impl Send for DynamicManagedRefMut {}
unsafe impl Sync for DynamicManagedRefMut {}

impl DynamicManagedRefMut {
    pub fn new<T: ?Sized>(data: &mut T, lifetime: LifetimeRefMut) -> Self {
        Self {
            type_hash: TypeHash::of::<T>(),
            lifetime,
            data: data as *mut T as *mut u8,
        }
    }

    /// # Safety
    pub unsafe fn new_raw(
        type_hash: TypeHash,
        lifetime: LifetimeRefMut,
        data: *mut u8,
    ) -> Option<Self> {
        if data.is_null() {
            None
        } else {
            Some(Self {
                type_hash,
                lifetime,
                data,
            })
        }
    }

    pub fn make<T: ?Sized>(data: &mut T) -> (Self, Lifetime) {
        let result = Lifetime::default();
        (Self::new(data, result.borrow_mut().unwrap()), result)
    }

    pub fn into_inner(self) -> (TypeHash, LifetimeRefMut, *mut u8) {
        (self.type_hash, self.lifetime, self.data)
    }

    pub fn into_typed<T>(self) -> Result<ManagedRefMut<T>, Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { Ok(ManagedRefMut::new_raw(self.data.cast::<T>(), self.lifetime).unwrap()) }
        } else {
            Err(self)
        }
    }

    pub fn type_hash(&self) -> &TypeHash {
        &self.type_hash
    }

    pub fn lifetime(&self) -> &LifetimeRefMut {
        &self.lifetime
    }

    pub fn borrow(&self) -> Option<DynamicManagedRef> {
        Some(DynamicManagedRef {
            type_hash: self.type_hash,
            lifetime: self.lifetime.borrow()?,
            data: self.data,
        })
    }

    pub fn borrow_mut(&mut self) -> Option<DynamicManagedRefMut> {
        Some(DynamicManagedRefMut {
            type_hash: self.type_hash,
            lifetime: self.lifetime.borrow_mut()?,
            data: self.data,
        })
    }

    pub fn lazy(&self) -> DynamicManagedLazy {
        DynamicManagedLazy {
            type_hash: self.type_hash,
            lifetime: self.lifetime.lazy(),
            data: self.data,
        }
    }

    pub fn is<T>(&self) -> bool {
        self.type_hash == TypeHash::of::<T>()
    }

    pub fn read<T>(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { self.lifetime.read_ptr(self.data.cast::<T>()) }
        } else {
            None
        }
    }

    pub fn write<T>(&'_ mut self) -> Option<ValueWriteAccess<'_, T>> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { self.lifetime.write_ptr(self.data.cast::<T>()) }
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn map<T, U>(self, f: impl FnOnce(&mut T) -> &mut U) -> Option<Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe {
                let data = f(&mut *self.data.cast::<T>());
                Some(Self {
                    type_hash: TypeHash::of::<U>(),
                    lifetime: self.lifetime,
                    data: data as *mut U as *mut u8,
                })
            }
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn try_map<T, U>(self, f: impl FnOnce(&mut T) -> Option<&mut U>) -> Option<Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe {
                let data = f(&mut *self.data.cast::<T>())?;
                Some(Self {
                    type_hash: TypeHash::of::<U>(),
                    lifetime: self.lifetime,
                    data: data as *mut U as *mut u8,
                })
            }
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn as_ptr<T>(&self) -> Option<*const T> {
        if self.type_hash == TypeHash::of::<T>() && self.lifetime.exists() {
            Some(self.data.cast::<T>())
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn as_mut_ptr<T>(&mut self) -> Option<*mut T> {
        if self.type_hash == TypeHash::of::<T>() && self.lifetime.exists() {
            Some(self.data.cast::<T>())
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn as_ptr_raw(&self) -> Option<*const u8> {
        if self.lifetime.exists() {
            Some(self.data)
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn as_mut_ptr_raw(&mut self) -> Option<*mut u8> {
        if self.lifetime.exists() {
            Some(self.data)
        } else {
            None
        }
    }
}

impl TryFrom<DynamicManagedValue> for DynamicManagedRefMut {
    type Error = ();

    fn try_from(value: DynamicManagedValue) -> Result<Self, Self::Error> {
        match value {
            DynamicManagedValue::RefMut(value) => Ok(value),
            _ => Err(()),
        }
    }
}

pub struct DynamicManagedLazy {
    type_hash: TypeHash,
    lifetime: LifetimeLazy,
    data: *mut u8,
}

unsafe impl Send for DynamicManagedLazy {}
unsafe impl Sync for DynamicManagedLazy {}

impl Clone for DynamicManagedLazy {
    fn clone(&self) -> Self {
        Self {
            type_hash: self.type_hash,
            lifetime: self.lifetime.clone(),
            data: self.data,
        }
    }
}

impl DynamicManagedLazy {
    pub fn new<T: ?Sized>(data: &mut T, lifetime: LifetimeLazy) -> Self {
        Self {
            type_hash: TypeHash::of::<T>(),
            lifetime,
            data: data as *mut T as *mut u8,
        }
    }

    /// # Safety
    pub unsafe fn new_raw(
        type_hash: TypeHash,
        lifetime: LifetimeLazy,
        data: *mut u8,
    ) -> Option<Self> {
        if data.is_null() {
            None
        } else {
            Some(Self {
                type_hash,
                lifetime,
                data,
            })
        }
    }

    pub fn make<T: ?Sized>(data: &mut T) -> (Self, Lifetime) {
        let result = Lifetime::default();
        (Self::new(data, result.lazy()), result)
    }

    pub fn into_inner(self) -> (TypeHash, LifetimeLazy, *mut u8) {
        (self.type_hash, self.lifetime, self.data)
    }

    pub fn into_typed<T>(self) -> Result<ManagedLazy<T>, Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { Ok(ManagedLazy::new_raw(self.data.cast::<T>(), self.lifetime).unwrap()) }
        } else {
            Err(self)
        }
    }

    pub fn type_hash(&self) -> &TypeHash {
        &self.type_hash
    }

    pub fn lifetime(&self) -> &LifetimeLazy {
        &self.lifetime
    }

    pub fn is<T>(&self) -> bool {
        self.type_hash == TypeHash::of::<T>()
    }

    pub fn read<T>(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { self.lifetime.read_ptr(self.data.cast::<T>()) }
        } else {
            None
        }
    }

    pub fn write<T>(&'_ self) -> Option<ValueWriteAccess<'_, T>> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { self.lifetime.write_ptr(self.data.cast::<T>()) }
        } else {
            None
        }
    }

    pub fn borrow(&self) -> Option<DynamicManagedRef> {
        Some(DynamicManagedRef {
            type_hash: self.type_hash,
            lifetime: self.lifetime.borrow()?,
            data: self.data,
        })
    }

    pub fn borrow_mut(&mut self) -> Option<DynamicManagedRefMut> {
        Some(DynamicManagedRefMut {
            type_hash: self.type_hash,
            lifetime: self.lifetime.borrow_mut()?,
            data: self.data,
        })
    }

    /// # Safety
    pub unsafe fn map<T, U>(self, f: impl FnOnce(&mut T) -> &mut U) -> Option<Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe {
                let data = f(&mut *self.data.cast::<T>());
                Some(Self {
                    type_hash: TypeHash::of::<U>(),
                    lifetime: self.lifetime,
                    data: data as *mut U as *mut u8,
                })
            }
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn try_map<T, U>(self, f: impl FnOnce(&mut T) -> Option<&mut U>) -> Option<Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe {
                let data = f(&mut *self.data.cast::<T>())?;
                Some(Self {
                    type_hash: TypeHash::of::<U>(),
                    lifetime: self.lifetime,
                    data: data as *mut U as *mut u8,
                })
            }
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn as_ptr<T>(&self) -> Option<*const T> {
        if self.type_hash == TypeHash::of::<T>() && self.lifetime.exists() {
            Some(self.data.cast::<T>())
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn as_mut_ptr<T>(&self) -> Option<*mut T> {
        if self.type_hash == TypeHash::of::<T>() && self.lifetime.exists() {
            Some(self.data.cast::<T>())
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn as_ptr_raw(&self) -> Option<*const u8> {
        if self.lifetime.exists() {
            Some(self.data)
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn as_mut_ptr_raw(&mut self) -> Option<*mut u8> {
        if self.lifetime.exists() {
            Some(self.data)
        } else {
            None
        }
    }
}

impl TryFrom<DynamicManagedValue> for DynamicManagedLazy {
    type Error = ();

    fn try_from(value: DynamicManagedValue) -> Result<Self, Self::Error> {
        match value {
            DynamicManagedValue::Lazy(value) => Ok(value),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::Any;

    fn is_async<T: Send + Sync + ?Sized>() {}

    #[test]
    fn test_managed() {
        is_async::<Managed<()>>();
        is_async::<ManagedRef<()>>();
        is_async::<ManagedRefMut<()>>();
        is_async::<ManagedLazy<()>>();
        is_async::<ManagedValue<()>>();

        let mut value = Managed::new(42);
        let mut value_ref = value.borrow_mut().unwrap();
        assert!(value_ref.write().is_some());
        let mut value_ref2 = value_ref.borrow_mut().unwrap();
        assert!(value_ref.write().is_some());
        assert!(value_ref2.write().is_some());
        drop(value_ref);
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
        let value_ref2 = value_ref.borrow().unwrap();
        drop(value_ref);
        assert!(value_ref2.read().is_some());
        let value_ref = value.borrow().unwrap();
        let value_lazy = value.lazy();
        assert_eq!(*value_lazy.read().unwrap(), 2);
        *value_lazy.write().unwrap() = 42;
        assert_eq!(*value_lazy.read().unwrap(), 42);
        drop(value);
        assert!(value_ref.read().is_none());
        assert!(value_ref2.read().is_none());
        assert!(value_lazy.read().is_none());
    }

    #[test]
    fn test_dynamic_managed() {
        is_async::<DynamicManaged>();
        is_async::<DynamicManagedRef>();
        is_async::<DynamicManagedRefMut>();
        is_async::<DynamicManagedLazy>();
        is_async::<DynamicManagedValue>();

        let mut value = DynamicManaged::new(42).unwrap();
        let mut value_ref = value.borrow_mut().unwrap();
        assert!(value_ref.write::<i32>().is_some());
        let mut value_ref2 = value_ref.borrow_mut().unwrap();
        assert!(value_ref.write::<i32>().is_some());
        assert!(value_ref2.write::<i32>().is_some());
        drop(value_ref);
        let value_ref = value.borrow().unwrap();
        assert!(value.borrow().is_some());
        assert!(value.borrow_mut().is_none());
        drop(value_ref);
        assert!(value.borrow().is_some());
        assert!(value.borrow_mut().is_some());
        *value.write::<i32>().unwrap() = 40;
        assert_eq!(*value.read::<i32>().unwrap(), 40);
        *value.borrow_mut().unwrap().write::<i32>().unwrap() = 2;
        assert_eq!(*value.read::<i32>().unwrap(), 2);
        let value_ref = value.borrow().unwrap();
        let value_ref2 = value_ref.borrow().unwrap();
        drop(value_ref);
        assert!(value_ref2.read::<i32>().is_some());
        let value_ref = value.borrow().unwrap();
        let value_lazy = value.lazy();
        assert_eq!(*value_lazy.read::<i32>().unwrap(), 2);
        *value_lazy.write::<i32>().unwrap() = 42;
        assert_eq!(*value_lazy.read::<i32>().unwrap(), 42);
        drop(value);
        assert!(value_ref.read::<i32>().is_none());
        assert!(value_ref2.read::<i32>().is_none());
        assert!(value_lazy.read::<i32>().is_none());
        let value = DynamicManaged::new("hello".to_owned()).unwrap();
        let value = value.consume::<String>().ok().unwrap();
        assert_eq!(value.as_str(), "hello");
    }

    #[test]
    fn test_conversion() {
        let value = Managed::new(42);
        assert_eq!(*value.read().unwrap(), 42);
        let value = value.into_dynamic().ok().unwrap();
        assert_eq!(*value.read::<i32>().unwrap(), 42);
        let mut value = value.into_typed::<i32>().ok().unwrap();
        assert_eq!(*value.read().unwrap(), 42);

        let value_ref = value.borrow().unwrap();
        assert_eq!(*value.read().unwrap(), 42);
        let value_ref = value_ref.into_dynamic();
        assert_eq!(*value_ref.read::<i32>().unwrap(), 42);
        let value_ref = value_ref.into_typed::<i32>().ok().unwrap();
        assert_eq!(*value_ref.read().unwrap(), 42);
        drop(value_ref);

        let value_ref_mut = value.borrow_mut().unwrap();
        assert_eq!(*value.read().unwrap(), 42);
        let value_ref_mut = value_ref_mut.into_dynamic();
        assert_eq!(*value_ref_mut.read::<i32>().unwrap(), 42);
        let value_ref_mut = value_ref_mut.into_typed::<i32>().ok().unwrap();
        assert_eq!(*value_ref_mut.read().unwrap(), 42);

        let value_lazy = value.lazy();
        assert_eq!(*value.read().unwrap(), 42);
        let value_lazy = value_lazy.into_dynamic();
        assert_eq!(*value_lazy.read::<i32>().unwrap(), 42);
        let value_lazy = value_lazy.into_typed::<i32>().ok().unwrap();
        assert_eq!(*value_lazy.read().unwrap(), 42);
    }

    #[test]
    fn test_unsized() {
        let lifetime = Lifetime::default();
        let mut data = 42usize;
        {
            let foo = ManagedRef::<dyn Any>::new(&data, lifetime.borrow().unwrap());
            assert_eq!(
                *foo.read().unwrap().downcast_ref::<usize>().unwrap(),
                42usize
            );
        }
        {
            let mut foo = ManagedRefMut::<dyn Any>::new(&mut data, lifetime.borrow_mut().unwrap());
            *foo.write().unwrap().downcast_mut::<usize>().unwrap() = 100;
        }
        {
            let foo = ManagedLazy::<dyn Any>::new(&mut data, lifetime.lazy());
            assert_eq!(
                *foo.read().unwrap().downcast_ref::<usize>().unwrap(),
                100usize
            );
        }

        let lifetime = Lifetime::default();
        let mut data = [0, 1, 2, 3];
        {
            let foo = ManagedRef::<[i32]>::new(&data, lifetime.borrow().unwrap());
            assert_eq!(*foo.read().unwrap(), [0, 1, 2, 3]);
        }
        {
            let mut foo = ManagedRefMut::<[i32]>::new(&mut data, lifetime.borrow_mut().unwrap());
            foo.write().unwrap().sort_by(|a, b| a.cmp(b).reverse());
        }
        {
            let foo = ManagedLazy::<[i32]>::new(&mut data, lifetime.lazy());
            assert_eq!(*foo.read().unwrap(), [3, 2, 1, 0]);
        }
    }

    #[test]
    fn test_moves() {
        let mut value = Managed::new(42);
        assert_eq!(*value.read().unwrap(), 42);
        {
            let value_ref = value.borrow_mut().unwrap();
            Managed::new(1).move_into_ref(value_ref).ok().unwrap();
            assert_eq!(*value.read().unwrap(), 1);
        }
        {
            let value_lazy = value.lazy();
            Managed::new(2).move_into_lazy(value_lazy).ok().unwrap();
            assert_eq!(*value.read().unwrap(), 2);
        }

        let mut value = DynamicManaged::new(42).unwrap();
        assert_eq!(*value.read::<i32>().unwrap(), 42);
        {
            let value_ref = value.borrow_mut().unwrap();
            DynamicManaged::new(1)
                .unwrap()
                .move_into_ref(value_ref)
                .ok()
                .unwrap();
            assert_eq!(*value.read::<i32>().unwrap(), 1);
        }
        {
            let value_lazy = value.lazy();
            DynamicManaged::new(2)
                .unwrap()
                .move_into_lazy(value_lazy)
                .ok()
                .unwrap();
            assert_eq!(*value.read::<i32>().unwrap(), 2);
        }
    }

    #[test]
    fn test_move_invalidation() {
        let value = Managed::new(42);
        let value_ref = value.borrow().unwrap();
        assert_eq!(value.lifetime().tag(), value_ref.lifetime().tag());
        assert!(value_ref.lifetime().exists());
        let value = Box::new(value);
        assert_ne!(value.lifetime().tag(), value_ref.lifetime().tag());
        assert!(!value_ref.lifetime().exists());
        let value = *value;
        assert_ne!(value.lifetime().tag(), value_ref.lifetime().tag());
        assert!(!value_ref.lifetime().exists());
    }
}
