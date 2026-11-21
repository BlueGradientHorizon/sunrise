//! The `render_level_stage` hooks: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait DrawContexts {
    #[draw_context(
        surface = "crate::world_geometry::StageSurface<'_>",
        name = "RenderLevelStageContext"
    )]
    fn render_level_stage_context();
}

pub trait Hooks {
    /// Observe or draw world geometry at one stage of a world view (NeoForge 26.3's
    /// `RenderLevelStageEvent`), asked for each stage of `RenderLevelStage::frame_order` in
    /// frame order. `context` exposes the active stage capability: `StagePass::None` for
    /// no-pass stages (`AfterSky`, `AfterLevel`), `StagePass::World` for standard pass stages,
    /// and `StagePass::Oit` for `OitTranslucent` sub-passes. Client-only, noncancellable.
    #[notes = r#"## World render stages

`Hooks::render_level_stage` is the client counterpart of NeoForge 26.3's
`RenderLevelStageEvent`, covering all 9 stages as one hook with a `RenderLevelStage`:
- `AfterSky` (no active render pass, before `prepare_render_buffers`)
- `AfterOpaqueBlocks` (after solid and cutout chunk geometry, before opaque features)
- `AfterOpaqueFeatures` (after opaque entities, block entities and particles)
- When classic transparency is active (`!view.improved_transparency`):
  - `AfterTranslucentFeatures` (after translucent entities and block entities)
  - `AfterTranslucentBlocks` (after translucent chunk geometry)
  - `AfterTranslucentParticles` (after translucent particles)
  - `AfterWeather` (after rain and snow, before the world border)
- When improved transparency / OIT is active (`view.improved_transparency`):
  - `OitTranslucent(OitStage::DepthBounds)`
  - `OitTranslucent(OitStage::Transmittance)`
  - `OitTranslucent(OitStage::Accumulate)`
- `AfterLevel` (no active render pass, after the level finishes rendering)

Each frame asks `Hooks::submit_custom_geometry` and registered debug renderers first (with
`WorldDrawContext`, for `name_tag`, `draw_model_instances`, `draw_instance_group`, and custom
world primitives drawn at the end of the world pass), then `AfterSky`, then
`Hooks::prepare_render_buffers`, then each pass-bearing stage in `RenderLevelStage::frame_order`,
and finally `AfterLevel`. Never fires on title or connect screens.

`event.context` is a `RenderLevelStageContext`: `AfterSky` and `AfterLevel` return
`StagePass::None` (`has_render_pass() == false`); standard pass stages return
`StagePass::World(&mut dyn WorldPassDrawContext)` (`lines`, `quads`, `pipeline_quads`); and
`OitTranslucent(oit)` stages return `StagePass::Oit(oit, &mut dyn WorldOitPassDrawContext)`
(`oit_stage`, `lines`, `quads`, without `pipeline_quads` because mod render pipelines target the
main world colour format rather than OIT float targets).

A shader-pack route that skips a standard stage draws that stage's geometry at the next stage it
reaches instead, depth tested as usual, preserving stage order without dropping submissions."#]
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        contract = "listen",
        neoforge = "RenderLevelStageEvent",
        cancellable = false,
        version = 2
    )]
    fn render_level_stage(
        stage: mod_api::RenderLevelStage,
        view: &mod_api::WorldRenderView<'_>,
        context: &mut dyn RenderLevelStageContext,
    );
}
