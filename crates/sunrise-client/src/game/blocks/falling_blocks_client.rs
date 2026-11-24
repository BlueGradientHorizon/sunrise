#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::falling_blocks::*;

impl Game<'_> {
    /// Cache only model-dependent work, never an entity's seed, light or pose.
    pub(in crate::game) fn falling_model_vertices(
        &self,
        state: BlockStateId,
        start_pos: BlockPos,
        origin: glam::Vec3,
        light: u32,
        out: &mut Vec<Vertex>,
    ) {
        // `FallingBlockRenderer`: the model is picked by the block's starting position.
        let seed = sunrise_core::random::position_seed(start_pos);
        let Ok(models) = self.pack.render.model_catalog.collect_seeded(state, seed) else {
            return;
        };
        // Baked models live at stable addresses until the pack is replaced.
        // Keep multipart order and duplicates: both affect the emitted vertices.
        let mut cache = self.client.falling_vertices.borrow_mut();
        let entries = cache.entry(state).or_default();
        let index = entries.iter().position(|(ptrs, _)| {
            ptrs.iter()
                .copied()
                .eq(models.iter().map(|(_, m)| *m as *const _ as usize))
        });
        let index = index.unwrap_or_else(|| {
            let mut vertices = Vec::new();
            self.block_model_vertices(state, seed, glam::Mat4::IDENTITY, 0, false, &mut vertices);
            // Preserve the original coordinates, including signed zero, rather
            // than composing an identity transform with the frame's translation.
            let positions = models.iter().flat_map(|(_, model)| {
                model
                    .faces
                    .iter()
                    .filter(|face| {
                        self.pack
                            .layer_names
                            .get(&face.layer)
                            .is_some_and(|name| self.gui.atlas.sprites.contains_key(name))
                    })
                    .flat_map(|face| face.quad.positions)
            });
            for (vertex, pos) in vertices.iter_mut().zip(positions) {
                vertex.pos = pos;
            }
            let ptrs = models
                .iter()
                .map(|(_, m)| *m as *const _ as usize)
                .collect();
            entries.push((ptrs, vertices));
            entries.len() - 1
        });
        let start = out.len();
        out.extend_from_slice(&entries[index].1);
        let matrix = glam::Mat4::from_translation(origin);
        for vertex in &mut out[start..] {
            vertex.pos = matrix
                .transform_point3(glam::Vec3::from_array(vertex.pos))
                .to_array();
            vertex.light = light;
        }
    }
}
