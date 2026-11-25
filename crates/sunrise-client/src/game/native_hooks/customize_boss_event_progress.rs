#[allow(unused_imports)]
use super::*;

impl Game<'_, crate::game::client_side::ClientState> {
    /// One boss bar is about to be drawn (`CustomizeGuiOverlayEvent.BossEventProgress`): `x`,
    /// `y` are its top-left corner and `increment` the spacing to the next bar, in GUI pixels,
    /// which a mod may change; `cancel` skips drawing it.
    pub(in crate::game) fn native_customize_boss_event_progress(
        &self,
        bar: &mod_api::ClientBossBar,
        x: &mut i32,
        y: &mut i32,
        increment: &mut i32,
        cancel: &mut bool,
    ) {
        self.fire_customize_boss_event_progress(bar, x, y, increment, cancel);
    }
}

#[cfg(test)]
pub(in crate::game) mod customize_boss_event_progress_tests {
    #[allow(unused_imports)]
    use crate::game::{api_values::TryIntoApi, native_entity_refs::tests::run_fresh_cases};
    #[allow(unused_imports)]
    use mod_api::{
        BossBarColor, ClientHost, CustomizeBossEventProgressEvent, Flow, Hooks, ModEntry,
        text::TextFormError,
    };
    #[allow(unused_imports)]
    use mod_platform::testing::with_mods;
    #[allow(unused_imports)]
    use std::sync::{Arc, Mutex};
    #[allow(unused_imports)]
    use sunrise_client::hud_layout::GuiDraw;
    #[allow(unused_imports)]
    use sunrise_core::nbt::Tag;
    #[allow(unused_imports)]
    use sunrise_protocol::packets::play::{BossAction, BossEvent};

    pub(in crate::game) type Seen = Arc<Mutex<Vec<(u128, BossBarColor, i32, i32, i32)>>>;

    /// Spaces the pink bar 30 pixels from the next, hides the blue one and moves the red one
    /// 10 pixels right, logging what it was told.
    pub(in crate::game) struct Rearrange(pub(in crate::game) Seen);
    #[mod_api::__subscriptions]
    impl Hooks for Rearrange {
        fn customize_boss_event_progress(
            &self,
            _: &dyn ClientHost,
            event: &mut CustomizeBossEventProgressEvent<'_>,
        ) -> Flow {
            let bar = event.bar;
            self.0
                .lock()
                .unwrap()
                .push((bar.uuid, bar.color, event.x, event.y, event.increment));
            match bar.color {
                BossBarColor::Pink => event.increment = 30,
                BossBarColor::Blue => return Flow::Cancel,
                _ => event.x += 10,
            }
            Flow::Continue
        }
    }

    #[allow(unused_imports)]
pub(in crate::game) use crate::game::client_side::native_hooks_client::customize_boss_event_progress::customize_boss_event_progress_tests::add_bars;

    /// Each bar background's sprite and top-left corner.
    pub(in crate::game) fn backgrounds(draws: &[GuiDraw]) -> Vec<(String, i32, i32)> {
        draws
            .iter()
            .filter_map(|draw| match draw {
                GuiDraw::Sprite { sprite, x, y, .. } if sprite.ends_with("_background") => {
                    Some((sprite.to_string(), *x, *y))
                }
                _ => None,
            })
            .collect()
    }

    /// `CustomizeGuiOverlayEvent.BossEventProgress`: with no mod the bars sit 19 pixels apart from
    /// y = 12; a mod hears each bar at vanilla's place, and hides one, spaces the next and moves
    /// another without touching the rest.
    #[test]
    #[ignore = "daily"]
    fn a_client_mod_hides_and_spaces_boss_bars() {
        let (w, h) = (480, 270);
        let left = w / 2 - 91;
        super::super::super::native_entity_refs::tests::with_client_game(|game| {
            add_bars(game);
            assert_eq!(
                backgrounds(&game.boss_bar_draws(w, h)),
                [
                    ("boss_bar/pink_background".to_owned(), left, 12),
                    ("boss_bar/blue_background".to_owned(), left, 31),
                    ("boss_bar/red_background".to_owned(), left, 50),
                ]
            );
        });
        let seen = Seen::default();
        let mods: mod_api::Mods = vec![ModEntry::new(
            "rearrange",
            Box::new(Rearrange(Arc::clone(&seen))),
        )]
        .into();
        with_mods(mods, || {
            super::super::super::native_entity_refs::tests::with_client_game(|game| {
                add_bars(game);
                assert_eq!(
                    backgrounds(&game.boss_bar_draws(w, h)),
                    [
                        ("boss_bar/pink_background".to_owned(), left, 12),
                        (
                            "boss_bar/red_background".to_owned(),
                            left + 10,
                            12 + 30 + 19
                        ),
                    ]
                );
                assert_eq!(
                    *seen.lock().unwrap(),
                    [
                        (1, BossBarColor::Pink, left, 12, 19),
                        (2, BossBarColor::Blue, left, 42, 19),
                        (3, BossBarColor::Red, left, 61, 19),
                    ]
                );
            });
        });
    }
}
