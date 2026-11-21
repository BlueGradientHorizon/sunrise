# Data files

A mod has one top-level `data/` tree and one `assets/` tree, with the same namespace and
content checks as a content mod. They are normal pack resources, not a second native-only data folder.
Native `blocks` and `items` listeners still receive each `DeclarationError` immediately from
`event.register`. A returning listener's invalid declaration refuses startup even if it ignored
the error. If a listener panics, neither its declarations nor its diagnostics join the cached
startup/reload declaration batch.
`Hooks::add_pack_finders` can also offer an optional bundled pack to a pack repository.
`event.packs.add_pack_finders` returns and logs rejected entries; a pack does not enter the repository
if its registration callback panics or incurs a phase refusal.
[Server data reload listeners](https://github.com/sunrise-org/sunrise/blob/main/crates/sunrise-mod-vocab/src/world/server_reload.rs) register through
`Hooks::add_server_reload_listeners`. During registration, use `event.listeners.get_registry().iter()`
to read names in registration order and `event.listeners.get_graph().iter()` to read explicit
dependency pairs `(first, second)`. Both lend IDs from the registry, so clone only the names you
need after the borrow ends. `event.listeners.get_last_vanilla_listener()` always borrows
`VanillaServerListeners::functions()` (matching NeoForge's
`SortedReloadListenerEvent.getLastVanillaListener`); the built-in `neoforge:loot_modifiers`
listener is registered before mod hooks without replacing it. `VanillaServerListeners::first()` and `last()` alias `functions()`, and
`VanillaClientListeners::first()` / `last()` alias `model_manager()` (Sunrise's single modeled
vanilla client reload anchor), with no synthetic `minecraft:first` or `minecraft:last` boundary
nodes or automatic edges. During sorting, only dangling non-`minecraft` listeners are linked after
the anchor (matching NeoForge's `ReloadListenerSort`). A dependency orders both phases:
`functions()` reads function files in prepare and admits the new library in apply;
`model_manager()` builds the models in prepare. Its apply publishes the validated
model generation and fires `model_baking_completed`. Before that apply, the atlas upload
publishes new sprites and fires `texture_atlas_stitched` while model lookups still see the previous
generation (no models on first load). A panicking registration discards that mod's batch and disables that hook callback for the session.
A panicking prepare or apply logs the mod and listener, shows a local mod error toast, and disables that
named listener for the session. Unrelated listeners still run. The game keeps any previously admitted
retained object instead of admitting a replacement from the failed listener. Publish changes to your
own memory atomically; the game cannot undo them.
On first load and later reloads, validation finishes before either the manager
or mod listeners publish.

A listener's `ServerReloadContext` lists namespaced IDs and reads the
winning bytes from selected datapacks and the mod's own `data/`. Parse once in `prepare`, then return
`Ok(Box::new(move || { /* publish prepared data */ }))` from the method, whose return type is
`Result<Box<dyn FnOnce() + Send + 'static>, ReloadResourceError>`. Capture owned data: the runner
consumes the action once on the owning thread
only after all listeners prepare successfully. A prepare error drops pending actions without
running them. This work does not run on the tick. Server world tables, including
`data_pack_files` contributions, prepare before any listener applies its new state.
Contributions share a list, so a failed contributed file cannot be attributed to a particular mod.
The original pack trees remain separate from the launcher's code-only horizon-mixins input.

Client-side asset reads use [client asset reload listeners](https://github.com/sunrise-org/sunrise/blob/main/crates/sunrise-mod-vocab/src/client/client_reload.rs); server
listeners cannot read `assets/`. Client listeners return the same once-only action, which runs on
the game thread, on the declared side of model publication. With no dependency linking
it to the vanilla anchor, a mod listener runs after model publication by default.
Packaged assets also participate in the game's normal resource packs.

Both prepare contexts lend converted IDs through `context.list()` and bytes through
`context.read(&id)`. These views and reload errors stay in-process, not on `Wire`. To abort on
an invalid file, return `Err(ReloadResourceError::new(id, cause))` with the original error, not
its message. `error.resource()` borrows the failing resource ID, and
`std::error::Error::source(&error)` exposes its original cause. To register, keep calling
`event.listeners.add_listener` and `event.listeners.add_dependency`, or
`event.listeners.add_retained_listener` on the server. An individual rejected call returns a
`ReloadOrderError` and is logged each time without failing the reload. A cycle in the completed
graph still fails it. A [failed registration callback](../../concepts/load-phases.md#reloading)
publishes none of its listeners, edges or retained entries; later mods can still register.
This applies to the initial client load as well as later client and server reloads.

Outside a listener, `host.resource(pack_type, &identifier)` reads one file from the selected
stack: `PackType::ServerData` takes `mymod:machines/furnace.json` from `data/`, and
`PackType::ClientResources` takes `mymod:models/block/furnace.json` from `assets/`. As with
`ResourceManager.getResource`, the highest selected pack that supplies the file wins; filters,
a world's datapacks and a mod's own root data all apply. The answer is the winning bytes, or
`None` for a missing, unreadable or over-64-MiB file. Use a reload listener for a whole folder
or for parsing at reload time; a single read does not reload packs. Directory-pack
symlinks follow vanilla pack behavior: linked resource files are read through the pack.

## Embedded data

Generate code-only tables as Rust source before packaging and include them under `src/`.
The launcher refuses mod build scripts with `E_NATIVE_BUILD_SCRIPT`. Embedded tables do not
participate in resource reloads.

## Package contents

A native bundle accepts the manifest, optional icon, root `Cargo.toml`, `Cargo.lock` and `build.rs`,
`.rs` and `.toml` files under `src/`, `assets/` and `data/`, a `runtime/` folder listed in `[runtime]`, and root text files (`README*`, `LICENSE*`,
`LICENCE*`, `COPYING*`, `NOTICE*`). Source and resource size limits are checked separately;
any other file causes the archive to be refused ([the manifest](../../reference/manifest.md)).

`runtime/` holds the data files your code opens beside itself: each is listed in `[runtime]` with its SHA-512, any
file type, up to 64 MiB a file, 256 MiB and 256 files in all. They are not pack data and no reload touches them;
a missing, extra or mismatched file refuses the package by name ([`[runtime]`](../../reference/manifest.md)).

The game hashes each file once, when the package opens, and retains the verified bytes. Get the handle with
`cx.runtime_files()` in registration; you can keep it and use it from any thread. A mod can read only its own
package's files. A package with no `[runtime]` returns `RuntimeFilesError::NoRuntimeFiles`.

- `files.names()` borrows the listed names in name order as `&[String]`.
- `files.file("nav.bin")` returns `Option<&[u8]>`: parse the borrowed bytes directly, or use `.to_vec()` only
  when the bytes must outlive the handle. An unlisted name returns `None`.
- `files.file_path("nav.bin")` borrows the exact disk path as `&Path`; use `.to_path_buf()` if you must keep
  the path after dropping the handle. An unlisted name returns `RuntimePathError::NotListed`; a listed
  file in a `.smod` archive returns `RuntimePathError::NotAFolder`. This is a live path: opening it after
  a disk edit may read different bytes. Use `files.file()` when you need the verified snapshot.

Although the archive accepts `build.rs`, a native build refuses it unless you disable it with
`build = false` in Cargo's `[package]` table and remove build dependencies.

A native `.smod` carries both source and content: the launcher compiles the source, and the game
loads the package's `assets/` and `data/`. All packages take their identity from `sunrise-mod.toml`.
