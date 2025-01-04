use intuicio_framework_ecs::prelude::*;
use std::error::Error;

#[derive(Debug, Default, Clone, Copy)]
struct Health(pub usize);

#[derive(Debug, Default, Clone, Copy)]
struct Strength(pub usize);

#[derive(Debug, Default, Clone, Copy)]
struct Gold(pub usize);

#[derive(Debug, Default, Clone, Copy)]
struct Food(pub usize);

fn main() -> Result<(), Box<dyn Error>> {
    let mut universe = Universe::default();

    // Setup Warriors.
    universe.simulation.spawn((Health(50), Strength(10)))?;
    universe.simulation.spawn((Health(90), Strength(20)))?;

    // Setup Villagers.
    universe.simulation.spawn((Gold(200), Food(100)))?;
    universe.simulation.spawn((Gold(50), Food(200)))?;

    // Heal weakened Warriors.
    for (health, strength) in universe
        .simulation
        .query::<true, (&mut Health, &Strength)>()
    {
        if strength.0 < 15 {
            println!("Healing weakened Warrior. Initial health: {}", health.0);
            health.0 = 100;
        }
    }

    // Villagers perform transactions: buying food for gold.
    for (gold, food) in universe.simulation.query::<true, (&mut Gold, &mut Food)>() {
        if gold.0 >= 50 {
            gold.0 -= 50;
            food.0 += 100;
            println!(
                "Villager traded gold for food. Now having: Gold = {} and Food = {}",
                gold.0, food.0
            );
        }
    }

    Ok(())
}
