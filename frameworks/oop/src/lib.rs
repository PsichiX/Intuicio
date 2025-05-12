use intuicio_core::{
    registry::Registry,
    types::{TypeHandle, TypeQuery},
};
use intuicio_data::{
    managed::{
        DynamicManagedLazy, DynamicManagedRef, DynamicManagedRefMut, ManagedLazy, ManagedRef,
        ManagedRefMut,
    },
    type_hash::TypeHash,
};
use std::ops::{Deref, DerefMut};

pub struct ObjectRef<T> {
    actual_type_hash: TypeHash,
    data: ManagedRef<T>,
}

impl<T> ObjectRef<T> {
    pub fn new(data: ManagedRef<T>) -> Self {
        Self {
            actual_type_hash: TypeHash::of::<T>(),
            data,
        }
    }

    pub fn actual_type_hash(&self) -> TypeHash {
        self.actual_type_hash
    }

    pub fn current_type_hash(&self) -> TypeHash {
        TypeHash::of::<T>()
    }

    pub fn upcast<U>(self, registry: &Registry) -> Option<ObjectRef<U>> {
        let offset = inheritance_offset(
            self.current_type_hash(),
            TypeHash::of::<U>(),
            None,
            registry,
        )?;
        let Self {
            actual_type_hash,
            data,
        } = self;
        let (lifetime, memory) = data.into_inner();
        let data =
            unsafe { ManagedRef::new_raw(memory.cast::<u8>().add(offset).cast::<U>(), lifetime)? };
        Some(ObjectRef {
            actual_type_hash,
            data,
        })
    }

    pub fn downcast<U>(self, registry: &Registry) -> Option<ObjectRef<U>> {
        let offset = inheritance_offset(
            TypeHash::of::<U>(),
            self.current_type_hash(),
            Some(self.actual_type_hash),
            registry,
        )?;
        let Self {
            actual_type_hash,
            data,
        } = self;
        let (lifetime, memory) = data.into_inner();
        let data =
            unsafe { ManagedRef::new_raw(memory.cast::<u8>().sub(offset).cast::<U>(), lifetime)? };
        Some(ObjectRef {
            actual_type_hash,
            data,
        })
    }

    pub fn into_dynamic(self) -> DynamicObjectRef {
        let Self {
            actual_type_hash,
            data,
        } = self;
        DynamicObjectRef {
            actual_type_hash,
            data: data.into_dynamic(),
        }
    }
}

impl<T> Deref for ObjectRef<T> {
    type Target = ManagedRef<T>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> DerefMut for ObjectRef<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

pub struct ObjectRefMut<T> {
    actual_type_hash: TypeHash,
    data: ManagedRefMut<T>,
}

impl<T> ObjectRefMut<T> {
    pub fn new(data: ManagedRefMut<T>) -> Self {
        Self {
            actual_type_hash: TypeHash::of::<T>(),
            data,
        }
    }

    pub fn actual_type_hash(&self) -> TypeHash {
        self.actual_type_hash
    }

    pub fn current_type_hash(&self) -> TypeHash {
        TypeHash::of::<T>()
    }

    pub fn upcast<U>(self, registry: &Registry) -> Option<ObjectRefMut<U>> {
        let offset = inheritance_offset(
            self.current_type_hash(),
            TypeHash::of::<U>(),
            None,
            registry,
        )?;
        let Self {
            actual_type_hash,
            data,
        } = self;
        let (lifetime, memory) = data.into_inner();
        let data = unsafe {
            ManagedRefMut::new_raw(memory.cast::<u8>().add(offset).cast::<U>(), lifetime)?
        };
        Some(ObjectRefMut {
            actual_type_hash,
            data,
        })
    }

    pub fn downcast<U>(self, registry: &Registry) -> Option<ObjectRefMut<U>> {
        let offset = inheritance_offset(
            TypeHash::of::<U>(),
            self.current_type_hash(),
            Some(self.actual_type_hash),
            registry,
        )?;
        let Self {
            actual_type_hash,
            data,
        } = self;
        let (lifetime, memory) = data.into_inner();
        let data = unsafe {
            ManagedRefMut::new_raw(memory.cast::<u8>().sub(offset).cast::<U>(), lifetime)?
        };
        Some(ObjectRefMut {
            actual_type_hash,
            data,
        })
    }

    pub fn into_dynamic(self) -> DynamicObjectRefMut {
        let Self {
            actual_type_hash,
            data,
        } = self;
        DynamicObjectRefMut {
            actual_type_hash,
            data: data.into_dynamic(),
        }
    }
}

impl<T> Deref for ObjectRefMut<T> {
    type Target = ManagedRefMut<T>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> DerefMut for ObjectRefMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

pub struct ObjectLazy<T> {
    actual_type_hash: TypeHash,
    data: ManagedLazy<T>,
}

impl<T> ObjectLazy<T> {
    pub fn new(data: ManagedLazy<T>) -> Self {
        Self {
            actual_type_hash: TypeHash::of::<T>(),
            data,
        }
    }

    pub fn actual_type_hash(&self) -> TypeHash {
        self.actual_type_hash
    }

    pub fn current_type_hash(&self) -> TypeHash {
        TypeHash::of::<T>()
    }

    pub fn upcast<U>(self, registry: &Registry) -> Option<ObjectLazy<U>> {
        let offset = inheritance_offset(
            self.current_type_hash(),
            TypeHash::of::<U>(),
            None,
            registry,
        )?;
        let Self {
            actual_type_hash,
            data,
        } = self;
        let (lifetime, memory) = data.into_inner();
        let data =
            unsafe { ManagedLazy::new_raw(memory.cast::<u8>().add(offset).cast::<U>(), lifetime)? };
        Some(ObjectLazy {
            actual_type_hash,
            data,
        })
    }

    pub fn downcast<U>(self, registry: &Registry) -> Option<ObjectLazy<U>> {
        let offset = inheritance_offset(
            TypeHash::of::<U>(),
            self.current_type_hash(),
            Some(self.actual_type_hash),
            registry,
        )?;
        let Self {
            actual_type_hash,
            data,
        } = self;
        let (lifetime, memory) = data.into_inner();
        let data =
            unsafe { ManagedLazy::new_raw(memory.cast::<u8>().sub(offset).cast::<U>(), lifetime)? };
        Some(ObjectLazy {
            actual_type_hash,
            data,
        })
    }

    pub fn into_dynamic(self) -> DynamicObjectLazy {
        let Self {
            actual_type_hash,
            data,
        } = self;
        DynamicObjectLazy {
            actual_type_hash,
            data: data.into_dynamic(),
        }
    }
}

impl<T> Deref for ObjectLazy<T> {
    type Target = ManagedLazy<T>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> DerefMut for ObjectLazy<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<T> Clone for ObjectLazy<T> {
    fn clone(&self) -> Self {
        Self {
            actual_type_hash: self.actual_type_hash,
            data: self.data.clone(),
        }
    }
}

pub struct DynamicObjectRef {
    actual_type_hash: TypeHash,
    data: DynamicManagedRef,
}

impl DynamicObjectRef {
    pub fn new(data: DynamicManagedRef) -> Self {
        Self {
            actual_type_hash: *data.type_hash(),
            data,
        }
    }

    pub fn actual_type_hash(&self) -> TypeHash {
        self.actual_type_hash
    }

    pub fn current_type_hash(&self) -> TypeHash {
        *self.data.type_hash()
    }

    pub fn upcast(self, type_hash: TypeHash, registry: &Registry) -> Option<Self> {
        let offset = inheritance_offset(self.current_type_hash(), type_hash, None, registry)?;
        let Self {
            actual_type_hash,
            data,
        } = self;
        let (_, lifetime, memory) = data.into_inner();
        let data = unsafe { DynamicManagedRef::new_raw(type_hash, lifetime, memory.add(offset))? };
        Some(Self {
            actual_type_hash,
            data,
        })
    }

    pub fn downcast(self, type_hash: TypeHash, registry: &Registry) -> Option<Self> {
        let offset = inheritance_offset(
            type_hash,
            self.current_type_hash(),
            Some(self.actual_type_hash),
            registry,
        )?;
        let Self {
            actual_type_hash,
            data,
        } = self;
        let (_, lifetime, memory) = data.into_inner();
        let data = unsafe { DynamicManagedRef::new_raw(type_hash, lifetime, memory.sub(offset))? };
        Some(Self {
            actual_type_hash,
            data,
        })
    }

    pub fn into_inner(self, registry: &Registry) -> Option<DynamicManagedRef> {
        let type_hash = self.actual_type_hash;
        Some(self.downcast(type_hash, registry)?.data)
    }

    pub fn into_typed<T>(self) -> Result<ObjectRef<T>, Self> {
        let Self {
            actual_type_hash,
            data,
        } = self;
        match data.into_typed::<T>() {
            Ok(data) => Ok(ObjectRef {
                actual_type_hash,
                data,
            }),
            Err(data) => Err(Self {
                actual_type_hash,
                data,
            }),
        }
    }
}

impl Deref for DynamicObjectRef {
    type Target = DynamicManagedRef;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for DynamicObjectRef {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

pub struct DynamicObjectRefMut {
    actual_type_hash: TypeHash,
    data: DynamicManagedRefMut,
}

impl DynamicObjectRefMut {
    pub fn new(data: DynamicManagedRefMut) -> Self {
        Self {
            actual_type_hash: *data.type_hash(),
            data,
        }
    }

    pub fn actual_type_hash(&self) -> TypeHash {
        self.actual_type_hash
    }

    pub fn current_type_hash(&self) -> TypeHash {
        *self.data.type_hash()
    }

    pub fn upcast(self, type_hash: TypeHash, registry: &Registry) -> Option<Self> {
        let offset = inheritance_offset(self.current_type_hash(), type_hash, None, registry)?;
        let Self {
            actual_type_hash,
            data,
        } = self;
        let (_, lifetime, memory) = data.into_inner();
        let data =
            unsafe { DynamicManagedRefMut::new_raw(type_hash, lifetime, memory.add(offset))? };
        Some(Self {
            actual_type_hash,
            data,
        })
    }

    pub fn downcast(self, type_hash: TypeHash, registry: &Registry) -> Option<Self> {
        let offset = inheritance_offset(
            type_hash,
            self.current_type_hash(),
            Some(self.actual_type_hash),
            registry,
        )?;
        let Self {
            actual_type_hash,
            data,
        } = self;
        let (_, lifetime, memory) = data.into_inner();
        let data =
            unsafe { DynamicManagedRefMut::new_raw(type_hash, lifetime, memory.sub(offset))? };
        Some(Self {
            actual_type_hash,
            data,
        })
    }

    pub fn into_inner(self, registry: &Registry) -> Option<DynamicManagedRefMut> {
        let type_hash = self.actual_type_hash;
        Some(self.downcast(type_hash, registry)?.data)
    }

    pub fn into_typed<T>(self) -> Result<ObjectRefMut<T>, Self> {
        let Self {
            actual_type_hash,
            data,
        } = self;
        match data.into_typed::<T>() {
            Ok(data) => Ok(ObjectRefMut {
                actual_type_hash,
                data,
            }),
            Err(data) => Err(Self {
                actual_type_hash,
                data,
            }),
        }
    }
}

impl Deref for DynamicObjectRefMut {
    type Target = DynamicManagedRefMut;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for DynamicObjectRefMut {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

pub struct DynamicObjectLazy {
    actual_type_hash: TypeHash,
    data: DynamicManagedLazy,
}

impl DynamicObjectLazy {
    pub fn new(data: DynamicManagedLazy) -> Self {
        Self {
            actual_type_hash: *data.type_hash(),
            data,
        }
    }

    pub fn actual_type_hash(&self) -> TypeHash {
        self.actual_type_hash
    }

    pub fn current_type_hash(&self) -> TypeHash {
        *self.data.type_hash()
    }

    pub fn upcast(self, type_hash: TypeHash, registry: &Registry) -> Option<Self> {
        let offset = inheritance_offset(self.current_type_hash(), type_hash, None, registry)?;
        let Self {
            actual_type_hash,
            data,
        } = self;
        let (_, lifetime, memory) = data.into_inner();
        let data = unsafe { DynamicManagedLazy::new_raw(type_hash, lifetime, memory.add(offset))? };
        Some(Self {
            actual_type_hash,
            data,
        })
    }

    pub fn downcast(self, type_hash: TypeHash, registry: &Registry) -> Option<Self> {
        let offset = inheritance_offset(
            type_hash,
            self.current_type_hash(),
            Some(self.actual_type_hash),
            registry,
        )?;
        let Self {
            actual_type_hash,
            data,
        } = self;
        let (_, lifetime, memory) = data.into_inner();
        let data = unsafe { DynamicManagedLazy::new_raw(type_hash, lifetime, memory.sub(offset))? };
        Some(Self {
            actual_type_hash,
            data,
        })
    }

    pub fn into_inner(self, registry: &Registry) -> Option<DynamicManagedLazy> {
        let type_hash = self.actual_type_hash;
        Some(self.downcast(type_hash, registry)?.data)
    }

    pub fn into_typed<T>(self) -> Result<ObjectLazy<T>, Self> {
        let Self {
            actual_type_hash,
            data,
        } = self;
        match data.into_typed::<T>() {
            Ok(data) => Ok(ObjectLazy {
                actual_type_hash,
                data,
            }),
            Err(data) => Err(Self {
                actual_type_hash,
                data,
            }),
        }
    }
}

impl Deref for DynamicObjectLazy {
    type Target = DynamicManagedLazy;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for DynamicObjectLazy {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl Clone for DynamicObjectLazy {
    fn clone(&self) -> Self {
        Self {
            actual_type_hash: self.actual_type_hash,
            data: self.data.clone(),
        }
    }
}

fn inheritance_offset(
    source: TypeHash,
    target: TypeHash,
    limit: Option<TypeHash>,
    registry: &Registry,
) -> Option<usize> {
    let source_type = registry.find_type(TypeQuery {
        type_hash: Some(source),
        ..Default::default()
    })?;
    inheritance_offset_inner(&source_type, target, limit)
}

fn inheritance_offset_inner(
    source_type: &TypeHandle,
    target: TypeHash,
    limit: Option<TypeHash>,
) -> Option<usize> {
    if source_type.type_hash() == target {
        return Some(0);
    }
    let source_type = source_type.as_struct()?;
    for field in source_type.fields() {
        if !field
            .meta
            .as_ref()
            .map(|meta| meta.has_id("inherit"))
            .unwrap_or_default()
        {
            continue;
        }
        if let Some(limit) = limit {
            if field.type_handle().type_hash() == limit {
                return None;
            }
        }
        if field.type_handle().type_hash() == target {
            return Some(field.address_offset());
        }
        if let Some(offset) = inheritance_offset_inner(field.type_handle(), target, limit) {
            return Some(field.address_offset() + offset);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use intuicio_core::IntuicioStruct;
    use intuicio_data::managed::Managed;
    use intuicio_derive::IntuicioStruct;

    #[derive(IntuicioStruct, Default)]
    struct A {
        a: usize,
    }

    #[derive(IntuicioStruct, Default)]
    struct B {
        #[intuicio(meta = "inherit")]
        a: A,
        b: f32,
    }

    #[derive(IntuicioStruct, Default)]
    struct C {
        #[intuicio(meta = "inherit")]
        b: B,
        c: bool,
    }

    #[test]
    fn test_simple() {
        let mut registry = Registry::default().with_basic_types();
        registry.add_type(A::define_struct(&registry));
        registry.add_type(B::define_struct(&registry));
        registry.add_type(C::define_struct(&registry));

        let mut data = Managed::new(C {
            b: B {
                a: A { a: 42 },
                b: 4.2,
            },
            c: true,
        });

        {
            let c = ObjectRef::new(data.borrow().unwrap());
            assert_eq!(c.read().unwrap().b.a.a, 42);

            let a = c.upcast::<A>(&registry).unwrap();
            assert_eq!(a.read().unwrap().a, 42);

            let c = a.downcast::<C>(&registry).unwrap();
            assert_eq!(c.read().unwrap().b.a.a, 42);
        }

        {
            let c = ObjectRefMut::new(data.borrow_mut().unwrap());
            assert_eq!(c.read().unwrap().b.a.a, 42);

            let mut a = c.upcast::<A>(&registry).unwrap();
            a.write().unwrap().a = 1;

            let c = a.downcast::<C>(&registry).unwrap();
            assert_eq!(c.read().unwrap().b.a.a, 1);
        }

        {
            let c = ObjectLazy::new(data.lazy());
            assert_eq!(c.read().unwrap().b.a.a, 1);

            let a = c.upcast::<A>(&registry).unwrap();
            a.write().unwrap().a = 42;

            let c = a.downcast::<C>(&registry).unwrap();
            assert_eq!(c.read().unwrap().b.a.a, 42);
        }
    }

    #[test]
    fn test_casting() {
        let mut registry = Registry::default().with_basic_types();
        registry.add_type(A::define_struct(&registry));
        registry.add_type(B::define_struct(&registry));
        registry.add_type(C::define_struct(&registry));

        let mut data = Managed::new(C {
            b: B {
                a: A { a: 42 },
                b: 4.2,
            },
            c: true,
        });

        {
            let c = ObjectRef::new(data.borrow().unwrap());
            assert_eq!(c.current_type_hash(), TypeHash::of::<C>());
            assert_eq!(c.actual_type_hash(), TypeHash::of::<C>());
            assert!(c.read().unwrap().c);
            assert_eq!(c.read().unwrap().b.b, 4.2);
            assert_eq!(c.read().unwrap().b.a.a, 42);

            let b = c.upcast::<B>(&registry).unwrap();
            assert_eq!(b.current_type_hash(), TypeHash::of::<B>());
            assert_eq!(b.actual_type_hash(), TypeHash::of::<C>());
            assert_eq!(b.read().unwrap().b, 4.2);
            assert_eq!(b.read().unwrap().a.a, 42);

            let a = b.upcast::<A>(&registry).unwrap();
            assert_eq!(a.current_type_hash(), TypeHash::of::<A>());
            assert_eq!(a.actual_type_hash(), TypeHash::of::<C>());
            assert_eq!(a.read().unwrap().a, 42);

            let b = a.downcast::<B>(&registry).unwrap();
            assert_eq!(b.read().unwrap().b, 4.2);
            assert_eq!(b.read().unwrap().a.a, 42);

            let c = b.downcast::<C>(&registry).unwrap();
            assert!(c.read().unwrap().c);
            assert_eq!(c.read().unwrap().b.b, 4.2);
            assert_eq!(c.read().unwrap().b.a.a, 42);
        }

        {
            let c = ObjectRefMut::new(data.borrow_mut().unwrap());
            assert_eq!(c.current_type_hash(), TypeHash::of::<C>());
            assert_eq!(c.actual_type_hash(), TypeHash::of::<C>());
            assert!(c.read().unwrap().c);
            assert_eq!(c.read().unwrap().b.b, 4.2);
            assert_eq!(c.read().unwrap().b.a.a, 42);

            let b = c.upcast::<B>(&registry).unwrap();
            assert_eq!(b.current_type_hash(), TypeHash::of::<B>());
            assert_eq!(b.actual_type_hash(), TypeHash::of::<C>());
            assert_eq!(b.read().unwrap().b, 4.2);
            assert_eq!(b.read().unwrap().a.a, 42);

            let a = b.upcast::<A>(&registry).unwrap();
            assert_eq!(a.current_type_hash(), TypeHash::of::<A>());
            assert_eq!(a.actual_type_hash(), TypeHash::of::<C>());
            assert_eq!(a.read().unwrap().a, 42);

            let b = a.downcast::<B>(&registry).unwrap();
            assert_eq!(b.read().unwrap().b, 4.2);
            assert_eq!(b.read().unwrap().a.a, 42);

            let c = b.downcast::<C>(&registry).unwrap();
            assert!(c.read().unwrap().c);
            assert_eq!(c.read().unwrap().b.b, 4.2);
            assert_eq!(c.read().unwrap().b.a.a, 42);
        }

        {
            let c = ObjectLazy::new(data.lazy());
            assert_eq!(c.current_type_hash(), TypeHash::of::<C>());
            assert_eq!(c.actual_type_hash(), TypeHash::of::<C>());
            assert!(c.read().unwrap().c);
            assert_eq!(c.read().unwrap().b.b, 4.2);
            assert_eq!(c.read().unwrap().b.a.a, 42);

            let b = c.clone().upcast::<B>(&registry).unwrap();
            assert_eq!(b.current_type_hash(), TypeHash::of::<B>());
            assert_eq!(b.actual_type_hash(), TypeHash::of::<C>());
            assert_eq!(b.read().unwrap().b, 4.2);
            assert_eq!(b.read().unwrap().a.a, 42);

            let a = b.clone().upcast::<A>(&registry).unwrap();
            assert_eq!(a.current_type_hash(), TypeHash::of::<A>());
            assert_eq!(a.actual_type_hash(), TypeHash::of::<C>());
            assert_eq!(a.read().unwrap().a, 42);

            let a = c.clone().upcast::<A>(&registry).unwrap();
            assert_eq!(a.current_type_hash(), TypeHash::of::<A>());
            assert_eq!(a.actual_type_hash(), TypeHash::of::<C>());
            assert_eq!(a.read().unwrap().a, 42);

            let b = a.clone().downcast::<B>(&registry).unwrap();
            assert_eq!(b.read().unwrap().b, 4.2);
            assert_eq!(b.read().unwrap().a.a, 42);

            let c = a.clone().downcast::<C>(&registry).unwrap();
            assert!(c.read().unwrap().c);
            assert_eq!(c.read().unwrap().b.b, 4.2);
            assert_eq!(c.read().unwrap().b.a.a, 42);

            let c = b.clone().downcast::<C>(&registry).unwrap();
            assert!(c.read().unwrap().c);
            assert_eq!(c.read().unwrap().b.b, 4.2);
            assert_eq!(c.read().unwrap().b.a.a, 42);
        }

        let mut data = Managed::new(B {
            a: A { a: 42 },
            b: 4.2,
        });

        {
            let b = ObjectLazy::new(data.lazy());
            assert_eq!(b.read().unwrap().b, 4.2);
            assert_eq!(b.read().unwrap().a.a, 42);

            let a = b.upcast::<A>(&registry).unwrap();
            assert_eq!(a.read().unwrap().a, 42);
            assert!(a.clone().downcast::<C>(&registry).is_none());

            let b = a.downcast::<B>(&registry).unwrap();
            assert_eq!(b.read().unwrap().b, 4.2);
            assert_eq!(b.read().unwrap().a.a, 42);
            assert!(b.clone().downcast::<C>(&registry).is_none());
        }

        let mut data = Managed::new(A { a: 42 });

        {
            let a = ObjectLazy::new(data.lazy());
            assert_eq!(a.read().unwrap().a, 42);
            assert!(a.clone().downcast::<B>(&registry).is_none());
            assert!(a.clone().downcast::<C>(&registry).is_none());
        }
    }
}
