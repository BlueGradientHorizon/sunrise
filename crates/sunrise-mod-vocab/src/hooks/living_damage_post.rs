//! The `living_damage_post` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// NeoForge's `LivingDamageEvent.Post`: entity `entity` of type `kind` has been hurt by the
    /// hit `container` (a read-only live [`DamageContainerView`]) describes, with its final
    /// values (health damage, the reductions, the post-attack invulnerability ticks). It fires
    /// once for every hit that passed `living_incoming_damage` and the invulnerability cooldown,
    /// at the end of `hurtServer`: after the hurt sounds and after that hit's death processing
    /// (even if vetoed) or totem protection. For a queued mob effect hurt this is before the next
    /// queued action, and for a dolphin's dry-out hurt it is before the dolphin's land hop.
    /// A server player's landing hurt finishes before the landing's `HIT_GROUND` and fall reset,
    /// and before the next accepted move.
    /// `container.inflicted()` is the amount before absorption;
    /// `container.health_damage()` is the amount applied to health. A hit that took no
    /// health (a shield, absorption, armor or a `living_damage_pre` listener took it all) fires
    /// it too, with `container.health_damage()` 0. `container.should_cause_side_effects()`
    /// carries `hurtServer`'s fresh-hit decision (`shouldCauseSideEffects`): false for a stronger
    /// hit inside the cooldown unless its source bypasses it, independent of the final damage
    /// or modded invulnerability ticks. It only listens. It can't write the
    /// entity or the world's blocks: the host (`HitHost`) gives the hit's random and level
    /// attachments, level sounds, and the own random of the entities the hit lends.
    #[hook(
        dispatch = "flat",
        side = "server",
        contract = "listen",
        host = "hit",
        neoforge = "LivingDamageEvent.Post",
        version = 3,
        target = "sunrise_client::game::native_hooks::living_damage_post::HitLevel<'_>::native_living_damage_post"
    )]
    fn living_damage_post(
        entity: i32,
        kind: &str,
        source: &HurtSource<'_>,
        container: mod_api::DamageContainerView<'_>,
    );
}
