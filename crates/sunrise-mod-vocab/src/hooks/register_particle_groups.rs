//! The `register_particle_groups` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Register the render groups a mod's particles draw in (NeoForge's `RegisterParticleGroupsEvent`).
    /// Called once at client startup. Use [`RegisterParticleGroupsEvent::register`] to bind an ID to
    /// a factory run once per particle engine construction. Sprite reload binds new particle
    /// providers but resolves their types against the existing groups.
    #[notes = r#"## Particle groups

`Hooks::register_particle_groups` is the client registration-phase counterpart of NeoForge
26.3's `RegisterParticleGroupsEvent`. It receives a `RegistrationContext` once at client
startup. Call `event.register` with a namespaced ID and a factory. The factory returns a
`ParticleGroup` with a `ParticleBlend` (`Opaque` or `Translucent`) and the particle types
that draw in it.

```rust,ignore
fn register_particle_groups(
    &self,
    _context: &RegistrationContext,
    event: &mut RegisterParticleGroupsEvent<'_>,
) {
    event.register(Identifier::parse("example:glow").unwrap(), || ParticleGroup {
        blend: ParticleBlend::Translucent,
        particles: vec!["example:orbit".into(), "example:spark".into()],
    });
}
```

A group is vanilla's `ParticleRenderType` with the `ParticleGroup` the particle engine keeps for
it. Each group uses vanilla's `ParticleGroup.add` admission rules and is ticked and drawn apart
from other groups. Custom groups draw after the built-in particles, in registration order.
A group picks its particles' layer (`blend`), not their motion. List each type by its namespaced
ID, such as `example:spark` for `assets/example/particles/spark.json`. The group admits both
particles with declarative motion and instances made by a bound native provider. A type no group
lists draws in the built-in group; the built-in particles are never moved. A type another group
already lists, or with no matching sprite set, is logged and left out.

The game freezes factories in registration order. IDs in `minecraft`, IDs with an empty
namespace or path, and duplicates are logged and dropped without invoking those factories.
Each particle engine calls the accepted factories once when it is constructed. If a factory
is disabled, refuses or panics, that engine omits the group; a duplicate never takes its place.
When sprite sets load or reload, the game resolves each type's group from the resulting
engine-owned definitions. Spawning, ticking and drawing use the resolved group handle, with no
lookup by group name. Sprite reload discards live and pending native instances in all groups
before binding the new providers."#]
    #[hook(
        receiver = "associated",
        dispatch = "flat",
        side = "client",
        contract = "collect",
        out = "groups",
        context = "registration",
        answered_by = "mod",
        registration_append = "custom",
        neoforge = "RegisterParticleGroupsEvent",
        version = 2
    )]
    fn register_particle_groups(groups: &mut Vec<Answered<ParticleGroupRegistration>>);
}
