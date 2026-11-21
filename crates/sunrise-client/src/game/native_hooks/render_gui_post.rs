use super::*;

/// Post-HUD hook (`RenderGuiEvent.Post`) and drawn HUD layer bounds queries.
impl Game<'_> {
    /// After the visible in-world HUD layer sequence finishes and before screens,
    /// toasts, debug overlays and IME draw (`RenderGuiEvent.Post`).
    pub(in crate::game) fn native_render_gui_post(&self, draw: &mut HudBatch) {
        self.fire_render_gui_post(draw);
    }

    /// Bounding rectangle `[x, y, width, height]` in GUI pixels of a HUD layer drawn
    /// during the current HUD pass (`ScreensClientHost::hud_layer_bounds`).
    pub(in crate::game) fn hud_layer_bounds(
        &self,
        layer: &str,
    ) -> Result<Option<[f32; 4]>, mod_api::LevelReadError> {
        if !self.native_window() || self.lan_acting.is_some() {
            return Err(mod_api::LevelReadError::Unavailable);
        }
        Ok(self
            .hud_layer_bounds
            .iter()
            .rev()
            .find_map(|&(id, rect)| (id == layer).then_some(rect)))
    }

    pub(in crate::game) fn record_hud_layer_bounds(
        &mut self,
        id: &'static str,
        rect: Option<[f32; 4]>,
    ) {
        if let Some(existing) = self
            .hud_layer_bounds
            .iter_mut()
            .find(|(existing_id, _)| *existing_id == id)
        {
            if let Some(rect) = rect {
                existing.1 = rect;
            } else {
                self.hud_layer_bounds
                    .retain(|(existing_id, _)| *existing_id != id);
            }
        } else if let Some(rect) = rect {
            self.hud_layer_bounds.push((id, rect));
        }
    }
}
