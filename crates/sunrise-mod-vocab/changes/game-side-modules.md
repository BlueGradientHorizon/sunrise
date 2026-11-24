# One-sided `game` modules re-parented under `client_side` and `server_side`

Gameplay modules under `sunrise_client::game` that touch only `ClientState` fields or only `ServerState` fields are now declared as children of `sunrise_client::game::client_side` or `sunrise_client::game::server_side` (files remain in place). `ClientState` and `ServerState` move from `game::client_state` and `game::server_state` to `game::client_side` and `game::server_side`, with fields whose users are all inside their own side restricted to that side module. The authoritative tables are `tools/game_module_sides.tsv` (module to side) and `tools/game_field_owners.tsv` (field owner and visibility); compute current module counts per side with:

```sh
awk -F'\t' 'NR > 1 { counts[$2]++ } END { for (side in counts) print side, counts[side] }' tools/game_module_sides.tsv
```

The stable `mod_api` surface and published loops (including `loop:block_entities.tick`, whose defining module `block_entity_ticks` remains at `sunrise_client::game::block_entity_ticks`) are unchanged. All path changes are advisory in `internals.json` and `functions.json`.

## What changes for native mods

- **`[[native.mixin]]` replacement targets:** every replaceable function in a module whose `tools/game_module_sides.tsv` side is `client` or `server` now includes the side module in its target path:
  - `sunrise_client::game::<module>::...` → `sunrise_client::game::client_side::<module>::...` (for `client` rows)
  - `sunrise_client::game::<module>::...` → `sunrise_client::game::server_side::<module>::...` (for `server` rows)
- **`[[native.bridge]]` targets on state structs:** bridges targeting `ClientState` or `ServerState` now target `sunrise_client::game::client_side::ClientState` or `sunrise_client::game::server_side::ServerState` instead of `client_state` / `server_state`.
- **`[[internals]]` entries:** listed internals under moved modules and under `ClientState` / `ServerState` carry the updated `client_side` / `server_side` paths.
- **Rust imports in helpers:** `game.rs` re-exports moved modules and aliases `client_state` / `server_state`, so `crate::game::<module>` paths in helper code keep resolving.

## How to repin

1. Update every `[[native.mixin]]` `target` in `sunrise-mod.toml` whose module moved according to `tools/game_module_sides.tsv` (to generate the exact old-to-new target map between two builds' `functions.json`, run `python3 tools/game_module_sides.py --renames <before-functions.json> <after-functions.json>`).
2. Update any `[[native.bridge]]` `target` naming `sunrise_client::game::client_state::ClientState` or `sunrise_client::game::server_state::ServerState` to `client_side` or `server_side`.
3. Update the mod's pinned Sunrise revision and refresh declared internals with `cargo sunrise-mod internals sync` followed by `cargo sunrise-mod check` (or `cargo sunrise-mod pin`).
