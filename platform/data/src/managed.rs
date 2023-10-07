use crate::{
    lifetime::{
        Lifetime, LifetimeLazy, LifetimeRef, LifetimeRefMut, ValueReadAccess, ValueWriteAccess,
    },
    type_hash::TypeHash,
    Finalize,
};
use std::{alloc::Layout, mem::MaybeUninit, ptr::NonNull};

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

    pub fn into_dynamic(self) -> DynamicManaged
    where
        T: 'static,
    {
        DynamicManaged::new(self.data)
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

    pub fn lazy(&mut self) -> ManagedLazy<T> {
        ManagedLazy::new(&mut self.data, self.lifetime.lazy())
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Managed<U> {
        Managed {
            lifetime: Default::default(),
            data: f(self.data),
        }
    }

    pub fn try_map<U>(self, f: impl FnOnce(T) -> Option<U>) -> Option<Managed<U>> {
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

impl<T> TryFrom<ManagedValue<T>> for Managed<T> {
    type Error = ();

    fn try_from(value: ManagedValue<T>) -> Result<Self, Self::Error> {
        match value {
            ManagedValue::Owned(value) => Ok(value),
            _ => Err(()),
        }
    }
}

pub struct ManagedRef<T> {
    lifetime: LifetimeRef,
    data: NonNull<T>,
}

unsafe impl<T> Send for ManagedRef<T> where T: Send {}
unsafe impl<T> Sync for ManagedRef<T> where T: Sync {}

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
            data: NonNull::new_unchecked(data as _),
        }
    }

    pub fn into_inner(self) -> (LifetimeRef, NonNull<T>) {
        (self.lifetime, self.data)
    }

    pub fn into_dynamic(self) -> DynamicManagedRef
    where
        T: 'static,
    {
        DynamicManagedRef::new(unsafe { self.data.as_ref() }, self.lifetime)
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

    pub fn map<U>(self, f: impl FnOnce(&T) -> &U) -> ManagedRef<U> {
        unsafe {
            let data = f(self.data.as_ref());
            ManagedRef {
                lifetime: self.lifetime,
                data: NonNull::new_unchecked(data as *const U as *mut U),
            }
        }
    }

    pub fn try_map<U>(self, f: impl FnOnce(&T) -> Option<&U>) -> Result<ManagedRef<U>, Self> {
        unsafe {
            if let Some(data) = f(self.data.as_ref()) {
                Ok(ManagedRef {
                    lifetime: self.lifetime,
                    data: NonNull::new_unchecked(data as *const U as *mut U),
                })
            } else {
                Err(self)
            }
        }
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

impl<T> TryFrom<ManagedValue<T>> for ManagedRef<T> {
    type Error = ();

    fn try_from(value: ManagedValue<T>) -> Result<Self, Self::Error> {
        match value {
            ManagedValue::Ref(value) => Ok(value),
            _ => Err(()),
        }
    }
}

pub struct ManagedRefMut<T> {
    lifetime: LifetimeRefMut,
    data: NonNull<T>,
}

unsafe impl<T> Send for ManagedRefMut<T> where T: Send {}
unsafe impl<T> Sync for ManagedRefMut<T> where T: Sync {}

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

    pub fn into_inner(self) -> (LifetimeRefMut, NonNull<T>) {
        (self.lifetime, self.data)
    }

    pub fn into_dynamic(mut self) -> DynamicManagedRefMut
    where
        T: 'static,
    {
        DynamicManagedRefMut::new(unsafe { self.data.as_mut() }, self.lifetime)
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

    pub fn map<U>(mut self, f: impl FnOnce(&mut T) -> &mut U) -> ManagedRefMut<U> {
        unsafe {
            let data = f(self.data.as_mut());
            ManagedRefMut {
                lifetime: self.lifetime,
                data: NonNull::new_unchecked(data as *mut U),
            }
        }
    }

    pub fn try_map<U>(
        mut self,
        f: impl FnOnce(&mut T) -> Option<&mut U>,
    ) -> Result<ManagedRefMut<U>, Self> {
        unsafe {
            if let Some(data) = f(self.data.as_mut()) {
                Ok(ManagedRefMut {
                    lifetime: self.lifetime,
                    data: NonNull::new_unchecked(data as *mut U),
                })
            } else {
                Err(self)
            }
        }
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

impl<T> TryFrom<ManagedValue<T>> for ManagedRefMut<T> {
    type Error = ();

    fn try_from(value: ManagedValue<T>) -> Result<Self, Self::Error> {
        match value {
            ManagedValue::RefMut(value) => Ok(value),
            _ => Err(()),
        }
    }
}

#[derive(Clone)]
pub struct ManagedLazy<T> {
    lifetime: LifetimeLazy,
    data: NonNull<T>,
}

unsafe impl<T> Send for ManagedLazy<T> where T: Send {}
unsafe impl<T> Sync for ManagedLazy<T> where T: Sync {}

impl<T> ManagedLazy<T> {
    pub fn new(data: &mut T, lifetime: LifetimeLazy) -> Self {
        Self {
            lifetime,
            data: unsafe { NonNull::new_unchecked(data as *mut T) },
        }
    }

    /// # Safety
    pub unsafe fn new_raw(data: *mut T, lifetime: LifetimeLazy) -> Self {
        Self {
            lifetime,
            data: NonNull::new_unchecked(data),
        }
    }

    pub fn into_inner(self) -> (LifetimeLazy, NonNull<T>) {
        (self.lifetime, self.data)
    }

    pub fn into_dynamic(mut self) -> DynamicManagedLazy
    where
        T: 'static,
    {
        DynamicManagedLazy::new(unsafe { self.data.as_mut() }, self.lifetime)
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

    pub fn map<U>(mut self, f: impl FnOnce(&mut T) -> &mut U) -> ManagedLazy<U> {
        unsafe {
            let data = f(self.data.as_mut());
            ManagedLazy {
                lifetime: self.lifetime,
                data: NonNull::new_unchecked(data as *mut U),
            }
        }
    }

    pub fn try_map<U>(
        mut self,
        f: impl FnOnce(&mut T) -> Option<&mut U>,
    ) -> Result<ManagedLazy<U>, Self> {
        unsafe {
            if let Some(data) = f(self.data.as_mut()) {
                Ok(ManagedLazy {
                    lifetime: self.lifetime,
                    data: NonNull::new_unchecked(data as *mut U),
                })
            } else {
                Err(self)
            }
        }
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

impl<T> TryFrom<ManagedValue<T>> for ManagedLazy<T> {
    type Error = ();

    fn try_from(value: ManagedValue<T>) -> Result<Self, Self::Error> {
        match value {
            ManagedValue::Lazy(value) => Ok(value),
            _ => Err(()),
        }
    }
}

pub enum ManagedValue<T> {
    Owned(Managed<T>),
    Ref(ManagedRef<T>),
    RefMut(ManagedRefMut<T>),
    Lazy(ManagedLazy<T>),
}

impl<T> ManagedValue<T> {
    pub fn as_owned(&self) -> Option<&Managed<T>> {
        match self {
            Self::Owned(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_mut_owned(&mut self) -> Option<&mut Managed<T>> {
        match self {
            Self::Owned(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_ref(&self) -> Option<&ManagedRef<T>> {
        match self {
            Self::Ref(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_mut_ref(&mut self) -> Option<&mut ManagedRef<T>> {
        match self {
            Self::Ref(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_ref_mut(&self) -> Option<&ManagedRefMut<T>> {
        match self {
            Self::RefMut(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_mut_ref_mut(&mut self) -> Option<&mut ManagedRefMut<T>> {
        match self {
            Self::RefMut(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_lazy(&self) -> Option<&ManagedLazy<T>> {
        match self {
            Self::Lazy(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_mut_lazy(&mut self) -> Option<&mut ManagedLazy<T>> {
        match self {
            Self::Lazy(value) => Some(value),
            _ => None,
        }
    }

    pub fn read(&self) -> Option<ValueReadAccess<T>> {
        match self {
            Self::Owned(value) => value.read(),
            Self::Ref(value) => value.read(),
            Self::RefMut(value) => value.read(),
            Self::Lazy(value) => value.read(),
        }
    }

    pub fn write(&mut self) -> Option<ValueWriteAccess<T>> {
        match self {
            Self::Owned(value) => value.write(),
            Self::RefMut(value) => value.write(),
            Self::Lazy(value) => value.write(),
            _ => None,
        }
    }

    pub fn borrow(&self) -> Option<ManagedRef<T>> {
        match self {
            Self::Owned(value) => value.borrow(),
            Self::Ref(value) => value.borrow(),
            Self::RefMut(value) => value.borrow(),
            _ => None,
        }
    }

    pub fn borrow_mut(&mut self) -> Option<ManagedRefMut<T>> {
        match self {
            Self::Owned(value) => value.borrow_mut(),
            Self::RefMut(value) => value.borrow_mut(),
            _ => None,
        }
    }
}

impl<T> From<Managed<T>> for ManagedValue<T> {
    fn from(value: Managed<T>) -> Self {
        Self::Owned(value)
    }
}

impl<T> From<ManagedRef<T>> for ManagedValue<T> {
    fn from(value: ManagedRef<T>) -> Self {
        Self::Ref(value)
    }
}

impl<T> From<ManagedRefMut<T>> for ManagedValue<T> {
    fn from(value: ManagedRefMut<T>) -> Self {
        Self::RefMut(value)
    }
}

impl<T> From<ManagedLazy<T>> for ManagedValue<T> {
    fn from(value: ManagedLazy<T>) -> Self {
        Self::Lazy(value)
    }
}

pub struct DynamicManaged {
    type_hash: TypeHash,
    lifetime: Lifetime,
    memory: Vec<u8>,
    finalizer: unsafe fn(*mut ()),
    drop: bool,
}

impl Drop for DynamicManaged {
    fn drop(&mut self) {
        if self.drop {
            unsafe {
                let data_pointer = self.memory.as_mut_ptr().cast::<()>();
                (self.finalizer)(data_pointer);
            }
        }
    }
}

impl DynamicManaged {
    pub fn new<T: Finalize + 'static>(data: T) -> Self {
        let mut memory = vec![0; Layout::new::<T>().pad_to_align().size()];
        unsafe { memory.as_mut_ptr().cast::<T>().write(data) };
        Self {
            type_hash: TypeHash::of::<T>(),
            lifetime: Default::default(),
            memory,
            finalizer: T::finalize_raw,
            drop: true,
        }
    }

    pub fn new_raw(
        type_hash: TypeHash,
        lifetime: Lifetime,
        memory: Vec<u8>,
        finalizer: unsafe fn(*mut ()),
    ) -> Self {
        Self {
            type_hash,
            lifetime,
            memory,
            finalizer,
            drop: true,
        }
    }

    pub fn into_inner(mut self) -> (TypeHash, Lifetime, Vec<u8>, unsafe fn(*mut ())) {
        self.drop = false;
        (
            self.type_hash,
            std::mem::take(&mut self.lifetime),
            std::mem::take(&mut self.memory),
            self.finalizer,
        )
    }

    pub fn into_typed<T: 'static>(self) -> Result<Managed<T>, Self> {
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

    /// # Safety
    pub unsafe fn memory(&self) -> &[u8] {
        &self.memory
    }

    /// # Safety
    pub unsafe fn memory_mut(&mut self) -> &mut [u8] {
        &mut self.memory
    }

    pub fn is<T: 'static>(&self) -> bool {
        self.type_hash == TypeHash::of::<T>()
    }

    pub fn read<T: 'static>(&self) -> Option<ValueReadAccess<T>> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { self.lifetime.read(&*(self.memory.as_ptr() as *const T)) }
        } else {
            None
        }
    }

    pub fn write<T: 'static>(&mut self) -> Option<ValueWriteAccess<T>> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe {
                self.lifetime
                    .write(&mut *(self.memory.as_mut_ptr() as *mut T))
            }
        } else {
            None
        }
    }

    pub fn consume<T: 'static>(self) -> Result<T, Self> {
        if self.type_hash == TypeHash::of::<T>() && !self.lifetime.state().is_in_use() {
            let mut result = MaybeUninit::<T>::uninit();
            unsafe {
                result
                    .as_mut_ptr()
                    .copy_from(self.memory.as_ptr() as *const T, 1);
                Ok(result.assume_init())
            }
        } else {
            Err(self)
        }
    }

    pub fn borrow(&self) -> Option<DynamicManagedRef> {
        unsafe {
            Some(DynamicManagedRef::new_raw(
                self.type_hash,
                self.lifetime.borrow()?,
                self.memory.as_ptr(),
            ))
        }
    }

    pub fn borrow_mut(&mut self) -> Option<DynamicManagedRefMut> {
        unsafe {
            Some(DynamicManagedRefMut::new_raw(
                self.type_hash,
                self.lifetime.borrow_mut()?,
                self.memory.as_mut_ptr(),
            ))
        }
    }

    pub fn lazy(&mut self) -> DynamicManagedLazy {
        unsafe {
            DynamicManagedLazy::new_raw(
                self.type_hash,
                self.lifetime.lazy(),
                self.memory.as_mut_ptr(),
            )
        }
    }

    pub fn map<T: 'static, U: Finalize + 'static>(
        self,
        f: impl FnOnce(T) -> U,
    ) -> Result<Self, Self> {
        self.consume::<T>().map(|data| Self::new(f(data)))
    }

    pub fn try_map<T: 'static, U: Finalize + 'static>(
        self,
        f: impl FnOnce(T) -> Option<U>,
    ) -> Option<Self> {
        f(self.consume::<T>().ok()?).map(|data| Self::new(data))
    }

    /// # Safety
    pub unsafe fn as_ptr<T: 'static>(&self) -> Option<*const T> {
        if self.type_hash == TypeHash::of::<T>() && !self.lifetime.state().is_in_use() {
            Some(self.memory.as_ptr().cast::<T>())
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn as_mut_ptr<T: 'static>(&mut self) -> Option<*mut T> {
        if self.type_hash == TypeHash::of::<T>() && !self.lifetime.state().is_in_use() {
            Some(self.memory.as_mut_ptr().cast::<T>())
        } else {
            None
        }
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
    data: NonNull<u8>,
}

unsafe impl Send for DynamicManagedRef {}
unsafe impl Sync for DynamicManagedRef {}

impl DynamicManagedRef {
    pub fn new<T: 'static>(data: &T, lifetime: LifetimeRef) -> Self {
        Self {
            type_hash: TypeHash::of::<T>(),
            lifetime,
            data: unsafe { NonNull::new_unchecked(data as *const T as *const u8 as *mut u8) },
        }
    }

    /// # Safety
    pub unsafe fn new_raw(type_hash: TypeHash, lifetime: LifetimeRef, data: *const u8) -> Self {
        Self {
            type_hash,
            lifetime,
            data: NonNull::new_unchecked(data as _),
        }
    }

    pub fn into_inner(self) -> (TypeHash, LifetimeRef, NonNull<u8>) {
        (self.type_hash, self.lifetime, self.data)
    }

    pub fn into_typed<T: 'static>(self) -> Result<ManagedRef<T>, Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe {
                Ok(ManagedRef::new_raw(
                    self.data.as_ptr().cast::<T>(),
                    self.lifetime,
                ))
            }
        } else {
            Err(self)
        }
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

    pub fn is<T: 'static>(&self) -> bool {
        self.type_hash == TypeHash::of::<T>()
    }

    pub fn read<T: 'static>(&self) -> Option<ValueReadAccess<T>> {
        if self.type_hash == TypeHash::of::<T>() {
            self.lifetime
                .read(unsafe { self.data.as_ptr().cast::<T>().as_ref()? })
        } else {
            None
        }
    }

    pub fn map<T: 'static, U: 'static>(self, f: impl FnOnce(&T) -> &U) -> Result<Self, Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe {
                let data = f(&*(self.data.as_ptr() as *const T));
                Ok(Self {
                    type_hash: TypeHash::of::<U>(),
                    lifetime: self.lifetime,
                    data: NonNull::new_unchecked(data as *const U as *const u8 as *mut u8),
                })
            }
        } else {
            Err(self)
        }
    }

    pub fn try_map<T: 'static, U: 'static>(
        self,
        f: impl FnOnce(&T) -> Option<&U>,
    ) -> Result<Self, Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe {
                if let Some(data) = f(&*(self.data.as_ptr() as *const T)) {
                    Ok(Self {
                        type_hash: TypeHash::of::<U>(),
                        lifetime: self.lifetime,
                        data: NonNull::new_unchecked(data as *const U as *const u8 as *mut u8),
                    })
                } else {
                    Err(self)
                }
            }
        } else {
            Err(self)
        }
    }

    /// # Safety
    pub unsafe fn as_ptr<T: 'static>(&self) -> Option<*const T> {
        if self.type_hash == TypeHash::of::<T>() && self.lifetime.exists() {
            Some(self.data.as_ptr().cast::<T>())
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
    data: NonNull<u8>,
}

unsafe impl Send for DynamicManagedRefMut {}
unsafe impl Sync for DynamicManagedRefMut {}

impl DynamicManagedRefMut {
    pub fn new<T: 'static>(data: &mut T, lifetime: LifetimeRefMut) -> Self {
        Self {
            type_hash: TypeHash::of::<T>(),
            lifetime,
            data: unsafe { NonNull::new_unchecked(data as *mut T as *mut u8) },
        }
    }

    /// # Safety
    pub unsafe fn new_raw(type_hash: TypeHash, lifetime: LifetimeRefMut, data: *mut u8) -> Self {
        Self {
            type_hash,
            lifetime,
            data: NonNull::new_unchecked(data),
        }
    }

    pub fn into_inner(self) -> (TypeHash, LifetimeRefMut, NonNull<u8>) {
        (self.type_hash, self.lifetime, self.data)
    }

    pub fn into_typed<T: 'static>(self) -> Result<ManagedRefMut<T>, Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe {
                Ok(ManagedRefMut::new_raw(
                    self.data.as_ptr().cast::<T>(),
                    self.lifetime,
                ))
            }
        } else {
            Err(self)
        }
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

    pub fn borrow_mut(&self) -> Option<DynamicManagedRefMut> {
        Some(DynamicManagedRefMut {
            type_hash: self.type_hash,
            lifetime: self.lifetime.borrow_mut()?,
            data: self.data,
        })
    }

    pub fn is<T: 'static>(&self) -> bool {
        self.type_hash == TypeHash::of::<T>()
    }

    pub fn read<T: 'static>(&self) -> Option<ValueReadAccess<T>> {
        if self.type_hash == TypeHash::of::<T>() {
            self.lifetime
                .read(unsafe { self.data.as_ptr().cast::<T>().as_ref()? })
        } else {
            None
        }
    }

    pub fn write<T: 'static>(&mut self) -> Option<ValueWriteAccess<T>> {
        if self.type_hash == TypeHash::of::<T>() {
            self.lifetime
                .write(unsafe { self.data.as_ptr().cast::<T>().as_mut()? })
        } else {
            None
        }
    }

    pub fn map<T: 'static, U: 'static>(
        self,
        f: impl FnOnce(&mut T) -> &mut U,
    ) -> Result<Self, Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe {
                let data = f(&mut *(self.data.as_ptr() as *mut T));
                Ok(Self {
                    type_hash: TypeHash::of::<U>(),
                    lifetime: self.lifetime,
                    data: NonNull::new_unchecked(data as *mut U as *mut u8),
                })
            }
        } else {
            Err(self)
        }
    }

    pub fn try_map<T: 'static, U: 'static>(
        self,
        f: impl FnOnce(&mut T) -> Option<&mut U>,
    ) -> Result<Self, Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe {
                if let Some(data) = f(&mut *(self.data.as_ptr() as *mut T)) {
                    Ok(Self {
                        type_hash: TypeHash::of::<U>(),
                        lifetime: self.lifetime,
                        data: NonNull::new_unchecked(data as *mut U as *mut u8),
                    })
                } else {
                    Err(self)
                }
            }
        } else {
            Err(self)
        }
    }

    /// # Safety
    pub unsafe fn as_ptr<T: 'static>(&self) -> Option<*const T> {
        if self.type_hash == TypeHash::of::<T>() && self.lifetime.exists() {
            Some(self.data.as_ptr().cast::<T>())
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn as_mut_ptr<T: 'static>(&mut self) -> Option<*mut T> {
        if self.type_hash == TypeHash::of::<T>() && self.lifetime.exists() {
            Some(self.data.as_ptr().cast::<T>())
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
    data: NonNull<u8>,
}

unsafe impl Send for DynamicManagedLazy {}
unsafe impl Sync for DynamicManagedLazy {}

impl DynamicManagedLazy {
    pub fn new<T: 'static>(data: &mut T, lifetime: LifetimeLazy) -> Self {
        Self {
            type_hash: TypeHash::of::<T>(),
            lifetime,
            data: unsafe { NonNull::new_unchecked(data as *mut T as *mut u8) },
        }
    }

    /// # Safety
    pub unsafe fn new_raw(type_hash: TypeHash, lifetime: LifetimeLazy, data: *mut u8) -> Self {
        Self {
            type_hash,
            lifetime,
            data: NonNull::new_unchecked(data),
        }
    }

    pub fn into_inner(self) -> (TypeHash, LifetimeLazy, NonNull<u8>) {
        (self.type_hash, self.lifetime, self.data)
    }

    pub fn into_typed<T: 'static>(self) -> Result<ManagedLazy<T>, Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe {
                Ok(ManagedLazy::new_raw(
                    self.data.as_ptr().cast::<T>(),
                    self.lifetime,
                ))
            }
        } else {
            Err(self)
        }
    }

    pub fn lifetime(&self) -> &LifetimeLazy {
        &self.lifetime
    }

    pub fn is<T: 'static>(&self) -> bool {
        self.type_hash == TypeHash::of::<T>()
    }

    pub fn read<T: 'static>(&self) -> Option<ValueReadAccess<T>> {
        if self.type_hash == TypeHash::of::<T>() {
            self.lifetime
                .read(unsafe { self.data.as_ptr().cast::<T>().as_ref()? })
        } else {
            None
        }
    }

    pub fn write<T: 'static>(&mut self) -> Option<ValueWriteAccess<T>> {
        if self.type_hash == TypeHash::of::<T>() {
            self.lifetime
                .write(unsafe { self.data.as_ptr().cast::<T>().as_mut()? })
        } else {
            None
        }
    }

    pub fn map<T: 'static, U: 'static>(
        self,
        f: impl FnOnce(&mut T) -> &mut U,
    ) -> Result<Self, Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe {
                let data = f(&mut *(self.data.as_ptr() as *mut T));
                Ok(Self {
                    type_hash: TypeHash::of::<U>(),
                    lifetime: self.lifetime,
                    data: NonNull::new_unchecked(data as *mut U as *mut u8),
                })
            }
        } else {
            Err(self)
        }
    }

    pub fn try_map<T: 'static, U: 'static>(
        self,
        f: impl FnOnce(&mut T) -> Option<&mut U>,
    ) -> Result<Self, Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe {
                if let Some(data) = f(&mut *(self.data.as_ptr() as *mut T)) {
                    Ok(Self {
                        type_hash: TypeHash::of::<U>(),
                        lifetime: self.lifetime,
                        data: NonNull::new_unchecked(data as *mut U as *mut u8),
                    })
                } else {
                    Err(self)
                }
            }
        } else {
            Err(self)
        }
    }

    /// # Safety
    pub unsafe fn as_ptr<T: 'static>(&self) -> Option<*const T> {
        if self.type_hash == TypeHash::of::<T>() && self.lifetime.exists() {
            Some(self.data.as_ptr().cast::<T>())
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn as_mut_ptr<T: 'static>(&mut self) -> Option<*mut T> {
        if self.type_hash == TypeHash::of::<T>() && self.lifetime.exists() {
            Some(self.data.as_ptr().cast::<T>())
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

pub enum DynamicManagedValue {
    Owned(DynamicManaged),
    Ref(DynamicManagedRef),
    RefMut(DynamicManagedRefMut),
    Lazy(DynamicManagedLazy),
}

impl DynamicManagedValue {
    pub fn as_owned(&self) -> Option<&DynamicManaged> {
        match self {
            Self::Owned(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_mut_owned(&mut self) -> Option<&mut DynamicManaged> {
        match self {
            Self::Owned(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_ref(&self) -> Option<&DynamicManagedRef> {
        match self {
            Self::Ref(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_mut_ref(&mut self) -> Option<&mut DynamicManagedRef> {
        match self {
            Self::Ref(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_ref_mut(&self) -> Option<&DynamicManagedRefMut> {
        match self {
            Self::RefMut(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_mut_ref_mut(&mut self) -> Option<&mut DynamicManagedRefMut> {
        match self {
            Self::RefMut(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_lazy(&self) -> Option<&DynamicManagedLazy> {
        match self {
            Self::Lazy(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_mut_lazy(&mut self) -> Option<&mut DynamicManagedLazy> {
        match self {
            Self::Lazy(value) => Some(value),
            _ => None,
        }
    }

    pub fn read<T: 'static>(&self) -> Option<ValueReadAccess<T>> {
        match self {
            Self::Owned(value) => value.read::<T>(),
            Self::Ref(value) => value.read::<T>(),
            Self::RefMut(value) => value.read::<T>(),
            Self::Lazy(value) => value.read::<T>(),
        }
    }

    pub fn write<T: 'static>(&mut self) -> Option<ValueWriteAccess<T>> {
        match self {
            Self::Owned(value) => value.write::<T>(),
            Self::RefMut(value) => value.write::<T>(),
            Self::Lazy(value) => value.write::<T>(),
            _ => None,
        }
    }

    pub fn borrow(&self) -> Option<DynamicManagedRef> {
        match self {
            Self::Owned(value) => value.borrow(),
            Self::Ref(value) => value.borrow(),
            Self::RefMut(value) => value.borrow(),
            _ => None,
        }
    }

    pub fn borrow_mut(&mut self) -> Option<DynamicManagedRefMut> {
        match self {
            Self::Owned(value) => value.borrow_mut(),
            Self::RefMut(value) => value.borrow_mut(),
            _ => None,
        }
    }
}

impl From<DynamicManaged> for DynamicManagedValue {
    fn from(value: DynamicManaged) -> Self {
        Self::Owned(value)
    }
}

impl From<DynamicManagedRef> for DynamicManagedValue {
    fn from(value: DynamicManagedRef) -> Self {
        Self::Ref(value)
    }
}

impl From<DynamicManagedRefMut> for DynamicManagedValue {
    fn from(value: DynamicManagedRefMut) -> Self {
        Self::RefMut(value)
    }
}

impl From<DynamicManagedLazy> for DynamicManagedValue {
    fn from(value: DynamicManagedLazy) -> Self {
        Self::Lazy(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_async<T: Send + Sync + 'static>() {}

    #[test]
    fn test_managed() {
        is_async::<Managed<()>>();
        is_async::<ManagedRef<()>>();
        is_async::<ManagedRefMut<()>>();
        is_async::<ManagedLazy<()>>();

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
        let mut value_lazy = value.lazy();
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

        let mut value = DynamicManaged::new(42);
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
        let mut value_lazy = value.lazy();
        assert_eq!(*value_lazy.read::<i32>().unwrap(), 2);
        *value_lazy.write::<i32>().unwrap() = 42;
        assert_eq!(*value_lazy.read::<i32>().unwrap(), 42);
        drop(value);
        assert!(value_ref.read::<i32>().is_none());
        assert!(value_ref2.read::<i32>().is_none());
        assert!(value_lazy.read::<i32>().is_none());
    }

    #[test]
    fn test_conversion() {
        let value = Managed::new(42);
        assert_eq!(*value.read().unwrap(), 42);
        let value = value.into_dynamic();
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
}
