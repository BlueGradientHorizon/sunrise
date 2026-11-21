//! A mod holds or clicks a resolved vanilla key mapping. The client tick's key step
//! acts on gameplay mappings with the same checks it makes for the player's own keys, so the
//! server hears the packets of a normal client at the vanilla moments.
use super::Game;
use mod_api::{MappingError, VanillaKeyMappingRef};
use sunrise_client::keymapping::{ATTACK, SNEAK, SPRINT, USE};
use winit::{event::MouseButton, keyboard::KeyCode};

/// The toggle slot of sneak's and sprint's default keys (`ToggleKeys`).
fn toggle_slot(key: KeyCode) -> Option<usize> {
    match key {
        KeyCode::ShiftLeft => Some(SNEAK),
        KeyCode::ControlLeft => Some(SPRINT),
        _ => None,
    }
}

impl Game<'_> {
    /// `KeyMapping.setDown` for the mapping on `button`: attack and use, toggled by their option;
    /// nothing reads the pick mapping's held state in gameplay.
    pub(super) fn mouse_mapping_set_down(&mut self, button: MouseButton, down: bool) {
        let (slot, option) = match button {
            MouseButton::Left => (ATTACK, self.settings.toggle_attack),
            MouseButton::Right => (USE, self.settings.toggle_use),
            _ => return,
        };
        self.toggles.0[slot].set_down(down, option);
        self.vanilla_mappings
            .sync_toggle_slot(slot, &self.toggles.0[slot]);
        self.left = self.toggles.is_down(ATTACK);
        self.right = self.toggles.is_down(USE);
    }
    /// `KeyMapping.click` for the mapping on `button`: on a server, the queued click
    /// `handleKeybinds` takes on the next tick (`interaction_input`); with no server, the attack
    /// and use clicks `tick_local_hand` takes, and pick block at once.
    pub(super) fn mouse_mapping_click(&mut self, button: MouseButton) {
        use sunrise_client::interaction_input::Button;
        if let Some(slot) = match button {
            MouseButton::Left => Some(ATTACK),
            MouseButton::Right => Some(USE),
            _ => None,
        } {
            self.toggles.0[slot].click();
            self.vanilla_mappings
                .sync_toggle_slot(slot, &self.toggles.0[slot]);
        }
        if let Some(remote) = self.remote.as_mut() {
            let input = match button {
                MouseButton::Left => Button::Attack,
                MouseButton::Right => Button::Use,
                MouseButton::Middle => Button::Pick,
                _ => return,
            };
            remote.input.press(input);
            return;
        }
        match button {
            // KeyMapping.click: Minecraft.tick starts the attack.
            MouseButton::Left => self.attack_clicks = self.attack_clicks.saturating_add(1),
            // handleKeybinds consumes use clicks without acting while an
            // item is in use. Minecraft.tick runs them after the attack
            // clicks, so a place and a break in one tick replace the block.
            MouseButton::Right => {
                if self.using.is_none() {
                    self.use_clicks = self.use_clicks.saturating_add(1);
                }
            }
            MouseButton::Middle => {
                // handleKeybinds: the number keys select before key.pickItem.
                self.select_queued_hotbar();
                // Minecraft.pickBlockOrEntity -> the server's handlePickItemFromBlock:
                // the block's clone item, with its block entity's data on Ctrl in
                // creative, picked from the inventory (`pick_into_inventory`).
                if let Some(entity) = self.pick_entity_hit() {
                    // handlePickItemFromEntity: the entity's pick result.
                    if let Some(stack) = entity.and_then(|id| self.entity_pick_result(id)) {
                        let infinite = self.creative;
                        self.pick_into_inventory(&stack, infinite);
                    }
                } else if let Some(hit) = self.pick() {
                    let include_data = self.creative && self.has_control_down();
                    if let Some(stack) = self.picked_stack(hit.pos, include_data) {
                        let infinite = self.creative;
                        self.pick_into_inventory(&stack, infinite);
                    }
                }
            }
            _ => {}
        }
    }
    /// `KeyMapping.setDown` for any resolved vanilla mapping, from a mod: attack and use
    /// as their mouse button sets them, sneak and sprint toggled by their option, and every
    /// other mapping held uniformly in `vanilla_mappings` (with non-debug keys also updating
    /// `self.keys` in lockstep). With a screen or overlay open a press does nothing: the
    /// mappings hear none behind it, and its opening lets go of all of them (`KeyMapping.releaseAll`).
    pub(in crate::game) fn vanilla_key_mapping_set_down(
        &mut self,
        mapping: VanillaKeyMappingRef,
        down: bool,
    ) -> Result<(), MappingError> {
        let slot = self.vanilla_mapping_slot(mapping)?;
        if down && self.mod_mappings_screen_open() {
            return Ok(());
        }
        let (_, _, category) = slot.definition();
        match (category, slot.default_input()) {
            ("gameplay", Some(Err(button @ (MouseButton::Left | MouseButton::Right)))) => {
                self.mouse_mapping_set_down(button, down);
            }
            ("movement", Some(Ok(key))) => {
                if let Some(t_slot) = toggle_slot(key) {
                    let option = [self.settings.toggle_crouch, self.settings.toggle_sprint][t_slot];
                    if self.toggles.0[t_slot].needs_toggle(option) {
                        self.toggles.0[t_slot].set_down(down, option);
                        self.keys.resume_device_input(&key);
                        self.sync_toggle(t_slot);
                        return Ok(());
                    }
                }
                self.keys.override_down(key, down);
                self.vanilla_mappings.set_down(slot, down, false);
            }
            (cat, Some(Ok(key))) if cat != "debug" => {
                self.keys.override_down(key, down);
                self.vanilla_mappings.set_down(slot, down, false);
            }
            _ => {
                self.vanilla_mappings.set_down(slot, down, false);
            }
        }
        Ok(())
    }
    /// `KeyMapping.click` for any resolved vanilla mapping, from a mod: records the click in
    /// `vanilla_mappings`, and when in a world dispatches the vanilla `handleKeybinds` action
    /// (attack/use/pick clicks on the next tick, hotbar/drop/inventory/swap/chat/perspective
    /// via `key_pressed`, smooth camera, spectator hotbar). Behind a screen or overlay the click
    /// is dropped, as its opening drops every click.
    pub(in crate::game) fn vanilla_key_mapping_click(
        &mut self,
        mapping: VanillaKeyMappingRef,
    ) -> Result<(), MappingError> {
        let slot = self.vanilla_mapping_slot(mapping)?;
        if self.mod_mappings_screen_open() {
            return Ok(());
        }
        let (name, _, category) = slot.definition();
        match (name, category, slot.default_input()) {
            ("key.attack" | "key.use", _, Some(Err(button))) => {
                if self.player.is_some() {
                    self.mouse_mapping_click(button);
                } else {
                    let t_slot = if button == MouseButton::Left {
                        ATTACK
                    } else {
                        USE
                    };
                    self.toggles.0[t_slot].click();
                    self.vanilla_mappings
                        .sync_toggle_slot(t_slot, &self.toggles.0[t_slot]);
                }
            }
            ("key.pickItem", _, Some(Err(button))) => {
                self.vanilla_mappings.click(slot);
                if self.player.is_some() {
                    self.mouse_mapping_click(button);
                }
            }
            ("key.spectatorHotbar", _, _) => {
                self.vanilla_mappings.click(slot);
                if self.player.is_some() && self.spectator() {
                    self.spectator_action_key();
                }
            }
            ("key.smoothCamera", _, _) => {
                self.vanilla_mappings.click(slot);
                if self.player.is_some() {
                    self.smooth_camera_action();
                }
            }
            // Window-global shortcuts (`key.fullscreen`, `key.screenshot`) and debug keys are
            // matched on physical input in vanilla (`KeyboardHandler.keyPress`), never via
            // `consumeClick` in `handleKeybinds`; record their click count uniformly.
            ("key.fullscreen" | "key.screenshot", _, _) | (_, "debug", _) => {
                self.vanilla_mappings.click(slot);
            }
            (_, _, Some(Ok(code))) => match toggle_slot(code) {
                Some(t_slot) => {
                    self.toggles.0[t_slot].click();
                    self.vanilla_mappings
                        .sync_toggle_slot(t_slot, &self.toggles.0[t_slot]);
                }
                None => {
                    self.vanilla_mappings.click(slot);
                    if self.player.is_some()
                        && let Err(error) = self.key_pressed(code, code)
                    {
                        log::warn!("vanilla_key_mapping_click {name}: {error:#}");
                    }
                }
            },
            _ => {
                self.vanilla_mappings.click(slot);
            }
        }
        Ok(())
    }
}
