#[allow(unused_imports)]
use super::*;

/// The hook points. Each does nothing in Sunrise.
#[cfg(feature = "client")]
impl Game<'_, crate::game::client_side::ClientState> {
    /// Before the in-world HUD, including under F1 (`RenderGuiEvent.Pre`). A veto skips
    /// the HUD and ordinary toasts, but not mandatory diagnostics.
    pub(in crate::game) fn native_render_gui_pre(&self, canceled: &mut bool) {
        self.fire_render_gui_pre(canceled);
    }
}

#[cfg(test)]
pub(in crate::game) mod local_sound_tests {
    #[allow(unused_imports)]
    use mod_api::{ClientHost, Flow, Hooks, ModEntry, RenderGuiPreEvent, SoundPlay};
    #[allow(unused_imports)]
    use mod_platform::testing::with_mods;

    pub(in crate::game) struct Mod;
    #[mod_api::__subscriptions]
    impl Hooks for Mod {
        fn render_gui_pre(&self, host: &dyn ClientHost, _: &mut RenderGuiPreEvent) -> Flow {
            assert!(
                host.play_local_sound(&SoundPlay::ui("minecraft:ui.button.click"), 0)
                    .is_some()
            );
            Flow::Continue
        }
    }
}
