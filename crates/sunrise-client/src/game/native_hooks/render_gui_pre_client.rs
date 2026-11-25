#[cfg(test)]
pub(in crate::game) mod local_sound_tests {
    use crate::game::native_hooks::render_gui_pre::local_sound_tests::Mod;
    use mod_api::ModEntry;
    use mod_platform::testing::with_mods;

    /// A sound a screen hook plays through its read-only host is held, then played by the
    /// next audio update, before its tick.
    #[test]
    #[ignore = "daily"]
    fn a_screen_hooks_sound_reaches_audio_at_the_next_update() {
        let mods: mod_api::Mods = vec![ModEntry::new("click", Box::new(Mod))].into();
        with_mods(mods, || {
            crate::game::native_entity_refs::tests::with_client_game(|game| {
                game.side.audio.0.heard.clear();
                game.native_render_gui_pre(&mut false);
                assert_eq!(
                    game.side.mod_sounds.queued.borrow().len(),
                    1,
                    "held for the audio update"
                );
                assert!(
                    !game
                        .side
                        .audio
                        .0
                        .heard
                        .contains_key("minecraft:ui.button.click")
                );
                game.tick_sound_engine();
                assert!(game.side.mod_sounds.queued.borrow().is_empty());
                assert_eq!(
                    game.side.audio.0.heard.get("minecraft:ui.button.click"),
                    Some(&1)
                );
            });
        });
    }
}
