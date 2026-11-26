use crate::game::commands::summon_bolt_tests::ORACLE_PLAYER_UUID;
use crate::game::native_entity_refs::tests::{
    bind_local_player, prepare_entity_ticking_neighborhood, with_game,
};
use sunrise_core::geometry::Vec3;
use sunrise_core::position::ChunkPos;
use sunrise_sim::motion::PlayerMotion;

/// Verifies `Silent:1b` sound suppression and `minecraft:lightning_strike` game-event timing on first server tick.
#[test]
#[ignore = "daily"]
fn silent_bolt_suppresses_sound_and_posts_strike_event_on_first_tick() {
    let _language = sunrise_game_data::lang::isolate_publication(
        sunrise_game_data::lang::Language::of("en_us", []),
    );
    with_game(|game| {
        prepare_entity_ticking_neighborhood(game, ChunkPos::new(0, 0));
        game.player = Some(PlayerMotion::new(game.motion, Vec3::new(8.5, 230.0, 8.5)).unwrap());
        bind_local_player(game, 1, ORACLE_PLAYER_UUID);
        game.creative = true;
        game.ui.difficulty = 2;
        game.set_allow_commands(true);
        game.build_registries();
        assert!(game.run_command("forceload add 0 0"));
        game.update_chunk_tickets();

        let take_strike_events = || {
            crate::game::sculk::TEST_POSTED_EVENTS.with(|events| {
                let mut guard = events.borrow_mut();
                let list = guard.get_or_insert_with(Vec::new);
                let count = list
                    .iter()
                    .filter(|(ev, _, _)| *ev == "minecraft:lightning_strike")
                    .count();
                list.clear();
                count
            })
        };

        // Silent bolt: no event on spawn; strikes rod + posts event on tick 1 without playing audio on server.
        assert!(game.run_command("setblock 4 209 4 minecraft:lightning_rod"));
        crate::game::sculk::TEST_POSTED_EVENTS.with(|e| *e.borrow_mut() = Some(Vec::new()));
        game.side.audio_capture_active = true;
        game.side.pending_sounds.clear();
        assert!(game.run_command(
            "summon minecraft:lightning_bolt 4.5 210 4.5 {Silent:1b,Tags:[\"silent_bolt\"]}"
        ));
        assert_eq!(
            take_strike_events(),
            0,
            "no lightning_strike game event at spawn"
        );
        assert!(
            game.side.pending_sounds.is_empty(),
            "no sound played at spawn"
        );
        game.tick_server(false).unwrap();
        assert_eq!(
            take_strike_events(),
            1,
            "lightning_strike event posted on first server tick"
        );
        assert!(
            game.side.pending_sounds.iter().all(|s| {
                s.event != "minecraft:entity.lightning_bolt.thunder"
                    && s.event != "minecraft:entity.lightning_bolt.impact"
            }),
            "server bolt tick must not emit local thunder or impact sounds"
        );
        assert!(game.run_command("execute if block 4 209 4 minecraft:lightning_rod[powered=true]"));

        // Audible bolt: no event on spawn; posts event on first server tick.
        assert!(game.run_command("summon minecraft:lightning_bolt 8.5 210 4.5 {}"));
        assert_eq!(
            take_strike_events(),
            0,
            "no lightning_strike game event at spawn"
        );
        game.tick_server(false).unwrap();
        assert_eq!(
            take_strike_events(),
            1,
            "lightning_strike event posted on first server tick"
        );
        crate::game::sculk::TEST_POSTED_EVENTS.with(|e| *e.borrow_mut() = None);
    });

    // Client bolt tick plays thunder + impact once for audible bolts and suppresses both for Silent:1b.
    crate::game::native_entity_refs::tests::with_client_game(|game| {
        game.bolts.clear();
        game.spawn_remote_bolt(1, [4.5, 210.0, 4.5]);
        game.bolts.update(0, |b| b.silent = true);
        game.side.audio.0.capture = Some(Vec::new());
        game.client_tick_bolts();
        assert!(
            game.side.audio.0.capture.as_ref().unwrap().iter().all(|s| {
                s.event != "minecraft:entity.lightning_bolt.thunder"
                    && s.event != "minecraft:entity.lightning_bolt.impact"
            }),
            "Silent:1b bolt must suppress thunder and impact sounds on client"
        );

        game.bolts.clear();
        game.spawn_remote_bolt(2, [8.5, 210.0, 4.5]);
        game.side.audio.0.capture = Some(Vec::new());
        game.client_tick_bolts();
        let sounds = game.side.audio.0.capture.take().unwrap();
        assert!(
            sounds
                .iter()
                .any(|s| s.event == "minecraft:entity.lightning_bolt.thunder"),
            "non-silent bolt plays thunder sound on first client tick"
        );
        assert!(
            sounds
                .iter()
                .any(|s| s.event == "minecraft:entity.lightning_bolt.impact"),
            "non-silent bolt plays impact sound on first client tick"
        );
    });
}
