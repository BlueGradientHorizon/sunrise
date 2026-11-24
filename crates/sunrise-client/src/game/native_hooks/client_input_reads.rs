#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use keybinds::VanillaMappingSlot;
#[allow(unused_imports)]
use mod_api::{MappingError, VanillaKeyMappingRef};

/// Helpers for native mods, kept as stable as the hook points.
// Native mods use these; nothing in Sunrise does.
impl Game<'_> {
    /// Milliseconds on the game's own clock (`Util.getMillis`), counting from the game's start:
    /// the clock the HUD's scrolling labels and the screens' text fields read too.
    pub(in crate::game) fn millis(&self) -> u64 {
        sunrise_client::hud_overlays::now_ms().max(0) as u64
    }

    pub(in crate::game) fn vanilla_key_mapping_binding(
        &self,
        mapping: VanillaKeyMappingRef,
    ) -> Result<&str, MappingError> {
        let slot = self.vanilla_mapping_slot(mapping)?;
        Ok(self.settings.keys.binding_at(slot))
    }

    pub(in crate::game) fn vanilla_key_mapping_translated_key_message(
        &self,
        mapping: VanillaKeyMappingRef,
    ) -> Result<Text, MappingError> {
        Ok(keybinds::display_name(
            self.vanilla_key_mapping_binding(mapping)?,
        ))
    }
}
