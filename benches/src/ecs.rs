use crate::{Benchmark, COMPARISON_FORMAT, DURATION};
use std::time::Duration;

struct Position(pub f32, pub f32);

struct Velocity(pub f32, pub f32);

pub fn bench() {
    println!();
    println!("--- ECS | BENCHMARKS ---");

    // hecs creations
    let hecs_creations_result = {
        println!();

        use hecs::World;
        let mut world = World::new();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "hecs - creations",
            || (Position(0.0, 0.0), Velocity(1.0, 1.0)),
            |(p, v)| {
                world.spawn((p, v));
            },
            |_| {},
        )
    };

    // ecs creations
    let ecs_creations_result = {
        println!();

        use intuicio_framework_ecs::world::World;
        let mut world = World::default().with_new_archetype_capacity(1024 * 1024);
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "ecs - creations",
            || (Position(0.0, 0.0), Velocity(1.0, 1.0)),
            |(p, v)| {
                world.spawn((p, v)).unwrap();
            },
            |_| {},
        )
    };

    // hecs deletions
    let hecs_deletions_result = {
        println!();

        use hecs::World;
        let mut world = World::new();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run_with_state(
            "hecs - deletions",
            &mut world,
            |world| world.spawn((Position(0.0, 0.0), Velocity(1.0, 1.0))),
            |world, entity| {
                world.despawn(entity).unwrap();
            },
            |_, _| {},
        )
    };

    // ecs deletions
    let ecs_deletions_result = {
        println!();

        use intuicio_framework_ecs::world::World;
        let mut world = World::default().with_new_archetype_capacity(1024 * 1024);
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run_with_state(
            "ecs - deletions",
            &mut world,
            |world| {
                world
                    .spawn((Position(0.0, 0.0), Velocity(1.0, 1.0)))
                    .unwrap()
            },
            |world, entity| {
                world.despawn(entity).unwrap();
            },
            |_, _| {},
        )
    };

    // hecs queries
    let hecs_queries_result = {
        println!();

        use hecs::World;
        let mut world = World::new();
        for _ in 0..10000 {
            world.spawn((Position(0.0, 0.0), Velocity(1.0, 1.0)));
        }
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "hecs - queries",
            || {},
            |_| {
                for (_, (position, velocity)) in world.query::<(&mut Position, &Velocity)>().iter()
                {
                    position.0 += velocity.0;
                    position.1 += velocity.1;
                }
            },
            |_| {},
        )
    };

    // ecs queries
    let ecs_queries_result = {
        println!();

        use intuicio_framework_ecs::world::World;
        let mut world = World::default().with_new_archetype_capacity(1024 * 1024);
        for _ in 0..10000 {
            world
                .spawn((Position(0.0, 0.0), Velocity(1.0, 1.0)))
                .unwrap();
        }
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "ecs - queries",
            || {},
            |_| {
                for (position, velocity) in world.query::<false, (&mut Position, &Velocity)>() {
                    position.0 += velocity.0;
                    position.1 += velocity.1;
                }
            },
            |_| {},
        )
    };

    println!();
    println!("--- ECS | RESULTS ---");

    println!();
    println!("= Hecs vs Ecs | Creation:");
    hecs_creations_result.print_comparison(&ecs_creations_result, COMPARISON_FORMAT);

    println!();
    println!("= Hecs vs Ecs | Deletion:");
    hecs_deletions_result.print_comparison(&ecs_deletions_result, COMPARISON_FORMAT);

    println!();
    println!("= Hecs vs Ecs | Queries:");
    hecs_queries_result.print_comparison(&ecs_queries_result, COMPARISON_FORMAT);
}
