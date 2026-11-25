#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::interaction::*;

impl Game<'_> {
    /// `ServerGamePacketListenerImpl.handleInteract`'s entity use for a guest or the persistent fake: the border and
    /// interaction range, then that entity and hand only, never an air-use fallback.
    pub(in crate::game) fn guest_interact(
        &mut self,
        conn: u32,
        gi: usize,
        id: i32,
        hand: Hand,
        location: [f64; 3],
    ) -> Result<bool, ActionRefused> {
        if let Some(target) = self.guest_target_box(gi, id) {
            if !self.inside_border(&target) {
                return Err(ActionRefused::Protected);
            }
            if self.as_guest_at(gi, conn, |g| {
                g.within_entity_interaction_range(&target, 3.0)
            }) != Ok(true)
            {
                return Err(ActionRefused::OutOfReach);
            }
        } else {
            return Err(ActionRefused::InvalidTarget);
        }
        // LeashFenceKnotEntity.interact: a knot has no entity here, only its fence.
        if let Some(knot) = self.knot_at_id(id) {
            let used = self
                .as_guest_at(gi, conn, |g| {
                    let swings = g.swings_started;
                    let consumed = g.with_hand(hand, |g| g.use_knot(knot, true));
                    reset_use_attack_strength(&mut g.server.attack_ticks, swings, g.swings_started);
                    (g.swings_started != swings, consumed)
                })
                .ok();
            self.lan_swing_back(conn, used.map(|u| u.0));
            return Ok(used.is_some_and(|u| u.1));
        }
        let host = self.lan.as_ref().ok_or(ActionRefused::Stale)?;
        let before = host.hub.guests[gi].vehicle;
        // Packet interactions aim the server's ray at the clicked entity. A fake already
        // supplies its own viewpoint; its explicit target must not change that rotation.
        if !host.hub.guests[gi].is_fake() && self.look_at_entity(gi, id, Some(location)).is_none() {
            return Ok(false);
        }
        let target = InteractTarget { id, location };
        let used = self
            .as_guest_at(gi, conn, |g| {
                let saved = g.server.guest_use;
                let failed = std::mem::replace(&mut g.server.use_failed, false);
                g.server.guest_use = Some(GuestUse::Entity(hand, target));
                let swings = g.swings_started;
                let consumed = matches!(g.use_item(), Ok(Some(_))) && !g.server.use_failed;
                g.server.guest_use = saved;
                g.server.use_failed = failed;
                reset_use_attack_strength(&mut g.server.attack_ticks, swings, g.swings_started);
                (g.swings_started != swings, consumed)
            })
            .ok();
        self.lan_swing_back(conn, used.map(|u| u.0));
        self.guest_boarded(conn, before);
        Ok(used.is_some_and(|u| u.1))
    }
    /// `ServerGamePacketListenerImpl.handlePlayerAction` for STAB: the spear in
    /// the guest's main hand jabs once its charge allows it (with five ticks
    /// of tolerance). The guest's client is sent the dash Lunge gave them.
    pub(in crate::game::server_side) fn guest_jab(&mut self, conn: u32) {
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        let Some(g) = host.hub.guest(conn) else {
            return;
        };
        let at = g.position;
        let players = self.stab_players_for(conn);
        let sound = self
            .guest_hit_by(conn, |g| {
                if g.cannot_attack_with_item(g.server.attack_ticks as i32, 5) {
                    None
                } else {
                    let outer = std::mem::replace(&mut g.server.stab_players, players);
                    let sound = g.jab();
                    g.server.stab_players = outer;
                    sound
                }
            })
            .flatten();
        self.land_player_stabs();
        // PiercingWeapon.makeSound: `playSound(causer, ...)`, so the guest's
        // own client plays it.
        if let Some(name) = sound {
            self.send_level_sound(LevelSound::new(name, at, 1.0, 1.0), Some(conn));
        }
    }
    /// `ServerGamePacketListenerImpl.handleUseItem` for guest `conn` (at `gi`) using the item in
    /// `hand`, after the packet's sequence is acknowledged: a non-empty, enabled stack permits
    /// the packet rotation (`absSnapRotationTo`), then `ServerPlayerGameMode.useItem`.
    /// True when the use consumed (`InteractionResult` a success or a consume). A fake player's
    /// `PlayerRef::perform_action` runs it as the packet does, with no rotation.
    pub(in crate::game::server_side) fn guest_use_item(
        &mut self,
        conn: u32,
        gi: usize,
        hand: Hand,
        rotation: Option<(f32, f32)>,
    ) -> Result<bool, ActionRefused> {
        let Some(host) = self.lan.as_ref() else {
            return Err(ActionRefused::Stale);
        };
        let g = &host.hub.guests[gi];
        let held = if hand == Hand::Off {
            g.offhand.as_ref()
        } else {
            g.inventory.get(g.selected).and_then(Option::as_ref)
        };
        let Some(held) = held.filter(|s| s.count != 0 && s.item.as_ref() != "minecraft:air") else {
            return Ok(false);
        };
        if !self.creative_item_enabled(held, self.enabled_feature_ids()) {
            return Ok(false);
        }
        let (spectator, spawn_egg) = (g.spectator(), held.item.ends_with("_spawn_egg"));
        if let Some((yaw, pitch)) = rotation {
            use sunrise_client::lan::hub::wrap_degrees;
            let (yaw, pitch) = (wrap_degrees(yaw), wrap_degrees(pitch));
            if yaw != g.yaw || pitch != g.pitch {
                self.lan.as_mut().unwrap().hub.guests.update(gi, |g| {
                    // Entity.setYRot/setXRot discard non-finite values independently.
                    if yaw.is_finite() {
                        g.yaw = yaw;
                    }
                    if pitch.is_finite() {
                        g.pitch = pitch.clamp(-90.0, 90.0);
                    }
                });
            }
        }
        // ServerPlayerGameMode.useItem returns PASS for spectators, after the turn.
        if spectator {
            return Ok(false);
        }
        // SpawnEggItem.use calls ServerLevel.mayInteract at its own fluid ray's position.
        if spawn_egg {
            let hit = self
                .as_guest_at(gi, conn, |g| g.with_hand(hand, |g| g.spawn_egg_fluid_hit()))
                .ok()
                .flatten();
            if hit.is_some_and(|h| self.under_spawn_protection(gi, h.pos)) {
                return Err(ActionRefused::Protected);
            }
        }
        let heard = self.audio_capture_len();
        let used = self
            .as_guest_at(gi, conn, |g| {
                // A horn's sound, inline instruments included.
                let horn = g.with_hand(hand, |g| {
                    let held = g.held()?;
                    g.res.items().instrument(held).map(|i| i.sound)
                });
                let saved = g.server.guest_use;
                g.server.guest_use = Some(GuestUse::Item(hand));
                let swings = g.swings_started;
                let consumed = matches!(g.use_item(), Ok(Some(_)));
                g.server.guest_use = saved;
                reset_use_attack_strength(&mut g.server.attack_ticks, swings, g.swings_started);
                (g.swings_started != swings, horn, consumed)
            })
            .ok();
        let (swung, horn, consumed) = match used {
            Some((swung, horn, consumed)) => (Some(swung), horn, consumed),
            None => (None, None, false),
        };
        self.lan_swing_back(conn, swung);
        self.guest_instrument_sound(conn, heard, horn);
        Ok(consumed)
    }
    /// `ServerGamePacketListenerImpl.handleUseItemOn` for guest `conn` (at `gi`) clicking
    /// `hit` with `hand`, after the packet's sequence is acknowledged: reach, the hit's place in
    /// its block, the build height, spawn protection and the border, then
    /// `ServerPlayerGameMode.useItemOn`, and the blocks the client predicted sent back. True
    /// when the use ends the click. An optional receipt reports block and item phases before
    /// their block updates. The packet handler answers refusals. A fake player's
    /// `PlayerRef::perform_action` runs it as the packet does.
    pub(in crate::game) fn guest_use_item_on(
        &mut self,
        conn: u32,
        gi: usize,
        hit: Hit,
        hand: Hand,
        receipt: Option<i64>,
    ) -> Result<bool, ActionRefused> {
        use super::super::cauldron_prediction::{UseOnOutcome, use_result};

        let reply = |game: &Self, outcome| {
            if let (Some(receipt), Some(host)) = (receipt, &game.lan) {
                host.hub.send_to(conn, &use_result(receipt, outcome));
            }
        };
        let (mut consumed, mut block_fell_through) = (false, false);
        let Some(host) = self.lan.as_mut() else {
            return Err(ActionRefused::Stale);
        };
        // handleUseItemOn: within reach, then the hit point within
        // a block of the block's centre (logged when it isn't).
        if !host.hub.guests.update(gi, |g| g.may_reach_block(hit.pos)) {
            return Err(ActionRefused::OutOfReach);
        }
        let inside = (0..3).all(|k| (hit.point[k] - f64::from(hit.pos[k]) - 0.5).abs() < 1.0000001);
        if !inside {
            let d = sunrise_core::java_number::double_to_string;
            let at = |k: usize| d(hit.point[k]);
            let line = format!(
                "Rejecting UseItemOnPacket from {}: Location ({}, {}, {}) too far away from hit block BlockPos{{x={}, y={}, z={}}}.",
                host.hub.guests[gi].name,
                at(0),
                at(1),
                at(2),
                hit.pos[0],
                hit.pos[1],
                hit.pos[2]
            );
            self.lan_log_event(sunrise_client::lan::hub::HostEvent::Warn(line));
            return Err(ActionRefused::OutOfReach);
        }
        let spectator = self
            .lan
            .as_ref()
            .is_some_and(|host| host.hub.guests[gi].spectator());
        let face = hit.face;
        // handleUseItemOn: above or below the world nothing is used and
        // the player is told the limit (the blocks are not resent);
        // under spawn protection nothing is used or placed; the player
        // is told and the blocks below go back. mayInteract: nothing is
        // used on a block outside the world border either.
        let min_y = self.world.shape.min_y;
        let max_y = self.world.shape.max_y() - 1;
        if hit.pos[1] > max_y {
            self.build_limit_message(conn, true, max_y);
            return Err(ActionRefused::Protected);
        }
        if hit.pos[1] < min_y {
            self.build_limit_message(conn, false, min_y);
            return Err(ActionRefused::Protected);
        }
        let mut refused = None;
        if self.under_spawn_protection(gi, hit.pos) {
            self.spawn_protection_message(conn, hit.pos);
            refused = Some(ActionRefused::Protected);
        } else if self
            .border
            .contains(f64::from(hit.pos[0]), f64::from(hit.pos[2]))
        {
            if spectator && self.spectator_use_portal(conn, gi, hit.pos) {
                reply(self, UseOnOutcome::default());
                return Ok(true);
            }
            let logged = self.world.block_log.as_ref().map_or(0, Vec::len);
            let heard = self.audio_capture_len();
            let used = self.as_guest_at(gi, conn, |g| {
                let attempt = g.was_block_placement_attempt(hand);
                let saved = (g.server.forced_hit, g.server.guest_use);
                g.server.forced_hit = Some(hit);
                g.server.guest_use = Some(GuestUse::on_block(hand));
                let swings = g.swings_started;
                let predicted = g.server.predicted_swings;
                let result = g.with_block_use_original(hand, |g| {
                    let result = g.use_item();
                    if matches!(result, Ok(Some(_))) && !g.server.use_failed {
                        g.finish_block_use(hit.pos, hand);
                    }
                    result
                });
                let fell_through = matches!(
                    g.server.guest_use,
                    Some(GuestUse::OnBlock {
                        block_fell_through: true,
                        ..
                    })
                );
                (g.server.forced_hit, g.server.guest_use) = saved;
                reset_use_attack_strength(&mut g.server.attack_ticks, swings, g.swings_started);
                (
                    g.swings_started != swings && g.server.predicted_swings == predicted,
                    result.map(|used| used.is_some()),
                    attempt,
                    fell_through,
                )
            });
            let (swung, result, attempt, fell_through) = used.map_err(|_| ActionRefused::Stale)?;
            block_fell_through = fell_through;
            self.lan_swing_back(conn, Some(swung));
            self.guest_use_sounds(conn, logged, heard);
            // A placement that did nothing at the top or bottom of the
            // world says why. Vanilla asks twice for the top (once
            // before the swing, once in the check that also has the
            // bottom), so a player there is told twice.
            consumed = match result {
                Ok(consumed) => consumed,
                Err(error) => {
                    log::warn!("block interaction for guest {conn} refused: {error:#}");
                    refused = Some(ActionRefused::InvalidTarget);
                    false
                }
            };
            if !consumed && attempt && refused.is_none() {
                if face == [0, 1, 0] && hit.pos[1] >= max_y {
                    self.build_limit_message(conn, true, max_y);
                    self.build_limit_message(conn, true, max_y);
                } else if face == [0, -1, 0] && hit.pos[1] <= min_y {
                    self.build_limit_message(conn, false, min_y);
                }
            }
        } else {
            self.build_limit_message(conn, true, max_y);
            refused = Some(ActionRefused::Protected);
        }
        // Let a waiting client run the item prediction while it still sees the
        // pre-use blocks, under the same sequence that will settle the changes.
        if refused.is_none() {
            reply(
                self,
                UseOnOutcome {
                    block_fell_through,
                    passed: !spectator && !consumed,
                },
            );
        }
        // The client predicted a result: correct it with what really is.
        let Some(host) = self.lan.as_mut() else {
            return refused.map_or(Ok(consumed), Err);
        };
        for pos in [hit.pos, hit.beside()] {
            host.hub.send_authoritative_block(conn, &self.world, pos);
        }
        refused.map_or(Ok(consumed), Err)
    }
    /// The fence of the knot whose entity id is `id` (`lan_assign_ids`).
    pub(in crate::game) fn knot_at_id(&self, id: i32) -> Option<[i32; 3]> {
        self.server
            .knot_ids
            .iter()
            .find(|&(_, &knot)| knot as i32 == id)
            .map(|(&fence, _)| fence)
    }
}
