//! Server-side `LivingEntity.onEquipItem` hook, sound, and game event for players.
use super::Game;
use crate::game::player_equip::ARMOR;
use sunrise_client::items::ItemStack;

impl Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game) fn server_player_equipped(&mut self, slot: &str, new: Option<&ItemStack>) {
        if self.spectator() {
            return;
        }
        self.native_on_equip(slot, new);
        let Some(at) = self
            .player
            .as_ref()
            .filter(|_| self.acting_in_level_in_place())
            .map(|p| p.position)
        else {
            return;
        };
        let at = [at.x, at.y, at.z];
        let equippable = new.and_then(|stack| {
            let equippable = sunrise_client::item_sync::equippable(stack, self.items())?;
            Some((
                equippable.slot == slot,
                sunrise_client::item_sync::equip_sound(stack, self.items()),
            ))
        });
        if let Some((true, sound)) = &equippable {
            let sound = sound
                .as_deref()
                .unwrap_or("minecraft:item.armor.equip_generic");
            let players = sunrise_client::settings::SOUND_SOURCES
                .iter()
                .position(|s| *s == "player")
                .unwrap_or(0);
            self.server_play_sound_on(players, sound, at, 1.0, 1.0);
        }
        if ARMOR.contains(&slot) {
            let event = if equippable.is_some() {
                "minecraft:equip"
            } else {
                "minecraft:unequip"
            };
            let ctx = self.player_context(None);
            self.post_game_event(event, at, ctx);
        }
    }
}
