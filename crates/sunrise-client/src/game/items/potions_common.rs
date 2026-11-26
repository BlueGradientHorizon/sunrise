use crate::game::dragon_common::FightWho;
use sunrise_core::{geometry::Vec3, nbt::Tag, random::LegacyRandom};
use sunrise_game_data::text::Text;
use sunrise_items::items::ItemStack;
use sunrise_level::{
    effects::PlayerEffects,
    particle_options::{ParticleOptions, ParticleType},
};
use sunrise_sim::mobs::Who;

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

/// An entity a cloud has affected (`AreaEffectCloud.victims`).
#[derive(Clone, Copy, PartialEq)]
pub(in crate::game) enum Victim {
    Mob(u32),
    Player(FightWho),
}

/// `AreaEffectCloud` wait time set by `ThrownLingeringPotion.onHitAsPotion`.
pub(in crate::game) const LINGERING_WAIT: i32 = 10;

/// `AreaEffectCloud.DEFAULT_REAPPLICATION_DELAY`.
pub(in crate::game) const REAPPLICATION_DELAY: i32 = 20;

/// The entity type of a thrown potion or bottle.
fn thrown_kind(stack: &ItemStack) -> &'static str {
    match &*stack.item {
        "minecraft:lingering_potion" => "minecraft:lingering_potion",
        "minecraft:experience_bottle" => "minecraft:experience_bottle",
        _ => "minecraft:splash_potion",
    }
}

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

impl PotionState {
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
    /// A server's move: thrown potions have no interpolation handler, so
    /// `moveOrInterpolateTo` puts them there at once. False when `id` isn't one.
    pub(in crate::game) fn remote_thrown_pose(&self, id: u32) -> Option<([f64; 3], [f64; 3])> {
        let t = self.thrown.iter().find(|t| t.id == id)?;
        Some((
            [t.position.x, t.position.y, t.position.z],
            [t.previous.x, t.previous.y, t.previous.z],
        ))
    }
    pub(in crate::game) fn has_thrown(&self, id: u32) -> bool {
        self.thrown.iter().any(|t| t.id == id)
    }
}
