use crate::registry::Registry;
use intuicio_data::{lifetime::*, managed::*, shared::*};
use std::{
    cell::{Ref, RefMut},
    marker::PhantomData,
};

pub trait ValueTransformer {
    type Type;
    type Borrow<'r>
    where
        Self::Type: 'r;
    type BorrowMut<'r>
    where
        Self::Type: 'r;
    type Dependency;
    type Owned;
    type Ref;
    type RefMut;

    fn from_owned(registry: &Registry, value: Self::Type) -> Self::Owned;
    fn from_ref(
        registry: &Registry,
        value: &Self::Type,
        dependency: Option<Self::Dependency>,
    ) -> Self::Ref;
    fn from_ref_mut(
        registry: &Registry,
        value: &mut Self::Type,
        dependency: Option<Self::Dependency>,
    ) -> Self::RefMut;
    fn into_owned(value: Self::Owned) -> Self::Type;
    fn into_ref(value: &Self::Ref) -> Self::Borrow<'_>;
    fn into_ref_mut(value: &mut Self::RefMut) -> Self::BorrowMut<'_>;
}

pub trait ValueDependency<T> {
    fn as_ref(value: &T) -> Self;
    fn as_ref_mut(value: &mut T) -> Self;
}

pub struct SharedValueTransformer<T: Default + Clone + 'static>(PhantomData<fn() -> T>);

impl<T: Default + Clone + 'static> ValueTransformer for SharedValueTransformer<T> {
    type Type = T;
    type Borrow<'r> = Ref<'r, T>;
    type BorrowMut<'r> = RefMut<'r, T>;
    type Dependency = ();
    type Owned = Shared<T>;
    type Ref = Shared<T>;
    type RefMut = Shared<T>;

    fn from_owned(_: &Registry, value: Self::Type) -> Self::Owned {
        Shared::new(value)
    }

    fn from_ref(_: &Registry, value: &Self::Type, _: Option<Self::Dependency>) -> Self::Ref {
        Shared::new(value.clone())
    }

    fn from_ref_mut(
        _: &Registry,
        value: &mut Self::Type,
        _: Option<Self::Dependency>,
    ) -> Self::RefMut {
        Shared::new(value.clone())
    }

    fn into_owned(value: Self::Owned) -> Self::Type {
        value.try_consume().ok().unwrap()
    }

    fn into_ref(value: &Self::Ref) -> Self::Borrow<'_> {
        value.read().unwrap()
    }

    fn into_ref_mut(value: &mut Self::RefMut) -> Self::BorrowMut<'_> {
        value.write().unwrap()
    }
}

pub struct ManagedValueTransformer<T>(PhantomData<fn() -> T>);

impl<T> ValueTransformer for ManagedValueTransformer<T> {
    type Type = T;
    type Borrow<'r> = ValueReadAccess<'r, T> where Self::Type: 'r;
    type BorrowMut<'r> = ValueWriteAccess<'r, T> where Self::Type: 'r;
    type Dependency = ManagedValueDependency;
    type Owned = Managed<T>;
    type Ref = ManagedRef<T>;
    type RefMut = ManagedRefMut<T>;

    fn from_owned(_: &Registry, value: Self::Type) -> Self::Owned {
        Managed::new(value)
    }

    fn from_ref(
        _: &Registry,
        value: &Self::Type,
        dependency: Option<Self::Dependency>,
    ) -> Self::Ref {
        if let ManagedValueDependency::Ref(lifetime) =
            dependency.expect("`ManagedRef` require dependency for lifetime bound!")
        {
            ManagedRef::new(value, lifetime)
        } else {
            panic!("Could not borrow lifetime to create `ManagedRef`!")
        }
    }

    fn from_ref_mut(
        _: &Registry,
        value: &mut Self::Type,
        dependency: Option<Self::Dependency>,
    ) -> Self::RefMut {
        if let ManagedValueDependency::RefMut(lifetime) =
            dependency.expect("`ManagedRefMut` require dependency for lifetime bound!")
        {
            ManagedRefMut::new(value, lifetime)
        } else {
            panic!("Could not borrow lifetime mutably to create `ManagedRefMut`!")
        }
    }

    fn into_owned(value: Self::Owned) -> Self::Type {
        value.consume().ok().unwrap()
    }

    fn into_ref(value: &Self::Ref) -> Self::Borrow<'_> {
        value.read().unwrap()
    }

    fn into_ref_mut(value: &mut Self::RefMut) -> Self::BorrowMut<'_> {
        value.write().unwrap()
    }
}

pub struct DynamicManagedValueTransformer<T: 'static>(PhantomData<fn() -> T>);

impl<T: 'static> ValueTransformer for DynamicManagedValueTransformer<T> {
    type Type = T;
    type Borrow<'r> = ValueReadAccess<'r, T> where Self::Type: 'r;
    type BorrowMut<'r> = ValueWriteAccess<'r, T> where Self::Type: 'r;
    type Dependency = ManagedValueDependency;
    type Owned = DynamicManaged;
    type Ref = DynamicManagedRef;
    type RefMut = DynamicManagedRefMut;

    fn from_owned(_: &Registry, value: Self::Type) -> Self::Owned {
        DynamicManaged::new(value).ok().unwrap()
    }

    fn from_ref(
        _: &Registry,
        value: &Self::Type,
        dependency: Option<Self::Dependency>,
    ) -> Self::Ref {
        if let ManagedValueDependency::Ref(lifetime) =
            dependency.expect("`DynamicManagedRef` require dependency for lifetime bound!")
        {
            DynamicManagedRef::new(value, lifetime)
        } else {
            panic!("Could not borrow lifetime to create `DynamicManagedRef`!")
        }
    }

    fn from_ref_mut(
        _: &Registry,
        value: &mut Self::Type,
        dependency: Option<Self::Dependency>,
    ) -> Self::RefMut {
        if let ManagedValueDependency::RefMut(lifetime) =
            dependency.expect("`DynamicManagedRefMut` require dependency for lifetime bound!")
        {
            DynamicManagedRefMut::new(value, lifetime)
        } else {
            panic!("Could not borrow lifetime mutably to create `DynamicManagedRefMut`!")
        }
    }

    fn into_owned(value: Self::Owned) -> Self::Type {
        value.consume().ok().unwrap()
    }

    fn into_ref(value: &Self::Ref) -> Self::Borrow<'_> {
        value.read().unwrap()
    }

    fn into_ref_mut(value: &mut Self::RefMut) -> Self::BorrowMut<'_> {
        value.write().unwrap()
    }
}

pub enum ManagedValueDependency {
    Ref(LifetimeRef),
    RefMut(LifetimeRefMut),
}

impl<T> ValueDependency<Managed<T>> for ManagedValueDependency {
    fn as_ref(value: &Managed<T>) -> Self {
        Self::Ref(value.lifetime().borrow().unwrap())
    }

    fn as_ref_mut(value: &mut Managed<T>) -> Self {
        Self::RefMut(value.lifetime().borrow_mut().unwrap())
    }
}

impl<T> ValueDependency<ManagedRef<T>> for ManagedValueDependency {
    fn as_ref(value: &ManagedRef<T>) -> Self {
        Self::Ref(value.lifetime().borrow().unwrap())
    }

    fn as_ref_mut(_: &mut ManagedRef<T>) -> Self {
        panic!("Cannot borrow lifetime mutably from `ManagedRef`!");
    }
}

impl<T> ValueDependency<ManagedRefMut<T>> for ManagedValueDependency {
    fn as_ref(value: &ManagedRefMut<T>) -> Self {
        Self::Ref(value.lifetime().borrow().unwrap())
    }

    fn as_ref_mut(value: &mut ManagedRefMut<T>) -> Self {
        Self::RefMut(value.lifetime().borrow_mut().unwrap())
    }
}

impl ValueDependency<DynamicManaged> for ManagedValueDependency {
    fn as_ref(value: &DynamicManaged) -> Self {
        Self::Ref(value.lifetime().borrow().unwrap())
    }

    fn as_ref_mut(value: &mut DynamicManaged) -> Self {
        Self::RefMut(value.lifetime().borrow_mut().unwrap())
    }
}

impl ValueDependency<DynamicManagedRef> for ManagedValueDependency {
    fn as_ref(value: &DynamicManagedRef) -> Self {
        Self::Ref(value.lifetime().borrow().unwrap())
    }

    fn as_ref_mut(_: &mut DynamicManagedRef) -> Self {
        panic!("Cannot borrow lifetime mutably from `DynamicManagedRef`!");
    }
}

impl ValueDependency<DynamicManagedRefMut> for ManagedValueDependency {
    fn as_ref(value: &DynamicManagedRefMut) -> Self {
        Self::Ref(value.lifetime().borrow().unwrap())
    }

    fn as_ref_mut(value: &mut DynamicManagedRefMut) -> Self {
        Self::RefMut(value.lifetime().borrow_mut().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as intuicio_core;
    use crate::prelude::*;
    use intuicio_derive::*;

    #[intuicio_function(transformer = "ManagedValueTransformer")]
    fn add(a: &i32, b: &mut i32) -> i32 {
        *a + *b
    }

    #[intuicio_function(transformer = "DynamicManagedValueTransformer")]
    fn sub(a: &i32, b: &mut i32) -> i32 {
        *a - *b
    }

    #[derive(IntuicioStruct, Default, Clone)]
    #[intuicio(name = "Foo")]
    struct Foo {
        bar: i32,
    }

    #[intuicio_methods()]
    impl Foo {
        #[intuicio_method(transformer = "ManagedValueTransformer")]
        fn new(bar: i32) -> Foo {
            Foo { bar }
        }

        #[intuicio_method(transformer = "ManagedValueTransformer", dependency = "foo")]
        fn get(foo: &Foo) -> &i32 {
            &foo.bar
        }
    }

    #[derive(IntuicioStruct, Debug, Default, Clone)]
    #[intuicio(name = "Bar")]
    struct Bar {
        foo: i32,
    }

    #[intuicio_methods()]
    impl Bar {
        #[intuicio_method(transformer = "DynamicManagedValueTransformer")]
        fn new(foo: i32) -> Bar {
            Bar { foo }
        }

        #[intuicio_method(transformer = "DynamicManagedValueTransformer", dependency = "bar")]
        fn get(bar: &Bar) -> &i32 {
            &bar.foo
        }
    }

    #[test]
    fn test_derive() {
        let mut registry = Registry::default().with_basic_types();
        registry.add_type(define_native_struct! {
            registry => struct (Managed<i32>) {}
        });
        registry.add_type(define_native_struct! {
            registry => struct (DynamicManaged) {}
            [uninitialized]
        });
        registry.add_type(Foo::define_struct(&registry));
        registry.add_function(Foo::new__define_function(&registry));
        registry.add_type(Bar::define_struct(&registry));
        registry.add_function(Bar::new__define_function(&registry));
        let mut context = Context::new(10240, 10240);

        let a = Managed::new(40);
        let mut b = Managed::new(2);
        context.stack().push(b.borrow_mut().unwrap());
        context.stack().push(a.borrow().unwrap());
        add::intuicio_function(&mut context, &registry);
        assert_eq!(
            context
                .stack()
                .pop::<Managed<i32>>()
                .unwrap()
                .consume()
                .ok()
                .unwrap(),
            42
        );

        let a = DynamicManaged::new(40).unwrap();
        let mut b = DynamicManaged::new(2).unwrap();
        context.stack().push(b.borrow_mut().unwrap());
        context.stack().push(a.borrow().unwrap());
        sub::intuicio_function(&mut context, &registry);
        assert_eq!(
            context
                .stack()
                .pop::<DynamicManaged>()
                .unwrap()
                .consume::<i32>()
                .ok()
                .unwrap(),
            38
        );

        let foo = Managed::new(Foo::new(42));
        context.stack().push(foo.borrow().unwrap());
        Foo::get__intuicio_function(&mut context, &registry);
        assert_eq!(
            *context
                .stack()
                .pop::<ManagedRef<i32>>()
                .unwrap()
                .read()
                .unwrap(),
            42
        );

        let bar = DynamicManaged::new(Bar::new(42)).unwrap();
        context.stack().push(bar.borrow().unwrap());
        Bar::get__intuicio_function(&mut context, &registry);
        assert_eq!(
            *context
                .stack()
                .pop::<DynamicManagedRef>()
                .unwrap()
                .read::<i32>()
                .unwrap(),
            42
        );
    }

    #[test]
    fn test_shared_value_transformer() {
        fn add_wrapped(
            a: <SharedValueTransformer<i32> as ValueTransformer>::Ref,
            mut b: <SharedValueTransformer<i32> as ValueTransformer>::RefMut,
        ) -> <SharedValueTransformer<i32> as ValueTransformer>::Owned {
            let a = SharedValueTransformer::into_ref(&a);
            let mut b = SharedValueTransformer::into_ref_mut(&mut b);
            let result = {
                let a = &a;
                let b = &mut b;
                add(a, b)
            };
            let registry = Registry::default();
            SharedValueTransformer::from_owned(&registry, result)
        }

        assert_eq!(
            add(&40, &mut 2),
            *add_wrapped(Shared::new(40), Shared::new(2)).read().unwrap(),
        );

        fn get_wrapped(
            foo: <SharedValueTransformer<Foo> as ValueTransformer>::Ref,
        ) -> <SharedValueTransformer<i32> as ValueTransformer>::Ref {
            let foo = SharedValueTransformer::into_ref(&foo);
            let result = {
                let foo = &foo;
                Foo::get(foo)
            };
            let registry = Registry::default();
            SharedValueTransformer::from_ref(&registry, result, None)
        }

        let foo = Shared::new(Foo { bar: 42 });
        let a = *Foo::get(&foo.read().unwrap());
        let b = *get_wrapped(foo.clone()).read().unwrap();
        assert_eq!(a, b,);
    }

    #[test]
    fn test_managed_value_transformer() {
        fn add_wrapped(
            a: <ManagedValueTransformer<i32> as ValueTransformer>::Ref,
            mut b: <ManagedValueTransformer<i32> as ValueTransformer>::RefMut,
        ) -> <ManagedValueTransformer<i32> as ValueTransformer>::Owned {
            let a = ManagedValueTransformer::into_ref(&a);
            let mut b = ManagedValueTransformer::into_ref_mut(&mut b);
            let result = {
                let a = &a;
                let b = &mut b;
                add(a, b)
            };
            let registry = Registry::default();
            ManagedValueTransformer::from_owned(&registry, result)
        }

        let a = Managed::new(40);
        let mut b = Managed::new(2);
        assert_eq!(
            add(&40, &mut 2),
            *add_wrapped(a.borrow().unwrap(), b.borrow_mut().unwrap())
                .read()
                .unwrap(),
        );

        fn get_wrapped(
            foo: <ManagedValueTransformer<Foo> as ValueTransformer>::Ref,
        ) -> <ManagedValueTransformer<i32> as ValueTransformer>::Ref {
            let dependency =
                Some(<ManagedValueTransformer<i32> as ValueTransformer>::Dependency::as_ref(&foo));
            let foo = ManagedValueTransformer::into_ref(&foo);
            let result = {
                let foo = &foo;
                Foo::get(foo)
            };
            let registry = Registry::default();
            ManagedValueTransformer::from_ref(&registry, result, dependency)
        }

        let foo = Managed::new(Foo { bar: 42 });
        let a = *Foo::get(&foo.read().unwrap());
        let b = *get_wrapped(foo.borrow().unwrap()).read().unwrap();
        assert_eq!(a, b,);
    }
}
