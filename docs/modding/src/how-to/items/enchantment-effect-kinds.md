# Enchantment effect kinds

Register a native effect codec in `Hooks::register_enchantment_effects`. Use
`event.effects.register_value` for a value transformation or
`event.effects.register_entity` for a server-side entity effect. The registries
are separate: each refuses duplicate IDs and reserves its vanilla kinds. Use
your mod's namespace so other mods can pick IDs independently.

The `type` in an enchantment selects one codec. Its `decode` method sees the
remaining JSON fields and returns an immutable handler. Read and validate your
fields there; a decode error rejects the definition with its enchantment and
component named. Only the selected data-pack definition is bound. A server's
frozen enchantment definitions do not change on `/reload`; syncing a definition
to a client for display does not require the client to decode its effects.

For example, register `example:scale` with an `EnchantmentValueEffectCodec`
that reads `factor` and returns an `EnchantmentValueEffect`. The `minecraft:damage`
component can compose it with vanilla effects:

```json
"minecraft:damage": [
  {
    "effect": {
      "type": "minecraft:all_of",
      "effects": [
        { "type": "minecraft:add", "value": 2 },
        { "type": "example:scale", "factor": 1.5 }
      ]
    }
  }
]
```

`process(level, random, value)` receives the current value and that stage's
live random stream. In a damage calculation, requirements draw from the level
and value effects draw from the victim. The next effect receives the returned
`f32` unchanged. In `all_of`, each child gets the preceding child's answer.

An `EnchantmentEntityEffectCodec` instead returns an `EnchantmentEntityEffect`.
Its `apply` method receives a `ServerHost` and an
`EnchantmentEntityEffectContext` containing the enchanted item, its holder and
slot when known, the selected entity and application position. A post-attack
context also names the victim, causing entity and direct entity when present.
An arrow's direct entity is the arrow, not the shooter. The handler is called
only if its component runs and its entry passes the requirements; this is not
a broadcast to other mods' handlers.

Entity effects in `minecraft:all_of` run in list order, interleaved with built-ins.
For `minecraft:post_attack`, the victim's equipped effects run before effects of
the attacking item. A refused hit does not enter this stage. A panicking codec
refuses the load; a panicking handler disables that registered effect kind for
the session without disabling other kinds.

When a new set of enchantment definitions is selected, it is decoded into new
handlers. An ordinary decode error does not disable the codec, and the server
does not publish a generation that failed to load.
