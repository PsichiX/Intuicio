use intuicio_framework_ecs::prelude::*;
use rand::{thread_rng, Rng};
use std::error::Error;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum MonsterEvolution {
    #[default]
    Puppy,
    Wolf,
}

#[derive(Debug, Default, Clone, Copy)]
struct Stats {
    created: usize,
    updated: usize,
    destroyed: usize,
}

fn main() -> Result<(), Box<dyn Error>> {
    struct MyPlugin;
    let plugin = GraphSchedulerQuickPlugin::<true, MyPlugin>::default()
        .resource(CommandBuffer::default())
        .resource(Stats::default())
        .system(evolve_monster, "evolve_monster", ())
        .system(spawn_monster, "spawn_monster", ())
        .system(stats_react, "stats_react", ())
        .commit();

    let mut universe = Universe::default().with_plugin(plugin);
    let mut scheduler = GraphScheduler::<true>::default();

    for index in 0..10 {
        println!("* Iteration: {}", index);
        scheduler.run(&mut universe)?;
    }

    Ok(())
}

fn spawn_monster(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let mut commands = context.fetch::<Res<true, &mut CommandBuffer>>()?;

    for _ in 0..thread_rng().gen_range(0..3) {
        commands.command(SpawnCommand::new((MonsterEvolution::default(),)));
    }

    Ok(())
}

fn evolve_monster(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (world, mut commands, monster_query) = context.fetch::<(
        &World,
        Res<true, &mut CommandBuffer>,
        Query<true, (Entity, Update<MonsterEvolution>)>,
    )>()?;

    for (entity, mut monster) in monster_query.query(world) {
        match *monster.read() {
            MonsterEvolution::Puppy => {
                *monster.write_notified(world) = MonsterEvolution::Wolf;
            }
            MonsterEvolution::Wolf => {
                commands.command(DespawnCommand::new(entity));
            }
        };
    }

    Ok(())
}

fn stats_react(context: SystemContext) -> Result<(), Box<dyn Error>> {
    let (world, mut stats) = context.fetch::<(&World, Res<true, &mut Stats>)>()?;

    for entity in world.added().iter_of::<MonsterEvolution>() {
        println!("Monster created: {}", entity);
        stats.created += 1;
    }

    for entity in world.updated().unwrap().iter_of::<MonsterEvolution>() {
        println!("Monster updated: {}", entity);
        stats.updated += 1;
    }

    for entity in world.removed().iter_of::<MonsterEvolution>() {
        println!("Monster destroyed: {}", entity);
        stats.destroyed += 1;
    }

    Ok(())
}
