//! Common enchanting-table bookshelf power queries (`EnchantmentMenu.slotsChanged`).
use super::{Game, tables::BlockProviderQuery};
use mod_api::LevelRef;
use sunrise_core::position::BlockPos;
use sunrise_sim::stations::bookshelf_offsets;
use sunrise_world::registry::BlockStateId;

pub trait BookshelvesCommonGameExt1<'__sg>: crate::game::SideGame<'__sg> {
    /// The enchanting power around the table at `pos`, truncated as NeoForge's `(int) power`.
     fn count_bookshelves(&self, pos: [i32; 3]) -> i32 {
        let table = BlockPos::new(pos[0], pos[1], pos[2]);
        let level = self.native_current_level();
        let power: f32 = bookshelf_offsets()
            .into_iter()
            .filter(|o| {
                self.enchant_transmits(self.world.block(
                    pos[0] + o[0] / 2,
                    pos[1] + o[1],
                    pos[2] + o[2] / 2,
                ))
            })
            .filter_map(|o| {
                let at = BlockPos::new(pos[0] + o[0], pos[1] + o[1], pos[2] + o[2]);
                let state = self.world.block(at.x, at.y, at.z);
                self.enchant_power_for_table(level, state, at, table)
            })
            .sum();
        (power as i32).max(0)
    }

     fn enchant_transmits(&self, state: BlockStateId) -> bool {
        self.items()
            .block_tags
            .get("minecraft:enchantment_power_transmitter")
            .is_some_and(|tag| tag.contains(&*self.res.facts().get(state).name))
    }

    /// Raw provider answer. The explicit read returns it as given.
     fn enchant_power(
        &self,
        level: Option<LevelRef>,
        state: BlockStateId,
        pos: BlockPos,
        table: BlockPos,
    ) -> Option<f32> {
        let mut power = self.default_enchant_power(state);
        if !super::api_values::block_state::block_listens(mod_api::HookId::EnchantPowerBonus, state)
        {
            return Some(power);
        }
        self.with_block_provider_query(state, BlockProviderQuery::EnchantPower, || {
            self.native_enchant_power_bonus(level, state, pos, table, &mut power);
            power
        })
    }

     fn default_enchant_power(&self, state: BlockStateId) -> f32 {
        self.items()
            .default_enchant_power(&self.res.facts().get(state).name)
    }

    /// Offers and particles ignore non-finite overrides and keep the tag default.
     fn enchant_power_for_table(
        &self,
        level: Option<LevelRef>,
        state: BlockStateId,
        pos: BlockPos,
        table: BlockPos,
    ) -> Option<f32> {
        self.enchant_power(level, state, pos, table).map(|power| {
            if power.is_finite() {
                power
            } else {
                self.default_enchant_power(state)
            }
        })
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> BookshelvesCommonGameExt1<'__sg> for G {}


use crate::game::tables::GameItems as _;

use crate::game::tables::WorldTablesExt as _;

use crate::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;
