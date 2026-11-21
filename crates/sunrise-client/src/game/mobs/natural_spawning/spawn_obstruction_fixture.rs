use super::*;
use std::fmt::Write;
use sunrise_client::mobs::{finalize::FinalizeLevel as _, spawner::SpawnerSpawn};
use sunrise_core::nbt::parse_snbt;

#[test]
#[ignore = "daily"]
fn spawn_entity_obstruction() {
    crate::game::in_own_process(
        "game::natural_spawning::spawn_obstruction_fixture::spawn_entity_obstruction",
        || {
            crate::game::native_entity_refs::tests::with_game(|game| {
                let cases = include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tests/fixtures/spawn_obstruction.tsv"
                ));
                let mut output = String::new();
                let mut failures = Vec::new();
                for (i, line) in cases
                    .lines()
                    .filter(|line| !line.starts_with('#'))
                    .enumerate()
                {
                    let [name, installation, id, dx, data, clear, unobstructed] = line
                        .split_ascii_whitespace()
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap();
                    let z = 8.5 + 16.0 * i as f64;
                    let at = [8.5 + dx.parse::<f64>().unwrap(), 64.0, z];
                    game.world.chunks.insert(
                        (0, i as i32),
                        std::sync::Arc::new(sunrise_client::chunk::Chunk::empty(0, i as i32)),
                    );
                    if installation == "falling" {
                        let state = game
                            .pack
                            .assets
                            .registry
                            .default_state("minecraft:sand")
                            .unwrap();
                        game.level_events
                            .push(sunrise_world::level::LevelEvent::FallingBlock {
                                pos: BlockPos::new(8, 64, z as i32),
                                state,
                                hurt_per_distance: None,
                            });
                        game.process_level_events().unwrap();
                    } else if matches!(installation, "live" | "removed") {
                        let summoned = game.summon_with_nbt(id, at, data);
                        assert!(matches!(summoned, Ok(Some(_))), "failed to summon {name}");
                        if installation == "removed" {
                            let mob = game.mobs.last_mut().unwrap();
                            assert_eq!(mob.kind.id, id);
                            mob.removed = true;
                        }
                    }
                    let level = game.natural_level();
                    if installation == "pending" {
                        let (Tag::Compound(mut entity), _) = parse_snbt(data).unwrap() else {
                            panic!("{name}: expected a compound");
                        };
                        entity.insert("id".into(), Tag::String(id.into()));
                        let request = SpawnerSpawn {
                            entity: &entity,
                            id,
                            position: at,
                            yaw: 0.0,
                            finalize: false,
                            equipment: None,
                            trial: false,
                        };
                        game.plan_spawner_mob(
                            &level,
                            &request,
                            None,
                            &mut Vec::new(),
                            &mut LegacyRandom::new(0),
                        )
                        .expect(name);
                    }
                    let pig = game.res.mob_types().get("minecraft:pig").unwrap();
                    let bounds = spawn_selection::spawn_aabb(
                        Species::Pig,
                        8.5,
                        64.0,
                        z,
                        pig.width as f32,
                        pig.height as f32,
                    );
                    let collision = level.no_collision(bounds);
                    let obstruction = spawn_selection::spawn_obstruction(
                        Species::Pig,
                        &level,
                        bounds,
                        Some(&[]),
                        8,
                        64,
                        z as i32,
                    );
                    let finalize = PassLevel::new(&level).no_collision(bounds);
                    writeln!(output, "{name} obstruction {collision} {obstruction}").unwrap();
                    if collision != clear.parse::<bool>().unwrap()
                        || obstruction != unobstructed.parse::<bool>().unwrap()
                        || finalize != collision
                    {
                        failures.push(name);
                    }
                    // The lazy grid must give the same answers as the initial scan.
                    for _ in 0..SCANNED_QUERIES {
                        assert_eq!(level.no_collision(bounds), collision, "{name}");
                        assert_eq!(level.unobstructed(bounds), obstruction, "{name}");
                    }
                }
                if let Some(path) = std::env::var_os("SUNRISE_SPAWN_OBSTRUCTION_RECORDING") {
                    std::fs::write(path, output).unwrap();
                }
                assert!(
                    failures.is_empty(),
                    "different spawn decisions: {failures:?}"
                );
            });
        },
    );
}
