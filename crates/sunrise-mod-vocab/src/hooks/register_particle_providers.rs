//! The `register_particle_providers` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Register sprite-set factories for code-defined particle providers (NeoForge's
    /// `RegisterParticleProvidersEvent`). Called once at client startup. Use
    /// [`RegisterParticleProvidersEvent::register_sprite_set`] with the particle type's ID and a
    /// factory that builds the provider after the matching sprite set loads.
    #[notes = r#"## Particle providers

A particle provider (NeoForge's `RegisterParticleProvidersEvent` and `ParticleProvider`) gives a registered
particle type code. `Hooks::register_particle_providers` stages its sprite-set factory at client startup.
The last accepted factory for an ID binds once per sprite load, with a `ParticleSpriteSet` describing
that load's nonempty frame list. See the modding book's “Particle providers” for an implementation.

After the distance and Particles settings checks, the bound provider's `create_particle` receives a
`ParticleSpawn` with the typed `ParticleOptions`, raw position and velocity slots, a read-only live level
and the engine's provider random. A `ParticleTickView` starts with the declared spawn values, and changes
are authoritative. Return `None` to decline a spawn, or a `Box<dyn CustomParticle>` to queue it for normal
FIFO group admission. The particle's own fields hold its state.

The instance ticks on the owning thread, with the same live particle and level views lent only for the
callback. Its default tick runs vanilla's base movement, interpolation and lifetime check. An override can
call `tick_base`, or `advance_age` and then run its own movement, or `remove` to expire the particle. Sprite
selection is explicit: `set_sprite_index` selects a frame directly and `set_sprite_from_age` uses vanilla's
age/lifetime formula. Bad inputs to setters are reported and leave the field unchanged, not the particle dead.

A missing sprite set or failed bind spawns nothing. A panic is named in the log and client error toast;
only the panicking callback kind for that registration is disabled, including across resource reloads.
An ordinary phase refusal does not disable the callback. Reloads discard existing native instances."#]
    #[hook(
        receiver = "associated",
        dispatch = "flat",
        side = "client",
        contract = "collect",
        out = "providers",
        context = "registration",
        answered_by = "mod",
        registration_append = "custom",
        neoforge = "RegisterParticleProvidersEvent",
        version = 2
    )]
    fn register_particle_providers(providers: &mut Vec<Answered<ParticleProviderRegistration>>);
}
