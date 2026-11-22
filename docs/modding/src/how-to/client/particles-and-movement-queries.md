# Client particles and movement queries

## Client particles

`ClientHost::add_particle` submits the API's `ParticleOptions`, a world position and
velocity to the ordinary client particle queue. The client must have a loaded player
and world. Non-finite numeric inputs are refused. A successful submission does not
promise a visible particle: distance, the player's particle settings, provider availability
and render-group admission still apply. A native provider can also decline the spawn.
This method is not available on `ServerHost`.

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

A native provider constructs one owned instance per spawn of a registered particle type. Give
`Registrar::particle_key("mymod:spark")` the same ID you pass to `register_sprite_set`, and
supply the sprite set in `assets/mymod/particles/spark.json`. Spawn with
`ParticleOptions::simple(spark_key)`. The same registered key, typed options, Wire encoding
and network packets work whether the type uses declarative motion or native code.

This spark rises at the speed supplied by the caller. It starts on the first sprite, switches
to the last sprite halfway through its life, and grows in its first rendered tick without
changing its stored size.

```rust,ignore
use mod_api::{
    CustomParticle, Hooks, Identifier, ParticleLevelView, ParticleProvider, ParticleQuad,
    ParticleSpawn, ParticleTickView, ParticleView, RegisterParticleProvidersEvent,
    RegistrationContext, sunrise_mod,
};

struct SparkProvider {
    last_frame: usize,
}

struct Spark {
    rise: f64,
    last_frame: usize,
}

impl ParticleProvider for SparkProvider {
    fn create_particle(
        &self,
        spawn: &mut ParticleSpawn<'_>,
        particle: &mut dyn ParticleTickView,
    ) -> Option<Box<dyn CustomParticle>> {
        particle.set_lifetime(20);
        particle.set_sprite_index(0);
        Some(Box::new(Spark {
            rise: spawn.velocity[1],
            last_frame: self.last_frame,
        }))
    }
}

impl CustomParticle for Spark {
    fn tick(&mut self, particle: &mut dyn ParticleTickView, _level: &dyn ParticleLevelView) {
        if !particle.advance_age() {
            return;
        }
        particle.move_by([0.0, self.rise, 0.0]);
        if particle.age() == 10 {
            particle.set_sprite_index(self.last_frame);
        }
    }

    fn extract(&self, particle: &dyn ParticleView, partial_tick: f32, quad: &mut ParticleQuad) {
        if particle.age() == 0 {
            quad.size *= partial_tick;
        }
    }
}

struct SparkMod;

#[sunrise_mod]
impl Hooks for SparkMod {
    fn register_particle_providers(
        &self,
        _context: &RegistrationContext,
        event: &mut RegisterParticleProvidersEvent<'_>,
    ) {
        event.register_sprite_set(
            Identifier::parse_namespaced("mymod:spark").unwrap(),
            |sprites| SparkProvider {
                last_frame: sprites.frame_count() - 1,
            },
        );
    }
}
```

The listener runs at client startup. The last accepted factory for an ID runs once per sprite
load, including reloads, on the logical client. It captures owned `Send + Sync + 'static` data
and returns a concrete `ParticleProvider`. `ParticleSpriteSet::frame_count()` is nonzero: an
unresolved texture uses the atlas missing sprite in its declared frame, and an empty description
uses one missing sprite. The descriptor exposes no atlas or engine handles. A missing description
or failed bind is reported for the ID and its mod and refuses native spawns without falling back
to declarative motion. A resource pack's Polytone definition still takes precedence when it
overrides the type. Types without a registration can still use declarative motion.

After the applicable distance and Particles-setting checks, `create_particle` runs synchronously
on the owning thread. `ParticleSpawn` lends the typed options, raw position and velocity slots,
loaded level and the engine's live provider RNG. Drawing from it does not change the position
or velocity spread of later particles in the same packet. `ParticleTickView` starts with the
declaration's spawn defaults. Return `None` to decline just this spawn without an error, or
return `Some` with an instance whose Rust fields hold its state. It joins the normal pending
queue; its render group can still refuse admission. Keep owned data, not loans from the callback.

The default `CustomParticle::tick` calls `tick_base` for vanilla's lifetime check, movement and
friction. An override can call `tick_base` or use `advance_age` to record the previous position
and roll and check expiry before applying its own motion. Age starts at zero. Both operations
remove the particle when its age at the start of the tick reaches its lifetime. Call `remove()`
to end it sooner or to handle expiry yourself. `move_by` uses the collision box and loaded level.
`set_position` relocates the collision box without changing the position that rendering
interpolates from; call `set_previous_position` too to snap instead of interpolate.

Use `set_sprite_index` for an exact frame or `set_sprite_from_age` for vanilla's
`age * (frame_count - 1) / lifetime`. An invalid index or a zero divisor is reported and leaves
the sprite unchanged. Malformed inputs to other setters are reported without changing their
field or expiring the particle. `ParticleFacing::Camera` faces the camera; `AroundY` uses the
camera quaternion's Y and W components alone; `Fixed([x, y, z, w])` sets a world orientation.
`set_roll` turns the quad about its own normal, interpolated from the previous roll when the
current roll is nonzero.

`CustomParticle::extract` receives a read-only `ParticleView`, the frame's partial tick and a
mutable `ParticleQuad` on the owning thread. The default draws the configured appearance.
Override it to change this frame's size, RGBA, facing, roll or visibility. The engine interpolates
position at the partial tick and still uses the particle's sprite, light and render group. Colors
use vanilla's `ARGB.colorFromFloat`. Fixed quaternions written directly to the quad are used as
supplied. Set `quad.visible = false` to hide just this draw. A refused callback or non-finite
vertices also skips just this draw. None of these changes the instance, tick state or lifetime.

`ParticleLevelView` reads loaded `block_state(pos)` and stored `brightness(layer, pos)` during
construction and tick. A missing chunk reads as air; the sky layer always reads zero in a
dimension without skylight. The callbacks run on the particle engine's owning thread. A panic
is named in the log and client error toast and disables that registration's panicking callback
kind for the session. A tick panic retires its instances; an extraction panic does not stop
ticks. An ordinary refusal is reported without disabling the callback. Sprite reload discards
live and pending native instances before rebinding factories.

### Particle render groups

In `Hooks::register_particle_groups`, call `event.register(id, factory)` with a namespaced
`Identifier` and a factory returning `ParticleGroup { blend, particles }`. The hook runs once at
client startup; each accepted factory runs once per particle engine construction. The first
registration for an ID wins, and a duplicate or a reserved `minecraft` ID never calls its factory.
On reload, sprite-set factories bind new providers, but group factories do not run again. The
engine associates the sprite sets with the groups it already built. A group chooses the draw
layer and applies vanilla's `ParticleGroup.add` admission rules to its listed types, whether
those types have native instances or declarative motion.

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
