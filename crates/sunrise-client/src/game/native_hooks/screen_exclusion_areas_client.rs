use super::*;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_, crate::game::client_side::ClientState> {
    /// Where the open screen draws outside its `rect`, and where mods draw
    /// over it: the recipe book's panel, the creative tab rows, the active
    /// effects beside the inventory, then whatever `native_screen_exclusion_areas`
    /// adds. Each is `[x, y, width, height]` in GUI pixels; an empty or
    /// non-finite one is dropped. A mod that lays out its own panels
    /// (a recipe viewer) keeps them clear of these. Client-side; empty with
    /// no screen up.
    pub(in crate::game) fn screen_exclusion(&self) -> Vec<[f32; 4]> {
        let Some(view) = self.observed_screen_view() else {
            return Vec::new();
        };
        let Some(renderer) = self.side.renderer.0.as_ref() else {
            return Vec::new();
        };
        let batch = HudBatch::new(
            [renderer.config.width, renderer.config.height],
            self.side.settings.gui_scale_inputs(),
        );
        let mut areas = self.screen_own_exclusion(&batch);
        self.native_screen_exclusion_areas(&view, &mut areas);
        areas.retain(|a| a.iter().all(|v| v.is_finite()) && a[2] > 0.0 && a[3] > 0.0);
        areas
    }
}
