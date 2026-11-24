#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    /// One client tick while connected (Minecraft.tick with a ClientLevel).
    pub(in crate::game) fn tick_remote(&mut self) -> Result<()> {
        self.tick_remote_level()?;
        // The rockets boosting this player tick after it, also while it waits
        // for chunks.
        self.tick_own_rockets();
        self.tick_book_snapshot_retry();
        // Far tiles the samplers asked for (`lod_net`).
        if let Some(r) = &self.remote {
            r.session.send_far_terrain();
        }
        // Minecraft.tick sends this even when the player cannot tick yet,
        // including while waiting for chunks. Keep it outside early returns.
        if self.remote.as_ref().is_some_and(Remote::has_level) {
            self.send(S::ClientTickEnd(Empty));
        }
        Ok(())
    }

    /// `Entity.commonTick`'s tickCount for an entity of the client level at its own visit: a
    /// root once its `EntityTickEvent.Pre` let it through (`tickNonPassenger`), a passenger at
    /// its ride visit whatever that Pre answered (`tickPassenger`).
    pub(in crate::game) fn count_client_tick(&mut self, id: u32) {
        if let Some(t) = self
            .remote
            .as_mut()
            .and_then(|r| r.entities.get_mut(&(id as i32)))
        {
            t.tick_count = t.tick_count.wrapping_add(1);
        }
    }
    /// Whether entity `id` of the client level rides another (`Entity.vehicle`): its tick is a
    /// ride visit, counted whatever its Pre answers.
    pub(in crate::game) fn client_passenger(&self, id: u32) -> bool {
        self.remote
            .as_ref()
            .and_then(|r| r.entities.get(&(id as i32)))
            .is_some_and(|t| t.vehicle.is_some())
    }
    /// The visit of a told entity whose Pre answered `ticks`: a root counts only when it ticks,
    /// a passenger always.
    pub(in crate::game) fn count_told_client_tick(&mut self, id: u32, ticks: bool) {
        if ticks || self.client_passenger(id) {
            self.count_client_tick(id);
        }
    }

    /// `ClientClockManager.tick`: the clock moves on by the game time that
    /// passed since its last tick, at its rate. A game time the server sets
    /// forward (its sync every second) moves the clock with it.
    pub(in crate::game) fn remote_clock_tick(&mut self) {
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        let delta = self.world.game_time - r.clock_game_time;
        r.clock_game_time = self.world.game_time;
        for clock in r.clocks.values_mut() {
            let _ = client_level_tick::advance_clock(clock, delta);
        }
        self.world.day_time = r.clock_time(r.default_clock);
    }
}

impl Game<'_> {
    /// `Level.tickBlockEntities` on a client: the block entities' animations
    /// and client ticks, and the moving pistons' pushes.
    pub(in crate::game) fn tick_remote_block_entities(&mut self) {
        self.tick_client_block_entities();
        self.with_level(|b, level, ticks| {
            b.client_tick_block_entities(level, ticks);
        });
        let pushes = std::mem::take(&mut self.block_ticks.piston_pushes);
        self.push_piston_entities(pushes);
        self.sync_moving_pistons();
    }

    /// Humanoid held arm poses, once the tick has set equipment and use state.
    pub(in crate::game) fn tick_remote_arm_poses(&mut self) {
        for i in 0..self.mobs.len() {
            let m = &self.mobs[i];
            if !sunrise_client::mobs::anim::extracts_humanoid(m) {
                continue;
            }
            // Extraction also runs while the world is frozen: derive the use
            // animation from the current equipment rather than last tick's state.
            if &*m.kind.id != "minecraft:player" {
                let slot = if m.extra.using_off_hand {
                    sunrise_items::equipment::OFFHAND
                } else {
                    sunrise_items::equipment::MAINHAND
                };
                let animation = (m.extra.using_item > 0)
                    .then(|| m.extra.slot_stack(slot))
                    .flatten()
                    .map_or_else(Default::default, |stack| self.item_use_animation(stack));
                self.mobs[i].extra.use_animation = animation;
            }
            let m = &self.mobs[i];
            let poses = self.held_arm_poses(m.id, m);
            let charge_duration = (&*m.kind.id != "minecraft:player").then(|| {
                let using_slot = if m.extra.using_off_hand {
                    sunrise_items::equipment::OFFHAND
                } else {
                    sunrise_items::equipment::MAINHAND
                };
                m.extra
                    .slot_stack(using_slot)
                    .map_or(25, |stack| Self::charge_ticks(stack).max(1)) as i32
            });
            self.mobs[i].extra.held_arm_pose = poses;
            if let Some(duration) = charge_duration {
                self.mobs[i].extra.ints[5] = duration;
            }
        }
    }
}
