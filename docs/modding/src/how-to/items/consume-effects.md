# Custom consume effects

In `Hooks::register`, call `register_consume_effect_type` on the engine's
`Registrar`. If you implement `Registrar` yourself, also implement
`ConsumeEffectsRegistrar`. Use a type ID in an owned, non-reserved
namespace. The two functions in `ConsumeEffectType::new` read and write the
saved map; the game handles the `type` field. `load` validates the fields
before a stack edit or load is admitted. `save` supplies the canonical fields
with exact NBT types, including when the value arrives over the network.

Derive `Wire` on the value itself. Its bytes follow the server's registry ID,
just as a built-in effect's stream fields do. Install the registering mod on
both sides: normal configuration sync resolves the server's IDs by key and
checks the Wire schemas. Switching off a live effect listener does not remove
its registered codec from the connection.

```rust
use mod_api::{Compound, ConsumeEffectType, Identifier, Registrar, Tag, Wire};

#[derive(Wire)]
struct Charge { points: u16 }

fn register(r: &mut dyn Registrar) -> Result<(), String> {
    r.register_consume_effect_type(
        Identifier::parse("example:charge").map_err(|e| e.to_string())?,
        ConsumeEffectType::new::<Charge>(
            |fields| match fields.get(&"points".into()) {
                Some(Tag::Int(n)) => u16::try_from(*n)
                    .map(|points| Charge { points })
                    .map_err(|e| e.to_string()),
                _ => Err("expected integer points".into()),
            },
            |effect| Ok(Compound::from([("points".into(), Tag::Int(effect.points.into()))])),
        ),
    )
}
```

An entry then reads `{type: "example:charge", points: 10}` in SNBT, in either
`on_consume_effects` or `death_effects`. The example accepts an NBT `Int`;
accept other numeric tag types in `load` if your data format calls for them.
Implement `Hooks::consume_effect` to handle a consumable's `on_consume_effects`
entry whose type is in your mod's namespace. Only that namespace's enabled
subscriber receives it. The hook runs on the server thread for the consuming
player or mob, including potions, ominous bottles, and witch and trader drinks.
Entries run in the stack's effective consumable list order, interleaved with
built-in effects; stack patches can change or remove the list.

- The event provides the effect kind and its definition payload as a borrowed `Tag`.
- `event.stack` is a read-only snapshot before the stack shrinks, components included.
- `event.user` is the original consuming entity, not necessarily the acting player. The
  handle can be stale before a world is published or if the entity was removed during an
  earlier effect.
- Replacing the stack during an effect does not change what later entries see. The finish
  does not shrink a replacement or generate a remainder from the invalidated use.
- You can apply effects, play sounds, teleport entities and drop items through the host.
- An unclaimed kind does nothing.
- Consumable and death protection payloads are decoded once for item definitions
  and stack component patches. The event borrows the retained payload. Player death
  protection uses this hook; mob death protection does not dispatch it.
- Invalid payloads refuse loading rather than failing during consumption.

To apply an effect without consuming an item, use
[`LivingRef::apply_consume_effect`](consumable-effects.md).

## Food eligibility

`Hooks::can_eat` receives the eating `player` on both sides, with `stack = None`
for cake and candle cake. Read food through `event.player.food(host)` and effects
through `host.living(event.player.entity())`. On the client these are the local
player's copies of server-sent state. Return `Some(true)` to permit eating,
`Some(false)` to refuse it, or `None` to keep `vanilla_can_eat`. Always-eat
eligibility comes from the food component, not a separate event field.
