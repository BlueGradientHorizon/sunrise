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
pub(in crate::game::server_side) use crate::game::client_side::potions::client_tick_effects;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::collect_dragon_breath;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::mob_can_be_affected;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::mob_effect_id;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::potion_cloud_from_nbt;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::potion_cloud_to_nbt;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::sound_on_added;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::splash_distance_sqr;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::thrown_kind;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::thrown_left_owner;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::thrown_margin;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::potions::thrown_player_at;
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
    /// `Arrow.doPostHurtEffects` and `SpectralArrow.doPostHurtEffects`: the
    /// arrow's item gives its target Glowing (spectral) or its potion contents
    /// at the stack's duration scale (a tipped arrow's default is an eighth);
    /// a mob's arrow without such an item (a stray's) carries its own effect.
    /// Instant effects act at once: harming hurts and healing heals, the
    /// other way round for the undead.
    pub(in crate::game) fn arrow_post_hurt_effects(&mut self, p: &Projectile, who: Who) {
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
            self.level_event_at(2002, block, 0xFF38_5DC6u32 as i32);
            if !silent {
                self.level_event_at(1053, block, 0);
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
        self.level_event_at(if instant { 2007 } else { 2002 }, block, color as i32);
        if !silent {
            self.level_event_at(if instant { 1054 } else { 1053 }, block, 0);
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
    /// Right-click with a potion, splash/lingering potion or glass bottle.
    pub(in crate::game) fn use_potion_item(
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
                self.client_audio_play(sound, Some(eye), 0.5, pitch);
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
                    self.client_audio_play(
                        "minecraft:item.bottle.fill_dragonbreath",
                        Some(player_pos),
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
                self.client_audio_play("minecraft:item.bottle.fill", Some(at), 1.0, 1.0);
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
    pub(in crate::game::server_side) use crate::game::client_side::potions::tests::flt;
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
