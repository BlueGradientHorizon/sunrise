# Runtime packages and identity

Move the executable, prepared resources and `sunrise-native-mods/` package directory
together when you distribute a native build. Keep each copied project's flat manifest,
crate, assets and data.

The launcher generates the game's embedded native inventory from your selected
`sunrise-mod.toml` manifests: mod id, version and side. Cargo package metadata and
hook factories do not supply identity.

| Rule | Behaviour |
| --- | --- |
| Package location | The game reads each compiled mod's package from `<executable directory>/sunrise-native-mods/<id>/`, not the working directory or a build record. |
| Identity | The package's id, version and side must match the embedded inventory. |
| Startup validation | Before installing hooks or starting a world, the game refuses missing, unreadable, content-only or identity-mismatched packages. The error begins `Compiled native mods cannot be loaded:` and lists the affected mods and paths. Restore the packages exported with that executable or rebuild it. |
| Content-only mods | These use the working directory, player toggles and explicit `--dev-mod` paths. A content package cannot shadow a compiled mod's id. |
| Display metadata | The mod list and crash details include code-only mods from the embedded inventory. Display names and descriptions come from validated manifests. |

Declare Controls bindings through `Registrar::register_key_mapping`. In `client_setup`, resolve each
one with `host.mod_key_mapping(owner, id)` and retain the `ModKeyMappingRef` for later callbacks.
`mapping.binding(host)` lends the current key name for the host borrow and returns its modifiers;
`mapping.translated_key_message(host)` builds its current Controls label when you need to display it.

Use `mapping.is_down(host)` for held state and `mapping.consume_click(host)` to take one pending click
(`KeyMapping.consumeClick`). Each press, auto-repeat and mouse press counts while no screen is open and
the mapping's conflict context and modifiers are active. Opening a screen or overlay drops the count.
Control-socket input counts like window input, and a key fires `input_key` once per press, repeat and release.

A mod's own screen can change its bindings as the player does in Controls. Call
`mapping.set_binding(host, key, modifiers)` with a key or mouse button name (`key.keyboard.x`,
`key.mouse.left`, `key.keyboard.unknown` for none) and shift (1), control (2) or alt (4).
`mapping.reset_binding(host)` restores the declared default. Changes take effect from the next input
and are saved to `sunrise-mods.txt`. You can read another mod's binding, but attempts to set or reset it
return `ModKeyRefusal::NotOwner`, even with a valid handle. An invalid key or modifier word returns
`UnknownKey` or `UnknownModifiers` without changing anything.

An unknown owner or id returns `ModKeyRefusal::NotRegistered` when resolving. A replaced client or
registration roster makes a retained handle return `Stale`; resolve it again in the new client or roster.
Rebinding and resetting alone do not make it stale. Save the owner and id, not the handle. `Unavailable`
means the host or phase has no client input access. The label call reports `Stale` and `Unavailable`
through `MappingError` and can also return `MappingError::Label` if the text cannot be converted.

`native-mods.json` is provenance and rebuild status, not game identity, authentication, announcements
or startup input. The launcher retains it for build/status displays. The separate `sunrise-server`
supervisor may report the adjacent record as a provenance snapshot, not a query of the running engine.
Its SHA-256 values and revision describe the built binary's inputs, not the running mod set.
The launcher prepares jar data from canonical manifests as non-authoritative candidates; the engine
validates identity against its embedded inventory. Original-jar pack order and
whole-package refusal rules are in [Original Java jar layers](../../reference/manifest.md#original-java-jar-layers).

## Resolve an entity by UUID

In a server hook, wrap a saved `u128` in `EntityUuid` and pass it to the host.
The lookup returns `None` if the entity is unloaded or belongs to a different
level than the hook. Try again from a hook running in the entity's level.

```rust,ignore
use mod_api::{EntityRef, EntityUuid, ServerHost};

fn find_saved_entity(host: &dyn ServerHost, saved_uuid: u128) -> Option<EntityRef> {
    host.entity_by_uuid(EntityUuid(saved_uuid))
}

fn entity_identity(host: &mut dyn ServerHost, entity: EntityRef) -> Option<EntityUuid> {
    entity.uuid(host).map(EntityUuid)
}
```

`EntityRef::uuid` returns the raw `u128`. Use `EntityUuid(raw_uuid)` to pass
that value back into the lookup.
