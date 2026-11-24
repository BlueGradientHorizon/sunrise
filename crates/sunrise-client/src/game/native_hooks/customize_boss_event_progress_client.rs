#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::customize_boss_event_progress::*;

#[cfg(test)]
pub(in crate::game) mod customize_boss_event_progress_tests {
    #[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::{api_values::TryIntoApi, native_entity_refs::tests::run_fresh_cases};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
        BossBarColor, ClientHost, CustomizeBossEventProgressEvent, Flow, Hooks, ModEntry,
        text::TextFormError,
    };
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_platform::testing::with_mods;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::hud_layout::GuiDraw;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::packets::play::{BossAction, BossEvent};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::customize_boss_event_progress::customize_boss_event_progress_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::customize_boss_event_progress::customize_boss_event_progress_tests::Seen;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::customize_boss_event_progress::customize_boss_event_progress_tests::Rearrange;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::customize_boss_event_progress::customize_boss_event_progress_tests::backgrounds;

    /// Pink, blue and red bars, in that order.
    pub(in crate::game) fn add_bars(game: &mut super::Game<'_>) {
        for (id, color) in [(1, 0), (2, 1), (3, 2)] {
            let packet = BossEvent {
                id,
                action: BossAction::Add {
                    name: Tag::String(format!("bar {id}").as_str().into()),
                    progress: 1.0,
                    color,
                    overlay: 0,
                    darken: false,
                    music: false,
                    fog: false,
                },
            };
            game.client.boss_overlay.apply(&packet, 0);
        }
    }

    #[test]
    #[ignore = "daily"]
    fn an_unconvertible_boss_bar_keeps_vanilla_draws_without_dispatch() {
        if run_fresh_cases(
            "game::client_side::native_hooks_client::customize_boss_event_progress::customize_boss_event_progress_tests::an_unconvertible_boss_bar_keeps_vanilla_draws_without_dispatch",
            "SUNRISE_BOSS_BAR_CONVERSION_CHILD",
            &["malformed-profile"],
        ) {
            return;
        }
        super::super::super::native_entity_refs::tests::with_game(|game| {
            // Inject a malformed local component without the packet codec. Its UUID has
            // three words instead of the four required by vanilla's UUIDUtil.CODEC.
            let name = Tag::compound([
                ("object", Tag::String("player".into())),
                (
                    "player",
                    Tag::compound([
                        ("name", Tag::String("Steve".into())),
                        ("id", Tag::IntArray(vec![1, 2, 3])),
                    ]),
                ),
            ]);
            game.client.boss_overlay.apply(
                &BossEvent {
                    id: 1,
                    action: BossAction::Add {
                        name,
                        progress: 1.0,
                        color: 0,
                        overlay: 0,
                        darken: false,
                        music: false,
                        fog: false,
                    },
                },
                0,
            );
            assert!(matches!(
                game.client
                    .boss_overlay
                    .mod_bar(0, 0)
                    .unwrap()
                    .try_into_api(),
                Err(TextFormError::Malformed)
            ));
            let vanilla = game.boss_bar_draws(480, 270);
            assert!(!backgrounds(&vanilla).is_empty());
            let seen = Seen::default();
            let mods = vec![ModEntry::new(
                "rearrange",
                Box::new(Rearrange(Arc::clone(&seen))),
            )]
            .into();
            with_mods(mods, || {
                assert_eq!(game.boss_bar_draws(480, 270), vanilla);
                assert!(seen.lock().unwrap().is_empty());
            });
        });
    }
}
