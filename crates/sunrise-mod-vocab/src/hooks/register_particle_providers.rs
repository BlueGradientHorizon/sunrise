//! The `register_particle_providers` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Register sprite-set factories at client startup (NeoForge's `RegisterParticleProvidersEvent`).
    /// Call [`RegisterParticleProvidersEvent::register_sprite_set`] with the ID used for
    /// `Registrar::declare_particle`. The accepted factory binds when that type's sprites load,
    /// and its `ParticleProvider` creates one owned `CustomParticle` per successful spawn.
    #[notes = r#"## Particle providers

Use `event.register_sprite_set(id, factory)` at client startup with the ID registered through
`Registrar::declare_particle` and the matching `assets/<namespace>/particles/<path>.json`.
Type declaration is independent of client resources; the description is required only to bind the
client sprite-set factory.
The last accepted factory for an ID binds once per sprite load on the logical client. The
`ParticleSpriteSet` uses the atlas missing sprite in each unresolved frame, or in one frame if
the description is empty. A missing description or failed bind is reported and refuses native
spawns. A pack's Polytone definition still takes precedence when it overrides the type.
See the modding book's “Particle providers” for a worked example.

Once the applicable distance and Particles-setting checks pass, `ParticleProvider::create_particle`
receives `ParticleSpawn` (typed options, raw position and velocity slots, live level and random)
and a mutable `ParticleTickView` seeded with the declared spawn values. Return `None` to decline
just this spawn without an error. A returned `Box<dyn CustomParticle>` is queued for ordinary
FIFO group admission. Local calls retain their typed option fields. The unchanged named
network encoding carries only the registered particle ID, so clients reconstruct default fields.

An instance holds its state in its own fields. On its owning thread, `CustomParticle::tick` can
use `tick_base` for vanilla motion and expiry, or `advance_age` to record the previous position and
roll and check expiry before applying custom motion. These operations remove the particle when
the age at the start of the tick reaches its lifetime. `remove` retires it explicitly. Select an
exact frame with `set_sprite_index` or animate with `set_sprite_from_age`. Invalid setter inputs
are reported and leave the field unchanged. The particle and level views are scoped to the call.

`CustomParticle::extract` reads the live pose and edits only this frame's `ParticleQuad`; the
engine interpolates the pose and uses the configured sprite, lighting and render group.
Hiding the quad, refusing the callback or producing non-finite geometry skips a draw, not a tick.
A panic is named in the log and client error toast and disables that registration's panicking
callback kind for the session. Ordinary refusals are reported without latching. Reloading sprites
discards existing native instances and binds new providers."#]
    #[hook(
        receiver = "associated",
        dispatch = "flat",
        side = "client",
        contract = "collect",
        out = "providers",
        context = "registration",
        answered_by = "mod",
        registration_append = "custom",
        event_per_listener = true,
        neoforge = "RegisterParticleProvidersEvent",
        version = 2
    )]
    fn register_particle_providers(providers: &mut Vec<Answered<ParticleProviderRegistration>>);
}
