#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{
    super::{Game, Screen},
    Connection, Pads,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::pad::hardware::Devices;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::pad::hardware::EXTRA_SDL_MAPPINGS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::pad::hardware::TAKEOVER;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::pad::hardware::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use serde_json::{Value, json};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::time::{Duration, Instant};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::controller_ui::Device;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::gamepad::{
    self, Button, GlyphStyle, PadState, Rumble, TriggerFeel,
};

impl Game<'_> {
    /// The use trigger while nothing is in use: a loaded crossbow clicks, an item that boosts (a
    /// rocket while gliding, the stick of the mount being ridden) catches, a shield weighs, and
    /// locks while an axe has disabled it.
    pub(in crate::game) fn idle_use_feel(&self) -> TriggerFeel {
        let gliding = self.player.as_ref().is_some_and(|p| p.fall_flying);
        let mount = self
            .riding
            .and_then(|id| self.mobs.iter().find(|m| m.id == id));
        let mount = mount.map_or("", |m| m.kind.id.as_str());
        let feel = |stack: Option<&sunrise_client::items::ItemStack>| match stack
            .map(|s| (s, &*s.item))
        {
            Some((s, "minecraft:crossbow")) if s.data().is_some_and(|d| !d.charged.is_empty()) => {
                TriggerFeel::Click
            }
            Some((_, "minecraft:firework_rocket")) if gliding => TriggerFeel::Boost,
            Some((_, "minecraft:carrot_on_a_stick")) if mount == "minecraft:pig" => {
                TriggerFeel::Boost
            }
            Some((_, "minecraft:warped_fungus_on_a_stick")) if mount == "minecraft:strider" => {
                TriggerFeel::Boost
            }
            Some((s, _)) if self.blocks_attacks(s).is_some() => match self.server.shield_cooldown {
                0 => TriggerFeel::Brace,
                _ => TriggerFeel::Locked,
            },
            _ => TriggerFeel::Free,
        };
        // Use tries the main hand first.
        match feel(self.stack_in(self.selected)) {
            TriggerFeel::Free => feel(self.offhand.as_ref()),
            main => main,
        }
    }
}
