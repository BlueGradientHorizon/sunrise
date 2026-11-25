//! Potions and status effects in the game: drinking (`PotionItem` via
//! `Consumable`), throwing splash/lingering potions (`ThrownSplashPotion`),
//! filling glass bottles (`BottleItem`), the per-tick effect application,
//! effect attribute modifiers and the HUD effect icons (`Hud.extractEffects`).
use super::{
    Game, Screen,
    dragon::{Breath, FightWho},
    item_draw::owner_at,
};
use sunrise_assets::item_model::DisplayContext;
#[allow(unused_imports)]
use sunrise_client::{
    block_entity_render::vertex_light,
    brewing,
    effects::{EffectInstance, PlayerEffects},
    hud::HudBatch,
    hud_layout, hud_overlays, inventory_effects,
    items::ItemStack,
    mesher::Vertex,
    mobs::{Damage, NamedDamage, Who},
    particles::{ParticleOptions, ParticleType},
    projectiles::{Projectile, ProjectileKind},
    text::Text,
};
use sunrise_core::nbt::Tag;
use sunrise_core::{
    geometry::{Aabb, Vec3},
    random::LegacyRandom,
};

/// Per-cloud settings; natural lingering clouds keep their creation defaults.
pub(in crate::game) struct CloudSettings {
    pub(in crate::game) wait_time: i32,
    pub(in crate::game) reapplication_delay: i32,
    pub(in crate::game) radius_on_use: f32,
    pub(in crate::game) duration_on_use: i32,
    pub(in crate::game) duration_scale: f32,
}

/// A thrown splash or lingering potion in flight.
#[derive(Clone, Debug)]
pub(in crate::game) struct ThrownPotion {
    /// Its entity id once a server's clients track it (0 until then).
    pub(in crate::game) id: u32,
    pub(in crate::game) stack: ItemStack,
    /// The position before the last tick, for drawing between ticks.
    pub(in crate::game) previous: Vec3,
    pub(in crate::game) position: Vec3,
    pub(in crate::game) velocity: Vec3,
    pub(in crate::game) age: u32,
    /// `Projectile.getOwner`'s display name: the player who threw it (none
    /// for a dispenser's).
    pub(in crate::game) owner: Option<Text>,
    /// `Projectile.getOwner` as a target: the thrower (`Who::Player`, a
    /// guest's with `guest` set), none for a dispenser's or a server's.
    pub(in crate::game) thrower: Option<Who>,
    /// On a server, the guest (by connection) who threw it.
    pub(in crate::game) guest: Option<u32>,
    /// `Projectile.leftOwner`: until then it can't hit its thrower.
    pub(in crate::game) left_owner: bool,
    /// `Projectile.lastDeflectedBy`: a mob deflects it once in a row.
    pub(in crate::game) last_deflected_by: Option<u32>,
    /// `Entity.random`: `ProjectileDeflection.REVERSE` draws from the
    /// projectile's own source, not the level's.
    pub(in crate::game) random: LegacyRandom,
}
/// The player (not one of `skip`, the thrower and who rides with them before
/// it has left them) whose box, grown by `margin` (`ProjectileUtil.computeMargin`),
/// holds `p` (`Projectile.canHitEntity`).
#[cfg(test)]
pub(in crate::game::client_side) fn thrown_player_at<'a>(
    players: &'a [(Who, Aabb)],
    skip: &[Who],
    p: Vec3,
    margin: f64,
) -> Option<&'a (Who, Aabb)> {
    players.iter().find(|(who, b)| {
        !skip.contains(who) && b.inflate(Vec3::new(margin, margin, margin)).contains(p)
    })
}
/// `ProjectileUtil.computeMargin`: how far the pick box is grown at `age`.
#[cfg(test)]
pub(in crate::game::client_side) fn thrown_margin(age: u32) -> f64 {
    ((f64::from(age) - 2.0) / 20.0).clamp(0.0, 0.3)
}
/// `Projectile.isOutsideOwnerCollisionRange` (`checkLeftOwner`): none of the
/// boxes of the thrower's vehicle tree (`owner.getRootVehicle().getSelfAndPassengers()`,
/// here `ridden`) is within a block of the potion's (0.25 wide) grown by its
/// motion; true too when there is no thrower (`ridden` empty).
#[cfg(test)]
pub(in crate::game::client_side) fn thrown_left_owner(
    ridden: &[Aabb],
    position: Vec3,
    velocity: Vec3,
) -> bool {
    let h = 0.125;
    let reach = Aabb::new(
        Vec3::new(position.x - h, position.y, position.z - h),
        Vec3::new(position.x + h, position.y + 2.0 * h, position.z + h),
    )
    .expand_towards(velocity)
    .inflate(Vec3::new(1.0, 1.0, 1.0));
    !ridden.iter().any(|b| b.intersects(reach))
}
/// `AreaEffectCloud` wait time set by `ThrownLingeringPotion.onHitAsPotion`.
pub(in crate::game) const LINGERING_WAIT: i32 = 10;
/// `AreaEffectCloud.DEFAULT_REAPPLICATION_DELAY`.
pub(in crate::game) const REAPPLICATION_DELAY: i32 = 20;
/// `AreaEffectCloud.readAdditionalSaveData`: load the cloud's simulation state and decode
/// its custom particle once. The entity id is the caller's (`load_potion_cloud`).
pub(in crate::game) fn potion_cloud_from_nbt(tag: &Tag) -> Option<(Vec3, Breath)> {
    use sunrise_client::save::{float, number, text};
    let at = sunrise_client::entity_nbt::position(tag)?;
    let motion = sunrise_client::entity_nbt::loaded_motion(tag);
    let int = |key, default| number(tag.get(key)).map_or(default, |v| v as i32);
    let real = |key, default| float(tag.get(key)).map_or(default, |v| v as f32);
    let contents = tag.get("potion_contents");
    let mut effects = contents
        .and_then(|c| text(c.get("potion")))
        .and_then(|id| brewing::potion(&id))
        .map_or_else(Vec::new, |p| p.effects.to_vec());
    let (mut hidden, mut ambient) = (Vec::new(), Vec::new());
    if let Some(Tag::List(custom)) = contents.and_then(|c| c.get("custom_effects")) {
        for effect in custom {
            let Some(def) = text(effect.get("id")).and_then(|id| brewing::effect(&id)) else {
                continue;
            };
            let duration = number(effect.get("duration")).unwrap_or(0) as i32;
            let amp = number(effect.get("amplifier")).unwrap_or(0).clamp(0, 255) as i32;
            effects.push((def.id, duration, amp));
            if number(effect.get("show_particles")) == Some(0) {
                hidden.push(def.id);
            }
            if number(effect.get("ambient")).unwrap_or(0) != 0 {
                ambient.push(def.id);
            }
        }
    }
    let color = contents
        .and_then(|c| number(c.get("custom_color")))
        .map_or_else(
            || {
                sunrise_client_core::potions::effects_color(
                    effects
                        .iter()
                        .filter(|(id, ..)| !hidden.contains(id))
                        .map(|(id, _, amp)| (*id, *amp)),
                )
            },
            |color| color as u32,
        );
    Some((
        at,
        Breath {
            attachment_lifetime: Default::default(),
            join: Default::default(),
            id: 0,
            at: [at.x, at.y, at.z],
            motion: [motion.x, motion.y, motion.z],
            fall_distance: sunrise_client::entity_nbt::ground_from_nbt(tag).1,
            fire_ticks: sunrise_client::entity_nbt::fire_ticks_from_nbt(tag),
            first_tick: true,
            radius: real("Radius", 3.0).clamp(0.0, 32.0),
            age: int("Age", 0),
            duration: int("Duration", -1),
            from_dragon: false,
            victims: Vec::new(),
            potion: Some(PotionCloud {
                settings: CloudSettings {
                    wait_time: int("WaitTime", 20),
                    reapplication_delay: int("ReapplicationDelay", 20),
                    radius_on_use: real("RadiusOnUse", 0.0),
                    duration_on_use: int("DurationOnUse", 0),
                    duration_scale: real("potion_duration_scale", 1.0),
                },
                effects,
                hidden,
                ambient,
                radius_per_tick: real("RadiusPerTick", 0.0),
                color,
                custom_particle: tag.get("custom_particle").and_then(|tag| {
                    let options = custom_particle_options(tag)?;
                    let vanilla_tag = options.typed_particle.is_none().then(|| tag.clone());
                    Some(CloudParticle {
                        options,
                        vanilla_tag,
                    })
                }),
                victims: Vec::new(),
                source: Text::translate("entity.minecraft.area_effect_cloud", &[]),
                owner: None,
                guest: None,
            }),
        },
    ))
}

/// `AreaEffectCloud.addAdditionalSaveData`: the lifetime, radius and use
/// settings with the effects the cloud applies, as `custom_effects` with the
/// colour they were drawn in. A cloud from a named potion is written effect by effect;
/// its custom typed particle is encoded by its registered map codec.
pub(in crate::game) fn potion_cloud_to_nbt(c: &Breath, uuid: [i32; 4], raw: Option<&Tag>) -> Tag {
    use sunrise_core::nbt::Compound;
    let Some(potion) = &c.potion else {
        return Tag::End;
    };
    let at = Vec3::new(c.at[0], c.at[1], c.at[2]);
    let motion = Vec3::new(c.motion[0], c.motion[1], c.motion[2]);
    // AreaEffectCloud never turns: the rotation it was loaded with.
    let rotation = raw.map_or([0.0; 2], sunrise_client::entity_nbt::loaded_rotation);
    let mut tag = sunrise_client::entity_nbt::entity_base_tag(
        "minecraft:area_effect_cloud",
        at,
        motion,
        rotation,
        uuid,
        raw,
    );
    let custom: Vec<Tag> = potion
        .effects
        .iter()
        .map(|(id, duration, amplifier)| {
            let mut effect = Compound::new();
            effect.insert("id".into(), Tag::String(brewing::namespaced(id).into()));
            effect.insert("duration".into(), Tag::Int(*duration));
            effect.insert("amplifier".into(), Tag::Int(*amplifier));
            if potion.hidden.contains(id) {
                effect.insert("show_particles".into(), Tag::Byte(0));
            }
            if potion.ambient.contains(id) {
                effect.insert("ambient".into(), Tag::Byte(1));
            }
            Tag::Compound(effect)
        })
        .collect();
    let mut contents = Compound::new();
    contents.insert("custom_effects".into(), Tag::List(custom));
    contents.insert("custom_color".into(), Tag::Int(potion.color as i32));
    let Tag::Compound(map) = &mut tag else {
        return tag;
    };
    map.insert(
        "fall_distance".into(),
        Tag::Double(if c.fall_distance == 0.0 {
            0
        } else {
            c.fall_distance.to_bits()
        }),
    );
    map.insert("Fire".into(), Tag::Short(c.fire_ticks as i16));
    map.insert("Age".into(), Tag::Int(c.age));
    map.insert("Duration".into(), Tag::Int(c.duration));
    map.insert("Radius".into(), Tag::Float(c.radius.to_bits()));
    map.insert(
        "RadiusPerTick".into(),
        Tag::Float(potion.radius_per_tick.to_bits()),
    );
    // The base tag can carry the original NBT, even if its custom particle did not decode.
    map.remove(&"custom_particle".into());
    if let Some(particle) = &potion.custom_particle {
        let saved = match &particle.options.typed_particle {
            Some((value, _)) => match mod_api::__private::encode_particle_map(value) {
                Ok(fields) => Some(Tag::Compound(fields)),
                Err(error) => {
                    log::warn!(
                        "Cannot save cloud particle {:?}: {error}",
                        particle.options.mod_particle
                    );
                    None
                }
            },
            None => particle.vanilla_tag.clone(),
        };
        if let Some(saved) = saved {
            map.insert("custom_particle".into(), saved);
        }
    }
    map.insert("WaitTime".into(), Tag::Int(potion.settings.wait_time));
    map.insert(
        "ReapplicationDelay".into(),
        Tag::Int(potion.settings.reapplication_delay),
    );
    map.insert(
        "RadiusOnUse".into(),
        Tag::Float(potion.settings.radius_on_use.to_bits()),
    );
    map.insert(
        "DurationOnUse".into(),
        Tag::Int(potion.settings.duration_on_use),
    );
    map.insert(
        "potion_duration_scale".into(),
        Tag::Float(potion.settings.duration_scale.to_bits()),
    );
    // Vanilla writes the key only while the contents differ from
    // PotionContents.EMPTY: no effects and the default water colour.
    if !(potion.effects.is_empty()
        && potion.color == sunrise_client_core::potions::BASE_POTION_COLOR)
    {
        map.insert("potion_contents".into(), Tag::Compound(contents));
    }
    tag
}

use sunrise_gameplay::particle_options::custom_particle_options;
#[allow(unused_imports)]
pub(in crate::game) use sunrise_gameplay::particle_options::particle_options;
/// An entity a cloud has affected (`AreaEffectCloud.victims`).
#[derive(Clone, Copy, PartialEq)]
pub(in crate::game) enum Victim {
    Mob(u32),
    Player(FightWho),
}
pub(in crate::game) struct CloudParticle {
    pub(in crate::game) options: ParticleOptions,
    /// Retain the pre-existing save representation only for built-in options.
    pub(in crate::game) vanilla_tag: Option<Tag>,
}

/// A potion-bearing `AreaEffectCloud`: its effects, settings and recent victims.
pub(in crate::game) struct PotionCloud {
    pub(in crate::game) settings: CloudSettings,
    pub(in crate::game) effects: Vec<(&'static str, i32, i32)>,
    /// The effects above that were added hidden (`MobEffectInstance.isVisible` false: no
    /// particles); the instances the cloud applies are copies and keep that.
    pub(in crate::game) hidden: Vec<&'static str>,
    /// The effects above that were ambient (`MobEffectInstance.isAmbient`).
    pub(in crate::game) ambient: Vec<&'static str>,
    /// `AreaEffectCloud.radiusPerTick`: natural clouds use `-radius / duration`.
    pub(in crate::game) radius_per_tick: f32,
    /// The potion's colour (`PotionContents.getColor`).
    pub(in crate::game) color: u32,
    /// `AreaEffectCloud.customParticle`: the value decoded when the cloud was loaded.
    pub(in crate::game) custom_particle: Option<CloudParticle>,
    /// Each victim and the tick it can be affected again.
    pub(in crate::game) victims: Vec<(Victim, i32)>,
    /// Who its instant damage names (`indirectMagic(cloud, owner)`): the
    /// thrower, else the cloud.
    pub(in crate::game) source: Text,
    /// `AreaEffectCloud.getOwner`: the thrower (`ThrownLingeringPotion.onHitAsPotion`), whose
    /// kill a mob's death to its instant damage is.
    pub(in crate::game) owner: Option<Who>,
    /// On a server, the guest (by connection) who threw it.
    pub(in crate::game) guest: Option<u32>,
}
/// `Projectile.getOwner` of a thrown potion: what its `indirectMagic` damage
/// is named after and who it counts against. Nothing for a dispenser's or a
/// summoned potion nobody owns, the potion itself then naming the death
/// message.
#[derive(Clone, Debug, Default)]
pub(in crate::game) struct PotionOwner {
    /// The thrower's name, when it has one.
    pub(in crate::game) name: Option<Text>,
    /// The mob that threw it (a witch): the hurt is its kill credit, as
    /// `indirectMagic(potion, owner)` gives it. A player's throw is the
    /// potion's own and only `name`s the message.
    pub(in crate::game) attacker: Option<u32>,
    /// A player threw it: the water damage `indirectMagic` deals is theirs.
    pub(in crate::game) player: bool,
    /// On a server, the guest (by connection) who threw it: whose kill the mobs it hurts are
    /// (`LivingEntity.lastHurtByPlayer`).
    pub(in crate::game) guest: Option<u32>,
}
/// A mob's effect ids carry the `minecraft:` namespace; a mod's has its own.
pub(in crate::game) fn mob_effect_id(id: &str) -> Option<&'static str> {
    brewing::effect(id).map(|e| brewing::namespaced(e.id))
}
/// `LivingEntity.canBeAffected` with the mob overrides (`WitherBoss` and
/// `WitherSkeleton` shrug off wither, `Spider` and `AbstractNautilus` poison,
/// `Parched` weakness): the immune-to-infested and immune-to-oozing tags
/// decide first, then the tag of mobs that ignore poison and regeneration.
/// `WitherBoss.addEffect` refusing everything is `Mob::add_effect`'s: a cloud
/// still takes the wither as a victim (`AreaEffectCloud` asks `canBeAffected`),
/// so its instant effects land and it shrinks.
pub(in crate::game) fn mob_can_be_affected(
    tags: &sunrise_client::items::EntityTypeTags,
    kind: &str,
    slot: Option<usize>,
    effect: &str,
) -> bool {
    use sunrise_client::items::EntityTypeTags;
    let effect = effect.trim_start_matches("minecraft:");
    let overridden = match kind.trim_start_matches("minecraft:") {
        "wither" | "wither_skeleton" => effect == "wither",
        "spider" | "cave_spider" | "nautilus" | "zombie_nautilus" => effect == "poison",
        "parched" => effect == "weakness",
        _ => false,
    };
    if overridden {
        return false;
    }
    let in_tag = |flag| tags.has_bit(slot, flag);
    if in_tag(EntityTypeTags::IMMUNE_TO_INFESTED) {
        effect != "infested"
    } else if in_tag(EntityTypeTags::IMMUNE_TO_OOZING) {
        effect != "oozing"
    } else {
        !in_tag(EntityTypeTags::IGNORES_POISON_AND_REGEN)
            || (effect != "regeneration" && effect != "poison")
    }
}
/// `ThrownSplashPotion.onHitAsPotion`: the squared distance from the potion's
/// box at the hit point to an entity's box grown by the hit margin
/// (`ProjectileUtil.computeMargin`), for entities that meet the potion's box
/// grown by (4, 2, 4); none when it is 16 or more.
pub(in crate::game) fn splash_distance_sqr(at: Vec3, age: u32, bounds: Aabb) -> Option<f64> {
    let potion = Aabb::new(
        Vec3::new(at.x - 0.125, at.y, at.z - 0.125),
        Vec3::new(at.x + 0.125, at.y + 0.25, at.z + 0.125),
    );
    if !potion.inflate(Vec3::new(4.0, 2.0, 4.0)).intersects(bounds) {
        return None;
    }
    let margin = ((f64::from(age) - 2.0) / 20.0).clamp(0.0, 0.3);
    let d2 = potion.distance_squared_to_box(bounds.inflate(Vec3::new(margin, margin, margin)));
    (d2 < 16.0).then_some(d2)
}
/// A splash potion, lingering potion or experience bottle about to fly (the
/// player's throws and dispensers both make one): `ThrownSplashPotion`,
/// `ThrownLingeringPotion` or `ThrownExperienceBottle`, which
/// `level.addFreshEntity` makes a level entity like a summoned one, so
/// selectors and `/data` find it (#13201).
pub(in crate::game) fn thrown_projectile(
    stack: ItemStack,
    position: Vec3,
    by_player: bool,
) -> Projectile {
    let kind = match &*stack.item {
        "minecraft:experience_bottle" => ProjectileKind::ExperienceBottle,
        item => {
            let lingering = item == "minecraft:lingering_potion";
            ProjectileKind::Potion(Box::new(stack), lingering)
        }
    };
    Projectile::new(kind, position, by_player.then_some(Who::Player))
}
/// What a player's body was last given for its effects and enchantments:
/// a guest keeps their own while the host's are in `PotionState`.
#[derive(Default)]
pub(in crate::game) struct BodyPotions {
    pub(in crate::game) protection_for: Option<(u64, Vec<Option<ItemStack>>)>,
    pub(in crate::game) enchant_modifiers: Vec<(String, String, f64, String)>,
}
/// Effects, thrown potions and the cached armor and enchantment modifiers.
#[derive(Default)]
pub(in crate::game) struct PotionState {
    pub(in crate::game) effects: PlayerEffects,
    /// Reused holder snapshot for the read-only mod effect tick event.
    pub(in crate::game) tick_keys: std::cell::RefCell<Vec<mod_api::EffectKey>>,
    pub(in crate::game) thrown: Vec<ThrownPotion>,
    /// Armor the protection table was computed for.
    pub(in crate::game) protection_for: Option<(u64, Vec<Option<ItemStack>>)>,

    /// Enchantment attribute modifiers on the player: (attribute, id, amount, operation).
    pub(in crate::game) enchant_modifiers: Vec<(String, String, f64, String)>,
}

impl PotionState {
    /// Trade the player's own state (effects and what their body was given)
    /// with a guest's, around `as_guest`. Potions in flight stay.
    pub(in crate::game) fn swap_player(
        &mut self,
        effects: &mut PlayerEffects,
        body: &mut BodyPotions,
    ) {
        std::mem::swap(&mut self.effects, effects);
        std::mem::swap(&mut self.protection_for, &mut body.protection_for);
        std::mem::swap(&mut self.enchant_modifiers, &mut body.enchant_modifiers);
    }
    /// Thrown potions in flight as (entity type, position, motion), for the
    /// dispenser parity fixture.
    pub(in crate::game) fn thrown_entities(&self) -> Vec<(&'static str, Vec3, Vec3)> {
        self.thrown
            .iter()
            .map(|t| (thrown_kind(&t.stack), t.position, t.velocity))
            .collect()
    }
    pub(in crate::game) fn clear_thrown(&mut self) {
        self.thrown.clear();
    }
    /// The numbered ones as a server's clients track them: (id, entity
    /// type, item, position, motion).
    pub(in crate::game) fn thrown_tracked(
        &self,
    ) -> impl Iterator<Item = (u32, &'static str, &ItemStack, Vec3, Vec3)> {
        self.thrown.iter().filter(|t| t.id != 0).map(|t| {
            (
                t.id,
                thrown_kind(&t.stack),
                &t.stack,
                t.position,
                t.velocity,
            )
        })
    }
    /// `handleAddEntity` for a thrown potion or bottle on a server's client:
    /// drawn where the server says (its item comes in its entity data).
    pub(in crate::game) fn add_remote_thrown(
        &mut self,
        id: u32,
        kind: &str,
        at: Vec3,
        velocity: Vec3,
    ) {
        self.thrown.retain(|t| t.id != id);
        let item = match kind {
            "minecraft:lingering_potion" | "minecraft:experience_bottle" => kind,
            _ => "minecraft:splash_potion",
        };
        self.thrown.push(ThrownPotion {
            id,
            stack: ItemStack::new(item, 1),
            previous: at,
            position: at,
            velocity,
            age: 0,
            owner: None,
            thrower: None,
            guest: None,
            left_owner: false,
            last_deflected_by: None,
            random: LegacyRandom::new(i64::from(id)),
        });
    }
    /// ThrowableItemProjectile.DATA_ITEM_STACK: the potion it shows.
    pub(in crate::game::client_side) fn remote_thrown_item(&mut self, id: u32, stack: ItemStack) {
        if let Some(t) = self.thrown.iter_mut().find(|t| t.id == id) {
            t.stack = stack;
        }
    }
    /// A server's move: thrown potions have no interpolation handler, so
    /// `moveOrInterpolateTo` puts them there at once. False when `id` isn't one.
    pub(in crate::game::client_side) fn remote_thrown_pose(
        &self,
        id: u32,
    ) -> Option<([f64; 3], [f64; 3])> {
        let t = self.thrown.iter().find(|t| t.id == id)?;
        Some((
            [t.position.x, t.position.y, t.position.z],
            [t.previous.x, t.previous.y, t.previous.z],
        ))
    }
    pub(in crate::game) fn move_remote_thrown(&mut self, id: u32, at: Vec3) -> bool {
        let Some(t) = self.thrown.iter_mut().find(|t| t.id == id) else {
            return false;
        };
        t.position = at;
        true
    }
    /// `handleSetEntityMotion` for a thrown potion. False when `id` isn't one.
    pub(in crate::game::client_side) fn remote_thrown_motion(
        &mut self,
        id: u32,
        velocity: Vec3,
    ) -> bool {
        let Some(t) = self.thrown.iter_mut().find(|t| t.id == id) else {
            return false;
        };
        t.velocity = velocity;
        true
    }
    pub(in crate::game::client_side) fn remove_remote_thrown(&mut self, id: u32) {
        self.thrown.retain(|t| t.id != id);
    }
    pub(in crate::game::client_side) fn has_thrown(&self, id: u32) -> bool {
        self.thrown.iter().any(|t| t.id == id)
    }
}

/// The entity type of a thrown potion or bottle.
pub(in crate::game::client_side) fn thrown_kind(stack: &ItemStack) -> &'static str {
    match &*stack.item {
        "minecraft:lingering_potion" => "minecraft:lingering_potion",
        "minecraft:experience_bottle" => "minecraft:experience_bottle",
        _ => "minecraft:splash_potion",
    }
}

/// `BottleItem.use`: find a living `AreaEffectCloud` owned by an `EnderDragon`
/// within the player's box grown by 2, and shrink its radius by 0.5. Gives
/// the cloud's position when one was found.
pub(in crate::game) fn collect_dragon_breath(
    player_box: Aabb,
    clouds: &mut sunrise_client::join_seq::ResidentList<super::dragon::Breath>,
) -> Option<[f64; 3]> {
    let search_box = player_box.inflate(Vec3::new(2.0, 2.0, 2.0));
    if let Some(i) = clouds.iter().position(|c| {
        if !c.from_dragon || c.potion.is_some() || super::dragon::breath_expired(c.age, c.duration)
        {
            return false;
        }
        let r = f64::from(c.radius);
        let cloud_box = Aabb::new(
            Vec3::new(c.at[0] - r, c.at[1], c.at[2] - r),
            Vec3::new(c.at[0] + r, c.at[1] + 0.5, c.at[2] + r),
        );
        search_box.intersects(cloud_box)
    }) {
        let at = clouds[i].at;
        clouds.update(i, |c| c.radius = (c.radius - 0.5).max(0.0));
        Some(at)
    } else {
        None
    }
}

#[cfg(feature = "client")]
impl Game<'_, crate::game::client_side::ClientState> {
    /// A client's own effects (`LivingEntity.tickEffects` on the client,
    /// `MobEffectInstance.tickClient`): they count down and blend, and the
    /// server removes them. Their modifiers and flags move the predicted
    /// player.
    pub(in crate::game) fn tick_client_effects(&mut self) {
        client_tick_effects(&mut self.potions.effects, &mut self.side.darkness_blend);
        self.apply_effect_status();
    }
    /// `ThrowableProjectile.tick` on a server's client: it falls and moves
    /// by itself between the server's updates and stops at a block; only
    /// the server breaks it.
    pub(in crate::game) fn step_remote_thrown(&mut self) {
        let mut thrown = std::mem::take(&mut self.potions.thrown);
        for t in &mut thrown {
            t.previous = t.position;
            t.age += 1;
            self.thrown_potion_motion(t);
            let next = Vec3::new(
                t.position.x + t.velocity.x,
                t.position.y + t.velocity.y,
                t.position.z + t.velocity.z,
            );
            let state = self.world.block(
                next.x.floor() as i32,
                next.y.floor() as i32,
                next.z.floor() as i32,
            );
            if self.res.facts().get(state).full_collision {
                t.velocity = Vec3::ZERO;
            } else {
                t.position = next;
            }
        }
        self.potions.thrown = thrown;
    }
    /// `ThrownItemRenderer` for thrown splash and lingering potions and
    /// experience bottles: the item (GROUND) facing the camera at scale 1,
    /// lit at the light probe (the eye, 0.85 of the 0.25 height).
    pub(in crate::game::client_side) fn thrown_potion_geometry(
        &self,
        camera: glam::DVec3,
        alpha: f64,
        out: &mut Vec<Vertex>,
    ) {
        let orientation = glam::Mat4::from_quat(self.camera_orientation());
        for t in &self.potions.thrown {
            if self.hides_as_camera(t.id) {
                continue;
            }
            let previous = glam::DVec3::new(t.previous.x, t.previous.y, t.previous.z);
            let position = glam::DVec3::new(t.position.x, t.position.y, t.position.z);
            // ThrowableProjectile.shouldRenderAtSqrDistance: hidden for its
            // first two ticks within 3.5 blocks (not in the thrower's face).
            if t.age < 2 && position.distance_squared(camera) < 12.25 {
                continue;
            }
            if !self.standing_entity_in_view(
                camera,
                super::entity_culling::at(t.previous),
                super::entity_culling::at(t.position),
                alpha,
                0.25,
                0.25,
                64.0,
                0.0,
            ) {
                continue;
            }
            let p = previous.lerp(position, alpha);
            let owner = owner_at(p.to_array(), 0.0, &t.stack.item);
            let state = self.resolve_item(&t.stack, DisplayContext::Ground, Some(&owner), 0);
            if state.is_empty() {
                continue;
            }
            let pose = glam::Mat4::from_translation((p - camera).as_vec3()) * orientation;
            let light = self.world.light(
                p.x.floor() as i32,
                (p.y + 0.2125).floor() as i32,
                p.z.floor() as i32,
            );
            // Frame generation and temporal upscaling follow its own flight; one not yet
            // given an id (0) can't be told apart from another, so it keeps the camera's.
            let motion = if t.id == 0 {
                0
            } else {
                crate::game::MOTION_OBJECT | u64::from(t.id)
            };
            self.mark_motion(out.len(), motion);
            self.submit_item_world(&state, pose, vertex_light(light), out);
        }
        self.mark_motion(out.len(), 0);
    }
    /// `Screen.showsActiveEffects`: the survival and creative inventories
    /// draw their own effect list when `EffectsInInventory.canSeeEffects`
    /// (32 or more pixels right of the screen).
    pub(in crate::game::client_side) fn screen_shows_active_effects(
        &self,
        batch: &HudBatch,
    ) -> bool {
        let (ox, image_width) = match self.side.screen {
            Screen::Inventory
                if self.container.is_none() && self.craft_size == 2 && !self.mount_active() =>
            {
                (self.inventory_origin(batch).0, 176)
            }
            Screen::Creative => (
                Self::creative_origin(batch).0,
                sunrise_client::creative::Creative::WIDTH as i32,
            ),
            _ => return false,
        };
        batch.width as i32 - (ox as i32 + image_width + 2) >= 32
    }
    /// `Hud.extractEffects`: icons top-right, beneficial row then harmful row;
    /// every screen but the inventories keeps them.
    pub(in crate::game::client_side) fn draw_effect_icons(&self, batch: &mut HudBatch) {
        if self.potions.effects.list.is_empty() || self.screen_shows_active_effects(batch) {
            return;
        }
        let icons: Vec<hud_overlays::EffectIcon> = self
            .potions
            .effects
            .list
            .iter()
            .map(hud_overlays::EffectIcon::from)
            .collect();
        let draws = hud_overlays::effects(batch.width as i32, false, &icons);
        hud_layout::paint(batch, &self.gui.atlas, &draws);
    }
    /// `EffectsInInventory`: panels to the right of survival/creative inventory.
    /// The caller draws the returned tooltip after every other tooltip.
    /// `ScreenEvent.RenderInventoryMobEffects` fires when the build lays the
    /// panels out once a frame, and its answer is kept in the build.
    pub(in crate::game::client_side) fn draw_inventory_effects(
        &self,
        batch: &mut HudBatch,
        origin: (f32, f32),
        image_width: i32,
    ) -> Option<Vec<(Text, u32)>> {
        let space = Self::effects_space(batch, [origin.0, origin.1], image_width as f32);
        let layout = self.built_container(batch).map_or_else(
            || inventory_effects::EffectsLayout::vanilla(space.available()),
            |layout| layout.effects_layout,
        );
        let panels = self.effect_panels(&space, &layout);
        let hovered = inventory_effects::paint(
            batch,
            &self.gui.atlas,
            &panels,
            [self.side.cursor.0 as i32, self.side.cursor.1 as i32],
        )?;
        Some(self.effect_tooltip(batch, hovered))
    }
    /// A hovered panel's tooltip lines, after `GatherEffectScreenTooltipsEvent`
    /// (fired only while a mod listens).
    pub(in crate::game::client_side) fn effect_tooltip(
        &self,
        batch: &HudBatch,
        hovered: inventory_effects::HoveredEffectPanel<'_, '_>,
    ) -> Vec<(Text, u32)> {
        let panel = hovered.panel;
        let mut lines = inventory_effects::tooltip(hovered);
        if mod_api::__private::listened(mod_api::HookId::GatherEffectScreenTooltips) {
            let view = self.screen_view(batch, self.side.screen);
            let effect = super::native_living_effects::active_effect(panel.effect);
            self.native_gather_effect_screen_tooltips(&view, &effect, &mut lines);
        }
        lines
    }
    /// The rectangle `draw_inventory_effects` covers, none with no effects
    /// or too little room right of the window, laid out as `layout` says.
    pub(in crate::game::client_side) fn inventory_effects_rect(
        &self,
        batch: &HudBatch,
        origin: [f32; 2],
        image_width: f32,
        layout: &inventory_effects::EffectsLayout,
    ) -> Option<[f32; 4]> {
        let space = Self::effects_space(batch, origin, image_width);
        let panels = self.effect_panels(&space, layout);
        let first = panels.first()?;
        let width = panels.iter().map(|p| p.width).max()?;
        let height = (panels.len() as i32 - 1) * first.spacing + 32;
        Some([first.x as f32, first.y as f32, width as f32, height as f32])
    }
    /// The room the effect panels lay out in (`EffectsInInventory`'s space):
    /// the window's width, the container's origin and its image width.
    pub(in crate::game::client_side) fn effects_space(
        batch: &HudBatch,
        origin: [f32; 2],
        image_width: f32,
    ) -> inventory_effects::EffectsSpace {
        inventory_effects::EffectsSpace {
            screen_width: batch.width as i32,
            origin: [origin[0] as i32, origin[1] as i32],
            image_width: image_width as i32,
        }
    }
    pub(in crate::game::client_side) fn effect_panels(
        &self,
        space: &inventory_effects::EffectsSpace,
        layout: &inventory_effects::EffectsLayout,
    ) -> Vec<inventory_effects::EffectPanel<'_>> {
        inventory_effects::panels(
            space,
            layout,
            &self.potions.effects.list,
            self.clock.rate(),
            &|key| self.pack.names.display_get(key),
            &|text| self.gui.atlas.font.width(text).ceil() as i32,
        )
    }
    /// Vanilla's layout, as `native_screen_render_inventory_mob_effects`
    /// leaves it, for the panels and their exclusion rect. No fire with no
    /// effect to show or no room (vanilla returns first), nor with no mod
    /// listening. The answer is taken once a frame: the display's change
    /// builds twice in one frame (`apply_ui_settings` then `observe_screen`),
    /// and the second build reuses the first one's answer.
    pub(in crate::game::client_side) fn inventory_effects_layout(
        &self,
        space: &inventory_effects::EffectsSpace,
        rect: [f32; 4],
    ) -> inventory_effects::EffectsLayout {
        let mut layout = inventory_effects::EffectsLayout::vanilla(space.available());
        if self.potions.effects.list.is_empty()
            || space.available() < 32
            || !mod_api::__private::listened(mod_api::HookId::ScreenRenderInventoryMobEffects)
        {
            return layout;
        }
        if let Some(answer) = self.side.screen_state.0.build.effects_answer(
            self.side.frames,
            self.side.screen,
            *space,
        ) {
            return answer;
        }
        // Asked while the screen is laid out: nothing is drawn beside it yet.
        let view = super::screen_backgrounds::without_regions(
            &self.screen_view_at(self.side.screen, rect),
        );
        let mut canceled = false;
        self.native_screen_render_inventory_mob_effects(
            &view,
            space.available(),
            &mut layout.compact,
            &mut layout.offset,
            &mut canceled,
        );
        layout.hidden = canceled;
        self.side.screen_state.0.build.keep_effects_answer(
            self.side.frames,
            self.side.screen,
            *space,
            layout,
        );
        layout
    }
    pub(in crate::game::client_side) fn effects_json(&self) -> serde_json::Value {
        serde_json::json!({
            "active": self.potions.effects.list.iter().map(|e| serde_json::json!({ "id": e.id, "duration": e.duration, "amplifier": e.amplifier })).collect::<Vec<_>>(),
            "thrown": self.potions.thrown.iter().map(|t| [t.position.x, t.position.y, t.position.z]).collect::<Vec<_>>(),
        })
    }
}

/// `LivingEntity.tickEffects` on a client: each `MobEffectInstance.tickClient`
/// counts its effect down, then ticks its `BlendState`. The darkness effect's
/// is the fade `darkness_lighting` and `darkness_fog` draw; the integrated
/// level's `tick_sculk_entities` is not run for a server's client (#3393).
pub(in crate::game::client_side) fn client_tick_effects(
    effects: &mut PlayerEffects,
    darkness: &mut (f32, f32),
) {
    for e in &mut effects.list {
        e.tick_client();
    }
    *darkness = super::sculk::darkness_blend_tick(effects, *darkness);
}

#[cfg(feature = "client")]
#[cfg(test)]
pub(in crate::game) mod tests {
    use super::super::sculk::darkness_blend_added;
    use super::*;
    use sunrise_client::screen_effects::nausea_blend_added;

    pub(in crate::game) fn flt(v: f32) -> Tag {
        Tag::Float(v.to_bits())
    }

    /// `AreaEffectCloud.updateParticle` with a `custom_particle`: the codec's
    /// type and payload win over the potion colour; none falls back to the
    /// entity effect in that colour.
    #[test]
    fn cloud_custom_particle_overrides_potion_colour() {
        let cloud = |custom: Option<Tag>| {
            let mut tag = Tag::compound([
                ("Pos", Tag::List(vec![Tag::Double(0f64.to_bits()); 3])),
                ("Radius", flt(3.0)),
                ("Duration", Tag::Int(200)),
            ]);
            if let (Tag::Compound(map), Some(custom)) = (&mut tag, custom) {
                map.insert("custom_particle".into(), custom);
            }
            potion_cloud_from_nbt(&tag).unwrap().1.potion.unwrap()
        };
        let none = cloud(None).particle();
        assert_eq!(none.ty, ParticleType::EntityEffect);
        let flame = cloud(Some(Tag::compound([("type", Tag::String("flame".into()))])));
        assert_eq!(flame.particle().ty, ParticleType::Flame);
        let dust = cloud(Some(Tag::compound([
            ("type", Tag::String("minecraft:dust".into())),
            ("color", Tag::List(vec![flt(1.0), flt(0.0), flt(0.0)])),
            ("scale", flt(2.0)),
        ])))
        .particle();
        assert_eq!(dust.ty, ParticleType::Dust);
        assert_eq!(dust.color[..3], [1.0, 0.0, 0.0]);
        assert_eq!(dust.scale, 2.0);
        let effect = cloud(Some(Tag::compound([
            ("type", Tag::String("entity_effect".into())),
            ("color", Tag::Int(0x00FF_8000u32 as i32)),
        ])))
        .particle();
        assert_eq!(effect.ty, ParticleType::EntityEffect);
        assert_eq!(effect.color[..3], [1.0, 128.0 / 255.0, 0.0]);
    }

    /// `AreaEffectCloud.addAdditionalSaveData` writes `custom_particle` back.
    #[test]
    fn cloud_custom_particle_survives_a_save() {
        let particle = Tag::compound([("type", Tag::String("flame".into()))]);
        let tag = Tag::compound([
            ("Pos", Tag::List(vec![Tag::Double(0f64.to_bits()); 3])),
            ("Radius", flt(3.0)),
            ("Duration", Tag::Int(200)),
            ("custom_particle", particle.clone()),
        ]);
        let (at, cloud) = potion_cloud_from_nbt(&tag).unwrap();
        let _ = at;
        let saved = potion_cloud_to_nbt(&cloud, [1, 2, 3, 4], None);
        let Tag::Compound(map) = saved else {
            panic!("not a compound");
        };
        assert_eq!(map.get(&"custom_particle".into()), Some(&particle));
    }

    #[test]
    #[ignore = "daily"]
    fn typed_cloud_remote_metadata_keeps_valid_options_on_malformed_update() {
        if crate::game::native_entity_refs::tests::run_fresh_cases(
            "game::client_side::potions::tests::typed_cloud_remote_metadata_keeps_valid_options_on_malformed_update",
            "SUNRISE_TEST_TYPED_CLOUD_CLIENT_CASE",
            &["cloud"],
        ) {
            return;
        }
        use crate::game::lan_particles::tests::MeasuredParticle;
        use sunrise_protocol::{
            entity_data::{Particle, ParticleData, Value, serializer},
            packets::play::AddEntity,
        };

        crate::game::native_entity_refs::tests::with_client_game(|game| {
            let entries = [mod_api::ModEntry::new("test", Box::new(MeasuredParticle))];
            mod_platform::__private::part_tests::install_parts_fixture(&entries).unwrap();
            crate::game::mods::resolve_particle_providers(&Default::default()).unwrap();
            let id = 42_i32;
            let particle = Particle {
                id: 0,
                data: Some(ParticleData::Mod {
                    name: "test:measured".into(),
                    options: vec![1, 1, 0, 0],
                }),
            };
            game.spawn_remote_projectile(
                "minecraft:area_effect_cloud",
                &AddEntity {
                    id,
                    uuid: 0,
                    kind: 0,
                    position: [0.0, 80.0, 0.0],
                    velocity: [0.0; 3],
                    pitch: 0,
                    yaw: 0,
                    head_yaw: 0,
                    data: 0,
                },
                (0.0, 0.0),
            );
            assert_eq!(
                game.side.remote_clouds[0].particle.ty,
                ParticleType::EntityEffect
            );
            game.remote_projectile_data(
                id,
                &[(10, serializer::PARTICLE, Value::Particle(particle.clone()))],
            );
            let original = game.side.remote_clouds[0].particle.typed_particle.clone();
            assert!(original.is_some());
            let mut bad = particle;
            let Some(ParticleData::Mod { options, .. }) = &mut bad.data else {
                unreachable!()
            };
            options.truncate(1);
            game.remote_projectile_data(id, &[(10, serializer::PARTICLE, Value::Particle(bad))]);
            assert_eq!(game.side.remote_clouds[0].particle.typed_particle, original);
        });
    }

    #[derive(Clone)]
    pub(in crate::game) struct FoodEffectMod(
        pub(in crate::game) std::sync::Arc<std::sync::atomic::AtomicBool>,
    );
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for FoodEffectMod {
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            for id in ["test:comfort", "test:nourishment"] {
                let key = r.effect_key(id);
                r.mob_effect_part(&key, self.clone()).unwrap();
            }
        }
        fn mob_effect_expired(
            &self,
            _: &dyn mod_api::Host,
            event: &mut mod_api::MobEffectExpiredEvent,
        ) -> mod_api::Flow {
            if self.0.load(std::sync::atomic::Ordering::SeqCst)
                && event.effect == mod_api::__private::effect_key("minecraft:regeneration").unwrap()
            {
                mod_api::Flow::Cancel
            } else {
                mod_api::Flow::Continue
            }
        }
    }
    #[mod_api::sunrise_part]
    impl mod_api::MobEffectPart for FoodEffectMod {
        fn should_apply_effect_tick_this_tick(
            &self,
            _: &dyn mod_api::Host,
            event: &mut mod_api::ShouldApplyEffectTickThisTickEvent,
        ) {
            let comfort = mod_api::__private::effect_key("test:comfort").unwrap();
            event.apply = event.effect != comfort || event.tick_count % 80 == 0;
        }
        fn apply_effect_tick(
            &self,
            host: &dyn mod_api::Host,
            event: &mut mod_api::ApplyEffectTickEvent<'_>,
        ) {
            let level = host.level(event.dimension).unwrap();
            let natural = level
                .game_rule(host, "minecraft:natural_health_regeneration")
                .unwrap()
                .unwrap();
            let mod_api::GameRuleValue::Bool(natural) = natural else {
                panic!("boolean game rule")
            };
            if event.effect == mod_api::__private::effect_key("test:comfort").unwrap() {
                if !event
                    .has_effect(mod_api::__private::effect_key("minecraft:regeneration").unwrap())
                    && event.food.is_none_or(|food| food.saturation <= 0.0)
                    && event.health < event.max_health
                {
                    event.heal(1.0);
                }
            } else if let Some(food) = event.food {
                if event.amplifier == 1 {
                    // Pin FoodData.addExhaustion's lack of a lower clamp.
                    event.add_exhaustion(-5.0);
                } else if !(natural && event.health < event.max_health && food.level >= 18)
                    && food.exhaustion > 0.0
                {
                    event.add_exhaustion(-food.exhaustion.min(4.0));
                }
            }
        }
    }
    pub(in crate::game) struct OtherEffectMod;
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for OtherEffectMod {
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            let effect = r.effect_key("other-mod:foreign");
            r.mob_effect_part(&effect, OtherEffectMod).unwrap();
        }
    }
    #[mod_api::sunrise_part]
    impl mod_api::MobEffectPart for OtherEffectMod {
        fn should_apply_effect_tick_this_tick(
            &self,
            _: &dyn mod_api::Host,
            _: &mut mod_api::ShouldApplyEffectTickThisTickEvent,
        ) {
            panic!("another mod's shouldApply listener must not run");
        }
        fn apply_effect_tick(
            &self,
            _: &dyn mod_api::Host,
            _: &mut mod_api::ApplyEffectTickEvent<'_>,
        ) {
            panic!("another mod's apply listener must not run");
        }
    }

    pub(in crate::game) fn effect_test_mod(
        extra: impl IntoIterator<Item = mod_api::ModEntry>,
    ) -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
        use mod_api::{self, *};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering::Relaxed},
        };

        struct TestMod(Arc<AtomicUsize>);
        #[mod_api::__subscriptions]
        impl Hooks for TestMod {
            fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
                let key = r.effect_key("test:effect");
                r.mob_effect_part(&key, TestMod(self.0.clone())).unwrap();
            }
        }
        #[mod_api::sunrise_part]
        impl MobEffectPart for TestMod {
            fn should_apply_effect_tick_this_tick(
                &self,
                _: &dyn Host,
                event: &mut ShouldApplyEffectTickThisTickEvent,
            ) {
                event.apply = true;
            }
            fn apply_effect_tick(&self, _: &dyn Host, event: &mut ApplyEffectTickEvent<'_>) {
                self.0.fetch_add(1, Relaxed);
                match event.amplifier {
                    0 => {
                        event.heal(10.0);
                        event.eat(4, 0.5);
                        event.add_exhaustion(1.5);
                    }
                    1 => event.hurt("minecraft:magic", 3.0),
                    3 if event.health > 1.0 => event.hurt("minecraft:magic", 1.0),
                    3 => {}
                    4 => {
                        event.hurt("minecraft:wither", 1.0);
                        event.hurt("minecraft:indirect_magic", 2.0);
                        event.heal(1.0);
                    }
                    _ => event.hurt("minecraft:wither", 100.0),
                }
            }
        }
        let applied = Arc::new(AtomicUsize::new(0));
        let effects = ModEntry::new("test-potions", Box::new(TestMod(applied.clone())));
        mod_platform::__private::install(
            std::iter::once(effects)
                .chain(extra)
                .collect::<Vec<_>>()
                .into(),
        );
        assert!(brewing::register_mod_effects([(
            "test:effect".into(),
            0,
            brewing::Category::Neutral,
            None,
            "test-potions".into(),
        )]));
        applied
    }

    /// A mod effect's damage type keeps its registry id and tags whether it is
    /// vanilla's or a datapack's, and one the registry lacks is refused.
    #[test]
    fn effect_damage_sources_retain_their_ids() {
        let type_json = |id: &str| {
            serde_json::json!({
                "id": id, "message_id": "m", "exhaustion": "00000000",
                "scaling": "never", "effects": "hurt", "death_message_type": "default"
            })
        };
        let catalog = sunrise_world::damage_source::Catalog::from_json(
            &serde_json::json!({
                "data_version": sunrise_world::DATA_VERSION,
                "types": [type_json("minecraft:magic"), type_json("demo:scorch")],
            }),
            &serde_json::json!({
                "minecraft:is_fire": "{\"values\":[\"demo:scorch\"]}",
                "minecraft:bypasses_armor": "{\"values\":[\"demo:scorch\",\"minecraft:magic\"]}",
            }),
        )
        .unwrap();
        for (id, tags) in [
            ("minecraft:magic", NamedDamage::BYPASSES_ARMOR),
            (
                "demo:scorch",
                NamedDamage::IS_FIRE | NamedDamage::BYPASSES_ARMOR,
            ),
        ] {
            let named = NamedDamage::registered(&catalog, id).unwrap();
            assert_eq!((named.name, named.tags), (id, tags));
            let kind = crate::game::ServerGame::damage_kind(&catalog, id);
            assert_eq!(
                super::super::creatures::source_damage_type(Damage::of(kind)),
                id
            );
        }
        assert_eq!(NamedDamage::registered(&catalog, "demo:missing"), None);
    }

    /// `AreaEffectCloud.addAdditionalSaveData` with `readAdditionalSaveData`: a
    /// lingering cloud written at a save reads back with its lifetime, radius,
    /// use settings and the effects it sprays, with the hidden and ambient
    /// flags it carried.
    #[test]
    fn a_lingering_cloud_survives_a_save() {
        let cloud = Breath {
            attachment_lifetime: Default::default(),
            join: Default::default(),
            id: 7,
            at: [1.5, 64.0, -2.5],
            motion: [0.6, 0.0, 0.0],
            fall_distance: 0.0,
            fire_ticks: 0,
            first_tick: true,
            radius: 3.5,
            age: 12,
            duration: 400,
            from_dragon: false,
            victims: Vec::new(),
            potion: Some(PotionCloud {
                settings: CloudSettings {
                    wait_time: 10,
                    reapplication_delay: 25,
                    radius_on_use: -0.5,
                    duration_on_use: -5,
                    duration_scale: 0.5,
                },
                effects: vec![("poison", 200, 1), ("slowness", 100, 0)],
                hidden: vec!["slowness"],
                ambient: vec!["poison"],
                radius_per_tick: -0.008_750,
                color: 0x0033_FF44,
                custom_particle: None,
                victims: Vec::new(),
                source: Text::translate("entity.minecraft.area_effect_cloud", &[]),
                owner: None,
                guest: None,
            }),
        };
        let tag = potion_cloud_to_nbt(&cloud, [1, 2, 3, 4], None);
        let (at, back) = potion_cloud_from_nbt(&tag).expect("the cloud reads back");
        assert_eq!(at, Vec3::new(1.5, 64.0, -2.5), "the position");
        assert_eq!(back.motion, [0.6, 0.0, 0.0], "the motion");
        assert_eq!((back.age, back.duration), (12, 400));
        assert!((back.radius - 3.5).abs() < 1.0e-6, "{}", back.radius);
        let potion = back.potion.expect("a potion cloud");
        assert_eq!(
            (
                potion.settings.wait_time,
                potion.settings.reapplication_delay,
                potion.settings.duration_on_use
            ),
            (10, 25, -5)
        );
        assert!((potion.settings.radius_on_use + 0.5).abs() < 1.0e-6);
        assert!((potion.settings.duration_scale - 0.5).abs() < 1.0e-6);
        assert!((potion.radius_per_tick + 0.008_750).abs() < 1.0e-6);
        assert_eq!(
            potion.effects,
            vec![("poison", 200, 1), ("slowness", 100, 0)]
        );
        assert_eq!(potion.hidden, vec!["slowness"]);
        assert_eq!(potion.ambient, vec!["poison"]);
        assert_eq!(potion.color, 0x0033_FF44, "the colour it was drawn in");
        // The vanilla field names the reader of another client would look for.
        for key in [
            "Age",
            "Duration",
            "Radius",
            "RadiusPerTick",
            "WaitTime",
            "ReapplicationDelay",
            "RadiusOnUse",
            "DurationOnUse",
            "potion_duration_scale",
            "potion_contents",
        ] {
            assert!(tag.get(key).is_some(), "{key}");
        }
        assert_eq!(
            sunrise_client::entity_nbt::kind(&tag).as_deref(),
            Some("minecraft:area_effect_cloud")
        );
    }

    /// `WitherBoss.canBeAffected` refuses only the wither effect: its `addEffect`
    /// refuses the rest, but a cloud of strength or healing still counts the boss
    /// as a victim (`AreaEffectCloud.serverTick` asks `canBeAffected`).
    #[test]
    fn a_cloud_still_counts_the_wither_as_a_victim() {
        let tags = sunrise_client::items::EntityTypeTags::default();
        for effect in ["strength", "instant_health"] {
            assert!(
                mob_can_be_affected(&tags, "minecraft:wither", None, effect),
                "{effect}"
            );
        }
        assert!(!mob_can_be_affected(
            &tags,
            "minecraft:wither",
            None,
            "wither"
        ));
    }

    /// #5876: a potion thrown straight up from inside its thrower's box does
    /// not hit them while it climbs out, then hits them on the way down;
    /// another player in its path is hit at once.
    #[test]
    fn a_thrown_potion_hits_its_thrower_only_after_leaving_them() {
        // A player's box: 0.6 wide, 1.8 high, feet at the origin.
        let body = |x: f64| Aabb::new(Vec3::new(x - 0.3, 0.0, -0.3), Vec3::new(x + 0.3, 1.8, 0.3));
        let thrower = Who::Mob(1);
        let players = [(thrower, body(0.0)), (Who::Mob(2), body(5.0))];
        let ridden = [body(0.0)];
        let eye = Vec3::new(0.0, 1.52, 0.0);
        let up = Vec3::new(0.0, 0.5, 0.0);
        let m = thrown_margin(10);
        assert!((m - 0.3).abs() < 1e-12 && thrown_margin(1) == 0.0);
        // Thrown from the eye: still within a block of its thrower.
        assert!(!thrown_left_owner(&ridden, eye, up));
        assert!(thrown_player_at(&players, &[thrower], eye, m).is_none());
        // Without the gate the thrower's box does hold it.
        assert_eq!(
            thrown_player_at(&players, &[], eye, m).map(|t| t.0),
            Some(thrower)
        );
        // Four blocks up and rising: clear of the box grown by a block.
        let high = Vec3::new(0.0, 5.0, 0.0);
        assert!(thrown_left_owner(&ridden, high, up));
        // Falling back (left_owner now set), the thrower is hit at their head.
        let head = Vec3::new(0.0, 2.0, 0.0);
        assert_eq!(
            thrown_player_at(&players, &[], head, m).map(|t| t.0),
            Some(thrower)
        );
        // Another player is hit though the thrower has not left; a miss is none.
        let beside = Vec3::new(5.2, 1.0, 0.0);
        assert_eq!(
            thrown_player_at(&players, &[thrower], beside, m).map(|t| t.0),
            Some(Who::Mob(2))
        );
        assert!(thrown_player_at(&players, &[thrower], Vec3::new(2.5, 1.0, 0.0), m).is_none());
        // A passenger sharing the thrower's vehicle is skipped like the thrower.
        assert!(thrown_player_at(&players, &[thrower, Who::Mob(2)], beside, m).is_none());
        // The vehicle's box keeps it from leaving: a mount 2 blocks wide.
        let mount = Aabb::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        let beside_mount = Vec3::new(1.9, 0.0, 0.0);
        assert!(thrown_left_owner(&[body(0.0)], beside_mount, up));
        assert!(!thrown_left_owner(&[body(0.0), mount], beside_mount, up));
        // No thrower (a dispenser's potion): it has left them already.
        assert!(thrown_left_owner(&[], eye, up));
    }

    /// A server's client (multiplayer and the default single player) fades
    /// the darkness effect in over its 22 blend ticks, holds it, and fades
    /// it out over the last 22 (#3393).
    #[test]
    fn a_servers_client_blends_the_darkness_effect() {
        let mut effects = PlayerEffects::default();
        effects.add(EffectInstance::new("darkness", 60, 0));
        let mut darkness = (0.0, 0.0);
        client_tick_effects(&mut effects, &mut darkness);
        assert_eq!(darkness, (0.0, 1.0 / 22.0), "the first tick fades in");
        for _ in 1..22 {
            client_tick_effects(&mut effects, &mut darkness);
        }
        assert!((darkness.1 - 1.0).abs() < 1e-5, "{darkness:?}");
        // 38 ticks left: full until 22 remain, then the fade out.
        for _ in 0..16 {
            client_tick_effects(&mut effects, &mut darkness);
        }
        assert_eq!(effects.get("darkness").map(|e| e.duration), Some(22));
        assert!(
            darkness.1 < darkness.0,
            "fading out at 22 left: {darkness:?}"
        );
        for _ in 0..21 {
            client_tick_effects(&mut effects, &mut darkness);
        }
        assert!(darkness.1.abs() < 1e-5, "{darkness:?}");
    }

    /// `ClientPacketListener.handleUpdateMobEffect`: a new darkness fades in
    /// when the server blends it; one sent unblended (joining, changing
    /// dimension) is at full strength at once.
    #[test]
    fn a_new_darkness_starts_blended_or_at_once() {
        let e = EffectInstance::new("darkness", 260, 0);
        assert_eq!(darkness_blend_added(&e, true), (0.0, 0.0));
        assert_eq!(darkness_blend_added(&e, false), (1.0, 1.0));
        let ending = EffectInstance::new("darkness", 10, 0);
        assert_eq!(darkness_blend_added(&ending, false), (0.0, 0.0));
        let infinite = EffectInstance::new("darkness", -1, 0);
        assert_eq!(darkness_blend_added(&infinite, false), (1.0, 1.0));
    }

    /// `ClientPacketListener.handleUpdateMobEffect`: a new nausea fades in
    /// when the server blends it; one sent unblended (joining, changing
    /// dimension) is at full strength at once, unless it is already within
    /// its blend-out ticks.
    #[test]
    fn a_new_nausea_starts_blended_or_at_once() {
        let e = EffectInstance::new("nausea", 600, 0);
        assert_eq!(nausea_blend_added(&e, true), (0.0, 0.0));
        assert_eq!(nausea_blend_added(&e, false), (1.0, 1.0));
        let ending = EffectInstance::new("nausea", 40, 0);
        assert_eq!(nausea_blend_added(&ending, false), (0.0, 0.0));
        let infinite = EffectInstance::new("nausea", -1, 0);
        assert_eq!(nausea_blend_added(&infinite, false), (1.0, 1.0));
    }

    /// `BottleItem.use`: collecting dragon's breath from an ender dragon's cloud (#8054).
    #[test]
    fn glass_bottle_collects_dragon_breath_from_dragon_cloud() {
        let player_box = Aabb::new(Vec3::new(-0.3, 64.0, -0.3), Vec3::new(0.3, 65.8, 0.3));
        let mut clouds = [super::super::dragon::Breath {
            attachment_lifetime: Default::default(),
            join: Default::default(),
            id: 1,
            at: [1.0, 64.0, 0.0],
            motion: [0.0; 3],
            fall_distance: 0.0,
            fire_ticks: 0,
            first_tick: true,
            radius: 5.0,
            age: 0,
            duration: 200,
            from_dragon: true,
            victims: Vec::new(),
            potion: None,
        }]
        .into_iter()
        .collect();
        assert!(collect_dragon_breath(player_box, &mut clouds).is_some());
        assert_eq!(clouds[0].radius, 4.5);
        assert!(collect_dragon_breath(player_box, &mut clouds).is_some());
        assert_eq!(clouds[0].radius, 4.0);
    }

    /// `BottleItem.use`: clouds that are expired, non-dragon, lingering potions,
    /// or out of reach cannot be bottled (#8054).
    #[test]
    fn glass_bottle_ignores_non_dragon_or_expired_or_out_of_range_clouds() {
        let player_box = Aabb::new(Vec3::new(-0.3, 64.0, -0.3), Vec3::new(0.3, 65.8, 0.3));
        // Non-dragon cloud (`from_dragon: false`).
        let mut non_dragon = [super::super::dragon::Breath {
            attachment_lifetime: Default::default(),
            join: Default::default(),
            id: 1,
            at: [0.0, 64.0, 0.0],
            motion: [0.0; 3],
            fall_distance: 0.0,
            fire_ticks: 0,
            first_tick: true,
            radius: 3.0,
            age: 0,
            duration: 600,
            from_dragon: false,
            victims: Vec::new(),
            potion: None,
        }]
        .into_iter()
        .collect();
        assert!(collect_dragon_breath(player_box, &mut non_dragon).is_none());
        assert_eq!(non_dragon[0].radius, 3.0);

        // Lingering potion cloud (`potion: Some(...)`).
        let mut lingering = [super::super::dragon::Breath {
            attachment_lifetime: Default::default(),
            join: Default::default(),
            id: 2,
            at: [0.0, 64.0, 0.0],
            motion: [0.0; 3],
            fall_distance: 0.0,
            fire_ticks: 0,
            first_tick: true,
            radius: 3.0,
            age: 0,
            duration: 600,
            from_dragon: true,
            victims: Vec::new(),
            potion: Some(PotionCloud {
                settings: CloudSettings::lingering(),
                effects: Vec::new(),
                hidden: Vec::new(),
                ambient: Vec::new(),
                radius_per_tick: -3.0 / 600.0,
                color: 0,
                custom_particle: None,
                victims: Vec::new(),
                source: Text::empty(),
                owner: None,
                guest: None,
            }),
        }]
        .into_iter()
        .collect();
        assert!(collect_dragon_breath(player_box, &mut lingering).is_none());
        assert_eq!(lingering[0].radius, 3.0);

        // Its last tick before the wait-shifted end still holds breath
        // (`tickCount - waitTime < duration`).
        let mut last = [super::super::dragon::Breath {
            attachment_lifetime: Default::default(),
            join: Default::default(),
            id: 3,
            at: [0.0, 64.0, 0.0],
            motion: [0.0; 3],
            fall_distance: 0.0,
            fire_ticks: 0,
            first_tick: true,
            radius: 5.0,
            age: 219,
            duration: 200,
            from_dragon: true,
            victims: Vec::new(),
            potion: None,
        }]
        .into_iter()
        .collect();
        assert!(collect_dragon_breath(player_box, &mut last).is_some());
        assert_eq!(last[0].radius, 4.5);

        // Expired cloud (`tickCount - waitTime >= duration`).
        let mut expired = [super::super::dragon::Breath {
            attachment_lifetime: Default::default(),
            join: Default::default(),
            id: 3,
            at: [0.0, 64.0, 0.0],
            motion: [0.0; 3],
            fall_distance: 0.0,
            fire_ticks: 0,
            first_tick: true,
            radius: 5.0,
            age: 220,
            duration: 200,
            from_dragon: true,
            victims: Vec::new(),
            potion: None,
        }]
        .into_iter()
        .collect();
        assert!(collect_dragon_breath(player_box, &mut expired).is_none());
        assert_eq!(expired[0].radius, 5.0);

        // Out of horizontal reach: cloud at 10 blocks away with radius 1:
        // cloud X range [9.0, 11.0], player inflated X range [-2.3, 2.3] -> no overlap.
        let mut far = [super::super::dragon::Breath {
            attachment_lifetime: Default::default(),
            join: Default::default(),
            id: 4,
            at: [10.0, 64.0, 0.0],
            motion: [0.0; 3],
            fall_distance: 0.0,
            fire_ticks: 0,
            first_tick: true,
            radius: 1.0,
            age: 0,
            duration: 200,
            from_dragon: true,
            victims: Vec::new(),
            potion: None,
        }]
        .into_iter()
        .collect();
        assert!(collect_dragon_breath(player_box, &mut far).is_none());
        assert_eq!(far[0].radius, 1.0);

        // Out of vertical reach: cloud at Y=64.0 (Y bounds [64.0, 64.5]),
        // player at Y=67.0 (inflated Y bounds [65.0, 70.8] does not reach 64.5).
        let high_player = Aabb::new(Vec3::new(-0.3, 67.0, -0.3), Vec3::new(0.3, 68.8, 0.3));
        let mut ground_cloud = [super::super::dragon::Breath {
            attachment_lifetime: Default::default(),
            join: Default::default(),
            id: 5,
            at: [0.0, 64.0, 0.0],
            motion: [0.0; 3],
            fall_distance: 0.0,
            fire_ticks: 0,
            first_tick: true,
            radius: 1.0,
            age: 0,
            duration: 200,
            from_dragon: true,
            victims: Vec::new(),
            potion: None,
        }]
        .into_iter()
        .collect();
        assert!(collect_dragon_breath(high_player, &mut ground_cloud).is_none());
        assert_eq!(ground_cloud[0].radius, 1.0);
    }

    /// Real panel painting/hover selection dispatches even for a fully visible
    /// widget. Both inventories retain vanilla lines, suppression and layout rules.
    #[test]
    #[ignore = "daily"]
    fn a_mod_adds_a_line_to_an_effect_tooltip() {
        use mod_api::{self, ClientHost, GatherEffectScreenTooltipsEvent, Hooks, ModEntry, Mods};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        #[derive(Clone, Copy)]
        struct Case {
            room: i32,
            id: &'static str,
            name: &'static str,
            default_tooltip: bool,
        }
        #[derive(Clone)]
        struct Mod {
            screen: &'static str,
            case: Case,
            suppress: bool,
            calls: Arc<AtomicUsize>,
        }
        #[mod_api::__subscriptions]
        impl Hooks for Mod {
            fn gather_effect_screen_tooltips(
                &self,
                _: &dyn ClientHost,
                event: &mut GatherEffectScreenTooltipsEvent<'_>,
            ) {
                self.calls.fetch_add(1, Ordering::Relaxed);
                assert_eq!(
                    event.effect.id,
                    mod_platform::__private::part_tests::effect_key(self.case.id)
                );
                assert_eq!((event.effect.duration, event.effect.amplifier), (450, 1));
                assert_eq!(event.screen.id, self.screen);
                let initial: Vec<_> = event
                    .tooltip
                    .iter()
                    .map(|(text, color)| (text.clone(), *color))
                    .collect();
                if self.case.default_tooltip {
                    assert_eq!(
                        initial,
                        [
                            (mod_api::Text::literal(self.case.name), u32::MAX),
                            (mod_api::Text::literal("00:22"), u32::MAX)
                        ]
                    );
                } else {
                    assert!(initial.is_empty());
                }
                if self.suppress {
                    event.tooltip.clear();
                } else {
                    event
                        .tooltip
                        .push((mod_api::Text::literal("Fast feet"), 0xff55ffff));
                }
            }
        }
        struct HidePanels;
        #[mod_api::__subscriptions]
        impl Hooks for HidePanels {
            fn screen_render_inventory_mob_effects(
                &self,
                _: &dyn ClientHost,
                _: &mut mod_api::ScreenRenderInventoryMobEffectsEvent<'_>,
            ) -> mod_api::Flow {
                mod_api::Flow::Cancel
            }
        }
        super::super::native_entity_refs::tests::with_client_game(|game| {
            for (screen, screen_id, image_width) in [
                (Screen::Inventory, "inventory", 176),
                (Screen::Creative, "creative", 195),
            ] {
                game.side.screen = screen;
                // Fixed panel position; right-side room selects wide or compact.
                // The longer effect name also clips in the smallest wide layout.
                for case in [
                    Case {
                        room: 200,
                        id: "speed",
                        name: "Speed II",
                        default_tooltip: false,
                    },
                    Case {
                        room: 100,
                        id: "speed",
                        name: "Speed II",
                        default_tooltip: true,
                    },
                    Case {
                        room: 120,
                        id: "fire_resistance",
                        name: "Fire Resistance II",
                        default_tooltip: true,
                    },
                ] {
                    game.potions.effects.list = vec![EffectInstance::new(case.id, 450, 1)];
                    let left = 102 + image_width;
                    let origin = (100.0, 50.0);
                    let mut batch = HudBatch::scratch([(left + case.room) as u32, 480]);
                    game.side.cursor = ((left + 7) as f32, 57.0);
                    let vanilla = if case.default_tooltip {
                        vec![
                            (case.name.to_owned(), u32::MAX),
                            ("00:22".to_owned(), u32::MAX),
                        ]
                    } else {
                        Vec::new()
                    };
                    let lines = game
                        .draw_inventory_effects(&mut batch, origin, image_width)
                        .unwrap();
                    assert_eq!(
                        lines
                            .iter()
                            .map(|(text, color)| (text.plain(), *color))
                            .collect::<Vec<_>>(),
                        vanilla
                    );
                    // The existing shared renderer draws nothing for an unchanged
                    // empty list, without running other tooltip hooks.
                    let painted = batch.vertices.len();
                    game.draw_tooltip_lines(
                        &mut batch,
                        &lines,
                        game.side.cursor.0,
                        game.side.cursor.1,
                    );
                    assert_eq!(batch.vertices.len() > painted, case.default_tooltip);

                    for suppress in [false, true] {
                        let calls = Arc::new(AtomicUsize::new(0));
                        let listener = Mod {
                            screen: screen_id,
                            case,
                            suppress,
                            calls: calls.clone(),
                        };
                        let mods: Mods =
                            vec![ModEntry::new("effect-tips", Box::new(listener.clone()))].into();
                        mod_platform::testing::with_mods(mods, || {
                            let lines = game
                                .draw_inventory_effects(&mut batch, origin, image_width)
                                .unwrap();
                            let plain: Vec<_> = lines
                                .iter()
                                .map(|(text, color)| (text.plain(), *color))
                                .collect();
                            let mut expected = vanilla.clone();
                            if suppress {
                                expected.clear();
                            } else {
                                expected.push(("Fast feet".to_owned(), 0xff55ffff));
                            }
                            assert_eq!(plain, expected);
                            assert_eq!(calls.load(Ordering::Relaxed), 1);
                            let painted = batch.vertices.len();
                            game.draw_tooltip_lines(
                                &mut batch,
                                &lines,
                                game.side.cursor.0,
                                game.side.cursor.1,
                            );
                            assert_eq!(batch.vertices.len() > painted, !suppress);

                            // A miss, a hidden layout and too little room must
                            // never gather tooltips, even with a subscriber.
                            game.side.cursor = (0.0, 0.0);
                            assert!(
                                game.draw_inventory_effects(&mut batch, origin, image_width)
                                    .is_none()
                            );
                            game.side.cursor = ((left + 7) as f32, 57.0);
                            let hidden: Mods = vec![
                                ModEntry::new("effect-tips", Box::new(listener.clone())),
                                ModEntry::new("hide-panels", Box::new(HidePanels)),
                            ]
                            .into();
                            mod_platform::testing::with_mods(hidden, || {
                                // Layout hooks run at screen construction, not painting.
                                game.side.frames += 1;
                                let space = crate::game::ClientGame::effects_space(
                                    &batch,
                                    [origin.0, origin.1],
                                    image_width as f32,
                                );
                                let layout = game.inventory_effects_layout(
                                    &space,
                                    [origin.0, origin.1, image_width as f32, 166.0],
                                );
                                let panels = game.effect_panels(&space, &layout);
                                assert!(
                                    inventory_effects::paint(
                                        &mut batch,
                                        &game.gui.atlas,
                                        &panels,
                                        [game.side.cursor.0 as i32, game.side.cursor.1 as i32],
                                    )
                                    .is_none()
                                );
                            });
                            let mut narrow = HudBatch::scratch([(left + 31) as u32, 480]);
                            assert!(
                                game.draw_inventory_effects(&mut narrow, origin, image_width)
                                    .is_none()
                            );
                            assert_eq!(calls.load(Ordering::Relaxed), 1);
                        });
                    }
                }
            }
        });
    }
}

/// `MobEffect.withSoundOnAdded`: the sound `onEffectAdded` plays when the
/// effect is new on an entity (`MobEffects`: the three omens).
pub(in crate::game) fn sound_on_added(effect: &str) -> Option<&'static str> {
    match effect.strip_prefix("minecraft:").unwrap_or(effect) {
        "bad_omen" => Some("minecraft:event.mob_effect.bad_omen"),
        "trial_omen" => Some("minecraft:event.mob_effect.trial_omen"),
        "raid_omen" => Some("minecraft:event.mob_effect.raid_omen"),
        _ => None,
    }
}

#[cfg(feature = "client")]
#[cfg(test)]
#[path = "potions/cloud_water_tests.rs"]
pub(in crate::game) mod cloud_water_tests;
