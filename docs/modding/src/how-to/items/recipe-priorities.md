# Recipe priorities

When two recipes match the same input, the first one wins. A content mod or data
pack picks the winner with `data/<namespace>/recipe_priorities.json`, in
NeoForge's format:

```json
{"replace": false, "entries": {"example:specific": 5, "example:broad": -1}}
```

| Key | Meaning |
|---|---|
| `entries` | recipe id to priority, a signed 32-bit integer; an id without a namespace is `minecraft:` |
| `replace` | `true` clears every priority an earlier layer set, even when `entries` is absent or empty |
| `neoforge:conditions` | [load conditions](load-conditions.md); a layer whose conditions fail changes nothing |

A recipe with no entry has priority 0.

## Layers

Files apply lowest layer first: the game's own data, then content mods in
dependency order (a mod's jars in declared order, below its own directory or
`.smod` files), then the world's enabled data packs in pack order. The namespaces
of one layer apply in name order. Every layer of this file counts, including a
lower copy at the same path and a copy in another namespace; other passive
resources keep their usual last-file-wins rule. A later entry overrides an
earlier one. Ids no recipe has are kept quietly, including recipes absent because
their load conditions failed.

A layer that is not valid is refused as a whole, and the load fails naming the
pack and file: a root that is not an object, a `replace` that is not a boolean,
an `entries` that is not an object, a priority outside the signed 32-bit range,
or a file that cannot be read. Nothing of it applies. A layer whose load
conditions fail is not read further.

## Order

The recipes are sorted once per server data load, by descending priority and then
by vanilla's registration order (path, then namespace). Crafting, every cooking
type, smithing, the stonecutter list and the recipes of mod-defined types follow
that order; nothing sorts or looks a priority up while recipes match. With no
file, the order is vanilla's.

A client joined to a server lists its stonecutter recipes in the order the
server sent them, and its buttons number those entries, whatever packs the client
has itself.
