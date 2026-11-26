//! An item in use, tick by tick (`LivingEntity.startUsingItem`, `updatingUsingItem`,
//! `releaseUsingItem`, `stopUsingItem`): eating and drinking, goat horns, bundles, and the
//! vibrations a use makes.
use super::*;

impl crate::game::ServerGame<'_> {
    /// `Consumable.startConsuming` on the server for a held consumable.
    pub(in crate::game) fn server_start_eating(
        &mut self,
        info: &sunrise_items::items::ItemInfo,
    ) -> bool {
        let Some(stack) = self.common.held().cloned() else {
            return false;
        };
        let Some(consumable) = self.common.consumable(&stack) else {
            return false;
        };
        if sunrise_sim::item_sync::food(&stack, info.food).is_some()
            && (self.common.player.is_none()
                || !self.common.can_eat(
                    Some(&stack),
                    sunrise_sim::item_sync::can_always_eat(&stack, info.can_always_eat),
                ))
        {
            return false;
        }
        // Instant consumables finish at once without starting a use countdown.
        if item_use_common::consumes_at_once(consumable.ticks()) {
            // Cooldown from ItemStack.use's applyAfterUseComponentSideEffects.
            self.apply_use_cooldown(&stack);
            self.finish_consuming(info, &consumable, &stack);
            return true;
        }
        self.common.start_using_item();
        true
    }
    /// `InstrumentItem.use` for a goat horn: the player holds it up for the
    /// instrument's `use_duration`, its sound plays at the player
    /// (`SoundSource.RECORDS`, volume `range / 16`), `INSTRUMENT_PLAY` is
    /// posted, the used statistic counts and the horn is on cooldown as long.
    /// A horn whose instrument is unknown fails, which still ends the click.
    pub(in crate::game::server_side) fn use_instrument(&mut self, stack: &ItemStack) {
        let Some(instrument) = self.common.res.items().instrument(stack) else {
            return;
        };
        self.common.start_using_item();
        self.server_play_player_bound_sound(
            &instrument.sound,
            2,
            instrument.range / 16.0,
            1.0,
            true,
        );
        self.post_entity_event("minecraft:instrument_play", sunrise_sim::mobs::Who::Player);
        if instrument.use_duration > 0.0 {
            let group = sunrise_items::cooldowns::group(stack, None);
            self.add_cooldown(group, instrument.use_ticks());
        }
        self.award_stat("used", &stack.item, 1);
    }
    /// `SpyglassItem.releaseUsing` / `finishUsingItem` (after 1200 ticks).
    pub(in crate::game) fn stop_spyglass(&mut self) {
        if let Some(at) = self.common.eye_position() {
            self.server_play_sound("minecraft:item.spyglass.stop_using", at, 1.0, 1.0);
        }
    }
    pub(in crate::game) fn server_on_start_using_item(&mut self, stack: &ItemStack) {
        self.use_vibration(stack, "minecraft:item_interact_start");
        self.server.crossbow_sounds = [false; 2];
    }
    pub(in crate::game) fn tick_using_server(&mut self) {
        assert!(
            self.is_logical_server(),
            "tick_using_server called on Client role"
        );
        let Some(hand) = self.common.used_hand() else {
            return;
        };
        self.common.with_hand(hand, Self::tick_used_item);
    }
    /// `LivingEntity.updatingUsingItem` for the item in use, its hand acting.
    pub(in crate::game::server_side) fn tick_used_item(&mut self) {
        let Some((slot, ticks)) = self.common.using else {
            return;
        };
        // An emptied hand is a used-up stack (`/clear`, a broken shield): its
        // `useItem` is empty too, so the use ends without a vibration.
        let Some(stack) = self.common.stack_in(slot).cloned() else {
            self.common.using = None;
            self.common.use_start = Default::default();
            return;
        };
        // `isSameItem(getItemInHand(hand), useItem)`: another item stops the use
        // of the stack it began with, without a release. The same item goes on;
        // only the item is compared, so the start's stack is kept as it was.
        if let Some(item) = self.common.use_start.item.take_if(|item| item.item != stack.item) {
            self.stop_using_item(&item);
            return;
        }
        let hand_changed = self.use_hand_changed(slot);
        if !self.server_acting_use() || hand_changed {
            self.common.using = None;
            self.release_used_item(&stack, ticks, hand_changed);
            return;
        }
        // ServerPlayer.updateUsingItem: USING_ITEM every tick of the use.
        self.trigger_using_item(&stack);
        let Some(info) = self.items().get(&stack.item).cloned() else {
            self.common.using = None;
            return;
        };
        let going_on = self.common.use_item_ticked(&stack, ticks);
        // A mod's tick released it: nothing of this use is left to run.
        if self.common.using != Some((slot, ticks)) {
            return;
        }
        // updateUsingItem decrements even when the event suppressed the effects.
        if !going_on {
            self.common.using = Some((slot, ticks.saturating_add(1)));
            if self.common.use_start.ticks_set && &*stack.item != "minecraft:crossbow" {
                self.complete_using_item(slot, &stack, &info);
            }
            return;
        }
        self.tick_used_item_effects(slot, ticks, &stack, &info);
        // Re-read the counter after onUseTick: a Part can extend or shorten
        // the use through LivingRef. Never release a bow on this path.
        if item_use_common::ticks_left(self.common.use_duration, ticks.saturating_add(1)) <= 0
            && self.common.using == Some((slot, ticks.saturating_add(1)))
            && &*stack.item != "minecraft:crossbow"
        {
            self.complete_using_item(slot, &stack, &info);
        }
    }
    /// `ItemStack.onUseTick` and the ordinary item-specific countdown.
    fn tick_used_item_effects(
        &mut self,
        slot: usize,
        ticks: u32,
        stack: &ItemStack,
        info: &sunrise_items::items::ItemInfo,
    ) {
        if self.common.blocks_attacks(stack).is_some() {
            // ShieldItem: used for 72000 ticks, i.e. until released.
            if self.common.item_on_use_tick(stack, ticks) {
                self.common.using = Some((slot, ticks + 1));
            }
            return;
        }
        if self.tick_drinking(slot, ticks, stack) {
            return;
        }
        if &*stack.item == "minecraft:bow" {
            self.common.using = Some((slot, ticks + 1));
            return;
        }
        if &*stack.item == "minecraft:crossbow" {
            self.tick_crossbow(slot, ticks, stack);
            return;
        }
        // TridentItem and EnderEyeItem: used until released (an eye's
        // getUseDuration 0 never counts down to finishUsingItem).
        if matches!(&*stack.item, "minecraft:trident" | "minecraft:ender_eye") {
            self.common.using = Some((slot, ticks + 1));
            return;
        }
        // ItemStack.onUseTick: a spear stabs what its speed allows, until released.
        if sunrise_sim::spear_sim::Kinetic::of(&stack.item).is_some() {
            self.spear_tick(stack, ticks);
            self.common.using = Some((slot, ticks + 1));
            return;
        }
        if &*stack.item == "minecraft:spyglass" {
            // SpyglassItem.getUseDuration (1200, as a mod left it), then
            // completeUsingItem: finishUsingItem lowers it, and the finish hook.
            match item_use::count_down(ticks, self.common.use_duration) {
                Some(ticks) => self.common.using = Some((slot, ticks)),
                None => self.complete_using_item(slot, stack, info),
            }
            return;
        }
        if self.common.res.items().instrument(stack).is_some() {
            // The shared updateUsingItem countdown, including a mod's replacement.
            self.common.using = Some((slot, ticks + 1));
            return;
        }
        if &*stack.item == "minecraft:brush" {
            self.tick_brush(stack, slot, ticks);
            return;
        }
        if info.bundle {
            self.tick_bundle(slot, ticks);
            return;
        }
        if self.common.mod_use(stack) {
            self.tick_mod_use(slot, ticks, stack);
            return;
        }
        let Some(consumable) = self.common.consumable(stack) else {
            self.common.using = None;
            return;
        };
        // ItemStack.onUseTick: Consumable.shouldEmitParticlesAndSounds with
        // the remaining ticks before this tick counts them down.
        // the duration is getUseDuration's when the use started, as a mod's
        // Start and Tick left it.
        let remaining = self.common.use_duration - ticks as i32;
        if remaining > 0 && consumable.should_emit(remaining) {
            self.common.emit_consume(&consumable, stack, 5);
        }
        if !self.common.item_on_use_tick(stack, ticks) {
            return;
        }
        match item_use::count_down(ticks, self.common.use_duration) {
            Some(ticks) => self.common.using = Some((slot, ticks)),
            None => self.complete_using_item(slot, stack, info),
        }
    }
    /// `BundleItem.onUseTick`: the first tick and then every second tick
    /// from the 11th on drops a stack. `updateUsingItem` owns completion.
    pub(in crate::game::server_side) fn tick_bundle(&mut self, slot: usize, ticks: u32) {
        const USE_DURATION: i32 = 200;
        let remaining = item_use_common::ticks_left(self.common.use_duration, ticks);
        if remaining == USE_DURATION || (remaining < USE_DURATION - 10 && remaining % 2 == 0) {
            self.drop_bundle_content(slot);
        }
        self.common.using = Some((slot, ticks + 1));
    }
    /// `LivingEntity.releaseUsingItem` (`Item.releaseUsing`) for `stack`,
    /// used for `ticks`, its hand acting: a bow or a trident lets go (not
    /// when its hand changed), a spyglass stops scoping, and a mod's own
    /// release follows. A mod's `Stop` may skip the release. `stopUsingItem`
    /// ends the use after it.
    pub(in crate::game::server_side) fn release_used_item(
        &mut self,
        stack: &ItemStack,
        ticks: u32,
        hand_changed: bool,
    ) {
        let canceled = !hand_changed && !self.common.use_item_stopped(stack, ticks);
        let (release, spyglass) = item_use::release_parts(canceled, hand_changed);
        match &*stack.item {
            "minecraft:bow" if release => self.release_bow(stack, self.common.ticks_held(stack, ticks)),
            "minecraft:trident" if release => self.release_trident(self.common.ticks_held(stack, ticks)),
            "minecraft:spyglass" if spyglass => self.stop_spyglass(),
            _ => {}
        }
        if release {
            self.common.mod_release_using(stack, ticks);
        }
        self.stop_using_item(stack);
    }
    /// Ends the current use through [`Self::stop_using_item`]. With no stack left in
    /// the use's slot there is nothing to vibrate, but the use still ends.
    pub(in crate::game) fn stop_active_item_use(&mut self) {
        match self.common.use_start.item.clone().or_else(|| {
            self.common.using
                .and_then(|(slot, _)| self.common.stack_in(slot))
                .cloned()
        }) {
            Some(stack) => self.stop_using_item(&stack),
            None => {
                self.common.using = None;
                self.common.use_start = Default::default();
            }
        }
    }

    /// A command or a mod puts `stack` in inventory `slot` (`/item replace`, a
    /// mod's slot write). When that slot's item is in use and `stack` isn't the
    /// same item, `useItem` keeps the old stack, so the use stops with its
    /// vibration, even for air: an emptied hand alone reads as used up.
    /// The stop is for the stack the use began with, whatever the slot holds now.
    pub(in crate::game::server_side) fn stop_use_replaced(
        &mut self,
        slot: usize,
        stack: Option<&ItemStack>,
    ) {
        if !self.common.using.is_some_and(|(used, _)| used == slot) {
            return;
        }
        let new = stack.map(|s| &s.item);
        if let Some(item) = self.common.use_start.item.take_if(|item| Some(&item.item) != new) {
            self.stop_using_item(&item);
        } else if self.common.use_start.item.is_none()
            && self.common.stack_in(slot).is_some_and(|s| Some(&s.item) != new)
        {
            self.stop_active_item_use();
        }
    }

    /// `LivingEntity.stopUsingItem`: the use of `stack` is over (`useItem` and
    /// its remaining ticks are cleared), and when the use effects of the stack
    /// ask for it, `ItemStack.causeUseVibration` sends an `ITEM_INTERACT_FINISH`
    /// game event from the player, at its feet.
    pub(in crate::game) fn stop_using_item(&mut self, stack: &ItemStack) {
        self.common.using = None;
        self.common.use_start = Default::default();
        self.use_vibration(stack, "minecraft:item_interact_finish");
    }
    /// `ItemStack.causeUseVibration` for `stack` used by the player: the
    /// `event` game event (`ITEM_INTERACT_START` when the use begins,
    /// `ITEM_INTERACT_FINISH` when it ends) from the player, at its feet, when
    /// the stack's use effects ask for interaction vibrations (the default).
    pub(in crate::game) fn use_vibration(&mut self, stack: &ItemStack, event: &'static str) {
        if !self.common.is_logical_server() || !self.use_vibrates(stack) {
            return;
        }
        let Some(p) = self.common.player.as_ref().map(|p| p.position) else {
            return;
        };
        let ctx = self.player_context(None);
        self.post_game_event(event, [p.x, p.y, p.z], ctx);
    }
}

impl crate::game::ServerGame<'_> {
    /// Whether the use effects of `stack` ask for interaction vibrations.
    pub(in crate::game::server_side) fn use_vibrates(&self, stack: &ItemStack) -> bool {
        item_use::interact_vibrations(
            stack,
            self.items().get(&stack.item).map(|i| i.interact_vibrations),
        )
    }
}

use sunrise_gameplay_common::game::tables::GameItems as _;
