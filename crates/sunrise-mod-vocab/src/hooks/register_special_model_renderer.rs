//! The `register_special_model_renderer` hook: declarations the mod API generator reads (see
//! `hooks/mod.rs`).

pub trait Hooks {
    /// Register decoders and bake factories for special item model types (NeoForge's
    /// `RegisterSpecialModelRendererEvent`). Each `minecraft:special` model node of a registered
    /// type gets its own baked `SpecialModelRenderer`. Last registration wins, including for
    /// vanilla's types. Asked once at client startup, before resources load.
    #[notes = r#"## Special item model renderers

`Hooks::register_special_model_renderer` registers a `SpecialModelFactory` for each type before
client resources load. Its decoder reads the inline `model` fields other than `type`; its bake
callback turns that typed config into a renderer for this node and resource load. Drawing uses
that instance, without decoding or baking it again. The example uses serde with `derive`:

```rust,ignore
use mod_api::*;

#[derive(serde::Deserialize)]
struct SkilletConfig {
    food_height: f32,
}

struct Skillet {
    food_height: f32,
}
impl SpecialModelRenderer for Skillet {
    fn extract(
        &self,
        _host: &dyn ClientHost,
        stack: &ItemStack,
        _context: &SpecialModelContext,
    ) -> Result<Vec<SpecialModelPart>, String> {
        let mut parts = vec![SpecialModelPart::new("example:item/skillet_pan")];
        if stack.count() > 1 {
            parts.push(
                SpecialModelPart::new("example:item/skillet_food").translated([0.0, self.food_height, 0.0]),
            );
        }
        Ok(parts)
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
            SpecialModelPolicy::Stack,
            SpecialModelFactory::new(
                |config| config.decode::<SkilletConfig>(),
                |config, _bake| Ok(Skillet { food_height: config.food_height }),
            ),
        );
    }
}
```

The item definition names the type in a `minecraft:special` model, with a `base` model:

```json
{"model": {"type": "minecraft:special", "base": "example:item/skillet",
  "model": {"type": "example:skillet", "food_height": 0.0625}}}
```

Each part names a model file of the packs, a row-major 3×4 transform in the special model's space (blocks, with the
origin at the model's corner, as an element's coordinates over 16) and its tints per tint index (ARGB, as an
item model's `tints`). The renderer never sees the `base` model: the game draws the parts with its display
transforms, particle and GUI light, as vanilla does, and the node's `transformation` applies to all of them.

Registration names a `SpecialModelPolicy`. `Stack` asks the renderer on the client's main thread when an item
of such a model is drawn with a stack other than the last one that model was asked about: an unchanged stack is
asked once, however many frames draw it, and every display context (a hand, a screen, the ground, a frame, a
head) draws that answer; the `context` it is given is always `SpecialModelContext::STACK`. This caching is a
deliberate difference from vanilla, which extracts every frame. `Frame` asks once per frame of the view for
each display context and game time the item is drawn with, and `context` carries the display context, the
level's game time (`None` outside a world) and the partial tick: for an ingredient that rises and turns over
a few ticks and must move smoothly between them. There is no cap on animated items: a `Frame` renderer costs one `extract` per drawn item per frame, for each
display context the item is drawn in (the same item in a hand and in an item frame is asked twice). A `Frame`
answer is not kept past its frame, and an item
drawn with one is never cached as a still GUI image. Nothing is asked, and no scan runs, for a model whose
type no mod registered.

A later registration of a type replaces the earlier one, whichever mod made it and vanilla's types included,
as in NeoForge; the log names both mods. An id that is not `<namespace>:<path>` is logged and left out. A
special model of a type no mod registered and vanilla doesn't have is an error naming the type and the item:
that item draws the missing model, and the rest of the resources load. An error or panic during decode or bake
rejects the enclosing item definition, never falling back to a lower pack. A part can also be an item stack (`SpecialModelPart::stack`), resolved through the item model resolver with the
inner item's own models, tints and foil under the display context you name; a cycle or more than
`MAX_NESTED_STACK_PARTS` (64) stack parts in one draw refuses the answer. A renderer that returns an `Err`,
panics, returns more than `MAX_SPECIAL_MODEL_PARTS` (256) parts or returns a non-finite transform
is logged once with its type and asked no more until resources reload. A part naming a model no
pack has refuses the answer, logged once for the model. A nested refusal discards the enclosing
special layer's parts too. The outermost special layer draws one missing model; unrelated item
layers are unchanged."#]
    #[hook(
        side = "client",
        contract = "listen",
        context = "registration",
        neoforge = "RegisterSpecialModelRendererEvent",
        version = 2
    )]
    fn register_special_model_renderer();
}
