use intuicio_framework_ecs::prelude::*;
use std::error::Error;

#[derive(Debug, Default, Clone, Copy)]
struct Gold(pub usize);

#[derive(Debug, Default, Clone, Copy)]
struct Food(pub usize);

fn main() -> Result<(), Box<dyn Error>> {
    struct MyPlugin;
    let plugin = GraphSchedulerQuickPlugin::<true, MyPlugin>::default()
        .resource(Gold(1000))
        .resource(Food(500))
        .commit();

    let universe = Universe::default().with_plugin(plugin);

    // Calling `Systems::run_one_shot` allows to execute specific system in-place.
    // Useful in cases where system doesn't need to be part of continous game loop.
    Systems::run_one_shot::<true>(&universe, add_daily_income)?;
    Systems::run_one_shot::<true>(&universe, feast)?;
    Systems::run_one_shot::<true>(&universe, village_report)?;

    Ok(())
}

fn add_daily_income(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let mut gold = context.fetch::<Res<true, &mut Gold>>()?;

    gold.0 += 100;

    Ok(())
}

fn feast(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let mut food = context.fetch::<Res<true, &mut Food>>()?;

    food.0 = food.0.saturating_sub(50);

    Ok(())
}

fn village_report(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (gold, food) = context.fetch::<(Res<true, &Gold>, Res<true, &Food>)>()?;

    println!("Gold: {} | Food: {}", gold.0, food.0);

    Ok(())
}
