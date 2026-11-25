#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use keybinds::VanillaMappingSlot;
#[allow(unused_imports)]
use mod_api::{MappingError, VanillaKeyMappingRef};

/// Helpers for native mods, kept as stable as the hook points.
// Native mods use these; nothing in Sunrise does.
impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// Milliseconds on the game's own clock (`Util.getMillis`), counting from the game's start:
    /// the clock the HUD's scrolling labels and the screens' text fields read too.
    pub(in crate::game) fn millis(&self) -> u64 {
        sunrise_client::hud_overlays::now_ms().max(0) as u64
    }
}
