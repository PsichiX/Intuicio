//! Value boxes that tolerate reference cycles.
//!
//! Reference counting leaks when two values point at each other. These boxes
//! avoid that by not counting at all: exactly one handle **owns** the
//! allocation and every other handle only **references** it. A cycle is then
//! just a reference pointing back, and nothing keeps the value alive on its
//! own.
//!
//! When the owner is dropped the value is destroyed, and every reference to
//! it reports that through [`DynamicManagedGc::exists`] rather than dangling.
//! Ownership can also be handed to a reference with
//! [`DynamicManagedGc::transfer_ownership`], which is how a value outlives the
//! handle it started in.
//!
//! [`ManagedGc`] is the typed box, [`DynamicManagedGc`] the type-erased one it
//! is built on.
//!
//! # Blocking and non-blocking access
//!
//! The `try_` methods return [`None`] when the value is busy. The plain ones
//! take a `LOCKING` constant: `true` spins until the value is free, `false`
//! panics right away. Pick `false` on a single thread, where a busy value
//! means a bug rather than a race.
//!
//! ```
//! # use intuicio_data::managed::gc::ManagedGc;
//! let mut owner = ManagedGc::new(42);
//! let handle = owner.reference();
//! assert!(handle.exists());
//! drop(owner);
//! // the owner is gone, so the reference knows the value is gone too
//! assert!(!handle.exists());
//! ```
use crate::{
    Finalize, Finalizer,
    lifetime::{Lifetime, LifetimeLazy, ValueReadAccess, ValueWriteAccess},
    managed::{
        DynamicManagedLazy, DynamicManagedRef, DynamicManagedRefMut, ManagedLazy, ManagedRef,
        ManagedRefMut,
        value::{DynamicManagedValue, ManagedValue},
    },
    non_zero_alloc, non_zero_dealloc,
    type_hash::TypeHash,
};
use std::{
    alloc::{Layout, handle_alloc_error},
    marker::PhantomData,
    mem::MaybeUninit,
};

/// Whether this handle owns the allocation or only points at it.
enum Kind {
    Owned {
        lifetime: Box<Lifetime>,
        data: *mut u8,
    },
    Referenced {
        lifetime: LifetimeLazy,
        data: *mut u8,
    },
}

/// Borrow state of a garbage collected box, as seen from a handle.
///
/// Which variant comes back tells whether the handle owns the value.
pub enum ManagedGcLifetime<'a> {
    /// This handle owns the value.
    Owned(&'a Lifetime),
    /// This handle only references the value.
    Referenced(&'a LifetimeLazy),
}

/// Typed garbage collected box.
///
/// See the [module docs](self) for the ownership model.
pub struct ManagedGc<T> {
    dynamic: DynamicManagedGc,
    _phantom: PhantomData<fn() -> T>,
}

unsafe impl<T> Send for ManagedGc<T> {}
unsafe impl<T> Sync for ManagedGc<T> {}

impl<T: Default> Default for ManagedGc<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> ManagedGc<T> {
    /// Allocates a value and owns it.
    pub fn new(data: T) -> Self {
        Self {
            dynamic: DynamicManagedGc::new(data),
            _phantom: PhantomData,
        }
    }

    /// Allocates a value that can point back at itself.
    ///
    /// `f` is handed a reference to the box before the value exists, so it can
    /// store it inside the value it returns.
    ///
    /// # Safety
    ///
    /// The handle passed to `f` points at memory that is not written yet.
    /// Storing it is fine, but reading or writing through it before `f`
    /// returns is undefined.
    pub unsafe fn new_cyclic(f: impl FnOnce(Self) -> T) -> Self {
        Self {
            dynamic: unsafe { DynamicManagedGc::new_cyclic(|dynamic| f(dynamic.into_typed())) },
            _phantom: PhantomData,
        }
    }

    /// Takes another handle that references the same value without owning it.
    pub fn reference(&self) -> Self {
        Self {
            dynamic: self.dynamic.reference(),
            _phantom: PhantomData,
        }
    }

    /// Takes the value out and frees the allocation.
    ///
    /// Gives the box back when it does not own the value or something is
    /// accessing it.
    pub fn consume(self) -> Result<T, Self> {
        self.dynamic.consume().map_err(|value| Self {
            dynamic: value,
            _phantom: PhantomData,
        })
    }

    /// Erases the type.
    pub fn into_dynamic(self) -> DynamicManagedGc {
        self.dynamic
    }

    /// Replaces the lifetime, killing every reference taken so far. Does
    /// nothing on a referencing handle.
    pub fn renew(&mut self) {
        self.dynamic.renew();
    }

    /// Returns the type of the value.
    pub fn type_hash(&self) -> TypeHash {
        self.dynamic.type_hash()
    }

    /// Returns the borrow state, and with it whether this handle owns the
    /// value.
    pub fn lifetime(&self) -> ManagedGcLifetime<'_> {
        self.dynamic.lifetime()
    }

    /// Returns `true` while the value is alive.
    ///
    /// Always `true` for the owner.
    pub fn exists(&self) -> bool {
        self.dynamic.exists()
    }

    /// Returns `true` when this handle owns the value.
    pub fn is_owning(&self) -> bool {
        self.dynamic.is_owning()
    }

    /// Returns `true` when this handle only references the value.
    pub fn is_referencing(&self) -> bool {
        self.dynamic.is_referencing()
    }

    /// Returns `true` when this handle references the value that `other` owns.
    pub fn is_owned_by(&self, other: &Self) -> bool {
        self.dynamic.is_owned_by(&other.dynamic)
    }

    /// Hands ownership over to a handle that references this value.
    ///
    /// Returns `false` unless this handle owns the value and `new_owner`
    /// references it.
    pub fn transfer_ownership(&mut self, new_owner: &mut Self) -> bool {
        self.dynamic.transfer_ownership(&mut new_owner.dynamic)
    }

    /// Guards the value for reading, or returns [`None`] when it is busy or
    /// gone.
    pub fn try_read(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        self.dynamic.try_read::<T>()
    }

    /// Guards the value for writing, or returns [`None`] when it is busy or
    /// gone.
    pub fn try_write(&'_ mut self) -> Option<ValueWriteAccess<'_, T>> {
        self.dynamic.try_write::<T>()
    }

    /// Guards the value for reading, spinning until it is free when `LOCKING`.
    ///
    /// # Panics
    ///
    /// Panics when the value is gone, or when it is busy and `LOCKING` is
    /// `false`.
    pub fn read<const LOCKING: bool>(&'_ self) -> ValueReadAccess<'_, T> {
        self.dynamic.read::<LOCKING, T>()
    }

    /// Guards the value for writing, spinning until it is free when `LOCKING`.
    ///
    /// # Panics
    ///
    /// Panics when the value is gone, or when it is busy and `LOCKING` is
    /// `false`.
    pub fn write<const LOCKING: bool>(&'_ mut self) -> ValueWriteAccess<'_, T> {
        self.dynamic.write::<LOCKING, T>()
    }

    /// Takes a shared handle, or returns [`None`] when the value is busy or
    /// gone.
    pub fn try_borrow(&self) -> Option<ManagedRef<T>> {
        self.dynamic.try_borrow()?.into_typed().ok()
    }

    /// Takes an exclusive handle, or returns [`None`] when the value is busy or
    /// gone.
    pub fn try_borrow_mut(&self) -> Option<ManagedRefMut<T>> {
        self.dynamic.try_borrow_mut()?.into_typed().ok()
    }

    /// Takes a shared handle, spinning until it is free when `LOCKING`.
    ///
    /// # Panics
    ///
    /// Panics when the value is gone, or when it is busy and `LOCKING` is
    /// `false`.
    pub fn borrow<const LOCKING: bool>(&self) -> ManagedRef<T> {
        self.dynamic
            .borrow::<LOCKING>()
            .into_typed()
            .ok()
            .expect("ManagedGc cannot be immutably borrowed")
    }

    /// Takes an exclusive handle, spinning until it is free when `LOCKING`.
    ///
    /// # Panics
    ///
    /// Panics when the value is gone, or when it is busy and `LOCKING` is
    /// `false`.
    pub fn borrow_mut<const LOCKING: bool>(&mut self) -> ManagedRefMut<T> {
        self.dynamic
            .borrow_mut::<LOCKING>()
            .into_typed()
            .ok()
            .expect("ManagedGc cannot be mutably borrowed")
    }

    /// Takes an unclaimed handle, which claims nothing and never blocks.
    ///
    /// # Panics
    ///
    /// Panics when the value is gone.
    pub fn lazy(&self) -> ManagedLazy<T> {
        self.dynamic
            .lazy()
            .into_typed()
            .ok()
            .expect("ManagedGc cannot be lazily borrowed")
    }

    /// Returns a pointer to the value, checking nothing.
    ///
    /// # Safety
    ///
    /// Neither the borrow state nor whether the value is still alive is
    /// checked.
    pub unsafe fn as_ptr(&self) -> *const T {
        unsafe { self.dynamic.as_ptr_raw().cast::<T>() }
    }

    /// Returns a mutable pointer to the value, checking nothing.
    ///
    /// # Safety
    ///
    /// Neither the borrow state nor whether the value is still alive is
    /// checked.
    pub unsafe fn as_mut_ptr(&mut self) -> *mut T {
        unsafe { self.dynamic.as_mut_ptr_raw().cast::<T>() }
    }
}

impl<T> TryFrom<ManagedValue<T>> for ManagedGc<T> {
    type Error = ();

    fn try_from(value: ManagedValue<T>) -> Result<Self, Self::Error> {
        match value {
            ManagedValue::Gc(value) => Ok(value),
            _ => Err(()),
        }
    }
}

/// Type-erased garbage collected box.
///
/// The [`ManagedGc`] counterpart for script values. See the
/// [module docs](self) for the ownership model.
pub struct DynamicManagedGc {
    type_hash: TypeHash,
    kind: Kind,
    layout: Layout,
    finalizer: Finalizer,
    drop: bool,
}

unsafe impl Send for DynamicManagedGc {}
unsafe impl Sync for DynamicManagedGc {}

impl Drop for DynamicManagedGc {
    fn drop(&mut self) {
        if let Kind::Owned { lifetime, data } = &mut self.kind
            && self.drop
        {
            while lifetime.state().is_in_use() {
                std::hint::spin_loop();
            }
            lifetime.invalidate();
            unsafe {
                if data.is_null() {
                    return;
                }
                self.finalizer.finalize(data.cast::<()>());
                non_zero_dealloc(*data, self.layout);
            }
        }
    }
}

impl DynamicManagedGc {
    /// Allocates a value and owns it.
    pub fn new<T: Finalize>(data: T) -> Self {
        let layout = Layout::new::<T>().pad_to_align();
        unsafe {
            let memory = non_zero_alloc(layout) as *mut T;
            if memory.is_null() {
                handle_alloc_error(layout);
            }
            memory.cast::<T>().write(data);
            Self {
                type_hash: TypeHash::of::<T>(),
                kind: Kind::Owned {
                    lifetime: Default::default(),
                    data: memory.cast::<u8>(),
                },
                layout,
                finalizer: Finalizer::of::<T>(),
                drop: true,
            }
        }
    }

    /// Allocates a value that can point back at itself.
    ///
    /// `f` is handed a reference to the box before the value exists, so it can
    /// store it inside the value it returns.
    ///
    /// # Safety
    ///
    /// The handle passed to `f` points at memory that is not written yet.
    /// Storing it is fine, but reading or writing through it before `f`
    /// returns is undefined.
    pub unsafe fn new_cyclic<T: Finalize>(f: impl FnOnce(Self) -> T) -> Self {
        let layout = Layout::new::<T>().pad_to_align();
        unsafe {
            let memory = non_zero_alloc(layout) as *mut T;
            if memory.is_null() {
                handle_alloc_error(layout);
            }
            let result = Self {
                type_hash: TypeHash::of::<T>(),
                kind: Kind::Owned {
                    lifetime: Default::default(),
                    data: memory.cast::<u8>(),
                },
                layout,
                finalizer: Finalizer::of::<T>(),
                drop: true,
            };
            let data = f(result.reference());
            memory.cast::<T>().write(data);
            result
        }
    }

    /// Takes ownership of an existing allocation.
    ///
    /// The box will free `memory` and run `finalizer` when it is dropped.
    ///
    /// # Panics
    ///
    /// Panics when `memory` is null.
    pub fn new_raw(
        type_hash: TypeHash,
        lifetime: Lifetime,
        memory: *mut u8,
        layout: Layout,
        finalizer: impl Into<Finalizer>,
    ) -> Self {
        if memory.is_null() {
            handle_alloc_error(layout);
        }
        Self {
            type_hash,
            kind: Kind::Owned {
                lifetime: Box::new(lifetime),
                data: memory,
            },
            layout,
            finalizer: finalizer.into(),
            drop: true,
        }
    }

    /// Allocates room for a value without writing one into it.
    ///
    /// The finalizer still runs on drop, so the caller must fill the memory
    /// before the box is dropped or read.
    pub fn new_uninitialized(
        type_hash: TypeHash,
        layout: Layout,
        finalizer: impl Into<Finalizer>,
    ) -> Self {
        let memory = unsafe { non_zero_alloc(layout) };
        if memory.is_null() {
            handle_alloc_error(layout);
        }
        Self {
            type_hash,
            kind: Kind::Owned {
                lifetime: Default::default(),
                data: memory,
            },
            layout,
            finalizer: finalizer.into(),
            drop: true,
        }
    }

    /// Takes another handle that references the same value without owning it.
    pub fn reference(&self) -> Self {
        match &self.kind {
            Kind::Owned { lifetime, data } => Self {
                type_hash: self.type_hash,
                kind: Kind::Referenced {
                    lifetime: lifetime.lazy(),
                    data: *data,
                },
                layout: self.layout,
                finalizer: self.finalizer.clone(),
                drop: true,
            },
            Kind::Referenced { lifetime, data } => Self {
                type_hash: self.type_hash,
                kind: Kind::Referenced {
                    lifetime: lifetime.clone(),
                    data: *data,
                },
                layout: self.layout,
                finalizer: self.finalizer.clone(),
                drop: true,
            },
        }
    }

    /// Takes the value out and frees the allocation.
    ///
    /// Gives the box back when it does not own the value, the type does not
    /// match, or something is accessing it.
    pub fn consume<T>(mut self) -> Result<T, Self> {
        if let Kind::Owned { lifetime, data } = &mut self.kind {
            if self.type_hash == TypeHash::of::<T>() && !lifetime.state().is_in_use() {
                if data.is_null() {
                    return Err(self);
                }
                self.drop = false;
                let mut result = MaybeUninit::<T>::uninit();
                unsafe {
                    result.as_mut_ptr().copy_from(data.cast::<T>(), 1);
                    non_zero_dealloc(*data, self.layout);
                    Ok(result.assume_init())
                }
            } else {
                Err(self)
            }
        } else {
            Err(self)
        }
    }

    /// Puts a type back on the box.
    pub fn into_typed<T>(self) -> ManagedGc<T> {
        ManagedGc {
            dynamic: self,
            _phantom: PhantomData,
        }
    }

    /// Replaces the lifetime, killing every reference taken so far. Does
    /// nothing on a referencing handle.
    pub fn renew(&mut self) {
        if let Kind::Owned { lifetime, .. } = &mut self.kind {
            **lifetime = Default::default();
        }
    }

    /// Returns the type of the value.
    pub fn type_hash(&self) -> TypeHash {
        self.type_hash
    }

    /// Returns the borrow state, and with it whether this handle owns the
    /// value.
    pub fn lifetime(&self) -> ManagedGcLifetime<'_> {
        match &self.kind {
            Kind::Owned { lifetime, .. } => ManagedGcLifetime::Owned(lifetime),
            Kind::Referenced { lifetime, .. } => ManagedGcLifetime::Referenced(lifetime),
        }
    }

    /// Returns the layout of the allocation.
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Returns the drop function of the stored type.
    pub fn finalizer(&self) -> &Finalizer {
        &self.finalizer
    }

    /// Returns the value as raw bytes.
    ///
    /// # Safety
    ///
    /// Bypasses the borrow state, and does not check that the value is still
    /// alive.
    pub unsafe fn memory(&self) -> &[u8] {
        let memory = match &self.kind {
            Kind::Owned { data, .. } => *data,
            Kind::Referenced { data, .. } => *data,
        };
        unsafe { std::slice::from_raw_parts(memory, self.layout.size()) }
    }

    /// Returns the value as mutable raw bytes.
    ///
    /// # Safety
    ///
    /// Bypasses the borrow state, does not check that the value is still
    /// alive, and writing bytes that are not a valid value of the stored type
    /// makes every later access undefined.
    pub unsafe fn memory_mut(&mut self) -> &mut [u8] {
        let memory = match &mut self.kind {
            Kind::Owned { data, .. } => *data,
            Kind::Referenced { data, .. } => *data,
        };
        unsafe { std::slice::from_raw_parts_mut(memory, self.layout.size()) }
    }

    /// Returns `true` while the value is alive.
    ///
    /// Always `true` for the owner.
    pub fn exists(&self) -> bool {
        match &self.kind {
            Kind::Owned { .. } => true,
            Kind::Referenced { lifetime, .. } => lifetime.exists(),
        }
    }

    /// Returns `true` when this handle owns the value.
    pub fn is_owning(&self) -> bool {
        matches!(self.kind, Kind::Owned { .. })
    }

    /// Returns `true` when this handle only references the value.
    pub fn is_referencing(&self) -> bool {
        matches!(self.kind, Kind::Referenced { .. })
    }

    /// Returns `true` when this handle references the value that `other` owns.
    pub fn is_owned_by(&self, other: &Self) -> bool {
        if let (
            Kind::Referenced {
                lifetime: l1,
                data: d1,
            },
            Kind::Owned {
                lifetime: l2,
                data: d2,
            },
        ) = (&self.kind, &other.kind)
        {
            *d1 == *d2 && l1.state().is_owned_by(l2.state())
        } else {
            false
        }
    }

    /// Hands ownership over to a handle that references this value.
    ///
    /// Returns `false` unless this handle owns the value and `new_owner`
    /// references it.
    pub fn transfer_ownership(&mut self, new_owner: &mut Self) -> bool {
        if let (
            Kind::Owned {
                lifetime: l1,
                data: d1,
            },
            Kind::Referenced {
                lifetime: l2,
                data: d2,
            },
        ) = (&mut self.kind, &new_owner.kind)
            && *d1 == *d2
            && l2.state().is_owned_by(l1.state())
        {
            std::mem::swap(&mut self.kind, &mut new_owner.kind);
            true
        } else {
            false
        }
    }

    /// Returns `true` when the value is a `T`.
    pub fn is<T>(&self) -> bool {
        self.type_hash == TypeHash::of::<T>()
    }

    /// Guards the value for reading, or returns [`None`] when it is busy or
    /// gone.
    ///
    /// # Panics
    ///
    /// Panics when the value is not a `T`.
    pub fn try_read<T>(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        if !self.is::<T>() {
            panic!(
                "DynamicManagedGc is not of the requested type: {}",
                std::any::type_name::<T>()
            );
        }
        unsafe {
            match &self.kind {
                Kind::Owned { lifetime, data } => {
                    let data = data.cast::<T>().as_ref()?;
                    lifetime.read(data)
                }
                Kind::Referenced { lifetime, data } => {
                    if lifetime.exists() {
                        let data = data.cast::<T>().as_ref()?;
                        lifetime.read(data)
                    } else {
                        None
                    }
                }
            }
        }
    }

    /// Guards the value for writing, or returns [`None`] when it is busy or
    /// gone.
    ///
    /// # Panics
    ///
    /// Panics when the value is not a `T`.
    pub fn try_write<T>(&'_ mut self) -> Option<ValueWriteAccess<'_, T>> {
        if !self.is::<T>() {
            panic!(
                "DynamicManagedGc is not of the requested type: {}",
                std::any::type_name::<T>()
            );
        }
        unsafe {
            match &self.kind {
                Kind::Owned { lifetime, data } => {
                    let data = data.cast::<T>().as_mut()?;
                    lifetime.write(data)
                }
                Kind::Referenced { lifetime, data } => {
                    if lifetime.exists() {
                        let data = data.cast::<T>().as_mut()?;
                        lifetime.write(data)
                    } else {
                        None
                    }
                }
            }
        }
    }

    /// Guards the value for reading, spinning until it is free when `LOCKING`.
    ///
    /// # Panics
    ///
    /// Panics when the value is not a `T`, is gone, or is busy and `LOCKING`
    /// is `false`.
    pub fn read<const LOCKING: bool, T>(&'_ self) -> ValueReadAccess<'_, T> {
        if !self.is::<T>() {
            panic!(
                "DynamicManagedGc is not of the requested type: {}",
                std::any::type_name::<T>()
            );
        }
        unsafe {
            if LOCKING {
                match &self.kind {
                    Kind::Owned { lifetime, data } => loop {
                        let data = data
                            .cast::<T>()
                            .as_ref()
                            .expect("DynamicManagedGc data pointer is null");
                        if let Some(access) = lifetime.read(data) {
                            return access;
                        }
                        std::hint::spin_loop();
                    },
                    Kind::Referenced { lifetime, data } => loop {
                        if !lifetime.exists() {
                            panic!("DynamicManagedGc owner is dead");
                        }
                        let data = data
                            .cast::<T>()
                            .as_ref()
                            .expect("DynamicManagedGc data pointer is null");
                        if let Some(access) = lifetime.read(data) {
                            return access;
                        }
                        std::hint::spin_loop();
                    },
                }
            } else {
                match &self.kind {
                    Kind::Owned { lifetime, data } => {
                        let data = data
                            .cast::<T>()
                            .as_ref()
                            .expect("DynamicManagedGc data pointer is null");
                        lifetime
                            .read(data)
                            .expect("DynamicManagedGc is inaccessible for reading")
                    }
                    Kind::Referenced { lifetime, data } => {
                        let data = data
                            .cast::<T>()
                            .as_ref()
                            .expect("DynamicManagedGc data pointer is null");
                        lifetime
                            .read(data)
                            .expect("DynamicManagedGc is inaccessible for reading")
                    }
                }
            }
        }
    }

    /// Guards the value for writing, spinning until it is free when `LOCKING`.
    ///
    /// # Panics
    ///
    /// Panics when the value is not a `T`, is gone, or is busy and `LOCKING`
    /// is `false`.
    pub fn write<const LOCKING: bool, T>(&'_ mut self) -> ValueWriteAccess<'_, T> {
        if !self.is::<T>() {
            panic!(
                "DynamicManagedGc is not of the requested type: {}",
                std::any::type_name::<T>()
            );
        }
        unsafe {
            if LOCKING {
                match &self.kind {
                    Kind::Owned { lifetime, data } => loop {
                        let data = data
                            .cast::<T>()
                            .as_mut()
                            .expect("DynamicManagedGc data pointer is null");
                        if let Some(access) = lifetime.write(data) {
                            return access;
                        }
                        std::hint::spin_loop();
                    },
                    Kind::Referenced { lifetime, data } => loop {
                        if !lifetime.exists() {
                            panic!("DynamicManagedGc owner is dead");
                        }
                        let data = data
                            .cast::<T>()
                            .as_mut()
                            .expect("DynamicManagedGc data pointer is null");
                        if let Some(access) = lifetime.write(data) {
                            return access;
                        }
                        std::hint::spin_loop();
                    },
                }
            } else {
                match &self.kind {
                    Kind::Owned { lifetime, data } => {
                        let data = data
                            .cast::<T>()
                            .as_mut()
                            .expect("DynamicManagedGc data pointer is null");
                        lifetime
                            .write(data)
                            .expect("DynamicManagedGc is inaccessible for writing")
                    }
                    Kind::Referenced { lifetime, data } => {
                        let data = data
                            .cast::<T>()
                            .as_mut()
                            .expect("DynamicManagedGc data pointer is null");
                        lifetime
                            .write(data)
                            .expect("DynamicManagedGc is inaccessible for writing")
                    }
                }
            }
        }
    }

    /// Takes a shared handle, or returns [`None`] when the value is busy or
    /// gone.
    pub fn try_borrow(&self) -> Option<DynamicManagedRef> {
        unsafe {
            match &self.kind {
                Kind::Owned { lifetime, data } => {
                    DynamicManagedRef::new_raw(self.type_hash, lifetime.borrow()?, *data)
                }
                Kind::Referenced { lifetime, data } => {
                    DynamicManagedRef::new_raw(self.type_hash, lifetime.borrow()?, *data)
                }
            }
        }
    }

    /// Takes an exclusive handle, or returns [`None`] when the value is busy or
    /// gone.
    pub fn try_borrow_mut(&self) -> Option<DynamicManagedRefMut> {
        unsafe {
            match &self.kind {
                Kind::Owned { lifetime, data } => {
                    DynamicManagedRefMut::new_raw(self.type_hash, lifetime.borrow_mut()?, *data)
                }
                Kind::Referenced { lifetime, data } => {
                    DynamicManagedRefMut::new_raw(self.type_hash, lifetime.borrow_mut()?, *data)
                }
            }
        }
    }

    /// Takes a shared handle, spinning until it is free when `LOCKING`.
    ///
    /// # Panics
    ///
    /// Panics when the value is gone, or when it is busy and `LOCKING` is
    /// `false`.
    pub fn borrow<const LOCKING: bool>(&self) -> DynamicManagedRef {
        unsafe {
            if LOCKING {
                match &self.kind {
                    Kind::Owned { lifetime, data } => loop {
                        if let Some(lifetime) = lifetime.borrow() {
                            return DynamicManagedRef::new_raw(self.type_hash, lifetime, *data)
                                .expect("DynamicManagedGc cannot be immutably borrowed");
                        }
                        std::hint::spin_loop();
                    },
                    Kind::Referenced { lifetime, data } => loop {
                        if !lifetime.exists() {
                            panic!("DynamicManagedGc owner is dead");
                        }
                        if let Some(lifetime) = lifetime.borrow() {
                            return DynamicManagedRef::new_raw(self.type_hash, lifetime, *data)
                                .expect("DynamicManagedGc cannot be immutably borrowed");
                        }
                        std::hint::spin_loop();
                    },
                }
            } else {
                match &self.kind {
                    Kind::Owned { lifetime, data } => DynamicManagedRef::new_raw(
                        self.type_hash,
                        lifetime
                            .borrow()
                            .expect("DynamicManagedGc is inaccessible for immutable borrowing"),
                        *data,
                    )
                    .expect("DynamicManagedGc cannot be immutably borrowed"),
                    Kind::Referenced { lifetime, data } => DynamicManagedRef::new_raw(
                        self.type_hash,
                        lifetime
                            .borrow()
                            .expect("DynamicManagedGc is inaccessible for immutable borrowing"),
                        *data,
                    )
                    .expect("DynamicManagedGc cannot be immutably borrowed"),
                }
            }
        }
    }

    /// Takes an exclusive handle, spinning until it is free when `LOCKING`.
    ///
    /// # Panics
    ///
    /// Panics when the value is gone, or when it is busy and `LOCKING` is
    /// `false`.
    pub fn borrow_mut<const LOCKING: bool>(&mut self) -> DynamicManagedRefMut {
        unsafe {
            if LOCKING {
                match &self.kind {
                    Kind::Owned { lifetime, data } => loop {
                        if let Some(lifetime) = lifetime.borrow_mut() {
                            return DynamicManagedRefMut::new_raw(self.type_hash, lifetime, *data)
                                .expect("DynamicManagedGc cannot be mutably borrowed");
                        }
                        std::hint::spin_loop();
                    },
                    Kind::Referenced { lifetime, data } => loop {
                        if !lifetime.exists() {
                            panic!("DynamicManagedGc owner is dead");
                        }
                        if let Some(lifetime) = lifetime.borrow_mut() {
                            return DynamicManagedRefMut::new_raw(self.type_hash, lifetime, *data)
                                .expect("DynamicManagedGc cannot be mutably borrowed");
                        }
                        std::hint::spin_loop();
                    },
                }
            } else {
                match &self.kind {
                    Kind::Owned { lifetime, data } => DynamicManagedRefMut::new_raw(
                        self.type_hash,
                        lifetime
                            .borrow_mut()
                            .expect("DynamicManagedGc is inaccessible for mutable borrowing"),
                        *data,
                    )
                    .expect("DynamicManagedGc cannot be mutably borrowed"),
                    Kind::Referenced { lifetime, data } => DynamicManagedRefMut::new_raw(
                        self.type_hash,
                        lifetime
                            .borrow_mut()
                            .expect("DynamicManagedGc is inaccessible for mutable borrowing"),
                        *data,
                    )
                    .expect("DynamicManagedGc cannot be mutably borrowed"),
                }
            }
        }
    }

    /// Takes an unclaimed handle, which claims nothing and never blocks.
    ///
    /// # Panics
    ///
    /// Panics when the value is gone.
    pub fn lazy(&self) -> DynamicManagedLazy {
        unsafe {
            match &self.kind {
                Kind::Owned { lifetime, data } => {
                    DynamicManagedLazy::new_raw(self.type_hash, lifetime.lazy(), *data)
                        .expect("DynamicManagedGc cannot be lazily borrowed")
                }
                Kind::Referenced { lifetime, data } => {
                    DynamicManagedLazy::new_raw(self.type_hash, lifetime.clone(), *data)
                        .expect("DynamicManagedGc cannot be lazily borrowed")
                }
            }
        }
    }

    /// Returns the allocation pointer, checking nothing.
    ///
    /// # Safety
    ///
    /// Neither the type, the borrow state, nor whether the value is still
    /// alive is checked.
    pub unsafe fn as_ptr_raw(&self) -> *const u8 {
        match &self.kind {
            Kind::Owned { data, .. } => *data as *const u8,
            Kind::Referenced { data, .. } => *data as *const u8,
        }
    }

    /// Returns the mutable allocation pointer, checking nothing.
    ///
    /// # Safety
    ///
    /// Neither the type, the borrow state, nor whether the value is still
    /// alive is checked.
    pub unsafe fn as_mut_ptr_raw(&mut self) -> *mut u8 {
        match &self.kind {
            Kind::Owned { data, .. } => *data,
            Kind::Referenced { data, .. } => *data,
        }
    }
}

impl TryFrom<DynamicManagedValue> for DynamicManagedGc {
    type Error = ();

    fn try_from(value: DynamicManagedValue) -> Result<Self, Self::Error> {
        match value {
            DynamicManagedValue::Gc(value) => Ok(value),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_async() {
        fn is_async<T: Send + Sync>() {}

        is_async::<ManagedGc<()>>();
        is_async::<DynamicManagedGc>();
    }

    #[test]
    fn test_managed_gc() {
        let mut managed = ManagedGc::new(42);
        {
            let read_access = managed.read::<true>();
            assert_eq!(*read_access, 42);
        }
        {
            let mut write_access = managed.write::<true>();
            *write_access = 100;
        }
        {
            let read_access = managed.read::<true>();
            assert_eq!(*read_access, 100);
        }
    }

    #[test]
    #[allow(unused)]
    fn test_managed_gc_lifetimes() {
        struct Car {
            gear: i32,
            engine: Option<ManagedGc<Engine>>,
        }

        struct Engine {
            owning_car: Option<ManagedGc<Car>>,
            horsepower: i32,
        }

        let mut car = ManagedGc::new(Car {
            gear: 1,
            engine: None,
        });
        let engine = ManagedGc::new(Engine {
            owning_car: Some(car.reference()),
            horsepower: 200,
        });
        let engine2 = engine.reference();
        car.write::<true>().engine = Some(engine);

        assert!(car.exists());
        assert!(car.is_owning());
        assert!(engine2.exists());
        assert!(engine2.is_referencing());
        assert!(engine2.is_owned_by(car.read::<true>().engine.as_ref().unwrap()));

        let car2 = car.reference();
        assert!(car2.exists());
        assert!(car2.is_referencing());

        drop(car);
        assert!(!car2.exists());
        assert!(car2.try_read().is_none());
        assert!(!engine2.exists());
        assert!(engine2.try_read().is_none());
    }

    #[test]
    fn test_managed_gc_cycles() {
        #[derive(Default)]
        struct Foo {
            other: Option<ManagedGc<Self>>,
        }

        {
            let mut a = ManagedGc::<Foo>::default();
            let mut b = ManagedGc::<Foo>::default();
            a.write::<true>().other = Some(b.reference());
            b.write::<true>().other = Some(a.reference());

            assert!(a.exists());
            assert!(a.is_owning());
            assert!(a.read::<true>().other.as_ref().unwrap().is_referencing());
            assert!(a.read::<true>().other.as_ref().unwrap().is_owned_by(&b));

            assert!(b.exists());
            assert!(b.is_owning());
            assert!(b.read::<true>().other.as_ref().unwrap().is_referencing());
            assert!(b.read::<true>().other.as_ref().unwrap().is_owned_by(&a));

            drop(b);
            assert!(!a.read::<true>().other.as_ref().unwrap().exists());
        }

        {
            let mut a = ManagedGc::<Foo>::default();
            a.write::<true>().other = Some(a.reference());

            assert!(a.exists());
            assert!(a.is_owning());
            assert!(a.read::<true>().other.as_ref().unwrap().is_referencing());
            assert!(a.read::<true>().other.as_ref().unwrap().is_owned_by(&a));
        }
    }

    #[test]
    fn test_dynamic_managed_gc() {
        let mut managed = DynamicManagedGc::new(42);
        {
            let read_access = managed.read::<true, i32>();
            assert_eq!(*read_access, 42);
        }
        {
            let mut write_access = managed.write::<true, i32>();
            *write_access = 100;
        }
        {
            let read_access = managed.read::<true, i32>();
            assert_eq!(*read_access, 100);
        }
    }

    #[test]
    fn test_dynamic_managed_gc_cycles() {
        #[derive(Default)]
        struct Foo {
            other: Option<DynamicManagedGc>,
        }

        {
            let mut a = DynamicManagedGc::new(Foo::default());
            let mut b = DynamicManagedGc::new(Foo::default());
            a.write::<true, Foo>().other = Some(b.reference());
            b.write::<true, Foo>().other = Some(a.reference());

            assert!(a.exists());
            assert!(a.is_owning());
            assert!(
                a.read::<true, Foo>()
                    .other
                    .as_ref()
                    .unwrap()
                    .is_referencing()
            );
            assert!(
                a.read::<true, Foo>()
                    .other
                    .as_ref()
                    .unwrap()
                    .is_owned_by(&b)
            );

            assert!(b.exists());
            assert!(b.is_owning());
            assert!(
                b.read::<true, Foo>()
                    .other
                    .as_ref()
                    .unwrap()
                    .is_referencing()
            );
            assert!(
                b.read::<true, Foo>()
                    .other
                    .as_ref()
                    .unwrap()
                    .is_owned_by(&a)
            );

            drop(b);
            assert!(!a.read::<true, Foo>().other.as_ref().unwrap().exists());
        }

        {
            let mut a = DynamicManagedGc::new(Foo::default());
            a.write::<true, Foo>().other = Some(a.reference());

            assert!(a.exists());
            assert!(a.is_owning());
            assert!(
                a.read::<true, Foo>()
                    .other
                    .as_ref()
                    .unwrap()
                    .is_referencing()
            );
            assert!(
                a.read::<true, Foo>()
                    .other
                    .as_ref()
                    .unwrap()
                    .is_owned_by(&a)
            );
        }
    }

    #[test]
    fn test_managed_gc_conversions() {
        let managed = ManagedGc::new(42);
        assert_eq!(*managed.read::<true>(), 42);

        let mut dynamic = managed.into_dynamic();
        *dynamic.write::<true, i32>() = 100;

        let managed = dynamic.into_typed::<i32>();
        assert_eq!(*managed.read::<true>(), 100);
    }

    #[test]
    fn test_managed_gc_dead_owner() {
        let a = ManagedGc::new(42);
        let mut b = a.reference();

        assert!(a.exists());
        assert!(b.exists());
        assert_eq!(*b.read::<true>(), 42);

        drop(a);
        assert!(!b.exists());
        assert!(b.try_write().is_none());
    }

    #[test]
    #[should_panic]
    fn test_managed_gc_dead_owner_panic() {
        let a = ManagedGc::new(42);
        let mut b = a.reference();

        assert!(a.exists());
        assert!(b.exists());
        assert_eq!(*b.read::<true>(), 42);

        drop(a);
        assert!(!b.exists());
        assert_eq!(*b.write::<true>(), 42);
    }

    #[test]
    fn test_managed_gc_cyclic() {
        struct SelfReferencial {
            value: i32,
            this: ManagedGc<SelfReferencial>,
        }

        let v = unsafe { ManagedGc::new_cyclic(|this| SelfReferencial { value: 42, this }) };
        assert_eq!(v.read::<true>().value, 42);
        let this = v.read::<true>().this.reference();
        assert_eq!(this.read::<true>().value, 42);
    }

    #[test]
    fn test_managed_gc_transfer_ownership() {
        let mut a = ManagedGc::new(42);
        let mut b = a.reference();

        assert!(!a.is_owned_by(&b));
        assert!(b.is_owned_by(&a));
        assert!(!b.transfer_ownership(&mut a));
        assert!(a.transfer_ownership(&mut b));
        assert!(a.is_owned_by(&b));
        assert!(!b.is_owned_by(&a));
        drop(b);
        assert!(!a.exists());
    }
}
