# Client particles and movement queries

## Client particles

`ClientHost::add_particle` submits the API's `ParticleOptions`, a world position and
velocity to the ordinary client particle queue. The client must have a loaded player
and world. Non-finite numeric inputs are refused. A successful submission does not
promise a visible particle: provider availability, sprite sets, distance and the
player's particle settings apply. This method is not available on `ServerHost`.

Particle options carry a `ParticleKey`, not an engine particle enum or a provider name.
Request the key with `Registrar::particle_key` during registration and retain it. The game resolves
provider identities once when content loads. `ParticleOptions::simple(key)` handles both vanilla
and custom sprite providers; custom providers require an `assets/<namespace>/particles/<path>.json`
sprite set. All option constructors take the resolved key, including `dust`, `item`, `trail`,
`vibration`, `shriek`, `sculk_charge` and `dust_transition`. Use the corresponding vanilla key
for these parameter families. Sending an option performs no string parsing or registry scan.

For stack-derived breaking or eating particles, call
`ClientHost::item_particle_options(&stack)` and pass the returned options to
`add_particle`. This read resolves the ground model with seed 0, preserving all
layer particle sprites and resource-pack and stack-component overrides. It returns
`None` for invalid stacks or unavailable resources/provider. It is available in
Setup, Reloading and World. Resolve again after resource reloads.
It allocates a sprite list only when called; it adds no per-tick work.

### Tinting an item crumb

The item crumbs (`item` and its `item_*` variants) multiply their red, green and blue by
`ParticleOptions::color` when they spawn and ignore its alpha; the default white changes
nothing, and `item_particle_options` results and server `item_stack` sends stay untinted.
A mod sets it with `with_color`, and a block-atlas sprite such as a fluid's still sprite is a
valid layer, so a fluid crumb is the sprite tinted by the fluid:

```rust,ignore
let crumb = ParticleOptions::item(item_key, vec!["minecraft:block/water_still".into()])
    .with_color([0.8 * r, 0.8 * g, 0.8 * b, 1.0]);
host.add_particle(&crumb, at, velocity);
```

An item crumb's colour must be finite with every component in `0.0..=1.0`; `add_particle`
returns `false` otherwise and nothing is clamped. The crumb keeps its small random spread of
velocity and the level's light.

### Particle providers

A particle provider (`Hooks::register_particle_providers`) gives a registered particle type its own
construction, tick behavior and per-frame quad. The type is named by its ID (`mymod:spark`); its sprite set and
spawn defaults come from `assets/<namespace>/particles/<path>.json`. Make a request with
`ParticleOptions::simple` (or another typed option constructor) on the key returned by
`Registrar::particle_key`. Register a factory on the client at startup:

```rust,ignore
struct SparkProvider;
struct Spark { rise: f64 }

impl ParticleProvider for SparkProvider {
    fn create_particle(
        &self,
        spawn: &mut ParticleSpawn<'_>,
        particle: &mut dyn ParticleTickView,
    ) -> Option<Box<dyn CustomParticle>> {
        particle.set_lifetime(20);
        Some(Box::new(Spark { rise: spawn.velocity[1] }))
    }
}

impl CustomParticle for Spark {
    fn tick(&mut self, particle: &mut dyn ParticleTickView, _level: &dyn ParticleLevelView) {
        if particle.advance_age() {
            particle.move_by([0.0, self.rise, 0.0]);
            particle.set_sprite_from_age();
        }
    }

    fn extract(&self, particle: &dyn ParticleView, partial_tick: f32, quad: &mut ParticleQuad) {
        // Grow during the first tick, without changing the particle's stored size.
        if particle.age() == 0 {
            quad.size *= partial_tick;
        }
    }
}

fn register_particle_providers(
    &self,
    _context: &RegistrationContext,
    event: &mut RegisterParticleProvidersEvent<'_>,
) {
    event.register_sprite_set(Identifier::parse("mymod:spark").unwrap(), |_| SparkProvider);
}
```

The factory captures owned `Send + Sync + 'static` data and returns a `ParticleProvider`.
The last accepted registration for an ID wins. Its factory runs once per sprite load, including
reloads, with a `ParticleSpriteSet` whose `frame_count()` reports the number of frames. No atlas
or engine handles are exposed. A missing set or failed bind is reported for the ID and its mod;
that type does not spawn. Types without a registration still use their declarations.

After the camera-distance and Particles-setting checks, `create_particle` runs synchronously.
`ParticleSpawn` borrows the original typed options, raw position and velocity, the loaded level
and the engine's live provider RNG. The mutable `ParticleTickView` starts with the declaration's
spawn defaults. Return `None` to decline this spawn, or return a fresh owned instance with its
state in its own fields. It joins the ordinary FIFO; the render group can still reject it when
admitting particles. The views are loans for that call only, not values to retain in the instance.

The default `CustomParticle::tick` calls `tick_base` for vanilla's lifetime check, movement and
friction. If you override it, call `tick_base` or use `advance_age` to capture the previous pose
and check the lifetime before applying your own motion. Age starts at zero and the base check
removes the particle when its age at the start of the tick reaches its lifetime. Call `remove`
to end it sooner. `move_by` uses the collision box and loaded level; `set_position` relocates the
particle with its collision box, without changing the previous position used for interpolation.

Use `set_sprite_index` for an exact frame or `set_sprite_from_age` for vanilla's
`age * (frame_count - 1) / lifetime`. An invalid index or a zero divisor is reported and leaves
the sprite unchanged. Malformed inputs to other setters are reported without changing their
field or expiring the particle. `ParticleFacing::Camera` faces the camera; `AroundY` uses the
camera quaternion's Y and W components alone; `Fixed([x, y, z, w])` sets a world orientation.
`set_roll` turns the quad about its own normal, interpolated from the previous roll when the
current roll is nonzero.

`CustomParticle::extract` receives a read-only `ParticleView`, the frame's partial tick and a
mutable `ParticleQuad` on the owning thread. The default draws the configured appearance.
Override it to change this frame's size, RGBA, facing, roll or visibility. The sprite, light and
render group still come from the particle. Colors use vanilla's `ARGB.colorFromFloat` (floor
and mask each channel, without clamping). Fixed quaternions written to the quad are used as
supplied. Hiding the quad, refusing the callback or producing non-finite vertices skips just
that draw: it does not change the instance, tick state or lifetime. Update those in `tick` instead.

`ParticleLevelView` reads loaded `block_state(pos)` and stored `brightness(layer, pos)` during
construction and tick. A missing chunk reads as air; the sky layer always reads zero in a
dimension without sky light. The instance ticks on the particle engine's owning thread. A panic
is reported in the log and error toast with the mod's name and disables that registration's
panicking callback kind for the session; other callbacks continue. An extraction panic does not
stop ticks. A phase refusal is reported without disabling the callback. Existing native instances are discarded when sprites reload.

### Particle render groups

In `Hooks::register_particle_groups`, call `event.register(id, factory)` with a namespaced
`Identifier` and a factory returning `ParticleGroup { blend, particles }`. The hook runs once at
client startup; each accepted factory runs once per particle engine construction. The first
registration for an ID wins, and a duplicate or a reserved `minecraft` ID never calls its factory.
Reloading resources re-associates sprite sets with the groups already built; it does not call
factories again. Provider registration is separate: a group chooses the draw layer and quota for
the listed particle types, not how those particles move.

## Client fireworks

`ClientHost::create_fireworks` draws a firework's explosions from a `FireworksRequest`: the
position, the rocket's velocity carried by the sparks, a list of `FireworkExplosion`s (shape,
colours, fade colours, trail, twinkle) and `audible`. It uses the starter a bursting rocket
uses, so the explosions follow each other every other tick as a rocket's do. With `audible`
false the sparks and flash draw but the blast and twinkle sounds don't play, as for a
silent rocket. Nothing reaches the server or other players: a mod's server half tells each
client half to draw it. The request is refused on a server, without a loaded client player
and world, for non-finite numbers and for more than 256 explosions.

## Bot movement and block collisions

`PlayerRef::simulate_move` predicts one server player's movement tick from
`PlayerInput`, using the game's movement implementation without replacing the
live player's body. It returns `SimulatedMove` with position, velocity, ground,
horizontal-collision and sprinting results. It refuses stale or inaccessible players,
dead players, spectators, riders and invalid numeric input. A supplied velocity is
input to the prediction, not a write to the player.

`LevelRef::collision_boxes` reads block collision boxes within a finite, ordered
axis-aligned box. It excludes entity and border collisions. A refused level or a
query covering more than 1,000,000 cells returns `None`; a valid query with no
collisions returns `Some` containing an empty vector. Keep queries bounded rather
than scanning a whole level each tick.

`LevelRef::block_shape(host, state, pos, kind)` reads an explicit `BlockState`'s
outline or collision boxes with an empty context at a position in that level.
The supplied state can differ from the world's block, without changing it;
level-dependent shape inputs come from the actual level. To query the
stored block, first read `level.block_state(host, pos)` and pass that state.
This server read returns block-local boxes, or `None` for a
stale handle, closed level or client. Parked levels are supported.
