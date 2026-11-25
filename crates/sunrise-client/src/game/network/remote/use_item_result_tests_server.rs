#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::interaction_event_tests::use_item_result_tests::*;

#[test]
#[ignore = "daily"]
fn failed_item_part_still_tries_the_off_hand() {
    fn check<S: crate::game::Side>(
        observer: &Observer,
        game: &mut Game<'_, S>,
        trigger: impl FnOnce(&mut Game<'_, S>),
    ) {
        *observer.0.lock().unwrap() = Controls {
            part_answer: Some(Answer::Fail),
            ..Controls::default()
        };
        game.inventory[game.selected] = Some(ItemStack::new("minecraft:shield", 1));
        game.offhand = Some(ItemStack::new("minecraft:shield", 1));
        game.set_test_forced_hit(None);
        game.swing = Default::default();
        trigger(game);
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
    }

    let observer = Observer::default();
    let mods: mod_api::Mods =
        vec![mod_api::ModEntry::new("failed", Box::new(observer.clone()))].into();
    crate::game::api_values::install_parts(&mods).unwrap();
    mod_platform::testing::with_mods(mods, || {
        with_interaction_server_game(|game| {
            check(&observer, game, |game| {
                game.side.guest_use = None;
                assert_eq!(game.use_item_as_player().unwrap(), None);
            });
        });
        with_interaction_client_game(|game| {
            check(&observer, game, |game| {
                game.remote_use_hands(Hand::Main, mod_api::InteractionAttempt::Press, None);
            });
        });
    });
}

#[test]
#[ignore = "daily"]
fn item_part_results_control_real_server_and_client_use() {
    fn check_answers<S: InteractionSide>(
        observer: &Observer,
        game: &mut Game<'_, S>,
        label: &str,
        mut before_use: impl FnMut(&mut Game<'_, S>),
    ) {
        let client = S::IS_CLIENT;
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
            before_use(game);
            let actual = S::interaction_use_item_as_player(game);
            assert_eq!(
                (actual, game.using.is_some(), game.swing.swinging()),
                (handled, using, swing),
                "{label}, answer={answer:?}"
            );
        }
    }

    let observer = Observer::default();
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
        "results",
        Box::new(observer.clone()),
    )]
    .into();
    crate::game::api_values::install_parts(&mods).unwrap();
    mod_platform::testing::with_mods(mods, || {
        with_interaction_server_game(|game| {
            for guest in [false, true] {
                check_answers(
                    &observer,
                    game,
                    &format!("client=false, guest={guest}"),
                    |game| {
                        game.side.use_failed = false;
                        game.side.guest_use =
                            guest.then_some(crate::game::GuestUse::Item(Hand::Main));
                    },
                );
            }
        });
        with_interaction_client_game(|game| {
            check_answers(&observer, game, "client=true", |_| {});
        });
    });
}
