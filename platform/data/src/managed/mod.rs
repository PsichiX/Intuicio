//! Value boxes with runtime checked borrowing.
//!
//! A managed box is an owner plus a [`Lifetime`],
//! so handles to it can be handed to a script and still be checked at
//! runtime. When the owner goes away, every handle reports it instead of
//! dangling.
//!
//! # The four roles
//!
//! | Role | Typed | Type-erased |
//! |------|-------|-------------|
//! | owns the value | [`Managed`] | [`DynamicManaged`] |
//! | shared handle | [`ManagedRef`] | [`DynamicManagedRef`] |
//! | exclusive handle | [`ManagedRefMut`] | [`DynamicManagedRefMut`] |
//! | unclaimed handle | [`ManagedLazy`] | [`DynamicManagedLazy`] |
//!
//! The typed boxes know their Rust type at compile time. The dynamic ones
//! carry a [`TypeHash`] instead and check it on every access, which is what
//! script values use. `into_dynamic` and `into_typed` convert between them.
//!
//! Two more shapes build on these: [`value`] wraps all roles in one enum, so
//! code can accept a value without caring how it is held, and [`gc`] adds
//! boxes that survive reference cycles.
//!
//! ```
//! # use intuicio_data::managed::Managed;
//! let mut value = Managed::new(42);
//! let borrow = value.borrow().unwrap();
//! // a shared handle is out, so an exclusive one is refused
//! assert!(value.borrow_mut().is_none());
//! assert_eq!(*borrow.read().unwrap(), 42);
//! ```
pub mod gc;
pub mod value;

use crate::{
    Finalize, Finalizer,
    lifetime::{
        Lifetime, LifetimeLazy, LifetimeRef, LifetimeRefMut, ValueReadAccess, ValueWriteAccess,
    },
    managed::value::{DynamicManagedValue, ManagedValue},
    non_zero_alloc, non_zero_dealloc,
    type_hash::TypeHash,
};
use std::{alloc::Layout, mem::MaybeUninit};

/// Owner of a value plus its runtime borrow state.
///
/// The value is stored inline, so this is just `T` with a lifetime attached.
/// Handles taken from it go dead when it is dropped. See the
/// [module docs](self).
#[derive(Default)]
pub struct Managed<T> {
    lifetime: Lifetime,
    data: T,
}

impl<T> Managed<T> {
    /// Takes ownership of a value with a fresh lifetime.
    pub fn new(data: T) -> Self {
        Self {
            lifetime: Default::default(),
            data,
        }
    }

    /// Takes ownership of a value with a lifetime prepared elsewhere.
    pub fn new_raw(data: T, lifetime: Lifetime) -> Self {
        Self { lifetime, data }
    }

    /// Splits into the lifetime and the value, dropping no handles.
    pub fn into_inner(self) -> (Lifetime, T) {
        (self.lifetime, self.data)
    }

    /// Moves the value into a type-erased box, giving `self` back when the
    /// allocation fails.
    ///
    /// The lifetime is not carried over, so handles taken so far go dead.
    pub fn into_dynamic(self) -> Result<DynamicManaged, Self> {
        match DynamicManaged::new(self.data) {
            Ok(value) => Ok(value),
            Err(data) => Err(Managed {
                lifetime: self.lifetime,
                data,
            }),
        }
    }

    /// Replaces the lifetime, killing every handle taken so far.
    pub fn renew(mut self) -> Self {
        self.lifetime = Lifetime::default();
        self
    }

    /// Returns the borrow state of this value.
    pub fn lifetime(&self) -> &Lifetime {
        &self.lifetime
    }

    /// Guards the value for reading, or returns [`None`] while it is written.
    pub fn read(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        self.lifetime.read(&self.data)
    }

    /// Guards the value for writing, or returns [`None`] while it is accessed.
    pub fn write(&'_ mut self) -> Option<ValueWriteAccess<'_, T>> {
        self.lifetime.write(&mut self.data)
    }

    /// Takes the value out, or gives the box back while any access guard is
    /// live.
    pub fn consume(self) -> Result<T, Self> {
        if self.lifetime.state().is_in_use() {
            Err(self)
        } else {
            Ok(self.data)
        }
    }

    /// Moves the value into the place `target` points at.
    ///
    /// # Panics
    ///
    /// Panics when `target` cannot be written.
    pub fn move_into_ref(self, mut target: ManagedRefMut<T>) -> Result<(), Self> {
        *target.write().unwrap() = self.consume()?;
        Ok(())
    }

    /// Moves the value into the place `target` points at.
    ///
    /// # Panics
    ///
    /// Panics when `target` cannot be written.
    pub fn move_into_lazy(self, target: ManagedLazy<T>) -> Result<(), Self> {
        *target.write().unwrap() = self.consume()?;
        Ok(())
    }

    /// Takes a shared handle, or returns [`None`] when an exclusive one is out.
    pub fn borrow(&self) -> Option<ManagedRef<T>> {
        Some(ManagedRef::new(&self.data, self.lifetime.borrow()?))
    }

    /// Takes an exclusive handle, or returns [`None`] when any handle is out.
    pub fn borrow_mut(&mut self) -> Option<ManagedRefMut<T>> {
        Some(ManagedRefMut::new(
            &mut self.data,
            self.lifetime.borrow_mut()?,
        ))
    }

    /// Takes an unclaimed handle.
    pub fn lazy(&mut self) -> ManagedLazy<T> {
        ManagedLazy::new(&mut self.data, self.lifetime.lazy())
    }

    /// [`Managed::lazy`] from a shared reference.
    ///
    /// # Safety
    ///
    /// The returned handle can write to a value the caller only borrowed
    /// immutably. Only use it when nothing else holds a `&T` to it.
    pub unsafe fn lazy_immutable(&self) -> ManagedLazy<T> {
        unsafe {
            ManagedLazy::new_raw(&self.data as *const T as *mut T, self.lifetime.lazy()).unwrap()
        }
    }

    /// Replaces the value with one built from it, under a fresh lifetime.
    ///
    /// # Safety
    ///
    /// Handles taken so far are not checked before the value is moved out, and
    /// they go dead. Nothing may be accessing the value.
    pub unsafe fn map<U>(self, f: impl FnOnce(T) -> U) -> Managed<U> {
        Managed {
            lifetime: Default::default(),
            data: f(self.data),
        }
    }

    /// [`Managed::map`] that can decline, dropping the value when it does.
    ///
    /// # Safety
    ///
    /// Same as [`Managed::map`].
    pub unsafe fn try_map<U>(self, f: impl FnOnce(T) -> Option<U>) -> Option<Managed<U>> {
        f(self.data).map(|data| Managed {
            lifetime: Default::default(),
            data,
        })
    }

    /// Returns a pointer to the value, bypassing the borrow state.
    ///
    /// # Safety
    ///
    /// The caller takes over checking for conflicting access, and must not
    /// outlive this box.
    pub unsafe fn as_ptr(&self) -> *const T {
        &self.data as _
    }

    /// Returns a mutable pointer to the value, bypassing the borrow state.
    ///
    /// # Safety
    ///
    /// The caller takes over checking for conflicting access, and must not
    /// outlive this box.
    pub unsafe fn as_mut_ptr(&mut self) -> *mut T {
        &mut self.data as _
    }
}

/// Shared handle to a value owned by a [`Managed`], the runtime `&T`.
///
/// Keeps its claim until dropped, and reports a dead owner by returning
/// [`None`] from [`ManagedRef::read`].
pub struct ManagedRef<T: ?Sized> {
    lifetime: LifetimeRef,
    data: *const T,
}

unsafe impl<T: ?Sized> Send for ManagedRef<T> where T: Send {}
unsafe impl<T: ?Sized> Sync for ManagedRef<T> where T: Sync {}

impl<T: ?Sized> ManagedRef<T> {
    /// Pairs a reference with a shared borrow taken from its lifetime.
    pub fn new(data: &T, lifetime: LifetimeRef) -> Self {
        Self {
            lifetime,
            data: data as *const T,
        }
    }

    /// [`ManagedRef::new`] over a raw pointer. A null pointer yields [`None`].
    ///
    /// # Safety
    ///
    /// `data` must stay valid for as long as `lifetime` says it does, and
    /// `lifetime` must be the borrow state that guards it.
    pub unsafe fn new_raw(data: *const T, lifetime: LifetimeRef) -> Option<Self> {
        if data.is_null() {
            None
        } else {
            Some(Self { lifetime, data })
        }
    }

    /// Builds a handle to a plain reference along with the lifetime that backs
    /// it.
    ///
    /// The caller has to keep the returned [`Lifetime`] alive for at least as
    /// long as the handle.
    pub fn make(data: &T) -> (Self, Lifetime) {
        let result = Lifetime::default();
        (Self::new(data, result.borrow().unwrap()), result)
    }

    /// [`ManagedRef::make`] over a raw pointer.
    ///
    /// # Safety
    ///
    /// `data` must stay valid for as long as the returned handle lives.
    pub unsafe fn make_raw(data: *const T) -> Option<(Self, Lifetime)> {
        let result = Lifetime::default();
        Some((
            unsafe { Self::new_raw(data, result.borrow().unwrap()) }?,
            result,
        ))
    }

    /// Splits into the borrow token and the pointer.
    pub fn into_inner(self) -> (LifetimeRef, *const T) {
        (self.lifetime, self.data)
    }

    /// Erases the type, keeping the same claim.
    pub fn into_dynamic(self) -> DynamicManagedRef {
        unsafe {
            DynamicManagedRef::new_raw(TypeHash::of::<T>(), self.lifetime, self.data as *const u8)
                .unwrap()
        }
    }

    /// Returns the borrow token.
    pub fn lifetime(&self) -> &LifetimeRef {
        &self.lifetime
    }

    /// Takes another shared handle to the same value.
    pub fn borrow(&self) -> Option<ManagedRef<T>> {
        Some(ManagedRef {
            lifetime: self.lifetime.borrow()?,
            data: self.data,
        })
    }

    /// Turns this shared handle into an unclaimed one that can also write.
    ///
    /// # Safety
    ///
    /// The value was only borrowed immutably, so writing through the result is
    /// only sound when nothing else holds a `&T` to it.
    pub unsafe fn lazy_immutable(&self) -> ManagedLazy<T> {
        ManagedLazy {
            lifetime: self.lifetime.lazy(),
            data: self.data as *mut T,
        }
    }

    /// Guards the value for reading, or returns [`None`] when it is written or
    /// the owner is gone.
    pub fn read(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        unsafe { self.lifetime.read_ptr(self.data) }
    }

    /// Narrows this handle down to a part of the value, such as one field.
    ///
    /// # Safety
    ///
    /// `f` must return a reference into the same value, and the owner must
    /// still be alive.
    pub unsafe fn map<U>(self, f: impl FnOnce(&T) -> &U) -> ManagedRef<U> {
        unsafe {
            let data = f(&*self.data);
            ManagedRef {
                lifetime: self.lifetime,
                data: data as *const U,
            }
        }
    }

    /// [`ManagedRef::map`] that can decline.
    ///
    /// # Safety
    ///
    /// Same as [`ManagedRef::map`].
    pub unsafe fn try_map<U>(self, f: impl FnOnce(&T) -> Option<&U>) -> Option<ManagedRef<U>> {
        unsafe {
            f(&*self.data).map(|data| ManagedRef {
                lifetime: self.lifetime,
                data: data as *const U,
            })
        }
    }

    /// Returns the pointer while the owner is alive, bypassing access checks.
    ///
    /// # Safety
    ///
    /// The caller takes over checking for conflicting access.
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

/// Exclusive handle to a value owned by a [`Managed`], the runtime `&mut T`.
///
/// Can be reborrowed, both shared and exclusive, one level deeper.
pub struct ManagedRefMut<T: ?Sized> {
    lifetime: LifetimeRefMut,
    data: *mut T,
}

unsafe impl<T: ?Sized> Send for ManagedRefMut<T> where T: Send {}
unsafe impl<T: ?Sized> Sync for ManagedRefMut<T> where T: Sync {}

impl<T: ?Sized> ManagedRefMut<T> {
    /// Pairs a mutable reference with an exclusive borrow of its lifetime.
    pub fn new(data: &mut T, lifetime: LifetimeRefMut) -> Self {
        Self {
            lifetime,
            data: data as *mut T,
        }
    }

    /// [`ManagedRefMut::new`] over a raw pointer. A null pointer yields
    /// [`None`].
    ///
    /// # Safety
    ///
    /// `data` must stay valid and unaliased for as long as `lifetime` says it
    /// does, and `lifetime` must be the borrow state that guards it.
    pub unsafe fn new_raw(data: *mut T, lifetime: LifetimeRefMut) -> Option<Self> {
        if data.is_null() {
            None
        } else {
            Some(Self { lifetime, data })
        }
    }

    /// Builds a handle to a plain mutable reference along with the lifetime that
    /// backs it.
    ///
    /// The caller has to keep the returned [`Lifetime`] alive for at least as
    /// long as the handle.
    pub fn make(data: &mut T) -> (Self, Lifetime) {
        let result = Lifetime::default();
        (Self::new(data, result.borrow_mut().unwrap()), result)
    }

    /// [`ManagedRefMut::make`] over a raw pointer.
    ///
    /// # Safety
    ///
    /// `data` must stay valid and unaliased for as long as the returned handle
    /// lives.
    pub unsafe fn make_raw(data: *mut T) -> Option<(Self, Lifetime)> {
        let result = Lifetime::default();
        Some((
            unsafe { Self::new_raw(data, result.borrow_mut().unwrap()) }?,
            result,
        ))
    }

    /// Splits into the borrow token and the pointer.
    pub fn into_inner(self) -> (LifetimeRefMut, *mut T) {
        (self.lifetime, self.data)
    }

    /// Erases the type, keeping the same claim.
    pub fn into_dynamic(self) -> DynamicManagedRefMut {
        unsafe {
            DynamicManagedRefMut::new_raw(TypeHash::of::<T>(), self.lifetime, self.data as *mut u8)
                .unwrap()
        }
    }

    /// Returns the borrow token.
    pub fn lifetime(&self) -> &LifetimeRefMut {
        &self.lifetime
    }

    /// Takes a shared handle nested under this one.
    pub fn borrow(&self) -> Option<ManagedRef<T>> {
        Some(ManagedRef {
            lifetime: self.lifetime.borrow()?,
            data: self.data,
        })
    }

    /// Takes an exclusive handle nested under this one.
    pub fn borrow_mut(&mut self) -> Option<ManagedRefMut<T>> {
        Some(ManagedRefMut {
            lifetime: self.lifetime.borrow_mut()?,
            data: self.data,
        })
    }

    /// Takes an unclaimed handle.
    pub fn lazy(&self) -> ManagedLazy<T> {
        ManagedLazy {
            lifetime: self.lifetime.lazy(),
            data: self.data,
        }
    }

    /// Guards the value for reading, or returns [`None`] when it is written or
    /// the owner is gone.
    pub fn read(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        unsafe { self.lifetime.read_ptr(self.data) }
    }

    /// Guards the value for writing, or returns [`None`] when it is accessed or
    /// the owner is gone.
    pub fn write(&'_ mut self) -> Option<ValueWriteAccess<'_, T>> {
        unsafe { self.lifetime.write_ptr(self.data) }
    }

    /// Narrows this handle down to a part of the value, such as one field.
    ///
    /// # Safety
    ///
    /// `f` must return a reference into the same value, and the owner must
    /// still be alive.
    pub unsafe fn map<U>(self, f: impl FnOnce(&mut T) -> &mut U) -> ManagedRefMut<U> {
        unsafe {
            let data = f(&mut *self.data);
            ManagedRefMut {
                lifetime: self.lifetime,
                data: data as *mut U,
            }
        }
    }

    /// [`ManagedRefMut::map`] that can decline.
    ///
    /// # Safety
    ///
    /// Same as [`ManagedRefMut::map`].
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

    /// Returns the pointer while the owner is alive, bypassing access checks.
    ///
    /// # Safety
    ///
    /// The caller takes over checking for conflicting access.
    pub unsafe fn as_ptr(&self) -> Option<*const T> {
        if self.lifetime.exists() {
            Some(self.data)
        } else {
            None
        }
    }

    /// Returns the mutable pointer while the owner is alive, bypassing access
    /// checks.
    ///
    /// # Safety
    ///
    /// The caller takes over checking for conflicting access.
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

/// Unclaimed handle to a value owned by a [`Managed`].
///
/// Holding one blocks nobody, and it can be cloned freely. Every access is
/// checked when it happens. This is the handle a script variable holds, because
/// such a variable is read and written at any point, with no Rust borrow to
/// model it.
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
    /// Pairs a mutable reference with an unclaimed handle to its lifetime.
    pub fn new(data: &mut T, lifetime: LifetimeLazy) -> Self {
        Self {
            lifetime,
            data: data as *mut T,
        }
    }

    /// [`ManagedLazy::new`] over a raw pointer. A null pointer yields [`None`].
    ///
    /// # Safety
    ///
    /// `data` must stay valid for as long as `lifetime` says it does, and
    /// `lifetime` must be the borrow state that guards it.
    pub unsafe fn new_raw(data: *mut T, lifetime: LifetimeLazy) -> Option<Self> {
        if data.is_null() {
            None
        } else {
            Some(Self { lifetime, data })
        }
    }

    /// Builds a handle to a plain mutable reference along with the lifetime that
    /// backs it.
    ///
    /// The caller has to keep the returned [`Lifetime`] alive for at least as
    /// long as the handle.
    pub fn make(data: &mut T) -> (Self, Lifetime) {
        let result = Lifetime::default();
        (Self::new(data, result.lazy()), result)
    }

    /// [`ManagedLazy::make`] over a raw pointer.
    ///
    /// # Safety
    ///
    /// `data` must stay valid for as long as the returned handle lives.
    pub unsafe fn make_raw(data: *mut T) -> Option<(Self, Lifetime)> {
        let result = Lifetime::default();
        Some((unsafe { Self::new_raw(data, result.lazy()) }?, result))
    }

    /// Splits into the lifetime handle and the pointer.
    pub fn into_inner(self) -> (LifetimeLazy, *mut T) {
        (self.lifetime, self.data)
    }

    /// Erases the type, keeping the same handle.
    pub fn into_dynamic(self) -> DynamicManagedLazy {
        unsafe {
            DynamicManagedLazy::new_raw(TypeHash::of::<T>(), self.lifetime, self.data as *mut u8)
                .unwrap()
        }
    }

    /// Returns the lifetime handle.
    pub fn lifetime(&self) -> &LifetimeLazy {
        &self.lifetime
    }

    /// Upgrades to a shared handle that holds its claim.
    pub fn borrow(&self) -> Option<ManagedRef<T>> {
        Some(ManagedRef {
            lifetime: self.lifetime.borrow()?,
            data: self.data,
        })
    }

    /// Upgrades to an exclusive handle that holds its claim.
    pub fn borrow_mut(&mut self) -> Option<ManagedRefMut<T>> {
        Some(ManagedRefMut {
            lifetime: self.lifetime.borrow_mut()?,
            data: self.data,
        })
    }

    /// Guards the value for reading, or returns [`None`] when it is written or
    /// the owner is gone.
    pub fn read(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        unsafe { self.lifetime.read_ptr(self.data) }
    }

    /// Guards the value for writing, or returns [`None`] when it is accessed or
    /// the owner is gone.
    ///
    /// Takes `&self`, since a lazy handle claims nothing of its own.
    pub fn write(&'_ self) -> Option<ValueWriteAccess<'_, T>> {
        unsafe { self.lifetime.write_ptr(self.data) }
    }

    /// Narrows this handle down to a part of the value, such as one field.
    ///
    /// # Safety
    ///
    /// `f` must return a reference into the same value, and the owner must
    /// still be alive.
    pub unsafe fn map<U>(self, f: impl FnOnce(&mut T) -> &mut U) -> ManagedLazy<U> {
        unsafe {
            let data = f(&mut *self.data);
            ManagedLazy {
                lifetime: self.lifetime,
                data: data as *mut U,
            }
        }
    }

    /// [`ManagedLazy::map`] that can decline.
    ///
    /// # Safety
    ///
    /// Same as [`ManagedLazy::map`].
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

    /// Returns the pointer while the owner is alive, bypassing access checks.
    ///
    /// # Safety
    ///
    /// The caller takes over checking for conflicting access.
    pub unsafe fn as_ptr(&self) -> Option<*const T> {
        if self.lifetime.exists() {
            Some(self.data)
        } else {
            None
        }
    }

    /// Returns the mutable pointer while the owner is alive, bypassing access
    /// checks.
    ///
    /// # Safety
    ///
    /// The caller takes over checking for conflicting access.
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

/// Owner of a value whose type is only known at runtime.
///
/// The [`Managed`] counterpart for script values: the value lives in its own
/// allocation, identified by a [`TypeHash`] and destroyed through a stored
/// drop function. Every typed access checks the hash first.
pub struct DynamicManaged {
    type_hash: TypeHash,
    lifetime: Lifetime,
    memory: *mut u8,
    layout: Layout,
    finalizer: Finalizer,
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
                self.finalizer.finalize(data_pointer);
                non_zero_dealloc(self.memory, self.layout);
                self.memory = std::ptr::null_mut();
            }
        }
    }
}

impl DynamicManaged {
    /// Moves a value into a new allocation, giving it back when the allocation
    /// fails.
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
                    finalizer: Finalizer::of::<T>(),
                    drop: true,
                })
            }
        }
    }

    /// Takes ownership of an existing allocation.
    ///
    /// Returns [`None`] for a null pointer. The box will free `memory` and run
    /// `finalizer` on drop.
    pub fn new_raw(
        type_hash: TypeHash,
        lifetime: Lifetime,
        memory: *mut u8,
        layout: Layout,
        finalizer: impl Into<Finalizer>,
    ) -> Option<Self> {
        if memory.is_null() {
            None
        } else {
            Some(Self {
                type_hash,
                lifetime,
                memory,
                layout,
                finalizer: finalizer.into(),
                drop: true,
            })
        }
    }

    /// Allocates room for a value without writing one into it.
    ///
    /// Nothing stops the box from running its finalizer on drop, so the caller
    /// must fill the memory before it is dropped or read.
    pub fn new_uninitialized(
        type_hash: TypeHash,
        layout: Layout,
        finalizer: impl Into<Finalizer>,
    ) -> Self {
        let layout = layout.pad_to_align();
        let memory = unsafe { non_zero_alloc(layout) };
        Self {
            type_hash,
            lifetime: Default::default(),
            memory,
            layout,
            finalizer: finalizer.into(),
            drop: true,
        }
    }

    /// Builds a box by copying a byte image of a value into a new allocation.
    ///
    /// # Safety
    ///
    /// `bytes` must be a valid image of a value of the type named by
    /// `type_hash`, matching `layout`, and it is moved, so the caller must not
    /// drop the source afterwards.
    pub unsafe fn from_bytes(
        type_hash: TypeHash,
        lifetime: Lifetime,
        bytes: Vec<u8>,
        layout: Layout,
        finalizer: impl Into<Finalizer>,
    ) -> Self {
        let layout = layout.pad_to_align();
        let memory = unsafe { non_zero_alloc(layout) };
        unsafe { memory.copy_from(bytes.as_ptr(), bytes.len()) };
        Self {
            type_hash,
            lifetime,
            memory,
            layout,
            finalizer: finalizer.into(),
            drop: true,
        }
    }

    /// Splits into the parts the box was built from, and stops it from freeing
    /// the allocation.
    ///
    /// The caller becomes responsible for running the finalizer and freeing the
    /// memory.
    #[allow(clippy::type_complexity)]
    pub fn into_inner(mut self) -> (TypeHash, Lifetime, *mut u8, Layout, Finalizer) {
        self.drop = false;
        (
            self.type_hash,
            std::mem::take(&mut self.lifetime),
            self.memory,
            self.layout,
            self.finalizer.clone(),
        )
    }

    /// Moves the value into a typed box, giving `self` back on a type mismatch
    /// or while it is accessed.
    pub fn into_typed<T>(self) -> Result<Managed<T>, Self> {
        Ok(Managed::new(self.consume()?))
    }

    /// Replaces the lifetime, killing every handle taken so far.
    pub fn renew(mut self) -> Self {
        self.lifetime = Lifetime::default();
        self
    }

    /// Returns the type of the stored value.
    pub fn type_hash(&self) -> &TypeHash {
        &self.type_hash
    }

    /// Returns the borrow state of this value.
    pub fn lifetime(&self) -> &Lifetime {
        &self.lifetime
    }

    /// Returns the layout of the allocation.
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Returns how the stored value is destroyed.
    pub fn finalizer(&self) -> &Finalizer {
        &self.finalizer
    }

    /// Returns the value as raw bytes.
    ///
    /// # Safety
    ///
    /// Bypasses the borrow state, so the caller must know that nothing is
    /// writing the value.
    pub unsafe fn memory(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.memory, self.layout.size()) }
    }

    /// Returns the value as mutable raw bytes.
    ///
    /// # Safety
    ///
    /// Bypasses the borrow state, and writing bytes that are not a valid value
    /// of the stored type makes every later access undefined.
    pub unsafe fn memory_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.memory, self.layout.size()) }
    }

    /// Returns `true` when the stored type is `T`.
    pub fn is<T>(&self) -> bool {
        self.type_hash == TypeHash::of::<T>()
    }

    /// Guards the value for reading, or returns [`None`] on a type mismatch or
    /// while it is written.
    pub fn read<T>(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { self.lifetime.read_ptr(self.memory.cast::<T>()) }
        } else {
            None
        }
    }

    /// Guards the value for writing, or returns [`None`] on a type mismatch or
    /// while it is accessed.
    pub fn write<T>(&'_ mut self) -> Option<ValueWriteAccess<'_, T>> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { self.lifetime.write_ptr(self.memory.cast::<T>()) }
        } else {
            None
        }
    }

    /// Takes the value out and frees the allocation.
    ///
    /// Gives the box back on a type mismatch or while any access guard is live.
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

    /// Moves the value into the place `target` points at and frees this
    /// allocation.
    ///
    /// Gives the box back when the types differ or both sides are the same
    /// allocation.
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

    /// Moves the value into the place `target` points at and frees this
    /// allocation.
    ///
    /// Gives the box back when the types differ or both sides are the same
    /// allocation.
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

    /// Takes a shared handle, or returns [`None`] when an exclusive one is out.
    pub fn borrow(&self) -> Option<DynamicManagedRef> {
        unsafe { DynamicManagedRef::new_raw(self.type_hash, self.lifetime.borrow()?, self.memory) }
    }

    /// Takes an exclusive handle, or returns [`None`] when any handle is out.
    pub fn borrow_mut(&mut self) -> Option<DynamicManagedRefMut> {
        unsafe {
            DynamicManagedRefMut::new_raw(self.type_hash, self.lifetime.borrow_mut()?, self.memory)
        }
    }

    /// Takes an unclaimed handle.
    pub fn lazy(&self) -> DynamicManagedLazy {
        unsafe {
            DynamicManagedLazy::new_raw(self.type_hash, self.lifetime.lazy(), self.memory).unwrap()
        }
    }

    /// Replaces the value with one built from it, in a new allocation.
    ///
    /// # Safety
    ///
    /// Handles taken so far are not checked before the value is moved out, and
    /// they go dead. Returns [`None`] when the stored type is not `T`.
    pub unsafe fn map<T, U: Finalize>(self, f: impl FnOnce(T) -> U) -> Option<Self> {
        let data = self.consume::<T>().ok()?;
        let data = f(data);
        Self::new(data).ok()
    }

    /// [`DynamicManaged::map`] that can decline, dropping the value when it
    /// does.
    ///
    /// # Safety
    ///
    /// Same as [`DynamicManaged::map`].
    pub unsafe fn try_map<T, U: Finalize>(self, f: impl FnOnce(T) -> Option<U>) -> Option<Self> {
        let data = self.consume::<T>().ok()?;
        let data = f(data)?;
        Self::new(data).ok()
    }

    /// Returns a typed pointer while nothing is accessing the value.
    ///
    /// # Safety
    ///
    /// The caller takes over checking for conflicting access.
    pub unsafe fn as_ptr<T>(&self) -> Option<*const T> {
        if self.type_hash == TypeHash::of::<T>() && !self.lifetime.state().is_in_use() {
            Some(self.memory.cast::<T>())
        } else {
            None
        }
    }

    /// Returns a typed mutable pointer while nothing is accessing the value.
    ///
    /// # Safety
    ///
    /// The caller takes over checking for conflicting access.
    pub unsafe fn as_mut_ptr<T>(&mut self) -> Option<*mut T> {
        if self.type_hash == TypeHash::of::<T>() && !self.lifetime.state().is_in_use() {
            Some(self.memory.cast::<T>())
        } else {
            None
        }
    }

    /// Returns the allocation pointer, checking nothing at all.
    ///
    /// # Safety
    ///
    /// Neither the type nor the borrow state is checked.
    pub unsafe fn as_ptr_raw(&self) -> *const u8 {
        self.memory
    }

    /// Returns the mutable allocation pointer, checking nothing at all.
    ///
    /// # Safety
    ///
    /// Neither the type nor the borrow state is checked.
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

/// Shared handle to a value whose type is only known at runtime.
///
/// The [`ManagedRef`] counterpart for script values.
pub struct DynamicManagedRef {
    type_hash: TypeHash,
    lifetime: LifetimeRef,
    data: *const u8,
}

unsafe impl Send for DynamicManagedRef {}
unsafe impl Sync for DynamicManagedRef {}

impl DynamicManagedRef {
    /// Pairs a reference with a shared borrow taken from its lifetime.
    pub fn new<T: ?Sized>(data: &T, lifetime: LifetimeRef) -> Self {
        Self {
            type_hash: TypeHash::of::<T>(),
            lifetime,
            data: data as *const T as *const u8,
        }
    }

    /// [`DynamicManagedRef::new`] for a type only known at runtime. A null
    /// pointer yields [`None`].
    ///
    /// # Safety
    ///
    /// `data` must point at a value of the type named by `type_hash`, and must
    /// stay valid for as long as `lifetime` says it does.
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

    /// Builds a handle to a plain reference along with the lifetime that backs
    /// it.
    pub fn make<T: ?Sized>(data: &T) -> (Self, Lifetime) {
        let result = Lifetime::default();
        (Self::new(data, result.borrow().unwrap()), result)
    }

    /// Splits into type, borrow token and pointer.
    pub fn into_inner(self) -> (TypeHash, LifetimeRef, *const u8) {
        (self.type_hash, self.lifetime, self.data)
    }

    /// Recovers the typed handle, or gives `self` back on a type mismatch.
    pub fn into_typed<T>(self) -> Result<ManagedRef<T>, Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { Ok(ManagedRef::new_raw(self.data.cast::<T>(), self.lifetime).unwrap()) }
        } else {
            Err(self)
        }
    }

    /// Returns the type of the value.
    pub fn type_hash(&self) -> &TypeHash {
        &self.type_hash
    }

    /// Returns the borrow token.
    pub fn lifetime(&self) -> &LifetimeRef {
        &self.lifetime
    }

    /// Takes another shared handle to the same value.
    pub fn borrow(&self) -> Option<DynamicManagedRef> {
        Some(DynamicManagedRef {
            type_hash: self.type_hash,
            lifetime: self.lifetime.borrow()?,
            data: self.data,
        })
    }

    /// Turns this shared handle into an unclaimed one that can also write.
    ///
    /// # Safety
    ///
    /// The value was only borrowed immutably, so writing through the result is
    /// only sound when nothing else holds a shared reference to it.
    pub unsafe fn lazy_immutable(&self) -> DynamicManagedLazy {
        DynamicManagedLazy {
            type_hash: self.type_hash,
            lifetime: self.lifetime.lazy(),
            data: self.data as *mut u8,
        }
    }

    /// Returns `true` when the value is a `T`.
    pub fn is<T>(&self) -> bool {
        self.type_hash == TypeHash::of::<T>()
    }

    /// Guards the value for reading, or returns [`None`] on a type mismatch or
    /// while it is written.
    pub fn read<T>(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { self.lifetime.read_ptr(self.data.cast::<T>()) }
        } else {
            None
        }
    }

    /// Narrows this handle down to a part of the value, retyping it to `U`.
    ///
    /// # Safety
    ///
    /// `f` must return a reference into the same value, and the owner must
    /// still be alive. Returns [`None`] when the value is not a `T`.
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

    /// [`DynamicManagedRef::map`] that can decline.
    ///
    /// # Safety
    ///
    /// Same as [`DynamicManagedRef::map`].
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

    /// Returns a typed pointer while the owner is alive.
    ///
    /// # Safety
    ///
    /// The caller takes over checking for conflicting access.
    pub unsafe fn as_ptr<T>(&self) -> Option<*const T> {
        if self.type_hash == TypeHash::of::<T>() && self.lifetime.exists() {
            Some(self.data.cast::<T>())
        } else {
            None
        }
    }

    /// Returns the untyped pointer while the owner is alive.
    ///
    /// # Safety
    ///
    /// Neither the type nor conflicting access is checked.
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

/// Exclusive handle to a value whose type is only known at runtime.
///
/// The [`ManagedRefMut`] counterpart for script values.
pub struct DynamicManagedRefMut {
    type_hash: TypeHash,
    lifetime: LifetimeRefMut,
    data: *mut u8,
}

unsafe impl Send for DynamicManagedRefMut {}
unsafe impl Sync for DynamicManagedRefMut {}

impl DynamicManagedRefMut {
    /// Pairs a mutable reference with an exclusive borrow of its lifetime.
    pub fn new<T: ?Sized>(data: &mut T, lifetime: LifetimeRefMut) -> Self {
        Self {
            type_hash: TypeHash::of::<T>(),
            lifetime,
            data: data as *mut T as *mut u8,
        }
    }

    /// [`DynamicManagedRefMut::new`] for a type only known at runtime. A null
    /// pointer yields [`None`].
    ///
    /// # Safety
    ///
    /// `data` must point at a value of the type named by `type_hash`, and must
    /// stay valid and unaliased for as long as `lifetime` says it does.
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

    /// Builds a handle to a plain mutable reference along with the lifetime that
    /// backs it.
    pub fn make<T: ?Sized>(data: &mut T) -> (Self, Lifetime) {
        let result = Lifetime::default();
        (Self::new(data, result.borrow_mut().unwrap()), result)
    }

    /// Splits into type, borrow token and pointer.
    pub fn into_inner(self) -> (TypeHash, LifetimeRefMut, *mut u8) {
        (self.type_hash, self.lifetime, self.data)
    }

    /// Recovers the typed handle, or gives `self` back on a type mismatch.
    pub fn into_typed<T>(self) -> Result<ManagedRefMut<T>, Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { Ok(ManagedRefMut::new_raw(self.data.cast::<T>(), self.lifetime).unwrap()) }
        } else {
            Err(self)
        }
    }

    /// Returns the type of the value.
    pub fn type_hash(&self) -> &TypeHash {
        &self.type_hash
    }

    /// Returns the borrow token.
    pub fn lifetime(&self) -> &LifetimeRefMut {
        &self.lifetime
    }

    /// Takes a shared handle nested under this one.
    pub fn borrow(&self) -> Option<DynamicManagedRef> {
        Some(DynamicManagedRef {
            type_hash: self.type_hash,
            lifetime: self.lifetime.borrow()?,
            data: self.data,
        })
    }

    /// Takes an exclusive handle nested under this one.
    pub fn borrow_mut(&mut self) -> Option<DynamicManagedRefMut> {
        Some(DynamicManagedRefMut {
            type_hash: self.type_hash,
            lifetime: self.lifetime.borrow_mut()?,
            data: self.data,
        })
    }

    /// Takes an unclaimed handle.
    pub fn lazy(&self) -> DynamicManagedLazy {
        DynamicManagedLazy {
            type_hash: self.type_hash,
            lifetime: self.lifetime.lazy(),
            data: self.data,
        }
    }

    /// Returns `true` when the value is a `T`.
    pub fn is<T>(&self) -> bool {
        self.type_hash == TypeHash::of::<T>()
    }

    /// Guards the value for reading, or returns [`None`] on a type mismatch or
    /// while it is written.
    pub fn read<T>(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { self.lifetime.read_ptr(self.data.cast::<T>()) }
        } else {
            None
        }
    }

    /// Guards the value for writing, or returns [`None`] on a type mismatch or
    /// while it is accessed.
    pub fn write<T>(&'_ mut self) -> Option<ValueWriteAccess<'_, T>> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { self.lifetime.write_ptr(self.data.cast::<T>()) }
        } else {
            None
        }
    }

    /// Narrows this handle down to a part of the value, retyping it to `U`.
    ///
    /// # Safety
    ///
    /// `f` must return a reference into the same value, and the owner must
    /// still be alive. Returns [`None`] when the value is not a `T`.
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

    /// [`DynamicManagedRefMut::map`] that can decline.
    ///
    /// # Safety
    ///
    /// Same as [`DynamicManagedRefMut::map`].
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

    /// Returns a typed pointer while the owner is alive.
    ///
    /// # Safety
    ///
    /// The caller takes over checking for conflicting access.
    pub unsafe fn as_ptr<T>(&self) -> Option<*const T> {
        if self.type_hash == TypeHash::of::<T>() && self.lifetime.exists() {
            Some(self.data.cast::<T>())
        } else {
            None
        }
    }

    /// Returns a typed mutable pointer while the owner is alive.
    ///
    /// # Safety
    ///
    /// The caller takes over checking for conflicting access.
    pub unsafe fn as_mut_ptr<T>(&mut self) -> Option<*mut T> {
        if self.type_hash == TypeHash::of::<T>() && self.lifetime.exists() {
            Some(self.data.cast::<T>())
        } else {
            None
        }
    }

    /// Returns the untyped pointer while the owner is alive.
    ///
    /// # Safety
    ///
    /// Neither the type nor conflicting access is checked.
    pub unsafe fn as_ptr_raw(&self) -> Option<*const u8> {
        if self.lifetime.exists() {
            Some(self.data)
        } else {
            None
        }
    }

    /// Returns the untyped mutable pointer while the owner is alive.
    ///
    /// # Safety
    ///
    /// Neither the type nor conflicting access is checked.
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

/// Unclaimed handle to a value whose type is only known at runtime.
///
/// The [`ManagedLazy`] counterpart for script values, and the handle script
/// variables usually hold.
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
    /// Pairs a mutable reference with an unclaimed handle to its lifetime.
    pub fn new<T: ?Sized>(data: &mut T, lifetime: LifetimeLazy) -> Self {
        Self {
            type_hash: TypeHash::of::<T>(),
            lifetime,
            data: data as *mut T as *mut u8,
        }
    }

    /// [`DynamicManagedLazy::new`] for a type only known at runtime. A null
    /// pointer yields [`None`].
    ///
    /// # Safety
    ///
    /// `data` must point at a value of the type named by `type_hash`, and must
    /// stay valid for as long as `lifetime` says it does.
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

    /// Builds a handle to a plain mutable reference along with the lifetime that
    /// backs it.
    pub fn make<T: ?Sized>(data: &mut T) -> (Self, Lifetime) {
        let result = Lifetime::default();
        (Self::new(data, result.lazy()), result)
    }

    /// Splits into type, lifetime handle and pointer.
    pub fn into_inner(self) -> (TypeHash, LifetimeLazy, *mut u8) {
        (self.type_hash, self.lifetime, self.data)
    }

    /// Recovers the typed handle, or gives `self` back on a type mismatch.
    pub fn into_typed<T>(self) -> Result<ManagedLazy<T>, Self> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { Ok(ManagedLazy::new_raw(self.data.cast::<T>(), self.lifetime).unwrap()) }
        } else {
            Err(self)
        }
    }

    /// Returns the type of the value.
    pub fn type_hash(&self) -> &TypeHash {
        &self.type_hash
    }

    /// Returns the lifetime handle.
    pub fn lifetime(&self) -> &LifetimeLazy {
        &self.lifetime
    }

    /// Returns `true` when the value is a `T`.
    pub fn is<T>(&self) -> bool {
        self.type_hash == TypeHash::of::<T>()
    }

    /// Guards the value for reading, or returns [`None`] on a type mismatch or
    /// while it is written.
    pub fn read<T>(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { self.lifetime.read_ptr(self.data.cast::<T>()) }
        } else {
            None
        }
    }

    /// Guards the value for writing, or returns [`None`] on a type mismatch or
    /// while it is accessed.
    ///
    /// Takes `&self`, since a lazy handle claims nothing of its own.
    pub fn write<T>(&'_ self) -> Option<ValueWriteAccess<'_, T>> {
        if self.type_hash == TypeHash::of::<T>() {
            unsafe { self.lifetime.write_ptr(self.data.cast::<T>()) }
        } else {
            None
        }
    }

    /// Upgrades to a shared handle that holds its claim.
    pub fn borrow(&self) -> Option<DynamicManagedRef> {
        Some(DynamicManagedRef {
            type_hash: self.type_hash,
            lifetime: self.lifetime.borrow()?,
            data: self.data,
        })
    }

    /// Upgrades to an exclusive handle that holds its claim.
    pub fn borrow_mut(&mut self) -> Option<DynamicManagedRefMut> {
        Some(DynamicManagedRefMut {
            type_hash: self.type_hash,
            lifetime: self.lifetime.borrow_mut()?,
            data: self.data,
        })
    }

    /// Narrows this handle down to a part of the value, retyping it to `U`.
    ///
    /// # Safety
    ///
    /// `f` must return a reference into the same value, and the owner must
    /// still be alive. Returns [`None`] when the value is not a `T`.
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

    /// [`DynamicManagedLazy::map`] that can decline.
    ///
    /// # Safety
    ///
    /// Same as [`DynamicManagedLazy::map`].
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

    /// Returns a typed pointer while the owner is alive.
    ///
    /// # Safety
    ///
    /// The caller takes over checking for conflicting access.
    pub unsafe fn as_ptr<T>(&self) -> Option<*const T> {
        if self.type_hash == TypeHash::of::<T>() && self.lifetime.exists() {
            Some(self.data.cast::<T>())
        } else {
            None
        }
    }

    /// Returns a typed mutable pointer while the owner is alive.
    ///
    /// # Safety
    ///
    /// The caller takes over checking for conflicting access.
    pub unsafe fn as_mut_ptr<T>(&self) -> Option<*mut T> {
        if self.type_hash == TypeHash::of::<T>() && self.lifetime.exists() {
            Some(self.data.cast::<T>())
        } else {
            None
        }
    }

    /// Returns the untyped pointer while the owner is alive.
    ///
    /// # Safety
    ///
    /// Neither the type nor conflicting access is checked.
    pub unsafe fn as_ptr_raw(&self) -> Option<*const u8> {
        if self.lifetime.exists() {
            Some(self.data)
        } else {
            None
        }
    }

    /// Returns the untyped mutable pointer while the owner is alive.
    ///
    /// # Safety
    ///
    /// Neither the type nor conflicting access is checked.
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
