//! The models a client mod draws as part of a block (`Chunk::extra_models`,
//! `ClientRenderClientHost::set_block_extra_models`): drawn after the section's own geometry and
//! held to the section's quad and byte budget (`mod_geometry::MAX_QUADS`, `MAX_SECTION_BYTES`),
//! so no list of models can grow a section's mesh past what its upload holds.
//!
//! A mod's own tint source (`BlockTintSource`) colouring an extra model is asked on the thread
//! that schedules the mesh ([`MeshTables::prepare_attached_tints`]), a failure contained there:
//! the workers read the prepared colours and call no mod code for an extra model.
use super::mod_geometry::{Budget, QUAD_BYTES};
use super::{
    CellFaces, GroupCells, MAX_OVERLAYS, MeshTables, Mesher, ModLayer, Padded, SectionMesh,
    TerrainSource, Tint, Tints, mod_tint,
};
use crate::{biome::Biomes, chunk::Chunk, fast_hash::FxHashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use sunrise_core::biome::BiomeZoom;

/// The colours mods' own tint sources gave the blocks with extra models in one section, by
/// section-local position, state and tint index, asked before the section's mesh was scheduled
/// ([`MeshTables::prepare_attached_tints`]). An extra model's face reads its colour here; one
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

impl MeshTables {
    /// The colours a mod's own tint source gives the extra models of section `sy` of
    /// `region`'s centre column (`Chunk::extra_models`): each source layer of each such
    /// block's state, its `colorInWorld` asked here, on the thread that schedules the mesh, as
    /// the section mesher asks it for the block's own faces. A source that panics colours
    /// white, logged once, and the client goes on. `None` when no such block has one.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_attached_tints(
        &self,
        region: &[[Option<&Chunk>; 3]; 3],
        sy: i32,
        biomes: &Biomes,
        zoom: BiomeZoom,
        blend_radius: i32,
        nether_shading: bool,
    ) -> Option<Arc<AttachedTints>> {
        let center = region[1][1]?;
        if center.extra_models.is_empty() || self.mod_tints.layers.is_empty() {
            return None;
        }
        let base_y = center.min_y() + sy * 16;
        let mut tints = None;
        let mut colors = FxHashMap::default();
        for &cell in center.extra_models.keys() {
            if (cell >> 12) as i32 != sy {
                continue;
            }
            let (x, y, z) = center.dynamic_cell_coords(cell);
            let (x, z) = (i32::from(x), i32::from(z));
            let id = center.get(x, y, z);
            let Some(info) = self.info.get(id as usize) else {
                continue;
            };
            let Tint::Mod(slot) = info.tints[0] else {
                continue;
            };
            let pos = [x, y - base_y, z];
            let layers = &self.mod_tints.layers[slot as usize];
            for (index, layer) in layers.iter().enumerate().take(usize::from(info.tint_count)) {
                let ModLayer::Source(source) = layer else {
                    continue;
                };
                let index = index as i32;
                let tints = tints.get_or_insert_with(|| {
                    Tints::new(
                        region,
                        biomes,
                        zoom,
                        blend_radius,
                        [center.x, center.z, base_y],
                    )
                    .with_colormaps(self)
                });
                let asked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    mod_tint::ask(
                        self,
                        &**source,
                        (id, index),
                        tints,
                        pos,
                        nether_shading,
                        false,
                    )
                }));
                let color = asked.unwrap_or_else(|_| {
                    if !SOURCE_FAILED.swap(true, Ordering::Relaxed) {
                        log::warn!(
                            "Sunrise: a block tint source panicked colouring an extra model;                              using white"
                        );
                    }
                    tints.asked = None;
                    tints.asked_colors.clear();
                    0xFF_FFFF
                });
                colors.insert((pos, id, index), color);
            }
        }
        (!colors.is_empty()).then(|| Arc::new(AttachedTints(colors)))
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
/// that could pass it, it and every extra model after it are dropped. The block geometry is
/// always kept, and the same section and lists always keep the same models. A face a mod's own
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
    attached: Option<Arc<AttachedTints>>,
) {
    tints.attached = Some(attached);
    draw(mesher, pad, groups, tints, out, cells, visibility_bytes);
    tints.attached = None;
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
                mesher.model_face(pad, groups, tints, out, at, face);
            }
        }
        if mesher.shader_pack {
            let source = TerrainSource::new(at.id, at.pos, at.state.emission, false);
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
