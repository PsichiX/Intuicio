use intuicio_framework_ecs::{observer::ChangeObserver, prelude::*};
use rand::{thread_rng, Rng};
use std::error::Error;

#[derive(Debug, Default, Clone, Copy)]
struct Temperature(pub isize);

#[derive(Debug, Default, Clone, Copy)]
struct Heat;

#[derive(Debug, Default, Clone, Copy)]
struct Cold;

fn main() -> Result<(), Box<dyn Error>> {
    struct MyPlugin;
    let plugin = GraphSchedulerQuickPlugin::<true, MyPlugin>::default()
        .resource(CommandBuffer::default())
        .system(spawn_temperature_change, "spawn_temperature_change", ())
        .commit();

    let mut universe = Universe::default().with_plugin(plugin);
    let mut scheduler = GraphScheduler::<true>::default();

    let temperature = universe.simulation.spawn((Temperature::default(),))?;

    let mut observer = ChangeObserver::default();
    observer.on_added::<Heat>(move |world, commands, entity| {
        let mut temperature = world
            .component_mut::<true, Temperature>(temperature)
            .unwrap();
        temperature.0 += 1;
        println!("Temperature increase");

        commands.command(DespawnCommand::new(entity));
    });
    observer.on_added::<Cold>(move |world, commands, entity| {
        let mut temperature = world
            .component_mut::<true, Temperature>(temperature)
            .unwrap();
        temperature.0 -= 1;
        println!("Temperature decrease");

        commands.command(DespawnCommand::new(entity));
    });

    for index in 0..10 {
        println!("* Iteration: {}", index);
        scheduler.run(&mut universe)?;
        observer.process_execute(&mut universe.simulation);

        let temperature = universe
            .simulation
            .component::<true, Temperature>(temperature)?;
        println!("Temperature: {}", temperature.0);
    }

    Ok(())
}

fn spawn_temperature_change(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let mut commands = context.fetch::<Res<true, &mut CommandBuffer>>()?;

    if thread_rng().gen_bool(0.5) {
        commands.command(SpawnCommand::new((Heat,)));
    } else {
        commands.command(SpawnCommand::new((Cold,)));
    }

    Ok(())
}
