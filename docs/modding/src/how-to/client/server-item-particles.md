# Server item particles

Construct `ParticleOptions::item_stack(item_particle_key, stack)` and call
`level.send_particles(host, &options, state, at, spread, speed, count)` with a
`LevelRef` and a `ServerHost`. The key is the
load-resolved `minecraft:item` particle key; the stack is an API `ItemStack`.

The server validates the stack against the registered items and component and
count limits. Invalid stacks return `false` without queueing a particle. A `true`
result means queued, not necessarily drawn. The ordinary particle packet carries
the item stack, not server-selected sprite names. Each client resolves its ground
item model with seed 0, through the same path as
`ClientHost::item_particle_options`, so its resource pack and stack model
components determine the layer particle sprites. Inside an open transaction, accepted particles
follow the [deferred effect rules](../../concepts/transactions.md#deferred-level-entity-and-player-effects).

The resolved `ParticleOptions::item` sprite list is for local rendering; it is
not a substitute for the stack on server sends. Block, block-marker, falling-dust,
dust-pillar and block-crumble requests require a valid block state argument.
Trail and vibration requests are refused by the server send.

## Send around several positions

For any supported server particle, pass a slice of positions to send the same effect
around each one:

```rust,ignore
let accepted = level.send_particles_batch(host, &options, state, &positions, spread, speed, count);
```

The server validates the whole request before queueing anything. A live server level
accepts an empty slice without checking the particle arguments. Otherwise each position,
including repeats, is a separate request, in slice order. As in vanilla
`ServerLevel.sendParticles`, only players in the named level whose block centre is
strictly less than 32 blocks from that position receive its packet. A successful
call does not guarantee a recipient. The batch follows the same deferred effect rules
as a single send: it publishes in journal order on the owning level's outermost commit
and is discarded on abort.
