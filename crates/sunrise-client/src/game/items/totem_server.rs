use super::Game;
use super::render_tick::player_tracking_body;
use crate::game::client_side::totem::Body;
use crate::game::client_side::totem::TOTEM;
use crate::game::client_side::totem::TOTEM_EVENT;
use sunrise_items::items::ItemStack;
use sunrise_sim::{health::HealthEvent, motion::MotionEvent};

impl Game<'_, crate::game::server_side::ServerState> {
    /// The `death_effects` of the stack's `minecraft:death_protection`.
    pub(in crate::game) fn death_effects(
        &self,
        s: &ItemStack,
    ) -> Vec<sunrise_items::items::ConsumeEffect> {
        s.death_protection(self.items().get(&s.item))
            .unwrap_or_default()
            .to_vec()
    }
    /// Whether either hand holds a death-protection item, for the next hurt:
    /// the window's player once a tick, a server's player for each of its
    /// turns.
    pub(in crate::game) fn update_totem_ready(&mut self) {
        let ready = self.held().is_some_and(|s| self.has_death_protection(s))
            || self
                .offhand
                .as_ref()
                .is_some_and(|s| self.has_death_protection(s));
        let off = !self.held().is_some_and(|s| self.has_death_protection(s));
        if let Some(p) = self.player.as_mut() {
            p.health.totem_ready = ready;
            p.health.totem_off_hand = off;
        }
    }
    /// Finish a hit before its caller can hurt the acting player again. Protection and
    /// `ServerPlayer.die` both run on the current body, not its between-turn projection.
    pub(in crate::game) fn finish_guest_damage(&mut self) {
        let _ = self.finish_guest_damage_before_resume();
    }
    /// The server's half once a totem fired: use it up (main hand first),
    /// count it, set health to 1, then clear and apply its effects. Returns
    /// the player's body with the protector copy the activation animation shows.
    /// A server guest's entity event is broadcast before the hit's Post.
    pub(in crate::game::server_side) fn apply_totem(&mut self, entity: i32) -> Option<Body> {
        // The mods hear a server player's totem before it is used up.
        if let (Some(uuid), Some(name)) =
            (self.side.lan_acting_uuid, self.side.lan_acting_name.clone())
        {
            self.native_player_totem(uuid, &name);
        }
        let p = self.player.as_mut().filter(|p| p.entity_id == entity)?;
        let _ = p.set_health(1.0, false);
        let body = Body {
            bounds: player_tracking_body(p),
            protector: None,
        };
        // `checkTotemDeathProtection`: the selected slot first, then the
        // offhand. The copy of that stack is the protector (`protectionItem`)
        // the use is counted and the activation animation shown with, taken
        // before the use takes one off the stack.
        let main = self.held().is_some_and(|s| self.has_death_protection(s));
        let cell = if main {
            &mut self.inventory[self.selected]
        } else {
            &mut self.offhand
        };
        let protector = cell
            .as_ref()
            .cloned()
            .unwrap_or_else(|| ItemStack::new(TOTEM, 1));
        let slot = if main {
            self.selected
        } else {
            super::shield::OFFHAND_SLOT
        };
        self.consume_player_slot(slot, 1);
        self.award_stat("used", &protector.item, 1);
        self.trigger_item("minecraft:used_totem", &protector);
        // ItemStack.causeUseVibration: the totem's use effects make one.
        let ctx = self.player_context(None);
        self.post_game_event("minecraft:item_interact_finish", body.bounds.feet, ctx);
        // `DeathProtection.applyEffects`: the component's own `death_effects`,
        // read from the protector copy (the totem's clear-then-effects, or a
        // custom list; `death_protection={}` has none).
        let user = self.consuming_player_entity();
        for effect in self.death_effects(&protector) {
            self.apply_consume_effect(&protector, &effect, None, user);
        }
        self.update_totem_ready();
        if self.side.lan_acting.is_some()
            && let Some(host) = &self.lan
        {
            host.hub.entity_event(entity, TOTEM_EVENT);
        }
        Some(Body {
            protector: Some(protector),
            ..body
        })
    }
    /// Settle the hit, returning the remaining events instead of resuming if the acting body is gone.
    pub(in crate::game) fn finish_guest_damage_before_resume(
        &mut self,
    ) -> Result<(), Vec<MotionEvent>> {
        let (Some(conn), Some(player)) = (self.side.lan_acting, self.player.as_mut()) else {
            return Err(Vec::new());
        };
        let Some(hit) = player.health.damage_completion.take() else {
            return Ok(());
        };
        let entity = player.entity_id;
        // Keep queued motion and packet effects with this body across the owning callbacks.
        let mut events = std::mem::take(&mut player.events);
        if std::mem::take(&mut player.health.totem_used) {
            let _ = self.apply_totem(entity);
        } else if let Some(index) = self.lan.as_ref().and_then(|host| {
            host.hub
                .guests
                .connection_index(conn)
                .filter(|&i| !host.hub.guests[i].dead)
        }) {
            // The killing blow and its weapon must reach the combat tracker before die reads it.
            // Packet and motion events still belong to the normal turn drain.
            let combat: Vec<_> = events
                .extract_if(.., |event| {
                    matches!(
                        event,
                        MotionEvent::Health(
                            HealthEvent::ArmorHurt { .. }
                                | HealthEvent::Criterion { .. }
                                | HealthEvent::MobHurt
                                | HealthEvent::RecordedDamage { .. }
                                | HealthEvent::Statistic { .. }
                        )
                    )
                })
                .collect();
            if !combat.is_empty() {
                self.record_combat(&combat);
                self.wear_armor(&combat);
            }
            self.guest_die(index, conn);
        }
        // The death or protection callback may change or remove the body. Post is still for
        // this hit, not for whatever state the callback left in its place.
        if let Some(post) = hit.post {
            self.finish_damage_post(entity, "minecraft:player", post);
        }
        if self.side.lan_acting == Some(conn)
            && let Some(player) = self.player.as_mut().filter(|p| p.entity_id == entity)
        {
            events.append(&mut player.events);
            player.events = events;
            Ok(())
        } else {
            Err(events)
        }
    }
    /// What each armed mob's hands carry as `minecraft:death_protection`, for
    /// `Mob::use_death_protection` (the mob has no item data of its own): any
    /// item with the component counts, not only the totem.
    pub(in crate::game::server_side) fn sync_mob_death_protection(&mut self) {
        let items = self.res.items();
        let of = |id: Option<&str>, stack: Option<&ItemStack>| {
            let item = stack.map(|s| &*s.item).or(id)?;
            match stack {
                Some(stack) => stack.death_protection(items.get(item)).map(<[_]>::to_vec),
                None => items
                    .get(item)
                    .and_then(|info| info.death_protection.clone()),
            }
        };
        for m in self.mobs.iter_mut().filter(|m| !m.removed) {
            if m.extra.main_hand_id().is_none() && m.extra.off_hand_id().is_none() {
                m.extra.hand_protection = None;
                continue;
            }
            m.extra.hand_protection = Some(sunrise_sim::mobs::HandProtection {
                main: of(m.extra.main_hand_id(), m.extra.main_hand_stack()),
                off: of(
                    m.extra.off_hand_id(),
                    m.extra.slot_stack(sunrise_items::equipment::OFFHAND),
                ),
            });
        }
    }
}
