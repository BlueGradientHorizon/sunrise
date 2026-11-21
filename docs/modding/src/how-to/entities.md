# Entities, players and commands

Pages for mods that work with players, mobs and other entities, and with the server's commands and services.

For custom status-effect behaviour, see [Ticking a registered mob effect](entities/effect-ticks.md).

`EntityRef::body_yaw(host)` reads stored living body rotation, in degrees without normalization.
It is available for live server mobs, armor stands, the ender dragon and players with a live body;
on a client it reads only the local player's avatar. Stale handles, nonliving entities and dragon
parts have no body yaw. In a mob goal, the active mob and disjoint live mobs are read from the goal's view.

`EntityRef::set_body_yaw(server_host, degrees)` changes the current body rotation, not the previous
angle used for interpolation, the head, or the view. Like vanilla's `LivingEntity.setYBodyRot`, it
neither normalizes the angle nor sends a packet. The dragon's body turn does not steer its flight.
Vanilla's overrides still apply: silverfish and endermites also turn their view for a finite angle;
armor stands update the previous view angle and both head angles instead of assigning current body yaw.
On the client, use `PlayerRef::set_body_yaw(client_host, degrees)` on the local player. It changes the
same live avatar state that feeds pose interpolation, not a pose already extracted for rendering.
Both setters return `Ok(false)` for an inapplicable or stale handle and a typed refusal from a host
that cannot write living state.

Use `entity.set_persistent(host, false)` from a server hook to keep a temporary entity, such as a display,
out of future world saves. It stays live until it is removed or its chunk unloads. Passing `true` restores
the normal save rules; neither call alters an existing snapshot, and both keep the same UUID the entity
would save with. To omit a vehicle and all its passengers, set the flag on the root vehicle, not on a rider.
The flag follows the live entity across dimensions but is never saved. The setter returns `Ok(false)` for players, dragon parts and handles that do not resolve in the current level.

Use `EntityRef::hurt` on the server with the fully qualified id of a registered damage type, vanilla
or modded. Hurt hooks see that exact id. The direct and causing entities are separate from the type:
passing a player as the cause of `minecraft:explosion` does not turn it into `minecraft:player_explosion`.
A return value of `true` means the hurt was accepted, not necessarily that the entity lost health.
For example, a heart-bound Creaking can react to a hit without losing health.

<!-- section-pages -->
