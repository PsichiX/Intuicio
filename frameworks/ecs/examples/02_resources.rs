use intuicio_framework_ecs::prelude::*;
use std::error::Error;

#[derive(Debug, Default, Clone, Copy)]
struct Gold(pub usize);

#[derive(Debug, Default, Clone, Copy)]
struct Food(pub usize);

fn main() -> Result<(), Box<dyn Error>> {
    // Plugins serve as units that setup universe and automatically handle lifetime of installed things.
    struct MyPlugin;
    let plugin = GraphSchedulerQuickPlugin::<true, MyPlugin>::default()
        // Village treasury.
        .resource(Gold(1000))
        // Village food supply.
        .resource(Food(500))
        .commit();

    let universe = Universe::default().with_plugin(plugin);

    // A feast is held and villagers are consuming food.
    {
        let mut food = universe.resources.get_mut::<true, Food>()?;
        food.0 = food.0.saturating_sub(200);
    }

    // Trader arrives and sells more food.
    {
        let mut gold = universe.resources.get_mut::<true, Gold>()?;
        let mut food = universe.resources.get_mut::<true, Food>()?;
        gold.0 = gold.0.saturating_sub(500);
        food.0 += 300;
    }

    Ok(())
}
