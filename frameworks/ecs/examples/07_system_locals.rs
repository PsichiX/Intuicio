use intuicio_framework_ecs::prelude::*;
use std::error::Error;

#[derive(Debug, Default, Clone, Copy)]
struct XP(pub usize);

#[derive(Debug, Default, Clone, Copy)]
struct Level(pub usize);

#[derive(Debug, Default, Clone, Copy)]
struct Boost(pub usize);

fn main() -> Result<(), Box<dyn Error>> {
    struct MyPlugin;
    let plugin = GraphSchedulerQuickPlugin::<true, MyPlugin>::default()
        .system(training, "training", (Boost(1),))
        .system(report, "report", ())
        .commit();

    let mut universe = Universe::default().with_plugin(plugin);
    let mut scheduler = GraphScheduler::<true>::default();

    // Setup heroes.
    universe.simulation.spawn((XP(5), Level(1)))?;
    universe.simulation.spawn((XP(45), Level(1)))?;

    // Run 10 frames of simulation.
    for _ in 0..10 {
        scheduler.run(&mut universe)?;
    }

    Ok(())
}

fn training(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (world, mut boost, hero_query) = context.fetch::<(
        // Fetching simulation World allows for later queries.
        &World,
        // Locals are special resources bound to specific systems, a persistent state.
        Local<true, &mut Boost>,
        // Query to run on simulation World.
        Query<true, (&mut XP, &mut Level)>,
    )>()?;

    let mut increase = 0;

    for (xp, level) in hero_query.query(world) {
        // add current hero progress to boost increase.
        increase += xp.0 * level.0;
        // increase XP by current boost.
        xp.0 += boost.0;
        // level up when XP exceeds a threshold.
        while xp.0 >= 100 {
            xp.0 -= 100;
            level.0 += 1;
        }
    }

    boost.0 += increase;
    println!(
        "Boost applied this round: {}. Total Boost: {}",
        increase, boost.0
    );

    Ok(())
}

fn report(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (world, hero_query) = context.fetch::<(&World, Query<true, (&XP, &Level)>)>()?;

    println!("Heroes report:");
    for (xp, level) in hero_query.query(world) {
        println!("Hero | XP: {}, Level: {}", xp.0, level.0);
    }

    Ok(())
}
