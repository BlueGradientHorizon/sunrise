#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::render_gui_pre::*;

#[cfg(test)]
pub(in crate::game) mod local_sound_tests {
    #[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{ClientHost, Flow, Hooks, ModEntry, RenderGuiPreEvent, SoundPlay};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_platform::testing::with_mods;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::render_gui_pre::local_sound_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::render_gui_pre::local_sound_tests::Mod;

    /// A sound a screen hook plays through its read-only host is held, then played by the
    /// next audio update, before its tick.
    #[test]
    #[ignore = "daily"]
    fn a_screen_hooks_sound_reaches_audio_at_the_next_update() {
        let mods: mod_api::Mods = vec![ModEntry::new("click", Box::new(Mod))].into();
        with_mods(mods, || {
            crate::game::native_entity_refs::tests::with_game(|game| {
                game.options.execution_role = super::ExecutionRole::Client;
                game.audio.0.heard.clear();
                game.native_render_gui_pre(&mut false);
                assert_eq!(
                    game.client.mod_sounds.queued.borrow().len(),
                    1,
                    "held for the audio update"
                );
                assert!(!game.audio.0.heard.contains_key("minecraft:ui.button.click"));
                game.tick_sound_engine();
                assert!(game.client.mod_sounds.queued.borrow().is_empty());
                assert_eq!(
                    game.audio.0.heard.get("minecraft:ui.button.click"),
                    Some(&1)
                );
            });
        });
    }
}
