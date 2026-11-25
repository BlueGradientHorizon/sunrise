//! Role-dispatched world/level/tick/transaction side operations (rewritten to `SideOps` traits at switch).
use crate::game::{Game, GameLevel};
use sunrise_client::{Result, crafting::Recipes, items::ItemStack};
use sunrise_core::position::BlockPos;
use sunrise_world::{
    level::{Behaviours, Ticks},
    registry::BlockStateId,
};

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn side_with_level_access<R>(
        &mut self,
        placement_preview: bool,
        f: impl FnOnce(&Behaviours, &mut GameLevel<'_>, &mut Ticks) -> R,
    ) -> R {
        if self.is_logical_server() {
            self.server_with_level_access(placement_preview, f)
        } else {
            self.client_with_level_access(placement_preview, f)
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_after_with_level(&mut self) {
        if self.is_logical_server() {
            self.server_after_with_level()
        } else {
            self.client_after_with_level()
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_set_block_live(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
        flags: i32,
        create_entity: bool,
    ) -> bool {
        if self.is_logical_server() {
            self.server_set_block_live(pos, state, flags, create_entity)
        } else {
            self.client_set_block_live(pos, state, flags, create_entity)
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_update_chunk_tickets(&mut self) {
        if self.is_logical_server() {
            self.server_update_chunk_tickets()
        } else {
            self.client_update_chunk_tickets()
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_section_mesh_pending_in(&self, section: (i32, i32, i32)) -> bool {
        if self.is_logical_server() {
            self.server_section_mesh_pending_in(section)
        } else {
            self.client_section_mesh_pending_in(section)
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_landing_change_cancelled(
        &mut self,
        player: Option<u128>,
        position: BlockPos,
        from: BlockStateId,
        to: BlockStateId,
    ) -> bool {
        if self.is_logical_server() {
            self.server_landing_change_cancelled(player, position, from, to)
        } else {
            self.client_landing_change_cancelled(player, position, from, to)
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_tick(&mut self) -> Result<()> {
        if self.is_logical_server() {
            self.server_tick()
        } else {
            self.client_tick()
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_drop_stack_from_eyes(
        &mut self,
        stack: ItemStack,
        thrown_from_hand: bool,
    ) -> Option<u32> {
        if self.is_logical_server() {
            self.server_drop_stack_from_eyes(stack, thrown_from_hand)
        } else {
            self.client_drop_stack_from_eyes(stack, thrown_from_hand)
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_give_experience_points(&mut self, points: i32) {
        if self.is_logical_server() {
            self.server_give_experience_points(points)
        } else {
            self.client_give_experience_points(points)
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_journal_entity(&mut self, id: u32) {
        if self.is_logical_server() {
            self.server_journal_entity(id)
        } else {
            self.client_journal_entity(id)
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_active_level_ref(
        &self,
        epoch: mod_api::WorldEpoch,
    ) -> Option<mod_api::LevelRef> {
        if self.is_logical_server() {
            self.server_active_level_ref(epoch)
        } else {
            self.client_active_level_ref(epoch)
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_recipes(&self) -> &Recipes {
        if self.is_logical_server() {
            self.server_recipes()
        } else {
            self.client_recipes()
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_reset_entity_residency(&mut self) {
        if self.is_logical_server() {
            self.server_reset_entity_residency()
        } else {
            self.client_reset_entity_residency()
        }
    }

    #[inline(always)]
    pub(in crate::game) fn side_game_side_meta(
        &self,
    ) -> (Option<u128>, Option<mod_api::LevelRef>, bool) {
        if self.is_logical_server() {
            self.server_game_side_meta()
        } else {
            self.client_game_side_meta()
        }
    }
}
