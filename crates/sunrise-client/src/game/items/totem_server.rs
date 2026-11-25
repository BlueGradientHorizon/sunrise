#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::render_tick::player_tracking_body;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::totem::Body;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::totem::TOTEM;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::totem::TOTEM_EVENT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::totem::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_assets::item_model::DisplayContext;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    block_entity_render::vertex_light,
    health::HealthEvent,
    items::ItemStack,
    mesher::Vertex,
    motion::MotionEvent,
    particles::{ParticleOptions, ParticleType, TrackingBody, TrackingTarget},
};

impl Game<'_> {
    /// The server's half once a totem fired: use it up (main hand first),
    /// count it, set health to 1, then clear and apply its effects. Returns
    /// the player's body with the protector copy the activation animation shows.
    /// A server guest's entity event is broadcast before the hit's Post.
    pub(in crate::game::server_side) fn apply_totem(&mut self, entity: i32) -> Option<Body> {
        // The mods hear a server player's totem before it is used up.
        if let (Some(uuid), Some(name)) = (
            self.server.lan_acting_uuid,
            self.server.lan_acting_name.clone(),
        ) {
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
        if self.server.lan_acting.is_some()
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
        let (Some(conn), Some(player)) = (self.server.lan_acting, self.player.as_mut()) else {
            return Err(Vec::new());
        };
        let Some(hit) = player.health.damage_completion.take() else {
            return Ok(());
        };
        let entity = player.entity_id;
        // Keep queued motion and packet effects with this body across the owning callbacks.
        let mut events = std::mem::take(&mut player.events);
        if std::mem::take(&mut player.health.totem_used) {
            if let Some(body) = self.apply_totem(entity)
                && self.serving.is_none()
                && !self.is_dedicated_server()
            {
                self.totem_event(TrackingTarget::Entity(entity), body);
            }
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
        if self.server.lan_acting == Some(conn)
            && let Some(player) = self.player.as_mut().filter(|p| p.entity_id == entity)
        {
            events.append(&mut player.events);
            player.events = events;
            Ok(())
        } else {
            Err(events)
        }
    }
}
