#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    /// The level path: `ServerLevel.tick` calls `ServerPlayer.tick` before the
    /// server starts its connection ticks.
    pub(in crate::game) fn tick_level_guests(&mut self) {
        building_cache(self.lan.as_ref());
        self.tick_level_guests_inner();
        building_cache(None);
    }

    pub(in crate::game) fn advance_guest_entity_clock(&mut self, conn: u32) {
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        let advanced = host.hub.update_guest(conn, |guest| {
            guest
                .motion
                .as_mut()
                .map(|motion| motion.health.advance_entity_clock())
        });
        if advanced == Some(None) {
            // Joining players have not borrowed their saved body yet.
            self.as_guest(conn, |game| {
                if let Some(player) = game.player.as_mut() {
                    player.health.advance_entity_clock();
                }
            });
        }
    }

    pub(in crate::game) fn finish_guest_connection_level(&mut self) {
        self.flush_guest_connection_effects();
        self.flush_attachment_sync();
        self.flush_native_sounds();
    }

    pub(in crate::game) fn tick_guest_connection(&mut self, conn: u32) {
        self.tick_guest_connection_body(conn);
        if !self.on_server_trip(conn) {
            // Like the rest of doTick outside Player.tick, LOCATION also runs
            // for unloaded spectators and canceled entity ticks.
            let tick_count = self
                .lan
                .as_ref()
                .and_then(|host| host.hub.guest(conn).filter(|g| host.hub.here(g)))
                .map(|guest| guest.motion.as_ref().map(|m| m.health.tick_count));
            match tick_count {
                Some(Some(tick_count)) if tick_count % 20 == 0 => {
                    let _ = self.as_guest(conn, |game| game.trigger_location_advancement());
                }
                // Joining players have not borrowed their saved body yet: read
                // its clock through the swap, as `advance_guest_entity_clock` does.
                Some(None) => {
                    let _ = self.as_guest(conn, |game| {
                        if game
                            .player
                            .as_ref()
                            .is_some_and(|player| player.health.tick_count % 20 == 0)
                        {
                            game.trigger_location_advancement();
                        }
                    });
                }
                _ => {}
            }
            self.guest_vehicle_movement_baseline(conn);
        }
        if self
            .lan
            .as_ref()
            .is_some_and(|host| !host.hub.native_advancement_grants.is_empty())
        {
            self.run_native_advancement_grants();
        }
    }

    #[cfg(test)]
    pub(in crate::game) fn tick_guests(&mut self) {
        building_cache(self.lan.as_ref());
        self.tick_guests_inner();
        building_cache(None);
    }

    /// Whether guest `g`'s body ticks this tick: `ServerPlayer.doTick`'s
    /// `!isSpectator() || !touchingUnloadedChunk()`. A spectator whose box,
    /// inflated by one block, reaches a chunk the level hasn't loaded skips
    /// `super.tick()` (Player.tick: its effects, death ticks, cooldowns, attack
    /// strength, inventory ticks and PlayerTickEvent.Pre/Post) and doTick's food tick
    /// and statistics; everything else the server does for it still runs.
    pub(in crate::game) fn guest_body_ticks(&self, g: &sunrise_client::lan::guest::Guest) -> bool {
        !g.spectator()
            || !touching_unloaded_chunk(g.position, g.dimensions(), |x, z| {
                self.world.chunks.contains_key(&(x, z))
            })
    }

    /// Only listeners pay for a guest swap; reuse the loop's resolved index.
    pub(in crate::game) fn guest_player_tick(
        &mut self,
        gi: usize,
        conn: u32,
        hook: mod_api::HookId,
        fire: fn(&mut Self, mod_api::PlayerRef),
    ) {
        if !mod_api::__private::listened(hook) {
            return;
        }
        let (Some(epoch), Some(host)) = (self.native_lifecycle.epoch(), self.lan.as_ref()) else {
            return;
        };
        let guest = &host.hub.guests[gi];
        let player = mod_api::__private::game::player_ref(
            mod_api::EntityRef::new(epoch, guest.entity_id as u32),
            guest.uuid,
        );
        let _ = self.as_guest_at(gi, conn, |game| fire(game, player));
    }

    /// `LivingEntity.checkAutoSpinAttack` for a server's copy of a Riptide
    /// spinner (it runs before `aiStep`'s countdown clears the damage): the
    /// first mob touching the player's box takes `Player.attack` with the
    /// spin's damage and the spin ends. A `ServerPlayer` is client
    /// authoritative, so its `aiStep` does not travel and both boxes vanilla
    /// passes are the current one: no sweep over the tick's moves.
    pub(in crate::game) fn server_spin_attack(&mut self) {
        let Some(p) = self.player.as_ref() else {
            return;
        };
        if p.auto_spin_attack_ticks <= 0 {
            return;
        }
        let swept = p.bounds();
        let riding = self.riding;
        let Some(index) = first_spin_target(
            swept,
            riding,
            self.mobs.iter().map(|m| (m.id, m.removed, m.bounds())),
        ) else {
            return;
        };
        self.attack_mob(index);
        if let Some(p) = self.player.as_mut() {
            p.auto_spin_attack_ticks = 0;
            p.auto_spin_attack_dmg = 0.0;
            p.velocity = p.velocity.scale(-0.2);
        }
    }
}

#[cfg(test)]
#[path = "tests/building_cache.rs"]
pub(in crate::game) mod building_cache_tests;

#[cfg(test)]
#[path = "tests/dig_tick_order_tests.rs"]
pub(in crate::game) mod dig_tick_order_tests;

#[cfg(test)]
#[path = "tests/fall_sound_tests.rs"]
pub(in crate::game) mod fall_sound_tests;

#[cfg(test)]
#[path = "tests/spectator_body_tick_tests.rs"]
pub(in crate::game) mod spectator_body_tick_tests;

#[cfg(test)]
#[path = "tests/equipment_tick_tests.rs"]
pub(in crate::game) mod equipment_tick_tests;
