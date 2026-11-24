//! The mod `tools/mod_pipeline_bench.py` measures: it registers [`PIPELINES`] render pipelines,
//! half of them textured, and draws [`QUADS_PER_STAGE`] quads through them at each of four world
//! render stages, every frame, in front of the camera. The driver writes the shaders
//! (`assets/pipeline_bench/shaders/pipeline/pNN.wgsl`) when it stages the mod, and the
//! benchmark's game (`crates/sunrise-mod-host/examples/mod_pipeline_bench.rs`) links it.
use mod_api::{
    ClientHost, ClientReloadHost, ClientResourcesReloadedEvent, ClientSetupContext,
    ClientSetupEvent, Hooks, Identifier, PipelineBlend, PipelineQuad, PipelineTexture,
    RegisterRenderPipelinesEvent, RegistrationContext, RenderAssets, RenderLevelStage,
    RenderLevelStageEvent, RenderPipeline, RenderPipelineRef, WorldDepth,
};
use std::sync::Mutex;

pub const ID: &str = "pipeline_bench";
pub const VERSION: &str = "0.1.0";
/// Pipelines registered; those from `PIPELINES / 2` on sample a block sprite.
pub const PIPELINES: usize = 32;
/// Quads drawn at each of [`STAGES`], spread over that stage's share of the pipelines. The four
/// stages' quads together stay under the view's quad limit.
pub const QUADS_PER_STAGE: usize = 500;
const STAGES: [RenderLevelStage; 4] = [
    RenderLevelStage::AfterOpaqueBlocks,
    RenderLevelStage::AfterTranslucentBlocks,
    RenderLevelStage::AfterWeather,
    RenderLevelStage::AfterLevel,
];
const PER_STAGE: usize = PIPELINES / STAGES.len();

fn id(index: usize) -> Identifier {
    Identifier::parse(&format!("{ID}:p{index:02}")).expect("bench pipeline id")
}

#[derive(Default)]
pub struct Bench {
    pipelines: Mutex<Vec<RenderPipelineRef>>,
    quads: Mutex<Vec<PipelineQuad>>,
}

impl Bench {
    fn resolve(&self, assets: &dyn RenderAssets) {
        let resolved: Vec<_> = (0..PIPELINES)
            .filter_map(|index| assets.render_pipeline(&id(index)).ok())
            .collect();
        if resolved.len() != PIPELINES {
            eprintln!(
                "pipeline_bench: {} of {PIPELINES} pipelines resolved",
                resolved.len()
            );
        }
        *self.pipelines.lock().unwrap() = resolved;
    }
}

#[mod_api::sunrise_mod]
impl Hooks for Bench {
    fn register_render_pipelines(
        &self,
        _context: &RegistrationContext,
        event: &mut RegisterRenderPipelinesEvent<'_>,
    ) {
        let stone = Identifier::parse("minecraft:block/stone").expect("stone sprite id");
        for index in 0..PIPELINES {
            let blend = if index % 2 == 0 {
                PipelineBlend::Translucent
            } else {
                PipelineBlend::Additive
            };
            let pipeline = RenderPipeline::new(id(index), id(index), blend, WorldDepth::Tested);
            event.register(if index >= PIPELINES / 2 {
                pipeline.textured(PipelineTexture::Sprite(stone.clone()))
            } else {
                pipeline
            });
        }
    }

    fn client_setup(&self, context: &mut dyn ClientSetupContext, _: &mut ClientSetupEvent) {
        self.resolve(context.render_assets());
    }

    fn client_resources_reloaded(
        &self,
        context: &dyn ClientReloadHost,
        _: &mut ClientResourcesReloadedEvent,
    ) {
        self.resolve(context.render_assets());
    }

    fn render_level_stage(&self, _: &dyn ClientHost, event: &mut RenderLevelStageEvent<'_>) {
        let Some(slot) = STAGES.iter().position(|stage| *stage == event.stage) else {
            return;
        };
        let pipelines = self.pipelines.lock().unwrap();
        if pipelines.len() != PIPELINES {
            return;
        }
        let [x, y, z] = event.view.camera;
        let mut quads = self.quads.lock().unwrap();
        // A wall of quads 8 blocks ahead along +z and one more block per stage, 25 across.
        let share = QUADS_PER_STAGE.div_ceil(PER_STAGE);
        for (n, pipeline) in pipelines[slot * PER_STAGE..][..PER_STAGE]
            .iter()
            .enumerate()
        {
            quads.clear();
            quads.extend(
                (n * share..QUADS_PER_STAGE.min((n + 1) * share)).map(|quad| {
                    let left = x + (quad % 25) as f64 * 0.5 - 6.25;
                    let bottom = y + (quad / 25) as f64 * 0.5 - 5.0;
                    let depth = z + 8.0 + slot as f64;
                    PipelineQuad::spanning(
                        [
                            [left, bottom, depth],
                            [left, bottom + 0.45, depth],
                            [left + 0.45, bottom + 0.45, depth],
                            [left + 0.45, bottom, depth],
                        ],
                        0x80FF_FFFF,
                    )
                }),
            );
            if let Err(error) = event.draw.pipeline_quads(*pipeline, &quads) {
                eprintln!("pipeline_bench: quads refused: {error:?}");
            }
        }
    }
}
