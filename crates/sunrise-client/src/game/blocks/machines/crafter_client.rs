#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::machines::crafter::CRAFTING_TICKS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::machines::crafter::RESULT_SLOT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::machines::crafter::center;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::machines::crafter::recipe_matches;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::machines::crafter::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use serde_json::Value;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    advancements as adv,
    containers::Crafter,
    hud::{HudBatch, rgba},
    menu::{Maps as _, Menu},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::packets::play::{self, Serverbound as S};

impl Game<'_> {
    /// `CrafterScreen.slotClicked`: an empty grid slot is enabled by a click
    /// (or a number key with a hotbar item) when disabled, and disabled by a
    /// click with nothing carried; the menu then gets the click itself.
    pub(in crate::game) fn crafter_slot_clicked(
        &mut self,
        slot: usize,
        button: i32,
        input: sunrise_client::menu::Input,
    ) {
        use sunrise_client::menu::Input;
        if !self.crafter_open() || !(300..309).contains(&slot) || self.spectator() {
            return;
        }
        let i = slot - 300;
        let Some(c) = self.container.and_then(|k| self.crafter_at(k)) else {
            return;
        };
        if c.items[i].is_some() {
            return;
        }
        let disabled = c.disabled[i];
        let enable = match input {
            Input::Pickup if disabled => Some(true),
            Input::Pickup if self.carried.is_none() => Some(false),
            Input::Swap if disabled => usize::try_from(button)
                .ok()
                .and_then(|b| {
                    if b == 40 {
                        Some(&self.offhand)
                    } else {
                        self.inventory.get(b)
                    }
                })
                .is_some_and(Option::is_some)
                .then_some(true),
            _ => None,
        };
        let Some(enabled) = enable else {
            return;
        };
        // ServerboundContainerSlotStateChangedPacket: CrafterBlockEntity.setSlotState.
        self.menu_session(|menu, _, _| {
            menu.set_slot_state(i, enabled);
            Some(true)
        });
        if let Some((container, _)) = self.remote.as_ref().and_then(|r| r.menu.as_ref().cloned()) {
            self.send(S::ContainerSlotStateChanged(
                play::ContainerSlotStateChanged {
                    slot: i as i32,
                    container,
                    enabled,
                },
            ));
        }
        let pitch = if enabled { 1.0 } else { 0.75 };
        self.audio
            .0
            .play("minecraft:ui.button.click", None, 0.4, pitch);
    }
}
