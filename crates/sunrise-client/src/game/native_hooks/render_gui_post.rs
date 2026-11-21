use super::*;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_> {
    /// After the final HUD slot, before the screen (`RenderGuiEvent.Post`).
    pub(in crate::game) fn native_render_gui_post(
        &self,
        draw: &mut HudBatch,
        corners: &mut Corners,
    ) {
        self.fire_render_gui_post(draw, corners);
    }
}
