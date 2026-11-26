use crate::game::ServerGame;
use sunrise_items::items::ItemStack;

impl ServerGame<'_> {
    pub(in crate::game) fn server_item_combat_actor(
        &self,
        actor: mod_api::LivingRef,
    ) -> Option<(bool, bool, f64, bool)> {
        use super::commands::Target;
        let target = self.server_living_read_target(actor)?;
        Some(match target {
            Target::Player => {
                let p = self.common.player.as_ref()?;
                (true, self.common.creative, p.fall_distance, p.fall_flying)
            }
            Target::Guest(conn) => {
                let guest = self.command_guest(conn)?;
                let p = guest.motion.as_ref()?;
                (true, guest.creative(), p.fall_distance, p.fall_flying)
            }
            Target::Mob(id) => {
                let mob = self.common.mobs.iter().find(|m| m.id == id && !m.removed)?;
                (false, false, mob.fall_distance, mob.extra.fall_flying)
            }
            Target::Stand(_) | Target::Dragon(_) => (false, false, 0.0, false),
            _ => return None,
        })
    }

    /// Roll through the game's enchantment registry and the active level's random source.
    pub(in crate::game) fn item_resource_enchant_randomly(
        &mut self,
        stack: &ItemStack,
        level: mod_api::LevelRef,
        power: i32,
        pool: &mod_api::Identifier,
    ) -> Result<Option<ItemStack>, mod_api::StackWriteError> {
        if level.dimension() != self.common.dim.key() || self.native_level_world(level).is_none() {
            return Ok(None);
        }
        let items = server_world_items!(self);
        let enchantments = std::sync::Arc::clone(self.item_enchantments());
        let pool = enchantments.tag(&pool.to_string());
        if pool.is_empty() {
            return Ok(None);
        }
        let value = enchantments.enchantable_of(stack);
        let picked = enchantments.select_with_value(
            items,
            &mut self.common.world.random,
            &stack.item,
            power,
            pool,
            value,
        );
        Ok(sunrise_sim::stations::apply_enchantment_list(stack, picked))
    }
}

use sunrise_gameplay_common::game::tables::GameItems as _;

use sunrise_gameplay_common::game::commands_common::CommandsCommonGameExt2 as _;

use sunrise_gameplay_common::game::native_level_refs_common::NativeLevelRefsCommonGameExt1 as _;
