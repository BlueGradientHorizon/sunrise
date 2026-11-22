//! The `register_particle_providers` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Register sprite-set factories for code-defined particle providers (NeoForge's
    /// `RegisterParticleProvidersEvent`). Called once at client startup. Use
    /// [`RegisterParticleProvidersEvent::register_sprite_set`] with the particle type's ID and a
    /// factory that builds the provider after the matching sprite set loads.
    #[notes = r#"## Particle providers

A particle provider (NeoForge's `RegisterParticleProvidersEvent` and `ParticleProvider`) gives a mod's own
particle type code. `Hooks::register_particle_providers` stages factories at client startup, before resources
load. The last registration for an ID wins. On initial load and each reload, the game binds the winning factory
once with a `ParticleSpriteSet` describing that load's nonempty set. Use `frame_count()` to read its number of
frames; the descriptor exposes no atlas or engine handle. Particle ticks dispatch to the bound provider with no
provider-name lookup or scan over the mods.

```rust,ignore
struct Orbit;
impl ParticleProvider for Orbit {
    fn tick(&self, particles: &mut ParticleBatch, _level: &dyn ParticleLevelView) {
        for particle in particles {
            let t = f64::from(particle.age) * 0.4;
            particle.position = [
                particle.state[0] + t.cos(),
                particle.state[1] + f64::from(particle.age) * 0.05,
                particle.state[2] + t.sin(),
            ];
            particle.rgba[3] = 1.0 - particle.age as f32 / particle.lifetime as f32;
            particle.facing = ParticleFacing::AroundY;
            if particle.age > 40 {
                particle.removed = true;
            }
        }
    }
}

fn register_particle_providers(
    &self,
    _context: &RegistrationContext,
    event: &mut RegisterParticleProvidersEvent<'_>,
) {
    event.register_sprite_set(Identifier::parse("example:orbit").unwrap(), |_sprites| Orbit);
}
```

The particle is built as a declarative one is: `ParticleOptions::simple(key)`, with the key
`Registrar::particle_key("example:orbit")` issued, draws the sprite set
of `assets/example/particles/orbit.json`, with the spawn values of its `sunrise:behaviour` (lifetime, size, alpha,
velocity). The engine keeps the sprite handling and the camera-distance, Particles-setting and population limits;
only the motion is the provider's. Before each `tick` the game advances `age` (1 on the first tick) and sets
`previous`, and it expires the particle once `age` passes `lifetime`, or after a tick that sets `removed` or
leaves a number that is not finite. The particle draws on the blended layer, so `rgba[3]` may fade it, unless a `register_particle_groups`
group lists its type: it then draws in that group, on the group's blend.
`position`, `frame` (a sprite of the set, clamped), `size`, `rgba`, `lifetime`, `removed`, `facing` and `roll` are the tick's to
write. `facing` is `Camera` (vanilla's `LOOKAT_XYZ`), `AroundY` (`LOOKAT_Y`: the camera's `y` and `w` alone) or
`Fixed(quaternion)`, a world-fixed orientation such as a quad lying flat; `roll` turns the quad about its normal and is
interpolated from the previous tick's (no turn is drawn while it is zero), and a roll beyond half of `f32::MAX` expires the
particle. `state` is four numbers that stay with the particle, zero at spawn; it is the only per-particle data,
since every particle of a type shares the one provider. `tick` is one call per type per tick with every live
particle of the type, and is not called when none is alive. Dispatch reuses its batch storage once it has
enough capacity. A tick has no `Host`, since the engine ticks particles below gameplay, but it reads the
level through `ParticleLevelView`: `block_state(pos)` and `brightness(layer, pos)`, both read-only. Light follows
what the render level stores and the sky layer always reads zero in a dimension without skylight. The game asks for
block states and builds the view only when a provider has a particle to tick; a registered provider with none alive
is not visited. A type with no registration keeps its declarative motion. If a registered type's sprite set
is missing or its factory cannot bind, the game reports the type and its mod and spawns no particle for it.
A provider callback panic is logged with its mod and shown in the error toast; that registration is disabled
for the session. Live provider particles are dropped when resources reload, as a compiled controller's are."#]
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
