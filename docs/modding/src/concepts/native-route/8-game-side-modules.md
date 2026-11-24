# Game side modules

One-sided gameplay modules under `sunrise_client::game` are declared as children of `client_side` or `server_side` so each side's private state stays behind Rust's module boundary. The stable `mod_api` surface and published loops (including `loop:block_entities.tick`, whose defining module `block_entity_ticks` remains at `sunrise_client::game::block_entity_ticks`) are unchanged.

Engine catalogs (`functions.json` and `internals.json`) are advisory and outside the stable API gate, but unresolved manifest declarations still fail `cargo sunrise-mod check` and `cargo sunrise-mod build` (see [Mod fit](../../reference/sdk/04-fit.md) and [Surface changes](../../reference/sdk/02-surface.md)).

## Canonical path rule

The authoritative module placement table is [`tools/game_module_sides.tsv`](https://github.com/sunrise-org/sunrise/blob/main/tools/game_module_sides.tsv):

- **Moved modules (`side = client` or `server`):** insert the side module segment after `sunrise_client::game::`. The source file stays in place on disk.
  - `sunrise_client::game::<module>::…` becomes `sunrise_client::game::client_side::<module>::…` (`client`)
  - `sunrise_client::game::<module>::…` becomes `sunrise_client::game::server_side::<module>::…` (`server`)
- **Shared modules (`side = game`):** keep `sunrise_client::game::<module>::…`.
- **State-module exceptions:** the two state modules replace their segment and rename their source files:
  - `sunrise_client::game::client_state::…` becomes `sunrise_client::game::client_side::…` (`crates/sunrise-client/src/game/client_state.rs` → `crates/sunrise-client/src/game/client_side.rs`)
  - `sunrise_client::game::server_state::…` becomes `sunrise_client::game::server_side::…` (`crates/sunrise-client/src/game/server_state.rs` → `crates/sunrise-client/src/game/server_side.rs`)

`game.rs` re-exports moved modules and aliases `client_state` / `server_state`, so `crate::game::<module>` paths in Rust helper code continue resolving. Manifest target strings and SDK catalogs resolve against the declaring module and require the canonical path.

## What to update in `sunrise-mod.toml`

- **[Mixins (`[[native.mixin]]`)](2-mixins.md#naming-the-target):** update `target` for methods, free functions, and trait methods in moved modules or renamed state modules. The generated rename map emits normalized SDK keys (without lifetimes); if your manifest writes lifetimes such as `Game<'_>` or trait-qualified paths such as `<Type as Trait>::method`, apply the same `client_side` / `server_side` segment insertion (or state-module replacement) right after `sunrise_client::game::`.
- **[Bridges (`[[native.bridge]]`)](4-bridges.md):** update `target` for field bridges on `ClientState` / `ServerState`, field bridges on types declared in any moved module (for example `sunrise_client::game::client_side::native_panel::Panel`), and `INVOKE` bridges whose method lives in a moved module.
- **[Support modules (`[[native.module]]`)](5-support-modules.md):** `file` paths for ordinary moved modules are unchanged because their files stay in place. Support modules targeting `crates/sunrise-client/src/game/client_state.rs` or `crates/sunrise-client/src/game/server_state.rs` must update `file` to `crates/sunrise-client/src/game/client_side.rs` or `crates/sunrise-client/src/game/server_side.rs`.
- **Explicit `[[internals]]` declarations:** update canonical paths under moved modules and under `ClientState` / `ServerState`.

## How to repin a native mod

1. **Pin the target build:** run `cargo sunrise-mod pin <build>`. `pin` refreshes hashes for declarations that still resolve and leaves missing declarations in place, so renamed paths must be updated explicitly.
2. **Rewrite moved declarations:** generate the selectable rename map from the prior and target builds' signed SDK `functions.json` catalogs:

```sh
python3 tools/game_module_sides.py --renames <before-functions.json> <after-functions.json> --out moved-mixin-targets.tsv
```

By default, `--renames` writes only rows whose destination is selectable in `<after-functions.json>`, prints summary counts (selectable, refused, missing, unchanged, and additional targets) to stderr, and exits `0`. Pass `--strict` to validate an exact transition and exit non-zero when any moved target is refused or missing or when `<after-functions.json>` contains additional unmapped targets.

3. **Sync discovered internals:** if your mod uses discovered internals, run `cargo sunrise-mod internals sync`.
4. **Check and rebuild:** run `cargo sunrise-mod check` followed by `cargo sunrise-mod build`. Because moved targets can change staged digests, follow [`E_FINGERPRINT`](2-mixins.md#fields) on any pinned `OVERWRITE` or fingerprinted mixin row: inspect the target function on the new build and update `fingerprint` to the reported `got` value once verified.
