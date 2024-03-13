use hecs::World;
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
fn test_ecs() {
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
        let health_lifetime = Lifetime::default();
        let damage_lifetime = Lifetime::default();

        deal_damage.call::<(), _>(
            &mut context,
            &registry,
            (
                ManagedRefMut::new(health, health_lifetime.borrow_mut().unwrap()),
                ManagedRef::new(damage, damage_lifetime.borrow().unwrap()),
            ),
            false,
        );
    }

    assert_eq!(world.get::<&Health>(a).unwrap().value, 58);
    assert_eq!(world.get::<&Health>(b).unwrap().value, 100);
}
