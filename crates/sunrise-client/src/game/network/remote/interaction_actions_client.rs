#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::interaction_actions::*;

impl Game<'_> {
    /// `Hud.onDisconnected`, from `Minecraft.disconnect` and `clearClientLevel`
    /// (a proxy's reconfiguration too), but the chat: joining clears it and a
    /// reconfiguration keeps it (`ChatComponent.storeState`).
    pub(in crate::game::client_side) fn hud_disconnected(&mut self) {
        self.tab_list.0.reset();
        self.client.boss_overlay.reset();
        self.client.toast_manager = Default::default();
        self.client.debug_charts.reset();
        self.client.titles.clear();
        self.client.titles.reset_times();
    }
    /// `handleAnimate`: a player wakes up, or an entity gets the critical
    /// hit (`CRIT`) or magic critical hit (`ENCHANTED_HIT`) emitter.
    pub(in crate::game::client_side) fn remote_animate(&mut self, id: i32, action: u8) {
        use sunrise_client::particles::{ParticleOptions, ParticleType};
        let particle = match action {
            // Player.stopSleepInBed.
            0 => {
                if self.player.is_some()
                    && self
                        .remote
                        .as_ref()
                        .is_some_and(|remote| remote.entity_id == id)
                {
                    self.sleeping = None;
                    self.sleep_timer = 100;
                }
                return;
            }
            1 => ParticleType::Crit,
            2 => ParticleType::EnchantedHit,
            _ => return,
        };
        let Some((target, body)) = self.remote_tracking_body(id) else {
            return;
        };
        self.particles
            .0
            .tracking_emitter(ParticleOptions::simple(particle), target, body);
    }
    pub(in crate::game) fn remote_continue_attack_inner(&mut self, down: bool) {
        if !down {
            self.clicks.miss_time = 0;
        }
        // A piercing weapon neither breaks blocks nor swings on its own.
        if self.clicks.miss_time > 0
            || self.using.is_some()
            || self.client_item_use.0.is_some()
            || self
                .held()
                .is_some_and(|s| sunrise_client::spear::Kinetic::of(&s.item).is_some())
        {
            return;
        }
        let entity = self.entity_targeted();
        if down
            && !entity
            && let Some(hit) = self.pick()
        {
            let state = self.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
            let air = self.res.facts().is_air(state);
            // InputEvent.InteractionKeyMappingTriggered, before the dig goes on:
            // a cancel digs nothing and swings unless the mod said not to.
            let canceled = if air {
                None
            } else {
                self.native_interaction(
                    InteractionKey::Attack,
                    Hand::Main,
                    mod_api::InteractionAttempt::Held,
                )
                .canceled_swing()
            };
            if canceled.is_none() && !air {
                // MultiPlayerGameMode.continueDestroyBlock starts with ensureHasSentCarriedItem.
                self.remote_send_carried();
            }
            if let Some(swing) = canceled {
                if swing {
                    self.remote_swing_packet();
                }
            } else if self.remote_break_restricted(hit.pos) {
                // continueDestroyBlock returned false: no swing.
            } else if !air {
                if self.remote_mining(false) {
                    self.remote_swing_packet();
                }
            } else {
                if let Some(r) = self.remote.as_mut() {
                    r.destroying = None;
                    r.is_destroying = false;
                }
                self.mining = None;
            }
        } else {
            // `stopDestroyBlock` returns at once for a break that is not running: a pick that
            // missed once a break stopped ends nothing, so the dig's accumulated progress stays
            // and goes on growing on the jumps that follow (#4124).
            if self.stop_remote_mining() {
                self.mining = None;
            }
        }
    }
    /// The server's death message; chat uses the shared ChatComponent.
    pub(in crate::game::client_side) fn remote_hud(&self, batch: &mut HudBatch) {
        let Some(r) = &self.remote else { return };
        let atlas = &self.gui.atlas;
        let w = batch.width;
        if self.client.screen == Screen::Dead
            && let Some(message) = &r.death_message
        {
            // DeathScreen draws the styled cause component.
            let tw = atlas.font.width_text(message);
            batch.text_styled(
                atlas,
                message,
                ((w - tw) / 2.0).floor(),
                85.0,
                u32::MAX,
                true,
            );
        }
    }
    pub(in crate::game) fn remote_start_attack_inner(&mut self) -> bool {
        if self.clicks.miss_time > 0 || self.hands_busy() {
            return false;
        }
        if self.spectator() {
            // Minecraft.startAttack: a spectator's click on an entity asks
            // to watch through it (MultiPlayerGameMode.spectate); a click on
            // nothing still tells the server (spectatorNoAction).
            let target = self.spectator_click_target();
            self.send(S::SpectatorAction(play::SpectatorAction { target }));
            return true;
        }
        let spear = self
            .held()
            .and_then(|s| sunrise_client::spear::Kinetic::of(&s.item));
        if spear.is_some() && self.cannot_attack_with_item(self.client.hand_strength.ticker, 0) {
            return false;
        }
        // InputEvent.InteractionKeyMappingTriggered, after the checks that stop
        // an attack: a cancel ends it there, swinging unless the mod said not to.
        if let Some(swing) = self
            .native_interaction(
                InteractionKey::Attack,
                Hand::Main,
                mod_api::InteractionAttempt::Press,
            )
            .canceled_swing()
        {
            if swing {
                let animation = spear.map_or(SwingAnimation::DEFAULT, |k| k.attack_animation());
                self.swing_hand(Hand::Main, animation);
            }
            return false;
        }
        // A spear jabs (`MultiPlayerGameMode.piercingAttack`): this client
        // reports the reach and combo of a mob in range, then the server stabs
        // along the look it last heard (the jab sends none of its own) while
        // this client swings, plays the sound and starts its charge again.
        if let Some(kinetic) = spear {
            if let Some((i, distance, _)) = self.mob_hit()
                && self.before_block(distance)
                && self.held_range_allows(distance)
            {
                self.pvp_attacked(self.mobs[i].id as i32, distance);
            }
            // ServerboundPlayerActionPacket.Action.STAB.
            self.send(S::PlayerAction(play::PlayerAction {
                action: 8,
                pos: [0; 3],
                direction: 0,
                sequence: 0,
            }));
            self.client.hand_strength.reset_attack();
            self.swing_hand(Hand::Main, kinetic.attack_animation());
            self.play_at_feet(kinetic.attack_sound);
            return true;
        }
        if self.mod_attack_targets() {
            self.remote_swing_packet();
            return false;
        }
        let mut instant = false;
        if let Some(hit) = self.crosshair_entity_hit() {
            // A range rejection is still an entity hit: skip the attack, but swing below.
            if let Some(distance) = self.attack_hit_distance(hit.point) {
                self.remote_attack_entity(hit.id, distance);
            }
        } else if let Some(hit) = self.pick() {
            // A block hit on air falls through to MISS. `instant` below reads the
            // block again after the dig, which may have broken it.
            if mod_api::__private::listened(mod_api::HookId::LeftClickEmpty)
                && self
                    .res
                    .facts()
                    .is_air(self.world.block(hit.pos[0], hit.pos[1], hit.pos[2]))
            {
                self.native_left_click_empty();
            }
            if !self.remote_mining(true) {
                return false;
            }
            instant = self
                .res
                .facts()
                .is_air(self.world.block(hit.pos[0], hit.pos[1], hit.pos[2]));
        } else {
            if mod_api::__private::listened(mod_api::HookId::LeftClickEmpty) {
                self.native_left_click_empty();
            }
            if self.has_miss_time() {
                self.clicks.miss_time = 10;
            }
            self.reset_attack_strength();
        }
        self.remote_swing_packet();
        instant
    }
    /// Minecraft.startUseItem: use one crosshair target, then the item itself.
    pub(in crate::game) fn remote_use_inner(&mut self, attempt: mod_api::InteractionAttempt) {
        // Another queued press in the same input drain cannot restart a capture.
        if self.client_item_use.0.is_some() {
            return;
        }
        // Minecraft.startUseItem returns before resetting rightClickDelay
        // while a survival block break is active.
        if self.remote.as_ref().is_some_and(|r| r.is_destroying) {
            return;
        }
        self.use_delay = 4;
        // Minecraft.startUseItem: nothing is used while the hands paddle a boat.
        if self.hands_busy() {
            return;
        }
        // InputEvent.InteractionKeyMappingTriggered for the main hand: a cancel
        // ends the whole click, the off hand gets no turn.
        if let Some(swing) = self
            .native_interaction(InteractionKey::Use, Hand::Main, attempt)
            .canceled_swing()
        {
            if swing {
                self.swing_hand(Hand::Main, SwingAnimation::DEFAULT);
            }
            return;
        }
        // A registered client item behavior consumes the click before target handling.
        if self.start_client_item_use() {
            return;
        }
        // Resolve the crosshair once. A passing entity interaction keeps
        // that target and may only fall back to using the item itself.
        let entity = self.remote_use_targets_entity();
        sunrise_client::interaction_input::use_on_target(
            &mut RemoteUseWorld(self, attempt),
            entity,
        );
    }
}
