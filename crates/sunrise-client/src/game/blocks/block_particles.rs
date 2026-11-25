//! Block particles: the sprite and tint of a block's faces and particle material, honey slides,
//! and the burst a destroyed block leaves (`ClientLevel.addDestroyBlockEffect`).
use super::*;
use sunrise_client::blocks::RenderShape;
use sunrise_core::geometry::Direction;

impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn particle_shape(
        &self,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> Vec<[f64; 6]> {
        self.state_shape_boxes(
            &self.world,
            &self.be_anims,
            state,
            pos,
            mod_api::BlockShapeKind::Outline,
        )
    }

    /// `ClientLevel.addBreakingBlockEffects`' particle step, called after the sound with a
    /// non-air state and the hit face. Only visible blocks have hit particles.
    pub(in crate::game) fn crack_particle(
        &mut self,
        pos: [i32; 3],
        face: [i32; 3],
        state: BlockStateId,
    ) {
        let facts = self.res.facts().get(state);
        if self.settings.extras.hide_block_particles || facts.render_shape == RenderShape::Invisible
        {
            return;
        }
        let Some(face) = Direction::ALL.into_iter().find(|d| d.normal() == face) else {
            return;
        };
        let spawn = facts.spawn_terrain_particles;
        if !self.mod_add_hit_effects(state, pos, face).unwrap_or(!spawn) {
            self.add_breaking_particle(pos, state, face);
        }
    }
}
