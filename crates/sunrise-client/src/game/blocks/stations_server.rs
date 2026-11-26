//! Server-side workstation logic (opening/closing stations, anvil/grindstone/smithing
//! side effects, experience levels, and brewing stand server ticking).
use super::Game;
use crate::game::stations_common::{Kind, Station};
use sunrise_core::random::RandomSource;
use sunrise_items::items::ItemStack;
use sunrise_level::{brewing, containers::BlockEntity};
use sunrise_sim::stations;

impl Game<'_, crate::game::server_side::ServerState> {
    /// Server arm of [`Game::side_station_changed`].
    pub(in crate::game) fn server_station_changed(&mut self) {
        let Some(mut s) = self.station.take() else {
            return;
        };
        let creative = self.creative;
        let (enchantments, data) = (
            std::sync::Arc::clone(self.item_enchantments()),
            self.stations(),
        );
        let items = self.items();
        match s.kind {
            Kind::Enchanting => {
                s.bookcases = self.count_bookshelves(s.pos);
                s.offers = stations::enchant_offers(
                    &enchantments,
                    items,
                    self.level.xp_seed,
                    s.bookcases,
                    s.inputs[0].as_ref(),
                );
            }
            Kind::Anvil => {
                let name = self.anvil_item_name(&s);
                let hover = s.inputs[0]
                    .as_ref()
                    .map(|i| {
                        sunrise_client_core::lang::returned_to_server(|| {
                            self.stack_hover_name_text(i).plain()
                        })
                    })
                    .unwrap_or_default();
                s.anvil = stations::anvil_result(
                    &enchantments,
                    items,
                    s.inputs[0].as_ref(),
                    s.inputs[1].as_ref(),
                    Some(&name),
                    &hover,
                    creative,
                );
                s.result = s.anvil.result.clone();
            }
            Kind::Grindstone => {
                s.result = stations::grindstone_result(
                    &self.loop_context,
                    &enchantments,
                    items,
                    s.inputs[0].as_ref(),
                    s.inputs[1].as_ref(),
                );
            }
            Kind::Smithing => {
                s.result = data.recipes.smithing_result(
                    &self.loop_context,
                    &data.enchantments,
                    s.inputs[0].as_ref(),
                    s.inputs[1].as_ref(),
                    s.inputs[2].as_ref(),
                );
            }
            Kind::Merchant | Kind::Beacon | Kind::Cartography | Kind::Loom | Kind::Stonecutter => {}
        }
        self.station = Some(s);
    }

    /// Server arm of [`Game::side_combiner_take_effects`].
    pub(in crate::game) fn server_combiner_take_effects(
        &mut self,
        kind: Kind,
        pos: [i32; 3],
        level_change: i32,
    ) -> bool {
        if kind == Kind::Smithing {
            self.server_level_event_at(1044, pos, 0);
            return false;
        }
        self.give_levels(level_change);
        self.anvil_used(pos, self.creative)
    }

    /// `AnvilMenu.onTake`'s level part: out of creative a 12% chance
    /// (`AnvilBlock.damage`) to wear the anvil a stage, or break a damaged one
    /// (level event 1029); otherwise level event 1030. True when it broke.
    pub(in crate::game) fn anvil_used(&mut self, pos: [i32; 3], creative: bool) -> bool {
        let state = self.world.block(pos[0], pos[1], pos[2]);
        let name = self.res.facts().get(state).name.clone();
        let anvil = matches!(
            &*name,
            "minecraft:anvil" | "minecraft:chipped_anvil" | "minecraft:damaged_anvil"
        );
        if !creative && anvil && self.world.random.next_float() < 0.12 {
            match &*name {
                "minecraft:anvil" => {
                    let _ = self.set_state_named(pos, "minecraft:chipped_anvil", &["facing"]);
                    self.native_anvil_damaged(self.dim.id(), pos, &name, "minecraft:chipped_anvil");
                }
                "minecraft:chipped_anvil" => {
                    let _ = self.set_state_named(pos, "minecraft:damaged_anvil", &["facing"]);
                    self.native_anvil_damaged(self.dim.id(), pos, &name, "minecraft:damaged_anvil");
                }
                _ => {
                    let _ = self.world.set(pos[0], pos[1], pos[2], self.world.air);
                    self.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
                    self.native_anvil_damaged(self.dim.id(), pos, &name, "minecraft:air");
                    self.server_level_event_at(1029, pos, 0);
                    return true;
                }
            }
        }
        self.server_level_event_at(1030, pos, 0);
        false
    }

    /// The grindstone result's `onTake` with a level (`amount`: the menu's
    /// `getExperienceAmount` sum): the orbs at the block's centre
    /// (`ExperienceOrb.award`), then level event 1042, the grindstone's sound
    /// for everyone around.
    pub(in crate::game) fn grindstone_taken(&mut self, pos: [i32; 3], amount: i32) {
        let xp = stations::grindstone_xp_roll(amount, &mut self.world.random);
        self.grindstone_award(pos, xp);
    }

    /// The rest of `onTake` with the experience `xp` decided: the orbs and
    /// level event 1042.
    pub(in crate::game) fn grindstone_award(&mut self, pos: [i32; 3], xp: u32) {
        if xp > 0 {
            self.spawn_orbs(pos.map(|v| f64::from(v) + 0.5), xp);
        }
        self.after_commit(super::transactions::After::GrindstoneEvent(pos));
    }

    pub(in crate::game) fn give_or_drop(&mut self, stack: ItemStack) {
        if let Some(rest) = self.add_stack_to_inventory(stack)
            && let Some(eye) = self.eye_position()
        {
            // Inventory.placeItemBackInInventory: `player.drop(stack, false)`.
            self.throw_from_eyes(rest, eye, false, false);
        }
    }

    /// `Player.giveExperienceLevels`, and the level change reported to the mods.
    pub(in crate::game) fn give_levels(&mut self, levels: i32) {
        let old_level = self.experience.level;
        self.add_experience_levels(levels);
        self.native_player_level_changed(old_level);
    }

    /// `ServerPlayer.giveExperienceLevels`: the mods of a server player may change
    /// or cancel the levels, and every fifth level gained plays the level-up sound
    /// (at most every 100 ticks).
    pub(in crate::game) fn add_experience_levels(&mut self, mut levels: i32) {
        // ServerPlayer.giveExperienceLevels
        if levels == 0 {
            return;
        }
        if mod_api::__private::listened(mod_api::HookId::PlayerXpLevelChange)
            && let (Some(player), Some(name)) =
                (self.side.lan_acting_uuid, self.side.lan_acting_name.clone())
        {
            let mut cancel = false;
            self.native_player_xp_level_change(player, &name, &mut levels, &mut cancel);
            if cancel {
                return;
            }
        }
        self.experience.add_levels(levels);
        let level = self.experience.level;
        self.play_level_up(levels, level);
    }

    /// `AbstractContainerMenu.clearContainer`: inputs go back to the player.
    pub(in crate::game) fn close_station_server(&mut self) {
        assert!(
            self.is_logical_server(),
            "close_station_server called on Client role"
        );
        if self
            .station
            .as_ref()
            .is_some_and(|s| s.kind == Kind::Merchant)
            && let Some(stack) = self.carried.take()
        {
            self.give_or_drop(stack);
        }
        let Some(mut station) = self.station.take() else {
            return;
        };
        if let Some(m) = station.merchant.as_mut() {
            self.merchant_closed(m);
        }
        for stack in station.inputs.into_iter().flatten() {
            self.give_or_drop(stack);
        }
    }

    /// `Player.onEnchantmentPerformed`: the cost leaves the level directly, not
    /// through `giveExperienceLevels`, and the mods hear of the level change.
    pub(in crate::game) fn enchantment_performed(&mut self, cost: i32) {
        let old_level = self.experience.level;
        self.experience.add_levels(-cost);
        self.native_player_level_changed(old_level);
    }

    /// Opens a workstation menu for the block; false when it is not one.
    pub(in crate::game) fn open_station(&mut self, pos: [i32; 3], name: &str) -> bool {
        let Some(kind) = Kind::from_block(name) else {
            return false;
        };
        if self.level.xp_seed == 0 {
            self.level.xp_seed = self.world.random.next_int();
        }
        self.station = Some(Station::new(kind, pos));
        self.station_changed();
        self.beacon_opened();
        // The block's interaction statistic.
        let stat = match name {
            "minecraft:crafting_table" => "interact_with_crafting_table",
            "minecraft:furnace" => "interact_with_furnace",
            "minecraft:blast_furnace" => "interact_with_blast_furnace",
            "minecraft:smoker" => "interact_with_smoker",
            "minecraft:brewing_stand" => "interact_with_brewingstand",
            "minecraft:beacon" => "interact_with_beacon",
            "minecraft:grindstone" => "interact_with_grindstone",
            "minecraft:smithing_table" => "interact_with_smithing_table",
            "minecraft:stonecutter" => "interact_with_stonecutter",
            "minecraft:loom" => "interact_with_loom",
            "minecraft:cartography_table" => "interact_with_cartography_table",
            n if n.ends_with("anvil") => "interact_with_anvil",
            _ => "",
        };
        if !stat.is_empty() {
            self.award_custom_stat(stat, 1);
        }
        true
    }

    pub(in crate::game::server_side) fn play_at(
        &mut self,
        sound: &str,
        pos: [i32; 3],
        volume: f32,
        pitch: f32,
    ) {
        let at = [
            f64::from(pos[0]) + 0.5,
            f64::from(pos[1]) + 0.5,
            f64::from(pos[2]) + 0.5,
        ];
        self.server_play_sound(sound, at, volume, pitch);
    }

    /// `Player.giveExperienceLevels`: all players near its feet hear the sound.
    pub(in crate::game::server_side) fn play_level_up(&mut self, amount: i32, level: i32) {
        if let Some(volume) = self.side.level_up_sound.give_levels(amount, level) {
            let at = self
                .player
                .as_ref()
                .map(|p| [p.position.x, p.position.y, p.position.z]);
            if let Some(at) = at {
                self.server_play_sound("minecraft:entity.player.levelup", at, volume, 1.0);
            }
        }
    }

    /// `BrewingStandBlockEntity.serverTick` of the brewing stand at `pos`, in
    /// its place among the level's block entity tickers.
    pub(in crate::game) fn tick_brewing_stand(
        &mut self,
        pos: (i32, i32, i32),
    ) -> sunrise_client_core::Result<()> {
        let fuel = self.brewing_fuel_remainder(pos);
        let Some(BlockEntity::Brewing(b)) = self.world.block_entities.get_mut(&pos) else {
            return Ok(());
        };
        let was_idle = b.brew_time <= 0;
        // `serverTick` takes the fuel item only while the stand has no fuel
        // uses left, which `out.fuel_taken` reports.
        let fuel_item = b.items[4].as_ref().map(|s| s.item.clone());
        let (mut out, due) = b.tick_to_brew(
            &world_stations!(self).brewing,
            world_items!(self),
            self.res.providers(),
            &|s| fuel.resolve(world_items!(self), s),
        );
        let started = was_idle && b.brew_time > 0;
        let ingredient = started.then(|| b.ingredient.clone()).flatten();
        let hooked = !mod_api::__private::mods().is_empty();
        let stacks = (due && hooked).then(|| b.items.clone());
        let at = [pos.0, pos.1, pos.2];
        let mut post = false;
        if due {
            // `doBrew`, with PotionBrewEvent.Pre before it and .Post after.
            let asked = stacks.map(|mut stacks| {
                let mut cancel = false;
                self.native_potion_brew_pre(at, &mut stacks, &mut cancel);
                (stacks, cancel)
            });
            // A cancelled Pre fires Post itself, with the slots the mods left.
            let cancelled = matches!(asked, Some((_, true)));
            let rest = self.brewing_ingredient_remainder(pos, cancelled);
            let Some(BlockEntity::Brewing(b)) = self.world.block_entities.get_mut(&pos) else {
                return Ok(());
            };
            let brewing = &world_stations!(self).brewing;
            b.brew_after_pre(asked, brewing, &|_| rest.clone(), &mut out);
            self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
            post = hooked && (out.brewed || cancelled);
        }
        if out.fuel_taken
            && let Some(item) = fuel_item
        {
            self.native_brewing_fuel(at, &item);
        }
        if let Some(ingredient) = ingredient {
            self.native_brewing_start(at, &ingredient);
        }
        if !was_idle || started || out != brewing::BrewTick::default() {
            self.container_changed(pos);
        }
        let (x, y, z) = pos;
        // `doBrew` drops the ingredient's remainder through
        // `Containers.dropItemStack`, before the brew's level event: no
        // `block_drops` test and no pickup delay.
        for d in std::mem::take(&mut out.drops) {
            self.drop_item_stack_containers(at, d);
        }
        if out.brewed {
            self.native_brewing_complete(at);
            self.play_at("minecraft:block.brewing_stand.brew", at, 1.0, 1.0);
        }
        // PotionBrewEvent.Post ends `doBrew`, after its sound and the
        // ingredient's remainder; the bottles' block state follows it.
        if post && let Some(BlockEntity::Brewing(b)) = self.world.block_entities.get(&pos) {
            let mut stacks = b.items.clone();
            self.native_potion_brew_post(at, &mut stacks);
            if let Some(BlockEntity::Brewing(b)) = self.world.block_entities.get_mut(&pos) {
                b.items = stacks;
            }
        }
        let Some(BlockEntity::Brewing(b)) = self.world.block_entities.get_mut(&pos) else {
            return Ok(());
        };
        b.sync_bottles(&mut out);
        if out != brewing::BrewTick::default() {
            self.world.modified.insert((x >> 4, z >> 4));
            if let Some(bottles) = out.bottles {
                let state = self.world.block(x, y, z);
                if let Some(mut s) = self.pack.assets.registry.state(state).cloned()
                    && AsRef::<str>::as_ref(&s.name) == "minecraft:brewing_stand"
                {
                    for (i, has) in bottles.iter().enumerate() {
                        s.properties
                            .insert(format!("has_bottle_{i}"), has.to_string());
                    }
                    if let Ok(id) = self.pack.assets.registry.resolve(&s) {
                        self.level_set_with_flags(
                            [x, y, z],
                            id,
                            sunrise_world::level::flags::UPDATE_CLIENTS,
                        )?;
                    }
                }
            }
        }
        if matches!(self.world.block_entities.get(&pos), Some(BlockEntity::Brewing(b)) if b.idle())
        {
            self.world.block_entities.sleep(pos);
        }
        Ok(())
    }
}
