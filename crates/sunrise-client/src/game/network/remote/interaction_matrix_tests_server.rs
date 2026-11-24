#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::interaction_event_tests::interaction_matrix_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::interaction_event_tests::interaction_matrix_tests::Native;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::interaction_event_tests::interaction_matrix_tests::Case;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::interaction_event_tests::interaction_matrix_tests::cases;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::interaction_event_tests::interaction_matrix_tests::advancement_tree;

pub(in crate::game) fn matrix(mods: mod_api::Mods) {
    // with_mods registers the mods again for their controllers: install Parts after it, as
    // registration-time declarations (a custom statistic) refuse once the registry is frozen.
    mod_platform::testing::with_mods(mods.clone(), || {
        crate::game::api_values::install_parts(&mods).unwrap();
        // The fixture declares a block-entity type of its own: the game stands over the content
        // root its mods make, built once for both of this matrix's games.
        let base = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let root = crate::game::mods::content_root(&base, &Default::default())
            .unwrap()
            .prepared()
            .to_owned();
        for client in [false, true] {
            with_interaction_game_at(client, root.clone(), |game| {
                let registry = game.pack.assets.registry;
                let stone = registry.default_state("minecraft:stone").unwrap();
                let lever = game
                    .with_props(
                        registry.default_state("minecraft:lever").unwrap(),
                        &[("face", "floor")],
                    )
                    .unwrap();
                game.world.set(0, 63, 0, stone).unwrap();
                game.server.forced_hit = Some(hit());
                for hand in [Hand::Main, Hand::Off] {
                    for case in cases(hand) {
                        let state = if case.lever { lever } else { stone };
                        game.world.set(0, 64, 0, state).unwrap();
                        let held = case.held.then(|| ItemStack::new("minecraft:stick", 1));
                        let other = case
                            .other_held
                            .then(|| ItemStack::new("minecraft:stick", 1));
                        (game.inventory[game.selected], game.offhand) = if hand == Hand::Main {
                            (held, other)
                        } else {
                            (other, held)
                        };
                        // Each case supplies the logical sneak state of the acting player.
                        game.set_mapping_held("key.sneak", case.sneak);
                        if let Some(remote) = game.remote.as_mut() {
                            remote.last_input = if case.sneak {
                                super::super::SHIFT_INPUT
                            } else {
                                0
                            };
                        }
                        game.swing = Default::default();
                        game.server.use_failed = false;
                        advancement_tree(game);
                        fixture::begin(case.controls);
                        let swings = game.swings_started;
                        let used = if client {
                            game.remote_use_item_on(hand, mod_api::InteractionAttempt::Press, None)
                        } else {
                            game.server.guest_use =
                                Some(super::super::super::GuestUse::on_block(hand));
                            game.with_hand(hand, |g| g.use_hand(Some(&hit()))).unwrap()
                        };
                        assert_eq!(used, case.used, "{client:?} {hand:?} {case:?}");
                        assert_eq!(
                            game.swings_started - swings,
                            case.swings[usize::from(client)],
                            "{client:?} {hand:?} {case:?}"
                        );
                        if game.swings_started != swings {
                            assert_eq!(game.swing.attack_hand(), hand);
                        }
                        assert_eq!(
                            game.world.block(0, 64, 0) != state,
                            case.default_award,
                            "{client:?} {hand:?} {case:?}"
                        );
                        for (name, expected) in [
                            ("item_used_on_block", case.item_award),
                            ("default_block_use", case.default_award),
                        ] {
                            assert_eq!(
                                game.adv.progress.has(&format!("test:{name}"), "seen"),
                                !client && expected,
                                "{client:?} {hand:?} {name} {case:?}"
                            );
                        }
                        let expected: Vec<u8> = case
                            .phases
                            .iter()
                            .flat_map(|phase| {
                                [
                                    *phase,
                                    u8::from(hand == Hand::Off),
                                    u8::from(client),
                                    u8::from(!case.held),
                                ]
                            })
                            .collect();
                        assert_eq!(fixture::finish(), expected, "{client:?} {hand:?} {case:?}");
                        assert_eq!(
                            game.with_hand(hand, |g| g
                                .held()
                                .map(|s| (s.item.to_string(), s.count))),
                            case.held.then(|| ("minecraft:stick".into(), 1))
                        );
                    }
                }
            });
        }
    });
}
