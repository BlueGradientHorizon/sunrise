use super::*;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_> {
    /// Before pass-bearing world render stages execute (NeoForge's `PrepareRenderBuffersEvent`).
    pub(in crate::game) fn native_prepare_render_buffers(
        &self,
        view: &mod_api::WorldRenderView<'_>,
    ) {
        self.fire_prepare_render_buffers(view);
    }
}
