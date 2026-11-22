//! Witness-driven parity tests for FishingHook hitting pickable targets and ItemEntities.
use super::{Hook, State};
use crate::game::Game;
use serde::Deserialize;
use std::path::PathBuf;
use sunrise_client::items::ItemStack;
use sunrise_core::geometry::Vec3;
use sunrise_core::random::{LegacyRandom, RandomSource};

#[derive(Deserialize)]
struct FishingWitness {
    game_version: String,
    hook_seed: i64,
    level_seed: i64,
    cases: Vec<FishingCase>,
}

#[derive(Deserialize)]
struct FishingCase {
    target: String,
    spawn_y: f64,
    ticks: Vec<FishingTick>,
    retrieve_damage: i32,
    target_vel_after_retrieve_bits: [String; 3],
    target_pos_after_retrieve_tick_bits: [String; 3],
}

#[derive(Deserialize)]
struct FishingTick {
    tick: usize,
    hook_state: String,
    hook_pos_bits: [String; 3],
    hooked_type: Option<String>,
    target_vel_bits: [String; 3],
    game_events: Vec<GameEventSnap>,
}

#[derive(Deserialize)]
struct GameEventSnap {
    id: String,
}

fn load_witness() -> FishingWitness {
    let dir = std::env::var_os("SUNRISE_PROJECTILE_WITNESS_DIR").unwrap_or_else(|| {
        panic!(
            "SUNRISE_PROJECTILE_WITNESS_DIR is unset; run tools/test_projectile_targets_parity.py"
        )
    });
    let path = PathBuf::from(dir).join("fishing_witness.json");
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "failed to read fishing witness {} (record via tools/test_projectile_targets_parity.py): {e}",
            path.display()
        )
    });
    let witness: FishingWitness = serde_json::from_str(&raw).expect("valid 26.3 fishing witness");
    assert_eq!(witness.game_version, "26.3");
    witness
}

fn bits(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

fn vec_bits(v: Vec3) -> [String; 3] {
    [bits(v.x), bits(v.y), bits(v.z)]
}

fn setup_game<'a>(resources: &'a mut Option<std::sync::Arc<crate::game::Resources>>) -> Game<'a> {
    let mut game = crate::game::game_tests::tests::server_game(resources);
    game.build_registries();
    game.set_allow_commands(true);
    game.load_ticking_neighbourhood(0, 0);
    assert!(game.run_command("forceload add 0 0"));
    for command in [
        "gamerule spawn_mobs false",
        "gamerule advance_time false",
        "gamerule advance_weather false",
        "gamerule mob_griefing true",
        "gamerule entity_drops true",
        "gamerule random_tick_speed 0",
    ] {
        let _ = game.run_command(command);
    }
    game.player = Some(
        sunrise_client::motion::PlayerMotion::new(game.motion, Vec3::new(0.5, 64.0, 4.5)).unwrap(),
    );
    crate::game::native_entity_refs::tests::bind_local_player(&mut game, 7, 7);
    game.next_entity = game.next_entity.max(100);
    game
}

fn clear_entities(game: &mut Game<'_>) {
    game.hook.discard();
    game.mobs.clear();
    game.dropped.clear();
    game.boats.clear();
    game.carts.clear();
    game.armor_stands.clear();
    game.tnts.clear();
    game.falling.clear();
    game.orbs.clear();
    game.creatures.projectiles.clear();
    game.rockets.clear();
    game.tridents.clear();
    game.item_frames.clear();
    game.paintings.clear();
    game.cushions.clear();
    game.crystals.clear();
    game.discard_resident_dragon();
    game.others.clear();
    game.knot_ids.clear();
    game.broken_knots.clear();
    game.resident_entities.knots.clear();
    game.reset_entity_residency();
    game.bind_local_player_residency();
}

fn reset_arena(game: &mut Game<'_>, target: &str) {
    clear_entities(game);
    let _ = game.run_command("fill 0 63 0 11 63 9 minecraft:stone");
    let _ = game.run_command("fill 0 64 0 11 70 9 minecraft:air");
    let _ = game.run_command("fill 10 64 2 10 67 6 minecraft:stone");
    let wall = target == "painting"
        || target.starts_with("item_frame")
        || target.starts_with("glow_item_frame");
    if wall {
        let _ = game.run_command("fill 4 64 2 4 66 6 minecraft:stone");
    }
    if target.starts_with("leash_knot") {
        let _ = game.run_command("setblock 3 64 4 minecraft:oak_fence");
    }
    game.set_mode(sunrise_client::command::args::GameMode::Survival);
    if let Some(p) = game.player.as_mut() {
        p.position = Vec3::new(0.5, 64.0, 4.5);
        p.velocity = Vec3::ZERO;
    }
    game.selected = 0;
    game.inventory[0] = Some(ItemStack::new("minecraft:fishing_rod", 1));
    game.tick_server(false).unwrap();
}

#[derive(Clone, Copy)]
enum TrackedTarget {
    Frame(u32),
    Painting(u32),
    Falling(u32),
    Crystal(u32),
    Dragon,
    Stand(u32),
    ExplicitKnot(u32),
    NaturalKnot(u32),
    Cushion(u32),
    Item(u32),
}

impl TrackedTarget {
    fn id(self) -> Option<u32> {
        match self {
            Self::Frame(id)
            | Self::Painting(id)
            | Self::Falling(id)
            | Self::Crystal(id)
            | Self::Stand(id)
            | Self::ExplicitKnot(id)
            | Self::NaturalKnot(id)
            | Self::Cushion(id)
            | Self::Item(id) => Some(id),
            Self::Dragon => None,
        }
    }
}

fn summon_target(game: &mut Game<'_>, target: &str) -> TrackedTarget {
    match target {
        "item_frame" => {
            assert!(game.run_command(
                r#"summon minecraft:item_frame 3.5 64.5 4.5 {block_pos:[I;3,64,4],Facing:4b,Item:{id:"minecraft:diamond",count:1}}"#
            ));
            TrackedTarget::Frame(game.item_frames.last().unwrap().id)
        }
        "item_frame_empty" => {
            assert!(game.run_command(
                "summon minecraft:item_frame 3.5 64.5 4.5 {block_pos:[I;3,64,4],Facing:4b}"
            ));
            TrackedTarget::Frame(game.item_frames.last().unwrap().id)
        }
        "item_frame_fixed" => {
            assert!(game.run_command(
                r#"summon minecraft:item_frame 3.5 64.5 4.5 {block_pos:[I;3,64,4],Facing:4b,Fixed:1b,Item:{id:"minecraft:diamond",count:1}}"#
            ));
            TrackedTarget::Frame(game.item_frames.last().unwrap().id)
        }
        "glow_item_frame" => {
            assert!(game.run_command(
                r#"summon minecraft:glow_item_frame 3.5 64.5 4.5 {block_pos:[I;3,64,4],Facing:4b,Item:{id:"minecraft:diamond",count:1}}"#
            ));
            TrackedTarget::Frame(game.item_frames.last().unwrap().id)
        }
        "painting" => {
            assert!(game.run_command(
                r#"summon minecraft:painting 3.5 64.5 4.5 {block_pos:[I;3,64,4],facing:1b,variant:"minecraft:kebab"}"#
            ));
            TrackedTarget::Painting(game.paintings.last().unwrap().id)
        }
        "falling_block" => {
            assert!(game.run_command(
                r#"summon minecraft:falling_block 3.5 65.0 4.5 {BlockState:"minecraft:sand",Time:1,Motion:[0.0d,0.0d,0.0d]}"#
            ));
            TrackedTarget::Falling(game.falling.last().unwrap().id)
        }
        "end_crystal" => {
            assert!(game.run_command("summon minecraft:end_crystal 4.5 64.0 4.5 {}"));
            TrackedTarget::Crystal(game.crystals.last().unwrap().id)
        }
        "ender_dragon" => {
            assert!(game.run_command(
                "summon minecraft:ender_dragon 7.5 64.0 4.5 {DragonPhase:10,Silent:1b}"
            ));
            game.tick_server(false).unwrap();
            TrackedTarget::Dragon
        }
        "armor_stand" => {
            assert!(game.run_command("summon minecraft:armor_stand 3.5 64.0 4.5 {}"));
            TrackedTarget::Stand(game.armor_stands.last().unwrap().id)
        }
        "armor_stand_marker" => {
            assert!(game.run_command("summon minecraft:armor_stand 3.5 64.0 4.5 {Marker:1b}"));
            TrackedTarget::Stand(game.armor_stands.last().unwrap().id)
        }
        "armor_stand_small" => {
            assert!(game.run_command("summon minecraft:armor_stand 3.5 64.0 4.5 {Small:1b}"));
            TrackedTarget::Stand(game.armor_stands.last().unwrap().id)
        }
        "armor_stand_nogravity" => {
            assert!(game.run_command("summon minecraft:armor_stand 3.5 64.0 4.5 {NoGravity:1b}"));
            TrackedTarget::Stand(game.armor_stands.last().unwrap().id)
        }
        "leash_knot" => {
            assert!(game.run_command("summon minecraft:leash_knot 3.5 64.375 4.5 {}"));
            TrackedTarget::ExplicitKnot(game.others.last().unwrap().id)
        }
        "leash_knot_tied" => {
            assert!(game.run_command(
                "summon minecraft:pig 3.5 64.0 6.5 {NoAI:1b,Silent:1b,leash:[I;3,64,4]}"
            ));
            game.tick_server(false).unwrap();
            let id = *game
                .knot_ids
                .get(&[3, 64, 4])
                .expect("natural leash knot created on fence");
            TrackedTarget::NaturalKnot(id)
        }
        "cushion" => {
            assert!(game.run_command(
                r#"summon minecraft:cushion 3.5 64.0 4.5 {block_pos:[I;3,64,4],color:"white"}"#
            ));
            TrackedTarget::Cushion(game.cushions.last().unwrap().id)
        }
        "dropped_item" => {
            assert!(game.run_command(
                r#"summon minecraft:item 3.5 64.0 4.5 {Item:{id:"minecraft:diamond",count:1},Motion:[0.0d,0.0d,0.0d],PickupDelay:32767}"#
            ));
            TrackedTarget::Item(game.dropped.last().unwrap().id)
        }
        other => panic!("unexpected fishing witness target {other}"),
    }
}

fn target_kinematics(game: &Game<'_>, target: TrackedTarget) -> Option<(Vec3, Vec3)> {
    match target {
        TrackedTarget::Falling(id) => game
            .falling
            .iter()
            .find(|f| f.id == id)
            .map(|f| (f.position, f.velocity)),
        TrackedTarget::Stand(id) => game
            .armor_stands
            .iter()
            .find(|a| a.id == id)
            .map(|a| (a.position, a.velocity)),
        TrackedTarget::Item(id) => game
            .dropped
            .iter()
            .find(|i| i.id == id)
            .map(|i| (i.position, i.velocity)),
        _ => None,
    }
}

fn run_case(
    game: &mut Game<'_>,
    case: &FishingCase,
    hook_seed: i64,
    level_seed: i64,
) -> Vec<String> {
    let mut errors = Vec::new();
    let label = &case.target;
    reset_arena(game, &case.target);
    game.world.random.set_seed(level_seed);

    let tracked = summon_target(game, &case.target);
    game.next_entity += 1;
    let id = game.next_entity;
    let mut hook = Hook::remote(id, Vec3::new(1.5, case.spawn_y, 4.5), [0, 0, 0, id as i32]);
    hook.velocity = Vec3::new(1.0, 0.0, 0.0);
    hook.random = LegacyRandom::new(hook_seed);
    game.install_fishing_hook(hook);
    game.world.random.set_seed(level_seed);

    for expected in &case.ticks {
        crate::game::sculk::TEST_POSTED_EVENTS.with(|e| {
            *e.borrow_mut() = Some(Vec::new());
        });
        game.tick_server(false).unwrap();
        let posted = crate::game::sculk::TEST_POSTED_EVENTS
            .with(|e| e.borrow_mut().take().unwrap_or_default());

        let exp_land = expected
            .game_events
            .iter()
            .any(|g| g.id == "minecraft:projectile_land");
        let got_land = posted
            .iter()
            .any(|(id, _, _)| *id == "minecraft:projectile_land");
        if exp_land && !got_land {
            errors.push(format!(
                "{label} t={}: expected minecraft:projectile_land, got {:?}",
                expected.tick,
                posted.iter().map(|(id, _, _)| *id).collect::<Vec<_>>()
            ));
        }

        // Steady-state checks on the final pre-retrieve tick (t=6).
        if expected.tick == case.ticks.len() {
            let Some(h) = game.hook.as_ref() else {
                errors.push(format!("{label} t={}: hook vanished", expected.tick));
                continue;
            };
            let exp_hooked = expected.hook_state == "HOOKED_IN_ENTITY";
            match (exp_hooked, h.state) {
                (true, State::Hooked(hid)) => {
                    if let Some(tid) = tracked.id()
                        && hid != tid
                    {
                        errors.push(format!(
                            "{label} t={}: hooked id expected {tid} got {hid}",
                            expected.tick
                        ));
                    }
                    let got_pos = vec_bits(h.position);
                    if got_pos != expected.hook_pos_bits {
                        errors.push(format!(
                            "{label} t={}: hooked pos expected {:?} got {:?} ({:?})",
                            expected.tick, expected.hook_pos_bits, got_pos, h.position
                        ));
                    }
                }
                (true, other) => {
                    errors.push(format!(
                        "{label} t={}: expected Hooked({:?}), got {other:?} at {:?}",
                        expected.tick, expected.hooked_type, h.position
                    ));
                }
                (false, State::Hooked(hid)) => {
                    errors.push(format!(
                        "{label} t={}: expected Flying (not hooked), got Hooked({hid})",
                        expected.tick
                    ));
                }
                (false, _) => {}
            }
        }
    }

    // Retrieve rod and verify durability damage and pull velocity.
    let vel_before = target_kinematics(game, tracked).map(|(_, v)| v);
    let rod = ItemStack::new("minecraft:fishing_rod", 1);
    let damage = game.retrieve_hook(&rod) as i32;
    if damage != case.retrieve_damage {
        errors.push(format!(
            "{label}: retrieve damage expected {} got {damage}",
            case.retrieve_damage
        ));
    }
    if let Some((_, vel)) = target_kinematics(game, tracked) {
        let got_vel = vec_bits(vel);
        if matches!(tracked, TrackedTarget::Item(_)) {
            // ItemEntity.tick zeroes ground y-velocity when (tickCount + id) % 4 == 0, so compare pull delta.
            let before = vel_before.unwrap();
            let parse = |s: &str| f64::from_bits(u64::from_str_radix(s, 16).unwrap());
            let exp_before = &case.ticks.last().unwrap().target_vel_bits;
            let exp_after = &case.target_vel_after_retrieve_bits;
            let got_delta = [vel.x - before.x, vel.y - before.y, vel.z - before.z];
            let exp_delta = [
                parse(&exp_after[0]) - parse(&exp_before[0]),
                parse(&exp_after[1]) - parse(&exp_before[1]),
                parse(&exp_after[2]) - parse(&exp_before[2]),
            ];
            if got_vel[0] != exp_after[0]
                || got_vel[2] != exp_after[2]
                || (got_delta[1] - exp_delta[1]).abs() > 1e-12
            {
                errors.push(format!(
                    "{label}: pull delta expected {exp_delta:?} got {got_delta:?} (vel {vel:?})"
                ));
            }
        } else if got_vel != case.target_vel_after_retrieve_bits {
            errors.push(format!(
                "{label}: vel after retrieve expected {:?} got {:?} ({vel:?})",
                case.target_vel_after_retrieve_bits, got_vel
            ));
        }
    }
    game.tick_server(false).unwrap();
    if let Some((pos, _)) = target_kinematics(game, tracked) {
        let got_pos = vec_bits(pos);
        if got_pos != case.target_pos_after_retrieve_tick_bits {
            errors.push(format!(
                "{label}: pos after retrieve tick expected {:?} got {:?} ({pos:?})",
                case.target_pos_after_retrieve_tick_bits, got_pos
            ));
        }
    }
    errors
}

#[test]
#[ignore = "run by tools/test_projectile_targets_parity.py"]
fn witness_fishing_bobber_targets() {
    let witness = load_witness();
    let mut resources = None;
    let mut game = setup_game(&mut resources);
    let mut failures = Vec::new();
    for case in &witness.cases {
        let errs = run_case(&mut game, case, witness.hook_seed, witness.level_seed);
        if !errs.is_empty() {
            failures.push(errs.join("\n  "));
        }
    }
    assert!(
        failures.is_empty(),
        "{} / {} fishing witness cases failed:\n{}",
        failures.len(),
        witness.cases.len(),
        failures.join("\n")
    );
}
