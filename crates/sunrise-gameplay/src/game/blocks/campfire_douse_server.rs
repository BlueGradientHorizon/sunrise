use super::{Game, Hit};
use sunrise_game_data::Result;
use sunrise_items::items::ItemStack;

impl crate::game::ServerGame<'_> {
    /// `CampfireBlock.useItemOn` with `held` in the hand that acts, the block
    /// at `pos` clicked: true when it doused a lit campfire (SUCCESS).
    pub(in crate::game) fn use_campfire_douse(
        &mut self,
        hit: &Hit,
        held: Option<&ItemStack>,
    ) -> Result<bool> {
        let pos = hit.pos;
        let (Some(used), Some(mut state)) = (held.cloned(), self.common.campfire_to_douse(pos, held))
        else {
            return Ok(false);
        };
        state.properties.insert("lit".into(), "false".into());
        let Ok(unlit) = self.common.pack.assets.registry.resolve(&state) else {
            return Ok(false);
        };
        let Some(unlit) = self.common.modified_campfire(hit, &used, unlit) else {
            return Ok(false);
        };
        // Level event 1009 (SOUND_EXTINGUISH_FIRE) goes to every player.
        self.server_level_event_at(1009, pos, 0);
        self.post_block_game_event("minecraft:block_change", pos, self.sculk_actor(), None);
        self.common.level_set(pos, unlit)?;
        self.damage_held(1);
        // ServerPlayerGameMode.useItemOn: the action was consumed.
        self.trigger_block("minecraft:item_used_on_block", pos, &used);
        Ok(true)
    }
}
