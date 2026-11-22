//! Witness-driven parity tests for projectile gaps #120-#124 against vanilla 26.3.
use crate::game::Game;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use sunrise_client::projectiles::{Projectile, ProjectileKind};
use sunrise_core::geometry::Vec3;
use sunrise_core::random::{LegacyRandom, RandomSource};

#[derive(Deserialize)]
struct Witness {
    game_version: String,
    proj_seed: i64,
    level_seed: i64,
    gap_120_spectator: Vec<Case120>,
    gap_121_wind_crystal: Vec<Case121>,
    gap_122_trident_enderman: Vec<Case122>,
    gap_123_piercing: Vec<Case123>,
    gap_124_fireworks: Vec<Case124>,
}

#[derive(Deserialize)]
struct GameEventSnap {
    id: String,
}

#[derive(Deserialize)]
struct Case120 {
    mode: String,
    projectile: String,
    ticks: Vec<Tick120>,
}

#[derive(Deserialize)]
struct Tick120 {
    tick: usize,
    proj_alive: bool,
    proj_pos_bits: [String; 3],
    proj_vel_bits: [String; 3],
    in_ground: bool,
    player_health: f32,
}

#[derive(Deserialize)]
struct Case121 {
    case: String,
    ticks: Vec<Tick121>,
}

#[derive(Deserialize)]
struct Tick121 {
    tick: usize,
    proj_alive: bool,
    proj_pos_bits: [String; 3],
    proj_vel_bits: [String; 3],
    crystal_alive: bool,
    pig_health: f32,
}

#[derive(Deserialize)]
struct Case122 {
    case: String,
    ticks: Vec<Tick122>,
}

#[derive(Deserialize)]
struct Tick122 {
    tick: usize,
    proj_alive: bool,
    proj_pos_bits: [String; 3],
    proj_vel_bits: [String; 3],
    dealt_damage: bool,
    target_health: f32,
    target_teleported: bool,
    behind_pig_health: f32,
    game_events: Vec<GameEventSnap>,
    sounds: Vec<String>,
}

#[derive(Deserialize)]
struct Case123 {
    case: String,
    ticks: Vec<Tick123>,
}

#[derive(Deserialize)]
struct Target123 {
    alive: bool,
    health: f32,
}

#[derive(Deserialize)]
struct Tick123 {
    tick: usize,
    proj_alive: bool,
    proj_pos_bits: [String; 3],
    proj_vel_bits: [String; 3],
    in_ground: bool,
    targets: Vec<Target123>,
    two_birds_done: bool,
    arbalistic_done: bool,
    game_events: Vec<GameEventSnap>,
    sounds: Vec<String>,
}

#[derive(Deserialize)]
struct Case124 {
    case: String,
    spawn_y: f64,
    ticks: Vec<Tick124>,
}

#[derive(Deserialize)]
struct Tick124 {
    tick: usize,
    rocket_alive: bool,
    rocket_pos_bits: [String; 3],
    rocket_vel_bits: [String; 3],
    targets_alive: BTreeMap<String, bool>,
    drops: Vec<String>,
    game_events: Vec<GameEventSnap>,
    sounds: Vec<String>,
}

const STAR_FIREWORK_ITEM: &str = r#"{id:"minecraft:firework_rocket",count:1,components:{"minecraft:fireworks":{flight_duration:1,explosions:[{shape:"small_ball",colors:[I;11743532]}]}}}"#;
const PLAIN_FIREWORK_ITEM: &str = r#"{id:"minecraft:firework_rocket",count:1}"#;

fn load_witness() -> Witness {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/witness/26.3/projectile_targets/projectiles_120_124_witness.json");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read witness {}: {e}", path.display()));
    let witness: Witness = serde_json::from_str(&raw).expect("valid 26.3 witness JSON");
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
    game.options.dedicated = None;
    let mut player =
        sunrise_client::motion::PlayerMotion::new(game.motion, Vec3::new(0.5, 64.0, 9.5)).unwrap();
    player.client_loaded = true;
    game.player = Some(player);
    crate::game::native_entity_refs::tests::bind_local_player(&mut game, 1, (413u128 << 96) | 1);
    game.level.health = 20.0;
    game.creative = false;
    game.build_registries();
    game.set_allow_commands(true);
    game.load_ticking_neighbourhood(0, 0);
    assert!(game.run_command("forceload add 0 0"));
    assert!(game.run_command("difficulty normal"));
    if game
        .adv
        .tree
        .find_id("minecraft:adventure/two_birds_one_arrow")
        .is_none()
    {
        assert!(game.run_command("reload"));
    }
    assert!(
        game.adv
            .tree
            .find_id("minecraft:adventure/two_birds_one_arrow")
            .is_some()
    );
    assert!(
        game.adv
            .tree
            .find_id("minecraft:adventure/arbalistic")
            .is_some()
    );
    for command in [
        "gamerule spawn_mobs false",
        "gamerule advance_time false",
        "gamerule advance_weather false",
        "gamerule mob_griefing true",
        "gamerule entity_drops true",
    ] {
        let _ = game.run_command(command);
    }
    game
}

fn clear_arena(game: &mut Game<'_>, backstop_x: i32) {
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
    game.others.clear();
    game.knot_ids.clear();
    game.broken_knots.clear();
    game.resident_entities.knots.clear();
    game.reset_entity_residency();
    game.bind_local_player_residency();
    let _ = game.run_command("fill 0 63 0 14 63 10 minecraft:stone");
    let _ = game.run_command("fill 0 64 0 14 70 10 minecraft:air");
    if backstop_x > 0 {
        let _ = game.run_command(&format!(
            "fill {backstop_x} 64 2 {backstop_x} 67 6 minecraft:stone"
        ));
    }
    game.set_mode(sunrise_client::command::args::GameMode::Survival);
    if let Some(p) = game.player.as_mut() {
        let _ = p.set_health(20.0, false);
        p.health.invulnerable_time = 0;
        p.health.hurt.cooldown = 0;
        p.health.hurt.last_hurt = 0.0;
        p.health.permanently_invulnerable = false;
        p.client_loaded = true;
        p.fire_ticks = 0;
        p.position = Vec3::new(0.5, 64.0, 9.5);
        p.velocity = Vec3::ZERO;
    }
    let _ = game.run_command("advancement revoke @s everything");
    game.tick_server(false).unwrap();
    game.audio.heard.clear();
}

fn player_uuid_nbt(game: &Game<'_>) -> String {
    let arr = game.level.player_uuid.unwrap_or([0, 0, 0, 1]);
    format!("[I;{},{},{},{}]", arr[0], arr[1], arr[2], arr[3])
}

fn adv_done(game: &Game<'_>, id: &str) -> bool {
    game.adv
        .tree
        .find_id(id)
        .is_some_and(|idx| game.adv.progress.done(&game.adv.tree.list[idx]))
}

fn step_recording(game: &mut Game<'_>) -> (Vec<&'static str>, BTreeMap<String, usize>) {
    game.audio.heard.clear();
    super::super::sculk::TEST_POSTED_EVENTS.with(|e| *e.borrow_mut() = Some(Vec::new()));
    game.tick_server(false).unwrap();
    let events = super::super::sculk::TEST_POSTED_EVENTS
        .with(|e| e.borrow_mut().take().unwrap_or_default())
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    let sounds = game
        .audio
        .heard
        .iter()
        .map(|(k, &v)| (k.clone(), v as usize))
        .collect();
    (events, sounds)
}

#[test]
#[ignore = "daily"]
fn witness_spectator_player_120() {
    let witness = load_witness();
    let mut resources = None;
    let mut game = setup_game(&mut resources);
    let mut failures = Vec::new();

    for case in &witness.gap_120_spectator {
        let label = format!("120 {} x {}", case.mode, case.projectile);
        clear_arena(&mut game, 8);
        game.world.random.set_seed(witness.level_seed);
        let mode = if case.mode == "spectator" {
            sunrise_client::command::args::GameMode::Spectator
        } else {
            sunrise_client::command::args::GameMode::Survival
        };
        game.set_mode(mode);
        if let Some(p) = game.player.as_mut() {
            let _ = p.set_health(20.0, false);
            p.health.invulnerable_time = 0;
            p.health.hurt.cooldown = 0;
            p.health.hurt.last_hurt = 0.0;
            p.position = Vec3::new(3.5, 64.0, 4.5);
            p.velocity = Vec3::ZERO;
        }
        game.tick_server(false).unwrap();

        let cmd = match case.projectile.as_str() {
            "arrow" => "summon minecraft:arrow 1.5 65.0 4.5 {Motion:[1.0d,0.0d,0.0d],damage:2.0d}",
            "snowball" => "summon minecraft:snowball 1.5 65.0 4.5 {Motion:[1.0d,0.0d,0.0d]}",
            "small_fireball" => {
                "summon minecraft:small_fireball 1.5 65.0 4.5 {Motion:[1.0d,0.0d,0.0d],acceleration_power:0.0d}"
            }
            "wind_charge" => {
                "summon minecraft:wind_charge 1.5 65.0 4.5 {Motion:[1.0d,0.0d,0.0d],acceleration_power:0.0d}"
            }
            "trident" => "summon minecraft:trident 1.5 65.0 4.5 {Motion:[1.0d,0.0d,0.0d]}",
            other => panic!("unexpected projectile {other}"),
        };
        assert!(game.run_command(cmd), "{cmd}");
        let is_trident = case.projectile == "trident";
        let id = if is_trident {
            let idx = game.tridents.len() - 1;
            let id = game.tridents[idx].id;
            game.tridents.update(idx, |t| {
                t.random = LegacyRandom::new(witness.proj_seed);
            });
            id
        } else {
            let idx = game.creatures.projectiles.len() - 1;
            let id = game.creatures.projectiles[idx].id;
            game.creatures.projectiles.update(idx, |p| {
                p.entity_random().set_seed(witness.proj_seed);
            });
            id
        };

        for exp in &case.ticks {
            game.tick_server(false).unwrap();
            let hp = game.player.as_ref().unwrap().health.health();
            if hp.to_bits() != exp.player_health.to_bits() {
                failures.push(format!(
                    "{label} t={}: player_health expected {} got {hp}",
                    exp.tick, exp.player_health
                ));
                break;
            }
            let (alive, pos, vel, ig) = if is_trident {
                match game.tridents.iter().find(|t| t.id == id) {
                    Some(t) => (true, Some(t.position), Some(t.velocity), t.in_ground),
                    None => (false, None, None, false),
                }
            } else {
                match game.creatures.projectiles.iter().find(|p| p.id == id) {
                    Some(p) => (
                        true,
                        Some(p.position),
                        Some(p.velocity),
                        p.arrow.as_ref().is_some_and(|a| a.in_ground),
                    ),
                    None => (false, None, None, false),
                }
            };
            if alive != exp.proj_alive {
                failures.push(format!(
                    "{label} t={}: proj_alive expected {} got {alive}",
                    exp.tick, exp.proj_alive
                ));
                break;
            }
            if let (Some(pos), Some(vel)) = (pos, vel) {
                if vec_bits(pos) != exp.proj_pos_bits {
                    failures.push(format!(
                        "{label} t={}: pos expected {:?} got {:?} ({pos:?})",
                        exp.tick,
                        exp.proj_pos_bits,
                        vec_bits(pos)
                    ));
                    break;
                }
                if vec_bits(vel) != exp.proj_vel_bits {
                    failures.push(format!(
                        "{label} t={}: vel expected {:?} got {:?} ({vel:?})",
                        exp.tick,
                        exp.proj_vel_bits,
                        vec_bits(vel)
                    ));
                    break;
                }
                if ig != exp.in_ground {
                    failures.push(format!(
                        "{label} t={}: in_ground expected {} got {ig}",
                        exp.tick, exp.in_ground
                    ));
                    break;
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "gap #120 failures:\n{}",
        failures.join("\n")
    );
}

#[test]
#[ignore = "daily"]
fn witness_wind_charge_end_crystal_121() {
    let witness = load_witness();
    let mut resources = None;
    let mut game = setup_game(&mut resources);
    let mut failures = Vec::new();

    for case in &witness.gap_121_wind_crystal {
        let label = format!("121 {}", case.case);
        clear_arena(&mut game, 9);
        game.world.random.set_seed(witness.level_seed);

        assert!(game.run_command("summon minecraft:end_crystal 4.25 64.0 4.5 {}"));
        let crystal_id = game.crystals.last().unwrap().id;
        let pig_id = if case.case == "wind_charge_crystal_then_pig" {
            assert!(game.run_command("summon minecraft:pig 7.5 64.0 4.5 {NoAI:1b,Silent:1b}"));
            Some(game.mobs.last().unwrap().id)
        } else {
            None
        };
        let cmd = match case.case.as_str() {
            "wind_charge_through_crystal" | "wind_charge_crystal_then_pig" => {
                "summon minecraft:wind_charge 1.5 65.0 4.5 {Motion:[1.0d,0.0d,0.0d],acceleration_power:0.0d}"
            }
            "breeze_wind_charge_through_crystal" => {
                "summon minecraft:breeze_wind_charge 1.5 65.0 4.5 {Motion:[1.0d,0.0d,0.0d],acceleration_power:0.0d}"
            }
            "arrow_hits_crystal" => {
                "summon minecraft:arrow 1.5 65.0 4.5 {Motion:[1.0d,0.0d,0.0d],damage:2.0d}"
            }
            other => panic!("unexpected case {other}"),
        };
        assert!(game.run_command(cmd), "{cmd}");
        let idx = game.creatures.projectiles.len() - 1;
        let proj_id = game.creatures.projectiles[idx].id;
        game.creatures.projectiles.update(idx, |p| {
            p.entity_random().set_seed(witness.proj_seed);
        });

        for exp in &case.ticks {
            game.tick_server(false).unwrap();
            let crystal_alive = game.crystals.iter().any(|c| c.id == crystal_id);
            if crystal_alive != exp.crystal_alive {
                failures.push(format!(
                    "{label} t={}: crystal_alive expected {} got {crystal_alive}",
                    exp.tick, exp.crystal_alive
                ));
                break;
            }
            if let Some(pid) = pig_id {
                let hp = game
                    .mobs
                    .iter()
                    .find(|m| m.id == pid)
                    .map_or(0.0, |m| m.health);
                if hp.to_bits() != exp.pig_health.to_bits() {
                    failures.push(format!(
                        "{label} t={}: pig_health expected {} got {hp}",
                        exp.tick, exp.pig_health
                    ));
                    break;
                }
            }
            let live = game.creatures.projectiles.iter().find(|p| p.id == proj_id);
            if live.is_some() != exp.proj_alive {
                failures.push(format!(
                    "{label} t={}: proj_alive expected {} got {}",
                    exp.tick,
                    exp.proj_alive,
                    live.is_some()
                ));
                break;
            }
            if let Some(p) = live {
                if vec_bits(p.position) != exp.proj_pos_bits {
                    failures.push(format!(
                        "{label} t={}: pos expected {:?} got {:?} ({:?})",
                        exp.tick,
                        exp.proj_pos_bits,
                        vec_bits(p.position),
                        p.position
                    ));
                    break;
                }
                if vec_bits(p.velocity) != exp.proj_vel_bits {
                    failures.push(format!(
                        "{label} t={}: vel expected {:?} got {:?} ({:?})",
                        exp.tick,
                        exp.proj_vel_bits,
                        vec_bits(p.velocity),
                        p.velocity
                    ));
                    break;
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "gap #121 failures:\n{}",
        failures.join("\n")
    );
}

#[test]
#[ignore = "daily"]
fn witness_trident_enderman_dodge_122() {
    let witness = load_witness();
    let mut resources = None;
    let mut game = setup_game(&mut resources);
    let mut failures = Vec::new();

    for case in &witness.gap_122_trident_enderman {
        let label = format!("122 {}", case.case);
        clear_arena(&mut game, 10);
        game.world.random.set_seed(witness.level_seed);

        let target_id = match case.case.as_str() {
            "trident_enderman_in_boat" => {
                assert!(game.run_command(
                    r#"summon minecraft:oak_boat 3.5 64.0 4.5 {Passengers:[{id:"minecraft:enderman",NoAI:1b,Silent:1b}]}"#
                ));
                game.mobs.last().unwrap().id
            }
            "trident_invulnerable_pig" => {
                assert!(game.run_command(
                    "summon minecraft:pig 3.5 64.0 4.5 {NoAI:1b,Silent:1b,Invulnerable:1b}"
                ));
                game.mobs.last().unwrap().id
            }
            _ => {
                assert!(
                    game.run_command("summon minecraft:enderman 3.5 64.0 4.5 {NoAI:1b,Silent:1b}")
                );
                game.mobs.last().unwrap().id
            }
        };
        let target_pos0 = game
            .mobs
            .iter()
            .find(|m| m.id == target_id)
            .unwrap()
            .position;
        assert!(game.run_command("summon minecraft:pig 6.5 64.0 4.5 {NoAI:1b,Silent:1b}"));
        let behind_id = game.mobs.last().unwrap().id;

        let cmd = if case.case == "trident_owned_enderman" {
            format!(
                "summon minecraft:trident 1.5 65.0 4.5 {{Motion:[1.0d,0.0d,0.0d],Owner:{},LeftOwner:1b}}",
                player_uuid_nbt(&game)
            )
        } else {
            "summon minecraft:trident 1.5 65.0 4.5 {Motion:[1.0d,0.0d,0.0d]}".to_owned()
        };
        assert!(game.run_command(&cmd), "{cmd}");
        let idx = game.tridents.len() - 1;
        let tr_id = game.tridents[idx].id;
        game.tridents.update(idx, |t| {
            t.random = LegacyRandom::new(witness.proj_seed);
        });

        for exp in &case.ticks {
            let (events, sounds) = step_recording(&mut game);
            let t = game.tridents.iter().find(|t| t.id == tr_id);
            if t.is_some() != exp.proj_alive {
                failures.push(format!(
                    "{label} t={}: alive expected {} got {}",
                    exp.tick,
                    exp.proj_alive,
                    t.is_some()
                ));
                break;
            }
            if let Some(t) = t {
                if vec_bits(t.position) != exp.proj_pos_bits {
                    failures.push(format!(
                        "{label} t={}: pos expected {:?} got {:?} ({:?})",
                        exp.tick,
                        exp.proj_pos_bits,
                        vec_bits(t.position),
                        t.position
                    ));
                    break;
                }
                if vec_bits(t.velocity) != exp.proj_vel_bits {
                    failures.push(format!(
                        "{label} t={}: vel expected {:?} got {:?} ({:?})",
                        exp.tick,
                        exp.proj_vel_bits,
                        vec_bits(t.velocity),
                        t.velocity
                    ));
                    break;
                }
                if t.dealt_damage != exp.dealt_damage {
                    failures.push(format!(
                        "{label} t={}: dealt_damage expected {} got {}",
                        exp.tick, exp.dealt_damage, t.dealt_damage
                    ));
                    break;
                }
            }
            let target = game.mobs.iter().find(|m| m.id == target_id).unwrap();
            if target.health.to_bits() != exp.target_health.to_bits() {
                failures.push(format!(
                    "{label} t={}: target_health expected {} got {}",
                    exp.tick, exp.target_health, target.health
                ));
                break;
            }
            let teleported = target.position.distance_squared(target_pos0) > 1e-6;
            if teleported != exp.target_teleported {
                failures.push(format!(
                    "{label} t={}: target_teleported expected {} got {teleported}",
                    exp.tick, exp.target_teleported
                ));
                break;
            }
            let behind_hp = game
                .mobs
                .iter()
                .find(|m| m.id == behind_id)
                .map_or(0.0, |m| m.health);
            if behind_hp.to_bits() != exp.behind_pig_health.to_bits() {
                failures.push(format!(
                    "{label} t={}: behind_pig_health expected {} got {behind_hp}",
                    exp.tick, exp.behind_pig_health
                ));
                break;
            }
            let exp_hit_snd = exp
                .sounds
                .iter()
                .filter(|s| s.as_str() == "minecraft:item.trident.hit")
                .count();
            let got_hit_snd = sounds
                .get("minecraft:item.trident.hit")
                .copied()
                .unwrap_or(0);
            if got_hit_snd != exp_hit_snd {
                failures.push(format!(
                    "{label} t={}: item.trident.hit count expected {exp_hit_snd} got {got_hit_snd}",
                    exp.tick
                ));
                break;
            }
            let exp_land = exp
                .game_events
                .iter()
                .filter(|e| e.id == "minecraft:projectile_land")
                .count();
            let got_land = events
                .iter()
                .filter(|&&e| e == "minecraft:projectile_land")
                .count();
            if got_land != exp_land {
                failures.push(format!(
                    "{label} t={}: projectile_land count expected {exp_land} got {got_land}",
                    exp.tick
                ));
                break;
            }
        }
    }

    // Also verify mob-thrown ProjectileKind::Trident path against unowned Enderman witness.
    let unowned = witness
        .gap_122_trident_enderman
        .iter()
        .find(|c| c.case == "trident_unowned_enderman")
        .unwrap();
    clear_arena(&mut game, 10);
    game.world.random.set_seed(witness.level_seed);
    assert!(game.run_command("summon minecraft:enderman 3.5 64.0 4.5 {NoAI:1b,Silent:1b}"));
    let enderman_id = game.mobs.last().unwrap().id;
    let enderman_pos0 = game
        .mobs
        .iter()
        .find(|m| m.id == enderman_id)
        .unwrap()
        .position;
    assert!(game.run_command("summon minecraft:pig 6.5 64.0 4.5 {NoAI:1b,Silent:1b}"));
    let behind_id = game.mobs.last().unwrap().id;
    let mut proj = Projectile::new(ProjectileKind::Trident, Vec3::new(1.5, 65.0, 4.5), None);
    proj.id = game.take_entity_id();
    proj.velocity = Vec3::new(1.0, 0.0, 0.0);
    proj.entity_random().set_seed(witness.proj_seed);
    let mob_tr_id = proj.id;
    game.creatures.projectiles.push(proj);
    for exp in &unowned.ticks {
        let (events, sounds) = step_recording(&mut game);
        let p = game
            .creatures
            .projectiles
            .iter()
            .find(|p| p.id == mob_tr_id);
        let Some(p) = p else {
            failures.push(format!("122 mob_trident t={}: disappeared", exp.tick));
            break;
        };
        if vec_bits(p.position) != exp.proj_pos_bits || vec_bits(p.velocity) != exp.proj_vel_bits {
            failures.push(format!(
                "122 mob_trident t={}: pos/vel expected {:?}/{:?} got {:?}/{:?}",
                exp.tick,
                exp.proj_pos_bits,
                exp.proj_vel_bits,
                vec_bits(p.position),
                vec_bits(p.velocity)
            ));
            break;
        }
        let dd = p.arrow.as_ref().is_some_and(|a| a.dealt_damage);
        if dd != exp.dealt_damage {
            failures.push(format!(
                "122 mob_trident t={}: dealt_damage expected {} got {dd}",
                exp.tick, exp.dealt_damage
            ));
            break;
        }
        let em = game.mobs.iter().find(|m| m.id == enderman_id).unwrap();
        let teleported = em.position.distance_squared(enderman_pos0) > 1e-6;
        let behind_hp = game
            .mobs
            .iter()
            .find(|m| m.id == behind_id)
            .map_or(0.0, |m| m.health);
        let hit_snd = sounds
            .get("minecraft:item.trident.hit")
            .copied()
            .unwrap_or(0);
        let got_land = events
            .iter()
            .filter(|&&e| e == "minecraft:projectile_land")
            .count();
        let exp_land = exp
            .game_events
            .iter()
            .filter(|e| e.id == "minecraft:projectile_land")
            .count();
        if em.health.to_bits() != exp.target_health.to_bits()
            || teleported != exp.target_teleported
            || behind_hp.to_bits() != exp.behind_pig_health.to_bits()
            || hit_snd != 0
            || got_land != exp_land
        {
            failures.push(format!(
                "122 mob_trident t={}: em_hp={} tp={teleported} behind_hp={behind_hp} snd={hit_snd} land={got_land}",
                exp.tick, em.health
            ));
            break;
        }
    }

    assert!(
        failures.is_empty(),
        "gap #122 failures:\n{}",
        failures.join("\n")
    );
}

#[test]
#[ignore = "daily"]
fn witness_piercing_arrow_multihit_123() {
    let witness = load_witness();
    let mut resources = None;
    let mut game = setup_game(&mut resources);
    let mut failures = Vec::new();
    let xbow = r#"{id:"minecraft:crossbow",count:1}"#;

    for case in &witness.gap_123_piercing {
        let label = format!("123 {}", case.case);
        let wall_x = if case.case == "pierce_through_mob_into_wall_same_tick" {
            4
        } else {
            11
        };
        clear_arena(&mut game, wall_x);
        game.world.random.set_seed(witness.level_seed);

        let mut tracked_mobs: Vec<(bool, u32)> = Vec::new();
        match case.case.as_str() {
            "pierce_two_mobs_same_tick" => {
                assert!(game.run_command(
                    "summon minecraft:chicken 2.5 64.0 4.5 {NoAI:1b,Silent:1b,Health:1.0f}"
                ));
                tracked_mobs.push((true, game.mobs.last().unwrap().id));
                assert!(game.run_command(
                    "summon minecraft:pig 3.8 64.0 4.5 {NoAI:1b,Silent:1b,Health:1.0f}"
                ));
                tracked_mobs.push((true, game.mobs.last().unwrap().id));
            }
            "pierce_two_phantoms_two_birds" => {
                assert!(game.run_command(
                    "summon minecraft:phantom 2.5 64.3 4.5 {NoAI:1b,Silent:1b,Health:1.0f}"
                ));
                tracked_mobs.push((true, game.mobs.last().unwrap().id));
                assert!(game.run_command(
                    "summon minecraft:phantom 3.8 64.3 4.5 {NoAI:1b,Silent:1b,Health:1.0f}"
                ));
                tracked_mobs.push((true, game.mobs.last().unwrap().id));
            }
            "pierce_five_unique_arbalistic" => {
                for (kind, x) in [
                    ("pig", "2.5"),
                    ("cow", "3.5"),
                    ("sheep", "4.5"),
                    ("chicken", "5.8"),
                    ("villager", "7.0"),
                ] {
                    let cmd = format!(
                        "summon minecraft:{kind} {x} 64.0 4.5 {{NoAI:1b,Silent:1b,Health:1.0f}}"
                    );
                    assert!(game.run_command(&cmd), "{cmd}");
                    tracked_mobs.push((true, game.mobs.last().unwrap().id));
                }
            }
            "pierce_limit_stops_at_pierce_plus_one" => {
                for x in ["2.5", "3.5", "4.5"] {
                    let cmd = format!(
                        "summon minecraft:pig {x} 64.0 4.5 {{NoAI:1b,Silent:1b,Health:1.0f}}"
                    );
                    assert!(game.run_command(&cmd), "{cmd}");
                    tracked_mobs.push((true, game.mobs.last().unwrap().id));
                }
            }
            "pierce_through_mob_into_wall_same_tick" => {
                assert!(game.run_command(
                    "summon minecraft:pig 2.5 64.0 4.5 {NoAI:1b,Silent:1b,Health:1.0f}"
                ));
                tracked_mobs.push((true, game.mobs.last().unwrap().id));
            }
            "pierce_hits_falling_block_deflects" => {
                assert!(game.run_command(
                    r#"summon minecraft:falling_block 2.5 65.0 4.5 {BlockState:"minecraft:sand",Time:1,Motion:[0.0d,0.0d,0.0d]}"#
                ));
                tracked_mobs.push((false, game.falling.last().unwrap().id));
                assert!(game.run_command(
                    "summon minecraft:zombie 3.8 64.0 4.5 {NoAI:1b,Silent:1b,Health:1.0f}"
                ));
                tracked_mobs.push((true, game.mobs.last().unwrap().id));
            }
            "pierce_hits_breeze_deflects" => {
                assert!(
                    game.run_command("summon minecraft:breeze 2.5 64.0 4.5 {NoAI:1b,Silent:1b}")
                );
                tracked_mobs.push((true, game.mobs.last().unwrap().id));
                assert!(game.run_command(
                    "summon minecraft:pig 3.8 64.0 4.5 {NoAI:1b,Silent:1b,Health:1.0f}"
                ));
                tracked_mobs.push((true, game.mobs.last().unwrap().id));
            }
            other => panic!("unexpected case {other}"),
        }

        let owner = player_uuid_nbt(&game);
        let arrow_cmd = match case.case.as_str() {
            "pierce_two_mobs_same_tick"
            | "pierce_two_phantoms_two_birds"
            | "pierce_through_mob_into_wall_same_tick"
            | "pierce_hits_breeze_deflects" => format!(
                "summon minecraft:arrow 1.5 64.5 4.5 {{Motion:[3.0d,0.0d,0.0d],damage:20.0d,PierceLevel:2b,Owner:{owner},LeftOwner:1b,weapon:{xbow}}}"
            ),
            "pierce_five_unique_arbalistic" => format!(
                "summon minecraft:arrow 1.5 64.5 4.5 {{Motion:[3.5d,0.0d,0.0d],damage:20.0d,PierceLevel:4b,Owner:{owner},LeftOwner:1b,weapon:{xbow}}}"
            ),
            "pierce_limit_stops_at_pierce_plus_one" => format!(
                "summon minecraft:arrow 1.5 64.5 4.5 {{Motion:[4.0d,0.0d,0.0d],damage:20.0d,PierceLevel:1b,Owner:{owner},LeftOwner:1b,weapon:{xbow}}}"
            ),
            "pierce_hits_falling_block_deflects" => format!(
                "summon minecraft:arrow 1.5 65.3 4.5 {{Motion:[3.0d,0.0d,0.0d],damage:20.0d,PierceLevel:2b,Owner:{owner},LeftOwner:1b,weapon:{xbow}}}"
            ),
            other => panic!("unexpected case {other}"),
        };
        assert!(game.run_command(&arrow_cmd), "{arrow_cmd}");
        let idx = game.creatures.projectiles.len() - 1;
        let arrow_id = game.creatures.projectiles[idx].id;
        game.creatures.projectiles.update(idx, |p| {
            p.entity_random().set_seed(witness.proj_seed);
        });

        for exp in &case.ticks {
            let (events, sounds) = step_recording(&mut game);
            let live = game.creatures.projectiles.iter().find(|p| p.id == arrow_id);
            if live.is_some() != exp.proj_alive {
                failures.push(format!(
                    "{label} t={}: proj_alive expected {} got {}",
                    exp.tick,
                    exp.proj_alive,
                    live.is_some()
                ));
                break;
            }
            if let Some(p) = live {
                if vec_bits(p.position) != exp.proj_pos_bits {
                    failures.push(format!(
                        "{label} t={}: pos expected {:?} got {:?} ({:?})",
                        exp.tick,
                        exp.proj_pos_bits,
                        vec_bits(p.position),
                        p.position
                    ));
                    break;
                }
                if vec_bits(p.velocity) != exp.proj_vel_bits {
                    failures.push(format!(
                        "{label} t={}: vel expected {:?} got {:?} ({:?})",
                        exp.tick,
                        exp.proj_vel_bits,
                        vec_bits(p.velocity),
                        p.velocity
                    ));
                    break;
                }
                let ig = p.arrow.as_ref().is_some_and(|a| a.in_ground);
                if ig != exp.in_ground {
                    failures.push(format!(
                        "{label} t={}: in_ground expected {} got {ig}",
                        exp.tick, exp.in_ground
                    ));
                    break;
                }
            }
            let mut target_err = false;
            for (k, ((is_mob, tid), exp_t)) in tracked_mobs.iter().zip(&exp.targets).enumerate() {
                let (alive, hp) = if *is_mob {
                    match game.mobs.iter().find(|m| m.id == *tid) {
                        Some(m) => (m.alive(), m.health),
                        None => (false, 0.0),
                    }
                } else {
                    (game.falling.iter().any(|f| f.id == *tid), -1.0)
                };
                if alive != exp_t.alive || hp.to_bits() != exp_t.health.to_bits() {
                    failures.push(format!(
                        "{label} t={} target[{k}]: expected (alive={}, hp={}) got ({alive}, {hp})",
                        exp.tick, exp_t.alive, exp_t.health
                    ));
                    target_err = true;
                    break;
                }
            }
            if target_err {
                break;
            }
            let two_birds = adv_done(&game, "minecraft:adventure/two_birds_one_arrow");
            if two_birds != exp.two_birds_done {
                failures.push(format!(
                    "{label} t={}: two_birds_one_arrow expected {} got {two_birds}",
                    exp.tick, exp.two_birds_done
                ));
                break;
            }
            let arbalistic = adv_done(&game, "minecraft:adventure/arbalistic");
            if arbalistic != exp.arbalistic_done {
                failures.push(format!(
                    "{label} t={}: arbalistic expected {} got {arbalistic}",
                    exp.tick, exp.arbalistic_done
                ));
                break;
            }
            let exp_land = exp
                .game_events
                .iter()
                .filter(|e| e.id == "minecraft:projectile_land")
                .count();
            let got_land = events
                .iter()
                .filter(|&&e| e == "minecraft:projectile_land")
                .count();
            if got_land != exp_land {
                failures.push(format!(
                    "{label} t={}: projectile_land count expected {exp_land} got {got_land}",
                    exp.tick
                ));
                break;
            }
            for snd in [
                "minecraft:entity.arrow.hit",
                "minecraft:entity.breeze.deflect",
            ] {
                let exp_n = exp.sounds.iter().filter(|s| s.as_str() == snd).count();
                let got_n = sounds.get(snd).copied().unwrap_or(0);
                if got_n != exp_n {
                    failures.push(format!(
                        "{label} t={}: {snd} count expected {exp_n} got {got_n}",
                        exp.tick
                    ));
                    target_err = true;
                    break;
                }
            }
            if target_err {
                break;
            }
        }
    }
    assert!(
        failures.is_empty(),
        "gap #123 failures:\n{}",
        failures.join("\n")
    );
}

#[derive(Clone, Copy)]
enum Target124 {
    Boat(u32),
    Cart(u32),
    Frame(u32),
    Painting(u32),
    Falling(u32),
    Knot(u32),
    Cushion(u32),
    Crystal(u32),
    Tnt(u32),
    Stand(u32),
}

fn alive_124(game: &Game<'_>, t: Target124) -> bool {
    match t {
        Target124::Boat(id) => game.boats.iter().any(|b| b.id == id),
        Target124::Cart(id) => game.carts.iter().any(|c| c.id == id),
        Target124::Frame(id) => game.item_frames.iter().any(|f| f.id == id),
        Target124::Painting(id) => game.paintings.iter().any(|p| p.id == id),
        Target124::Falling(id) => game.falling.iter().any(|f| f.id == id),
        Target124::Knot(id) => game.others.iter().any(|o| o.id == id),
        Target124::Cushion(id) => game.cushions.iter().any(|c| c.id == id),
        Target124::Crystal(id) => game.crystals.iter().any(|c| c.id == id),
        Target124::Tnt(id) => game.tnts.iter().any(|t| t.id == id),
        Target124::Stand(id) => game.armor_stands.iter().any(|a| a.id == id),
    }
}

#[test]
#[ignore = "daily"]
fn witness_firework_rocket_targets_124() {
    let witness = load_witness();
    let mut resources = None;
    let mut game = setup_game(&mut resources);
    let mut failures = Vec::new();

    for case in &witness.gap_124_fireworks {
        let label = format!("124 {}", case.case);
        clear_arena(&mut game, 10);
        game.world.random.set_seed(witness.level_seed);

        if case.case == "item_frame" || case.case == "painting" {
            let _ = game.run_command("fill 4 64 2 4 66 6 minecraft:stone");
        }
        if case.case == "leash_knot" {
            let _ = game.run_command("setblock 3 64 4 minecraft:oak_fence");
        }
        if case.case == "armor_stand_blast_nearby" {
            let _ = game.run_command("fill 6 64 3 6 66 5 minecraft:stone");
        }

        let mut targets: BTreeMap<String, Target124> = BTreeMap::new();
        match case.case.as_str() {
            "oak_boat" | "oak_boat_no_star" => {
                assert!(game.run_command("summon minecraft:oak_boat 3.5 64.0 4.5 {}"));
                targets.insert(
                    "boat".into(),
                    Target124::Boat(game.boats.last().unwrap().id),
                );
            }
            "minecart" => {
                assert!(game.run_command("summon minecraft:minecart 3.5 64.0 4.5 {}"));
                targets.insert(
                    "minecart".into(),
                    Target124::Cart(game.carts.last().unwrap().id),
                );
            }
            "item_frame" => {
                assert!(game.run_command(
                    r#"summon minecraft:item_frame 3.5 64.5 4.5 {block_pos:[I;3,64,4],Facing:4b,Item:{id:"minecraft:diamond",count:1}}"#
                ));
                targets.insert(
                    "frame".into(),
                    Target124::Frame(game.item_frames.last().unwrap().id),
                );
            }
            "painting" => {
                assert!(game.run_command(
                    r#"summon minecraft:painting 3.5 64.5 4.5 {block_pos:[I;3,64,4],facing:1b,variant:"minecraft:kebab"}"#
                ));
                targets.insert(
                    "painting".into(),
                    Target124::Painting(game.paintings.last().unwrap().id),
                );
            }
            "falling_block" => {
                assert!(game.run_command(
                    r#"summon minecraft:falling_block 3.5 65.0 4.5 {BlockState:"minecraft:sand",Time:1,Motion:[0.0d,0.0d,0.0d]}"#
                ));
                targets.insert(
                    "falling".into(),
                    Target124::Falling(game.falling.last().unwrap().id),
                );
            }
            "leash_knot" => {
                assert!(game.run_command("summon minecraft:leash_knot 3.5 64.375 4.5 {}"));
                targets.insert(
                    "knot".into(),
                    Target124::Knot(game.others.last().unwrap().id),
                );
            }
            "cushion" => {
                assert!(game.run_command(
                    r#"summon minecraft:cushion 3.5 64.0 4.5 {block_pos:[I;3,64,4],color:"white"}"#
                ));
                targets.insert(
                    "cushion".into(),
                    Target124::Cushion(game.cushions.last().unwrap().id),
                );
            }
            "end_crystal" => {
                assert!(game.run_command("summon minecraft:end_crystal 4.25 64.0 4.5 {}"));
                targets.insert(
                    "crystal".into(),
                    Target124::Crystal(game.crystals.last().unwrap().id),
                );
            }
            "tnt" => {
                assert!(game.run_command("summon minecraft:tnt 3.5 64.0 4.5 {fuse:80}"));
                targets.insert("tnt".into(), Target124::Tnt(game.tnts.last().unwrap().id));
            }
            "armor_stand" => {
                assert!(game.run_command("summon minecraft:armor_stand 3.5 64.0 4.5 {}"));
                targets.insert(
                    "stand".into(),
                    Target124::Stand(game.armor_stands.last().unwrap().id),
                );
            }
            "armor_stand_marker" => {
                assert!(game.run_command("summon minecraft:armor_stand 3.5 64.0 4.5 {Marker:1b}"));
                targets.insert(
                    "stand".into(),
                    Target124::Stand(game.armor_stands.last().unwrap().id),
                );
            }
            "armor_stand_blast_nearby" => {
                assert!(game.run_command("summon minecraft:armor_stand 5.5 64.0 4.5 {}"));
                targets.insert(
                    "normal".into(),
                    Target124::Stand(game.armor_stands.last().unwrap().id),
                );
                assert!(
                    game.run_command("summon minecraft:armor_stand 3.5 64.0 6.5 {Invisible:1b}")
                );
                targets.insert(
                    "invisible".into(),
                    Target124::Stand(game.armor_stands.last().unwrap().id),
                );
                assert!(game.run_command("summon minecraft:armor_stand 3.5 64.0 2.5 {Marker:1b}"));
                targets.insert(
                    "marker".into(),
                    Target124::Stand(game.armor_stands.last().unwrap().id),
                );
                assert!(game.run_command("summon minecraft:armor_stand 7.5 64.0 4.5 {}"));
                targets.insert(
                    "occluded".into(),
                    Target124::Stand(game.armor_stands.last().unwrap().id),
                );
                assert!(game.run_command("summon minecraft:armor_stand 9.5 64.0 4.5 {}"));
                targets.insert(
                    "far".into(),
                    Target124::Stand(game.armor_stands.last().unwrap().id),
                );
            }
            other => panic!("unexpected case {other}"),
        }

        let rocket_cmd = if case.case == "armor_stand_blast_nearby" {
            format!(
                "summon minecraft:firework_rocket 3.5 64.5 4.5 {{LifeTime:1,ShotAtAngle:1b,Motion:[0.0d,0.0d,0.0d],FireworksItem:{STAR_FIREWORK_ITEM}}}"
            )
        } else {
            let plain = matches!(
                case.case.as_str(),
                "oak_boat_no_star" | "item_frame" | "painting" | "leash_knot"
            );
            let item = if plain {
                PLAIN_FIREWORK_ITEM
            } else {
                STAR_FIREWORK_ITEM
            };
            format!(
                "summon minecraft:firework_rocket 1.5 {} 4.5 {{LifeTime:20,ShotAtAngle:1b,Motion:[1.0d,0.0d,0.0d],FireworksItem:{item}}}",
                case.spawn_y
            )
        };
        assert!(game.run_command(&rocket_cmd), "{rocket_cmd}");
        let idx = game.rockets.len() - 1;
        let rocket_id = game.rockets[idx].id;
        game.rockets.update(idx, |r| {
            r.random = LegacyRandom::new(witness.proj_seed);
        });

        for exp in &case.ticks {
            let (events, sounds) = step_recording(&mut game);
            let live = game.rockets.iter().find(|r| r.id == rocket_id);
            if live.is_some() != exp.rocket_alive {
                failures.push(format!(
                    "{label} t={}: rocket_alive expected {} got {}",
                    exp.tick,
                    exp.rocket_alive,
                    live.is_some()
                ));
                break;
            }
            if let Some(r) = live
                && (vec_bits(r.position) != exp.rocket_pos_bits
                    || vec_bits(r.velocity) != exp.rocket_vel_bits)
            {
                failures.push(format!(
                    "{label} t={}: pos/vel expected {:?}/{:?} got {:?}/{:?}",
                    exp.tick,
                    exp.rocket_pos_bits,
                    exp.rocket_vel_bits,
                    vec_bits(r.position),
                    vec_bits(r.velocity)
                ));
                break;
            }
            let got_alive: BTreeMap<String, bool> = targets
                .iter()
                .map(|(k, &t)| (k.clone(), alive_124(&game, t)))
                .collect();
            if got_alive != exp.targets_alive {
                failures.push(format!(
                    "{label} t={}: targets_alive expected {:?} got {got_alive:?}",
                    exp.tick, exp.targets_alive
                ));
                break;
            }
            let mut got_drops: Vec<String> = game
                .dropped
                .iter()
                .map(|d| d.stack.item.to_string())
                .collect();
            got_drops.sort();
            if got_drops != exp.drops {
                failures.push(format!(
                    "{label} t={}: drops expected {:?} got {got_drops:?}",
                    exp.tick, exp.drops
                ));
                break;
            }
            let mut ev_err = false;
            for key in ["minecraft:explode", "minecraft:projectile_land"] {
                let exp_n = exp.game_events.iter().filter(|e| e.id == key).count();
                let got_n = events.iter().filter(|&&e| e == key).count();
                if got_n != exp_n {
                    failures.push(format!(
                        "{label} t={}: {key} count expected {exp_n} got {got_n}",
                        exp.tick
                    ));
                    ev_err = true;
                    break;
                }
            }
            if ev_err {
                break;
            }
            let exp_break = exp
                .sounds
                .iter()
                .filter(|s| s.as_str() == "minecraft:entity.armor_stand.break")
                .count();
            let got_break = sounds
                .get("minecraft:entity.armor_stand.break")
                .copied()
                .unwrap_or(0);
            if got_break != exp_break {
                failures.push(format!(
                    "{label} t={}: armor_stand.break count expected {exp_break} got {got_break}",
                    exp.tick
                ));
                break;
            }
        }
    }
    assert!(
        failures.is_empty(),
        "gap #124 failures:\n{}",
        failures.join("\n")
    );
}
