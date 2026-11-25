#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{
    Game, Screen,
    commands::Target,
    dragon::{Breath, FightPlayer, FightWho},
    item_draw::owner_at,
    living_attributes::AttributeWrite,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::BodyPotions;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::CloudParticle;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::CloudSettings;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::LINGERING_WAIT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::PotionCloud;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::PotionOwner;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::PotionState;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::REAPPLICATION_DELAY;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::ThrownPotion;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::Victim;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::collect_dragon_breath;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::mob_can_be_affected;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::mob_effect_id;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::potion_cloud_to_nbt;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::sound_on_added;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::splash_distance_sqr;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::thrown_projectile;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
    EffectApplicable, EffectTickChange, EntityRef, LivingRef, PlayerFood,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_assets::item_model::DisplayContext;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::health::Attack;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    block_entity_render::vertex_light,
    brewing,
    effects::{EffectAction, EffectInstance, OmenTick, PlayerEffects, effect_modifier},
    hud::HudBatch,
    hud_layout, hud_overlays, inventory_effects,
    items::ItemStack,
    mesher::Vertex,
    mobs::{Damage, DamageKind, Mob, MobEffectAction, ModEffectTick, NamedDamage, Who},
    particles::{ParticleOptions, ParticleType},
    projectiles::{Projectile, ProjectileKind},
    text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{
    geometry::{Aabb, Direction, Vec3},
    random::{LegacyRandom, RandomSource},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_gameplay::particle_options::custom_particle_options;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_gameplay::particle_options::particle_options;

impl CloudSettings {
    pub(in crate::game) fn lingering() -> Self {
        Self {
            wait_time: LINGERING_WAIT,
            reapplication_delay: REAPPLICATION_DELAY,
            radius_on_use: -0.5,
            duration_on_use: 0,
            duration_scale: 0.25,
        }
    }
}

impl Game<'_> {
    pub(in crate::game) fn apply_effect_action(&mut self, action: EffectAction) {
        let action = match action {
            EffectAction::Heal(mut amount) => {
                if let Some(player) = self.player.as_ref() {
                    let mut cancel = false;
                    sunrise_client::hunger::native_living_heal(
                        &self.loop_context,
                        player.entity_id as u32,
                        &mut amount,
                        &mut cancel,
                    );
                    if cancel {
                        return;
                    }
                }
                EffectAction::Heal(amount)
            }
            action => action,
        };
        let rules = self.world.health_rules;
        let Some(p) = self.player.as_mut() else {
            self.combat.0.forget_noted(false);
            return;
        };
        match action {
            EffectAction::Heal(amount) => {
                let max = p.attribute("minecraft:max_health").unwrap_or(20.0) as f32;
                let h = p.health.health();
                if h > 0.0 {
                    let _ = p.set_health((h + amount).min(max), false);
                }
            }
            EffectAction::Hurt(source, amount) => {
                let mut canceled = false;
                if let Some(id) = self.health_resources.damage.id(source)
                    && let Ok(o) = p.hurt_blocking(
                        crate::game::entity::damage_level_of!(
                            self,
                            self.combat.0.attack(),
                            no_push,
                            None,
                            acting_equipment!(self)
                        ),
                        &self.health_resources,
                        rules,
                        id,
                        amount,
                        None,
                    )
                {
                    canceled = o.canceled;
                }

                self.combat.0.forget_noted(canceled);
                self.finish_guest_damage();
            }
            EffectAction::Poison => {
                if p.health.health() > 1.0
                    && let Some(id) = self.health_resources.damage.id("minecraft:magic")
                {
                    let _ = p.hurt(
                        crate::game::entity::damage_level_of!(
                            self,
                            self.combat.0.attack(),
                            no_push,
                            None,
                            acting_equipment!(self)
                        ),
                        &self.health_resources,
                        rules,
                        id,
                        1.0,
                    );
                    self.finish_guest_damage();
                }
            }
            EffectAction::Exhaust(amount) => {
                p.cause_food_exhaustion(amount);
            }
            EffectAction::Eat(nutrition, saturation) => {
                let food = &mut p.health.food;
                food.food = (food.food + nutrition).clamp(0, 20);
                food.saturation = (food.saturation + nutrition as f32 * saturation * 2.0)
                    .clamp(0.0, food.food as f32);
            }
            EffectAction::Update(id, amplifier) => {
                self.update_player_effect_attributes(id, amplifier, true)
            }
            EffectAction::Remove(id) => self.remove_player_effect_attributes(&[id]),
        }
    }
    /// `HealOrHarmMobEffect.applyInstantaneousEffect` with a source (a
    /// potion or a cloud): indirectMagic, not plain magic, credited to the
    /// mob that threw it when there is one.
    pub(in crate::game::server_side) fn apply_indirect_effect_action(
        &mut self,
        a: EffectAction,
        owner: &PotionOwner,
    ) {
        let a = match a {
            EffectAction::Hurt("minecraft:magic", amount) => {
                match (owner.attacker, &owner.name) {
                    (Some(id), Some(name)) => {
                        self.note_named_attacker("minecraft:indirect_magic", id, name.clone())
                    }
                    _ => {
                        let name = owner.name.clone().unwrap_or_else(|| {
                            Text::translate("entity.minecraft.splash_potion", &[])
                        });
                        self.note_direct_entity("minecraft:indirect_magic", name);
                    }
                }
                EffectAction::Hurt("minecraft:indirect_magic", amount)
            }
            a => a,
        };
        self.apply_effect_action(a);
    }
    /// `Arrow.doPostHurtEffects` and `SpectralArrow.doPostHurtEffects`: the
    /// arrow's item gives its target Glowing (spectral) or its potion contents
    /// at the stack's duration scale (a tipped arrow's default is an eighth);
    /// a mob's arrow without such an item (a stray's) carries its own effect.
    /// Instant effects act at once: harming hurts and healing heals, the
    /// other way round for the undead.
    pub(in crate::game::server_side) fn arrow_post_hurt_effects(
        &mut self,
        p: &Projectile,
        who: Who,
    ) {
        let from_item = p.arrow.as_ref().map(|a| &a.pickup_item);
        let mut effects: Vec<(&'static str, i32, i32)> = match from_item {
            // SpectralArrow.doPostHurtEffects: Glowing for the arrow's own
            // (saved) Duration.
            Some(s) if &*s.item == "minecraft:spectral_arrow" => {
                let ticks = p.arrow.as_ref().map_or(200, |a| a.spectral_duration);
                vec![("glowing", ticks, 0)]
            }
            Some(s) => {
                let scale = s.potion_duration_scale();
                brewing::contents_effects(s)
                    .into_iter()
                    .map(|(id, d, amp)| {
                        (id, sunrise_client::effects::scaled_duration(d, scale), amp)
                    })
                    .collect()
            }
            None => Vec::new(),
        };
        if effects.is_empty()
            && let ProjectileKind::Arrow {
                effect: Some((id, ticks)),
                ..
            } = p.kind
            && let Some(e) = brewing::effect(id)
        {
            effects.push((e.id, ticks, 0));
        }
        if effects.is_empty() {
            return;
        }
        match who {
            Who::Player => {
                for (id, duration, amp) in effects {
                    if brewing::effect(id).is_some_and(|e| e.instant) {
                        if let Some(a) = sunrise_client::effects::instant(id, amp, 1.0) {
                            self.apply_effect_action(a);
                        }
                    } else {
                        self.add_player_effect(EffectInstance::new(id, duration, amp));
                    }
                }
            }
            Who::Mob(id) => {
                let Some(i) = self.mobs.iter().position(|m| m.id == id && m.alive()) else {
                    return;
                };
                let inverted = self.items().entity_type_tags.has_bit(
                    self.mobs[i].kind.flags.entity_tag_slot,
                    sunrise_client::items::EntityTypeTags::INVERTED_HEALING_AND_HARM,
                );
                for (effect, duration, amp) in effects {
                    // A mod's `onEffectAdded` may have changed the mobs.
                    let Some(index) = self.mob_index.find(&self.mobs, id) else {
                        return;
                    };
                    let m = &mut self.mobs[index];
                    match effect {
                        "instant_damage" | "instant_health" => {
                            // `LivingEntity.addEffect` refuses nothing here; the
                            // tick hurts with magic (6 << amp) or heals (4 << amp).
                            if (effect == "instant_damage") != inverted {
                                m.damage(
                                    crate::game::entity::damage_level_of!(
                                        self,
                                        self.combat.0.attack(),
                                        no_push,
                                        None
                                    ),
                                    (6 << amp.min(30)) as f32,
                                    Damage::of(DamageKind::Magic),
                                );
                                self.finish_mob_damage_at(index);
                            } else {
                                m.heal(&self.loop_context, (4 << amp.min(30)) as f32);
                            }
                        }
                        _ => {
                            // `LivingEntity.addEffect` refuses what the mob
                            // cannot be affected by: the borrow ends before
                            // the hook asks, and the mob is found again.
                            let kind = m.kind.id.clone();
                            if self.can_be_affected(Some(id as i32), &kind, effect)
                                && let Some(name) = mob_effect_id(effect)
                            {
                                let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) else {
                                    return;
                                };
                                let new = !m.has_effect(name);
                                m.add_effect(name, duration, amp);
                                self.mod_effect_added(name, Some(id as i32), amp, new);
                            }
                        }
                    }
                }
            }
        }
    }
    /// The dragon's breath (`AreaEffectCloud` with instant damage at
    /// `amplifier`) on a mob: what a cloud puts on it, `indirectMagic(cloud,
    /// dragon)` with no owner to credit (the dragon is not a mob here).
    pub(in crate::game) fn breath_affect_mob(&mut self, mob: u32, amplifier: i32, at: [f64; 3]) {
        let source = Text::translate("entity.minecraft.area_effect_cloud", &[]);
        let effect = [("instant_damage", 1, amplifier)];
        // DragonFireball.onHit and the flame phase: `setPotionDurationScale(0.25)`.
        self.cloud_affect(
            Victim::Mob(mob),
            &effect,
            (&[], &[], 0.25),
            at,
            &source,
            (None, None),
        );
    }
    /// What a cloud puts on one entity: instant effects at half strength
    /// (`applyInstantaneousEffect` with 0.5), the others at the cloud's duration
    /// scale (`PotionContents.forEachEffect`).
    pub(in crate::game) fn cloud_affect(
        &mut self,
        victim: Victim,
        effects: &[(&'static str, i32, i32)],
        (hidden, ambient, duration_scale): (&[&'static str], &[&'static str], f32),
        at: [f64; 3],
        source: &Text,
        (owner, guest): (Option<Who>, Option<u32>),
    ) {
        // `indirectMagic(cloud, owner)` names the cloud in the death message
        // (it gives no kill credit).
        let cloud_owner = PotionOwner {
            name: Some(source.clone()),
            ..Default::default()
        };
        // MobEffectInstance.withScaledDuration: floor(duration * scale), at least 1; an infinite
        // effect stays infinite.
        let scaled = |duration| sunrise_client::effects::scaled_duration(duration, duration_scale);
        match victim {
            Victim::Player(who) => self.as_fight_player(who, |g| {
                for (id, duration, amp) in effects {
                    let Some(e) = brewing::effect(id) else {
                        continue;
                    };
                    if e.instant {
                        if let Some(a) = sunrise_client::effects::instant(id, *amp, 0.5) {
                            g.apply_indirect_effect_action(a, &cloud_owner);
                        }
                    } else {
                        let mut instance = EffectInstance::new(e.id, scaled(*duration), *amp);
                        instance.visible = !hidden.contains(id);
                        instance.ambient = ambient.contains(id);
                        g.add_player_effect(instance);
                    }
                }
            }),
            Victim::Mob(mob) => {
                let Some(i) = self.mobs.iter().position(|m| m.id == mob) else {
                    return;
                };
                // Undead invert healing and harming.
                let inverted = self.items().entity_type_tags.has_bit(
                    self.mobs[i].kind.flags.entity_tag_slot,
                    sunrise_client::items::EntityTypeTags::INVERTED_HEALING_AND_HARM,
                );
                for (id, duration, amp) in effects {
                    // A mod's `onEffectAdded` may have changed the mobs.
                    let Some(m) = self.mobs.iter_mut().find(|m| m.id == mob) else {
                        return;
                    };
                    match *id {
                        "instant_damage" | "instant_health" => {
                            let amount = |base: i32| {
                                (0.5 * f64::from(base.wrapping_shl(*amp as u32)) + 0.5) as i32
                                    as f32
                            };
                            if (*id == "instant_damage") != inverted {
                                // HealOrHarmMobEffect.applyInstantaneousEffect: indirectMagic(cloud, owner).
                                let source = Damage::indirect_magic(owner, at);
                                let attack = self.potion_attack(owner, guest);
                                let plan =
                                    self.prepare_knockback_credit(mob as i32, owner, &attack);
                                let mut credit = plan.lend(&mut self.server.last_hurt_by_player);
                                let Some(index) = self.mob_index.find(&self.mobs, mob) else {
                                    return;
                                };
                                self.mobs[index].damage_by(
                                    crate::game::entity::damage_level_of!(
                                        self,
                                        &attack,
                                        &mut credit,
                                        None
                                    ),
                                    amount(6),
                                    source,
                                    "minecraft:area_effect_cloud",
                                );
                                self.finish_mob_damage_at(index);
                            } else {
                                m.heal(&self.loop_context, amount(4));
                            }
                        }
                        _ => {
                            // `LivingEntity.addEffect` refuses what the mob
                            // cannot be affected by: the borrow ends before
                            // the hook asks, and the mob is found again.
                            let kind = m.kind.id.clone();
                            if self.can_be_affected(Some(mob as i32), &kind, id)
                                && let Some(name) = mob_effect_id(id)
                            {
                                let Some(m) = self.mobs.iter_mut().find(|m| m.id == mob) else {
                                    return;
                                };
                                let new = !m.has_effect(name);
                                m.add_effect_with(
                                    name,
                                    scaled(*duration),
                                    *amp,
                                    ambient.contains(id),
                                    !hidden.contains(id),
                                );
                                self.mod_effect_added(name, Some(mob as i32), *amp, new);
                            }
                        }
                    }
                }
            }
        }
    }
    /// `AreaEffectCloud.serverTick`: configured lifetime/radius changes and
    /// five-tick doses, with each victim's configured reapplication delay.
    /// False once the cloud is gone.
    /// `Entity.baseTick`'s fluid steps for a cloud: `updateFluidInteraction`
    /// (the box is `radius * 2` wide and 0.5 high) resets the fall distance
    /// in water and adds the currents to its motion, then lava halves it once
    /// `first_tick` has cleared (`Entity.isInLava` is false on the first tick).
    pub(in crate::game) fn cloud_fluid_step(&self, c: &mut Breath) {
        let (r, [x, y, z]) = (f64::from(c.radius), c.at);
        let bounds = sunrise_core::geometry::Aabb::new(
            Vec3::new(x - r, y, z - r),
            Vec3::new(x + r, y + 0.5, z + r),
        );
        let mut motion = Vec3::new(c.motion[0], c.motion[1], c.motion[2]);
        // EntityDimensions.scalable: the eye at 0.85 of the height.
        let eye = y + f64::from(0.5_f32 * 0.85);
        let (water, lava, _) = sunrise_client::entities::update_entity_fluids(
            bounds,
            Vec3::new(x, y, z),
            eye,
            &mut motion,
            &self.world,
            &self.res.motion().fluids,
            &self.loop_context,
        );
        c.motion = [motion.x, motion.y, motion.z];
        if water > 0.0 {
            c.fall_distance = 0.0;
        }
        if !c.first_tick && lava > 0.0 {
            c.fall_distance *= 0.5;
        }
        c.first_tick = false;
    }
    /// `LivingEntity.triggerOnDeathMobEffects` (`onMobRemoved` with the reason
    /// KILLED) for what Oozing, Weaving and Wind Charged do when their bearer
    /// dies. `bearer` is the dying mob's id, none when a player dies; `silent`
    /// whether that source is heard, and `random` the dying mob's own
    /// (`mob.getRandom()`; the world's for the player).
    pub(in crate::game) fn effects_on_death(
        &mut self,
        effects: &[&str],
        bounds: Aabb,
        silent: bool,
        bearer: Option<u32>,
        mut random: Option<&mut LegacyRandom>,
    ) {
        let player = bearer.is_none();
        let feet = bounds.bottom_center();
        if effects.contains(&"oozing") {
            // OozingMobEffect.numberOfSlimesToSpawn: two, fewer when the
            // slimes close by already fill the cramming limit.
            let limit = self
                .ui
                .rules
                .int_at(sunrise_client::gamerules::vanilla::MAX_ENTITY_CRAMMING);
            let mut count = 2;
            if limit >= 1 {
                let near = bounds.inflate(Vec3::new(2.0, 2.0, 2.0));
                let nearby = self
                    .mobs
                    .iter()
                    .filter(|m| {
                        m.species() == sunrise_client::mobs::Species::Slime
                            && Some(m.id) != bearer
                            && m.bounds().intersects(near)
                    })
                    .count()
                    .min(limit as usize) as i32;
                count = (limit - nearby).clamp(0, count);
            }
            for _ in 0..count {
                // spawnSlimeOffspring: a size 2 slime half a block up.
                if let Some(id) = self.spawn_mob(
                    "minecraft:slime",
                    [feet.x, feet.y + 0.5, feet.z],
                    false,
                    super::creatures::SpawnSite::Plain,
                ) && let Some(m) = self.mobs.iter_mut().find(|m| m.id == id)
                {
                    sunrise_client::mobs::cube::set_size(m, 2);
                }
            }
        }
        // `WeavingMobEffect.onMobRemoved`: `canEntityGrief` for the dying mob.
        if effects.contains(&"weaving") && (player || self.can_entity_grief(bearer)) {
            self.spawn_cobwebs_around(
                [
                    feet.x.floor() as i32,
                    feet.y.floor() as i32,
                    feet.z.floor() as i32,
                ],
                random.as_deref_mut(),
            );
        }
        if effects.contains(&"wind_charged") {
            // A burst centred at half the height, radius 3 to 5.
            let center = [feet.x, feet.y + bounds.size().y / 2.0, feet.z];
            let r = random.unwrap_or(&mut self.world.random);
            let radius = 3.0 + r.next_float() * 2.0;
            // WindChargedMobEffect: the bearer bursts, a living source.
            let bearer = if player {
                Some(self.own_entity_id() as u32)
            } else {
                bearer
            };
            let cause = super::wind::BurstCause {
                source: bearer,
                owner: bearer,
            };
            let _ = self.wind_burst(
                center,
                radius,
                1.0,
                super::wind::Source::wind_charged(player, silent).with_cause(cause),
            );
        }
    }
    /// `ItemUtils.createFilledResult`: replace one of the held stack.
    pub(in crate::game) fn fill_held(&mut self, filled: ItemStack) {
        if self.creative {
            let _ = self.add_stack_to_inventory(filled);
            return;
        }
        self.consume_held();
        if self.held().is_none() {
            *self.held_mut() = Some(filled);
        } else if let Some(rest) = self.add_stack_to_inventory(filled)
            && let Some(eye) = self.eye_position()
        {
            // `player.drop(stack, false)`.
            self.throw_from_eyes(rest, eye, false, false);
        }
    }
    /// `PotionItem.finishUsingItem` (and `OminousBottleItem`'s) of `stack` in
    /// `slot`, the use complete: false for anything else.
    pub(in crate::game::server_side) fn finish_drinking(
        &mut self,
        slot: usize,
        stack: &ItemStack,
    ) -> bool {
        let ominous = &*stack.item == "minecraft:ominous_bottle";
        if &*stack.item != "minecraft:potion" && !ominous {
            return false;
        }
        let eye = self.eye_position().unwrap_or_default();
        self.using = None;
        self.send_use_item_complete();
        // Consumable.onConsume: the used statistic and the consume_item
        // trigger, with the stack as drunk (before it shrinks).
        self.award_stat("used", &stack.item, 1);
        self.trigger_item("minecraft:consume_item", stack);
        if ominous {
            self.drink_ominous_bottle(stack);
        } else {
            // PotionContents.onConsume: the potion's and the custom effects,
            // at the stack's POTION_DURATION_SCALE.
            let (lasting, now) = PlayerEffects::contents_effects(
                &brewing::contents_effects(stack),
                stack.potion_duration_scale(),
            );
            for e in lasting {
                self.add_player_effect(e);
            }
            for a in now {
                self.apply_effect_action(a);
            }
        }
        self.apply_drink_effects(stack);
        self.post_entity_event("minecraft:drink", sunrise_client::mobs::Who::Player);
        if !self.creative && self.stack_in(slot) == Some(stack) {
            self.consume_player_slot(slot, 1);
            if !ominous {
                let bottle = ItemStack::new("minecraft:glass_bottle", 1);
                if self.stack_in(slot).is_none() {
                    *self.stack_in_mut(slot) = Some(bottle);
                } else if let Some(rest) = self.add_stack_to_inventory(bottle) {
                    self.spawn_item(rest, eye, [0.0, 0.1, 0.0], 40);
                }
            }
        }
        true
    }
    /// The attack of a potion's or cloud's hit: its thrower and what they hold, for
    /// `living_knockback`.
    pub(in crate::game::server_side) fn potion_attack(
        &self,
        owner: Option<Who>,
        guest: Option<u32>,
    ) -> Attack {
        match owner {
            Some(Who::Mob(id)) => self.attack_of(id as i32),
            Some(Who::Player) => self.attack_of(
                self.player_entity_id_by_connection(guest)
                    .unwrap_or_default(),
            ),
            None => Attack::default(),
        }
    }
    /// `Projectile.getOwner` of a thrown potion, as [`splash`](Self::splash)
    /// names it: a player's, a mob's (a witch keeps the kill credit), or
    /// nobody's (a dispenser's, a summoned potion's).
    pub(in crate::game) fn potion_owner(&self, owner: Option<Who>) -> PotionOwner {
        match owner {
            Some(Who::Player) => PotionOwner {
                name: Some(self.player_display_text()),
                attacker: None,
                player: true,
                guest: self.server.lan_acting,
            },
            Some(Who::Mob(id)) => PotionOwner {
                name: self.mob_display_name(id),
                attacker: Some(id),
                player: false,
                guest: None,
            },
            None => PotionOwner::default(),
        }
    }
    /// Effects saved in level.dat (`Player.active_effects`).
    pub(in crate::game) fn restore_effects(&mut self) {
        self.potions = PotionState::default();
        self.potions.effects = PlayerEffects::from_saved(&self.level.active_effects);
    }
    /// `WeavingMobEffect.spawnCobwebsRandomlyAround`: two or three cobwebs in
    /// free spaces with a solid floor, picked among 15 random blocks within one
    /// block of `pos`.
    pub(in crate::game::server_side) fn spawn_cobwebs_around(
        &mut self,
        pos: [i32; 3],
        mut random: Option<&mut LegacyRandom>,
    ) {
        let r = random.as_deref_mut().unwrap_or(&mut self.world.random);
        let wanted = 2 + r.next_int_bound(2).unwrap_or(0) as usize;
        let mut chosen: Vec<[i32; 3]> = Vec::new();
        for _ in 0..15 {
            let r = random.as_deref_mut().unwrap_or(&mut self.world.random);
            let at = [
                pos[0] - 1 + r.next_int_bound(3).unwrap_or(0),
                pos[1] - 1 + r.next_int_bound(3).unwrap_or(0),
                pos[2] - 1 + r.next_int_bound(3).unwrap_or(0),
            ];
            let facts = self.res.facts();
            let here = facts.get(self.world.block(at[0], at[1], at[2]));
            let below = facts.get(self.world.block(at[0], at[1] - 1, at[2]));
            if !chosen.contains(&at)
                && here.replaceable
                && below.sturdy_full & (1 << Direction::Up as u8) != 0
            {
                chosen.push(at);
                if chosen.len() >= wanted {
                    break;
                }
            }
        }
        for at in chosen {
            if self.set_state_named(at, "minecraft:cobweb", &[]).is_ok() {
                self.server_level_event_at(3018, at, 0);
            }
        }
    }
    /// `Creeper.spawnLingeringCloud`: an `AreaEffectCloud` of radius 2.5, wait time 10, duration 300,
    /// shrinking by `-radius / duration` a tick and by 0.5 for each entity it affects, applying
    /// the creeper's effects at 0.25 of their remaining duration.
    pub(in crate::game) fn spawn_creeper_cloud(
        &mut self,
        at: [f64; 3],
        effects: Vec<(&'static str, i32, i32)>,
        hidden: Vec<&'static str>,
        ambient: Vec<&'static str>,
        color: u32,
    ) {
        let id = self.take_entity_id();
        self.breath_clouds.push(Breath {
            attachment_lifetime: Default::default(),
            join: Default::default(),
            id,
            at,
            motion: [0.0; 3],
            fall_distance: 0.0,
            fire_ticks: 0,
            first_tick: true,
            radius: 2.5,
            age: 0,
            duration: 300,
            from_dragon: false,
            victims: Vec::new(),
            potion: Some(PotionCloud {
                settings: CloudSettings::lingering(),
                effects,
                hidden,
                ambient,
                radius_per_tick: -2.5 / 300.0,
                color,
                custom_particle: None,
                victims: Vec::new(),
                source: Text::translate("entity.minecraft.area_effect_cloud", &[]),
                owner: None,
                guest: None,
            }),
        });
    }
    /// `AbstractThrownPotion.onHit` with `ThrownSplashPotion.onHitAsPotion`, or
    /// `ThrownLingeringPotion.onHitAsPotion` for a lingering potion, read from
    /// the stack's `potion_contents`; `mob` is where it hit a mob, `hit` the
    /// block and face it hit, `age` its age, `silent` its Silent flag, `motion` its motion as it hit.
    pub(in crate::game) fn splash(
        &mut self,
        stack: &ItemStack,
        at: Vec3,
        mob: Option<Vec3>,
        hit: Option<([i32; 3], Direction)>,
        age: u32,
        owner: &PotionOwner,
        silent: bool,
        motion: Vec3,
    ) {
        let block = [at.x, at.y, at.z].map(|v| v.floor() as i32);
        if &*stack.item == "minecraft:experience_bottle" {
            // ThrownExperienceBottle.onHit: the water-coloured splash (level
            // event 2002, its sound 1053) and 3 to 11 experience.
            self.server_level_event_at(2002, block, 0xFF38_5DC6u32 as i32);
            if !silent {
                self.server_level_event_at(1053, block, 0);
            }
            let r = &mut self.world.random;
            let value = 3 + r.next_int_bound(5).unwrap_or(0) + r.next_int_bound(5).unwrap_or(0);
            // awardWithDirection: away from the face it hit, or back along
            // its own motion when it hit an entity (#11388).
            let roughly = match hit {
                Some((_, face)) => face.normal().map(f64::from),
                None => [-motion.x, -motion.y, -motion.z],
            };
            self.spawn_orbs_toward([at.x, at.y, at.z], roughly, value as u32);
            self.native_exp_bottle([at.x, at.y, at.z], value as u32);
            return;
        }
        // Projectile.onHit calls onHitBlock first, then the potion's
        // affectEntitiesAround; both read the stack's `potion_contents`.
        if let Some((cell, face)) = hit {
            // `AbstractThrownPotion.douseFire` passes `getOwner()` to the
            // campfire's `douse`, which is where the game event's source is.
            let who = if owner.player {
                Some(Who::Player)
            } else {
                owner.attacker.map(Who::Mob)
            };
            self.potion_hit_block(stack, cell, face, who);
        }
        self.affect_entities_around(stack, at, owner.player);
        let lingering = &*stack.item == "minecraft:lingering_potion";
        // AbstractThrownPotion.onHit: level event 2007 and sound 1054 for a
        // potion with instant effects, 2002 and 1053 otherwise.
        let potion = stack.potion().and_then(brewing::potion);
        let instant = potion.is_some_and(|d| {
            d.effects
                .iter()
                .any(|(id, ..)| brewing::effect(id).is_some_and(|e| e.instant))
        });
        // `PotionContents.getColor`: an explicit `custom_color` wins over the
        // amplifier-weighted mix of the potion's and the stack's custom effects.
        let color = brewing::contents_color(stack);
        self.server_level_event_at(if instant { 2007 } else { 2002 }, block, color as i32);
        if !silent {
            self.server_level_event_at(if instant { 1054 } else { 1053 }, block, 0);
        }
        // `PotionContents.hasEffects`: the potion's effects and the stack's
        // custom ones; without either `onHitAsPotion` is not called at all.
        let effects = brewing::contents_effects(stack);
        if effects.is_empty() {
            return;
        }
        if lingering {
            // The cloud sits at the mob it hit (its feet), else the impact;
            // radius 3, wait 10, 600 ticks, shrinking to nothing, with the
            // stack's `potion_contents` and `potion_duration_scale`.
            let at = mob.unwrap_or(at);
            let id = self.take_entity_id();
            self.breath_clouds.push(Breath {
                attachment_lifetime: Default::default(),
                join: Default::default(),
                id,
                at: [at.x, at.y, at.z],
                motion: [0.0; 3],
                fall_distance: 0.0,
                fire_ticks: 0,
                first_tick: true,
                radius: 3.0,
                age: 0,
                duration: 600,
                from_dragon: false,
                victims: Vec::new(),
                potion: Some(PotionCloud {
                    settings: CloudSettings {
                        duration_scale: stack.potion_duration_scale(),
                        ..CloudSettings::lingering()
                    },
                    effects,
                    hidden: Vec::new(),
                    ambient: Vec::new(),
                    radius_per_tick: -3.0 / 600.0,
                    color,
                    custom_particle: None,
                    victims: Vec::new(),
                    source: owner.name.clone().unwrap_or_else(|| {
                        Text::translate("entity.minecraft.area_effect_cloud", &[])
                    }),
                    owner: owner.who(),
                    guest: owner.guest,
                }),
            });
            return;
        }
        let duration_scale = f64::from(stack.potion_duration_scale());
        // Every player in range (`ThrownSplashPotion.applySplash` over
        // `LivingEntity`s, the players among them).
        self.for_each_player(|g| g.splash_on_player(&effects, at, age, owner, duration_scale));
        // Mobs take the instant effects (undead invert healing and harming).
        let undead = &self.items().entity_type_tags;
        let mut hits = Vec::new();
        for (i, m) in self.mobs.iter().enumerate() {
            if m.alive()
                && let Some(d2) = splash_distance_sqr(at, age, m.bounds())
            {
                let inverted = undead.has_bit(
                    m.kind.flags.entity_tag_slot,
                    sunrise_client::items::EntityTypeTags::INVERTED_HEALING_AND_HARM,
                );
                hits.push((i, 1.0 - d2.sqrt() / 4.0, inverted));
            }
        }
        for (i, intensity, inverted) in hits {
            for (id, duration, amp) in &effects {
                let harm = match *id {
                    "instant_damage" => !inverted,
                    "instant_health" => inverted,
                    _ => {
                        // `Potion.applySplash`: timed effects last in
                        // proportion to how close the mob was.
                        let d = (intensity * f64::from(*duration) * duration_scale + 0.5) as i32;
                        if d > 20
                            && let Some(name) = mob_effect_id(id)
                        {
                            self.mobs[i].add_effect(name, d, *amp);
                        }
                        continue;
                    }
                };
                let m = &mut self.mobs[i];
                if harm && m.species() == sunrise_client::mobs::Species::Enderman {
                    // `Enderman.hurtServer` for a thrown potion: only water hurts
                    // (`hurtWithCleanWater`), then `repeatedlyTryToTeleport`, which
                    // the AI step runs and which does nothing while it rides.
                    if m.alive() && !m.extra.passenger {
                        m.extra.counter2 = 64;
                    }
                } else if harm {
                    // HealOrHarmMobEffect.applyInstantaneousEffect: indirectMagic(potion, owner).
                    let harm = (intensity * f64::from(6 << amp) + 0.5) as i32 as f32;
                    let source = Damage::indirect_magic(owner.who(), [at.x, at.y, at.z]);
                    let id = m.id;
                    let mut attack = self.potion_attack(owner.who(), owner.guest);
                    attack.projectile = self.combat.0.attack().projectile;
                    attack.projectile_kind = self.combat.0.attack().projectile_kind;
                    attack.projectile_type = self.combat.0.attack().projectile_type;
                    let plan = self.prepare_knockback_credit(id as i32, owner.who(), &attack);
                    let mut credit = plan.lend(&mut self.server.last_hurt_by_player);
                    self.mobs[i].damage_by(
                        crate::game::entity::damage_level_of!(self, &attack, &mut credit, None),
                        harm,
                        source,
                        "minecraft:splash_potion",
                    );
                    self.finish_mob_damage_at(i);
                } else {
                    m.heal(
                        &self.loop_context,
                        (intensity * f64::from(4 << amp) + 0.5) as i32 as f32,
                    );
                }
            }
        }
    }
    /// A splash potion's effects on the player in place, by their distance,
    /// for the stack's `potion_duration_scale`.
    pub(in crate::game::server_side) fn splash_on_player(
        &mut self,
        effects: &[(&'static str, i32, i32)],
        at: Vec3,
        age: u32,
        owner: &PotionOwner,
        duration_scale: f64,
    ) {
        if let Some(p) = self.player.as_ref() {
            let w = f64::from(p.dimensions.width) / 2.0;
            let bounds = Aabb::new(
                Vec3::new(p.position.x - w, p.position.y, p.position.z - w),
                Vec3::new(
                    p.position.x + w,
                    p.position.y + f64::from(p.dimensions.height),
                    p.position.z + w,
                ),
            );
            if let Some(d2) = splash_distance_sqr(at, age, bounds) {
                let intensity = 1.0 - d2.sqrt() / 4.0;
                for (id, duration, amp) in effects {
                    let instant = brewing::effect(id).is_some_and(|e| e.instant);
                    if instant {
                        if let Some(a) = sunrise_client::effects::instant(id, *amp, intensity) {
                            self.apply_indirect_effect_action(a, owner);
                        }
                    } else {
                        let d = (intensity * f64::from(*duration) * duration_scale + 0.5) as i32;
                        if d > 20
                            && let Some(e) = brewing::effect(id)
                        {
                            self.add_player_effect(EffectInstance::new(e.id, d, *amp));
                        }
                    }
                }
            }
        }
    }
    /// Drinking a potion while right-click is held; true when handled.
    pub(in crate::game::server_side) fn tick_drinking(
        &mut self,
        slot: usize,
        ticks: u32,
        stack: &ItemStack,
    ) -> bool {
        let ominous = &*stack.item == "minecraft:ominous_bottle";
        if &*stack.item != "minecraft:potion" && !ominous {
            return false;
        }
        // the duration the use started with (32, as a mod's Start or Tick left it).
        let duration = self.use_duration;
        let remaining = duration.saturating_sub_unsigned(ticks);
        let ticks = ticks + 1;
        let eye = self.eye_position().unwrap_or_default();
        // Consumable.emitParticlesAndSounds: the drink sound every 4 ticks near the end.
        if remaining > 0 && remaining <= 25 && remaining % 4 == 0 {
            let pitch = self.world.random.next_float() * 0.1 + 0.9;
            self.server_play_sound("minecraft:entity.generic.drink", eye, 0.5, pitch);
        }
        // ItemStack.onUseTick: the item's own tick after the consumable's.
        if !self.item_on_use_tick(stack, ticks - 1) {
            return true;
        }
        if i64::from(ticks) < i64::from(duration) {
            self.using = Some((slot, ticks));
            return true;
        }
        self.finish_drinking(slot, stack);
        self.use_item_finished(stack);
        true
    }
    pub(in crate::game) fn tick_potion_cloud(
        &mut self,
        c: &mut Breath,
        players: &[FightPlayer],
    ) -> bool {
        c.age = c.age.wrapping_add(1);
        let Some(potion) = c.potion.as_mut() else {
            return true;
        };
        if c.duration != -1 && c.age.wrapping_sub(potion.settings.wait_time) >= c.duration {
            return false;
        }
        if c.age < potion.settings.wait_time {
            return true;
        }
        if potion.radius_per_tick != 0.0 {
            c.radius += potion.radius_per_tick;
            if c.radius < 0.5 {
                return false;
            }
            c.radius = c.radius.clamp(0.0, 32.0);
        }
        if c.age % 5 != 0 {
            return true;
        }
        potion.victims.retain(|(_, until)| c.age < *until);
        if potion.effects.is_empty() {
            potion.victims.clear();
            return true;
        }
        let r = f64::from(c.radius);
        let area = Aabb::new(
            Vec3::new(c.at[0] - r, c.at[1], c.at[2] - r),
            Vec3::new(c.at[0] + r, c.at[1] + 0.5, c.at[2] + r),
        );
        let mut inside = Vec::new();
        for p in players.iter().filter(|p| p.bounds.intersects(area)) {
            inside.push((Victim::Player(p.who), p.position.x, p.position.z));
        }
        let effects = potion.effects.clone();
        // Only mobs that at least one of the cloud's effects can affect are
        // taken (`allEffects.stream().anyMatch(entity::canBeAffected)`), so
        // the others neither become victims nor use up its radius.
        let candidates: Vec<(u32, String, f64, f64)> = self
            .mobs
            .iter()
            .filter(|m| m.alive() && m.bounds().intersects(area))
            .map(|m| (m.id, m.kind.id.clone(), m.position.x, m.position.z))
            .collect();
        for (mob_id, kind, x, z) in candidates {
            let affected = effects
                .iter()
                .any(|(id, ..)| self.can_be_affected(Some(mob_id as i32), &kind, id));
            if affected {
                inside.push((Victim::Mob(mob_id), x, z));
            }
        }
        for (victim, x, z) in inside {
            let (dx, dz) = (x - c.at[0], z - c.at[2]);
            if potion.victims.iter().any(|(v, _)| *v == victim)
                || dx * dx + dz * dz > f64::from(c.radius * c.radius)
            {
                continue;
            }
            potion.victims.push((
                victim,
                c.age.wrapping_add(potion.settings.reapplication_delay),
            ));
            self.cloud_affect(
                victim,
                &effects,
                (
                    &potion.hidden,
                    &potion.ambient,
                    potion.settings.duration_scale,
                ),
                c.at,
                &potion.source,
                (potion.owner, potion.guest),
            );
            if potion.settings.radius_on_use != 0.0 {
                c.radius += potion.settings.radius_on_use;
                if c.radius < 0.5 {
                    return false;
                }
                c.radius = c.radius.clamp(0.0, 32.0);
            }
            if potion.settings.duration_on_use != 0 && c.duration != -1 {
                c.duration = c.duration.wrapping_add(potion.settings.duration_on_use);
                if c.duration <= 0 {
                    return false;
                }
            }
        }
        true
    }
    /// The authoritative `AreaEffectCloud.serverTick` for lingering potions' clouds.
    /// The logical client draws particles from tracked cloud data.
    pub(in crate::game) fn tick_potion_clouds(&mut self) {
        if self.breath_clouds.iter().all(|c| c.potion.is_none()) {
            return;
        }
        let players: Vec<FightPlayer> = self
            .fight_players()
            .into_iter()
            .filter(|p| p.alive)
            .collect();
        let mut clouds = self.breath_clouds.lift();
        #[cfg(test)]
        super::entity::taken_pass_tests::after_take(self);
        let ticking = self.entity_ticking_area();
        let mut expired: Vec<u32> = Vec::new();
        clouds.retain_mut(|c| {
            let keep =
                c.potion.is_none() || !ticking(c.at[0].floor() as i32, c.at[2].floor() as i32) || {
                    self.cloud_fluid_step(c);
                    // Entity.baseTick: `fireImmune()` clears the fire.
                    c.fire_ticks = c.fire_ticks.min(0);
                    self.tick_potion_cloud(c, &players)
                };
            if !keep {
                expired.push(c.id);
            }
            keep
        });
        self.breath_clouds
            .restore(clouds, sunrise_client::join_seq::RestoreOrder::SpawnsFirst);
        // A cloud that expired on its own takes its `noPhysics` write with it.
        for id in expired {
            self.forget_entity(id);
        }
    }
    /// Right-click with a potion, splash/lingering potion or glass bottle.
    pub(in crate::game::server_side) fn use_potion_item(
        &mut self,
        stack: &ItemStack,
    ) -> sunrise_client::Result<bool> {
        match &*stack.item {
            "minecraft:potion" | "minecraft:ominous_bottle" => {
                self.start_using_item();
                Ok(true)
            }
            "minecraft:splash_potion"
            | "minecraft:lingering_potion"
            | "minecraft:experience_bottle" => {
                let Some(eye) = self.eye_position() else {
                    return Ok(true);
                };
                let pitch = 0.4 / (self.world.random.next_float() * 0.4 + 0.8);
                let (sound, power) = match &*stack.item {
                    "minecraft:splash_potion" => ("minecraft:entity.splash_potion.throw", 0.5),
                    "minecraft:experience_bottle" => {
                        ("minecraft:entity.experience_bottle.throw", 0.7)
                    }
                    _ => ("minecraft:entity.lingering_potion.throw", 0.5),
                };
                self.server_play_sound(sound, eye, 0.5, pitch);
                // Projectile.shootFromRotation(xRot, yRot, -20, power, 1.0), with
                // what the thrower last moved (getKnownMovement).
                let velocity = self.known_player_movement();
                let on_ground = self.player.as_ref().is_some_and(|p| p.ground.on_ground);
                let one = ItemStack {
                    count: 1,
                    ..stack.clone()
                };
                let mut shot =
                    thrown_projectile(one, Vec3::new(eye[0], eye[1] - 0.1, eye[2]), true);
                shot.shoot_from_rotation(
                    self.pitch, self.yaw, -20.0, power, 1.0, velocity, on_ground,
                );
                shot.guest = self.server.lan_acting;
                self.add_projectile(shot);
                // ThrowablePotionItem.use / ExperienceBottleItem.use: awardStat(ITEM_USED).
                self.award_stat("used", &stack.item, 1);
                self.swing_arm();
                self.consume_held();
                Ok(true)
            }
            "minecraft:glass_bottle" => {
                // `BottleItem.use`: before the water check, it looks for a living
                // AreaEffectCloud owned by an EnderDragon within the player's box
                // grown by 2. If one is found, it shrinks the cloud's radius by 0.5,
                // plays item.bottle.fill_dragonbreath at the player, emits
                // GameEvent.FLUID_PICKUP, fires PLAYER_INTERACTED_WITH_ENTITY on the
                // cloud, and turns one bottle into dragon_breath.
                if let Some(player) = self.player.as_ref()
                    && let Some(cloud_at) =
                        collect_dragon_breath(player.bounds(), &mut self.breath_clouds)
                {
                    let player_pos = [player.position.x, player.position.y, player.position.z];
                    self.server_play_sound(
                        "minecraft:item.bottle.fill_dragonbreath",
                        player_pos,
                        1.0,
                        1.0,
                    );
                    self.post_entity_event("minecraft:fluid_pickup", Who::Player);
                    let cloud = sunrise_client::advancements::EntityView {
                        kind: "minecraft:area_effect_cloud".into(),
                        position: cloud_at,
                        ..Default::default()
                    };
                    self.trigger_item_entity(
                        "minecraft:player_interacted_with_entity",
                        stack,
                        &cloud,
                    );
                    // BottleItem.turnBottleIntoItem.
                    self.award_stat("used", &stack.item, 1);
                    self.fill_held(ItemStack::new("minecraft:dragon_breath", 1));
                    return Ok(true);
                }
                let Some(hit) = self.pick_fluid() else {
                    return Ok(false);
                };
                let state = self.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
                if self.res.facts().get(state).fluid != sunrise_client::blocks::Fluid::Water {
                    return Ok(false);
                }
                // The sound at the player, the game event at the water block.
                let Some(at) = self
                    .player
                    .as_ref()
                    .map(|p| [p.position.x, p.position.y, p.position.z])
                else {
                    return Ok(false);
                };
                self.server_play_sound("minecraft:item.bottle.fill", at, 1.0, 1.0);
                self.post_block_game_event(
                    "minecraft:fluid_pickup",
                    hit.pos,
                    Some(Who::Player),
                    None,
                );
                // BottleItem.turnBottleIntoItem.
                self.award_stat("used", &stack.item, 1);
                let water = ItemStack::new("minecraft:potion", 1).with_potion("minecraft:water");
                self.fill_held(water);
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    /// `PotionItem.useOn`: a water bottle used on a block that turns to mud
    /// (`#convertible_to_mud`), not on its underside, is poured out and the
    /// block becomes mud.
    pub(in crate::game::server_side) fn use_water_bottle_on_block(
        &mut self,
        pos: [i32; 3],
        face: [i32; 3],
        stack: &ItemStack,
    ) -> sunrise_client::Result<bool> {
        if &*stack.item != "minecraft:potion"
            || stack.potion() != Some("minecraft:water")
            || face == [0, -1, 0]
            || !self.named_block_in_tag("minecraft:convertible_to_mud", &self.block_name(pos))
        {
            return Ok(false);
        }
        let centre = pos.map(|v| f64::from(v) + 0.5);
        self.server_play_sound("minecraft:entity.generic.splash", centre, 1.0, 1.0);
        self.fill_held(ItemStack::new("minecraft:glass_bottle", 1));
        // PotionItem.useOn: five ServerLevel.sendParticles(SPLASH). The level
        // random places each one (count 1, no spread, speed 1).
        for _ in 0..5 {
            let r = &mut self.world.random;
            let at = [
                f64::from(pos[0]) + r.next_double(),
                f64::from(pos[1] + 1),
                f64::from(pos[2]) + r.next_double(),
            ];
            self.server_send_particles(crate::lan::server_effects::ServerQueuedParticle {
                dimension: self.dim.id(),
                particle: crate::lan::server_effects::ServerParticle::simple(ParticleType::Splash),
                at,
                spread: [0.0; 3],
                speed: [1.0; 3],
                count: 1,
                randomization: 0,
                except: None,
            });
        }
        self.server_play_sound("minecraft:item.bottle.empty", centre, 1.0, 1.0);
        self.post_block_game_event("minecraft:fluid_place", pos, None, None);
        if let Some(mud) = self.pack.assets.registry.default_state("minecraft:mud") {
            self.level_set(pos, mud)?;
        }
        self.award_stat("used", &stack.item, 1);
        self.swing_arm();
        Ok(true)
    }
}

impl PotionCloud {
    /// `AreaEffectCloud.updateParticle`: the custom particle when one is set,
    /// else an entity effect in the potion's colour.
    pub(in crate::game) fn particle(&self) -> ParticleOptions {
        self.custom_particle
            .as_ref()
            .map(|particle| particle.options.clone())
            .unwrap_or_else(|| {
                let c = |shift: u32| (self.color >> shift & 255) as f32 / 255.0;
                ParticleOptions::colored(ParticleType::EntityEffect, [c(16), c(8), c(0), 1.0])
            })
    }
}

impl PotionOwner {
    /// The owner as a damage source's causing entity.
    pub(in crate::game) fn who(&self) -> Option<Who> {
        match self.attacker {
            Some(id) => Some(Who::Mob(id)),
            None => self.player.then_some(Who::Player),
        }
    }
}

impl PotionState {
    /// Their entity ids, for a server to number them.
    pub(in crate::game) fn thrown_ids(&mut self) -> impl Iterator<Item = &mut u32> {
        self.thrown.iter_mut().map(|t| &mut t.id)
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::super::sculk::darkness_blend_added;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::client_side::potions::tests::FoodEffectMod;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::client_side::potions::tests::OtherEffectMod;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::client_side::potions::tests::effect_test_mod;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::client_side::potions::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_client::{
        motion::PlayerMotion, screen_effects::nausea_blend_added,
    };

    pub(in crate::game) fn cloud_spawns_during_pass(potion_pass: bool) {
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.breath_clouds.clear();
            for _ in 0..2 {
                game.spawn_creeper_cloud([0.0, 80.0, 0.0], Vec::new(), Vec::new(), Vec::new(), 0);
            }
            if !potion_pass {
                game.breath_clouds.for_each_mut(|c| c.potion = None);
            }
            let first = game.breath_clouds[0].id;
            let second = game.breath_clouds[1].id;
            let next = game.server.next_entity;
            super::super::entity::taken_pass_tests::with_spawn(
                |game| {
                    assert!(game.breath_clouds.is_empty());
                    for _ in 0..2 {
                        game.spawn_creeper_cloud(
                            [0.0, 80.0, 0.0],
                            Vec::new(),
                            Vec::new(),
                            Vec::new(),
                            0,
                        );
                    }
                },
                || {
                    if potion_pass {
                        game.tick_potion_clouds();
                    } else {
                        game.tick_breath();
                    }
                },
            );
            assert_eq!(
                game.breath_clouds
                    .iter()
                    .map(|c| (c.id, c.age))
                    .collect::<Vec<_>>(),
                [(next + 1, 0), (next + 2, 0), (first, 1), (second, 1)]
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn residency_cloud_load_expiry_bottling_and_late_ids() {
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.breath_clouds.clear();
            game.ominous_spawners.clear();
            let (tag, _) =
                sunrise_core::nbt::parse_snbt("{Pos:[0d,80d,0d],Duration:2,WaitTime:0,Radius:3f}")
                    .unwrap();
            let potion = game.load_potion_cloud(&tag).unwrap();
            for (age, duration) in [(20, 1), (0, 40)] {
                game.breath_clouds.push(super::super::dragon::Breath {
                    attachment_lifetime: Default::default(),
                    join: Default::default(),
                    id: 0,
                    at: [0.0, 80.0, 0.0],
                    motion: [0.0; 3],
                    fall_distance: 0.0,
                    fire_ticks: 0,
                    first_tick: true,
                    radius: 3.0,
                    age,
                    duration,
                    from_dragon: false,
                    victims: Vec::new(),
                    potion: None,
                });
            }
            game.add_remote_ominous_item_spawner(0, [0.0, 80.0, 0.0]);
            let before = game.server.next_entity;
            assert_eq!(game.breath_clouds[1].id, 0);
            game.lan_assign_ids();
            assert_eq!(game.ominous_spawners[0].id, before + 1);
            assert_eq!(game.breath_clouds[1].id, before + 2);
            assert_eq!(game.breath_clouds[2].id, before + 3);
            game.tick_breath();
            assert_eq!(
                game.breath_clouds
                    .iter()
                    .map(|c| (c.id, c.age))
                    .collect::<Vec<_>>(),
                [(potion, 0), (before + 3, 1)]
            );
            game.tick_potion_clouds();
            game.tick_potion_clouds();
            assert_eq!(game.breath_clouds.len(), 1);
            assert_eq!(game.breath_clouds[0].age, 1);
            game.breath_clouds.update(0, |c| c.from_dragon = true);
            assert!(
                collect_dragon_breath(
                    Aabb::new(Vec3::new(-0.3, 80.0, -0.3), Vec3::new(0.3, 81.8, 0.3)),
                    &mut game.breath_clouds
                )
                .is_some()
            );
            assert_eq!(game.breath_clouds[0].radius, 2.5);
            game.breath_clouds.clear();
            assert!(game.breath_clouds.is_empty());
        });
    }
}

#[cfg(test)]
#[path = "potions/cloud_water_tests_server.rs"]
pub(in crate::game) mod cloud_water_tests;
