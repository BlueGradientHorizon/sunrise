//! The models a client mod draws as part of a block (`Chunk::extra_models`,
//! `ClientRenderClientHost::set_block_extra_models`): drawn after the section's own geometry and
//! held to the section's quad and byte budget (`mod_geometry::MAX_QUADS`, `MAX_SECTION_BYTES`),
//! so no list of models can grow a section's mesh past what its upload holds.
//!
//! A mod's own tint source (`BlockTintSource`) colouring an extra model is asked on the thread
//! that schedules the mesh ([`Mesher::prepare_attached_tints`]), a failure contained there:
//! the workers read the prepared colours and call no mod code for an extra model.
use super::mod_geometry::{Budget, QUAD_BYTES};
use super::{
    CellFaces, FaceCell, GroupCells, MAX_OVERLAYS, Mesher, ModLayer, Padded, Pick, SectionMesh,
    SectionVisibilityPlan, TerrainSource, Tint, Tints, mod_tint, pi, row_masks,
    visibility_plan::{CellMasks, MODEL, NoMasks},
};
use crate::{
    chunk::Chunk,
    fast_hash::{FxHashMap, FxHashSet},
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use sunrise_core::position::BlockPos;

/// The colours mods' own tint sources gave the blocks with extra models in one section, by
/// section-local position, state and tint index, asked before the section's mesh was scheduled
/// ([`Mesher::prepare_attached_tints`]). An extra model's face reads its colour here; one
/// not prepared is white.
#[derive(Debug, Default, PartialEq)]
pub struct AttachedTints(FxHashMap<([i32; 3], u32, i32), u32>);

impl AttachedTints {
    /// The prepared colour of tint `index` of state `id` at section-local `pos`.
    pub(super) fn get(&self, pos: [i32; 3], id: u32, index: i32) -> Option<u32> {
        self.0.get(&(pos, id, index)).copied()
    }
}

/// Whether a tint source has panicked while asked for an extra model: logged once.
static SOURCE_FAILED: AtomicBool = AtomicBool::new(false);

impl Mesher<'_> {
    /// Ask only the mod tint sources used by a section's attachment faces and overlays, before
    /// sending the mesh to a worker. A failed source supplies white to both passes.
    pub(crate) fn prepare_attached_tints(
        &mut self,
        region: &[[Option<&Chunk>; 3]; 3],
        sy: i32,
        visibility_job: bool,
        plan: Option<&SectionVisibilityPlan>,
    ) -> Option<Arc<AttachedTints>> {
        let center = region[1][1]?;
        let mut cells: Vec<_> = center
            .extra_models
            .iter()
            .filter(|(cell, _)| (**cell >> 12) as i32 == sy)
            .collect();
        if cells.is_empty() || self.tables.mod_tints.layers.is_empty() {
            return None;
        }
        cells.sort_unstable_by_key(|(cell, _)| **cell);
        let colors = match plan {
            Some(plan) if plan.hides_terrain() => return None,
            Some(plan) => self.collect_attached_tints::<_, true>(region, center, sy, &cells, plan),
            None if visibility_job => {
                self.collect_attached_tints::<_, true>(region, center, sy, &cells, &NoMasks)
            }
            None => self.collect_attached_tints::<_, false>(region, center, sy, &cells, &NoMasks),
        };
        (!colors.0.is_empty()).then(|| Arc::new(colors))
    }

    fn collect_attached_tints<M: CellMasks, const MISSING_AS_AIR: bool>(
        &mut self,
        region: &[[Option<&Chunk>; 3]; 3],
        center: &Chunk,
        sy: i32,
        cells: &[(&u32, &Arc<[mod_api::ModelRef]>)],
        masks: &M,
    ) -> AttachedTints {
        let base_y = center.min_y() + sy * 16;
        let mut pad = Padded::default();
        pad.fill::<MISSING_AS_AIR>(region, sy, &mut Box::new([0; 4096]));
        let mut rows = Vec::new();
        self.prepare_rows(&mut pad, &mut rows, masks);
        let (_, hidden) = row_masks(&rows);
        let mut groups = None;
        let groups = self.group_cells(region, base_y, &mut groups);
        let mut tints = Tints::new(
            region,
            self.biomes,
            self.zoom,
            self.blend_radius,
            [center.x, center.z, base_y],
        )
        .with_colormaps(self.tables);
        let mut colors = FxHashMap::default();
        let mut seen = FxHashSet::default();
        for &(&key, models) in cells {
            let (x, wy, z) = center.dynamic_cell_coords(key);
            let pos @ [x, y, z] = [i32::from(x), wy - base_y, i32::from(z)];
            let cell = pi(x, y, z);
            let id = pad.blocks[cell];
            let state = &self.tables.info[id as usize];
            if state.air
                || hidden[(y * 16 + z) as usize] >> x & 1 != 0
                || masks.at(cell) & MODEL == 0
            {
                continue;
            }
            let at = FaceCell {
                pos,
                cell,
                id,
                state,
                world: BlockPos::new(center.x * 16 + x, wy, center.z * 16 + z),
                visible: self.visible_faces(region, &pad, base_y, pos, masks),
            };
            seen.clear();
            for &model in models.iter() {
                if !seen.insert(model) {
                    continue;
                }
                let Ok(model) = self.render.resolve_model(model) else {
                    continue;
                };
                for face in &model.faces {
                    if !self.draws_model_face(&pad, &at, face) {
                        continue;
                    }
                    let (pick, overlays) = self.face_tiles(&pad, groups, &tints, &at, face);
                    let face_tint =
                        (!matches!(pick, Pick::Nothing)).then_some((face.quad.tint_index, None));
                    for (index, block) in face_tint.into_iter().chain(
                        overlays
                            .into_iter()
                            .flatten()
                            .filter_map(|overlay| overlay.tint),
                    ) {
                        let (id, state) = self.tables.overlay_tint_state((id, at.state), block);
                        let Tint::Mod(slot) = state.tints[0] else {
                            continue;
                        };
                        if index < 0 || index >= i32::from(state.tint_count) {
                            continue;
                        }
                        let ModLayer::Source(source) =
                            &self.tables.mod_tints.layers[slot as usize][index as usize]
                        else {
                            continue;
                        };
                        if state
                            .colormap
                            .is_some_and(|map| tints.colormap(map, x, y, z).is_some())
                        {
                            continue;
                        }
                        colors.entry((pos, id, index)).or_insert_with(|| {
                            crate::crash::contain(|| {
                                mod_tint::ask(self.tables, &**source, (id, index), &mut tints, pos,
                                    self.nether_shading, false)
                            }).unwrap_or_else(|_| {
                                if !SOURCE_FAILED.swap(true, Ordering::Relaxed) {
                                    log::warn!("Sunrise: a block tint source panicked colouring an extra model; using white");
                                }
                                0xFF_FFFF
                            })
                        });
                    }
                }
            }
        }
        AttachedTints(colors)
    }
}

/// The most quads one model face becomes: its `ctm_compact` pieces (up to 3 by 3), and as many
/// again for each connected-texture overlay drawn over it.
pub(super) const FACE_QUADS: usize = 9 * (1 + MAX_OVERLAYS);
/// The most mesh bytes one quad takes: [`QUAD_BYTES`] and its shader-pack source.
pub(super) const QUAD_MAX_BYTES: usize = QUAD_BYTES + size_of::<TerrainSource>();

/// Draws `cells`' extra models into `out`, after the block geometry of the whole section, in
/// cell order and then list order. A face is drawn only while the section's budget still holds
/// [`FACE_QUADS`] more quads of [`QUAD_MAX_BYTES`], whatever it expands to; at the first face
/// that cannot fit, drawing stops, so the last model may be partially drawn. The block geometry
/// is always kept. A face a mod's own
/// tint source colours takes its colour from `attached`, prepared before the mesh was scheduled:
/// no mod code runs here.
#[allow(clippy::too_many_arguments)]
pub(super) fn add(
    mesher: &Mesher<'_>,
    pad: &Padded,
    groups: Option<&GroupCells>,
    tints: &mut Tints<'_>,
    out: &mut SectionMesh,
    cells: &[(CellFaces<'_>, &Arc<[mod_api::ModelRef]>)],
    visibility_bytes: usize,
) {
    tints.drawing_extras = true;
    draw(mesher, pad, groups, tints, out, cells, visibility_bytes);
    tints.drawing_extras = false;
}

fn draw(
    mesher: &Mesher<'_>,
    pad: &Padded,
    groups: Option<&GroupCells>,
    tints: &mut Tints<'_>,
    out: &mut SectionMesh,
    cells: &[(CellFaces<'_>, &Arc<[mod_api::ModelRef]>)],
    visibility_bytes: usize,
) {
    for (at, models) in cells {
        let start = [out.solid.len(), out.translucent.len()];
        let mut full = false;
        'models: for &model in models.iter() {
            // A handle a resource reload made stale draws nothing.
            let Ok(model) = mesher.render.resolve_model(model) else {
                continue;
            };
            for face in &model.faces {
                if !fits(out, start, mesher.shader_pack, visibility_bytes) {
                    full = true;
                    break 'models;
                }
                mesher.model_face(pad, groups, tints, out, at, face, false);
            }
        }
        if mesher.shader_pack {
            let source =
                TerrainSource::new(at.sample.id, at.sample.pos, at.sample.state.emission, false);
            out.record_source(start, source);
        }
        if full {
            return;
        }
    }
}

/// Whether the section `out` has room for one more face at its largest. With a shader pack, the
/// quads drawn since `start` take their sources once the cell is done: counted now.
fn fits(out: &SectionMesh, start: [usize; 2], shader_pack: bool, visibility_bytes: usize) -> bool {
    let mut left = Budget::left_after(out, visibility_bytes);
    if shader_pack {
        let pending = (out.solid.len() - start[0] + out.translucent.len() - start[1]) / 4;
        left.bytes = left
            .bytes
            .saturating_sub(pending * size_of::<TerrainSource>());
    }
    left.quads >= FACE_QUADS && left.bytes >= FACE_QUADS * QUAD_MAX_BYTES
}
