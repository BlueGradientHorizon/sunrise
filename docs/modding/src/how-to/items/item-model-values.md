# Item model values

Item tint sources, custom item model types, and numeric or categorical item model
properties bind their providers at client startup, while conditional properties
evaluate through `Hooks::item_model_property`. Every callback receives an API-owned
`ItemStack` snapshot (`stack: &ItemStack`) and runtime `bundle_selected: i32`
(**-1 means none**); item models and range/select properties also receive
`context: &ItemModelContext`. The engine's resolver keeps its own `StackView`, which
is not part of `mod_api`. This applies to nested bundle contents as well as the outer
item being drawn.

| Feature | Registration / Hook | Decode input | Runtime callback input | Answer |
| --- | --- | --- | --- | --- |
| Item tint source (`tints`) | `Hooks::register_item_tint_sources` via `event.register(id, decode)` | `&Tag` (tint JSON object) | `ItemTintInput { stack, bundle_selected }` | `u32` (ARGB colour) |
| Custom item model type | `Hooks::register_item_models` via `event.register(id, decode)` | `&Tag` (model node JSON object) | `ItemModelUpdateInput { stack, bundle_selected, context }` + `&mut Vec<ItemModelLayer>` | Push `ItemModelLayer` values |
| Numeric `range_dispatch` property | `Hooks::register_range_select_item_model_property` via `event.register(id, decode)` | `&Tag` (`range_dispatch` JSON object) | `ItemModelPropertyInput { stack, bundle_selected, context }` | `f32` |
| Categorical `select` property | `Hooks::register_select_item_model_property` via `event.register(id, decode)` | `&Tag` (`select` JSON object) | `ItemModelPropertyInput { stack, bundle_selected, context }` | `Option<String>` |
| Conditional `condition` property | `Hooks::item_model_property` | none | `ItemModelPropertyEvent { property, stack, bundle_selected, value }` | `event.value: &mut bool` |

## Bind providers at registration

For item tint sources, custom item model types, range-select properties, and select
properties, register a decode-time factory once at client startup (before resources
load) with `event.register(id, decode)`.

On each resource load and reload, the game converts each matching JSON node into a
borrowed `Tag` once and invokes `decode(&Tag)`. Parse configuration fields with
`Tag::get` there and return `Ok(eval)` — the bound closure invoked per stack during
item resolution with `&dyn ClientHost` and the borrowed input struct. Resolving a
stack never re-parses or clones JSON configuration.

```rust,ignore
use mod_api::prelude::*;
use mod_api::{
    Identifier, ItemModelLayer, ItemTransformMode, RegisterItemModelsEvent,
    RegisterItemTintSourcesEvent, RegisterRangeSelectItemModelPropertyEvent,
    RegisterSelectItemModelPropertyEvent, RegistrationContext, Tag,
};

fn register_item_tint_sources(
    &self,
    _context: &RegistrationContext,
    event: &mut RegisterItemTintSourcesEvent<'_>,
) {
    event.register(Identifier::parse("example:charge").unwrap(), |config| {
        let default = match config.get("default") {
            Some(Tag::Int(value)) => *value as u32,
            _ => 0xFFFFFF,
        };
        Ok(move |_host, input| {
            let rgb = if input.stack.count() > 1 { 0x55FF55 } else { default };
            0xFF000000 | rgb
        })
    });
}

fn register_item_models(
    &self,
    _context: &RegistrationContext,
    event: &mut RegisterItemModelsEvent<'_>,
) {
    event.register(Identifier::parse("example:charge").unwrap(), |_config| {
        Ok(|_host, input, layers| {
            let name = if input.stack.count() > 1 { "full" } else { "empty" };
            layers.push(
                ItemModelLayer::new(name).with_item_transform(ItemTransformMode::Model),
            );
        })
    });
}

fn register_range_select_item_model_property(
    &self,
    _context: &RegistrationContext,
    event: &mut RegisterRangeSelectItemModelPropertyEvent<'_>,
) {
    event.register(Identifier::parse("example:fill").unwrap(), |_config| {
        Ok(|_host, input| input.stack.count() as f32 / 64.0)
    });
}

fn register_select_item_model_property(
    &self,
    _context: &RegistrationContext,
    event: &mut RegisterSelectItemModelPropertyEvent<'_>,
) {
    event.register(Identifier::parse("example:mode").unwrap(), |_config| {
        Ok(|_host, _input| Some("dig".into()))
    });
}
```

### Namespace, override, and failure rules

- Any valid `<namespace>:<path>` identifier (including `minecraft:`) is accepted.
- Duplicate registrations use last-writer-wins replacement (matching NeoForge's
  `LateBoundIdMapper`).
- Registered providers are checked ahead of hardcoded vanilla built-ins during item
  definition decode, so registering a vanilla identifier overrides that built-in.
- Returning `Err(reason)` from `decode` rejects the enclosing item definition with a
  diagnostic without falling back to a built-in, and does not disable the provider.
- A panic in a decoder or per-stack callback disables that mod's callback for the
  session, logs an error, and shows an error toast naming the mod.

## Conditional properties (`item_model_property`)

Conditional `minecraft:condition` properties still evaluate through
`Hooks::item_model_property`. Match `event.property` for names your mod owns, write
`event.value`, and leave other names untouched. Unanswered callbacks retain `false`,
and dispatch is skipped when no loaded mod subscribes.

## Read the stack and context

`bundle_selected` is the runtime bundle selection index (**-1 means none**). It is
not a saved component and is deliberately separate from the stack's component patch.
The patch contains explicit overrides and removals, not merged defaults.

| Accessor | Value |
| --- | --- |
| `input.stack.item()` | `ItemKey`; retain keys for comparisons. |
| `input.stack.count()` | Stack count. |
| `input.stack.components()` | Optional borrowed component patch owned by the snapshot. |
| `input.stack.get(&descriptor)` | Typed component value. |
| `input.bundle_selected` | Runtime bundle selection (`-1` when none), separate from components. |
| `input.context` | Borrowed `&ItemModelContext` (`display`, `owner`, `level`, `seed`) for item models and range/select properties. |

Snapshot construction occurs only when a bound provider (or subscribed
`item_model_property` listener) is actually invoked, so vanilla item resolution with
no custom provider allocates no snapshot. An empty or unresolved resolver view does
not dispatch an invalid stack; its property or tint retains the unanswered default.
