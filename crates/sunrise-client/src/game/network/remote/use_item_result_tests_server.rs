#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::interaction_event_tests::use_item_result_tests::*;

#[test]
#[ignore = "daily"]
fn failed_item_part_still_tries_the_off_hand() {
    let observer = Observer::default();
    let mods: mod_api::Mods =
        vec![mod_api::ModEntry::new("failed", Box::new(observer.clone()))].into();
    crate::game::api_values::install_parts(&mods).unwrap();
    mod_platform::testing::with_mods(mods, || {
        for client in [false, true] {
            with_interaction_game(client, |game| {
                *observer.0.lock().unwrap() = Controls {
                    part_answer: Some(Answer::Fail),
                    ..Controls::default()
                };
                game.inventory[game.selected] = Some(ItemStack::new("minecraft:shield", 1));
                game.offhand = Some(ItemStack::new("minecraft:shield", 1));
                game.server.forced_hit = None;
                game.server.guest_use = None;
                game.swing = Default::default();
                if client {
                    game.remote_use_hands(Hand::Main, mod_api::InteractionAttempt::Press, None);
                } else {
                    assert_eq!(game.use_item_as_player().unwrap(), None);
                }
                assert!(!game.swing.swinging());
                assert!(game.using.is_none());
                assert_eq!(
                    observer
                        .0
                        .lock()
                        .unwrap()
                        .steps
                        .iter()
                        .filter(|step| **step == Step::ItemPart)
                        .count(),
                    2
                );
            });
        }
    });
}

#[test]
#[ignore = "daily"]
fn item_part_results_control_real_server_and_client_use() {
    let observer = Observer::default();
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
        "results",
        Box::new(observer.clone()),
    )]
    .into();
    crate::game::api_values::install_parts(&mods).unwrap();
    mod_platform::testing::with_mods(mods, || {
        for client in [false, true] {
            with_interaction_game(client, |game| {
                for guest in [false, true] {
                    for (answer, handled, using, swing) in [
                        (Answer::Consume, true, false, false),
                        (Answer::Success, true, false, client),
                        (Answer::SuccessServer, true, false, !client),
                        (Answer::Fail, false, false, false),
                        (Answer::Pass, true, true, false),
                        (Answer::TryWithEmptyHand, true, true, false),
                    ] {
                        *observer.0.lock().unwrap() = Controls {
                            part_answer: Some(answer),
                            ..Controls::default()
                        };
                        game.inventory[game.selected] = Some(ItemStack::new("minecraft:shield", 1));
                        game.offhand = None;
                        game.using = None;
                        game.swing = Default::default();
                        game.server.use_failed = false;
                        game.server.guest_use =
                            guest.then_some(super::super::super::GuestUse::Item(Hand::Main));
                        let actual = if client {
                            game.remote_use_item(Hand::Main)
                        } else {
                            game.use_item_as_player().unwrap().is_some()
                        };
                        assert_eq!(
                            (actual, game.using.is_some(), game.swing.swinging()),
                            (handled, using, swing),
                            "client={client}, guest={guest}, answer={answer:?}"
                        );
                    }
                }
            });
        }
    });
}
