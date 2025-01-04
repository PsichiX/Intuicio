use intuicio_framework_ecs::prelude::*;
use std::error::Error;

#[derive(Debug, Default, Clone, Copy)]
struct Gold(pub usize);

#[derive(Debug, Default, Clone, Copy)]
struct Food(pub usize);

#[derive(Debug, Default, Clone, Copy)]
struct Heat(pub usize);

fn main() -> Result<(), Box<dyn Error>> {
    struct MyPlugin;
    let plugin = GraphSchedulerQuickPlugin::<true, MyPlugin>::default()
        .resource(Gold(1000))
        .resource(Food(500))
        .resource(Heat(20))
        .group("summer", (), |group| {
            group.system(generate_income, "generate_income", ()).system(
                harvest_food,
                "harvest_food",
                (),
            )
        })
        .group("winter", (), |group| {
            group.system(consume_food, "consume_food", ()).system(
                increase_heat,
                "increase_heat",
                (),
            )
        })
        .commit();

    let mut universe = Universe::default().with_plugin(plugin);
    // Create a scheduler instance that will run universe systems.
    let mut scheduler = GraphScheduler::<true>::default();

    // Perform single frame universe systems run.
    scheduler.run(&mut universe)?;

    Ok(())
}

fn generate_income(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let mut gold = context.fetch::<Res<true, &mut Gold>>()?;

    gold.0 += 200;
    println!("Income generated during summer. Gold now: {}", gold.0);

    Ok(())
}

fn harvest_food(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let mut food = context.fetch::<Res<true, &mut Food>>()?;

    food.0 += 100;
    println!("Food harvested. Food now: {}", food.0);

    Ok(())
}

fn consume_food(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let mut food = context.fetch::<Res<true, &mut Food>>()?;

    if food.0 >= 50 {
        food.0 -= 50;
        println!("Food consumed. Winter survived!");
    } else {
        println!("Not enough food to survive the winter!")
    }

    Ok(())
}

fn increase_heat(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let mut heat = context.fetch::<Res<true, &mut Heat>>()?;

    heat.0 += 10;
    println!("Heat increased. Heat now: {}", heat.0);

    Ok(())
}
