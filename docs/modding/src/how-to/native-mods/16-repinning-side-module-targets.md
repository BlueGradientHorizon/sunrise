# Repinning native targets for game side modules

One-sided gameplay modules and function-level side mirrors under `sunrise_client::game` are declared as children of `client_side` or `server_side` so each side's private state stays behind Rust's module boundary. While `game.rs` re-exports moved modules for Rust helper code, manifest target strings and SDK catalogs resolve against the declaring module and require the canonical path.

## Canonical paths

[`tools/game_module_sides.tsv`](https://github.com/sunrise-org/sunrise/blob/main/tools/game_module_sides.tsv) lists the declaring module for each gameplay module, and [`tools/game_fn_sides.tsv`](https://github.com/sunrise-org/sunrise/blob/main/tools/game_fn_sides.tsv) lists functions moved into side mirror modules:

- **Client and server modules (`side = client` or `server`):** insert `client_side` or `server_side` after `sunrise_client::game::`. The source file stays at its path under `crates/sunrise-client/src/game/`.
  - `sunrise_client::game::client_side::<module>::…` (`client`)
  - `sunrise_client::game::server_side::<module>::…` (`server`)
- **Function-level mirror modules (`tools/game_fn_sides.tsv`):** one-sided functions moved out of `game.rs` or a shared (`side = game`) module live in a side mirror module under `client_side` or `server_side`:
  - From `game.rs`: `sunrise_client::game::client_side::game_client::…` (`crates/sunrise-client/src/game/game_client.rs`) or `sunrise_client::game::server_side::game_server::…` (`crates/sunrise-client/src/game/game_server.rs`)
  - From a shared module `<module>`: `sunrise_client::game::client_side::<module>_client::…` (`<stem>_client.rs` or `mod_client.rs`) or `sunrise_client::game::server_side::<module>_server::…` (`<stem>_server.rs` or `mod_server.rs`)
- **Shared modules (`side = game`):** `sunrise_client::game::<module>::…`.
- **State modules (`ClientState` and `ServerState`):** `sunrise_client::game::client_side::…` (`crates/sunrise-client/src/game/client_side.rs`) and `sunrise_client::game::server_side::…` (`crates/sunrise-client/src/game/server_side.rs`).

## Declarations to update in `sunrise-mod.toml`

- **[Mixins (`[[native.mixin]]`)](../../concepts/native-route/2-mixins.md#naming-the-target):** update `target` for methods, free functions, and trait methods declared in side modules, mirror modules, or state modules. The rename maps emit normalized SDK keys without lifetimes; for targets that spell lifetimes (`Game<'_>`) or trait-qualified paths (`<Type as Trait>::method`), apply the same `client_side` or `server_side` segment and mirror-module name after `sunrise_client::game::`.
- **[Bridges (`[[native.bridge]]`)](../../concepts/native-route/4-bridges.md):** update `target` for field bridges on `ClientState` or `ServerState`, field bridges on types declared in side modules (for example `sunrise_client::game::client_side::native_panel::Panel`), and `INVOKE` bridges whose method is declared in a side or mirror module.
- **[Support modules (`[[native.module]]`)](../../concepts/native-route/5-support-modules.md):** set `file` to `crates/sunrise-client/src/game/client_side.rs` or `crates/sunrise-client/src/game/server_side.rs` when mirroring a state module, and to the corresponding mirror file (`game_{client,server}.rs`, `<stem>_{client,server}.rs`, or `mod_{client,server}.rs`) when mirroring items moved into a function-side mirror; other side modules keep their source file paths.
- **Explicit `[[internals]]` entries:** update canonical paths for items declared in side modules, mirror modules, or state modules. Confirm each key against the pinned build's `internals.json` (`.sunrise/cache/<build>/build-data/internals.json`), since an item's path follows the module holding its declaration or `impl`.

## Steps to repin a native mod

1. **Pin the target build:** run `cargo sunrise-mod pin <build>`. `pin` updates `[native] sunrise` and refreshes hashes for declarations that resolve on the target build, reporting unresolvable paths as `missing` with their old hashes untouched.
2. **Generate the rename maps and update paths:** run the module-side and function-side rename scripts against the prior and target builds' `functions.json` catalogs (see `--help` on each script for options):

```sh
python3 tools/game_module_sides.py --renames <before-functions.json> <after-functions.json> --out moved-module-targets.tsv
python3 tools/game_fn_sides.py --renames <before-functions.json> <after-functions.json> --out moved-fn-targets.tsv
```

Update affected `[[native.mixin]]`, `[[native.bridge]]`, `[[native.module]]`, and explicit `[[internals]]` entries in `sunrise-mod.toml`.
3. **Regenerate internal hashes:**
   - If your mod uses discovered internals, run `cargo sunrise-mod internals sync` to replace `internals` with compiler-discovered paths, kinds, and hashes for the pinned build.
   - If your mod declares explicit hashed `[[internals]]` entries without `internals sync`, refresh every rewritten entry's hash with `cargo sunrise-mod internals add <new-path>…` (pass `--kind <kind>` if a path is ambiguous), or re-run `cargo sunrise-mod pin <build>` after updating the paths (when the mod has no `[[native.module]]` rows).
4. **Verify and rebuild:** run `cargo sunrise-mod check` to validate declarations and refreshed internal hashes, then run `cargo sunrise-mod build` to compile the package and regenerate `uses.json` with the target build's `made_for` id, internal hashes, and split function digests (see [Fitting another build](../../reference/sdk/04-fit.md); mods with `[[native.module]]` or `[[native.bridge]]` target one build only and pack no `uses.json`). Follow [`E_FINGERPRINT`](../../concepts/native-route/2-mixins.md#fields) for any fingerprinted mixin whose digest differs on the target build.
