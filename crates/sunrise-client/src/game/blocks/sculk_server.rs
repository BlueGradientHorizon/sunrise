#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, Result};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::DARKNESS_BLEND;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::HELP;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::Index;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::KIND_CALIBRATED;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::KIND_CATALYST;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::KIND_SENSOR;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::KIND_SHRIEKER;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::LISTENING_MOB_REACH;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::MobSlots;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::PlayerEvents;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::SculkState;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::TEST_POSTED_EVENT_ORDER;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::TEST_POSTED_EVENTS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::Tables;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::block_of;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::darkness_blend_added;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::dist_sqr_f;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::event_cause;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::flaps;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::sensor_signal;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::sculk::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::client::particles::PositionSource;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::collections::HashSet;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::actions::Action;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::join_seq::ListStamp;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    containers::BlockEntity,
    effects::{EffectInstance, PlayerEffects},
    fast_hash::{FxHashMap, FxHashSet},
    mobs::{Mob, MovementEmission as Steps, Species, Who, allay, warden},
    particles::{ParticleOptions, ParticleType},
    sculk::{self, Context, Info, SculkEntity, SculkKind, WardenSpawnTracker},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client_core::who::EventSource;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{
    geometry::{Aabb, Direction, Vec3},
    position::BlockPos,
    random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::registry::BlockStateId;

impl Game<'_> {
    /// `Entity.gameEvent(event, attributedTo)` at `at`, attributed to `source`
    /// (a LAN guest posts as that player), else to the entity itself.
    pub(in crate::game) fn entity_event_by(
        &mut self,
        event: &'static str,
        at: [f64; 3],
        source: Option<super::commands::Target>,
    ) {
        use super::commands::Target;
        if let Some(Target::Guest(conn)) = source {
            if self.server.lan_acting == Some(conn) {
                let ctx = self.player_context(None);
                self.post_game_event(event, at, ctx);
                return;
            }
            let posted = self
                .as_guest(conn, |g| {
                    let ctx = g.player_context(None);
                    g.post_game_event(event, at, ctx);
                })
                .is_some();
            if posted {
                return;
            }
        }
        let mut ctx = match source {
            Some(Target::Player) => self.player_context(None),
            Some(Target::Mob(id)) => self.mob_context(id, None),
            Some(target) => self
                .network_entity_id(target)
                .map(|id| self.entity_event_context(id as u32, None))
                .unwrap_or_default(),
            None => Context::default(),
        };
        // A stand is living, but Warden.canTargetEntity explicitly excludes it.
        ctx.living |= matches!(source, Some(Target::Stand(_)));
        self.post_game_event(event, at, ctx);
    }
    /// `GameEvent.Context.of(entity, state)` for a non-living entity.
    pub(in crate::game) fn entity_event_context(
        &self,
        id: u32,
        state: Option<BlockStateId>,
    ) -> Context {
        let dampens = self
            .dropped
            .iter()
            .find(|item| item.id == id)
            .is_some_and(|item| {
                world_items!(self)
                    .item_tags
                    .get("minecraft:dampens_vibrations")
                    .is_some_and(|tag| tag.contains(item.stack.item.as_ref()))
            });
        Context {
            source: Some(EventSource::Entity(id)),
            source_uuid: self.persist.uuid_of(id),
            affected_state: state,
            dampening_state: self.state_dampens(state),
            dampens,
            ..Context::default()
        }
    }
    /// `GameEvent.Context.of(player, state)`.
    pub(in crate::game) fn player_context(&self, state: Option<BlockStateId>) -> Context {
        let (dead, creative) = self
            .player
            .as_ref()
            .map_or((true, true), |p| (p.health.dead_or_dying(), self.creative));
        Context {
            source: Some(Who::Player.into()),
            owner: None,
            affected_state: state,
            source_uuid: None,
            owner_uuid: None,
            dampening_state: self.state_dampens(state),
            spectator: self.spectator(),
            stepping_carefully: self.server_acting_sneak(),
            dampens: false,
            targetable: !dead && !creative && !self.spectator(),
            living: true,
            player: true,
        }
    }
    /// `Level.gameEvent`: tell the listeners within reach.
    pub(in crate::game) fn post_game_event(
        &mut self,
        event: &'static str,
        pos: [f64; 3],
        ctx: Context,
    ) {
        #[cfg(test)]
        TEST_POSTED_EVENTS.with(|events| {
            if let Some(events) = events.borrow_mut().as_mut() {
                events.push((event, pos, ctx.source.and_then(|source| source.actor())));
            }
        });
        #[cfg(test)]
        TEST_POSTED_EVENT_ORDER.with(|order| {
            if let Some(order) = order.borrow_mut().as_mut() {
                order.push((
                    event,
                    ctx.source.and_then(|source| source.actor()),
                    self.server.lan_entity_events.len(),
                    if self.is_logical_server() {
                        self.server.pending_sounds.len()
                    } else {
                        0
                    },
                ));
            }
        });
        #[cfg(test)]
        super::item_use::interact_vibration_tests::record_event(
            event,
            pos,
            ctx.source.and_then(|source| source.actor()),
        );
        self.post_game_event_with(event, pos, |_| ctx);
    }
    /// [`Self::post_game_event`] with the context worked out only when some
    /// listener may hear the event (`GameEventDispatcher.post` visits only the
    /// listeners registered in the sections around it): most of a server's
    /// mobs are near none. A subscribed mod hook still hears every event first.
    pub(in crate::game) fn post_game_event_with(
        &mut self,
        event: &'static str,
        pos: [f64; 3],
        mut make_ctx: impl FnMut(&mut Self) -> Context,
    ) {
        super::dispense_fixture::record(|| {
            format!(
                "G {},{},{} {event}",
                pos[0].floor() as i32,
                pos[1].floor() as i32,
                pos[2].floor() as i32
            )
        });
        if !self.is_logical_server() {
            return;
        }
        let ctx = if mod_api::__private::listened(mod_api::HookId::VanillaGameEvent) {
            let ctx = make_ctx(self);
            let cause = event_cause(ctx.source, self.server.lan_acting_entity);
            let mut cancel = false;
            self.native_vanilla_game_event(
                self.dim.id(),
                event,
                pos,
                cause,
                ctx.affected_state,
                &mut cancel,
            );
            if cancel {
                return;
            }
            Some(ctx)
        } else {
            None
        };
        if !self.sculk.tables.built {
            return;
        }
        let at = block_of(pos);
        let radius = sculk::notification_radius(event).min(16);
        let near = self.sculk.index.within(at, radius);
        // The wardens and allays that may hear it, in the mobs' order, while
        // the slots taken for the mobs' events hold; else every mob is asked.
        let mobs = self.sculk.mob_slots.as_ref().and_then(|s| {
            let mut near = Vec::new();
            for &(i, id) in &s.listeners {
                let mob = self.mobs.get(i)?;
                if mob.id != id {
                    return None;
                }
                // A mod's game-event listener may have moved it since the slots were taken.
                let from = [mob.position.x, mob.position.y, mob.position.z];
                let d = dist_sqr_f(from, pos);
                if d.is_nan() || d <= LISTENING_MOB_REACH * LISTENING_MOB_REACH {
                    near.push(i);
                }
            }
            (s.next_entity == self.server.next_entity).then_some(near)
        });
        if near.is_empty() && mobs.as_ref().is_some_and(Vec::is_empty) {
            return;
        }
        let ctx = ctx.unwrap_or_else(|| make_ctx(self));
        let ctx = self.with_vibration_uuids(ctx);
        for key in near {
            self.sculk_listen(key, event, pos, &ctx);
        }
        match mobs {
            Some(mobs) => {
                for i in mobs {
                    self.mob_listen(i, event, pos, &ctx);
                }
            }
            None => {
                let stamp = self.mobs.stamp();
                let slots = self.listener_slots(stamp);
                for &i in &slots {
                    if (i as usize) < self.mobs.len() {
                        self.mob_listen(i as usize, event, pos, &ctx);
                    }
                }
                self.sculk.listeners = slots;
                self.sculk.listeners_of = Some(stamp);
            }
        }
    }
    /// A projectile's own event (`gameEvent(event)`, `Context.of(this, state)`):
    /// its owner as the projectile's owner, `state` the affected state. A
    /// guest's projectile posts as theirs, or with no owner once they have left
    /// (`Projectile.getOwner`), as its shot does.
    pub(in crate::game) fn projectile_own_event(
        &mut self,
        event: &'static str,
        owner: Option<Who>,
        guest: Option<u32>,
        at: [f64; 3],
        state: Option<BlockStateId>,
    ) {
        if let Some(conn) =
            guest.filter(|conn| owner.is_some() && self.server.lan_acting != Some(*conn))
        {
            let posted = self
                .as_guest(conn, |g| {
                    g.projectile_own_event(event, owner, None, at, state);
                })
                .is_some();
            if !posted {
                self.projectile_own_event(event, None, None, at, state);
            }
            return;
        }
        let mut ctx = Context {
            dampening_state: self.state_dampens(state),
            ..Context::default()
        };
        ctx.owner = owner;
        ctx.player = owner == Some(Who::Player) || self.rides_player(owner);
        self.post_game_event(event, at, ctx);
    }
    /// Events from entities moving, getting hurt and dying, derived from what
    /// changed this tick.
    pub(in crate::game) fn tick_entity_events(&mut self) {
        self.tick_player_sculk_events();
        self.tick_mob_sculk_events();
        if !self.sculk.falling_items.is_empty() {
            let present: std::collections::HashSet<u32> =
                self.dropped.iter().map(|item| item.id).collect();
            self.sculk.falling_items.retain(|id| present.contains(id));
        }
        // Dropped items are entities too: Entity.applyEffectsFromBlocks, from
        // ItemEntity.tick, so only for the items in entity-ticking chunks.
        let ticking = self.entity_ticking_area();
        let items: Vec<[f64; 3]> = self
            .dropped
            .iter()
            .filter(|i| {
                i.ground.on_ground
                    && ticking(i.position.x.floor() as i32, i.position.z.floor() as i32)
            })
            .map(|i| [i.position.x, i.position.y, i.position.z])
            .collect();
        for at in items {
            self.step_on_redstone_ore(at, false, false);
        }
    }
    pub(in crate::game) fn tick_mob_sculk_events(&mut self) {
        let snapshot: Vec<(
            u32,
            [f64; 3],
            bool,
            bool,
            Species,
            Option<i64>,
            Option<u32>,
            u32,
            bool,
        )> = self
            .mobs
            .iter()
            .filter(|m| m.alive())
            .map(|m| {
                (
                    m.id,
                    [m.position.x, m.position.y, m.position.z],
                    m.ground.on_ground,
                    m.fluids.in_water,
                    m.species(),
                    m.last_damage.map(|(_, t)| t),
                    m.extra.mount,
                    m.age,
                    m.species() == Species::Bat && sunrise_client::mobs::bat::resting(m),
                )
            })
            .collect();
        let mut by_id = FxHashMap::default();
        for (i, m) in self.mobs.iter().enumerate() {
            by_id.entry(m.id).or_insert(i);
        }
        let listeners = self
            .mobs
            .iter()
            .enumerate()
            .filter(|(_, m)| matches!(m.species(), Species::Warden | Species::Allay))
            .map(|(i, m)| (i, m.id))
            .collect();
        self.sculk.mob_slots = Some(MobSlots {
            by_id,
            listeners,
            next_entity: self.server.next_entity,
        });
        // Steps, landings, splashes, flaps and stepping on redstone ore come
        // from the mob's own tick (Entity.move, LivingEntity.aiStep's
        // applyEffectsFromBlocks), which ServerLevel.tick runs only in
        // entity-ticking chunks: for the mobs `tick_creature_entities` ticked,
        // and those standing in that area (the sculk fixture posts before the
        // mobs tick). Damage and mounting are raised by `hurt` and
        // `startRiding`, ticking or not.
        let ticking = self.entity_ticking_area();
        for (id, at, on_ground, in_water, species, damaged, vehicle, age, resting) in snapshot {
            let ticked = self.creatures.ticked.contains(&id)
                || ticking(at[0].floor() as i32, at[2].floor() as i32);
            if on_ground && ticked {
                self.step_on_redstone_ore(at, false, false);
            }
            // A mob first seen already riding raised its event when it spawned.
            let seen = self
                .mob_slot(id)
                .is_some_and(|i| self.mobs[i].movement_emission.is_some());
            self.mount_events(Who::Mob(id), vehicle, seen);
            let emits = !matches!(species, Species::Shulker);
            if emits && !ticked {
                // Where it is now, without a move: a mob put elsewhere while
                // it did not tick (a command) walked nowhere.
                if let Some(slot) = self.motion_steps_mut(Who::Mob(id)) {
                    let entry = slot.get_or_insert(Steps {
                        move_dist: 0.0,
                        next_step: 1.0,
                        last: at,
                        on_ground,
                        in_water,
                    });
                    entry.last = at;
                    entry.on_ground = on_ground;
                    entry.in_water = in_water;
                }
            } else if emits {
                let warden = species == Species::Warden;
                self.entity_motion_events(
                    Who::Mob(id),
                    at,
                    on_ground,
                    in_water,
                    false,
                    false,
                    warden,
                );
                // Entity.processFlappingMovement: what flaps this move flaps
                // over air only (the mob's own `MobEvent::GameEvent` is not
                // read here).
                if flaps(species, resting, on_ground, age) {
                    let below = self.world.block(
                        at[0].floor() as i32,
                        (at[1] - 1.0e-5).floor() as i32,
                        at[2].floor() as i32,
                    );
                    if self.res.facts().is_air(below) {
                        self.post_entity_event("minecraft:flap", Who::Mob(id));
                    }
                }
            }
            if let Some(t) = damaged
                && self.sculk.damaged.get(&id) != Some(&t)
            {
                self.sculk.damaged.insert(id, t);
                self.post_entity_event("minecraft:entity_damage", Who::Mob(id));
            }
        }
        self.sculk.mob_slots = None;
        let alive: FxHashSet<u32> = self.mobs.iter().map(|m| m.id).collect();
        self.sculk.damaged.retain(|id, _| alive.contains(id));
        self.sculk.mounts.retain(|who, _| match who {
            Who::Player => true,
            Who::Mob(id) => alive.contains(id),
        });
    }
    /// Entity.move emits on the authoritative server too, for each ServerPlayer.
    pub(in crate::game) fn tick_player_sculk_events(&mut self) {
        // The player.
        let player = self.player.as_ref().map(|p| {
            (
                [p.position.x, p.position.y, p.position.z],
                p.ground.on_ground,
                p.health.health(),
                p.flying,
            )
        });
        if let Some((at, on_ground, health, flying)) = player {
            let crouch = self.server_acting_sneak();
            if on_ground && !self.spectator() {
                self.step_on_redstone_ore(at, crouch, true);
            }
            self.mount_events(Who::Player, self.riding, true);
            let in_water = self.in_water_at(at);
            self.entity_motion_events(
                Who::Player,
                at,
                on_ground,
                in_water,
                crouch && on_ground,
                flying,
                false,
            );
            if let Some(last) = self.sculk.last_health
                && health < last
            {
                self.post_entity_event("minecraft:entity_damage", Who::Player);
            }
            self.sculk.last_health = Some(health);
        }
    }
    /// The block entity phase for sculk: vibration tickers and catalysts, in
    /// the order the block entities were added (`Level.tickBlockEntities`).
    pub(in crate::game::server_side) fn tick_sculk_blocks(&mut self) -> Result<()> {
        self.sculk_prepare();
        self.sculk_sync_index();
        // `ServerLevel.shouldTickBlocksAt` in a block-ticking chunk, from
        // every player's tickets (`update_chunk_tickets`), once the chunk is
        // lit: asked once per chunk, as the sensors share few chunks. Nothing
        // a sculk tick does loads, unloads or lights a chunk or moves a ticket.
        let levels = self.tickets.0.levels();
        let chunks = &self.world.chunks;
        let pending = &self.world.entities_pending;
        let mut ticking = FxHashMap::<(i32, i32), bool>::default();
        let mut ticks = |cx: i32, cz: i32| {
            *ticking.entry((cx, cz)).or_insert_with(|| {
                levels.block_ticking(cx, cz) && chunks.get(&(cx, cz)).is_some_and(|c| c.lit)
            })
        };
        // The ticking ones in the index's order, with whether the chunks
        // around them tick too. Most sensors and catalysts wait for a
        // vibration or a death: their tick would change nothing, so they
        // are passed over here (a vibration scheduled during these ticks is
        // chosen on the next tick at the earliest, as in vanilla).
        let entities = &self.world.block_entities;
        let due: Vec<((i32, i32, i32), bool)> = self
            .sculk
            .index
            .iter()
            .filter_map(|&key| {
                let (cx, cz) = (key.0 >> 4, key.2 >> 4);
                // `LevelChunk.isTicking`: its own chunk's entities loaded too.
                if !ticks(cx, cz)
                    || pending.contains(&(cx, cz))
                    || matches!(entities.get(&key), Some(BlockEntity::Sculk(e)) if e.idle())
                {
                    return None;
                }
                let adjacent = (-1..=1).all(|dx| (-1..=1).all(|dz| ticks(cx + dx, cz + dz)));
                Some((key, adjacent))
            })
            .collect();
        for (key, adjacent) in due {
            self.tick_sculk_block(key, adjacent)?;
        }
        Ok(())
    }
    /// `SpawnUtil.trySpawnMob(WARDEN, TRIGGERED, level, pos, 20, 5, 6,
    /// ON_TOP_OF_COLLIDER, false)`: every candidate spot creates a warden
    /// (`EntityType.create` draws its yaw from the level random and
    /// `finalizeSpawn` plays its agitated sound), which is discarded when
    /// `checkSpawnObstruction` fails.
    pub(in crate::game) fn try_summon_warden(
        &mut self,
        key: (i32, i32, i32),
        warning: i32,
    ) -> bool {
        if warning < 4 {
            return false;
        }
        let Some(kind) = self.res.mob_types().get("minecraft:warden").cloned() else {
            return false;
        };
        for _ in 0..20 {
            let r = &mut self.world.random;
            let dx = r.next_int_bound(11).unwrap_or(0) - 5;
            let dz = r.next_int_bound(11).unwrap_or(0) - 5;
            let Some(at) = self.spawn_position(key.0 + dx, key.1 + 6, key.2 + dz, 6) else {
                continue;
            };
            let spot = [
                f64::from(at.0) + 0.5,
                f64::from(at.1),
                f64::from(at.2) + 0.5,
            ];
            let yaw = mod_api::client::camera::wrap_degrees(self.world.random.next_float() * 360.0);
            self.server.next_entity += 1;
            let mut mob = Mob::new(
                self.server.next_entity,
                kind.clone(),
                sunrise_core::geometry::Vec3::new(spot[0], spot[1], spot[2]),
                yaw,
            );
            // Warden.finalizeSpawn, then Mob.finalizeSpawn's draws from the
            // level random: the follow range bonus and left-handedness.
            let mut events = Vec::new();
            warden::finalize(&mut mob, true, &mut events);
            let _ = self.handle_mob_events(events);
            let r = &mut self.world.random;
            let bonus = r.triangle(0.0, 0.11485000000000001);
            mob.attrs.follow_range += mob.kind.follow_range * bonus;
            mob.update_max_visited_nodes();
            let _left_handed = r.next_float() < 0.05;
            if !self.warden_fits(&mob) {
                continue;
            }
            self.mobs.push(mob);
            self.native_mobs_joined(self.mobs.len() - 1, false);
            return true;
        }
        false
    }
}
