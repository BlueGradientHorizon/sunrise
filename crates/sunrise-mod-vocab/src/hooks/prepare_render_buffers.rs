//! The `prepare_render_buffers` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Prepare or update retained render buffers before pass-bearing world render stages execute
    /// (NeoForge 26.3's `PrepareRenderBuffersEvent`, vanilla `LevelRenderer.prepareTranslucents`).
    /// Client-only, noncancellable. `view` lends the frame's camera, world partial tick, game
    /// time, improved-transparency state and visible sections.
    #[notes = r#"## Preparing render buffers

`Hooks::prepare_render_buffers` is the client counterpart of NeoForge 26.3's
`PrepareRenderBuffersEvent`. It fires once per world view, after `RenderLevelStage::AfterSky`
and before any pass-bearing world render stages execute, and never on title or connect screens.

Use it to prepare or update retained GPU buffers — such as `InstanceGroups` or dynamic textures —
before the render passes read them. Its `WorldRenderView` lends the frame's `Camera`, world
partial tick, game time, whether vanilla's improved transparency (OIT) is active this frame, and
the visible `RenderableSection` slice."#]
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        contract = "listen",
        neoforge = "PrepareRenderBuffersEvent",
        cancellable = false
    )]
    fn prepare_render_buffers(view: &mod_api::WorldRenderView<'_>);
}
