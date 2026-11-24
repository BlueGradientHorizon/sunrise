#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::player_equip::ARMOR;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::player_equip::OFFHAND;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::player_equip::same;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::player_equip::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::items::ItemStack;

impl Game<'_> {
    /// The body of `onEquipItem` once the item changed: the item's
    /// `Equippable.equipSound` when it is equipped in its own slot, and the
    /// `equip` / `unequip` game event for armour. Nothing on a client (the
    /// server plays it) or for a spectator. The player's `firstTick` never
    /// sees an equip here: loading a save sets the slots directly.
    pub(in crate::game) fn player_equipped(&mut self, slot: &str, new: Option<&ItemStack>) {
        if self.remote.is_some() || self.spectator() {
            return;
        }
        self.native_on_equip(slot, new);
        // The equip sound plays at the player: none for one in another level than this one.
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
            // Player.getSoundSource: PLAYERS.
            let players = sunrise_client::settings::SOUND_SOURCES
                .iter()
                .position(|s| *s == "player")
                .unwrap_or(0);
            self.audio.0.play_on(players, sound, Some(at), 1.0, 1.0);
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
