use super::*;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_> {
    /// The in-game chat is about to be drawn (`CustomizeGuiOverlayEvent.Chat`): `x`, `y` are its
    /// bottom-left corner in GUI pixels, which a mod may move.
    pub(in crate::game) fn native_customize_chat_overlay(
        &mut self,
        gui_width: i32,
        gui_height: i32,
        x: &mut i32,
        y: &mut i32,
    ) {
        self.fire_customize_chat_overlay(gui_width, gui_height, x, y);
    }
}

#[cfg(test)]
mod customize_chat_overlay_tests {
    use mod_api::{ClientHost, CustomizeChatOverlayEvent, Hooks, ModEntry};
    use mod_platform::testing::with_mods;
    use std::sync::{Arc, Mutex};
    use sunrise_client::hud::HudBatch;
    use sunrise_client::text::Text;

    type Seen = Arc<Mutex<Vec<(i32, i32, i32, i32)>>>;

    /// Lifts the chat 100 GUI pixels, logging what it was told.
    struct Lift(Seen);
    #[mod_api::__subscriptions]
    impl Hooks for Lift {
        fn customize_chat_overlay(
            &self,
            _: &mut dyn ClientHost,
            event: &mut CustomizeChatOverlayEvent,
        ) {
            let e = &*event;
            self.0
                .lock()
                .unwrap()
                .push((e.gui_width, e.gui_height, e.x, e.y));
            event.y -= 100;
        }
    }

    /// The top of what the chat draws, in GUI pixels, and the batch's GUI scale.
    fn chat_top(game: &mut super::Game<'_>) -> (Option<f32>, f32) {
        let mut batch = HudBatch::scratch([1280, 720]);
        game.draw_chat(&mut batch);
        let top = batch.vertices.iter().map(|v| v.pos[1]).reduce(f32::min);
        (top, batch.scale)
    }

    /// `CustomizeGuiOverlayEvent.Chat`: the mod gets vanilla's place (0, GUI height - 40) and the
    /// GUI size and moves background chat only. F1 hides background chat, and the chat
    /// screen paints independently at its original position.
    #[test]
    #[ignore = "daily"]
    fn a_client_mod_moves_the_chat() {
        let message = Text::literal("hello from the chat test");
        let mut plain = None;
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.options.execution_role = super::ExecutionRole::Client;
            game.system_message(message.clone());
            plain = chat_top(game).0;
        });
        let plain = plain.expect("the message is drawn");
        let seen = Seen::default();
        let mods: mod_api::Mods =
            vec![ModEntry::new("lift", Box::new(Lift(Arc::clone(&seen))))].into();
        with_mods(mods, || {
            crate::game::native_entity_refs::tests::with_game(|game| {
                game.options.execution_role = super::ExecutionRole::Client;
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
                assert_eq!(*seen.lock().unwrap(), [(w, h, 0, h - 40)]);
                // Hidden: F1 with the chat closed asks nothing.
                game.ui.hide_gui = true;
                chat_top(game);
                assert_eq!(seen.lock().unwrap().len(), 1);
                // Focused chat draws even under F1, but never asks the background listener.
                game.screen = super::Screen::Chat;
                assert!(chat_top(game).0.is_some());
                assert_eq!(*seen.lock().unwrap(), [(w, h, 0, h - 40)]);
            });
        });
    }
}
