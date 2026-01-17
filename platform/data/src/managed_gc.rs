use crate::{
    Finalize,
    lifetime::{Lifetime, LifetimeLazy, ValueReadAccess, ValueWriteAccess},
    managed::{
        DynamicManagedLazy, DynamicManagedRef, DynamicManagedRefMut, ManagedLazy, ManagedRef,
        ManagedRefMut,
    },
    non_zero_alloc, non_zero_dealloc,
    type_hash::TypeHash,
};
use std::{
    alloc::{Layout, handle_alloc_error},
    marker::PhantomData,
    mem::MaybeUninit,
};

enum Kind {
    Owned {
        lifetime: Lifetime,
        data: *mut u8,
    },
    Referenced {
        lifetime: LifetimeLazy,
        data: *mut u8,
    },
}

pub enum ManagedGcLifetime<'a> {
    Owned(&'a Lifetime),
    Referenced(&'a LifetimeLazy),
}

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
    pub fn new(data: T) -> Self {
        Self {
            dynamic: DynamicManagedGc::new(data),
            _phantom: PhantomData,
        }
    }

    pub fn consume(self) -> Result<T, Self> {
        self.dynamic.consume().map_err(|value| Self {
            dynamic: value,
            _phantom: PhantomData,
        })
    }

    pub fn into_dynamic(self) -> DynamicManagedGc {
        self.dynamic
    }

    pub fn renew(&mut self) {
        self.dynamic.renew();
    }

    pub fn type_hash(&self) -> TypeHash {
        self.dynamic.type_hash()
    }

    pub fn lifetime(&self) -> ManagedGcLifetime<'_> {
        self.dynamic.lifetime()
    }

    pub fn exists(&self) -> bool {
        self.dynamic.exists()
    }

    pub fn is_owning(&self) -> bool {
        self.dynamic.is_owning()
    }

    pub fn is_referencing(&self) -> bool {
        self.dynamic.is_referencing()
    }

    pub fn is_owned_by(&self, other: &Self) -> bool {
        self.dynamic.is_owned_by(&other.dynamic)
    }

    pub fn try_read(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        self.dynamic.try_read::<T>()
    }

    pub fn try_write(&'_ mut self) -> Option<ValueWriteAccess<'_, T>> {
        self.dynamic.try_write::<T>()
    }

    pub fn read<const LOCKING: bool>(&'_ self) -> ValueReadAccess<'_, T> {
        self.dynamic.read::<LOCKING, T>()
    }

    pub fn write<const LOCKING: bool>(&'_ mut self) -> ValueWriteAccess<'_, T> {
        self.dynamic.write::<LOCKING, T>()
    }

    pub fn borrow<const LOCKING: bool>(&self) -> ManagedRef<T> {
        self.dynamic
            .borrow::<LOCKING>()
            .into_typed()
            .ok()
            .expect("ManagedGc cannot be immutably borrowed")
    }

    pub fn borrow_mut<const LOCKING: bool>(&mut self) -> ManagedRefMut<T> {
        self.dynamic
            .borrow_mut::<LOCKING>()
            .into_typed()
            .ok()
            .expect("ManagedGc cannot be mutably borrowed")
    }

    pub fn lazy(&self) -> ManagedLazy<T> {
        self.dynamic
            .lazy()
            .into_typed()
            .ok()
            .expect("ManagedGc cannot be lazily borrowed")
    }

    /// # Safety
    pub unsafe fn as_ptr(&self) -> *const T {
        unsafe { self.dynamic.as_ptr_raw().cast::<T>() }
    }

    /// # Safety
    pub unsafe fn as_mut_ptr(&mut self) -> *mut T {
        unsafe { self.dynamic.as_mut_ptr_raw().cast::<T>() }
    }
}

impl<T> Clone for ManagedGc<T> {
    fn clone(&self) -> Self {
        Self {
            dynamic: self.dynamic.clone(),
            _phantom: PhantomData,
        }
    }
}

pub struct DynamicManagedGc {
    type_hash: TypeHash,
    kind: Kind,
    layout: Layout,
    finalizer: unsafe fn(*mut ()),
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
                (self.finalizer)(data.cast::<()>());
                non_zero_dealloc(*data, self.layout);
            }
        }
    }
}

impl DynamicManagedGc {
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
                finalizer: T::finalize_raw,
                drop: true,
            }
        }
    }

    pub fn new_raw(
        type_hash: TypeHash,
        lifetime: Lifetime,
        memory: *mut u8,
        layout: Layout,
        finalizer: unsafe fn(*mut ()),
    ) -> Self {
        if memory.is_null() {
            handle_alloc_error(layout);
        }
        Self {
            type_hash,
            kind: Kind::Owned {
                lifetime,
                data: memory,
            },
            layout,
            finalizer,
            drop: true,
        }
    }

    pub fn new_uninitialized(
        type_hash: TypeHash,
        layout: Layout,
        finalizer: unsafe fn(*mut ()),
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
            finalizer,
            drop: true,
        }
    }

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

    pub fn into_typed<T>(self) -> ManagedGc<T> {
        ManagedGc {
            dynamic: self,
            _phantom: PhantomData,
        }
    }

    pub fn renew(&mut self) {
        if let Kind::Owned { lifetime, .. } = &mut self.kind {
            *lifetime = Default::default();
        }
    }

    pub fn type_hash(&self) -> TypeHash {
        self.type_hash
    }

    pub fn lifetime(&self) -> ManagedGcLifetime<'_> {
        match &self.kind {
            Kind::Owned { lifetime, .. } => ManagedGcLifetime::Owned(lifetime),
            Kind::Referenced { lifetime, .. } => ManagedGcLifetime::Referenced(lifetime),
        }
    }

    pub fn exists(&self) -> bool {
        match &self.kind {
            Kind::Owned { .. } => true,
            Kind::Referenced { lifetime, .. } => lifetime.exists(),
        }
    }

    pub fn is_owning(&self) -> bool {
        matches!(self.kind, Kind::Owned { .. })
    }

    pub fn is_referencing(&self) -> bool {
        matches!(self.kind, Kind::Referenced { .. })
    }

    pub fn is_owned_by(&self, other: &Self) -> bool {
        if let (Kind::Referenced { lifetime: l1, .. }, Kind::Owned { lifetime: l2, .. }) =
            (&self.kind, &other.kind)
        {
            l1.state().is_owned_by(l2.state())
        } else {
            false
        }
    }

    pub fn is<T>(&self) -> bool {
        self.type_hash == TypeHash::of::<T>()
    }

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

    /// # Safety
    pub unsafe fn as_ptr_raw(&self) -> *const u8 {
        match &self.kind {
            Kind::Owned { data, .. } => *data as *const u8,
            Kind::Referenced { data, .. } => *data as *const u8,
        }
    }

    /// # Safety
    pub unsafe fn as_mut_ptr_raw(&mut self) -> *mut u8 {
        match &self.kind {
            Kind::Owned { data, .. } => *data,
            Kind::Referenced { data, .. } => *data,
        }
    }
}

impl Clone for DynamicManagedGc {
    fn clone(&self) -> Self {
        match &self.kind {
            Kind::Owned { lifetime, data } => Self {
                type_hash: self.type_hash,
                kind: Kind::Referenced {
                    lifetime: lifetime.lazy(),
                    data: *data,
                },
                layout: self.layout,
                finalizer: self.finalizer,
                drop: true,
            },
            Kind::Referenced { lifetime, data } => Self {
                type_hash: self.type_hash,
                kind: Kind::Referenced {
                    lifetime: lifetime.clone(),
                    data: *data,
                },
                layout: self.layout,
                finalizer: self.finalizer,
                drop: true,
            },
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
    fn test_managed_gc_cycles() {
        #[derive(Default)]
        struct Foo {
            other: Option<ManagedGc<Self>>,
        }

        {
            let mut a = ManagedGc::<Foo>::default();
            let mut b = ManagedGc::<Foo>::default();
            a.write::<true>().other = Some(b.clone());
            b.write::<true>().other = Some(a.clone());

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
            a.write::<true>().other = Some(a.clone());

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
            a.write::<true, Foo>().other = Some(b.clone());
            b.write::<true, Foo>().other = Some(a.clone());

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
            a.write::<true, Foo>().other = Some(a.clone());

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
    fn test_gc_conversions() {
        let managed = ManagedGc::new(42);
        assert_eq!(*managed.read::<true>(), 42);

        let mut dynamic = managed.into_dynamic();
        *dynamic.write::<true, i32>() = 100;

        let managed = dynamic.into_typed::<i32>();
        assert_eq!(*managed.read::<true>(), 100);
    }

    #[test]
    fn test_gc_dead_owner() {
        let a = ManagedGc::new(42);
        let mut b = a.clone();

        assert!(a.exists());
        assert!(b.exists());
        assert_eq!(*b.read::<true>(), 42);

        drop(a);
        assert!(!b.exists());
        assert!(b.try_write().is_none());
    }

    #[test]
    #[should_panic]
    fn test_gc_dead_owner_panic() {
        let a = ManagedGc::new(42);
        let mut b = a.clone();

        assert!(a.exists());
        assert!(b.exists());
        assert_eq!(*b.read::<true>(), 42);

        drop(a);
        assert!(!b.exists());
        assert_eq!(*b.write::<true>(), 42);
    }
}
