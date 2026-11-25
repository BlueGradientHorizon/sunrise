#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{
    Game,
    commands::Target,
    entity::{
        Entity as _, EntityHandle, ThunderHit, entity_lists_of, for_each_entity, with_entity,
    },
    minecarts::CartHit,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::lightning::Bolt;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::lightning::StrikeState;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::lightning::thunder_strike_column;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::lightning::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::advancements::EntityView;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::mobs::{Damage, DamageKind, Species};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::random_source;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{
    geometry::{Aabb, Vec3},
    position::BlockPos,
    random::{LegacyRandom, RandomSource},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::level::Ticks;

impl Game<'_> {
    /// Load a `minecraft:lightning_bolt` from NBT (`/summon`, `/execute summon`, or a `Passengers` entry).
    pub(in crate::game::server_side) fn load_bolt(
        &mut self,
        mut tag: sunrise_core::nbt::Tag,
        mount: Option<u32>,
        from_command: bool,
    ) -> Option<u32> {
        let pos = sunrise_client::entity_nbt::position(&tag).unwrap_or(Vec3::ZERO);
        let at = [
            pos.x.clamp(-3.000_051_2e7, 3.000_051_2e7),
            pos.y.clamp(-2.0e7, 2.0e7),
            pos.z.clamp(-3.000_051_2e7, 3.000_051_2e7),
        ];
        if from_command {
            let block = at.map(|v| v.floor() as i32);
            if !self.lightning_strike_allowed(self.server.lan_acting_uuid, block, "command") {
                return None;
            }
        }
        Self::normalize_loaded_mob_tags(&mut tag);
        let id = self.take_entity_id();
        let (mut bolt, constructor_uuid) = Bolt::new(id, at, false);
        bolt.apply_nbt(&tag);
        let uuid = sunrise_client::entity_nbt::entity_uuid(&tag).unwrap_or(constructor_uuid);
        if self.is_loaded_uuid_in_level(uuid) {
            return None;
        }
        if let Some(vehicle) = mount {
            bolt.mount = Some(vehicle);
            bolt.vehicle_order = i64::from(id);
            if let Some(seat) = self.vehicle_passenger_pos(vehicle) {
                bolt.at = seat;
            }
        }
        let mut kept = sunrise_core::nbt::Compound::new();
        if let Some(tags @ sunrise_core::nbt::Tag::List(_)) = tag.get("Tags") {
            kept.insert("Tags".into(), tags.clone());
        }
        if let Some(data @ sunrise_core::nbt::Tag::Compound(d)) = tag.get("data")
            && !d.is_empty()
        {
            kept.insert("data".into(), data.clone());
        }
        if let Some(team) = tag.get("Team") {
            kept.insert("Team".into(), team.clone());
        }
        self.remember(id, Some(uuid), sunrise_core::nbt::Tag::Compound(kept));
        self.load_bolt_client();
        self.bolts.push(bolt);
        Some(id)
    }
    pub(in crate::game) fn thunder_hit_state(
        &mut self,
        bolt: u32,
        at: [f64; 3],
        state: &mut StrikeState,
    ) -> Vec<EntityView> {
        let strike = Aabb::new(
            Vec3::new(at[0] - 3.0, at[1] - 3.0, at[2] - 3.0),
            Vec3::new(at[0] + 3.0, at[1] + 9.0, at[2] + 3.0),
        );
        // Whether the box of an entity standing at `p` (half width `w`,
        // height `h`) meets the strike box.
        let meets = move |p: [f64; 3], w: f64, h: f64| {
            strike.intersects(Aabb::new(
                Vec3::new(p[0] - w, p[1], p[2] - w),
                Vec3::new(p[0] + w, p[1] + h, p[2] + w),
            ))
        };
        let damage = Damage {
            kind: DamageKind::Generic,
            attacker: None,
            from: Some(at),
            attacker_species: None,
        };
        let mut struck = Vec::new();
        let mut powered = Vec::new();
        let peaceful = self.ui.difficulty == 0;
        let mut left = self.thunder_count(strike, ThunderHit::Mob);
        for h in self.entity_handles() {
            if left == 0 {
                break;
            }
            let EntityHandle::Mob(i) = h else { continue };
            if !self.thunder_struck(h, strike, ThunderHit::Mob) {
                continue;
            }
            left -= 1;
            struck.push(self.mob_entity(&self.mobs[i]));
            // LivingConversionEvent.Pre: a canceled conversion leaves the
            // mob to the lightning's fire and damage, as `super.thunderHit`.
            let outcome = match self.mobs[i].species() {
                Species::Pig if !peaceful => Some("minecraft:zombified_piglin"),
                Species::Villager if !peaceful => Some("minecraft:witch"),
                _ => None,
            };
            state.hit.push(self.mobs[i].id);
            if !self.lightning_may_strike(self.mobs[i].id, bolt) {
                continue;
            }
            if let Some(outcome) = outcome {
                let old = self.mobs[i].clone();
                let params = sunrise_client::mobs::conversion::Params {
                    keep_equipment: false,
                    preserve_can_pick_up_loot: old.species() == Species::Pig,
                };
                if self
                    .convert_to(&old, outcome, params, false, |_, _| {})
                    .is_some()
                {
                    continue;
                }
            }
            let m = &mut self.mobs[i];
            let at = [m.position.x, m.position.y, m.position.z];
            match m.species() {
                Species::Mooshroom => {
                    if !state.converted.contains(&m.id) {
                        state.converted.push(m.id);
                        m.extra.variant = 1 - m.extra.variant.min(1);
                        self.server_play_sound("minecraft:entity.mooshroom.convert", at, 2.0, 1.0);
                    }
                    continue;
                }
                Species::Turtle => {
                    m.damage(
                        crate::game::entity::damage_level_of!(
                            self,
                            &sunrise_client::health::NO_ATTACK,
                            no_push,
                            None
                        ),
                        f32::MAX,
                        damage,
                    );
                    self.finish_mob_damage_at(i);
                    continue;
                }
                Species::Creeper => {
                    m.extra.charged = true;
                    powered.push(m.id);
                }
                // CopperGolem.thunderHit (after Entity.thunderHit's fire and
                // damage): once per bolt, an oxidized golem goes back one step
                // and its weathering timer (or wax, -2) is reset to -1.
                Species::CopperGolem if !state.converted.contains(&m.id) => {
                    state.converted.push(m.id);
                    if m.extra.variant > 0 {
                        m.extra.vector[0] = -1.0;
                        m.extra.variant =
                            sunrise_client::mobs::copper_golem::weather_previous(m.extra.variant);
                        self.persist.force_meta(m.id, 16);
                    }
                }
                _ => {}
            }
            // Entity.thunderHit: burning (8 seconds) and 5 lightning damage.
            if !m.mod_fire_immune {
                m.fire_ticks += 1;
                if m.fire_ticks == 0 {
                    m.ignite_for_ticks(160);
                }
            }

            m.damage(
                crate::game::entity::damage_level_of!(
                    self,
                    &sunrise_client::health::NO_ATTACK,
                    no_push,
                    None
                ),
                5.0,
                damage,
            );
            self.finish_mob_damage_at(i);
        }
        for entity in powered {
            self.native_creeper_power(entity, [at[0], at[1], at[2]]);
        }
        // Cushion.thunderHit: a cushion whose box meets the strike box breaks and drops its item
        // (the bolt is no player, so no creative spare). Last first, since
        // break_cushion removes the entry.
        for i in (0..self.cushions.len()).rev() {
            let (lo, hi) = self.cushions[i].bounds();
            if strike.intersects(Aabb::new(
                Vec3::new(lo[0], lo[1], lo[2]),
                Vec3::new(hi[0], hi[1], hi[2]),
            )) && self.lightning_may_strike(self.cushions[i].id, bolt)
            {
                let before = self.dropped.len();
                self.break_cushion(i, false);
                state
                    .spared
                    .extend(self.dropped[before..].iter().map(|e| e.id));
            }
        }
        // Vehicles, then orbs: `VehicleEntity.hurtServer` for 5 damage (x 10
        // over the 40 a vehicle takes) destroys them. Lightning is no fire
        // source, so a TNT cart does not prime. `ExperienceOrb.hurtServer`:
        // 5 damage off 5 health. Last first, since each removes its entry.
        let mut left = self.thunder_count(strike, ThunderHit::Vehicle)
            + self.thunder_count(strike, ThunderHit::Orb);
        for h in self.entity_handles_rev() {
            if left == 0 {
                break;
            }
            if !self.thunder_struck(h, strike, ThunderHit::Vehicle)
                && !self.thunder_struck(h, strike, ThunderHit::Orb)
            {
                continue;
            }
            left -= 1;
            if !self.lightning_may_strike(with_entity!(self, h, e => e.id()), bolt) {
                continue;
            }
            match h {
                EntityHandle::Boat(i) => {
                    let id = self.boats[i].id;
                    let kind = self.boats[i].entity_type();
                    let source = sunrise_client::health::HurtSource {
                        damage_type: "minecraft:lightning_bolt",
                        causing: bolt as i32,
                        direct: bolt as i32,
                        position: Some(at),
                    };
                    let vanilla = self.saved_refuses_damage(id, "minecraft:lightning_bolt", false);
                    if !self.native_nonliving_invulnerable(id, &kind, source, vanilla) {
                        self.hurt_boat(i, 5.0, false);
                    }
                }
                EntityHandle::Cart(i) => {
                    let (id, kind) = (self.carts[i].id, self.carts[i].kind.id());
                    let source = sunrise_client::health::HurtSource {
                        damage_type: "minecraft:lightning_bolt",
                        causing: bolt as i32,
                        direct: bolt as i32,
                        position: Some(at),
                    };
                    let vanilla = self.saved_refuses_damage(id, "minecraft:lightning_bolt", false);
                    if !self.native_nonliving_invulnerable(id, kind, source, vanilla) {
                        self.hurt_cart(i, 5.0, false, false, CartHit::default());
                    }
                }
                EntityHandle::Orb(i) => {
                    let id = self.orbs[i].id;
                    let source = sunrise_client::health::HurtSource {
                        damage_type: "minecraft:lightning_bolt",
                        causing: bolt as i32,
                        direct: bolt as i32,
                        position: Some(at),
                    };
                    let vanilla = self.saved_refuses_damage(id, "minecraft:lightning_bolt", false);
                    if !self.native_nonliving_invulnerable(
                        id,
                        "minecraft:experience_orb",
                        source,
                        vanilla,
                    ) && self.orbs[i].hurt(5.0)
                    {
                        self.orbs.remove(i);
                    }
                }
                _ => {}
            }
        }
        self.native_leaves_flush();
        // Every player in the box, the server's players too (0.6 x 1.8).
        let strike_players = move |g: &mut Self| {
            if let Some(p) = g.player.as_ref().map(|p| p.position)
                && meets([p.x, p.y, p.z], 0.3, 1.8)
                && g.lightning_may_strike(g.own_entity_id() as u32, bolt)
            {
                if let Some(p) = g.player.as_mut()
                    && !p.mod_fire_immune
                {
                    p.fire_ticks += 1;
                    if p.fire_ticks == 0
                        && let Err(error) = p.ignite_for_ticks(160)
                    {
                        g.error = Some(error);
                    }
                }
                g.hurt_player_by("minecraft:lightning_bolt", 5.0);
            }
        };
        strike_players(self);
        self.each_guest_hit(strike_players);
        // Items run Entity.thunderHit, then the shared ItemEntity hurt path.
        let mut left = self.thunder_count(strike, ThunderHit::Item);
        for h in self.entity_handles_rev() {
            if left == 0 {
                break;
            }
            let EntityHandle::Item(i) = h else { continue };
            if !self.thunder_struck(h, strike, ThunderHit::Item) {
                continue;
            }
            left -= 1;
            let e = &self.dropped[i];
            state.hit_items.push(e.id);
            if !self.lightning_may_strike(e.id, bolt) {
                continue;
            }
            let e = &mut self.dropped[i];
            e.fire_ticks += 1;
            if e.fire_ticks == 0 {
                e.fire_ticks = 160;
            }
            // Cushion spare stands in for vanilla's invulnerableTime window.
            if e.age < 20 && state.spared.contains(&e.id) {
                continue;
            }
            let id = e.id;
            self.hurt_item_entity(
                id,
                5.0,
                "minecraft:lightning_bolt",
                Some(Target::Bolt(bolt)),
                None,
                Some(at),
            );
        }
        self.native_leaves_flush();
        struck
    }
    /// `ServerChunkCache.tickSpawningChunk`: one `tickThunder` roll per
    /// spawning chunk (within 128 blocks of a player) that is in entity
    /// ticking range.
    pub(in crate::game::server_side) fn tick_thunder(&mut self) {
        // isRaining (rain level > 0.2) and isThundering (> 0.9) both need
        // Level.canHaveWeather: sky light, no ceiling and not the End.
        let (rain, thunder) = self.world.weather.levels(1.0);
        let weather = self.world.shape.sky_light
            && !self.world.shape.has_ceiling
            && self.dim != super::Dim::End;
        if rain <= 0.2 || thunder <= 0.9 || !weather || self.server.world_debug {
            return;
        }
        let mut scratch = std::mem::take(&mut self.server.spawn_scratch);
        self.spawning_chunks(&self.level_players(), &mut scratch);
        let levels = self.tickets.0.levels();
        scratch
            .candidates
            .retain(|&(x, z)| levels.random_ticking(x, z));
        for (cx, cz) in scratch.candidates.drain(..) {
            if self.world.random.next_int_bound(100_000).unwrap_or(1) != 0 {
                continue;
            }
            // getBlockRandomPos(minX, 0, minZ, 15)
            let (x, z) = thunder_strike_column(&mut self.block_ticks, cx, cz);
            let Some(pos) = self.lightning_target(x, z) else {
                continue;
            };
            let p = BlockPos::new(pos[0], pos[1], pos[2]);
            if !self.with_level(|b, level, _| b.is_raining_at(level, p)) {
                continue;
            }
            // Skeleton trap: the local effective difficulty / 100
            // (`getCurrentDifficultyAt(pos)`).
            let difficulty = f64::from(
                self.local_difficulty([f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2])])
                    .effective,
            );
            let below = &self
                .res
                .facts()
                .get(self.world.block(pos[0], pos[1] - 1, pos[2]))
                .name;
            let trap = self
                .ui
                .rules
                .bool_at(sunrise_client::gamerules::vanilla::SPAWN_MOBS)
                && self.world.random.next_double() < difficulty * 0.01
                && !below.ends_with("lightning_rod");
            if !self.lightning_strike_allowed(None, pos, "weather") {
                continue;
            }
            if trap {
                let at = [f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2])];
                // ServerLevel.tickChunk: a trap horse waits for a player.
                if let Some(id) = self.spawn_mob_raw("minecraft:skeleton_horse", at, false)
                    && let Some(h) = self.mobs.iter_mut().find(|m| m.id == id)
                {
                    h.extra.trap = true;
                    h.extra.age = 0;
                }
            }
            let at = [
                f64::from(pos[0]) + 0.5,
                f64::from(pos[1]),
                f64::from(pos[2]) + 0.5,
            ];
            self.spawn_bolt(at, trap);
        }
        self.server.spawn_scratch = scratch;
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::client_side::lightning::tests::Spare;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::client_side::lightning::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_entity_refs::tests::collect_entities_in;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use mod_api::{self, ServerHost};
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use std::sync::Arc;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_client::mobs::{
        Anim, Brain, Mob, MobType, Part, Species,
    };
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_core::geometry::Vec3;

    #[test]
    #[ignore = "daily"]
    fn conversion_post_reads_original_and_lightning_outcome() {
        use mod_api::{
            EntityJoinLevelEvent, EquipmentSlot, Flow, Hooks, LivingConversionPostEvent,
            LivingConversionPreEvent,
        };
        use mod_platform::__private::install;
        use std::sync::{
            Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        };

        #[derive(Debug)]
        struct SeenConversion {
            old: Option<String>,
            outcome: Option<String>,
            weapon: Option<String>,
            mushrooms: usize,
        }
        struct Observer {
            seen: Arc<Mutex<Vec<SeenConversion>>>,
            pre: Arc<AtomicUsize>,
            cancel_cow: Arc<AtomicBool>,
        }
        #[mod_api::__subscriptions]
        impl Hooks for Observer {
            fn living_conversion_pre(
                &self,
                _: &mut dyn ServerHost,
                _: &mut LivingConversionPreEvent<'_>,
            ) -> Flow {
                self.pre.fetch_add(1, Ordering::SeqCst);
                Flow::Continue
            }
            fn entity_join_level(
                &self,
                _: &mut dyn ServerHost,
                event: &mut EntityJoinLevelEvent<'_>,
            ) -> Flow {
                if event.kind == "minecraft:cow" && self.cancel_cow.load(Ordering::SeqCst) {
                    Flow::Cancel
                } else {
                    Flow::Continue
                }
            }
            fn living_conversion_post(
                &self,
                host: &mut dyn ServerHost,
                event: &mut LivingConversionPostEvent<'_>,
            ) {
                let old = host
                    .entity(event.entity)
                    .and_then(|entity| entity.kind(host)?.id(host))
                    .map(|name| name.as_str().to_owned());
                let outcome = host.entity(event.converted);
                let weapon = outcome
                    .and_then(|entity| host.living(entity))
                    .and_then(|living| {
                        living.with_equipment(host, EquipmentSlot::MainHand, |stack| {
                            mod_api::__private::item_key_id(stack.item())
                                .unwrap()
                                .to_string()
                        })
                    });
                let mushrooms = host
                    .level("minecraft:overworld")
                    .and_then(|level| {
                        collect_entities_in(host, level, [-10.0, 60.0, -10.0], [20.0, 70.0, 20.0])
                            .ok()
                    })
                    .unwrap()
                    .into_iter()
                    .filter_map(|entity| host.item_entity(entity))
                    .filter(|item| {
                        item.with_stack(host, |stack| {
                            mod_api::__private::item_key_id(stack.item())
                                .unwrap()
                                .ends_with("mushroom")
                        }) == Some(true)
                    })
                    .count();
                self.seen.lock().unwrap().push(SeenConversion {
                    old,
                    outcome: outcome
                        .and_then(|entity| entity.kind(host)?.id(host))
                        .map(|name| name.as_str().to_owned()),
                    weapon,
                    mushrooms,
                });
            }
        }
        let seen = Arc::new(Mutex::new(Vec::new()));
        let pre = Arc::new(AtomicUsize::new(0));
        let cancel_cow = Arc::new(AtomicBool::new(false));
        let mods: mod_api::Mods = Arc::from([mod_api::ModEntry::new(
            "lightning-observer",
            Box::new(Observer {
                seen: seen.clone(),
                pre: pre.clone(),
                cancel_cow: cancel_cow.clone(),
            }),
        )]);
        install(mods);
        super::super::native_entity_refs::tests::with_game(|game| {
            let piglin_type = game
                .res
                .mob_types()
                .get("minecraft:piglin")
                .unwrap()
                .clone();
            game.server.next_entity += 1;
            let mut old = Mob::new(
                game.server.next_entity,
                piglin_type,
                Vec3::new(100.0, 64.0, 100.0),
                0.0,
            );
            old.removed = true; // The tick loop has already taken this mob out.
            let converted = game
                .convert_to(
                    &old,
                    "minecraft:zombified_piglin",
                    sunrise_client::mobs::conversion::Params::KEEP_ALL,
                    false,
                    |_, _| {},
                )
                .unwrap();
            assert!(game.entity(old.id).is_none());
            assert!(
                game.mobs
                    .iter()
                    .find(|mob| mob.id == converted)
                    .unwrap()
                    .has_effect("minecraft:nausea")
            );

            let pig_type = game.res.mob_types().get("minecraft:pig").unwrap().clone();
            game.server.next_entity += 1;
            let pig_id = game.server.next_entity;
            game.mobs.push(Mob::new(
                pig_id,
                pig_type.clone(),
                Vec3::new(0.0, 64.0, 0.0),
                0.0,
            ));
            game.ui.difficulty = 2;
            let mut bolt = Bolt::struck_at(0, [0.0, 64.0, 0.0]);
            game.thunder_hit(&mut bolt);
            assert!(game.entity(pig_id).is_none());
            let mushroom_type = game
                .res
                .mob_types()
                .get("minecraft:mooshroom")
                .unwrap()
                .clone();
            for (x, player) in [(5, true), (10, false)] {
                game.server.next_entity += 1;
                let id = game.server.next_entity;
                game.mobs.push(Mob::new(
                    id,
                    mushroom_type.clone(),
                    Vec3::new(f64::from(x) + 0.5, 64.0, 0.5),
                    0.0,
                ));
                if player {
                    game.inventory[game.selected] =
                        Some(sunrise_client::items::ItemStack::new("minecraft:shears", 1));
                    assert!(
                        game.interact_mob(Some(super::super::InteractTarget {
                            id: id as i32,
                            location: [0.0; 3]
                        }))
                        .unwrap()
                    );
                } else {
                    assert!(game.shear_entity((x, 64, 0)).unwrap());
                }
            }
            assert_eq!(pre.load(Ordering::SeqCst), 4);
            {
                let seen = seen.lock().unwrap();
                assert_eq!(seen.len(), 4);
                assert_eq!(seen[2].mushrooms, 5);
                assert_eq!(seen[3].mushrooms, 10);
            }
            cancel_cow.store(true, Ordering::SeqCst);
            game.server.next_entity += 1;
            let old_id = game.server.next_entity;
            let old = Mob::new(old_id, mushroom_type, Vec3::new(15.0, 64.0, 0.0), 0.0);
            game.mobs.push(old.clone());
            game.server.next_entity += 1;
            let rider_id = game.server.next_entity;
            let mut rider = Mob::new(rider_id, pig_type, Vec3::new(15.0, 64.0, 0.0), 0.0);
            rider.extra.mount = Some(old_id);
            game.mobs.push(rider);
            assert!(
                game.convert_to(
                    &old,
                    "minecraft:cow",
                    sunrise_client::mobs::conversion::Params::KEEP_ALL,
                    false,
                    |_, _| {}
                )
                .is_none()
            );
            assert_eq!(
                game.mobs
                    .iter()
                    .find(|m| m.id == rider_id)
                    .unwrap()
                    .extra
                    .mount,
                Some(old_id)
            );
            assert!(!game.mobs.iter().find(|m| m.id == old_id).unwrap().removed);
            let seen = seen.lock().unwrap();
            assert_eq!(seen.len(), 4);
            assert_eq!(seen[0].old.as_deref(), Some("minecraft:piglin"));
            assert_eq!(
                seen[0].outcome.as_deref(),
                Some("minecraft:zombified_piglin")
            );
            assert_eq!(seen[1].old.as_deref(), Some("minecraft:pig"));
            assert_eq!(
                seen[1].outcome.as_deref(),
                Some("minecraft:zombified_piglin")
            );
            assert!(matches!(
                seen[1].weapon.as_deref(),
                Some("minecraft:golden_sword" | "minecraft:golden_spear")
            ));
        });
    }
}
