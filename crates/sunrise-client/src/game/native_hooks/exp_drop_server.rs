use super::*;
use crate::game::native_hooks::exp_drop::LiveBlockEntity;
use crate::game::spill::{BlockDropEntity, BlockDrops};

impl Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game) fn entity_block_dropped(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
        breaker: Option<u32>,
        block_entity: BlockDropEntity<'_>,
        drops: Vec<ItemStack>,
    ) -> BlockDrops {
        let breaker =
            if crate::game::api_values::block_state::block_listens(mod_api::HookId::ExpDrop, state)
                || mod_api::__private::listened(mod_api::HookId::BlockDrops)
            {
                breaker.and_then(|id| self.entity(id))
            } else {
                None
            };
        self.with_level(|_, level, _| {
            let pos = BlockPos::new(pos[0], pos[1], pos[2]);
            // with_level rebuilds GameLevel with both fields None on every call.
            level.drop_entity = breaker;
            level.drop_breaker_at = Some(pos);
            level.block_dropped(pos, state, None, None, block_entity, drops, None)
        })
    }
    /// The block entity at `key` as a player's break finds it before the removal takes it out of
    /// the world. The saved data is read only when a `block_drops` listener will want it.
    pub(in crate::game) fn live_block_entity(
        &self,
        key: (i32, i32, i32),
        block: &str,
    ) -> Option<LiveBlockEntity> {
        let entity = self.world.block_entities.get(&key)?.clone();
        let data = (self.native_authoritative()
            && mod_api::__private::listened(mod_api::HookId::BlockDrops))
        .then(|| self.saved_block_entity_data(key, &entity, block))
        .flatten();
        Some((entity, data))
    }
    /// `Block.dropResources` for a player's break (`block_dropped`), lending the saved data
    /// captured before removing the original block entity. `at` is the position and broken
    /// state; `who` the breaker's UUID, their tool and the block entity's payload; `loot`
    /// the stacks and an experience override.
    pub(in crate::game) fn player_block_dropped(
        &mut self,
        at: ([i32; 3], BlockStateId),
        who: (Option<u128>, Option<&ItemStack>, Option<&Compound>),
        loot: (Vec<ItemStack>, Option<u32>),
    ) -> crate::game::spill::BlockDrops {
        let ([x, y, z], state) = at;
        let (breaker, tool, block_entity_data) = who;
        let block_entity = match block_entity_data {
            Some(data) => BlockDropEntity::Saved(data),
            None => BlockDropEntity::None,
        };
        self.with_level(|_, level, _| {
            level.block_dropped(
                BlockPos::new(x, y, z),
                state,
                breaker,
                tool,
                block_entity,
                loot.0,
                loot.1,
            )
        })
    }
    pub(in crate::game::server_side) fn prepare_part_break_experience(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
        experience: &mut Option<u32>,
    ) {
        if !self.native_authoritative()
            || !crate::game::api_values::block_state::block_listens(mod_api::HookId::ExpDrop, state)
        {
            return;
        }
        let tool = self.held().cloned();
        let breaker = Some(self.native_acting_id());
        *experience = self.with_level(|_, level, _| {
            let pos = BlockPos::new(pos[0], pos[1], pos[2]);
            let block_entity = level.resident_block_entity(pos);
            let amount = level
                .part_break_experience(pos, state, breaker, tool.as_ref(), block_entity)
                .unwrap_or_else(|| {
                    sunrise_world::level::removal::break_experience(
                        &mut level.world.random,
                        &level.facts.get(state).name,
                    )
                    .unwrap_or(0)
                });
            Some(
                if tool
                    .as_ref()
                    .is_some_and(|tool| tool.enchantment_level("minecraft:silk_touch") > 0)
                    && &*level.facts.get(state).name != "minecraft:spawner"
                {
                    0
                } else {
                    amount
                },
            )
        });
    }
}

impl GameLevel<'_> {
    /// Capture the resident block entity's generation at `pos`, if one currently exists.
    pub(in crate::game) fn resident_block_entity(
        &self,
        pos: BlockPos,
    ) -> crate::game::spill::BlockDropEntity<'static> {
        BlockDropEntity::at(self.world, (pos.x, pos.y, pos.z), None)
    }
}
