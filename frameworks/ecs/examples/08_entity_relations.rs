use intuicio_framework_ecs::prelude::*;
use std::error::Error;

#[derive(Debug, Default, Clone, Copy)]
struct Health(pub usize);

#[derive(Debug, Default, Clone, Copy)]
struct Strength(pub usize);

#[derive(Debug, Default, Clone, Copy)]
struct Hero;

#[derive(Debug, Default, Clone, Copy)]
struct Monster;

#[derive(Debug, Default, Clone, Copy)]
struct Pet;

fn main() -> Result<(), Box<dyn Error>> {
    struct MyPlugin;
    let plugin = GraphSchedulerQuickPlugin::<true, MyPlugin>::default()
        .system(attack, "attack", ())
        .system(report_alive, "report_alive", ())
        .commit();

    let mut universe = Universe::default().with_plugin(plugin);
    let mut scheduler = GraphScheduler::<true>::default();

    // Setup hero and monsters.
    let hero = universe.simulation.spawn((Hero,))?;
    let wolf = universe
        .simulation
        .spawn((Monster, "wolf", Health(50), Strength(70)))?;
    let rabbit = universe
        .simulation
        .spawn((Monster, "rabbit", Health(20), Strength(30)))?;
    universe
        .simulation
        .spawn((Monster, "bear", Health(100), Strength(120)))?;
    universe
        .simulation
        .spawn((Monster, "lizard", Health(10), Strength(40)))?;

    // Setup pet relations.
    universe.simulation.relate::<true, _>(Pet, wolf, hero)?;
    universe.simulation.relate::<true, _>(Pet, rabbit, hero)?;

    scheduler.run(&mut universe)?;

    Ok(())
}

fn attack(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (world, pet_monster_query, wild_monster_query) = context.fetch::<(
        &World,
        Query<
            true,
            (
                &&'static str,
                &mut Health,
                &Strength,
                Include<Monster>,
                Include<Relation<Pet>>,
            ),
        >,
        Query<
            true,
            (
                &&'static str,
                &mut Health,
                &Strength,
                Include<Monster>,
                Exclude<Relation<Pet>>,
            ),
        >,
    )>()?;

    for (pet_name, pet_health, pet_strength, _, _) in pet_monster_query.query(world) {
        for (wild_name, wild_health, wild_strength, _, _) in wild_monster_query.query(world) {
            if pet_health.0 == 0 || wild_health.0 == 0 {
                continue;
            }
            wild_health.0 = wild_health.0.saturating_sub(pet_strength.0);
            pet_health.0 = pet_health.0.saturating_sub(wild_strength.0);
            println!(
                "Pet {:?} and {:?} monster exchanged attacks. Pet health: {}, monster health: {}",
                pet_name, wild_name, pet_health.0, wild_health.0
            );
        }
    }

    Ok(())
}

fn report_alive(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (world, monster_query) =
        context.fetch::<(&World, Query<true, (&&'static str, &Health)>)>()?;

    let mut num_alive = 0;
    for (name, health) in monster_query.query(world) {
        println!("{:?} monster has {} health", name, health.0);

        if health.0 > 0 {
            num_alive += 1;
        }
    }

    if num_alive == 0 {
        println!("Dear Gods, what a bloodbath...");
    }

    Ok(())
}
