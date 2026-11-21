//! Per-view custom geometry; buffers pre-reserve initial space and grow dynamically.
use crate::gfx::mod_pipelines::{self, PipelineRun, PipelineVertex};
use crate::gfx::world_primitives::{Batch, DEFAULT_CAPACITY as CAPACITY, STAGES};
use crate::{gizmos, mesher::Vertex};
use glam::{DVec3, Mat4, Vec3, Vec4};
use mod_api::{
    DrawError, InstanceError, ModelInstance, PipelineDrawError, PipelineQuad, RenderLevelStage,
    WorldDepth, WorldLine, WorldQuad,
};

/// Initial pre-reservation hint for model copies per view; storage grows dynamically.
pub(crate) const INSTANCE_CAPACITY: u32 = 16_384;

/// One accepted copy, already checked. `light` and `overlay` are the renderer's vanilla packing.
#[derive(Clone, Copy)]
pub struct ModelCopy {
    pub slot: u32,
    pub generation: u64,
    pub origin: [f64; 3],
    pub transform: [f32; 12],
    pub tint: u32,
    pub light: u32,
    pub overlay: u32,
    /// `ModelInstance::color` and `hidden_faces`, as given.
    pub color: u32,
    pub hidden_faces: u8,
}

/// The model a batch draws. Resolved once, then applied to every copy.
#[derive(Clone, Copy)]
pub struct ModelBatch {
    pub slot: u32,
    pub generation: u64,
    pub bounds: [[f32; 3]; 2],
}

/// One label's span of the name-tag vertices, and its squared distance from the camera.
#[derive(Clone, Copy, Debug)]
struct LabelRun {
    distance_sq: f64,
    start: u32,
    end: u32,
}

/// Reused between views, including frames with no subscribers or output.
pub struct WorldGeometry {
    camera: [f64; 3],
    view: Mat4,
    white: [f32; 2],
    lines: [Vec<[f32; 8]>; 2],
    quads: [Vec<Vertex>; 2],
    /// Quads drawn by mod pipelines, 4 vertices each, and the runs that share a pipeline and
    /// a stage.
    pipeline_quads: Vec<PipelineVertex>,
    pipeline_runs: Vec<PipelineRun>,
    /// Labels (`name_tag`), 4 vertices a quad: drawn with the world's name tags, then their
    /// see-through copies.
    labels: [Vec<Vertex>; 2],
    /// Each label's span of `labels[0]` and its squared distance from the camera, so the
    /// translucent name tags draw far to near ([`Self::sort_labels`]).
    label_runs: Vec<LabelRun>,
    /// Reused while [`Self::sort_labels`] reorders `labels[0]`.
    label_scratch: Vec<Vertex>,
    /// `PipelineFrame.time` for this view.
    time: f32,
    copies: Vec<ModelCopy>,
    groups: std::collections::HashSet<mod_api::InstanceGroupRef>,
    model_cut: u32,
    /// The stage lines and quads go to now (`RenderLevelStage::ALL`'s index).
    stage: usize,
    /// Where each stage before `stage` ended (`Batch::stage_ends`).
    stage_ends: [[u32; 4]; STAGES - 1],
}
impl Default for WorldGeometry {
    fn default() -> Self {
        Self {
            camera: [0.0; 3],
            view: Mat4::IDENTITY,
            white: [0.0; 2],
            lines: std::array::from_fn(|_| Vec::with_capacity(CAPACITY as usize)),
            quads: std::array::from_fn(|_| Vec::with_capacity(CAPACITY as usize * 4)),
            pipeline_quads: Vec::with_capacity(CAPACITY as usize * 4),
            pipeline_runs: Vec::with_capacity(CAPACITY as usize),
            labels: std::array::from_fn(|_| Vec::with_capacity(CAPACITY as usize * 4)),
            label_runs: Vec::with_capacity(CAPACITY as usize),
            label_scratch: Vec::with_capacity(CAPACITY as usize * 4),
            time: 0.0,
            copies: Vec::with_capacity(INSTANCE_CAPACITY as usize),
            groups: std::collections::HashSet::with_capacity(INSTANCE_CAPACITY as usize),
            model_cut: 0,
            stage: 0,
            stage_ends: [[0; 4]; STAGES - 1],
        }
    }
}
impl WorldGeometry {
    /// Start a view even if nothing will be submitted; previous output disappears.
    pub fn reset(&mut self, camera: [f64; 3], view: Mat4, white: [f32; 2]) {
        self.camera = camera;
        self.view = view;
        self.white = white;
        self.lines.iter_mut().for_each(Vec::clear);
        self.quads.iter_mut().for_each(Vec::clear);
        self.pipeline_quads.clear();
        self.pipeline_runs.clear();
        self.labels.iter_mut().for_each(Vec::clear);
        self.label_runs.clear();
        self.clear_copies();
        self.model_cut = 0;
        self.groups.clear();
        self.stage = 0;
        self.stage_ends = [[0; 4]; STAGES - 1];
    }
    /// From now on, lines and quads draw at `stage`: the stages before it end here. Stages
    /// come in frame order, so a stage before the current one is refused and keeps the current.
    pub fn begin_stage(&mut self, stage: RenderLevelStage) {
        let index = match stage {
            RenderLevelStage::AfterSky | RenderLevelStage::AfterOpaqueBlocks => {
                crate::gfx::world_primitives::STAGE_AFTER_OPAQUE_BLOCKS
            }
            RenderLevelStage::AfterTranslucentBlocks => {
                crate::gfx::world_primitives::STAGE_AFTER_TRANSLUCENT_BLOCKS
            }
            RenderLevelStage::AfterWeather => crate::gfx::world_primitives::STAGE_AFTER_WEATHER,
            RenderLevelStage::AfterLevel => crate::gfx::world_primitives::STAGE_CUSTOM_GEOMETRY,
            _ => STAGES - 1,
        };
        let counts = self.counts();
        for ended in self.stage..index.min(STAGES - 1) {
            self.stage_ends[ended] = counts;
        }
        self.stage = self.stage.max(index);
    }
    /// The game time this view's pipeline shaders read, in seconds (`PipelineFrame.time`).
    pub fn set_time(&mut self, seconds: f32) {
        self.time = seconds;
    }
    /// Entity-stream cut used by directly staged instances in this view.
    pub fn set_model_cut(&mut self, cut: u32) {
        self.model_cut = cut;
    }
    pub fn model_cut(&self) -> u32 {
        self.model_cut
    }
    pub fn camera(&self) -> [f64; 3] {
        self.camera
    }
    /// Cull each copy against the frustum first, then skip individual visible copies whose
    /// coordinates/transform are non-finite or whose light channels exceed 15 without rolling
    /// back earlier valid copies. `stage` returns true for copies it consumes directly.
    pub fn submit_model(
        &mut self,
        batch: ModelBatch,
        instances: &[ModelInstance],
        cull: Option<&impl crate::gfx::Cull>,
        mut stage: impl FnMut(&ModelCopy) -> bool,
    ) {
        self.submit_model_inner(batch, instances, cull, true, |_, copy| stage(copy));
    }
    /// Submit copies already validated when written into persistent storage.
    pub fn submit_validated_model(
        &mut self,
        batch: ModelBatch,
        instances: &[ModelInstance],
        cull: Option<&impl crate::gfx::Cull>,
        stage: impl FnMut(usize, &ModelCopy) -> bool,
    ) {
        self.submit_model_inner(batch, instances, cull, false, stage);
    }
    fn submit_model_inner(
        &mut self,
        batch: ModelBatch,
        instances: &[ModelInstance],
        cull: Option<&impl crate::gfx::Cull>,
        check_valid: bool,
        mut stage: impl FnMut(usize, &ModelCopy) -> bool,
    ) {
        for (index, instance) in instances.iter().enumerate() {
            if !copy_visible(
                batch.bounds,
                instance.transform,
                instance.origin,
                self.camera,
                cull,
            ) {
                continue;
            }
            if check_valid && !instance_valid(instance) {
                continue;
            }
            let copy = ModelCopy {
                slot: batch.slot,
                generation: batch.generation,
                origin: instance.origin,
                transform: instance.transform,
                tint: instance.tint,
                light: pack_light(instance.light),
                overlay: pack_overlay(instance.overlay),
                color: instance.color,
                hidden_faces: instance.hidden_faces,
            };
            if !stage(index, &copy) {
                self.copies.push(copy);
            }
        }
    }
    pub fn has_group(&self, group: mod_api::InstanceGroupRef) -> bool {
        self.groups.contains(&group)
    }
    pub fn mark_group(&mut self, group: mod_api::InstanceGroupRef) {
        self.groups.insert(group);
    }
    pub fn copy_count(&self) -> usize {
        self.copies.len()
    }
    pub fn copy(&self, index: usize) -> ModelCopy {
        self.copies[index]
    }
    pub fn clear_copies(&mut self) {
        self.copies.clear();
    }
    /// Validate once at the submission boundary, before any output is appended.
    fn validate_positions(&self, points: impl Iterator<Item = [f64; 3]>) -> Result<(), DrawError> {
        if points
            .flatten()
            .enumerate()
            .any(|(i, p)| !p.is_finite() || !((p - self.camera[i % 3]) as f32).is_finite())
        {
            return Err(DrawError::NonFinite);
        }
        Ok(())
    }
    /// Append an entire line batch or leave the output unchanged.
    pub fn lines(&mut self, lines: &[WorldLine]) -> Result<(), DrawError> {
        self.validate_positions(lines.iter().flat_map(|l| [l.from, l.to]))?;
        for l in lines {
            if !l.width.is_finite() {
                return Err(DrawError::NonFinite);
            }
            if l.width <= 0.0 {
                return Err(DrawError::InvalidWidth);
            }
        }
        for l in lines {
            let line = gizmos::Line {
                start: l.from,
                end: l.to,
                color: l.color,
                width: l.width,
            };
            let out = &mut self.lines[usize::from(l.depth == WorldDepth::ThroughWalls)];
            gizmos::camera_relative_to(&[line], self.camera, self.view, |line| out.push(line));
        }
        Ok(())
    }
    /// Append an entire quad batch or leave the output unchanged.
    pub fn quads(&mut self, quads: &[WorldQuad]) -> Result<(), DrawError> {
        self.validate_positions(quads.iter().flat_map(|q| q.corners))?;
        for q in quads {
            let color = crate::world_primitives::vertex_color(q.color, 255);
            self.quads[usize::from(q.depth == WorldDepth::ThroughWalls)].extend(q.corners.map(
                |p| Vertex {
                    pos: std::array::from_fn(|i| (p[i] - self.camera[i]) as f32),
                    uv: self.white,
                    color,
                    layer: 0,
                    light: 0,
                    normal: [0; 4],
                    glint_uv: [0.0; 2],
                },
            ));
        }
        Ok(())
    }
    /// Refuses a label's position before its text is read or laid out.
    pub fn check_label(&self, at: [f64; 3]) -> Result<(), DrawError> {
        self.validate_positions(std::iter::once(at))
    }
    /// Append one label's quads, laid out by the caller (`world` drawn with the name tags,
    /// `see_through` over everything). `distance_sq` is the label's squared distance from
    /// the camera, by which the name tags are sorted.
    pub fn label(&mut self, distance_sq: f64, world: &[Vertex], see_through: &[Vertex]) {
        if !world.is_empty() {
            let start = self.labels[0].len() as u32;
            self.label_runs.push(LabelRun {
                distance_sq,
                start,
                end: start + world.len() as u32,
            });
        }
        self.labels[0].extend_from_slice(world);
        self.labels[1].extend_from_slice(see_through);
    }
    /// Orders the labels drawn with the name tags far to near, as vanilla sorts its translucent
    /// name tags (`NameTagFeatureRenderer`): equally far ones keep their submission order.
    /// Their see-through copies are not depth tested and keep submission order.
    pub fn sort_labels(&mut self) {
        if self
            .label_runs
            .windows(2)
            .all(|pair| pair[0].distance_sq >= pair[1].distance_sq)
        {
            return;
        }
        self.label_runs
            .sort_by(|a, b| b.distance_sq.total_cmp(&a.distance_sq));
        let mut sorted = std::mem::take(&mut self.label_scratch);
        sorted.clear();
        for run in &mut self.label_runs {
            let start = sorted.len() as u32;
            sorted.extend_from_slice(&self.labels[0][run.start as usize..run.end as usize]);
            (run.start, run.end) = (start, sorted.len() as u32);
        }
        self.label_scratch = std::mem::replace(&mut self.labels[0], sorted);
    }
    /// This view's labels: those drawn with the name tags, then their see-through copies.
    pub fn labels(&self) -> [&[Vertex]; 2] {
        [&self.labels[0], &self.labels[1]]
    }
    /// Append an entire batch of quads drawn by the mod pipeline in `slot`, or leave the
    /// output unchanged. The caller has resolved the handle; quads that follow the last ones
    /// with the same pipeline and stage join their run.
    pub fn pipeline_quads(
        &mut self,
        slot: u32,
        quads: &[PipelineQuad],
    ) -> Result<(), PipelineDrawError> {
        self.validate_positions(quads.iter().flat_map(|q| q.corners))
            .map_err(|_| PipelineDrawError::NonFinite)?;
        if quads
            .iter()
            .flat_map(|q| q.uv)
            .flatten()
            .any(|c| !c.is_finite())
        {
            return Err(PipelineDrawError::NonFinite);
        }
        if quads.is_empty() {
            return Ok(());
        }
        for q in quads {
            let color = crate::world_primitives::vertex_color(q.color, 255);
            self.pipeline_quads
                .extend((0..4).map(|corner| PipelineVertex {
                    pos: std::array::from_fn(|i| (q.corners[corner][i] - self.camera[i]) as f32),
                    uv: q.uv[corner],
                    color,
                }));
        }
        let (stage, count) = (self.stage as u32, quads.len() as u32);
        match self.pipeline_runs.last_mut() {
            Some(run) if run.slot == slot && run.stage == stage => run.quads += count,
            _ => self.pipeline_runs.push(PipelineRun {
                slot,
                stage,
                quads: count,
            }),
        }
        Ok(())
    }
    /// Borrow the existing renderer's batch, with no copying or frame allocation. The frame's
    /// builder names the pipeline set the queued slots belong to.
    pub fn batch(&self) -> Batch<'_> {
        Batch {
            lines: &self.lines[0],
            lines_through: &self.lines[1],
            quads: &self.quads[0],
            quads_through: &self.quads[1],
            stage_ends: self.stage_ends(),
            oit: Default::default(),
            pipelines: mod_pipelines::Batch {
                set: None,
                vertices: &self.pipeline_quads,
                runs: &self.pipeline_runs,
                time: self.time,
            },
        }
    }
    /// [`Self::batch`] for a frame drawn with the resources that built `pipelines`, the set
    /// the queued pipeline quads' slots were resolved in.
    pub fn batch_with<'a>(&'a self, pipelines: &'a crate::mod_pipelines::Pipelines) -> Batch<'a> {
        let mut batch = self.batch();
        batch.pipelines.set = Some(pipelines);
        batch
    }
    /// Quads, quads through walls, lines and lines through walls queued so far.
    fn counts(&self) -> [u32; 4] {
        [
            self.quads[0].len() as u32 / 4,
            self.quads[1].len() as u32 / 4,
            self.lines[0].len() as u32,
            self.lines[1].len() as u32,
        ]
    }
    /// The ends of the stages before the current one; those not reached yet end at the counts
    /// so far, so what follows draws at the current stage.
    fn stage_ends(&self) -> [[u32; 4]; STAGES - 1] {
        let counts = self.counts();
        std::array::from_fn(|stage| {
            if stage < self.stage {
                self.stage_ends[stage]
            } else {
                counts
            }
        })
    }
}

/// Row-major 3×4 to a column-major matrix. The last row is the affine 0,0,0,1.
#[inline]
pub fn row_major_3x4(m: [f32; 12]) -> Mat4 {
    Mat4::from_cols(
        Vec4::new(m[0], m[4], m[8], 0.0),
        Vec4::new(m[1], m[5], m[9], 0.0),
        Vec4::new(m[2], m[6], m[10], 0.0),
        Vec4::new(m[3], m[7], m[11], 1.0),
    )
}

/// Block-entity vertex light: sky in the high nibble and block in the low one.
pub fn pack_light(light: mod_api::LightCoords) -> u32 {
    crate::block_entity_render::vertex_light((light.sky << 4) | light.block)
}

/// `LivingEntityRenderer.getOverlayCoords` as the entity shader's overlay code.
pub fn pack_overlay(overlay: mod_api::Overlay) -> u32 {
    if overlay.hurt {
        return 1;
    }
    match u32::from(overlay.white.min(15)) {
        0 => 0,
        column => crate::overlay::WHITE + column,
    }
}

fn copy_visible(
    bounds: [[f32; 3]; 2],
    transform: [f32; 12],
    origin: [f64; 3],
    camera: [f64; 3],
    cull: Option<&impl crate::gfx::Cull>,
) -> bool {
    let Some(cull) = cull else {
        return true;
    };
    if !bounds.iter().flatten().all(|v| v.is_finite()) {
        return true;
    }
    let local = row_major_3x4(transform);
    let delta = DVec3::from(origin) - DVec3::from(camera);
    let shift = Vec3::new(delta.x as f32, delta.y as f32, delta.z as f32);
    // Affine extrema are selected independently for each output axis. Keep the
    // multiply/add order of transform_point3, without transforming eight corners.
    let x0 = local.x_axis.truncate() * bounds[0][0];
    let x1 = local.x_axis.truncate() * bounds[1][0];
    let y0 = local.y_axis.truncate() * bounds[0][1];
    let y1 = local.y_axis.truncate() * bounds[1][1];
    let z0 = local.z_axis.truncate() * bounds[0][2];
    let z1 = local.z_axis.truncate() * bounds[1][2];
    let translation = local.w_axis.truncate();
    let mut min = translation + (z0.min(z1) + (y0.min(y1) + x0.min(x1))) + shift;
    let mut max = translation + (z0.max(z1) + (y0.max(y1) + x0.max(x1))) + shift;
    if !min.is_finite() || !max.is_finite() {
        // Overflow can produce NaNs whose min/max depends on corner order.
        // Preserve that order only for these exceptional finite inputs.
        min = Vec3::splat(f32::INFINITY);
        max = Vec3::splat(f32::NEG_INFINITY);
        for corner in 0..8 {
            let point = Vec3::new(
                bounds[usize::from(corner & 1 != 0)][0],
                bounds[usize::from(corner & 2 != 0)][1],
                bounds[usize::from(corner & 4 != 0)][2],
            );
            let world = local.transform_point3(point) + shift;
            min = min.min(world);
            max = max.max(world);
        }
    }
    !min.is_finite() || !max.is_finite() || cull.intersects(min, max)
}

fn instance_valid(instance: &ModelInstance) -> bool {
    instance.origin.iter().all(|v| v.is_finite())
        && instance.transform.iter().all(|v| v.is_finite())
        && instance.light.block <= 15
        && instance.light.sky <= 15
}

pub fn validate_instances(instances: &[ModelInstance]) -> Result<(), InstanceError> {
    for instance in instances {
        if instance.origin.iter().any(|v| !v.is_finite())
            || instance.transform.iter().any(|v| !v.is_finite())
        {
            return Err(InstanceError::NonFinite);
        }
        if instance.light.block > 15 || instance.light.sky > 15 {
            return Err(InstanceError::InvalidLight);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stages_end_where_the_next_begins_and_never_go_back() {
        use RenderLevelStage::*;
        let mut draw = WorldGeometry::default();
        draw.reset([0.0; 3], Mat4::IDENTITY, [0.5, 0.5]);
        let line = |depth| WorldLine::new([0.0, 0.0, -1.0], [1.0, 0.0, -1.0], !0, 1.0, depth);
        let quad = WorldQuad::new([[0.0, 0.0, -1.0]; 4], !0, WorldDepth::ThroughWalls);
        draw.begin_stage(AfterSky);
        draw.lines(&[line(WorldDepth::Tested)]).unwrap();
        // Nothing at AfterOpaqueBlocks; a quad through walls at AfterTranslucentBlocks.
        draw.begin_stage(AfterTranslucentBlocks);
        draw.quads(&[quad]).unwrap();
        // Still in the current stage: what follows a backwards step is not reordered.
        draw.begin_stage(AfterSky);
        draw.lines(&[line(WorldDepth::ThroughWalls)]).unwrap();
        assert_eq!(
            draw.batch().stage_ends,
            [
                [0, 0, 1, 0],
                [0, 0, 1, 0],
                [0, 0, 1, 0],
                [0, 1, 1, 1],
                [0, 1, 1, 1],
                [0, 1, 1, 1],
            ],
            "the current stage runs to the end until the next begins"
        );
        draw.begin_stage(AfterLevel);
        draw.lines(&[line(WorldDepth::Tested)]).unwrap();
        assert_eq!(
            draw.batch().stage_ends,
            [
                [0, 0, 1, 0],
                [0, 0, 1, 0],
                [0, 0, 1, 0],
                [0, 1, 1, 1],
                [0, 1, 1, 1],
                [0, 1, 1, 1],
            ]
        );
        assert_eq!(draw.batch().lines.len(), 2);
        // A new view starts at the first stage again.
        draw.reset([0.0; 3], Mat4::IDENTITY, [0.5, 0.5]);
        draw.lines(&[line(WorldDepth::Tested)]).unwrap();
        assert_eq!(draw.batch().stage_ends, [[0, 0, 1, 0]; STAGES - 1]);
    }
    #[test]
    fn world_geometry_batches_are_atomic_and_reset_per_view() {
        let mut draw = WorldGeometry::default();
        draw.reset([30_000_000.0, 20.0, 0.0], Mat4::IDENTITY, [0.25, 0.75]);
        let line = WorldLine::new(
            [30_000_000.25, 20.0, -1.0],
            [30_000_001.0, 20.0, -1.0],
            0x80402010,
            2.0,
            WorldDepth::Tested,
        );
        draw.lines(&[line]).unwrap();
        assert_eq!(
            &draw.batch().lines[0][..6],
            &[0.25, 0.0, -1.0, 1.0, 0.0, -1.0]
        );
        assert_eq!(draw.batch().lines[0][6].to_bits(), line.color);
        let mut bad = line;
        bad.width = 0.0;
        assert_eq!(draw.lines(&[line, bad]), Err(DrawError::InvalidWidth));
        bad.width = f32::NAN;
        assert_eq!(draw.lines(&[line, bad]), Err(DrawError::NonFinite));
        bad = line;
        bad.to[0] = f64::MAX;
        assert_eq!(draw.lines(&[line, bad]), Err(DrawError::NonFinite));
        assert_eq!(draw.batch().lines.len(), 1);
        let quad = WorldQuad::new([line.from; 4], line.color, WorldDepth::ThroughWalls);
        draw.quads(&[quad]).unwrap();
        assert_eq!(draw.batch().quads_through.len(), 4);
        assert_eq!(draw.batch().quads_through[0].pos, [0.25, 0.0, -1.0]);
        assert_eq!(
            draw.batch().quads_through[0].color.to_le_bytes(),
            [0x40, 0x20, 0x10, 0x80]
        );
        let mut invalid_quad = quad;
        invalid_quad.corners[3][1] = f64::INFINITY;
        assert_eq!(draw.quads(&[quad, invalid_quad]), Err(DrawError::NonFinite));
        assert_eq!(draw.batch().quads_through.len(), 4);
        let mut through = line;
        through.depth = WorldDepth::ThroughWalls;
        draw.lines(&vec![through; CAPACITY as usize + 1]).unwrap();
        assert_eq!(draw.batch().lines_through.len(), CAPACITY as usize + 1);
        draw.quads(&vec![quad; CAPACITY as usize + 1]).unwrap();
        assert_eq!(
            draw.batch().quads_through.len(),
            (CAPACITY as usize + 2) * 4
        );
        let capacities = (
            draw.lines.each_ref().map(Vec::capacity),
            draw.quads.each_ref().map(Vec::capacity),
        );
        draw.reset([0.0; 3], Mat4::IDENTITY, [0.0; 2]);
        assert!(draw.batch().is_empty());
        assert_eq!(
            capacities,
            (
                draw.lines.each_ref().map(Vec::capacity),
                draw.quads.each_ref().map(Vec::capacity)
            )
        );
    }

    #[test]
    fn pipeline_quads_are_atomic_grow_dynamically_and_run_by_pipeline_and_stage() {
        use RenderLevelStage::*;
        let mut draw = WorldGeometry::default();
        draw.reset([30_000_000.0, 20.0, 0.0], Mat4::IDENTITY, [0.5, 0.5]);
        draw.set_time(12.5);
        let quad = PipelineQuad::new(
            [
                [30_000_000.25, 21.0, -1.0],
                [30_000_000.25, 20.0, -1.0],
                [30_000_001.25, 20.0, -1.0],
                [30_000_001.25, 21.0, -1.0],
            ],
            [[0.0, 0.0], [0.0, 2.0], [3.0, 2.0], [3.0, 0.0]],
            0x80402010,
        );
        draw.begin_stage(AfterSky);
        draw.pipeline_quads(4, &[quad, quad]).unwrap();
        // The same pipeline at the same stage joins the run; another pipeline starts one, and
        // so does the first pipeline again after it, to keep the order submitted.
        draw.pipeline_quads(4, &[quad]).unwrap();
        draw.pipeline_quads(1, &[quad]).unwrap();
        draw.pipeline_quads(4, &[quad]).unwrap();
        draw.begin_stage(AfterWeather);
        draw.pipeline_quads(4, &[quad]).unwrap();
        draw.pipeline_quads(4, &[]).unwrap();

        let pipelines = draw.batch().pipelines;
        let run = |slot, stage, quads| PipelineRun { slot, stage, quads };
        assert_eq!(
            pipelines.runs,
            [run(4, 0, 3), run(1, 0, 1), run(4, 0, 1), run(4, 5, 1)]
        );
        assert_eq!(pipelines.vertices.len(), 24);
        assert_eq!(pipelines.time, 12.5);
        assert!(pipelines.set.is_none());
        // Camera-relative before narrowing to f32, texture coordinates as given, and the
        // colour's bytes red first.
        let corner = pipelines.vertices[2];
        assert_eq!(corner.pos, [1.25, 0.0, -1.0]);
        assert_eq!(corner.uv, [3.0, 2.0]);
        assert_eq!(corner.color.to_le_bytes(), [0x40, 0x20, 0x10, 0x80]);
        assert!(!draw.batch().is_empty());

        // A fault anywhere refuses the whole batch.
        let mut bad_corner = quad;
        bad_corner.corners[3][1] = f64::NAN;
        let mut bad_uv = quad;
        bad_uv.uv[1][0] = f32::INFINITY;
        for bad in [bad_corner, bad_uv] {
            assert_eq!(
                draw.pipeline_quads(4, &[quad, bad]),
                Err(PipelineDrawError::NonFinite)
            );
        }
        assert_eq!(draw.batch().pipelines.vertices.len(), 24);

        // Plain and pipeline quads both grow past the initial hint.
        let plain = WorldQuad::new(quad.corners, !0, WorldDepth::Tested);
        draw.quads(&vec![plain; CAPACITY as usize]).unwrap();
        draw.pipeline_quads(4, &vec![quad; CAPACITY as usize])
            .unwrap();
        assert_eq!(
            draw.batch().pipelines.vertices.len(),
            (6 + CAPACITY as usize) * 4
        );
        let capacity = (
            draw.pipeline_quads.capacity(),
            draw.pipeline_runs.capacity(),
        );
        draw.reset([0.0; 3], Mat4::IDENTITY, [0.0; 2]);
        assert!(draw.batch().is_empty());
        assert_eq!(
            capacity,
            (
                draw.pipeline_quads.capacity(),
                draw.pipeline_runs.capacity()
            ),
            "reset keeps grown capacity for the next view"
        );
    }

    struct NearX;
    impl crate::gfx::Cull for NearX {
        fn intersects(&self, min: glam::Vec3, max: glam::Vec3) -> bool {
            max.x >= 0.0 && min.x <= 4.0
        }
        fn contains(&self, min: glam::Vec3, max: glam::Vec3) -> bool {
            min.x >= 0.0 && max.x <= 4.0
        }
    }

    #[test]
    fn model_batches_cull_first_skip_invalid_visible_and_grow_dynamically() {
        let mut draw = WorldGeometry::default();
        draw.reset([0.0; 3], Mat4::IDENTITY, [0.0; 2]);
        let batch = ModelBatch {
            slot: 3,
            generation: 9,
            bounds: [[0.0; 3], [1.0; 3]],
        };
        let near = ModelInstance::at([1.0, 2.0, 3.0], 0xFF80_C040);
        let far = ModelInstance::at([40.0, 0.0, 0.0], 0xFF80_C040);
        let mut bad_light = near;
        bad_light.light.block = 16;
        let mut bad_origin = near;
        bad_origin.origin[0] = f64::NAN;
        let mut bad_transform = near;
        bad_transform.transform[4] = f32::INFINITY;
        let mut culled_bad_light = far;
        culled_bad_light.light.block = 16;
        // Valid copies survive alongside culled and invalid visible copies; no rollback.
        draw.submit_model(
            batch,
            &[
                near,
                far,
                bad_light,
                bad_origin,
                bad_transform,
                culled_bad_light,
            ],
            Some(&NearX),
            |_| false,
        );
        assert_eq!(draw.copy_count(), 1);
        assert_eq!(draw.copy(0).light, 0xF0F0);
        assert_eq!(draw.copy(0).overlay, 0);
        let mut hurt = near;
        hurt.overlay = mod_api::Overlay::new(8, true).unwrap();
        hurt.light = mod_api::LightCoords::new(15, 3).unwrap();
        draw.submit_model(batch, &[hurt], None::<&NearX>, |_| false);
        assert_eq!(draw.copy_count(), 2);
        assert_eq!(draw.copy(1).light, 0xF030);
        assert_eq!(draw.copy(1).overlay, 1);
        // Growth past the initial 16,384 hint succeeds without error.
        draw.submit_model(
            batch,
            &vec![near; INSTANCE_CAPACITY as usize],
            None::<&NearX>,
            |_| false,
        );
        assert_eq!(draw.copy_count(), INSTANCE_CAPACITY as usize + 2);
        let reserved = draw.copies.capacity();
        draw.reset([0.0; 3], Mat4::IDENTITY, [0.0; 2]);
        assert_eq!(draw.copy_count(), 0);
        assert!(draw.copies.capacity() >= reserved);
    }

    #[test]
    fn directly_staged_models_are_not_baked_and_grow_past_initial_hint() {
        let mut draw = WorldGeometry::default();
        let batch = ModelBatch {
            slot: 7,
            generation: 3,
            bounds: [[0.0; 3], [1.0; 3]],
        };
        let copy = ModelInstance::at([1.0, 2.0, 3.0], u32::MAX);
        let mut staged = 0usize;
        draw.submit_model(
            batch,
            &vec![copy; INSTANCE_CAPACITY as usize + 5],
            None::<&crate::gfx::Frustum>,
            |_| {
                staged += 1;
                true
            },
        );
        assert_eq!(staged, INSTANCE_CAPACITY as usize + 5);
        assert_eq!(draw.copy_count(), 0, "direct copies must not also be baked");
    }

    #[test]
    fn affine_culling_matches_the_eight_corner_box() {
        use std::cell::Cell;
        struct Bounds(Cell<[Vec3; 2]>);
        impl crate::gfx::Cull for Bounds {
            fn intersects(&self, min: Vec3, max: Vec3) -> bool {
                self.0.set([min, max]);
                true
            }
            fn contains(&self, _: Vec3, _: Vec3) -> bool {
                false
            }
        }
        let capture = Bounds(Cell::new([Vec3::ZERO; 2]));
        let bounds = [[-0.75, -2.0, 0.125], [1.5, 0.5, 3.0]];
        let origin = [30_000_000.25, -100.5, -30_000_000.75];
        let camera = [30_000_000.0, 5.25, -30_000_000.0];
        let shift = (DVec3::from(origin) - DVec3::from(camera)).as_vec3();
        let mut seed = 42u32;
        for case in 0..1024 {
            // Include shear, reflections, nonuniform scales, and finite inputs that overflow.
            let scale = if case < 512 { 1.0 } else { f32::MAX * 0.25 };
            let transform = std::array::from_fn(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                ((seed >> 8) as f32 / 2097152.0 - 4.0) * scale
            });
            let matrix = row_major_3x4(transform);
            let mut expected = [Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)];
            for corner in 0..8 {
                let point = Vec3::new(
                    bounds[corner & 1][0],
                    bounds[(corner >> 1) & 1][1],
                    bounds[(corner >> 2) & 1][2],
                );
                let point = matrix.transform_point3(point) + shift;
                expected[0] = expected[0].min(point);
                expected[1] = expected[1].max(point);
            }
            assert!(copy_visible(
                bounds,
                transform,
                origin,
                camera,
                Some(&capture)
            ));
            if expected.iter().all(|point| point.is_finite()) {
                assert_eq!(capture.0.get(), expected);
            }
        }
    }

    #[test]
    fn labels_grow_dynamically_and_reset_per_view() {
        let mut draw = WorldGeometry::default();
        draw.reset([30_000_000.0, 20.0, 0.0], Mat4::IDENTITY, [0.5, 0.5]);
        assert_eq!(draw.check_label([30_000_000.5, 70.0, 0.5]), Ok(()));
        for at in [
            [f64::NAN, 70.0, 0.5],
            [0.5, f64::INFINITY, 0.5],
            [f64::MAX, 70.0, 0.5],
        ] {
            assert_eq!(draw.check_label(at), Err(DrawError::NonFinite));
        }
        let quad = [<Vertex as bytemuck::Zeroable>::zeroed(); 4];
        // One glyph in the world, and its see-through copy on a backdrop: three quads.
        draw.label(1.0, &quad, &[quad, quad].concat());
        assert_eq!(draw.labels().map(<[Vertex]>::len), [4, 8]);
        let world = WorldQuad::new([[30_000_000.0, 20.0, -1.0]; 4], !0, WorldDepth::Tested);
        draw.quads(&vec![world; CAPACITY as usize]).unwrap();
        // Labels grow past the initial 4,096 quad hint alongside plain quads.
        draw.label(1.0, &quad, &quad);
        assert_eq!(draw.labels().map(<[Vertex]>::len), [8, 12]);
        let capacities = draw.labels.each_ref().map(Vec::capacity);
        draw.reset([0.0; 3], Mat4::IDENTITY, [0.0; 2]);
        assert_eq!(draw.labels().map(<[Vertex]>::len), [0, 0]);
        assert_eq!(draw.labels.each_ref().map(Vec::capacity), capacities);
    }

    #[test]
    fn labels_draw_far_to_near_and_equal_distances_keep_their_order() {
        let mut draw = WorldGeometry::default();
        draw.reset([0.0; 3], Mat4::IDENTITY, [0.5, 0.5]);
        // Each label's quad is told apart by its colour; two quads for the second.
        let quad = |color| {
            [Vertex {
                color,
                ..<Vertex as bytemuck::Zeroable>::zeroed()
            }; 4]
        };
        let colors = |draw: &WorldGeometry| -> Vec<u32> {
            draw.labels()[0]
                .iter()
                .step_by(4)
                .map(|v| v.color)
                .collect()
        };
        draw.label(4.0, &quad(1), &[]);
        draw.label(16.0, &[quad(2), quad(2)].concat(), &quad(9));
        draw.label(4.0, &quad(3), &[]);
        draw.label(1.0, &quad(4), &[]);
        draw.sort_labels();
        assert_eq!(colors(&draw), [2, 2, 1, 3, 4]);
        assert_eq!(
            draw.labels()[1].len(),
            4,
            "see-through copies are not reordered"
        );
        // Sorted runs stay sorted when labels are added and sorted again.
        draw.label(9.0, &quad(5), &[]);
        draw.sort_labels();
        assert_eq!(colors(&draw), [2, 2, 5, 1, 3, 4]);
        assert_eq!(draw.labels()[0].len() + draw.labels()[1].len(), 28);
        draw.reset([0.0; 3], Mat4::IDENTITY, [0.5, 0.5]);
        draw.sort_labels();
        assert!(draw.labels()[0].is_empty());
    }
}
