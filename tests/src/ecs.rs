use intuicio_core::prelude::*;
use intuicio_data::{
    lifetime::Lifetime,
    managed::{ManagedRef, ManagedRefMut},
};
use intuicio_derive::*;

#[derive(IntuicioStruct, Default)]
struct Health {
    value: usize,
}

#[derive(IntuicioStruct, Default)]
struct Damage {
    value: usize,
}

#[intuicio_function(module_name = "test", transformer = "ManagedValueTransformer")]
fn deal_damage(health: &mut Health, damage: &Damage) {
    health.value = health.value.saturating_sub(damage.value);
}

#[test]
fn test_hecs() {
    use hecs::World;

    let mut context = Context::new(10240, 10240);
    let mut registry = Registry::default().with_basic_types();

    registry.add_type(NativeStructBuilder::new_uninitialized::<ManagedRefMut<Health>>().build());
    registry.add_type(NativeStructBuilder::new_uninitialized::<ManagedRef<Damage>>().build());

    let deal_damage = registry.add_function(deal_damage::define_function(&registry));

    let mut world = World::new();
    let a = world.spawn((Health { value: 100 }, Damage { value: 42 }));
    let b = world.spawn((Health { value: 100 },));
    world.spawn((Damage { value: 42 },));

    assert_eq!(world.get::<&Health>(a).unwrap().value, 100);
    assert_eq!(world.get::<&Health>(b).unwrap().value, 100);

    for (_, (health, damage)) in world.query::<(&mut Health, &Damage)>().iter() {
        deal_damage.call::<(), _>(
            &mut context,
            &registry,
            (ManagedRefMut::make(health).0, ManagedRef::make(damage).0),
            false,
        );
    }

    assert_eq!(world.get::<&Health>(a).unwrap().value, 58);
    assert_eq!(world.get::<&Health>(b).unwrap().value, 100);
}

#[test]
fn test_ecs() {
    use intuicio_framework_ecs::prelude::*;

    let mut context = Context::new(10240, 10240);
    let mut registry = Registry::default().with_basic_types();

    registry.add_type(NativeStructBuilder::new_uninitialized::<ManagedRefMut<Health>>().build());
    registry.add_type(NativeStructBuilder::new_uninitialized::<ManagedRef<Damage>>().build());

    let deal_damage = registry.add_function(deal_damage::define_function(&registry));

    let mut world = World::default();
    let a = world
        .spawn((Health { value: 100 }, Damage { value: 42 }))
        .unwrap();
    let b = world.spawn((Health { value: 100 },)).unwrap();
    world.spawn((Damage { value: 42 },)).unwrap();

    assert_eq!(
        world
            .get::<true, Health>(a, false)
            .unwrap()
            .read()
            .unwrap()
            .value,
        100
    );
    assert_eq!(
        world
            .get::<true, Health>(b, false)
            .unwrap()
            .read()
            .unwrap()
            .value,
        100
    );

    for (health, damage) in world.query::<true, (&mut Health, &Damage)>() {
        deal_damage.call::<(), _>(
            &mut context,
            &registry,
            (ManagedRefMut::make(health).0, ManagedRef::make(damage).0),
            false,
        );
    }

    assert_eq!(
        world
            .get::<true, Health>(a, false)
            .unwrap()
            .read()
            .unwrap()
            .value,
        58
    );
    assert_eq!(
        world
            .get::<true, Health>(b, false)
            .unwrap()
            .read()
            .unwrap()
            .value,
        100
    );
}
