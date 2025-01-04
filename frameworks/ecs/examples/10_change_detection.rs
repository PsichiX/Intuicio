use intuicio_framework_ecs::prelude::*;
use rand::{thread_rng, Rng};
use std::error::Error;

#[derive(Debug, Default, Clone, Copy)]
struct Pigeon;

fn main() -> Result<(), Box<dyn Error>> {
    struct MyPlugin;
    let plugin = GraphSchedulerQuickPlugin::<true, MyPlugin>::default()
        .resource(CommandBuffer::default())
        .system(send_pigeon, "send_pigeon", ())
        .system(report_alive, "report_alive", ())
        .system(kill_received_pigeons, "kill_received_pigeons", ())
        .commit();

    let mut universe = Universe::default().with_plugin(plugin);
    let mut scheduler = GraphScheduler::<true>::default();

    for index in 0..10 {
        println!("* Iteration: {}", index);
        scheduler.run(&mut universe)?;
    }

    Ok(())
}

fn send_pigeon(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let mut commands = context.fetch::<Res<true, &mut CommandBuffer>>()?;

    for _ in 0..thread_rng().gen_range(0..3) {
        println!("Send pigeon");
        commands.command(SpawnCommand::new((Pigeon,)));
    }

    Ok(())
}

fn kill_received_pigeons(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (world, mut commands) = context.fetch::<(&World, Res<true, &mut CommandBuffer>)>()?;

    for entity in world.added().iter_of::<Pigeon>() {
        println!("Kill pigeon: {}", entity);
        commands.command(DespawnCommand::new(entity));
    }

    Ok(())
}

fn report_alive(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (world, pigeon_query) =
        context.fetch::<(&World, Query<true, (Entity, Include<Pigeon>)>)>()?;

    for (entity, _) in pigeon_query.query(world) {
        println!("Pigeon alive: {}", entity);
    }

    Ok(())
}
