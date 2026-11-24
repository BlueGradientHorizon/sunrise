# Advisory: gameplay side modules (`client_side` and `server_side`)

The stable `mod_api` surface and published loops (including `loop:block_entities.tick`, whose defining module `block_entity_ticks` remains at `sunrise_client::game::block_entity_ticks`) are unchanged. Native engine catalog changes are outside the stable API gate, but unresolved manifest declarations still fail `cargo sunrise-mod check` and `cargo sunrise-mod build`.

For the modding-book guide, see [Game side modules](https://playsunrise.net/docs/modding/concepts/native-route/game-side-modules) ([`docs/modding/src/concepts/native-route/8-game-side-modules.md`](../../../docs/modding/src/concepts/native-route/8-game-side-modules.md)).

## Path rule and state-module exceptions

- **One-sided modules (`tools/game_module_sides.tsv`):** any module whose `side` column is `client` or `server` gains that side segment in its canonical path while its source file stays in place:
  - `sunrise_client::game::<module>::...` → `sunrise_client::game::client_side::<module>::...` (`client`)
  - `sunrise_client::game::<module>::...` → `sunrise_client::game::server_side::<module>::...` (`server`)
  - Modules with `side = game` keep `sunrise_client::game::<module>::...`.
- **State-module exceptions:** the two state modules replace their segment and rename their source files:
  - `sunrise_client::game::client_state::...` → `sunrise_client::game::client_side::...` (`crates/sunrise-client/src/game/client_state.rs` → `crates/sunrise-client/src/game/client_side.rs`)
  - `sunrise_client::game::server_state::...` → `sunrise_client::game::server_side::...` (`crates/sunrise-client/src/game/server_state.rs` → `crates/sunrise-client/src/game/server_side.rs`)

`game.rs` re-exports moved modules and aliases `client_state` / `server_state`, so `crate::game::<module>` paths in Rust helper code continue resolving. Manifest target strings and SDK catalogs read the declaring module and require the canonical path.

## Affected manifest declarations

- **`[[native.mixin]]` targets:** methods, free functions, and trait methods in moved modules or renamed state modules. The rename map emits normalized SDK keys; in manifests that spell lifetimes (`Game<'_>`) or trait-qualified targets (`<Type as Trait>::method`), apply the same `client_side` / `server_side` segment insertion or replacement before `<module>`.
- **`[[native.bridge]]` targets:** field bridges targeting `ClientState` or `ServerState`, field bridges targeting types declared in any moved module (for example `sunrise_client::game::client_side::native_panel::Panel`), and `INVOKE` bridges naming methods in moved modules.
- **`[[native.module]]` file paths:** ordinary moved modules keep their source file path; support modules targeting `crates/sunrise-client/src/game/client_state.rs` or `crates/sunrise-client/src/game/server_state.rs` must update `file` to `client_side.rs` or `server_side.rs`.
- **Explicit `[[internals]]` entries:** declared engine items under moved modules or under `ClientState` / `ServerState` carry the canonical `client_side` / `server_side` path.

## How to repin a native mod

1. Pin the new game build with `cargo sunrise-mod pin <build>` (`pin` refreshes hashes for declarations that still resolve and leaves missing declarations in place).
2. Generate the selectable rename map from the prior and target builds' `functions.json` indexes with the script below, then update affected `[[native.mixin]]`, `[[native.bridge]]`, `[[native.module]]` (state files), and explicit `[[internals]]` declarations:

```sh
python3 tools/game_module_sides.py --renames <before-functions.json> <after-functions.json> --out moved-mixin-targets.tsv
```

The default `--renames` command emits only rows whose destination is selectable in `<after-functions.json>` and exits `0` (printing refused, missing, and additional-target counts to stderr). Pass `--strict` to validate an exact transition and exit non-zero if any moved target is refused or missing or if `<after-functions.json>` adds unmapped targets.

3. For mods using discovered internals, run `cargo sunrise-mod internals sync`.
4. Run `cargo sunrise-mod check` and `cargo sunrise-mod build`. If a pinned mixin `fingerprint` fails with `E_FINGERPRINT`, inspect the target function on the new build and update the fingerprint to the reported `got` value once verified.
