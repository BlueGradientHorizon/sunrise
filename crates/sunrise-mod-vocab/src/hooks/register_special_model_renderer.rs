//! The `register_special_model_renderer` hook: declarations the mod API generator reads (see
//! `hooks/mod.rs`).

pub trait Hooks {
    /// Register decoders and bake factories for special item model types (NeoForge's
    /// `RegisterSpecialModelRendererEvent`). Each `minecraft:special` node gets its own baked
    /// renderer, which extracts a typed stack argument and submits geometry when drawn. Last
    /// registration wins, including for vanilla's types. Asked at client startup before resources load.
    #[notes = r#"Register each type with `event.renderers.register(id, factory)`. The `SpecialModelFactory`
decodes the inline `model` fields other than `type`, then bakes a renderer for that node and
resource load. Retain leases from `bake_model` or `bake_geometry` in the renderer.

For each draw, `SpecialModelRenderer::extract_argument` reads the borrowed `SpecialModelStack`
and returns its `Argument`. `submit` receives that argument and the actual draw context; use
`SpecialModelCollector` to submit geometry or nested item stacks under scoped poses. The `base`
model still supplies display transforms, particle and GUI light, and the engine handles foil,
packed light, overlay and outline.

Unknown types and decode or bake failures refuse the item definition without trying a lower
pack. During drawing, an error, panic or invalid geometry or pose rolls back the layer and
disables the offending type until resources reload. Unreadable input or a nested stack refusal
rolls back the enclosing layer without disabling it. Only the outermost refused layer draws a
missing model."#]
    #[hook(
        side = "client",
        contract = "listen",
        context = "registration",
        neoforge = "RegisterSpecialModelRendererEvent",
        version = 3
    )]
    fn register_special_model_renderer();
}
