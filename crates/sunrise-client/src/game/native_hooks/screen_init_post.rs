use super::*;

/// The hook points. Each does nothing in Sunrise.
impl Game<'_> {
    /// A screen initialized or resized (`ScreenEvent.Init.Post`).
    pub(in crate::game) fn native_screen_init_post(
        &self,
        screen: &ScreenView<'_>,
        widgets: &mut dyn ScreenWidgets,
    ) {
        self.fire_screen_init_post(screen, widgets);
    }
}
