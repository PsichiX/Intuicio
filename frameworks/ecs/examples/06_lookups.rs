use intuicio_framework_ecs::prelude::*;
use std::error::Error;

#[derive(Debug, Default, Clone, Copy)]
struct Health(pub usize);

#[derive(Debug, Default, Clone, Copy)]
struct Soldier;

#[derive(Debug, Default, Clone, Copy)]
struct Commander;

#[derive(Debug, Default, Clone, Copy)]
struct Commands;

fn main() -> Result<(), Box<dyn Error>> {
    let mut universe = Universe::default();

    // Setup soldiers.
    let commander = universe
        .simulation
        .spawn((Commander, Health(200), "Commander"))?;
    let soldier_1 = universe
        .simulation
        .spawn((Soldier, Health(50), "Soldier 1"))?;
    let soldier_2 = universe
        .simulation
        .spawn((Soldier, Health(30), "Soldier 2"))?;
    let soldier_3 = universe
        .simulation
        .spawn((Soldier, Health(70), "Soldier 3"))?;

    // Setup relations.
    universe
        .simulation
        .relate::<true, _>(Commands, commander, soldier_1)?;
    universe
        .simulation
        .relate::<true, _>(Commands, commander, soldier_2)?;
    universe
        .simulation
        .relate::<true, _>(Commands, commander, soldier_3)?;

    // List commanded troops.
    let troops = universe
        .simulation
        .relations_outgoing::<true, Commands>(commander)
        .map(|(_, _, entity)| entity);

    // Heal commanded soldiers.
    for (health, name) in universe
        .simulation
        .lookup::<true, (&mut Health, &&'static str)>(troops)
    {
        if health.0 < 60 {
            health.0 += 20;
            println!("Healed solder {:?} to {} health", name, health.0);
        }
    }

    Ok(())
}
