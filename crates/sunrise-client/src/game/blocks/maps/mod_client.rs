#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::BANNER_WHITE;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::DECORATION_TYPES;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::Decoration;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::FRAME;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::Framed;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::HOST;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::Holder;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::Key;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::MapBanner;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::MapData;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::MapFrame;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::Maps;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::PLAYER;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::PLAYER_OFF_LIMITS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::PLAYER_OFF_MAP;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::StaticDecoration;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::decoration_pose;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::decoration_pose_at;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::map_id;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::packed_rgba;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::shown_on_frame;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::maps::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::collections::HashMap;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    blocks::{Fluid, MAP_COLORS},
    hud::{HudBatch, MAP_SLOTS, Rect, map_slot_sprite},
    items::ItemStack,
    mesher::Vertex,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::{JavaString, Tag};

impl Game<'_> {
    /// The atlas slot showing map `id`, its texture uploaded when it changed.
    pub(in crate::game) fn map_texture(&mut self, id: i32) -> Option<Rect> {
        self.map(id)?;
        let slot = match self.maps.slots.show(&id) {
            Some(slot) => slot,
            None => {
                let (slot, _) = self.maps.slots.claim(id)?;
                if let Some(m) = self.maps.data.get_mut(&id) {
                    m.texture_dirty = true;
                }
                slot
            }
        };
        let rect = self
            .gui
            .atlas
            .sprites
            .get(map_slot_sprite(slot)?)
            .copied()?;
        let m = self.maps.data.get_mut(&id)?;
        if m.texture_dirty {
            m.texture_dirty = false;
            let rgba: Vec<u8> = m.colors.iter().flat_map(|&c| packed_rgba(c)).collect();
            let (w, h) = (self.gui.atlas.width as f32, self.gui.atlas.height as f32);
            self.client.map_patches.push(sunrise_client::font::Patch {
                x: (rect.u0 * w).round() as u32,
                y: (rect.v0 * h).round() as u32,
                width: 128,
                height: 128,
                rgba,
            });
        }
        Some(rect)
    }
    /// `MapRenderer.render`: the map texture and its decorations under `m`
    /// (map pixels 0..128, y down), drawn at depth offsets toward the viewer.
    /// `show_only_frame` (a map in an item frame) leaves out the decorations that
    /// do not show on frames.
    pub(in crate::game) fn map_vertices(
        &mut self,
        id: i32,
        show_only_frame: bool,
        m: glam::Mat4,
        light: u32,
        out: &mut Vec<Vertex>,
    ) {
        let Some(rect) = self.map_texture(id) else {
            return;
        };
        let quad = |corners: [[f32; 3]; 4], uv: [[f32; 2]; 4], out: &mut Vec<Vertex>| {
            for (c, uv) in corners.into_iter().zip(uv) {
                out.push(Vertex {
                    normal: [0; 4],
                    pos: m.transform_point3(glam::Vec3::from_array(c)).to_array(),
                    uv,
                    layer: 0,
                    color: 0xFFFF_FFFF,
                    light,

                    glint_uv: [0.0, 0.0],
                });
            }
        };
        let z = -0.01;
        quad(
            [
                [0.0, 128.0, z],
                [128.0, 128.0, z],
                [128.0, 0.0, z],
                [0.0, 0.0, z],
            ],
            [
                [rect.u0, rect.v1],
                [rect.u1, rect.v1],
                [rect.u1, rect.v0],
                [rect.u0, rect.v0],
            ],
            out,
        );
        let decorations = self.maps.data.get(&id).map_or(&[][..], |m| &m.decorations);
        let mut count = 0;
        for d in decorations {
            if show_only_frame && !shown_on_frame(d.sprite) {
                continue;
            }
            let Some(s) = self.gui.atlas.gui_rect(d.sprite).copied() else {
                continue;
            };
            let dm = decoration_pose(d, -0.02 - count as f32 * 0.001);
            let p = |x: f32, y: f32| dm.transform_point3(glam::Vec3::new(x, y, 0.0)).to_array();
            let mut q = |corners: [[f32; 3]; 4], uv: [[f32; 2]; 4]| {
                for (c, uv) in corners.into_iter().zip(uv) {
                    out.push(Vertex {
                        normal: [0; 4],
                        pos: m.transform_point3(glam::Vec3::from_array(c)).to_array(),
                        uv,
                        layer: 0,
                        color: 0xFFFF_FFFF,
                        light,

                        glint_uv: [0.0, 0.0],
                    });
                }
            };
            q(
                [p(-1.0, 1.0), p(1.0, 1.0), p(1.0, -1.0), p(-1.0, -1.0)],
                [[s.u0, s.v0], [s.u1, s.v0], [s.u1, s.v1], [s.u0, s.v1]],
            );
            if let Some(name) = &d.name {
                let text = sunrise_client::text::Text::literal(name);
                let f = self.gui.atlas.font.width_text(&text);
                if f > 0.0 {
                    let f1 = (25.0 / f).clamp(0.0, 6.0 / 9.0);
                    let mut text_batch = HudBatch::scratch([1, 1]);
                    let behind =
                        text_batch.name_tag(&self.gui.atlas, &text, 0.0, 0xFFFF_FFFF, 0x8000_0000);
                    self.client.map_patches.append(&mut text_batch.patches);
                    let tm = glam::Mat4::from_translation(glam::Vec3::new(
                        f32::from(d.x) / 2.0 + 64.0,
                        f32::from(d.y) / 2.0 + 64.0 + 4.0,
                        // MapRenderer translates by -0.025, then flips Z and
                        // translates by 0.1 in text space: -0.025 - 0.1.
                        -0.125,
                    )) * glam::Mat4::from_scale(glam::Vec3::new(f1, f1, 1.0));
                    for (i, quad) in text_batch.vertices.as_chunks::<6>().0.iter().enumerate() {
                        let z_shift = if i * 6 < behind { 0.0005 } else { -0.0005 };
                        for v in [quad[5], quad[2], quad[1], quad[0]] {
                            out.push(Vertex {
                                normal: [0; 4],
                                pos: m
                                    .transform_point3(tm.transform_point3(glam::Vec3::new(
                                        v.pos[0], v.pos[1], z_shift,
                                    )))
                                    .to_array(),
                                uv: v.uv,
                                layer: 0,
                                color: v.color,
                                light,
                                glint_uv: [0.0, 0.0],
                            });
                        }
                    }
                }
            }
            count += 1;
        }
        // A native mod's own icons, over the vanilla ones.
        for d in self.mod_map_decorations(id, show_only_frame) {
            let Some(s) = self.gui.atlas.gui_rect(d.sprite.as_str()).copied() else {
                continue;
            };
            let dm = decoration_pose_at(d.x, d.y, d.rot, -0.02 - count as f32 * 0.001);
            let p = |x: f32, y: f32| dm.transform_point3(glam::Vec3::new(x, y, 0.0)).to_array();
            let corners = [p(-1.0, 1.0), p(1.0, 1.0), p(1.0, -1.0), p(-1.0, -1.0)];
            let uvs = [[s.u0, s.v0], [s.u1, s.v0], [s.u1, s.v1], [s.u0, s.v1]];
            for (c, uv) in corners.into_iter().zip(uvs) {
                out.push(Vertex {
                    normal: [0; 4],
                    pos: m.transform_point3(glam::Vec3::from_array(c)).to_array(),
                    uv,
                    layer: 0,
                    color: 0xFFFF_FFFF,
                    light,
                    glint_uv: [0.0, 0.0],
                });
            }
            count += 1;
        }
    }
    /// `CompassItem.useOn`: a compass used on a lodestone locks onto it (a
    /// single one in survival changes in place, else one is split off).
    pub(in crate::game) fn use_lodestone_compass(
        &mut self,
        hit: &super::Hit,
        stack: &ItemStack,
        before: &ItemStack,
    ) -> bool {
        if &*stack.item != "minecraft:compass" {
            return false;
        }
        let [x, y, z] = hit.pos;
        if self
            .res
            .facts()
            .get(self.world.block(x, y, z))
            .name
            .as_ref()
            != "minecraft:lodestone"
        {
            return false;
        }
        let at = [x, y, z].map(|v| f64::from(v) + 0.5);
        self.trigger_block("minecraft:item_used_on_block", hit.pos, before);
        self.audio
            .0
            .play("minecraft:item.lodestone_compass.lock", Some(at), 1.0, 1.0);
        let target = Tag::compound([
            (
                "target",
                Tag::compound([
                    ("dimension", Tag::String(self.dimension_id().into())),
                    ("pos", Tag::IntArray(vec![x, y, z])),
                ]),
            ),
            ("tracked", Tag::Byte(1)),
        ]);
        if !self.creative && stack.count == 1 {
            self.edit_block_use_stack(|cell| {
                if let Some(held) = cell {
                    held.data_mut()
                        .set_other("minecraft:lodestone_tracker", Some(target.clone()));
                }
            });
        } else {
            let mut locked = stack.clone();
            locked.count = 1;
            locked
                .data_mut()
                .set_other("minecraft:lodestone_tracker", Some(target));
            self.consume_held();
            self.give_or_drop_stack(locked);
        }
        self.award_stat("used", &stack.item, 1);
        self.swing_arm();
        true
    }
}
