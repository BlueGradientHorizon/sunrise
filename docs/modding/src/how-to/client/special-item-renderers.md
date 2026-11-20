# Drawing an item with a special model renderer

Use `register_special_model_renderer` (NeoForge's `RegisterSpecialModelRendererEvent`) for an item
whose ordinary item model cannot describe its drawing, such as a skillet with moving food.
Register a factory for a `minecraft:special` model type before client resources load.

## Decode, bake, then draw

`SpecialModelFactory::new(decode, bake)` pairs a decoder and a baker. The decoder receives
`SpecialModelConfig`, the fields of the inline `model` object except `type`. The baker consumes
that typed configuration and can retain geometry leases from `bake_model` (a selected pack model)
or `bake_geometry` (custom `SpecialModelQuad`s referencing atlas sprites). Each model node has
its own baked renderer; a reload makes new instances and leases.

On each draw, `extract_argument` receives the `ClientHost` and a borrowed `SpecialModelStack`.
Its `item()`, `count()` and `get(&descriptor)` read only what you ask for, including inherited
components unless removed by the stack. If the node came from a bundle's selected item, this
is that item, not the bundle. `submit` receives the typed argument and the current display context,
level game time (if in a world) and partial tick. Drawing does not decode or bake models again,
and it does not keep answers between draws.

This example uses serde's `derive` feature:

```rust,ignore
use mod_api::*;

#[derive(serde::Deserialize)]
struct SkilletConfig {
    food_height: f32,
}

struct Skillet {
    pan: SpecialModelGeometry,
    food: SpecialModelGeometry,
    food_height: f32,
}

impl SpecialModelRenderer for Skillet {
    type Argument = bool;

    fn extract_argument(&self, _host: &dyn ClientHost, stack: &SpecialModelStack<'_>) -> bool {
        stack.count() > 1
    }

    fn submit(
        &self,
        show_food: &bool,
        _context: &SpecialModelContext,
        collector: &mut SpecialModelCollector<'_>,
    ) -> Result<(), String> {
        collector.submit_geometry(&self.pan, SpecialModelTints::Untinted)?;
        if *show_food {
            collector.with_pose(
                [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, self.food_height, 0.0, 0.0, 1.0, 0.0],
                |child| child.submit_geometry(&self.food, SpecialModelTints::Untinted),
            )?;
        }
        Ok(())
    }
}

#[mod_api::sunrise_mod]
impl Hooks for MyMod {
    fn register_special_model_renderer(
        &self,
        _context: &RegistrationContext,
        event: &mut RegisterSpecialModelRendererEvent<'_>,
    ) {
        event.renderers.register(
            "example:skillet".into(),
            SpecialModelFactory::new(
                |config| config.decode::<SkilletConfig>(),
                |config, bake| Ok(Skillet {
                    pan: bake.bake_model("example:item/skillet_pan")?,
                    food: bake.bake_model("example:item/skillet_food")?,
                    food_height: config.food_height,
                }),
            ),
        );
    }
}
```

The item definition names the registered type and a `base` model:

```json
{"model": {"type": "minecraft:special", "base": "example:item/skillet",
  "model": {"type": "example:skillet", "food_height": 0.0625}}}
```

## What to submit

- `submit_geometry(&lease, tints)` draws a lease from this resource load. Use `Untinted`,
  `Uniform(0xAARRGGBB)` for every quad, or `Indexed(Arc<[u32]>)` for a shared palette by quad
  tint index. With an indexed palette, unindexed quads and missing entries draw white.
- `with_pose(transform, |child| ...)` lends a collector under a finite row-major 3×4 affine.
  The current pose multiplies the child transform on the left; on return, the parent pose is
  unchanged. Model space is in blocks from the model's corner (element coordinates divided by 16).
- `submit_stack(&stack, ItemDisplayContext::Fixed)` resolves a nested item's own models, tints
  and foil, using its display transform. The current pose places its centre. A cycle, an
  unreadable stack or exhausting `MAX_NESTED_STACK_PARTS` refuses the enclosing layer.

The game draws the geometry with the `base` model's GUI light, particle and display transforms,
plus the node's own `transformation`. It carries the outer stack's foil and the draw's packed
light, overlay and outline through the ordinary item draw path.

## Registrations and refusals

The last registration of a fully qualified type (`namespace:path`) wins, including replacements
for vanilla types. Unknown types, errors and panics during decode or bake refuse the enclosing
item definition. The item draws the missing model instead; the game does not try a lower pack.

A draw error, panic, or invalid pose or geometry submission rolls back the whole special layer.
The offending type is logged and disabled until client resources reload. Unreadable input or a
nested refusal also rolls back the enclosing layer, even if a mod ignores the nested error, but
it does not disable the enclosing renderer. Only the outermost refused special layer draws a
missing model; unrelated layers of a composite item still draw.
