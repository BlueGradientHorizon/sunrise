# Drop-order side-owned `Game` fields wrapped in transparent side wrappers

Single-side fields of `sunrise_client::game::Game<'a>` that stay directly on `Game` to preserve drop order (rows in `tools/game_field_owners.tsv` whose `owner` is `game` and whose `wrapper` column is not `-`) are now wrapped in `#[repr(transparent)]` single-field tuple structs defined in `sunrise_client::game::client_side` and `sunrise_client::game::server_side`:

- `ClientOnly<T>` / `ServerOnly<T>`: inner tuple field `.0` is visible only inside `game::client_side` / `game::server_side`.
- `ClientShared<T>` / `ServerShared<T>`: inner tuple field `.0` is visible across `game`, while marking ownership on the client or server side so a field can be narrowed to `*Only<T>` by changing a single type name.

The authoritative table is `tools/game_field_owners.tsv` (`wrapper` column); compute current counts per wrapper kind with:

```sh
awk -F'\t' 'NR > 1 { counts[$7]++ } END { for (w in counts) print w, counts[w] }' tools/game_field_owners.tsv
```

Field declaration order, layout, and drop order on `Game<'a>` are unchanged. The stable `mod_api` surface, published loops, and selectable `functions.json` replacement targets are unchanged; all changes are advisory in `internals.json`.

## What changes for native mods

- **`[[internals]]` entries on `Game<'_>`:** every wrapped field listed in `tools/game_field_owners.tsv` now has field type `client_side::ClientOnly<T>`, `client_side::ClientShared<T>`, `server_side::ServerOnly<T>`, or `server_side::ServerShared<T>` instead of bare `T`, and the four wrapper structs appear under `sunrise_client::game::client_side` and `sunrise_client::game::server_side`.
- **Direct field access in native helpers / mixins:** reaching the inner value through a `Game` handle uses field projection (`game.<field>.0`), preserving disjoint borrows across `Game` fields.

## How to repin

1. Update direct accesses to wrapped `Game` fields in native mod helpers from `game.<field>` to `game.<field>.0` according to the `wrapper` column in `tools/game_field_owners.tsv` (`tools/game_field_owners.py` supports `--list`, `--check`, and `--fix`, failing closed outside `crates/sunrise-client/src/game*` with candidate sites on stderr).
2. Update any `[[native.bridge]]` or helper signature that names a wrapped `Game` field's exact type.
3. Update the mod's pinned Sunrise revision and refresh declared internals with `cargo sunrise-mod internals sync` followed by `cargo sunrise-mod check` (or `cargo sunrise-mod pin`).
