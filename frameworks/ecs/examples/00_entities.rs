use intuicio_framework_ecs::{bundle::DynamicBundle, prelude::*};
use std::error::Error;

#[derive(Debug)]
struct Health;

#[derive(Debug)]
struct Strength;

#[derive(Debug)]
struct Mana;

fn main() -> Result<(), Box<dyn Error>> {
    // Create the universe.
    let mut universe = Universe::default();

    // Spawn static bundle.
    let arthas = universe.simulation.spawn((Health, Strength))?;

    // Spawn dynamic bundle.
    let bundle = DynamicBundle::default()
        .with_component(Health)
        .unwrap()
        .with_component(Strength)
        .unwrap();
    let lyra = universe.simulation.spawn(bundle)?;

    // Insert additional components to existing entity.
    universe.simulation.insert(arthas, (Mana,))?;

    // Remove components from entity.
    universe.simulation.remove::<(Strength, Health)>(arthas)?;

    // Despawn entities.
    universe.simulation.despawn(arthas)?;
    universe.simulation.despawn(lyra)?;

    Ok(())
}
