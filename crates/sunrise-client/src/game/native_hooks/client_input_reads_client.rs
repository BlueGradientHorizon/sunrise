use super::*;
use keybinds::VanillaMappingSlot;
use mod_api::{MappingError, VanillaKeyMappingRef};

/// Helpers for native mods, kept as stable as the hook points.
// Native mods use these; nothing in Sunrise does.
impl Game<'_, crate::game::client_side::ClientState> {
    /// The current window modifier keys in the same GLFW encoding as input hooks.
    /// Cleared when the window loses focus; available even outside a world.
    pub(in crate::game) fn modifiers(&self) -> ClientModifiers {
        ClientModifiers::new(crate::game::glfw_modifiers(self.side.modifiers))
    }
    /// Playing, with nothing over the world: keys and zoom apply.
    pub(in crate::game) fn playing(&self) -> bool {
        self.side.screen == Screen::Playing
            && self.side.title.0.is_none()
            && self.side.sign_edit.is_none()
            && self.side.book.is_none()
    }
    pub(in crate::game) fn vanilla_key_mapping(
        &self,
        name: &str,
    ) -> Result<VanillaKeyMappingRef, MappingError> {
        if !self.options.execution_role.is_client() {
            return Err(MappingError::Unavailable);
        }
        let generation = self
            .side
            .mod_keys
            .client_generation()
            .ok_or(MappingError::Unavailable)?;
        let slot = VanillaMappingSlot::by_name(name)
            .ok_or_else(|| MappingError::Unknown { key: name.into() })?;
        Ok(mod_api::__private::vanilla_key_mapping_ref(
            generation,
            slot.index(),
        ))
    }
    pub(in crate::game) fn vanilla_key_mapping_is_down(
        &self,
        mapping: VanillaKeyMappingRef,
    ) -> Result<bool, MappingError> {
        let slot = self.vanilla_mapping_slot(mapping)?;
        Ok(
            if let ("movement", Some(Ok(key))) = (slot.definition().2, slot.default_input()) {
                self.movement_key_held(key)
            } else {
                self.side.vanilla_mappings.is_down(slot)
            },
        )
    }
    pub(in crate::game) fn vanilla_key_mapping_physical_down(
        &self,
        mapping: VanillaKeyMappingRef,
    ) -> Result<bool, MappingError> {
        let slot = self.vanilla_mapping_slot(mapping)?;
        Ok(match self.side.settings.keys.physical_at(slot) {
            Some(Ok(key)) => self.side.physical_keys.contains(&key),
            Some(Err(button)) => self.side.physical_buttons.contains(&button),
            None => false,
        })
    }
    pub(in crate::game) fn vanilla_key_mapping_binding(
        &self,
        mapping: VanillaKeyMappingRef,
    ) -> Result<&str, MappingError> {
        let slot = self.vanilla_mapping_slot(mapping)?;
        Ok(self.side.settings.keys.binding_at(slot))
    }
    pub(in crate::game) fn vanilla_key_mapping_translated_key_message(
        &self,
        mapping: VanillaKeyMappingRef,
    ) -> Result<Text, MappingError> {
        Ok(keybinds::display_name(
            self.vanilla_key_mapping_binding(mapping)?,
        ))
    }
    /// Validate an API mapping handle before using it to index the current bindings.
    pub(in crate::game) fn vanilla_mapping_slot(
        &self,
        mapping: VanillaKeyMappingRef,
    ) -> Result<VanillaMappingSlot, MappingError> {
        if !self.options.execution_role.is_client() {
            return Err(MappingError::Unavailable);
        }
        let (generation, index) = mod_api::__private::vanilla_key_mapping_parts(mapping);
        if self.side.mod_keys.client_generation() != Some(generation) {
            return Err(MappingError::Stale);
        }
        VanillaMappingSlot::from_index(index).ok_or(MappingError::Stale)
    }
}

/// Helpers for native mods, kept as stable as the hook points.
// Native mods use these; nothing in Sunrise does.
impl Game<'_, crate::game::client_side::ClientState> {
    /// Milliseconds on the game's own clock (`Util.getMillis`), counting from the game's start:
    /// the clock the HUD's scrolling labels and the screens' text fields read too.
    pub(in crate::game) fn millis(&self) -> u64 {
        sunrise_client::hud_overlays::now_ms().max(0) as u64
    }
}
