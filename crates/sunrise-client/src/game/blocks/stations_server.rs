#[allow(unused_imports)]
pub(in crate::game::server_side) use super::api_values::{TryIntoApi, TryIntoEngine};
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::smithing_icons::smithing_slot_icons;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, Screen};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::stations::CyclingSlot;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::stations::EMPTY_TEMPLATE_ICONS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::stations::EnchantBook;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::stations::EngineTooltipLines;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::stations::GREY;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::stations::IN_TOOLTIP_BUILD;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::stations::INPUT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::stations::Kind;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::stations::RESULT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::stations::Station;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::stations::TooltipBuildGuard;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::stations::WORDS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::stations::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::TooltipElement;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::cell::Cell;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    block_entity_item, brewing,
    containers::BlockEntity,
    hud::{HudBatch, rgba},
    items::ItemStack,
    random_source,
    stations::{self, AnvilResult, EnchantOffers},
    text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::random::{LegacyRandom, RandomSource};

impl Game<'_> {
    /// `ServerPlayer.giveExperienceLevels`: the mods of a server player may change
    /// or cancel the levels, and every fifth level gained plays the level-up sound
    /// (at most every 100 ticks).
    pub(in crate::game) fn add_experience_levels(&mut self, mut levels: i32) {
        // ServerPlayer.giveExperienceLevels
        if levels == 0 {
            return;
        }
        if mod_api::__private::listened(mod_api::HookId::PlayerXpLevelChange)
            && let (Some(player), Some(name)) = (
                self.server.lan_acting_uuid,
                self.server.lan_acting_name.clone(),
            )
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
        {
            // MerchantMenu.removed: the carried stack and the payment go back.
            self.menu_session(|menu, env, _| {
                menu.merchant_close(env);
                Some(true)
            });
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
        if let Some(volume) = self.server.level_up_sound.give_levels(amount, level) {
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
    ) -> sunrise_client::Result<()> {
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
