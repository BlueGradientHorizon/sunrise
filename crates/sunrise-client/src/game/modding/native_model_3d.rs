//! An item or a standalone model drawn in a screen in 3D under a caller's turn
//! (`DrawContext::model_3d`): what a symmetry wand's screen shows its wand and its mirror with.
//! The item goes through the slot's own drawing, under a pose with the turn in it; the model
//! is projected as a scene's blocks are, with a depth of its own.
use super::Game;
use super::native_scene::{face_shade, shaded};
use glam::{Mat4, Vec3};
use mod_api::{Gui3dError, Gui3dSubject, Gui3dView, ModelError};
use sunrise_client::{hud::HudBatch, world_geometry::row_major_3x4};

/// The pose of a subject drawn as `view` says: its centre at `view.at`, a block `view.size`
/// pixels across, the caller's `transform` applied in the frame `x` right, `y` up, `z` towards
/// the viewer. With the identity at size 16 it is a slot's pose (`translate(x + 8, y + 8)`,
/// `scale(16, -16, 16)`).
fn view_pose(view: Gui3dView) -> Mat4 {
    Mat4::from_translation(Vec3::new(view.at[0], view.at[1], 0.0))
        * Mat4::from_scale(Vec3::new(view.size, -view.size, view.size))
        * row_major_3x4(view.transform)
}

impl Game<'_> {
    /// Draws `subject` as `view` says into `batch`. See `DrawContext::model_3d`.
    pub(super) fn native_model_3d(
        &self,
        batch: &mut HudBatch,
        subject: Gui3dSubject,
        view: Gui3dView,
    ) -> Result<(), Gui3dError> {
        if !self.native_window() {
            return Err(Gui3dError::Unavailable);
        }
        if !view.is_drawable() {
            return Err(Gui3dError::NotFinite);
        }
        let pose = view_pose(view);
        if !pose.is_finite() {
            return Err(Gui3dError::NotFinite);
        }
        let start = batch.vertices.len();
        match subject {
            Gui3dSubject::Item(stack) => {
                let stack = crate::game::api_values::stack_value(&stack)
                    .map_err(|_| Gui3dError::UnknownItem)?;
                // GuiGraphicsExtractor.item draws nothing for an empty stack.
                if &*stack.item == "minecraft:air"
                    || stack.count == 0
                    || self.res.items().get(&stack.item).is_none()
                {
                    return Err(Gui3dError::UnknownItem);
                }
                self.draw_item_model_posed(batch, &stack, pose);
            }
            Gui3dSubject::Model(model) => self.draw_model_posed(batch, model, pose)?,
            _ => unreachable!("Gui3dSubject: unhandled variant"),
        }
        // A turn can overflow a corner: draw nothing rather than something broken.
        if !batch.vertices[start..]
            .iter()
            .all(|v| v.pos.iter().all(|c| c.is_finite()) && v.depth.is_finite())
        {
            batch.vertices.truncate(start);
            return Err(Gui3dError::NotFinite);
        }
        Ok(())
    }

    /// A standalone model's faces under `pose` about the centre of its 0 to 1 cube, full bright
    /// and shaded by face, back faces dropped, far to near, with a depth of its own.
    fn draw_model_posed(
        &self,
        batch: &mut HudBatch,
        model: mod_api::ModelRef,
        pose: Mat4,
    ) -> Result<(), Gui3dError> {
        use mod_api::__private::game::Fields;
        let fields = model.fields();
        if fields.generation != self.standalone_geometry.generation() {
            return Err(Gui3dError::Model(ModelError::StaleModel));
        }
        let model = self
            .standalone_geometry
            .instance_slot(fields.slot)
            .and_then(|_| self.pack.render.model_catalog.model_by_slot(fields.slot))
            .ok_or(Gui3dError::Model(ModelError::UnknownModel))?;
        let m = pose * Mat4::from_translation(Vec3::splat(-0.5));
        let mut seen: Vec<(f32, [[f32; 3]; 4], [[f32; 2]; 4], u32)> = Vec::new();
        for face in &model.faces {
            let Some(sprite) = self
                .pack
                .layer_names
                .get(&face.layer)
                .and_then(|name| self.gui.atlas.sprites.get(name))
            else {
                continue;
            };
            let [nx, ny, nz] =
                sunrise_core::geometry::Direction::ALL[face.quad.facing as usize].normal();
            let normal = Vec3::new(nx as f32, ny as f32, nz as f32);
            if m.transform_vector3(normal).z <= 0.0 {
                continue;
            }
            let corners = face
                .quad
                .positions
                .map(|p| m.transform_point3(Vec3::from_array(p)));
            seen.push((
                corners.iter().map(|c| c.z).sum::<f32>() / 4.0,
                corners.map(|c| c.to_array()),
                face.quad.uv.map(|uv| {
                    [
                        sprite.u0 + uv[0] * (sprite.u1 - sprite.u0),
                        sprite.v0 + uv[1] * (sprite.v1 - sprite.v0),
                    ]
                }),
                shaded(0xFFFF_FFFF, face_shade(normal)),
            ));
        }
        if seen.iter().any(|(depth, corners, ..)| {
            !depth.is_finite() || corners.iter().flatten().any(|v| !v.is_finite())
        }) {
            return Err(Gui3dError::NotFinite);
        }
        // Far to near, for the faces that let others show through.
        seen.sort_by(|a, b| a.0.total_cmp(&b.0));
        let start = batch.vertices.len();
        let (mut lo, mut hi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
        for (_, corners, uv, color) in seen {
            for c in corners {
                for axis in 0..2 {
                    lo[axis] = lo[axis].min(c[axis]);
                    hi[axis] = hi[axis].max(c[axis]);
                }
            }
            batch.quad_3d(corners, uv, color);
        }
        if batch.vertices.len() > start {
            batch.depth_since(start, [lo[0], lo[1], hi[0], hi[1]]);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::IDENTITY_TRANSFORM;

    #[test]
    fn the_identity_at_size_16_is_the_slots_pose() {
        let pose = view_pose(Gui3dView::new([40.0, 30.0], 16.0, IDENTITY_TRANSFORM));
        let slot = Mat4::from_translation(Vec3::new(40.0, 30.0, 0.0))
            * Mat4::from_scale(Vec3::new(16.0, -16.0, 16.0));
        assert_eq!(pose, slot);
    }

    #[test]
    fn a_quarter_turn_about_z_moves_the_right_edge_to_the_top() {
        // Row-major 3x4: x' = -y, y' = x, z' = z (counterclockwise seen from the viewer).
        let turn = [0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let pose = view_pose(Gui3dView::new([100.0, 100.0], 10.0, turn));
        // The model's +x point lands above the centre on the screen (y down).
        let right = pose.transform_point3(Vec3::X);
        assert!(
            (right - Vec3::new(100.0, 90.0, 0.0)).length() < 1e-4,
            "{right}"
        );
    }
}
