//! Client presentation options in the game (`sunrise_client::pvp`, `docs/pvp.md`). Each
//! does nothing while it is off.
use sunrise_client::{
    hud::{HudBatch, rgba},
    mesher::Vertex,
    pvp::{ChatOptions, Colour, CrosshairCache, Frame},
};
use sunrise_gameplay_common::game::Side;
use sunrise_level::particle_options::ParticleType;

#[derive(Default)]
pub(in crate::game) struct PvpState {
    crosshair: CrosshairCache,
    /// While the chat box holds the search query (Ctrl+F), its message text and draft state.
    pub(in crate::game) chat_search_aside: Option<(String, bool)>,
}
impl crate::game::ClientGame<'_> {
    /// Whether the custom crosshair replaces vanilla's sprite.
    pub(in crate::game::client_side) fn custom_crosshair(&self) -> bool {
        self.client.settings.extras.pvp.crosshair.custom
    }
    /// The custom crosshair, centred where vanilla's 15x15 sprite is (`Hud.extractCrosshair`):
    /// its outline in black, then the shape, inverting what is under it when its colour is
    /// `Inverted`. The runs are cached, so a frame only adds their quads.
    pub(in crate::game::client_side) fn draw_custom_crosshair(
        &mut self,
        batch: &mut HudBatch,
        w: i32,
        h: i32,
    ) {
        let mut c = self.client.settings.extras.pvp.crosshair;
        if let Some(hover) = c.hover
            && self.crosshair_living() == Some(true)
        {
            c.colour = hover;
        }
        let (cx, cy) = ((w - 15) / 2 + 7, (h - 15) / 2 + 7);
        let atlas = &self.client.gui.atlas;
        let (shape, rim) = self.client.pvp.crosshair.runs(&c);
        let fill = |batch: &mut HudBatch, &(x0, y, x1): &(i32, i32, i32), colour: u32| {
            let (x, y) = ((cx + x0) as f32, (cy + y) as f32);
            batch.fill(atlas, x, y, (x1 - x0) as f32, 1.0, colour);
        };
        for run in rim {
            fill(batch, run, rgba(0, 0, 0, 255));
        }
        let start = batch.vertices.len();
        let [a, r, g, b] = c.colour.argb().to_be_bytes();
        for run in shape {
            fill(batch, run, rgba(r, g, b, a));
        }
        if c.colour == Colour::Inverted {
            batch.invert_since(start);
        }
    }
    /// The block outline's fill: each box of the targeted block's shape (`getShape`, the boxes
    /// the outline's edges come from) as translucent faces drawn after translucent terrain,
    /// pushed out a little so they don't fight the block's own faces. Only while the outline
    /// shows (`outline` is its edges).
    pub(in crate::game::client_side) fn outline_fill(
        &self,
        outline: &[[f64; 6]],
        camera: glam::DVec3,
        out: &mut Vec<Vertex>,
    ) {
        let Some(argb) = self.client.settings.extras.pvp.outline.fill() else {
            return;
        };
        let Some(hit) = self.pick().filter(|_| !outline.is_empty()) else {
            return;
        };
        let [x, y, z] = hit.pos;
        let state = self.common.world.block(x, y, z);
        let boxes = self.outline_boxes_iter(
            &self.common.world,
            state,
            self.client.shulkers(),
            hit.pos,
            true,
        );
        let uv = sunrise_client::gfx::lightning::white_uv(&self.client.gui.atlas.font.white());
        let [a, r, g, b] = argb.to_be_bytes();
        let color = u32::from_le_bytes([r, g, b, a]);
        const OUT: f64 = 0.002;
        for shape in boxes {
            let corner = |i: usize, high: bool| {
                let v = if high {
                    shape[i + 3] + OUT
                } else {
                    shape[i] - OUT
                };
                (v - camera[i]) as f32
            };
            // Each face: the axis it faces along, its side, and its corners' other two axes.
            for axis in 0..3 {
                let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                for high in [false, true] {
                    for (hu, hv) in [(false, false), (true, false), (true, true), (false, true)] {
                        let mut pos = [0.0; 3];
                        pos[axis] = corner(axis, high);
                        pos[u] = corner(u, hu);
                        pos[v] = corner(v, hv);
                        out.push(Vertex {
                            pos,
                            uv,
                            layer: sunrise_client::gfx::TRANSLUCENT_PARTICLE_OVERLAY,
                            color,
                            light: 240 | 240 << 8,
                            normal: [0; 4],
                            glint_uv: [0.0; 2],
                        });
                    }
                }
            }
        }
    }
    /// Combat feel Legacy's pictures: 1.8.9's sword block, flat item pose and hand swing.
    pub(in crate::game::client_side) fn pvp_legacy_look(&self) -> bool {
        self.client.settings.extras.pvp.combat.legacy
    }
    /// Combat feel Legacy hides the attack indicator unless the player keeps it.
    pub(in crate::game::client_side) fn pvp_hides_attack_indicator(&self) -> bool {
        let combat = self.client.settings.extras.pvp.combat;
        combat.legacy && !combat.indicator
    }
    /// A sound Combat feel Legacy keeps quiet.
    pub(in crate::game) fn pvp_mutes_sound(&self, name: &str) -> bool {
        self.client.settings.extras.pvp.combat.mutes_sound(name)
    }
    /// A particle Combat feel Legacy doesn't draw (sweeps, damage hearts).
    pub(in crate::game::client_side) fn pvp_hides_particle(&self, ty: ParticleType) -> bool {
        self.client.settings.extras.pvp.combat.hides_particle(ty)
    }
    /// The block outline and hurt overlay colours the renderer draws with.
    pub(in crate::game::client_side) fn pvp_render_frame(&self) -> Frame {
        self.client.settings.extras.pvp.frame()
    }
    /// The chat's options: Ctrl+F search, copy on click, and the rest (`ChatOptions::qol`).
    pub(in crate::game::client_side) fn pvp_chat(&self) -> ChatOptions {
        self.client.settings.extras.pvp.chat
    }
}
