use crate::{
    lifetime::{ValueReadAccess, ValueWriteAccess},
    managed::{
        DynamicManaged, DynamicManagedLazy, DynamicManagedRef, DynamicManagedRefMut, Managed,
        ManagedLazy, ManagedRef, ManagedRefMut,
        gc::{DynamicManagedGc, ManagedGc},
    },
};

pub enum ManagedValue<T> {
    Owned(Managed<T>),
    Ref(ManagedRef<T>),
    RefMut(ManagedRefMut<T>),
    Lazy(ManagedLazy<T>),
    Gc(ManagedGc<T>),
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

    pub fn as_gc(&self) -> Option<&ManagedGc<T>> {
        match self {
            Self::Gc(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_mut_gc(&mut self) -> Option<&mut ManagedGc<T>> {
        match self {
            Self::Gc(value) => Some(value),
            _ => None,
        }
    }

    pub fn read(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        match self {
            Self::Owned(value) => value.read(),
            Self::Ref(value) => value.read(),
            Self::RefMut(value) => value.read(),
            Self::Lazy(value) => value.read(),
            Self::Gc(value) => value.try_read(),
        }
    }

    pub fn write(&'_ mut self) -> Option<ValueWriteAccess<'_, T>> {
        match self {
            Self::Owned(value) => value.write(),
            Self::RefMut(value) => value.write(),
            Self::Lazy(value) => value.write(),
            Self::Gc(value) => value.try_write(),
            _ => None,
        }
    }

    pub fn borrow(&self) -> Option<ManagedRef<T>> {
        match self {
            Self::Owned(value) => value.borrow(),
            Self::Ref(value) => value.borrow(),
            Self::RefMut(value) => value.borrow(),
            Self::Gc(value) => value.try_borrow(),
            _ => None,
        }
    }

    pub fn borrow_mut(&mut self) -> Option<ManagedRefMut<T>> {
        match self {
            Self::Owned(value) => value.borrow_mut(),
            Self::RefMut(value) => value.borrow_mut(),
            Self::Gc(value) => value.try_borrow_mut(),
            _ => None,
        }
    }

    pub fn lazy(&mut self) -> Option<ManagedLazy<T>> {
        match self {
            Self::Owned(value) => Some(value.lazy()),
            Self::Lazy(value) => Some(value.clone()),
            Self::RefMut(value) => Some(value.lazy()),
            Self::Gc(value) => Some(value.lazy()),
            _ => None,
        }
    }

    /// # Safety
    pub unsafe fn lazy_immutable(&self) -> ManagedLazy<T> {
        unsafe {
            match self {
                Self::Owned(value) => value.lazy_immutable(),
                Self::Lazy(value) => value.clone(),
                Self::Ref(value) => value.lazy_immutable(),
                Self::RefMut(value) => value.lazy(),
                Self::Gc(value) => value.lazy(),
            }
        }
    }

    pub fn into_dynamic(self) -> Result<DynamicManagedValue, Self> {
        match self {
            Self::Owned(value) => match value.into_dynamic() {
                Ok(dynamic) => Ok(DynamicManagedValue::Owned(dynamic)),
                Err(original) => Err(Self::Owned(original)),
            },
            Self::Ref(value) => Ok(DynamicManagedValue::Ref(value.into_dynamic())),
            Self::RefMut(value) => Ok(DynamicManagedValue::RefMut(value.into_dynamic())),
            Self::Lazy(value) => Ok(DynamicManagedValue::Lazy(value.into_dynamic())),
            Self::Gc(value) => Ok(DynamicManagedValue::Gc(value.into_dynamic())),
        }
    }
}

impl<T> From<Managed<T>> for ManagedValue<T> {
    fn from(value: Managed<T>) -> Self {
        Self::Owned(value)
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

impl<T> From<ManagedGc<T>> for ManagedValue<T> {
    fn from(value: ManagedGc<T>) -> Self {
        Self::Gc(value)
    }
}

pub enum DynamicManagedValue {
    Owned(DynamicManaged),
    Ref(DynamicManagedRef),
    RefMut(DynamicManagedRefMut),
    Lazy(DynamicManagedLazy),
    Gc(DynamicManagedGc),
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

    pub fn as_gc(&self) -> Option<&DynamicManagedGc> {
        match self {
            Self::Gc(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_mut_gc(&mut self) -> Option<&mut DynamicManagedGc> {
        match self {
            Self::Gc(value) => Some(value),
            _ => None,
        }
    }

    pub fn read<T>(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        match self {
            Self::Owned(value) => value.read::<T>(),
            Self::Ref(value) => value.read::<T>(),
            Self::RefMut(value) => value.read::<T>(),
            Self::Lazy(value) => value.read::<T>(),
            Self::Gc(value) => value.try_read::<T>(),
        }
    }

    pub fn write<T>(&'_ mut self) -> Option<ValueWriteAccess<'_, T>> {
        match self {
            Self::Owned(value) => value.write::<T>(),
            Self::RefMut(value) => value.write::<T>(),
            Self::Lazy(value) => value.write::<T>(),
            Self::Gc(value) => value.try_write::<T>(),
            _ => None,
        }
    }

    pub fn borrow(&self) -> Option<DynamicManagedRef> {
        match self {
            Self::Owned(value) => value.borrow(),
            Self::Ref(value) => value.borrow(),
            Self::RefMut(value) => value.borrow(),
            Self::Gc(value) => value.try_borrow(),
            _ => None,
        }
    }

    pub fn borrow_mut(&mut self) -> Option<DynamicManagedRefMut> {
        match self {
            Self::Owned(value) => value.borrow_mut(),
            Self::RefMut(value) => value.borrow_mut(),
            Self::Gc(value) => value.try_borrow_mut(),
            _ => None,
        }
    }

    pub fn lazy(&self) -> Option<DynamicManagedLazy> {
        match self {
            Self::Owned(value) => Some(value.lazy()),
            Self::Lazy(value) => Some(value.clone()),
            Self::RefMut(value) => Some(value.lazy()),
            Self::Gc(value) => Some(value.lazy()),
            _ => None,
        }
    }

    /// # Safety
    pub unsafe fn lazy_immutable(&self) -> DynamicManagedLazy {
        unsafe {
            match self {
                Self::Owned(value) => value.lazy(),
                Self::Lazy(value) => value.clone(),
                Self::Ref(value) => value.lazy_immutable(),
                Self::RefMut(value) => value.lazy(),
                Self::Gc(value) => value.lazy(),
            }
        }
    }

    pub fn into_typed<T>(self) -> Result<ManagedValue<T>, Self> {
        match self {
            Self::Owned(value) => match value.into_typed() {
                Ok(typed) => Ok(ManagedValue::Owned(typed)),
                Err(original) => Err(Self::Owned(original)),
            },
            Self::Ref(value) => match value.into_typed() {
                Ok(typed) => Ok(ManagedValue::Ref(typed)),
                Err(original) => Err(Self::Ref(original)),
            },
            Self::RefMut(value) => match value.into_typed() {
                Ok(typed) => Ok(ManagedValue::RefMut(typed)),
                Err(original) => Err(Self::RefMut(original)),
            },
            Self::Lazy(value) => match value.into_typed() {
                Ok(typed) => Ok(ManagedValue::Lazy(typed)),
                Err(original) => Err(Self::Lazy(original)),
            },
            Self::Gc(value) => Ok(ManagedValue::Gc(value.into_typed())),
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

impl From<DynamicManagedGc> for DynamicManagedValue {
    fn from(value: DynamicManagedGc) -> Self {
        Self::Gc(value)
    }
}
