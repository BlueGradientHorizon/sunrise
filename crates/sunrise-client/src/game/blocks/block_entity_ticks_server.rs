use super::Game;
use crate::game::block_entity_ticks::Enclosing;
use sunrise_client_core::Result;
use sunrise_gameplay::block_entity_ticks::block_ticking_at;
use sunrise_level::{
    block_entities::{Pos, Ticking},
    containers::{BlockEntity, FurnaceKind},
};
use sunrise_sim::furnace::FurnaceCooking;
use sunrise_world::registry::BlockStateId;

impl Drop for Enclosing<'_, '_> {
    fn drop(&mut self) {
        // A pass that ended early left its own; a pass in progress is known by them.
        self.game.side.item_columns = None;
        self.game.side.pass_mobs = None;
        if let Some(scratch) = self.scratch.take() {
            (
                self.game.side.item_columns,
                self.game.side.pass_mobs,
                self.game.spawner_tick_positions,
                self.game.trial_spawner_tick_positions,
            ) = scratch;
        }
    }
}

impl Game<'_, crate::game::server_side::ServerState> {
    /// `LevelChunk.isTicking` (block coordinates): a block-ticking chunk from
    /// every player's tickets (`update_chunk_tickets`), its entities loaded
    /// (`ServerLevel.areEntitiesLoaded`).
    pub(in crate::game) fn block_ticking_area(&self) -> impl Fn(i32, i32) -> bool + use<> {
        let levels = self.side.tickets.0.levels();
        let pending = self.world.entities_pending.clone();
        move |x, z| block_ticking_at(&levels, &pending, x, z)
    }
    /// `AbstractFurnaceBlockEntity.serverTick` of the furnace at `pos`, and
    /// its block's `lit` following it.
    pub(in crate::game) fn tick_furnace(&mut self, pos: Pos) -> Result<()> {
        // What the hooks need from before the tick, without cloning stacks: the
        // fuel's id if the furnace can be out after this tick's countdown, the
        // input's id (a last item is gone after it smelts) and the result count.
        let before = match self.world.block_entities.get(&pos) {
            Some(BlockEntity::Furnace(f)) => (
                f.cooking_timer,
                (f.lit_remaining <= 1)
                    .then(|| f.items[1].as_ref().map(|s| s.item.clone()))
                    .flatten(),
                f.items[0].as_ref().map(|s| s.item.clone()),
                f.items[2].as_ref().map_or(0, |s| s.count),
            ),
            _ => return Ok(()),
        };
        let mut drops = Vec::new();
        let (lit, burning, smelted, smelting, fueled) = {
            let needs_part = mod_api::__private::has_part_hook(mod_api::HookId::CraftingRemainder)
                && matches!(self.world.block_entities.get(&pos), Some(BlockEntity::Furnace(f))
                    if f.lit_remaining <= 1 && f.items[1].as_ref().is_some_and(|s|
                        mod_api::__private::item_listens(mod_api::HookId::CraftingRemainder, &s.item)));
            let Some(BlockEntity::Furnace(furnace)) = self.world.block_entities.get_mut(&pos)
            else {
                return Ok(());
            };
            let was_lit = furnace.count_down_burn();
            let Some(BlockEntity::Furnace(furnace)) = self.world.block_entities.get(&pos) else {
                return Ok(());
            };
            let items = world_items!(self);
            let mut random = self.world.random.clone();
            let plan = furnace.prepare_tick(
                &self.loop_context,
                world_recipes!(self),
                items,
                was_lit,
                |fuel| {
                    sunrise_level::providers::Evaluation::new(self.active_providers(), &mut random)
                        .cooking_fuel(
                            fuel,
                            &mut sunrise_sim::furnace::FurnaceConditions {
                                world: &super::advancements::View(self),
                                pos: [pos.0, pos.1, pos.2],
                            },
                        )
                },
            );
            self.world.random = random;
            let step = if needs_part {
                let mut furnace = furnace.clone();
                let step = plan.apply(&mut furnace, &mut drops, |s| self.crafting_remainder(s));
                self.world
                    .block_entities
                    .insert(pos, BlockEntity::Furnace(furnace));
                step
            } else {
                let Some(BlockEntity::Furnace(furnace)) = self.world.block_entities.get_mut(&pos)
                else {
                    return Ok(());
                };
                plan.apply(furnace, &mut drops, |s| {
                    sunrise_sim::crafting::CraftingRemainder::crafting_remainder(items, s)
                })
            };
            let Some(BlockEntity::Furnace(f)) = self.world.block_entities.get(&pos) else {
                return Ok(());
            };
            let burning = before.1.filter(|_| step.consumed_fuel);
            let smelted = f.items[2]
                .as_ref()
                .filter(|after| after.count > before.3)
                .map(|after| (before.2.clone(), after.item.clone()));
            let started = before.0 == 0 && f.cooking_timer > 0;
            let smelting = started
                .then(|| f.items[0].as_ref().map(|item| item.item.clone()))
                .flatten();
            (step.lit, burning, smelted, smelting, step.consumed_fuel)
        };
        // `AbstractFurnaceBlockEntity.serverTick`: `burn` and consuming fuel
        // set the changed flag, so a completed smelt or a refuel marks the
        // chunk and tells comparators even though the block state stays LIT.
        if smelted.is_some() || fueled {
            self.container_changed(pos);
        }
        if let Some(item) = smelting {
            self.native_furnace_start_smelt([pos.0, pos.1, pos.2], &item);
        }
        if let Some(fuel) = burning {
            self.native_furnace_burn([pos.0, pos.1, pos.2], &fuel);
        }
        if let Some((Some(item), result)) = smelted {
            self.native_furnace_smelt([pos.0, pos.1, pos.2], &item, &result);
        }
        for stack in drops {
            self.campfire_drop(pos, stack);
        }
        let Some(lit) = lit else {
            if matches!(self.world.block_entities.get(&pos), Some(BlockEntity::Furnace(f)) if f.idle())
            {
                self.world.block_entities.sleep(pos);
            }
            return Ok(());
        };
        let state = self.world.block(pos.0, pos.1, pos.2);
        if let Some(mut s) = self.pack.assets.registry.state(state).cloned()
            && s.properties.contains_key("lit")
        {
            s.properties.insert("lit".into(), lit.to_string());
            if let Ok(id) = self.pack.assets.registry.resolve(&s) {
                self.level_set([pos.0, pos.1, pos.2], id)?;
            }
        }
        Ok(())
    }
    /// `BlockState.getTicker` of every state, for the block entities ticked
    /// here. A creaking heart keeps its slot while uprooted (see
    /// `tick_creaking_heart`).
    pub(in crate::game) fn ticking_states(&self) -> Vec<Ticking> {
        let registry = self.pack.assets.registry;
        (0..registry.len())
            .map(|i| {
                let state = BlockStateId(i as u32);
                let name: &str = &self.res.facts().get(state).name;
                let factory = sunrise_client_core::block_entity_types::factory_block(name);
                match factory {
                    "minecraft:hopper"
                    | "minecraft:crafter"
                    | "minecraft:beacon"
                    | "minecraft:creaking_heart"
                    | "minecraft:spawner"
                    | "minecraft:trial_spawner"
                    | "minecraft:vault"
                    | "minecraft:beehive"
                    | "minecraft:bee_nest"
                    | "minecraft:brewing_stand"
                    | "minecraft:daylight_detector"
                    | "minecraft:end_gateway"
                    // Lit, cookTick; unlit, cooldownTick.
                    | "minecraft:campfire"
                    | "minecraft:soul_campfire" => Ticking::Yes,
                    // Every state but dry has a ticker.
                    "minecraft:potent_sulfur" => {
                        let dry = registry
                            .state(state)
                            .and_then(|s| s.properties.get("potent_sulfur_state"))
                            .is_some_and(|v| v == "dry");
                        if dry { Ticking::Idle } else { Ticking::Yes }
                    }
                    "minecraft:jukebox" => {
                        let record = registry
                            .state(state)
                            .and_then(|s| s.properties.get("has_record"))
                            .is_some_and(|v| v == "true");
                        if record { Ticking::Yes } else { Ticking::Idle }
                    }
                    n if FurnaceKind::from_block(n).is_some() => Ticking::Yes,
                    _ if !name.starts_with("minecraft:") && self.native_ticks(name) => Ticking::Yes,
                    _ => Ticking::No,
                }
            })
            .collect()
    }
}
