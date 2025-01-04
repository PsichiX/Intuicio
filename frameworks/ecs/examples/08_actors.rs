use intuicio_framework_ecs::{actor::Actor, prelude::*};
use std::error::Error;

#[derive(Debug, Default, Clone, Copy)]
struct Gold(pub usize);

fn main() -> Result<(), Box<dyn Error>> {
    let mut universe = Universe::default();

    // Setup capital city.
    let eldoria = Actor::spawn(&mut universe.simulation, ("Eldoria", Gold(1000)))?;
    // Setup dependent cities.
    let thalnar = Actor::spawn(&mut universe.simulation, ("Thalnar", Gold(230)))?;
    let virella = Actor::spawn(&mut universe.simulation, ("Virella", Gold(50)))?;

    // Setup dependent cities relations to capital.
    eldoria.add_child::<true>(&mut universe.simulation, thalnar)?;
    eldoria.add_child::<true>(&mut universe.simulation, virella)?;

    // Pay taxes.
    {
        let mut collected_money = 0;
        for city in eldoria.children::<true>(&universe.simulation) {
            let mut gold = city.component_mut::<true, Gold>(&universe.simulation)?;
            let tax = gold.0 / 10;
            collected_money += tax;
            gold.0 -= tax;
        }
        let mut gold = eldoria.component_mut::<true, Gold>(&universe.simulation)?;
        gold.0 += collected_money;
    }

    // Show cities treasury report.
    for (name, gold) in universe.simulation.query::<true, (&&'static str, &Gold)>() {
        println!("City {:?} has {} gold", name, gold.0);
    }

    Ok(())
}
