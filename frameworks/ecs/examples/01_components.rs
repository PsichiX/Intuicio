use intuicio_framework_ecs::prelude::*;
use std::error::Error;

#[derive(Debug, Default, Clone, Copy)]
struct Health(pub usize);

#[derive(Debug, Default, Clone, Copy)]
struct Strength(pub usize);

fn main() -> Result<(), Box<dyn Error>> {
    // Create the universe.
    let mut universe = Universe::default();

    // Create heroes.
    let arthas = universe.simulation.spawn((Health(100), Strength(20)))?;

    let lyra = universe.simulation.spawn((Health(120), Strength(15)))?;

    // Arthas buffs himself to get stronger.
    let attack = {
        let mut strength = universe
            .simulation
            .component_mut::<true, Strength>(arthas)?;
        strength.0 += 5;
        strength.0
    };

    // Lyra takes damage.
    let mut health = universe.simulation.component_mut::<true, Health>(lyra)?;
    health.0 = health.0.saturating_sub(attack);

    Ok(())
}
