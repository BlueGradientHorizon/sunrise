use super::*;
use std::fmt::Write;
use sunrise_client::mobs::{
    finalize::FinalizeLevel as _,
    spawner::{Spawner, SpawnerExt, SpawnerSpawn},
};
use sunrise_core::nbt::parse_snbt;
use sunrise_core::random::{InvalidBound, PositionalRandom};

/// Two attempts at y=64 with a common z and a specified x displacement.
/// No other draws should occur with this configured spawner and fixed delay.
struct Attempts {
    doubles: std::array::IntoIter<f64, 8>,
    heights: usize,
    yaws: usize,
}

impl RandomSource for Attempts {
    type Positional = Self;
    fn next_double(&mut self) -> f64 {
        self.doubles.next().expect("spawn position draw")
    }
    fn next_int_bound(&mut self, bound: i32) -> Result<i32, InvalidBound> {
        assert_eq!(bound, 3);
        self.heights += 1;
        Ok(1)
    }
    fn next_float(&mut self) -> f32 {
        self.yaws += 1;
        0.75
    }
    fn set_seed(&mut self, _: i64) {
        unreachable!()
    }
    fn next_int(&mut self) -> i32 {
        unreachable!()
    }
    fn next_long(&mut self) -> i64 {
        unreachable!()
    }
    fn next_boolean(&mut self) -> bool {
        unreachable!()
    }
    fn next_gaussian(&mut self) -> f64 {
        unreachable!()
    }
    fn fork_positional(&mut self) -> Self {
        unreachable!()
    }
    fn fork(&mut self) -> Self {
        unreachable!()
    }
}

impl PositionalRandom for Attempts {
    type Source = Self;
    fn at(&self, _: BlockPos) -> Self {
        unreachable!()
    }
    fn from_seed(&self, _: i64) -> Self {
        unreachable!()
    }
    fn from_hash_of(&self, _: &str) -> Self {
        unreachable!()
    }
    fn from_hash_of_utf16(&self, _: &[u16]) -> Self {
        unreachable!()
    }
}

fn spawner_batch(game: &mut Game<'_>, z: f64, dx: f64, id: &str, nbt: &str) -> (usize, usize) {
    let (Tag::Compound(mut entity), _) = parse_snbt(nbt).unwrap() else {
        panic!("expected a compound");
    };
    entity.insert("id".into(), Tag::String(id.into()));
    let mut spawner = Spawner::load(&parse_snbt(concat!(
        "{Delay:0s,SpawnCount:2s,SpawnRange:1s,MaxNearbyEntities:8s,",
        "MinSpawnDelay:20s,MaxSpawnDelay:20s,SpawnPotentials:[],SpawnData:{custom_spawn_rules:{},entity:{id:\"minecraft:pig\"}}}"
    )).unwrap().0);
    spawner.next_data_mut().unwrap().entity = entity;
    game.player = Some(
        sunrise_client::motion::PlayerMotion::new(game.motion, Vec3::new(4.5, 64.0, z)).unwrap(),
    );
    let level = game.natural_level();
    let mut random = Attempts {
        doubles: [0.0, 0.0, 0.0, 0.0, dx, 0.0, 0.0, 0.0].into_iter(),
        heights: 0,
        yaws: 0,
    };
    let mut own = LegacyRandom::new(0);
    let mut planned = Vec::new();
    let mut spawn = |l: &GameLevel, _: &mut Attempts, r: &SpawnerSpawn| {
        assert!(!r.finalize);
        game.plan_spawner_mob(l, r, None, &mut planned, &mut own)
    };
    spawner.server_tick(
        &level,
        &mut random,
        [8, 64, z as i32],
        &mut spawn,
        &mut Vec::new(),
    );
    assert_eq!(random.doubles.len(), 0);
    assert_eq!(random.heights, 2);
    (planned.len(), random.yaws)
}

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
                    let dx = dx.parse::<f64>().unwrap();
                    let at = [8.5 + dx, 64.0, z];
                    game.world.chunks.insert(
                        (0, i as i32),
                        std::sync::Arc::new(sunrise_client::chunk::Chunk::empty(0, i as i32)),
                    );
                    if installation == "batch" {
                        let (accepted, yaws) = spawner_batch(game, z, dx, id, data);
                        writeln!(output, "{name} batch {accepted} {yaws}").unwrap();
                        if (accepted, yaws)
                            != (clear.parse().unwrap(), unobstructed.parse().unwrap())
                        {
                            failures.push(name);
                        }
                        continue;
                    }
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
                    } else if id == "minecraft:ender_dragon" {
                        // Game stores one dragon; install each independent case anew.
                        game.discard_resident_dragon();
                        game.spawn_dragon_facing(
                            at,
                            crate::game::creatures::SpawnSite::Raw,
                            Some(0.0),
                        )
                        .expect(name);
                        if installation == "dragon-part" {
                            game.dragon.as_mut().unwrap().parts[0].0 = Vec3::new(8.5, 64.0, z);
                        }
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
                    if matches!(installation, "pending" | "pending-riders") {
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
                    // At this offset a pig meets the shulker, but not either zombie.
                    let x = if installation == "pending-riders" {
                        8.625
                    } else {
                        8.5
                    };
                    let bounds = spawn_selection::spawn_aabb(
                        Species::Pig,
                        x,
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
                        true,
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
