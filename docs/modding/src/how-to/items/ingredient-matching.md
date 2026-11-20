# Match custom ingredients

Implement `Hooks::ingredient_test(context, event)` to check a concrete stack's components for your [custom ingredient kind](ingredient-kinds.md). Calling `Ingredient::test(host, stack)` reaches this hook after any nonempty load-time candidate list accepts the stack's item.

Set `event.accept` to `false` to reject the stack. It starts as `true`.

## Callback contract

- `context` is the loop context of the side performing the match. The hook itself takes no `Host`.
- The event supplies the ingredient kind, the metadata compound retained from the load-time hook and the stack being matched. The load-time candidate answer is separate from that metadata.
- The callback can run many times during crafting. Give the same answer for the same stack and ingredient.
- An item absent from a nonempty load-time list fails without this callback. An empty custom list still invokes the predicate: manual crafting can match an accepted stack, although recipe-book placement, property sets and stonecutter selection are unavailable.

A client without this mod predicts vanilla here; the server corrects it.

Read `event.data` as an API `Compound` with typed `Tag` variants (`Int`, `Long`, `List`, `Compound`, and others). The loader converts recipe metadata once using vanilla JsonOps-to-NbtOps rules; matching does not reconvert it. Numeric JSON arrays remain lists. Nested null values refuse the ingredient and fail the recipe during loading.

## Borrowed inputs for bound predicates

`IngredientDefinition::new(items, test)` pairs the ordered item IDs with a test that takes
`(&dyn LoopContext, IngredientStack<'_>)` and returns whether the stack matches. The test can
capture parsed configuration in a `move` closure; its captures must be `Send + Sync + 'static`.
The existing `ingredient` and `ingredient_test` hooks still work as above.

On an `IngredientStack`, `item_id()` reads the namespaced ID and `item_key()` gives the resolved
key when available. `count()` reads the live count. `damage()` returns `Some(0)` when zero damage
is present, and `None` for an absent or removed damage component. Ask
`has_component("minecraft:custom_data")` about effective presence and `component_removed(id)`
about an explicit removal. Use `stored_component(id)` to borrow NBT already stored on the stack,
such as custom data or an opaque mod component. This does not encode typed fields or return
inherited defaults, so a component can be present without a stored NBT value. Neither the stack
view nor its NBT borrow can outlive the call.
