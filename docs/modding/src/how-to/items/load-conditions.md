# Registered data load conditions

Implement `LoadCondition` as a handler that reads the condition's JSON, then
register it from
`Hooks::register_load_conditions` using its `RegistrationContext` and
`event.conditions.register(Identifier::parse("testmod:flag").unwrap(), Flag)`.
Registration happens once per mod load, with one handler per id and a read-only
`RegistrationContext`. Any mod may register an unused ID in any namespace:
mod B can register `a:x` while mod A registers `a:y`. Built-in IDs such as
`neoforge:always`, `neoforge:never` and `neoforge:mod_loaded` cannot be replaced.
A duplicate ID names both mods in the error. A registration error refuses
admission even if you ignore it.

```rust,ignore
struct Flag;
impl mod_api::LoadCondition for Flag {
    fn test(&self, args: &mod_api::serde_json::Value)
        -> Result<bool, mod_api::ConditionError>
    {
        args["value"].as_bool().ok_or_else(|| mod_api::ConditionError {
            message: "value must be a boolean".into(),
        })
    }
}
```

A recipe can use the registered name alongside the built-in conditions:

```json
{
  "type": "minecraft:crafting_shapeless",
  "ingredients": ["minecraft:flint"],
  "result": {"id": "minecraft:stick"},
  "neoforge:conditions": [{"type": "testmod:flag", "value": true}]
}
```

The handler receives a borrowed raw JSON value: the condition's object without
`type`. It reads and validates its own arguments, retaining JSON booleans,
numbers, strings, lists and objects. Invalid mod input must return
`ConditionError`; handlers must not panic while reading it.
Startup data loaders and the server's candidate data reload both use registered
handlers before entries become visible. Results are retained for each file and
condition payload in that load. Built-in reads of feature flags, registries or
tags also distinguish different context views. A string in a custom handler's
arguments does not count as such a read. A new load reevaluates the entry.
Handlers must not mutate live data.
A handler panic becomes a load error naming the condition and its owning mod.
It is logged and queues an error toast. Only that handler is disabled for the
rest of the session, including after a reload. An evaluation that reaches the
disabled handler reports an error without running it. A returned
`ConditionError` or an ordinary phase refusal is reported by the loader but
leaves the handler available for the next evaluation.

`true` loads the entry; `false` omits it, without exposing a lower pack's
version. The loader handles callback errors and unknown condition types as
[described in Conditional data](conditional-data.md#errors-by-loader).

Registering an ID works the same in every namespace. The evaluator decodes the
whole conditions array before testing it, including children of `not`, `and`
and `or`. Neither `neoforge:mod_loaded` nor an already true or false branch
can guard an unknown condition ID. Testing goes left to right and skips
handlers after a false condition in an array or `and`, or after a true
condition in `or`.

The public [NeoForge data load condition documentation](https://docs.neoforged.net/docs/resources/server/conditions/)
describes `ICondition` and the top-level `neoforge:conditions` array.
