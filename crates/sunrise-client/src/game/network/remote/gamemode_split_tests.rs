//! Red parity tests for Minecraft 26.3 client local-game-mode vs `PlayerInfo` split (Item 3).
//! Witnesses:
//! - `tools/witness/26.3/genpvp_menu/{settled.json,ticks.jsonl}` (real 26.3 client replay)
//! - `tools/witness/26.3/gamemode_split/witness.json` (`VanillaClientGameModeSplitOracle`, JDK 25)
use super::super::*;
use crate::game::game_mode::mode_from_id;
use crate::game::native_entity_refs::tests::with_client_game;
use serde_json::Value as JsonValue;
use std::path::Path;
use sunrise_client::command::args::GameMode;
use sunrise_protocol::connection::ConnReader;

const ORACLE_WITNESS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tools/witness/26.3/gamemode_split/witness.json"
));
const SELF_UUID: u128 = 0xdf71_dcc7_da3b_3506_b229_6d1f_589e_2569;
const SELF_NAME: &str = "BX0W";

fn f32_bits(hex: &JsonValue) -> u32 {
    u32::from_str_radix(hex.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
}

fn f64_bits(hex: &JsonValue) -> u64 {
    u64::from_str_radix(hex.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
}

fn mode_id_from_name(name: &str) -> i32 {
    GameMode::from_name(name).unwrap().id()
}

fn spawn_info(game_type: i32, previous_game_type: Option<i32>) -> play::SpawnInfo {
    play::SpawnInfo {
        dimension_type: 0,
        dimension: "minecraft:overworld".into(),
        seed: 0,
        game_type,
        previous_game_type,
        debug: false,
        flat: true,
        last_death: None,
        portal_cooldown: 0,
        sea_level: -63,
    }
}

fn login_packet(player_id: i32, game_type: i32, previous_game_type: Option<i32>) -> C {
    C::Login(play::Login {
        player_id,
        hardcore: false,
        levels: vec!["minecraft:overworld".into()],
        max_players: 20,
        chunk_radius: 2,
        simulation_distance: 2,
        reduced_debug_info: false,
        show_death_screen: true,
        limited_crafting: false,
        spawn: spawn_info(game_type, previous_game_type),
        online_mode: false,
        enforces_secure_chat: false,
    })
}

fn position_packet(teleport: i32, position: [f64; 3]) -> C {
    C::PlayerPosition(play::PlayerPosition {
        teleport,
        position,
        delta: [0.0; 3],
        yaw: 0.0,
        pitch: 0.0,
        relatives: 0,
    })
}

fn abilities_packet(flags: i8, flying_speed: f32, walking_speed: f32) -> C {
    C::PlayerAbilities(play::PlayerAbilities {
        flags,
        flying_speed,
        walking_speed,
    })
}

fn change_mode_packet(param: f32) -> C {
    C::GameEvent(play::GameEvent { event: 3, param })
}

fn player_info_add_mode(uuid: u128, name: &str, game_mode: i32) -> C {
    C::PlayerInfoUpdate(play::PlayerInfoUpdate {
        actions: 0x01 | 0x04 | 0x08,
        entries: vec![play::PlayerInfo {
            uuid,
            name: Some(name.into()),
            game_mode: Some(game_mode),
            listed: Some(true),
            ..play::PlayerInfo::default()
        }],
    })
}

fn player_info_mode(uuid: u128, game_mode: i32) -> C {
    C::PlayerInfoUpdate(play::PlayerInfoUpdate {
        actions: 0x04,
        entries: vec![play::PlayerInfo {
            uuid,
            game_mode: Some(game_mode),
            ..play::PlayerInfo::default()
        }],
    })
}

fn install_stone_floor(game: &mut Game<'_>) {
    game.streaming
        .insert_remote(&mut game.world, Chunk::empty(0, 0));
    let stone = game
        .pack
        .assets
        .registry
        .default_state("minecraft:stone")
        .unwrap();
    for x in 0..=1 {
        for z in 0..=1 {
            for y in 62..=63 {
                game.world.set(x, y, z, stone).unwrap();
            }
        }
    }
}

fn setup_client(
    game: &mut Game<'_>,
    local_mode: i32,
    previous_mode: Option<i32>,
    pos: [f64; 3],
) -> (ConnReader, sunrise_protocol::connection::ConnWriter) {
    let (peer, writer) = super::super::tests::connect_menu_test(game);
    peer.set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    {
        let remote = game.remote.as_mut().unwrap();
        remote.profile = SELF_UUID;
        remote.profile_name = SELF_NAME.into();
    }
    game.handle_packet(login_packet(83081, local_mode, previous_mode))
        .unwrap();
    install_stone_floor(game);
    game.handle_packet(position_packet(1, pos)).unwrap();
    {
        let remote = game.remote.as_mut().unwrap();
        remote.loaded_sent = true;
        remote.level_load.finish();
    }
    game.screen = Screen::Playing;
    (peer, writer)
}

fn drain_sent_abilities(game: &Game<'_>, peer: &mut ConnReader) -> Vec<bool> {
    game.send(S::ClientTickEnd(Empty));
    let mut sent = Vec::new();
    while let Ok((id, bytes)) = peer.recv() {
        match S::decode(id, &bytes) {
            Ok(S::ClientTickEnd(_)) => break,
            Ok(S::PlayerAbilities(a)) => sent.push(a.flags & 2 != 0),
            _ => {}
        }
    }
    sent
}

fn run_local_tick(game: &mut Game<'_>) {
    game.tick_local_player(Vec::new(), 7).unwrap();
}

fn player_info_mode_name(game: &Game<'_>) -> Option<&'static str> {
    let r = game.remote.as_ref()?;
    let t = r.tab_players.get(&r.profile)?;
    Some(mode_from_id(t.game_mode).name())
}

fn assert_row_core(
    game: &Game<'_>,
    row: &JsonValue,
    check_pos_vel: bool,
    sent_abilities: Option<&[bool]>,
) {
    let label = row["step"].as_str().unwrap();
    let local_ref = game.local_player().expect("local_player handle");
    let ab = local_ref.abilities(game).expect("client abilities");
    let p = game.player.as_ref().expect("local player present");
    let want_ab = &row["abilities"];

    assert_eq!(
        game.mode.name(),
        row["local_game_mode"].as_str().unwrap(),
        "{label}: local_game_mode"
    );
    assert_eq!(
        game.previous_mode.map(GameMode::name),
        row["previous_local_game_mode"].as_str(),
        "{label}: previous_local_game_mode"
    );
    assert_eq!(
        player_info_mode_name(game),
        row["player_info_game_mode"].as_str(),
        "{label}: player_info_game_mode"
    );
    assert_eq!(
        p.spectator,
        row["is_spectator"].as_bool().unwrap(),
        "{label}: is_spectator"
    );
    assert_eq!(
        p.spectator,
        row["no_physics"].as_bool().unwrap(),
        "{label}: no_physics"
    );
    assert_eq!(
        p.ground.on_ground,
        row["on_ground"].as_bool().unwrap(),
        "{label}: on_ground"
    );
    assert_eq!(
        (p.invulnerable, ab.invulnerable),
        (
            want_ab["invulnerable"].as_bool().unwrap(),
            want_ab["invulnerable"].as_bool().unwrap()
        ),
        "{label}: abilities.invulnerable"
    );
    assert_eq!(
        (p.flying, ab.flying),
        (
            want_ab["flying"].as_bool().unwrap(),
            want_ab["flying"].as_bool().unwrap()
        ),
        "{label}: abilities.flying"
    );
    assert_eq!(
        (p.may_fly, ab.may_fly),
        (
            want_ab["may_fly"].as_bool().unwrap(),
            want_ab["may_fly"].as_bool().unwrap()
        ),
        "{label}: abilities.may_fly"
    );
    assert_eq!(
        (game.instabuild(), ab.instabuild),
        (
            want_ab["instabuild"].as_bool().unwrap(),
            want_ab["instabuild"].as_bool().unwrap()
        ),
        "{label}: abilities.instabuild"
    );
    assert_eq!(
        (game.may_build(), ab.may_build),
        (
            want_ab["may_build"].as_bool().unwrap(),
            want_ab["may_build"].as_bool().unwrap()
        ),
        "{label}: abilities.may_build"
    );
    let want_fly_bits = f32_bits(&want_ab["fly_speed_bits"]);
    let want_walk_bits = f32_bits(&want_ab["walk_speed_bits"]);
    assert_eq!(
        (p.flying_speed.to_bits(), ab.fly_speed.to_bits()),
        (want_fly_bits, want_fly_bits),
        "{label}: abilities.fly_speed_bits"
    );
    assert_eq!(
        (
            game.remote_walking_speed().to_bits(),
            ab.walk_speed.to_bits()
        ),
        (want_walk_bits, want_walk_bits),
        "{label}: abilities.walk_speed_bits"
    );
    if check_pos_vel {
        let want_pos = row["pos_bits"].as_array().unwrap();
        let got_pos = [
            p.position.x.to_bits(),
            p.position.y.to_bits(),
            p.position.z.to_bits(),
        ];
        let want_pos = [
            f64_bits(&want_pos[0]),
            f64_bits(&want_pos[1]),
            f64_bits(&want_pos[2]),
        ];
        assert_eq!(got_pos, want_pos, "{label}: pos_bits");

        let want_vel = row["vel_bits"].as_array().unwrap();
        let got_vel = [
            p.velocity.x.to_bits(),
            p.velocity.y.to_bits(),
            p.velocity.z.to_bits(),
        ];
        let want_vel = [
            f64_bits(&want_vel[0]),
            f64_bits(&want_vel[1]),
            f64_bits(&want_vel[2]),
        ];
        assert_eq!(got_vel, want_vel, "{label}: vel_bits");
    }
    if let Some(got_sent) = sent_abilities {
        let want_sent: Vec<bool> = row["sent_abilities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_bool().unwrap())
            .collect();
        assert_eq!(got_sent, want_sent.as_slice(), "{label}: sent_abilities");
    }
}

/// Real 26.3 client menu witness (`tools/witness/26.3/genpvp_menu/{settled.json,ticks.jsonl}`):
/// after the genpvp join sequence (`CHANGE_GAME_MODE` Spectator, `PlayerInfo` Adventure,
/// `PlayerAbilities` 0x00) and 100 idle ticks over solid ground, the local player matches the
/// recorded vanilla `player.*` state bit for bit instead of falling into the void.
#[test]
#[ignore = "daily"]
fn genpvp_menu_settled_witness_player_mode_and_physics_26_3() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/witness/26.3/genpvp_menu");
    let settled: JsonValue = serde_json::from_str(
        &std::fs::read_to_string(dir.join("settled.json"))
            .expect("genpvp_menu/settled.json witness must exist"),
    )
    .unwrap();
    let ticks_raw = std::fs::read_to_string(dir.join("ticks.jsonl"))
        .expect("genpvp_menu/ticks.jsonl witness must exist");
    let last_tick: JsonValue = serde_json::from_str(
        ticks_raw
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .expect("ticks.jsonl non-empty"),
    )
    .unwrap();
    assert_eq!(settled["meta"]["game_version"].as_str(), Some("26.3"));
    let wp = &settled["player"];

    with_client_game(|game| {
        let (mut peer, _writer) = super::super::tests::connect_menu_test(game);
        peer.set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        {
            let remote = game.remote.as_mut().unwrap();
            remote.profile = SELF_UUID;
            remote.profile_name = SELF_NAME.into();
        }
        // Replay genpvp's exact join packet order up to the settled menu state.
        game.handle_packet(login_packet(83081, 1, None)).unwrap();
        install_stone_floor(game);
        game.handle_packet(abilities_packet(0x0d, 0.05, 0.10))
            .unwrap();
        game.handle_packet(position_packet(1, [0.5, 64.0, 0.5]))
            .unwrap();
        game.handle_packet(player_info_add_mode(SELF_UUID, SELF_NAME, 1))
            .unwrap();
        game.handle_packet(abilities_packet(0x00, 0.05, 0.10))
            .unwrap();
        game.handle_packet(player_info_mode(SELF_UUID, 2)).unwrap();
        game.handle_packet(change_mode_packet(2.0)).unwrap();
        game.handle_packet(abilities_packet(0x00, 0.05, 0.10))
            .unwrap();
        game.handle_packet(position_packet(2, [0.5, 64.0, 0.5]))
            .unwrap();
        game.handle_packet(change_mode_packet(1.0)).unwrap();
        game.handle_packet(abilities_packet(0x0d, 0.0, 0.0))
            .unwrap();
        game.handle_packet(change_mode_packet(3.0)).unwrap();
        game.handle_packet(abilities_packet(0x00, 0.0, 0.0))
            .unwrap();
        {
            let remote = game.remote.as_mut().unwrap();
            remote.loaded_sent = true;
            remote.level_load.finish();
        }
        game.screen = Screen::Playing;
        let _ = drain_sent_abilities(game, &mut peer);

        for _ in 0..100 {
            run_local_tick(game);
        }
        let sent = drain_sent_abilities(game, &mut peer);
        assert!(sent.is_empty(), "no flight toggle packets during idle menu");

        let ab = game.local_player().unwrap().abilities(&*game).unwrap();
        let p = game.player.as_ref().unwrap();
        let want_pos = wp["pos_bits"].as_array().unwrap();
        assert_eq!(
            [
                p.position.x.to_bits(),
                p.position.y.to_bits(),
                p.position.z.to_bits(),
            ],
            [
                f64_bits(&want_pos[0]),
                f64_bits(&want_pos[1]),
                f64_bits(&want_pos[2]),
            ],
            "settled player.pos_bits"
        );
        assert_eq!(
            p.ground.on_ground,
            wp["on_ground"].as_bool().unwrap(),
            "settled player.on_ground"
        );
        assert_eq!(
            p.spectator,
            wp["no_physics"].as_bool().unwrap(),
            "settled player.no_physics"
        );
        assert_eq!(
            p.spectator,
            wp["is_spectator"].as_bool().unwrap(),
            "settled player.is_spectator"
        );
        assert_eq!(
            game.mode.name().to_ascii_lowercase(),
            wp["local_game_mode"].as_str().unwrap().to_ascii_lowercase(),
            "settled player.local_game_mode"
        );
        assert_eq!(
            player_info_mode_name(game).map(str::to_ascii_lowercase),
            Some(
                wp["player_info_game_mode"]
                    .as_str()
                    .unwrap()
                    .to_ascii_lowercase()
            ),
            "settled player.player_info_game_mode"
        );
        let wab = &wp["abilities"];
        assert_eq!(
            (p.flying, ab.flying),
            (
                wab["flying"].as_bool().unwrap(),
                wab["flying"].as_bool().unwrap()
            ),
            "settled abilities.flying"
        );
        assert_eq!(
            (p.may_fly, ab.may_fly),
            (
                wab["may_fly"].as_bool().unwrap(),
                wab["may_fly"].as_bool().unwrap()
            ),
            "settled abilities.may_fly"
        );
        assert_eq!(
            (p.invulnerable, ab.invulnerable),
            (
                wab["invulnerable"].as_bool().unwrap(),
                wab["invulnerable"].as_bool().unwrap()
            ),
            "settled abilities.invulnerable"
        );
        assert_eq!(
            (game.instabuild(), ab.instabuild),
            (
                wab["instabuild"].as_bool().unwrap(),
                wab["instabuild"].as_bool().unwrap()
            ),
            "settled abilities.instabuild"
        );
        assert_eq!(
            (p.flying_speed.to_bits(), ab.fly_speed.to_bits()),
            (
                f32_bits(&wab["fly_speed_bits"]),
                f32_bits(&wab["fly_speed_bits"])
            ),
            "settled abilities.fly_speed_bits"
        );
        assert_eq!(
            (
                game.remote_walking_speed().to_bits(),
                ab.walk_speed.to_bits()
            ),
            (
                f32_bits(&wab["walk_speed_bits"]),
                f32_bits(&wab["walk_speed_bits"])
            ),
            "settled abilities.walk_speed_bits"
        );

        let tick_pos = last_tick["player_pos_bits"].as_array().unwrap();
        assert_eq!(
            [
                p.position.x.to_bits(),
                p.position.y.to_bits(),
                p.position.z.to_bits(),
            ],
            [
                f64_bits(&tick_pos[0]),
                f64_bits(&tick_pos[1]),
                f64_bits(&tick_pos[2]),
            ],
            "ticks.jsonl final player_pos_bits"
        );
        assert_eq!(
            p.ground.on_ground,
            last_tick["on_ground"].as_bool().unwrap(),
            "ticks.jsonl final on_ground"
        );
        assert_eq!(
            p.spectator,
            last_tick["no_physics"].as_bool().unwrap(),
            "ticks.jsonl final no_physics"
        );
        assert_eq!(
            game.mode.name().to_ascii_lowercase(),
            last_tick["local_game_mode"]
                .as_str()
                .unwrap()
                .to_ascii_lowercase(),
            "ticks.jsonl final local_game_mode"
        );
        assert_eq!(
            player_info_mode_name(game).map(str::to_ascii_lowercase),
            last_tick["player_info_game_mode"]
                .as_str()
                .map(str::to_ascii_lowercase),
            "ticks.jsonl final player_info_game_mode"
        );
    });
}

/// Real 26.3 oracle witness (`tools/witness/26.3/gamemode_split/witness.json`):
/// step-by-step replay of the genpvp packet sequence plus 5 ticks on stone, verifying every
/// mode, ability (including `may_build`), physics flag, and exact position/velocity bits.
#[test]
#[ignore = "daily"]
fn genpvp_packet_sequence_matches_26_3_oracle_step_by_step() {
    let witness: JsonValue = serde_json::from_str(ORACLE_WITNESS).unwrap();
    assert_eq!(witness["version"].as_str(), Some("26.3"));
    let rows = witness["scenarios"]["genpvp_packet_sequence"]
        .as_array()
        .unwrap();

    with_client_game(|game| {
        let (mut peer, _writer) = setup_client(game, 1, None, [0.5, 64.0, 0.5]);
        let _ = drain_sent_abilities(game, &mut peer);
        assert_row_core(game, &rows[0], true, Some(&[]));

        let packets = [
            abilities_packet(0x0d, 0.05, 0.10),
            player_info_add_mode(SELF_UUID, SELF_NAME, 1),
            abilities_packet(0x00, 0.05, 0.10),
            player_info_mode(SELF_UUID, 2),
            change_mode_packet(2.0),
            abilities_packet(0x00, 0.05, 0.10),
            change_mode_packet(1.0),
            abilities_packet(0x0d, 0.0, 0.0),
            change_mode_packet(3.0),
            abilities_packet(0x00, 0.0, 0.0),
        ];
        for (idx, pkt) in packets.into_iter().enumerate() {
            game.handle_packet(pkt).unwrap();
            let sent = drain_sent_abilities(game, &mut peer);
            assert_row_core(game, &rows[idx + 1], true, Some(&sent));
        }
        for tick in 0..5 {
            run_local_tick(game);
            let sent = drain_sent_abilities(game, &mut peer);
            assert_row_core(game, &rows[11 + tick], true, Some(&sent));
        }
    });
}

/// Real 26.3 oracle witness (`scenarios.change_game_mode_abilities`):
/// `CHANGE_GAME_MODE` calls only `MultiPlayerGameMode.setLocalMode` + `GameType.updatePlayerAbilities`
/// — never resetting `flying_speed`, never forcing `flying` on `Spectator -> Creative`, never
/// running server-side `in_range_of_ground` landing, and applying in exact packet order relative
/// to `PlayerAbilities`.
#[test]
#[ignore = "daily"]
fn change_game_mode_updates_only_local_mode_and_abilities_26_3() {
    let witness: JsonValue = serde_json::from_str(ORACLE_WITNESS).unwrap();
    assert_eq!(witness["version"].as_str(), Some("26.3"));
    let rows = witness["scenarios"]["change_game_mode_abilities"]
        .as_array()
        .unwrap();

    // Case 0: Spectator (flying=false, custom speeds) -> Creative keeps flying=false and speeds.
    with_client_game(|game| {
        let (_peer, _writer) = setup_client(game, 3, None, [0.5, 64.0, 0.5]);
        game.handle_packet(player_info_add_mode(SELF_UUID, SELF_NAME, 2))
            .unwrap();
        game.handle_packet(abilities_packet(0x00, 0.125, 0.2))
            .unwrap();
        game.handle_packet(change_mode_packet(1.0)).unwrap();
        assert_row_core(game, &rows[0], false, None);
    });

    // Case 1: Spectator (flying=true, y=64.5 within 1 block of stone) -> Creative does NOT land.
    with_client_game(|game| {
        let (_peer, _writer) = setup_client(game, 3, None, [0.5, 64.5, 0.5]);
        game.handle_packet(abilities_packet(0x07, 0.125, 0.2))
            .unwrap();
        game.handle_packet(change_mode_packet(1.0)).unwrap();
        assert_row_core(game, &rows[1], false, None);
    });

    // Cases 2..=8: PlayerAbilities then CHANGE_GAME_MODE order, fractional rounding, invalid ids.
    with_client_game(|game| {
        let (_peer, _writer) = setup_client(game, 0, None, [0.5, 64.0, 0.5]);
        game.handle_packet(abilities_packet(0x0f, 0.08, 0.15))
            .unwrap();
        game.handle_packet(change_mode_packet(0.0)).unwrap();
        assert_row_core(game, &rows[2], false, None);

        game.handle_packet(abilities_packet(0x0f, 0.08, 0.15))
            .unwrap();
        game.handle_packet(change_mode_packet(2.0)).unwrap();
        assert_row_core(game, &rows[3], false, None);

        for (offset, param) in [(4, 0.6_f32), (5, 1.6), (6, 2.6), (7, 4.0), (8, -1.0)] {
            game.handle_packet(change_mode_packet(param)).unwrap();
            assert_row_core(game, &rows[offset], false, None);
        }
    });

    // Cases 9..=10: CHANGE_GAME_MODE then PlayerAbilities preserves mayBuild.
    with_client_game(|game| {
        let (_peer, _writer) = setup_client(game, 0, None, [0.5, 64.0, 0.5]);
        game.handle_packet(change_mode_packet(2.0)).unwrap();
        game.handle_packet(abilities_packet(0x0f, 0.07, 0.12))
            .unwrap();
        assert_row_core(game, &rows[9], false, None);

        game.handle_packet(change_mode_packet(0.0)).unwrap();
        game.handle_packet(abilities_packet(0x00, 0.05, 0.10))
            .unwrap();
        assert_row_core(game, &rows[10], false, None);
    });

    // Case 11: repeating the same mode leaves previous_mode intact.
    with_client_game(|game| {
        let (_peer, _writer) = setup_client(game, 0, None, [0.5, 64.0, 0.5]);
        game.handle_packet(change_mode_packet(1.0)).unwrap();
        game.handle_packet(change_mode_packet(1.0)).unwrap();
        assert_row_core(game, &rows[11], false, None);
    });
}

/// Real 26.3 oracle witness (`scenarios.split_mode_physics_matrix`, 48 rows):
/// `Player.tick()` (`noPhysics`, `onGround`, solid collision) follows `PlayerInfo`, while
/// `LocalPlayer.aiStep()` (forced spectator flight when `may_fly`, keeping `flying` on landing)
/// follows `MultiPlayerGameMode.isSpectator()`.
#[test]
#[ignore = "daily"]
fn split_local_and_player_info_physics_matrix_26_3() {
    let witness: JsonValue = serde_json::from_str(ORACLE_WITNESS).unwrap();
    assert_eq!(witness["version"].as_str(), Some("26.3"));
    let rows = witness["scenarios"]["split_mode_physics_matrix"]
        .as_array()
        .unwrap();
    assert_eq!(rows.len(), 48);

    with_client_game(|game| {
        let (mut peer, _writer) = super::super::tests::connect_menu_test(game);
        peer.set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();

        let modes = ["survival", "creative", "adventure", "spectator"];
        let info_modes: [Option<&str>; 5] = [
            None,
            Some("survival"),
            Some("creative"),
            Some("adventure"),
            Some("spectator"),
        ];
        let mut row_idx = 0;

        for local in modes {
            for info in info_modes {
                for may_fly in [false, true] {
                    {
                        let remote = game.remote.as_mut().unwrap();
                        remote.profile = SELF_UUID;
                        remote.profile_name = SELF_NAME.into();
                        remote.tab_players.clear();
                        remote.listed.clear();
                    }
                    game.handle_packet(login_packet(83081, mode_id_from_name(local), None))
                        .unwrap();
                    install_stone_floor(game);
                    if let Some(info_name) = info {
                        game.handle_packet(player_info_add_mode(
                            SELF_UUID,
                            SELF_NAME,
                            mode_id_from_name(info_name),
                        ))
                        .unwrap();
                    }
                    game.handle_packet(position_packet(1, [0.5, 64.0, 0.5]))
                        .unwrap();
                    game.player.as_mut().unwrap().ground.on_ground = true;
                    game.handle_packet(abilities_packet(
                        if may_fly { 0x04 } else { 0x00 },
                        0.05,
                        0.10,
                    ))
                    .unwrap();
                    {
                        let remote = game.remote.as_mut().unwrap();
                        remote.loaded_sent = true;
                        remote.level_load.finish();
                    }
                    game.screen = Screen::Playing;
                    let _ = drain_sent_abilities(game, &mut peer);

                    for _ in 0..3 {
                        run_local_tick(game);
                    }
                    let sent = drain_sent_abilities(game, &mut peer);
                    assert_row_core(game, &rows[row_idx], true, Some(&sent));
                    row_idx += 1;
                }
            }
        }

        // 8 landing rows: flying=true with downward velocity -0.1 into stone floor at y=64.0.
        for local in modes {
            for info_name in ["adventure", "spectator"] {
                {
                    let remote = game.remote.as_mut().unwrap();
                    remote.profile = SELF_UUID;
                    remote.profile_name = SELF_NAME.into();
                    remote.tab_players.clear();
                    remote.listed.clear();
                }
                game.handle_packet(login_packet(83081, mode_id_from_name(local), None))
                    .unwrap();
                install_stone_floor(game);
                game.handle_packet(player_info_add_mode(
                    SELF_UUID,
                    SELF_NAME,
                    mode_id_from_name(info_name),
                ))
                .unwrap();
                game.handle_packet(position_packet(1, [0.5, 64.0, 0.5]))
                    .unwrap();
                game.handle_packet(abilities_packet(0x06, 0.05, 0.10))
                    .unwrap();
                game.player.as_mut().unwrap().velocity = Vec3::new(0.0, -0.1, 0.0);
                {
                    let remote = game.remote.as_mut().unwrap();
                    remote.loaded_sent = true;
                    remote.level_load.finish();
                }
                game.screen = Screen::Playing;
                let _ = drain_sent_abilities(game, &mut peer);

                run_local_tick(game);
                let sent = drain_sent_abilities(game, &mut peer);
                assert_row_core(game, &rows[row_idx], true, Some(&sent));
                row_idx += 1;
            }
        }
        assert_eq!(row_idx, 48);
    });
}

/// Real 26.3 oracle witness (`scenarios.live_player_info_transitions`):
/// while `local_game_mode` stays Spectator and abilities stay 0x00, `PlayerInfoUpdate` flips
/// `noPhysics` live between `false` (absent / Adventure / Survival) and `true` (Spectator).
#[test]
#[ignore = "daily"]
fn live_player_info_updates_toggle_no_physics_while_local_spectator_26_3() {
    let witness: JsonValue = serde_json::from_str(ORACLE_WITNESS).unwrap();
    assert_eq!(witness["version"].as_str(), Some("26.3"));
    let rows = witness["scenarios"]["live_player_info_transitions"]
        .as_array()
        .unwrap();

    with_client_game(|game| {
        let (mut peer, _writer) = setup_client(game, 3, None, [0.5, 64.0, 0.5]);
        game.handle_packet(abilities_packet(0x00, 0.0, 0.0))
            .unwrap();
        let _ = drain_sent_abilities(game, &mut peer);

        for _ in 0..3 {
            run_local_tick(game);
        }
        let sent = drain_sent_abilities(game, &mut peer);
        assert_row_core(game, &rows[0], true, Some(&sent));

        game.handle_packet(player_info_add_mode(SELF_UUID, SELF_NAME, 2))
            .unwrap();
        for _ in 0..3 {
            run_local_tick(game);
        }
        let sent = drain_sent_abilities(game, &mut peer);
        assert_row_core(game, &rows[1], true, Some(&sent));

        game.handle_packet(player_info_mode(SELF_UUID, 3)).unwrap();
        for _ in 0..3 {
            run_local_tick(game);
        }
        let sent = drain_sent_abilities(game, &mut peer);
        assert_row_core(game, &rows[2], true, Some(&sent));

        game.handle_packet(position_packet(2, [0.5, 64.0, 0.5]))
            .unwrap();
        game.handle_packet(player_info_mode(SELF_UUID, 0)).unwrap();
        for _ in 0..3 {
            run_local_tick(game);
        }
        let sent = drain_sent_abilities(game, &mut peer);
        assert_row_core(game, &rows[3], true, Some(&sent));
    });
}

/// Real 26.3 oracle witness (`scenarios.login_and_respawn_previous_mode`):
/// `Login` applies `spawn.previous_game_type`, and `Respawn` resets `Abilities` to defaults,
/// runs `adjustPlayer` with the previous local mode (`Spectator -> Creative` preserves `flying`),
/// then applies `setLocalMode(gameType, previousGameType)`.
#[test]
#[ignore = "daily"]
fn login_and_respawn_set_previous_mode_and_fresh_abilities_26_3() {
    let witness: JsonValue = serde_json::from_str(ORACLE_WITNESS).unwrap();
    assert_eq!(witness["version"].as_str(), Some("26.3"));
    let rows = witness["scenarios"]["login_and_respawn_previous_mode"]
        .as_array()
        .unwrap();

    with_client_game(|game| {
        let (_peer, _writer) = setup_client(game, 1, Some(2), [0.5, 64.0, 0.5]);
        assert_row_core(game, &rows[0], false, None);

        game.handle_packet(change_mode_packet(3.0)).unwrap();
        game.handle_packet(abilities_packet(0x07, 0.125, 0.2))
            .unwrap();
        assert_row_core(game, &rows[1], false, None);

        game.handle_packet(C::Respawn(play::Respawn {
            spawn: spawn_info(1, Some(0)),
            keep: 0,
        }))
        .unwrap();
        game.handle_packet(position_packet(2, [0.5, 64.0, 0.5]))
            .unwrap();
        assert_row_core(game, &rows[2], false, None);

        game.handle_packet(C::Respawn(play::Respawn {
            spawn: spawn_info(2, None),
            keep: 0,
        }))
        .unwrap();
        game.handle_packet(position_packet(3, [0.5, 64.0, 0.5]))
            .unwrap();
        assert_row_core(game, &rows[3], false, None);
    });
}
