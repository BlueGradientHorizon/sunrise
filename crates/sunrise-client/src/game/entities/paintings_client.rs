#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Game, Hit, entity_culling::default_range};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::paintings::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    entities::Painting, items::ItemStack, mesher::Vertex, random_source,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Vec3},
    random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::painting_variants::{
    PaintingVariants, Variant, frame_coexists, painting_coexists, painting_facing,
};

impl Game<'_> {
    /// PaintingRenderer: variant on the front, back texture behind and on the edges.
    pub(in crate::game) fn painting_geometry(
        &self,
        camera: glam::DVec3,
        alpha: f64,
        out: &mut Vec<Vertex>,
    ) {
        let sprites = &self.gui.atlas.sprites;
        let partial = alpha as f32;
        for p in &self.paintings {
            if self.hides_as_camera(p.id) {
                continue;
            }
            // EntityRenderer.shouldRender: default range measured from the box centre.
            let (lo, hi) = p.bounds();
            let (lo, hi) = (glam::DVec3::from(lo), glam::DVec3::from(hi));
            let range = default_range(hi - lo, self.view_scale());
            if !self.entity_in_view(camera, (lo + hi) / 2.0, range, lo, hi) {
                continue;
            }
            let culling = self.entity_cull_decision(p, 1.0);
            if culling.hides_entity() {
                continue;
            }
            if let Some(resolved) = super::native_entity_renderers::painting_renderer() {
                let entity_start = out.len();
                self.mark_motion(entity_start, crate::game::MOTION_OBJECT | u64::from(p.id));
                let (lo, hi) = p.bounds();
                let at = glam::DVec3::new(
                    (lo[0] + hi[0]) / 2.0,
                    (lo[1] + hi[1]) / 2.0,
                    (lo[2] + hi[2]) / 2.0,
                );
                let facing = match p.facing & 3 {
                    0 => mod_api::Direction::South,
                    1 => mod_api::Direction::West,
                    2 => mod_api::Direction::North,
                    _ => mod_api::Direction::East,
                };
                let entity: mod_api::EntityRenderState =
                    mod_api::__private::built::EntityRenderState {
                        id: p.id,
                        partial,
                        age: p.ticks as f32 + partial,
                        yaw: f32::from(p.facing & 3) * 90.0,
                        pitch: 0.0,
                        living: None,
                        tnt_fuse: None,
                        orb_value: None,
                        painting: Some(mod_api::__private::built::PaintingRenderState {
                            facing,
                            width: p.width,
                            height: p.height,
                        }),
                        item_frame: None,
                        attachments: None,
                    }
                    .into();
                let light = self.world.light(p.pos[0], p.pos[1], p.pos[2]);
                let packed = (u32::from(light >> 4) * 16) | ((u32::from(light & 15) * 16) << 8);
                if self.mod_entity_renderer_geometry(
                    &resolved,
                    &entity,
                    "minecraft:painting",
                    at - camera,
                    Some(at),
                    p.pos,
                    packed,
                    out,
                ) {
                    self.finish_entity_occlusion(culling, p, out, entity_start);
                    continue;
                }
            }
            let Some(back) = sprites.get("painting/back").copied() else {
                continue;
            };
            let asset = match p.asset.as_deref() {
                Some(asset) => asset,
                None => match self.painting_variants.get(&p.variant) {
                    Some(v) => v.asset.as_str(),
                    None => continue,
                },
            };
            let Some(front) = sprites.get(&format!("painting/{asset}")).copied() else {
                continue;
            };
            let (lo, hi) = p.bounds();
            let c = glam::DVec3::new(
                (lo[0] + hi[0]) / 2.0,
                (lo[1] + hi[1]) / 2.0,
                (lo[2] + hi[2]) / 2.0,
            ) - camera;
            let s = p.step().map(f64::from);
            let l = p.left().map(f64::from);
            let out_dir = glam::DVec3::new(s[0], 0.0, s[2]);
            // The viewer's right is the painting's left.
            let right = glam::DVec3::new(l[0], 0.0, l[2]);
            let up = glam::DVec3::Y;
            let (hw, hh, d) = (
                f64::from(p.width) / 2.0,
                f64::from(p.height) / 2.0,
                1.0 / 32.0,
            );
            let entity_start = out.len();
            self.mark_motion(entity_start, crate::game::MOTION_OBJECT | u64::from(p.id));
            let light = self.world.light(p.pos[0], p.pos[1], p.pos[2]);
            let packed = (u32::from(light >> 4) * 16) | ((u32::from(light & 15) * 16) << 8);
            let mut quad =
                |corners: [glam::DVec3; 4], normal: glam::DVec3, uv: [[f32; 2]; 4], shade: f32| {
                    // Back faces are culled relative to the camera.
                    if !faces_camera(&corners, normal) {
                        return;
                    }
                    let k = (255.0 * shade) as u32;
                    let color = 0xFF00_0000 | k << 16 | k << 8 | k;
                    for (p, uv) in corners.iter().zip(uv) {
                        out.push(Vertex {
                            normal: [0; 4],
                            pos: p.as_vec3().to_array(),
                            uv,
                            layer: 0,
                            color,
                            light: packed,

                            glint_uv: [0.0, 0.0],
                        });
                    }
                };
            let full = |r: sunrise_client::hud::Rect| {
                [[r.u0, r.v1], [r.u1, r.v1], [r.u1, r.v0], [r.u0, r.v0]]
            };
            let f = c + out_dir * d;
            quad(
                [
                    f - right * hw - up * hh,
                    f + right * hw - up * hh,
                    f + right * hw + up * hh,
                    f - right * hw + up * hh,
                ],
                out_dir,
                full(front),
                1.0,
            );
            let b = c - out_dir * d;
            quad(
                [
                    b + right * hw - up * hh,
                    b - right * hw - up * hh,
                    b - right * hw + up * hh,
                    b + right * hw + up * hh,
                ],
                -out_dir,
                full(back),
                0.8,
            );
            // Edges: a one-pixel strip of the back texture.
            let strip = [
                [back.u0, back.v0 + (back.v1 - back.v0) / 16.0],
                [back.u1, back.v0 + (back.v1 - back.v0) / 16.0],
                [back.u1, back.v0],
                [back.u0, back.v0],
            ];
            let (t, bt) = (up * hh, -up * hh);
            quad(
                [
                    b + t + right * hw,
                    b + t - right * hw,
                    f + t - right * hw,
                    f + t + right * hw,
                ],
                up,
                strip,
                1.0,
            );
            quad(
                [
                    f + bt + right * hw,
                    f + bt - right * hw,
                    b + bt - right * hw,
                    b + bt + right * hw,
                ],
                -up,
                strip,
                0.5,
            );
            quad(
                [
                    b - right * hw - up * hh,
                    f - right * hw - up * hh,
                    f - right * hw + up * hh,
                    b - right * hw + up * hh,
                ],
                -right,
                strip,
                0.6,
            );
            quad(
                [
                    f + right * hw - up * hh,
                    b + right * hw - up * hh,
                    b + right * hw + up * hh,
                    f + right * hw + up * hh,
                ],
                right,
                strip,
                0.6,
            );
            self.finish_entity_occlusion(culling, p, out, entity_start);
        }
    }
    /// `Painting.dropItem`: break sound, then item unless drops off or creative.
    pub(in crate::game) fn drop_painting(&mut self, p: Painting, at: [f64; 3], creative: bool) {
        if !self
            .ui
            .rules
            .bool_at(sunrise_client::gamerules::vanilla::ENTITY_DROPS)
        {
            return;
        }
        self.server_play_sound("minecraft:entity.painting.break", at, 1.0, 1.0);
        if creative {
            return;
        }
        let stack = ItemStack {
            item: Arc::from("minecraft:painting"),
            count: 1,
            damage: 0,
            extra: None,
        };
        self.spawn_item(
            stack,
            super::frames::hanging_drop_position(at, p.step()),
            [0.0; 3],
            10,
        );
    }
}
