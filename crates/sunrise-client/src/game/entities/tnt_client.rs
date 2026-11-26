use super::Game;
use crate::game::tnt_common::PrimedTnt;
use sunrise_client::mesher::Vertex;
use sunrise_core::position::BlockPos;

impl Game<'_, crate::game::client_side::ClientState> {
    /// `TntRenderer`: swelling over the last 10 ticks and flashing white every 5 ticks.
    pub(in crate::game) fn tnt_geometry(
        &self,
        camera: glam::DVec3,
        alpha: f64,
        out: &mut Vec<Vertex>,
        run: std::ops::Range<usize>,
        sprites: Option<[Option<sunrise_client_render::hud::Rect>; 6]>,
    ) {
        let Some(sprites) = sprites else {
            return;
        };
        let range = super::entity_culling::default_range(
            glam::DVec3::splat(PrimedTnt::SIZE),
            self.view_scale(),
        );
        for t in &self.tnts[run] {
            // LevelExtractor: the camera entity is not drawn from its own eyes.
            if self.hides_as_camera(t.id) {
                continue;
            }
            // EntityRenderer.shouldRender.
            if !self.standing_entity_in_view(
                camera,
                super::entity_culling::at(t.previous),
                super::entity_culling::at(t.position),
                alpha,
                PrimedTnt::SIZE,
                PrimedTnt::SIZE,
                range,
                0.0,
            ) {
                continue;
            }
            self.mark_motion(out.len(), crate::game::MOTION_OBJECT | u64::from(t.id));
            let fuse = t.fuse as f32 - alpha as f32 + 1.0;
            if let Some(resolved) = super::native_entity_renderers::tnt_renderer() {
                let partial = alpha as f32;
                let at = glam::DVec3::new(
                    t.previous.x + (t.position.x - t.previous.x) * alpha,
                    t.previous.y + (t.position.y - t.previous.y) * alpha,
                    t.previous.z + (t.position.z - t.previous.z) * alpha,
                );
                let entity: mod_api::EntityRenderState =
                    mod_api::__private::built::EntityRenderState {
                        id: t.id,
                        partial,
                        age: t.ticks as f32 + partial,
                        yaw: t.rotation[0],
                        pitch: t.rotation[1],
                        living: None,
                        tnt_fuse: Some(fuse),
                        orb_value: None,
                        painting: None,
                        item_frame: None,
                        attachments: None,
                    }
                    .into();
                let light = self.world.light(
                    t.position.x.floor() as i32,
                    (t.position.y + 0.5).floor() as i32,
                    t.position.z.floor() as i32,
                );
                let packed = (u32::from(light >> 4) * 16) | ((u32::from(light & 15) * 16) << 8);
                let block = [
                    at.x.floor() as i32,
                    at.y.floor() as i32,
                    at.z.floor() as i32,
                ];
                if self.mod_entity_renderer_geometry(
                    &resolved,
                    &entity,
                    "minecraft:tnt",
                    at - camera,
                    Some(at),
                    block,
                    packed,
                    out,
                ) {
                    continue;
                }
            }
            let scale = if fuse < 10.0 {
                let g = (1.0 - fuse / 10.0).clamp(0.0, 1.0);
                1.0 + g * g * g * g * 0.3
            } else {
                1.0
            };
            let lit = fuse >= 0.0 && (fuse / 5.0) as i32 % 2 == 0;
            let base = glam::DVec3::new(
                t.previous.x + (t.position.x - t.previous.x) * alpha,
                t.previous.y + (t.position.y - t.previous.y) * alpha + 0.5,
                t.previous.z + (t.position.z - t.previous.z) * alpha,
            ) - camera;
            let o = base.as_vec3();
            let light = self.world.light(
                t.position.x.floor() as i32,
                (t.position.y + 0.5).floor() as i32,
                t.position.z.floor() as i32,
            );
            let packed = (u32::from(light >> 4) * 16) | ((u32::from(light & 15) * 16) << 8);
            let v = |x: f32, y: f32, z: f32| o + glam::Vec3::new(x - 0.5, y - 0.5, z - 0.5) * scale;
            let faces = [
                (
                    [v(0., 1., 1.), v(1., 1., 1.), v(1., 1., 0.), v(0., 1., 0.)],
                    1.0,
                ),
                (
                    [v(0., 0., 0.), v(1., 0., 0.), v(1., 0., 1.), v(0., 0., 1.)],
                    0.5,
                ),
                (
                    [v(1., 0., 0.), v(0., 0., 0.), v(0., 1., 0.), v(1., 1., 0.)],
                    0.8,
                ),
                (
                    [v(0., 0., 1.), v(1., 0., 1.), v(1., 1., 1.), v(0., 1., 1.)],
                    0.8,
                ),
                (
                    [v(0., 0., 0.), v(0., 0., 1.), v(0., 1., 1.), v(0., 1., 0.)],
                    0.6,
                ),
                (
                    [v(1., 0., 1.), v(1., 0., 0.), v(1., 1., 0.), v(1., 1., 1.)],
                    0.6,
                ),
            ];
            for ((corners, shade), sprite) in faces.into_iter().zip(sprites) {
                let Some(r) = sprite else { continue };
                let c = (255.0 * shade) as u32;
                let color = 0xFF00_0000 | c << 16 | c << 8 | c;
                for (p, uv) in
                    corners
                        .iter()
                        .zip([[r.u0, r.v1], [r.u1, r.v1], [r.u1, r.v0], [r.u0, r.v0]])
                {
                    // Overlay 5: OverlayTexture white flash.
                    out.push(Vertex {
                        normal: [0; 4],
                        pos: p.to_array(),
                        uv,
                        layer: if lit { 5 } else { 0 },
                        color,
                        light: packed,

                        glint_uv: [0.0, 0.0],
                    });
                }
            }
        }
    }
    /// The sprites of the TNT block's six faces (up, down, north, south, west, east).
    pub(in crate::game) fn tnt_face_sprites(
        &self,
    ) -> Option<[Option<sunrise_client_render::hud::Rect>; 6]> {
        let state = self.pack.assets.registry.default_state("minecraft:tnt")?;
        let Ok(models) = self
            .side
            .client_pack
            .render
            .model_catalog
            .collect(state, BlockPos::new(0, 0, 0))
        else {
            return None;
        };
        let mut sprites = [None; 6];
        for face in models.iter().flat_map(|(_, m)| m.faces.iter()) {
            use sunrise_assets::model::Direction as D;
            let index = match face.quad.facing {
                D::Up => 0,
                D::Down => 1,
                D::North => 2,
                D::South => 3,
                D::West => 4,
                D::East => 5,
            };
            if sprites[index].is_none() {
                sprites[index] = self
                    .side
                    .client_pack
                    .layer_names
                    .get(&face.layer)
                    .and_then(|name| self.side.gui.atlas.sprites.get(name).copied());
            }
        }
        Some(sprites)
    }
}
