#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::customize_chat_overlay::*;

#[cfg(test)]
pub(in crate::game) mod customize_chat_overlay_tests {
    #[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_entity_refs::tests::with_client_game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{ClientHost, CustomizeChatOverlayEvent, Hooks, ModEntry};
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_platform::testing::with_mods;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::hud::HudBatch;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::text::Text;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::customize_chat_overlay::customize_chat_overlay_tests::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::customize_chat_overlay::customize_chat_overlay_tests::Seen;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::customize_chat_overlay::customize_chat_overlay_tests::Lift;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::native_hooks::customize_chat_overlay::customize_chat_overlay_tests::chat_top;

    /// `CustomizeGuiOverlayEvent.Chat`: the mod gets vanilla's place (0, GUI height - 40) and the
    /// GUI size, focused or not, and the chat is drawn where it moved it. With the chat hidden
    /// (F1, chat closed) nothing is asked.
    #[test]
    #[ignore = "daily"]
    fn a_client_mod_moves_the_chat() {
        let message = Text::literal("hello from the chat test");
        let mut plain = None;
        with_client_game(|game| {
            game.system_message(message.clone());
            plain = chat_top(game).0;
        });
        let plain = plain.expect("the message is drawn");
        let seen = Seen::default();
        let mods: mod_api::Mods =
            vec![ModEntry::new("lift", Box::new(Lift(Arc::clone(&seen))))].into();
        with_mods(mods, || {
            with_client_game(|game| {
                game.system_message(message.clone());
                let (moved, scale) = chat_top(game);
                let moved = moved.expect("the message is drawn");
                assert!(
                    (plain - moved - 100.0).abs() < 0.5,
                    "lifted by 100 GUI pixels: {plain} -> {moved} at scale {scale}"
                );
                let (w, h) = {
                    let b = HudBatch::scratch([1280, 720]);
                    (b.width as i32, b.height as i32)
                };
                assert_eq!(*seen.lock().unwrap(), [(w, h, false, 0, h - 40)]);
                // Hidden: F1 with the chat closed asks nothing.
                game.ui.hide_gui = true;
                chat_top(game);
                assert_eq!(seen.lock().unwrap().len(), 1);
                // Focused, the chat shows under F1 and is asked again.
                game.client.screen = super::Screen::Chat;
                chat_top(game);
                let seen = seen.lock().unwrap();
                assert_eq!(seen.len(), 2);
                assert!(seen[1].2, "focused");
            });
        });
    }
}
