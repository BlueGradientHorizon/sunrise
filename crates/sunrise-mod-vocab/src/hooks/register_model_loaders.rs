//! The `register_model_loaders` hooks: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Register the model loaders a mod adds (NeoForge's `ModelEvent.RegisterLoaders`). Asked once
    /// at start on the client, before resources load. Use [`RegisterModelLoadersEvent::register`]
    /// to bind each loader identifier to its bake callback; a model whose `loader` names a
    /// registered id invokes that callback once per load or reload and bakes its quads with the
    /// model's elements. Any valid `<namespace>:<path>` identifier is accepted; duplicates and
    /// collisions with built-in loaders (`neoforge:composite`, `neoforge:obj`) are rejected as
    /// registration errors.
    #[notes = r#"## Model loaders

A model JSON may name a `loader`: NeoForge's `neoforge:composite` and `neoforge:obj` are built in, and a mod
registers one of its own (NeoForge's `ModelEvent.RegisterLoaders`) once at client startup via
`Hooks::register_model_loaders`. Call `event.register(id, loader)` to bind the loader identifier directly to
its bake callback; when a model naming that loader is resolved on each resource load and reload, the game
calls the bound loader directly without scanning other mods. A loader is for geometry the vanilla `elements`
can't express; a model that only needs cuboids uses `elements`.

```rust,ignore
fn register_model_loaders(
    &self,
    _context: &RegistrationContext,
    event: &mut RegisterModelLoadersEvent<'_>,
) {
    event.register(Identifier::parse("example:pillar").unwrap(), |_input| {
        // The north face of a 4-pixel post in the middle of the block.
        Ok(vec![ModelLoaderQuad {
            positions: [
                [0.625, 0.0, 0.375],
                [0.375, 0.0, 0.375],
                [0.375, 1.0, 0.375],
                [0.625, 1.0, 0.375],
            ],
            uvs: [[1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0]],
            texture: "side".into(),
            cull: None,
            tint_index: -1,
            shade: true,
        }])
    });
}
```

`ModelLoaderInput` hands the callback the model's identifier (`input.model`), its whole JSON object converted
to a `Tag` (`input.config`), and its resolved texture slots (`input.textures`). A quad's `texture` is a key
of the model's `textures`, resolved through its parents as an element face's is. Quads are in block units,
the way an obj model's are, and the blockstate variant's rotation moves them whole. Each model is asked once
per load.

Any valid `<namespace>:<path>` identifier (including `minecraft:`) is accepted. Duplicate registrations and
collisions with `neoforge:composite` or `neoforge:obj` are logged as registration errors and the first
registration stands. A loader ID written as a string, or as an object with `id` and no `optional` flag,
is required. When the loader is absent, only `"loader": {"id":"example:pillar","optional":true}` uses
ordinary cuboid geometry. A required loader that is absent or a present loader that returns `Err(reason)`
uses the missing model and logs the error. An empty successful quad list is not an error. Panicking also
disables that loader callback for the session and surfaces an error toast. A quad with a position or UV
that isn't finite, or a texture key the model doesn't define, is skipped by itself with a reason."#]
    #[hook(
        receiver = "associated",
        dispatch = "flat",
        side = "client",
        contract = "collect",
        out = "loaders",
        context = "registration",
        answered_by = "mod",
        registration_append = "custom",
        neoforge = "ModelEvent.RegisterLoaders",
        version = 2
    )]
    fn register_model_loaders(loaders: &mut Vec<Answered<ModelLoaderRegistration>>);
}
