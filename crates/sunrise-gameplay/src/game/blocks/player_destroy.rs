//! The removal in a player's break (`ServerPlayerGameMode.destroyBlock`): the block's parts
//! (`on_destroyed_by_player`, `IBlockExtension.onDestroyedByPlayer`) may take it over, and with
//! none answering `playerWillDestroy` and `Level.removeBlock` are vanilla's.
use super::{Game, storage_common};
use sunrise_core::position::BlockPos;
use sunrise_items::{blocks::StateFacts, items::ItemStack};
use sunrise_world::registry::BlockStateId;

impl crate::game::ServerGame<'_> {
    /// Remove the block `state` at `pos` for the player breaking it with `held`, and say whether
    /// it was removed, the state the removal saw (vanilla's `adjustedState`: a cracked pot) and
    /// whether the break harvests (drops loot). `filled_hive` is a beehive with bees or honey.
    ///
    /// The order is vanilla's: `playerWillDestroy` (pot cracking, piglin anger, a creaking
    /// heart's protector, tripwire disarming, a creative shulker box's or beehive's drop), then
    /// `Level.removeBlock`, then the harvest answer (`hasCorrectToolForDrops`, with
    /// `can_harvest_block` and `player_harvest_check`, asked once) of the adjusted state and the
    /// held stack as it is then. A part that answers replaces all of `playerWillDestroy` as well
    /// as the removal. It is told `will_harvest` first, so that one is only the tool's own answer
    /// before the removal; the mods' harvest listeners decide after it. A door's break also asks
    /// them before its removal, as it always has.
    pub(in crate::game) fn player_destroy_block_removal(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
        facts: &StateFacts,
        held: Option<&ItemStack>,
        filled_hive: bool,
    ) -> (bool, BlockStateId, bool) {
        let creative = self.common.creative;
        let mut correct_tool =
            !facts.requires_tool || self.common.res.items().correct_for_drops(held, &facts.name);
        let block_entity = self.removed_block_entity_data(pos, state);
        let mut destroyed = None;
        if sunrise_gameplay_common::game::api_values::block_state::block_listens(
            mod_api::HookId::OnDestroyedByPlayer,
            state,
        ) {
            let player = self.common.native_acting_id();
            self.native_on_destroyed_by_player(
                player,
                pos,
                state,
                held,
                !creative && correct_tool,
                &mut destroyed,
            );
        }
        if let Some(removed) = destroyed {
            if removed {
                self.native_player_destroyed_block(pos, state, block_entity);
            }
            return (removed, state, self.harvest_after_removal(pos, state));
        }
        let state = self.player_will_destroy(pos, state, facts, filled_hive);
        if !creative
            && self
                .common.res
                .behaviours()
                .data(state)
                .is(sunrise_world::level::classes::Class::Door)
        {
            correct_tool = self.common.acting_can_harvest(pos, state);
        }
        let removed = self.remove_block_as_player(pos, creative, correct_tool);
        (removed, state, self.harvest_after_removal(pos, state))
    }

    /// Whether the break that has just removed `state` harvests: the tool's answer for the selected
    /// hotbar stack as it is now, then the mods' (`can_harvest_block`, `player_harvest_check`). Only a
    /// survival player's break asks.
    fn harvest_after_removal(&mut self, pos: [i32; 3], state: BlockStateId) -> bool {
        if self.common.creative {
            return false;
        }
        self.common.acting_can_harvest(pos, state)
    }

    /// The block side effects of `Block.playerWillDestroy` that run before the vanilla
    /// behaviour library's removal (which adds the particles, the game event and the block
    /// entity's own effects); returns the adjusted state.
    fn player_will_destroy(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
        facts: &StateFacts,
        filled_hive: bool,
    ) -> BlockStateId {
        let creative = self.common.creative;
        let state = self.common.crack_mined_pot(pos, state);
        // Block.playerWillDestroy: mining their gold angers all nearby piglins.
        if self.guarded_by_piglins(&facts.name) {
            self.anger_piglins(false);
        }
        self.creaking_heart_broken(pos, true);
        // TripWireBlock.playerWillDestroy: shears disarm the string first (flags
        // 260), so taking it down does not trip the hook.
        if &*facts.name == "minecraft:tripwire"
            && self.common.held().is_some_and(|s| &*s.item == "minecraft:shears")
            && let Some(disarmed) = self.common.with_props(state, &[("disarmed", "true")])
        {
            let at = BlockPos::new(pos[0], pos[1], pos[2]);
            self.common.with_level(|b, level, ticks| b.set_block(level, ticks, at, disarmed, 260));
            self.post_block_game_event("minecraft:shear", pos, self.sculk_actor(), None);
        }
        if creative && storage_common::is_shulker_box(&facts.name) {
            self.drop_creative_shulker(pos);
        }
        // BeehiveBlock.playerWillDestroy: a creative break still drops the hive
        // with its bees and honey level.
        if creative
            && filled_hive
            && self
                .common.ui
                .rules
                .bool_at(sunrise_game_data::gamerules::vanilla::BLOCK_DROPS)
            && let Some(stack) = self.common.picked_stack(pos, true)
        {
            self.spawn_item_own(stack, pos.map(f64::from), 10);
        }
        state
    }
}

use sunrise_gameplay_common::game::tables::GameItems as _;
