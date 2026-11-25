use super::*;
use crate::game::native_hooks::client_options::resolved_gui_scale;

/// Helpers for native mods, kept as stable as the hook points.
// Native mods use these; nothing in Sunrise does.
impl Game<'_, crate::game::client_side::ClientState> {
    /// The GUI scale in effect (`Window.getGuiScale`): the scale the window
    /// applies from the GUI Scale option and its size, at least 1; none on a
    /// server or when no window exists. `gui_options().gui_scale` is only the
    /// option, where 0 means automatic.
    pub(in crate::game) fn gui_scale(&self) -> Option<u32> {
        // The size the renderer applies, as the drawing code reads it: it keeps its previous
        // configuration when the window reports a size it cannot draw at.
        let framebuffer = self
            .side
            .window
            .0
            .as_ref()
            .and(self.side.renderer.0.as_ref())
            .map(|r| [r.config.width, r.config.height]);
        resolved_gui_scale(
            framebuffer,
            self.settings.gui_scale,
            self.settings.force_unicode,
        )
    }
    /// `Window.getWidth` and `getHeight`: the window's framebuffer pixels, the size
    /// `window_resize` reports; none on a server or when no window exists.
    pub(in crate::game) fn window_size(&self) -> Option<[u32; 2]> {
        let size = self.side.window.0.as_ref()?.inner_size();
        Some([size.width, size.height])
    }
}
