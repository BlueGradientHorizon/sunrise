#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::loot::*;

impl Game<'_> {
    /// The mods' say on a broken block's drops (`native_loot_drops`).
    pub(in crate::game) fn native_block_loot(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
        player: Option<u128>,
        tool: Option<&ItemStack>,
        explosion: Option<f32>,
        drops: &mut Vec<ItemStack>,
    ) {
        if !mod_api::__private::listened(mod_api::HookId::LootDrops) {
            return;
        }
        let Ok(snapshot) = tool.try_into_api() else {
            return;
        };
        let items = match &self.world_tables.0 {
            Some(tables) => &tables.items,
            None => self.res.items(),
        };
        let table = items.block_loot_table(self.pack.assets.registry, state);
        let Ok(state) = state.try_into_api() else {
            return;
        };
        let context = LootDrops::from(mod_api::__private::built::LootDrops {
            weapon: None,
            damage_source: None,
            table: table.as_deref().unwrap_or_default(),
            dimension: self.dim.id(),
            origin: pos.map(|c| f64::from(c) + 0.5),
            block: Some((pos, state)),
            entity: None,
            player,
            tool: snapshot.as_ref(),
            explosion,
            used: false,
        });
        self.native_loot_drops(&context, drops);
        drops.retain(|stack| stack.count > 0 && &*stack.item != "minecraft:air");
    }
}
