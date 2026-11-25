//! The subtitle overlay in the game (`Hud` -> `SubtitleOverlay`): the sound
//! engine reports the sounds it plays while the overlay listens, the HUD
//! renders it (which starts or stops the listening as the "Show Subtitles"
//! option says).
use super::Game;
use std::cell::{Cell, RefCell};
use sunrise_client::{
    hud::HudBatch,
    hud_layout,
    hud_overlays::{SubtitleFrame, SubtitleOverlay, now_ms},
};

#[derive(Default)]
pub(in crate::game) struct SubtitleState {
    overlay: RefCell<SubtitleOverlay>,
    pub(in crate::game) deferred: Cell<bool>,
}
impl Game<'_, crate::game::client_side::ClientState> {
    /// Feed the sounds the engine reported since the last tick
    /// (`SoundEventListener.onPlaySound`) while the overlay listens.
    pub(in crate::game) fn tick_subtitles(&mut self) {
        let mut overlay = self.side.subtitles.overlay.borrow_mut();
        let listening = overlay.listening();
        self.side.audio.0.record_heard = listening;
        let heard = self.side.audio.0.take_heard();
        if !listening {
            return;
        }
        let now = now_ms();
        for h in heard {
            let at = h.at.unwrap_or(self.side.audio.0.listener);
            overlay.on_play(Some(&h.subtitle), at, h.range, h.looping, now);
        }
    }
    /// Consume a paint scheduled by the original subtitle layer, without dispatching it again.
    pub(in crate::game::client_side) fn draw_deferred_subtitles(&self, batch: &mut HudBatch) {
        if self.side.subtitles.deferred.take() {
            self.draw_subtitles(batch);
        }
    }

    pub(in crate::game::client_side) fn draw_subtitles(&self, batch: &mut HudBatch) {
        let v = &self.side.view;
        let frame = SubtitleFrame {
            show: self.settings.show_subtitles,
            position: v.position,
            forward: v.forwards.map(f64::from),
            up: v.up.map(f64::from),
            multiplier: self.settings.notification_display_time,
            background: self.settings.text_background(0.8),
            width: batch.width as i32,
            height: batch.height as i32,
        };
        let atlas = &self.gui.atlas;
        let draws = self
            .side
            .subtitles
            .overlay
            .borrow_mut()
            .extract(&frame, &atlas.font, now_ms());
        hud_layout::paint(batch, atlas, &draws);
    }
}
