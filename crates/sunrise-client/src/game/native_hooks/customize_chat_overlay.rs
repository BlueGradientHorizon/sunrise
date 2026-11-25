#[allow(unused_imports)]
use super::*;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_, crate::game::client_side::ClientState> {
    /// The in-game chat is about to be drawn (`CustomizeGuiOverlayEvent.Chat`): `x`, `y` are its
    /// bottom-left corner in GUI pixels, which a mod may move.
    pub(in crate::game) fn native_customize_chat_overlay(
        &mut self,
        gui_width: i32,
        gui_height: i32,
        focused: bool,
        x: &mut i32,
        y: &mut i32,
    ) {
        self.fire_customize_chat_overlay(gui_width, gui_height, focused, x, y);
    }
}

#[cfg(test)]
pub(in crate::game) mod customize_chat_overlay_tests {
    #[allow(unused_imports)]
    use crate::game::native_entity_refs::tests::with_client_game;
    #[allow(unused_imports)]
    use mod_api::{ClientHost, CustomizeChatOverlayEvent, Hooks, ModEntry};
    #[allow(unused_imports)]
    use mod_platform::testing::with_mods;
    #[allow(unused_imports)]
    use std::sync::{Arc, Mutex};
    #[allow(unused_imports)]
    use sunrise_client::hud::HudBatch;
    #[allow(unused_imports)]
    use sunrise_client::text::Text;

    pub(in crate::game) type Seen = Arc<Mutex<Vec<(i32, i32, bool, i32, i32)>>>;

    /// Lifts the chat 100 GUI pixels, logging what it was told.
    pub(in crate::game) struct Lift(pub(in crate::game) Seen);
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
                .push((e.gui_width, e.gui_height, e.focused, e.x, e.y));
            event.y -= 100;
        }
    }

    /// The top of what the chat draws, in GUI pixels, and the batch's GUI scale.
    pub(in crate::game) fn chat_top(game: &mut super::Game<'_>) -> (Option<f32>, f32) {
        let mut batch = HudBatch::scratch([1280, 720]);
        game.draw_chat(&mut batch);
        let top = batch.vertices.iter().map(|v| v.pos[1]).reduce(f32::min);
        (top, batch.scale)
    }
}
