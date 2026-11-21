//! 2D overlay: one atlas holding GUI sprites, the bitmap font, item and block
//! textures, and a batch of textured quads in GUI pixels (vanilla's automatic
//! GUI scale is applied when drawing).
use crate::{Result, resource_source::ResourceSource};
use smallvec::SmallVec;
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};
use sunrise_assets::animation::AnimationMetadata;
use wgpu::util::DeviceExt;

use model_reducing::reducible_model_sprites;

mod animate;
pub mod cit_glint;
mod clip_corner;
pub mod dynamic_textures;
mod gui_sprite;
pub use gui_sprite::atlas_key_of_gui_sprite;
mod menu_pictures;
mod model_reducing;
pub(crate) mod pack_text;
pub mod pictures;
pub(crate) mod sprite_modifiers;

/// GUI sprite ids a draw asked for that the atlas lacks, each warned about once.
static MISSING_GUI_SPRITES: std::sync::Mutex<BTreeSet<String>> =
    std::sync::Mutex::new(BTreeSet::new());

#[cfg(test)]
thread_local! {
    static GUI_SPRITE_MISSES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The GUI sprite ids this thread's draws missed since the last call, for tests that a screen names only real sprites.
#[cfg(test)]
pub(crate) fn take_gui_sprite_misses() -> Vec<String> {
    GUI_SPRITE_MISSES.with(|misses| std::mem::take(&mut *misses.borrow_mut()))
}

/// A draw named a GUI sprite the atlas lacks (a wrong name, a pack's filter, a mod's id): one warning per id.
fn missing_gui_sprite(id: &str) {
    #[cfg(test)]
    GUI_SPRITE_MISSES.with(|misses| misses.borrow_mut().push(id.to_owned()));
    let mut warned = MISSING_GUI_SPRITES
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if warned.insert(id.to_owned()) {
        log::warn!("no GUI sprite {id:?} in the atlas; drawing the missing texture");
    }
}

/// A sprite's atlas region, and its size in GUI pixels: the size of the
/// default resources' texture of that name, whatever the resolution of the
/// pack that replaced it (vanilla's GUI code draws in those units: `blit`
/// divides by the texture size it is given, sprites by their `gui.scaling`
/// size). Textures the default resources lack keep their own size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub w: u32,
    pub h: u32,
}
impl Rect {
    /// The region's size in texels of an atlas this large.
    pub fn texels(&self, (width, height): (u32, u32)) -> (u32, u32) {
        (
            ((self.u1 - self.u0) * width as f32).round() as u32,
            ((self.v1 - self.v0) * height as f32).round() as u32,
        )
    }
}
/// `TextureMetadataSection`'s `blur` and `clamp`: how vanilla samples a
/// standalone texture (`ReloadableTexture.setSampler`: linear rather than
/// nearest filtering, clamp-to-edge rather than repeat addressing).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextureFlags {
    pub blur: bool,
    pub clamp: bool,
}
impl TextureFlags {
    /// The `texture` section of a `.png.mcmeta` file, if it sets either flag.
    pub fn from_mcmeta(json: &str) -> Option<Self> {
        let meta: serde_json::Value = crate::packs::first_json(json.as_bytes()).ok()?;
        let section = meta.get("texture")?;
        let flag = |key| section.get(key).and_then(serde_json::Value::as_bool) == Some(true);
        let flags = Self {
            blur: flag("blur"),
            clamp: flag("clamp"),
        };
        (flags != Self::default()).then_some(flags)
    }
}
mod atlas;
pub use atlas::*;
mod image;
pub use image::*;
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HudVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: u32,
    pub depth: f32,
    /// The `GLINT_TEXTURING` UV into the repeating glint texture, unused
    /// unless `uv.x >= 2`.
    pub glint_uv: [f32; 2],
    /// 1 for the GUI's text (`RenderPipelines.GUI_TEXT`), which a resource
    /// pack's text shader draws when it has one.
    pub text: u32,
}
/// Sutherland-Hodgman against one half plane, interpolating every attribute.
fn clip_polygon(poly: &[HudVertex], axis: usize, limit: f32, keep_below: bool) -> Vec<HudVertex> {
    let inside = |v: &HudVertex| {
        if keep_below {
            v.pos[axis] < limit
        } else {
            v.pos[axis] >= limit
        }
    };
    let lerp = |a: &HudVertex, b: &HudVertex, t: f32| {
        let mix = |p: f32, q: f32| p + (q - p) * t;
        let mut v = *a;
        v.pos = [mix(a.pos[0], b.pos[0]), mix(a.pos[1], b.pos[1])];
        v.uv = [mix(a.uv[0], b.uv[0]), mix(a.uv[1], b.uv[1])];
        v.depth = mix(a.depth, b.depth);
        // A shaded shape (a gradient, `hud_kit::shapes`) keeps its colours across the cut.
        v.color = crate::hud_kit::shapes::mix_colour(a.color, b.color, t);
        v.glint_uv = [
            mix(a.glint_uv[0], b.glint_uv[0]),
            mix(a.glint_uv[1], b.glint_uv[1]),
        ];
        v.pos[axis] = limit;
        v
    };
    let mut out = Vec::with_capacity(poly.len() + 2);
    for i in 0..poly.len() {
        let (a, b) = (&poly[i], &poly[(i + 1) % poly.len()]);
        let (ia, ib) = (inside(a), inside(b));
        if ia {
            out.push(*a);
        }
        if ia != ib {
            let t = (limit - a.pos[axis]) / (b.pos[axis] - a.pos[axis]);
            out.push(lerp(a, b, t));
        }
    }
    out
}
/// Scale inputs for one view, copied from its owner's settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuiScale {
    pub requested: u32,
    pub unicode: bool,
}

/// One frame of overlay geometry in GUI pixels.
pub struct HudBatch {
    pub pictures: Vec<Arc<pictures::Picture>>,
    pub vertices: Vec<HudVertex>,
    pub scale: f32,
    pub width: f32,
    pub height: f32,
    /// Glyph pixels to upload into the atlas before drawing.
    pub patches: Vec<crate::font::Patch>,
    /// Client mods' dynamic textures to upload into their atlas slots before drawing, sharing
    /// the pixels the game keeps of them.
    pub image_patches: Vec<dynamic_textures::ImagePatch>,
    /// Vertex ranges with another blend or depth/scissor state, in order.
    pub blends: Vec<(std::ops::Range<u32>, Blend)>,
    /// The textures and time of the `Blend::EndPortal` runs.
    pub end_portal: EndPortalFill,
    /// `options.glintStrength` for the glint pass (`GlintAlpha`).
    pub glint_alpha: f32,
    /// PvP & HUD's glint colour (`pvp::Tweaks::glint_texel`), 0 for vanilla's.
    pub glint_colour: u32,
    /// A resource pack text shader's `GameTime`: the day's fraction
    /// (`gameTime % 24000` and the partial tick, over 24000).
    pub game_time: f32,
    /// Screen's first vertex: the preceding HUD is part of its blurred background.
    pub blur_at: u32,
    /// Player object profiles encountered while painting text this frame.
    pub inline_profiles: Vec<std::sync::Arc<serde_json::Value>>,
}
/// How a run of HUD vertices blends and tests depth.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Blend {
    Picture(u64),
    /// `BlendFunction.TRANSLUCENT`.
    Alpha,
    /// `BlendFunction.INVERT` (`RenderPipelines.CROSSHAIR`, `GUI_INVERT`).
    Invert,
    /// `BlendFunction.ADDITIVE` (`RenderPipelines.GUI_TEXT_HIGHLIGHT`).
    Additive,
    /// Alpha blending with an isolated depth buffer and a pixel scissor.
    Depth([u32; 4]),
    /// The glint over the `Depth` run before it with the same scissor, in its
    /// pass: `RenderPipelines.GLINT`'s additive blend, depth tested against the
    /// model and never written.
    DepthGlint([u32; 4]),
    /// `RenderPipelines.VIGNETTE`: the destination darkened by the texture
    /// (`ZERO, ONE_MINUS_SRC_COLOR`).
    Vignette,
    /// `RenderPipelines.END_PORTAL` as a GUI fill: the End portal's layers
    /// projected from the screen (`HudBatch::end_portal` sets the textures).
    EndPortal,
    /// An OptiFine CIT glint layer (`cit_glint`), blended as entry `n` of
    /// `GlintTextures::blends` says.
    CitGlint(u8),
}
/// What a `Blend::EndPortal` fill samples: the atlas rects (u0, v0, width,
/// height) of `end_sky.png` and `end_portal.png`, and the `GameTime` uniform.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EndPortalFill {
    pub sprites: [[f32; 4]; 2],
    pub game_time: f32,
}
pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    (a as u32) << 24 | (b as u32) << 16 | (g as u32) << 8 | r as u32
}
impl HudBatch {
    /// Window.calculateScale: the largest scale keeping >= 320x240 GUI pixels,
    /// capped by the GUI Scale option (0 = auto).
    pub fn new(window: [u32; 2], inputs: GuiScale) -> Self {
        let gui =
            sunrise_client_input::window::GuiSize::new(window, inputs.requested, inputs.unicode);
        Self::with_gui_size(gui)
    }
    /// An offscreen or test batch, independent of the active view's settings.
    pub fn scratch(window: [u32; 2]) -> Self {
        Self::with_gui_size(sunrise_client_input::window::GuiSize::new(window, 0, false))
    }
    fn with_gui_size(gui: sunrise_client_input::window::GuiSize) -> Self {
        Self {
            pictures: Vec::new(),
            vertices: Vec::new(),
            scale: gui.scale as f32,
            width: gui.size[0] as f32,
            height: gui.size[1] as f32,
            patches: Vec::new(),
            image_patches: Vec::new(),
            blends: Vec::new(),
            end_portal: EndPortalFill::default(),
            glint_alpha: 0.75,
            glint_colour: 0,
            game_time: 0.0,
            blur_at: 0,
            inline_profiles: Vec::new(),
        }
    }
    /// Draw the vertices added since `start` with the inverting blend.
    pub fn invert_since(&mut self, start: usize) {
        self.blend_since(start, Blend::Invert);
    }
    /// Draw the vertices added since `start` with `blend`.
    pub fn blend_since(&mut self, start: usize, blend: Blend) {
        let range = start as u32..self.vertices.len() as u32;
        if range.is_empty() || blend == Blend::Alpha {
            return;
        }
        match self.blends.last_mut() {
            Some((last, b)) if last.end == range.start && *b == blend => last.end = range.end,
            _ => self.blends.push((range, blend)),
        }
    }
    /// `GuiGraphicsExtractor.textHighlight`: with `invert_text` the area's
    /// colours are inverted first (white text on a dark field turns blue on
    /// white), then blue is added.
    pub fn text_highlight(
        &mut self,
        atlas: &HudAtlas,
        (x, y, w, h): (f32, f32, f32, f32),
        invert_text: bool,
    ) {
        if invert_text {
            let start = self.vertices.len();
            self.fill(atlas, x, y, w, h, rgba(255, 255, 255, 255));
            self.invert_since(start);
        }
        let start = self.vertices.len();
        self.fill(atlas, x, y, w, h, rgba(0, 0, 255, 255));
        self.blend_since(start, Blend::Additive);
    }
    /// The vertex ranges in order, each with its blend.
    pub fn runs(&self) -> Vec<(std::ops::Range<u32>, Blend)> {
        let mut out = Vec::new();
        let mut at = 0;
        for (r, blend) in &self.blends {
            if r.start > at {
                out.push((at..r.start, Blend::Alpha));
            }
            out.push((r.clone(), *blend));
            at = r.end;
        }
        let end = self.vertices.len() as u32;
        if end > at {
            out.push((at..end, Blend::Alpha));
        }
        out
    }
    fn runs_in(
        &self,
        vertices: std::ops::Range<u32>,
    ) -> SmallVec<[(std::ops::Range<u32>, Blend); 16]> {
        let mut runs = SmallVec::new();
        let mut at = vertices.start;
        for (range, blend) in &self.blends {
            if range.start > at {
                let start = at.max(vertices.start);
                let end = range.start.min(vertices.end);
                if start < end {
                    runs.push((start..end, Blend::Alpha));
                }
            }
            let start = range.start.max(vertices.start);
            let end = range.end.min(vertices.end);
            if start < end {
                runs.push((start..end, *blend));
            }
            at = at.max(range.end);
        }
        let end = (self.vertices.len() as u32).min(vertices.end);
        if at < end {
            runs.push((at.max(vertices.start)..end, Blend::Alpha));
        }
        runs
    }
    /// `AbstractContainerScreen.init`: center in integer GUI coordinates.
    pub fn centered_origin(&self, width: f32, height: f32) -> (f32, f32) {
        (
            ((self.width - width) / 2.0).trunc(),
            ((self.height - height) / 2.0).trunc(),
        )
    }
    pub fn quad(&mut self, p: [[f32; 2]; 4], uv: [[f32; 2]; 4], color: u32) {
        for i in [0, 1, 2, 0, 2, 3] {
            self.vertices.push(HudVertex {
                pos: p[i],
                uv: uv[i],
                color,
                depth: 0.0,
                glint_uv: [0.0, 0.0],
                text: 0,
            });
        }
    }
    /// A quad whose texture is the `GLINT_TEXTURING` sample at `glint_uv`;
    /// `uv` is the item's atlas UV, which the shader masks by.
    pub fn quad_glint(
        &mut self,
        p: [[f32; 2]; 4],
        uv: [[f32; 2]; 4],
        glint_uv: [[f32; 2]; 4],
        color: u32,
    ) {
        for i in [0, 1, 2, 0, 2, 3] {
            self.vertices.push(HudVertex {
                pos: p[i],
                uv: uv[i],
                color,
                depth: 0.0,
                glint_uv: glint_uv[i],
                text: 0,
            });
        }
    }
    pub fn quad_3d(&mut self, p: [[f32; 3]; 4], uv: [[f32; 2]; 4], color: u32) {
        for i in [0, 1, 2, 0, 2, 3] {
            self.vertices.push(HudVertex {
                pos: [p[i][0], p[i][1]],
                uv: uv[i],
                color,
                depth: p[i][2],
                glint_uv: [0.0, 0.0],
                text: 0,
            });
        }
    }
    /// `RenderTypes.armorCutoutNoCullGlint` (and the items' glint types) over a
    /// [`Self::quad_3d`] face: `uv` is the face's atlas UV,
    /// which the shader masks by, and `glint_uv` the transformed glint UV.
    /// `armor` samples `enchanted_glint_armor` (`ARMOR_ENTITY_GLINT`), otherwise
    /// `enchanted_glint_item`. Drawn by [`Self::depth_glint_since`].
    pub fn quad_3d_glint(
        &mut self,
        p: [[f32; 3]; 4],
        uv: [[f32; 2]; 4],
        glint_uv: [[f32; 2]; 4],
        armor: bool,
    ) {
        let marker = if armor { 4.0 } else { 2.0 };
        for i in [0, 1, 2, 0, 2, 3] {
            self.vertices.push(HudVertex {
                pos: [p[i][0], p[i][1]],
                uv: [uv[i][0] + marker, uv[i][1]],
                color: u32::MAX,
                depth: p[i][2],
                glint_uv: glint_uv[i],
                text: 0,
            });
        }
    }
    /// Keep per-vertex depth for an orthographic GUI model, with +z toward the viewer. `clip` is
    /// an explicit scissor in GUI pixels, cut to whole device pixels as it always was.
    pub fn depth_since(&mut self, start: usize, clip: [f32; 4]) {
        self.depth_glint_since(start, self.vertices.len(), clip);
    }
    /// `RenderPipelines.GLINT` after a GUI model: [`Self::depth_since`] for a model whose glint faces
    /// ([`Self::quad_3d_glint`]) follow its own from `glint`: both share the
    /// depth buffer, and the glint is drawn over the model in the same pass.
    pub fn depth_glint_since(&mut self, start: usize, glint: usize, clip: [f32; 4]) {
        let [x0, y0, x1, y1] = clip.map(|v| (v * self.scale).max(0.0) as u32);
        self.depth_pass_since(
            start,
            glint,
            [x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0)],
        );
    }
    /// [`Self::depth_glint_since`] for the screen bounds of the geometry itself (`lo` to `hi`, in
    /// GUI pixels): the scissor takes every device pixel the bounds touch, so the min edges round
    /// down and the max edges up.
    pub fn depth_glint_over(&mut self, start: usize, glint: usize, lo: [f32; 2], hi: [f32; 2]) {
        let [x0, y0] = lo.map(|v| (v * self.scale).max(0.0).floor() as u32);
        let [x1, y1] = hi.map(|v| (v * self.scale).max(0.0).ceil() as u32);
        self.depth_pass_since(
            start,
            glint,
            [x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0)],
        );
    }
    /// [`Self::depth_since`] for the screen bounds of the geometry itself: see
    /// [`Self::depth_glint_over`].
    pub fn depth_over(&mut self, start: usize, lo: [f32; 2], hi: [f32; 2]) {
        self.depth_glint_over(start, self.vertices.len(), lo, hi);
    }
    /// The depth runs of `start..` with the scissor `clip` in device pixels (`[x, y, w, h]`).
    fn depth_pass_since(&mut self, start: usize, glint: usize, clip: [u32; 4]) {
        let glint = glint.clamp(start, self.vertices.len());
        let vertices = &mut self.vertices[start..];
        if vertices.is_empty() {
            return;
        }
        let min = vertices
            .iter()
            .map(|v| v.depth)
            .fold(f32::INFINITY, f32::min);
        let max = vertices
            .iter()
            .map(|v| v.depth)
            .fold(f32::NEG_INFINITY, f32::max);
        for v in vertices {
            v.depth = (max - v.depth) / (max - min).max(f32::EPSILON);
        }
        // Separate previews clear depth independently, even if their scissors match.
        if glint > start {
            self.blends
                .push((start as u32..glint as u32, Blend::Depth(clip)));
        }
        let end = self.vertices.len();
        if end > glint {
            self.blends
                .push((glint as u32..end as u32, Blend::DepthGlint(clip)));
        }
    }
    pub fn rect_uv(&mut self, x: f32, y: f32, w: f32, h: f32, r: Rect, color: u32) {
        self.quad(
            [[x, y], [x + w, y], [x + w, y + h], [x, y + h]],
            [[r.u0, r.v0], [r.u1, r.v0], [r.u1, r.v1], [r.u0, r.v1]],
            color,
        );
    }
    pub fn sprite(&mut self, atlas: &HudAtlas, name: &str, x: f32, y: f32) {
        if let Some(r) = atlas.gui_draw_rect(name).copied() {
            self.rect_uv(x, y, r.w as f32, r.h as f32, r, u32::MAX);
        }
    }
    pub fn sprite_scaled(
        &mut self,
        atlas: &HudAtlas,
        name: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: u32,
    ) {
        if let Some(r) = atlas.gui_draw_rect(name).copied() {
            self.rect_uv(x, y, w, h, r, color);
        }
    }
    /// Draw part of a sprite, `fraction` of its width from the left.
    pub fn sprite_part(&mut self, atlas: &HudAtlas, name: &str, x: f32, y: f32, fraction: f32) {
        if let Some(mut r) = atlas.gui_draw_rect(name).copied() {
            let w = r.w as f32 * fraction;
            r.u1 = r.u0 + (r.u1 - r.u0) * fraction;
            self.rect_uv(x, y, w, r.h as f32, r, u32::MAX);
        }
    }
    /// Clip the quads drawn since `start` to `[x0, x1) x [y0, y1)` (the
    /// GUI scissor): axis-aligned quads are cut with their UVs, others (item
    /// triangles) are clipped as polygons.
    /// Scale the vertices pushed since `start` about `(cx, cy)` (a GUI pose
    /// matrix of translate, scale, translate back).
    pub fn scale_since(&mut self, start: usize, cx: f32, cy: f32, sx: f32, sy: f32) {
        for v in &mut self.vertices[start..] {
            v.pos[0] = cx + (v.pos[0] - cx) * sx;
            v.pos[1] = cy + (v.pos[1] - cy) * sy;
        }
    }
    pub fn clip_since(&mut self, start: usize, x0: f32, y0: f32, x1: f32, y1: f32) {
        let quads: Vec<[HudVertex; 6]> = self.vertices[start..].as_chunks::<6>().0.to_vec();
        self.vertices.truncate(start);
        // Where each source quad now begins, to move the blend ranges (foil
        // items are an additive run) along with their vertices.
        let mut moved = Vec::with_capacity(quads.len() + 1);
        for q in quads {
            moved.push(self.vertices.len() as u32);
            let corners = [q[0], q[1], q[2], q[5]];
            let (mut x_lo, mut x_hi) = (f32::INFINITY, f32::NEG_INFINITY);
            let (mut y_lo, mut y_hi) = (f32::INFINITY, f32::NEG_INFINITY);
            for v in &corners {
                x_lo = x_lo.min(v.pos[0]);
                x_hi = x_hi.max(v.pos[0]);
                y_lo = y_lo.min(v.pos[1]);
                y_hi = y_hi.max(v.pos[1]);
            }
            // An axis-aligned rectangle, its corners in any order.
            let find = |x: f32, y: f32| corners.iter().find(|v| v.pos == [x, y]).copied();
            let rect = (
                find(x_lo, y_lo),
                find(x_hi, y_lo),
                find(x_hi, y_hi),
                find(x_lo, y_hi),
            );
            let (Some(tl), Some(tr), Some(br), Some(bl)) = rect else {
                // Item geometry arrives as triangles: clip each against the
                // rectangle and re-emit the fan, so a triangle straddling the
                // edge keeps its visible part instead of vanishing whole.
                for tri in [[q[0], q[1], q[2]], [q[3], q[4], q[5]]] {
                    let area = (tri[1].pos[0] - tri[0].pos[0]) * (tri[2].pos[1] - tri[0].pos[1])
                        - (tri[2].pos[0] - tri[0].pos[0]) * (tri[1].pos[1] - tri[0].pos[1]);
                    if area == 0.0 {
                        continue;
                    }
                    let mut poly = tri.to_vec();
                    for (axis, limit, keep_below) in
                        [(0, x0, false), (0, x1, true), (1, y0, false), (1, y1, true)]
                    {
                        poly = clip_polygon(&poly, axis, limit, keep_below);
                    }
                    for i in 1..poly.len().saturating_sub(1) {
                        // A triangle as the quad `a, b, c, c`.
                        let (a, b, c) = (poly[0], poly[i], poly[i + 1]);
                        self.vertices.extend_from_slice(&[a, b, c, a, c, c]);
                    }
                }
                continue;
            };
            let (cx0, cy0) = (x_lo.max(x0), y_lo.max(y0));
            let (cx1, cy1) = (x_hi.min(x1), y_hi.min(y1));
            if cx0 >= cx1 || cy0 >= cy1 {
                continue;
            }
            let t = |p: f32, lo: f32, hi: f32| if hi == lo { 0.0 } else { (p - lo) / (hi - lo) };
            let corner = |x, y| {
                clip_corner::clip_corner(
                    [tl, tr, br, bl],
                    [x, y],
                    [t(x, x_lo, x_hi), t(y, y_lo, y_hi)],
                )
            };
            let c = [
                corner(cx0, cy0),
                corner(cx1, cy0),
                corner(cx1, cy1),
                corner(cx0, cy1),
            ];
            self.vertices.extend([0, 1, 2, 0, 2, 3].map(|i| c[i]));
        }
        moved.push(self.vertices.len() as u32);
        let at = |v: u32| moved[((v as usize).saturating_sub(start) / 6).min(moved.len() - 1)];
        for (range, _) in &mut self.blends {
            if range.start as usize >= start {
                *range = at(range.start)..at(range.end);
            }
        }
        self.blends.retain(|(range, _)| !range.is_empty());
    }
    /// `blit` of the top-left `w x h` texels of a `tw x th` texture.
    pub fn sprite_region(
        &mut self,
        atlas: &HudAtlas,
        name: &str,
        [x, y, w, h]: [f32; 4],
        tw: f32,
        th: f32,
    ) {
        if let Some(mut r) = atlas.sprites.get(name).copied() {
            r.u1 = r.u0 + (r.u1 - r.u0) * w / tw;
            r.v1 = r.v0 + (r.v1 - r.v0) * h / th;
            self.rect_uv(x, y, w, h, r, u32::MAX);
        }
    }
    pub fn fill(&mut self, atlas: &HudAtlas, x: f32, y: f32, w: f32, h: f32, color: u32) {
        let mut r = atlas.white;
        let (cu, cv) = ((r.u0 + r.u1) / 2.0, (r.v0 + r.v1) / 2.0);
        r.u0 = cu;
        r.u1 = cu;
        r.v0 = cv;
        r.v1 = cv;
        self.rect_uv(x, y, w, h, r, color);
    }
    /// Any four-cornered shape in one colour, corners in order round it.
    pub fn fill_quad(&mut self, atlas: &HudAtlas, corners: [[f32; 2]; 4], color: u32) {
        let w = atlas.white;
        let uv = [(w.u0 + w.u1) / 2.0, (w.v0 + w.v1) / 2.0];
        self.quad(corners, [uv; 4], color);
    }
    /// GuiGraphics.fillGradient: `top` to `bottom`, down the rectangle.
    pub fn fill_gradient(
        &mut self,
        atlas: &HudAtlas,
        (x, y, w, h): (f32, f32, f32, f32),
        top: u32,
        bottom: u32,
    ) {
        let r = atlas.white;
        let uv = [(r.u0 + r.u1) / 2.0, (r.v0 + r.v1) / 2.0];
        let p = [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
        let c = [top, top, bottom, bottom];
        for i in [0, 1, 2, 0, 2, 3] {
            self.vertices.push(HudVertex {
                pos: p[i],
                uv,
                color: c[i],
                depth: 0.0,
                glint_uv: [0.0, 0.0],
                text: 0,
            });
        }
    }
    pub fn text_width(atlas: &HudAtlas, text: &str) -> f32 {
        atlas.font.width(text)
    }
    /// Draw plain text (legacy `§` codes are honoured); returns the advance.
    pub fn text(
        &mut self,
        atlas: &HudAtlas,
        text: &str,
        x: f32,
        y: f32,
        color: u32,
        shadow: bool,
    ) -> f32 {
        let t = if text.contains('§') {
            crate::text::Text::legacy(text)
        } else {
            crate::text::Text::literal(text)
        };
        self.text_styled(atlas, &t, x, y, color, shadow)
    }
    /// Font.drawInternal: styled glyphs, shadows first, with bold, italic,
    /// underline and strikethrough. `color` is the default colour and alpha.
    pub fn text_styled(
        &mut self,
        atlas: &HudAtlas,
        text: &crate::text::Text,
        x: f32,
        y: f32,
        color: u32,
        shadow: bool,
    ) -> f32 {
        self.styled_text(atlas, text, x, y, color, shadow, false, None)
    }
    /// `text_styled` for text drawn in the world (`TextRenderable.render`
    /// with `flat` false): each quad's `depth` is its z in text units, the
    /// glyphs `Font.SHADOW_DEPTH` in front of their shadow, a bold copy 0.001
    /// in front of its glyph and the lines 0.01 (`BakedSheetGlyph`), so they
    /// do not fight over the same pixels.
    pub fn text_styled_world(
        &mut self,
        atlas: &HudAtlas,
        text: &crate::text::Text,
        x: f32,
        y: f32,
        color: u32,
        shadow: bool,
    ) -> f32 {
        self.styled_text(atlas, text, x, y, color, shadow, true, None)
    }
    /// `Font.drawInBatch8xOutline`: eight glyph offsets in the outline color,
    /// then the original text. Returns (advance, first front-text vertex), so
    /// world rendering can use POLYGON_OFFSET for the front and NORMAL for
    /// its outline. Offsets use each glyph's own shadow offset (unihex: 0.5).
    pub fn text_styled_outline_world(
        &mut self,
        atlas: &HudAtlas,
        text: &crate::text::Text,
        x: f32,
        y: f32,
        color: u32,
        outline: u32,
    ) -> (f32, usize) {
        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx != 0 || dy != 0 {
                    self.styled_text(
                        atlas,
                        text,
                        x,
                        y,
                        color,
                        false,
                        true,
                        Some((outline, [dx as f32, dy as f32])),
                    );
                }
            }
        }
        let front = self.vertices.len();
        (
            self.styled_text(atlas, text, x, y, color, false, true, None),
            front,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn styled_text(
        &mut self,
        atlas: &HudAtlas,
        text: &crate::text::Text,
        x: f32,
        y: f32,
        color: u32,
        shadow: bool,
        world: bool,
        outline: Option<(u32, [f32; 2])>,
    ) -> f32 {
        // StringDecomposer.iterateFormatted inside Font.drawInternal:
        // `§` codes inside a span restyle what follows and are not drawn.
        let text = text.resolved();
        // Language.getVisualOrder: shaped and reordered for right-to-left text.
        let text = crate::bidi::visual(&text);
        let font = &atlas.font;
        let alpha = (color >> 24) as u8;
        let mut glyphs = Vec::new();
        let mut cursor = x;
        let mut previous = None;
        for (i, (ch, style)) in text.chars().enumerate() {
            let object = (ch == '\u{fffc}')
                .then_some(style.inline_object.as_ref())
                .flatten();
            if let Some(crate::text::InlineObject::Player { profile, .. }) = object
                && self.inline_profiles.len() < 256
                && !self.inline_profiles.contains(profile)
            {
                self.inline_profiles.push(profile.clone());
            }
            // Font.PreparedTextBuilder.accept: lines start 1 left of a
            // sequence's first character. Language.getVisualOrder makes a
            // sequence of each run of one style, which world text has.
            let first = i == 0 || world && previous != Some(style);
            previous = Some(style);
            // The style's font (`Style.getFont`), the default one unless set.
            let mut g = if object.is_some() {
                crate::font::Glyph {
                    rect: None,
                    left: 0.0,
                    top: 0.0,
                    right: 0.0,
                    bottom: crate::font::INLINE_OBJECT_SIZE,
                    advance: crate::font::INLINE_OBJECT_SIZE,
                    offset: 1.0,
                }
            } else {
                font.glyph_styled(ch, style)
            };
            // Font.StringRenderOutput.accept: obfuscated characters draw a
            // random glyph of the same width each frame.
            if object.is_none() && style.obfuscated == Some(true) && ch != ' ' {
                g = font.random_glyph(style, g.advance);
            }
            let c = style.color.map_or(color, |rgb| {
                rgba((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, alpha)
            });
            let bold = object.is_none() && style.is_bold();
            let advance = g.advance + if bold { g.offset } else { 0.0 };
            glyphs.push((
                cursor,
                g,
                c,
                style.shadow_color,
                bold,
                style.is_italic(),
                style.underlined == Some(true),
                style.strikethrough == Some(true),
                first,
                advance,
                object.cloned(),
            ));
            cursor += advance;
        }
        self.patches.extend(font.take_patches());
        let white = font.white();
        let has_shadow = outline.is_none()
            && (shadow
                || glyphs
                    .iter()
                    .any(|glyph| glyph.3.is_some_and(|color| color != 0)));
        let passes: &[bool] = if has_shadow { &[true, false] } else { &[false] };
        // ARGB.scaleRGB(color, 0.25): the default shadow colour.
        let shade = |c: u32| {
            let b = c.to_le_bytes();
            rgba(
                (f32::from(b[0]) * 0.25) as u8,
                (f32::from(b[1]) * 0.25) as u8,
                (f32::from(b[2]) * 0.25) as u8,
                b[3],
            )
        };
        for &is_shadow in passes {
            for (
                gx,
                g,
                c,
                custom_shadow,
                bold,
                italic,
                underline,
                strike,
                first,
                advance,
                object,
            ) in &glyphs
            {
                let shadow_color = if let Some(argb) = custom_shadow {
                    let text_alpha = (c >> 24) as u8 as u16;
                    let shadow_alpha = (argb >> 24) as u8 as u16;
                    let alpha = (shadow_alpha * text_alpha / 255) as u8;
                    rgba((argb >> 16) as u8, (argb >> 8) as u8, *argb as u8, alpha)
                } else {
                    shade(*c)
                };
                let has_shadow = outline.is_none()
                    && (custom_shadow.is_some_and(|argb| argb != 0)
                        || custom_shadow.is_none() && shadow)
                    && !(world && shadow_color == 0);
                if is_shadow && !has_shadow {
                    continue;
                }
                let (dx, col) = if is_shadow {
                    (g.offset, shadow_color)
                } else {
                    (0.0, *c)
                };
                let (px, py, col) = if let Some((color, [dx, dy])) = outline {
                    (*gx + dx * g.offset, y + dy * g.offset, color)
                } else {
                    (*gx + dx, y + dx, col)
                };
                if let Some(object) = object {
                    // `PlainTextRenderable.render`: sprite glyphs use the same
                    // shadow colour and offset as text, with the front at SHADOW_DEPTH.
                    let start = self.vertices.len();
                    self.draw_inline_object(atlas, object, px, py - 1.0, col);
                    if world && has_shadow && !is_shadow {
                        for v in &mut self.vertices[start..] {
                            v.depth = 0.03;
                        }
                    }
                    continue;
                }
                let shear = |v: f32| if *italic { 1.0 - 0.25 * v } else { 0.0 };
                let thick = if *bold { 0.1 } else { 0.0 };
                let draw = |batch: &mut Self, ox: f32| match g.rect {
                    Some(r) => {
                        let (x0, x1, y0, y1) = (
                            px + ox + g.left,
                            px + ox + g.right,
                            py + g.top,
                            py + g.bottom,
                        );
                        batch.quad(
                            [
                                [x0 + shear(g.top) - thick, y0 - thick],
                                [x1 + shear(g.top) + thick, y0 - thick],
                                [x1 + shear(g.bottom) + thick, y1 + thick],
                                [x0 + shear(g.bottom) - thick, y1 + thick],
                            ],
                            [[r.u0, r.v0], [r.u1, r.v0], [r.u1, r.v1], [r.u0, r.v1]],
                            col,
                        );
                    }
                    None if g.right > 0.0 => {
                        // SpecialGlyphs.MISSING: a 5x8 outline.
                        let (bx, by) = (px + ox, py);
                        for (fx, fy, fw, fh) in [
                            (0.0, 0.0, 5.0, 1.0),
                            (0.0, 7.0, 5.0, 1.0),
                            (0.0, 1.0, 1.0, 6.0),
                            (4.0, 1.0, 1.0, 6.0),
                        ] {
                            batch.rect_uv(bx + fx, by + fy, fw, fh, white, col);
                        }
                    }
                    None => {}
                };
                let base = if has_shadow && !is_shadow { 0.03 } else { 0.0 };
                let lift = |batch: &mut Self, start: usize, z: f32| {
                    if world {
                        for v in &mut batch.vertices[start..] {
                            v.depth = z;
                        }
                    }
                };
                let glyph = self.vertices.len();
                let start = self.vertices.len();
                draw(self, 0.0);
                lift(self, start, base);
                if *bold {
                    let start = self.vertices.len();
                    draw(self, g.offset);
                    lift(self, start, base + 0.001);
                }
                let x0 = if *first { px - 1.0 } else { px };
                let start = self.vertices.len();
                if *strike {
                    self.rect_uv(x0, py + 3.5, px + *advance - x0, 1.0, white, col);
                }
                if *underline {
                    self.rect_uv(x0, py + 8.0, px + *advance - x0, 1.0, white, col);
                }
                lift(self, start, base + 0.01);
                if !world {
                    for v in &mut self.vertices[glyph..] {
                        v.text = 1;
                    }
                }
            }
        }
        cursor - x
    }

    fn draw_inline_object(
        &mut self,
        atlas: &HudAtlas,
        object: &crate::text::InlineObject,
        x: f32,
        y: f32,
        tint: u32,
    ) {
        use crate::text::InlineObject;
        match object {
            InlineObject::Player { profile, hat, .. } => {
                let id = crate::skins::parse_profile(profile).id;
                let name = inline_profile_name(profile);
                let by_id = atlas.object_skins.get(&format!("id:{id:032x}"));
                let by_name = name.and_then(|name| {
                    atlas
                        .object_skins
                        .get(&format!("name:{}", name.to_ascii_lowercase()))
                });
                let resolved = if profile.get("id").is_some() {
                    by_id.or(by_name)
                } else {
                    by_name.or(by_id)
                };
                let default = crate::tab_list::default_skin(id);
                let skin = resolved
                    .filter(|path| atlas.sprites.contains_key(*path))
                    .map_or(default, String::as_str);
                let Some(rect) = atlas.sprites.get(skin).copied() else {
                    return;
                };
                for u in [8.0, 40.0].into_iter().take(if *hat { 2 } else { 1 }) {
                    let du = (rect.u1 - rect.u0) / 64.0;
                    let dv = (rect.v1 - rect.v0) / 64.0;
                    self.rect_uv(
                        x,
                        y,
                        8.0,
                        8.0,
                        Rect {
                            u0: rect.u0 + du * u,
                            v0: rect.v0 + dv * 8.0,
                            u1: rect.u0 + du * (u + 8.0),
                            v1: rect.v0 + dv * 16.0,
                            w: 8,
                            h: 8,
                        },
                        tint,
                    );
                }
            }
            InlineObject::Atlas { .. } => {
                if let Some(rect) = atlas.inline_object_sprite(object) {
                    let size = crate::font::INLINE_OBJECT_SIZE;
                    self.rect_uv(x, y, size, size, rect, tint);
                }
            }
        }
    }

    /// SubmitNodeCollection.submitNameTag's text in its own space (x right,
    /// y down, in GUI pixels): centred at `-Font.width / 2`, drawn as
    /// Font.prepareText draws any text (styles, fonts and glyph boxes; no
    /// shadow), over `background` (RGBA) unless its alpha is 0. Returns how
    /// many vertices at the start of the batch are the background.
    pub fn name_tag(
        &mut self,
        atlas: &HudAtlas,
        text: &crate::text::Text,
        y: f32,
        color: u32,
        background: u32,
    ) -> usize {
        let x = -atlas.font.width_text(text) / 2.0;
        let start = self.vertices.len();
        let advance = self.text_styled(atlas, text, x, y, color, false);
        if background >> 24 == 0 {
            return 0;
        }
        // PreparedTextBuilder.markBackground: one pixel round the advances,
        // from a pixel above the line to 9 below its top.
        let glyphs = self.vertices.split_off(start);
        self.fill(atlas, x - 1.0, y - 1.0, advance + 1.0, 10.0, background);
        let behind = self.vertices.len() - start;
        self.vertices.extend(glyphs);
        behind
    }
}

fn inline_profile_name(profile: &serde_json::Value) -> Option<&str> {
    profile
        .as_str()
        .or_else(|| profile.get("name").and_then(serde_json::Value::as_str))
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct HudUniform {
    size: [f32; 4],
    /// `EndPortalFill`: the two sprites, then the game time in x.
    end_portal: [[f32; 4]; 3],
}
impl HudUniform {
    fn new(size: [f32; 4], fill: EndPortalFill, glint_colour: u32) -> Self {
        let [r, g, b, _] = glint_colour.to_le_bytes().map(|c| f32::from(c) / 255.0);
        let tint = if glint_colour == 0 {
            [-1.0, 0.0, 0.0]
        } else {
            [r, g, b]
        };
        Self {
            size,
            end_portal: [
                fill.sprites[0],
                fill.sprites[1],
                [fill.game_time, tint[0], tint[1], tint[2]],
            ],
        }
    }
}
use crate::gfx::hud_coverage::{self, Pipeline};

pub struct HudRenderer {
    pictures: HashMap<u64, (Arc<pictures::Picture>, wgpu::BindGroup)>,
    pub sprite_animations: Vec<(Rect, crate::sprites::SpriteAnimation, (usize, usize, u32))>,
    /// The shared atlas texture, for run-time glyph uploads.
    pub atlas_texture: Option<wgpu::Texture>,
    pipeline: Pipeline,
    /// `RenderPipelines.CROSSHAIR`.
    invert_pipeline: Pipeline,
    additive_pipeline: Pipeline,
    depth_pipeline: Pipeline,
    /// `RenderPipelines.GLINT` over a GUI model (`Blend::DepthGlint`).
    depth_glint_pipeline: Pipeline,
    vignette_pipeline: Pipeline,
    end_portal_pipeline: Pipeline,
    /// The layout of `group`, which the CIT glint pipelines share.
    layout: wgpu::BindGroupLayout,
    group: wgpu::BindGroup,
    /// The pack's CIT enchantment glints, when it has any.
    cit_glint: Option<cit_glint::CitGlintPass>,
    uniform: wgpu::Buffer,
    buffer: wgpu::Buffer,
    capacity: u64,
    format: wgpu::TextureFormat,
    coverage: bool,
    atlas_view: wgpu::TextureView,
    /// The resource pack's text shader, when it has one that works.
    pack_text: Option<pack_text::PackText>,
    /// Target pixels per window pixel: above 1 for a screenshot drawn larger than the
    /// window (`Renderer::screenshot_quality`).
    pub zoom: f32,
}
impl HudRenderer {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        view: &wgpu::TextureView,
        glint_view: &wgpu::TextureView,
        glint_armor_view: &wgpu::TextureView,
        glint_sampler: &wgpu::Sampler,
        coverage: bool,
    ) -> Result<Self> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sunrise HUD"),
            source: wgpu::ShaderSource::Wgsl(include_str!("hud.wgsl").into()),
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("HUD"),
            ..Default::default()
        });
        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("HUD linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("HUD size"),
            contents: bytemuck::bytes_of(&HudUniform::new([1.0; 4], EndPortalFill::default(), 0)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HUD bindings"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HUD resources"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(glint_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(glint_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&linear_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(glint_armor_view),
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HUD layout"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let attributes =
            wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Unorm8x4,3=>Float32,4=>Float32x2];
        // `depth`: the model's depth test, and whether the run writes depth.
        let create = |label, blend, fragment_entry, depth: Option<bool>| {
            let desc = wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<HudVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &attributes,
                    })],
                },
                primitive: Default::default(),
                depth_stencil: depth.map(|write| wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(write),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment_entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            };
            let source = wgpu::ShaderSource::Wgsl(include_str!("hud.wgsl").into());
            Pipeline::new(device, &desc, source, coverage).expect("the HUD's coverage")
        };
        let pipeline = create("HUD", wgpu::BlendState::ALPHA_BLENDING, "fs_main", None);
        let depth_pipeline = create(
            "HUD model",
            wgpu::BlendState::ALPHA_BLENDING,
            "fs_main",
            Some(true),
        );
        // BlendFunction.INVERT: ONE_MINUS_DST_COLOR, ONE_MINUS_SRC_COLOR,
        // ONE, ZERO.
        let invert_pipeline = create(
            "HUD crosshair",
            wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::OneMinusDst,
                    dst_factor: wgpu::BlendFactor::OneMinusSrc,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
            },
            "fs_crosshair",
            None,
        );
        // BlendFunction.ADDITIVE: ONE, ONE.
        let add = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let additive_pipeline = create(
            "HUD text highlight",
            wgpu::BlendState {
                color: add,
                alpha: add,
            },
            "fs_main",
            None,
        );
        // RenderPipelines.GLINT tests depth EQUAL and writes none; the faces are
        // the model's own, so LEQUAL passes the same pixels.
        let depth_glint_pipeline = create(
            "HUD model glint",
            wgpu::BlendState {
                color: add,
                alpha: add,
            },
            "fs_main",
            Some(false),
        );
        // RenderPipelines.VIGNETTE: ZERO, ONE_MINUS_SRC_COLOR, ZERO, ONE.
        let vignette_pipeline = create(
            "HUD vignette",
            wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Zero,
                    dst_factor: wgpu::BlendFactor::OneMinusSrc,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Zero,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            },
            "fs_vignette",
            None,
        );
        // RenderPipelines.END_PORTAL: opaque.
        let end_portal_pipeline = create(
            "HUD end portal",
            wgpu::BlendState::REPLACE,
            "fs_end_portal",
            None,
        );
        let capacity = 1 << 16;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("HUD vertices"),
            size: capacity * std::mem::size_of::<HudVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Self {
            pictures: HashMap::new(),
            atlas_texture: None,
            sprite_animations: Vec::new(),
            pipeline,
            invert_pipeline,
            additive_pipeline,
            depth_pipeline,
            depth_glint_pipeline,
            vignette_pipeline,
            end_portal_pipeline,
            layout,
            group,
            cit_glint: None,
            uniform,
            buffer,
            capacity,
            format,
            coverage,
            atlas_view: view.clone(),
            pack_text: None,
            zoom: 1.0,
        })
    }
    /// Draw the CIT glint layers of `textures` (the pack's enchantment rules); `None` or no
    /// textures draws none.
    pub fn set_cit_glints(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        textures: Option<&cit_glint::GlintTextures>,
    ) {
        self.cit_glint = textures.and_then(|t| {
            cit_glint::CitGlintPass::new(device, queue, self.format, &self.layout, t, self.coverage)
        });
    }
    /// Draw the GUI's text with `shader` (`HudAtlas::text_shaders`' `gui`, with the Polytone
    /// expression uniforms its stages read, `shader_modifiers`), or natively.
    pub fn set_text_shader(
        &mut self,
        device: &wgpu::Device,
        shader: Option<(
            &crate::core_shader::CoreProgram,
            &[crate::polytone_post::ExpressionUniform],
        )>,
    ) {
        self.pack_text = shader.and_then(|shader| {
            pack_text::PackText::new(device, self.format, &self.atlas_view, shader, self.coverage)
        });
    }
    /// The GUI text's Polytone blocks, when a pack's text shader draws it.
    pub(crate) fn text_expression_blocks(
        &mut self,
    ) -> Option<&mut crate::gfx::expression_blocks::ExpressionBlocks> {
        self.pack_text.as_mut().map(|text| &mut text.extra)
    }
    /// Stage the batch's `patches` and `image_patches` (client mods' dynamic textures) into
    /// the shared atlas texture.
    pub fn upload_patches(
        &self,
        uploads: &mut crate::gfx::upload::Uploads,
        encoder: &mut wgpu::CommandEncoder,
        batch: &HudBatch,
    ) {
        let Some(texture) = &self.atlas_texture else {
            return;
        };
        let glyphs = batch
            .patches
            .iter()
            .map(|p| ([p.x, p.y], [p.width, p.height], &p.rgba[..]));
        let images = batch
            .image_patches
            .iter()
            .map(|p| (p.origin, p.size, &p.rgba[..]));
        for ([x, y], [width, height], rgba) in glyphs.chain(images) {
            if x.saturating_add(width) <= texture.width()
                && y.saturating_add(height) <= texture.height()
            {
                uploads.texture(
                    encoder,
                    wgpu::TexelCopyTextureInfo {
                        texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x, y, z: 0 },
                        aspect: wgpu::TextureAspect::All,
                    },
                    width,
                    height,
                    rgba,
                );
            }
        }
    }
    /// Stage the batch's atlas patches, vertices and uniform: before the
    /// frame's first pass, so the copies share one command buffer.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        uploads: &mut crate::gfx::upload::Uploads,
        encoder: &mut wgpu::CommandEncoder,
        batch: &HudBatch,
        size: [u32; 2],
    ) {
        self.prepare_pictures(device, uploads, encoder, batch);
        self.upload_patches(uploads, encoder, batch);
        if batch.vertices.is_empty() || size.contains(&0) {
            return;
        }
        let count = batch.vertices.len() as u64;
        if count > self.capacity {
            self.capacity = count.next_power_of_two();
            self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("HUD vertices"),
                size: self.capacity * std::mem::size_of::<HudVertex>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        uploads.buffer(
            encoder,
            &self.buffer,
            0,
            bytemuck::cast_slice(&batch.vertices),
        );
        uploads.buffer(
            encoder,
            &self.uniform,
            0,
            bytemuck::bytes_of(&HudUniform::new(
                [
                    size[0] as f32,
                    size[1] as f32,
                    batch.scale * self.zoom,
                    batch.glint_alpha,
                ],
                batch.end_portal,
                batch.glint_colour,
            )),
        );
        if let Some(pack_text) = &self.pack_text {
            pack_text.prepare(uploads, encoder, batch, size, batch.scale * self.zoom);
        }
    }
    /// Draw the batch `prepare` staged.
    pub fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        batch: &HudBatch,
        size: [u32; 2],
    ) {
        self.draw_range(
            encoder,
            target,
            None,
            depth,
            batch,
            size,
            0..batch.vertices.len() as u32,
            None,
        );
    }
    /// Draw part of the prepared batch, preserving its blend/depth runs.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_range(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        coverage: Option<&wgpu::TextureView>,
        depth: &wgpu::TextureView,
        batch: &HudBatch,
        size: [u32; 2],
        vertices: std::ops::Range<u32>,
        mut timing: Option<&mut crate::gfx::pack_timing::PassTiming>,
    ) {
        if vertices.is_empty() || size.contains(&0) {
            return;
        }
        let mut attachments = [None, None];
        attachments[0] = Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        });
        let attachment_count;
        if let Some(view) = coverage {
            attachments[1] = Some(hud_coverage::attachment(view));
            attachment_count = 2;
        } else {
            attachment_count = 1;
        }
        let runs = batch.runs_in(vertices);
        let mut at = 0;
        while at < runs.len() {
            let clip = match runs[at].1 {
                Blend::Depth(clip) | Blend::DepthGlint(clip) => Some(clip),
                _ => None,
            };
            let end = if let Some(clip) = clip {
                // The model's glint shares its pass and depth buffer.
                at + 1
                    + runs[at + 1..]
                        .iter()
                        .take_while(|(_, b)| *b == Blend::DepthGlint(clip))
                        .count()
            } else {
                runs[at..]
                    .iter()
                    .position(|(_, b)| matches!(b, Blend::Depth(_) | Blend::DepthGlint(_)))
                    .map_or(runs.len(), |n| at + n)
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HUD"),
                timestamp_writes: timing.as_deref_mut().and_then(|t| t.writes("HUD")),
                color_attachments: &attachments[..attachment_count],
                depth_stencil_attachment: clip.map(|_| wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_bind_group(0, &self.group, &[]);
            pass.set_vertex_buffer(0, self.buffer.slice(..));
            if let Some(clip) = clip {
                let [x, y, w, h] = clip.map(|v| (v as f32 * self.zoom) as u32);
                let (x, y) = (x.min(size[0]), y.min(size[1]));
                let (w, h) = (w.min(size[0] - x), h.min(size[1] - y));
                if w == 0 || h == 0 {
                    at = end;
                    continue;
                }
                pass.set_scissor_rect(x, y, w, h);
            }
            for (range, blend) in &runs[at..end] {
                let pipeline = match blend {
                    Blend::Picture(id) => {
                        self.draw_picture(&mut pass, *id, range.clone(), coverage.is_some());
                        continue;
                    }
                    Blend::Alpha => &self.pipeline,
                    Blend::Invert => &self.invert_pipeline,
                    Blend::Additive => &self.additive_pipeline,
                    Blend::Depth(_) => &self.depth_pipeline,
                    Blend::DepthGlint(_) => &self.depth_glint_pipeline,
                    Blend::Vignette => &self.vignette_pipeline,
                    Blend::EndPortal => &self.end_portal_pipeline,
                    Blend::CitGlint(blend) => {
                        if let Some(cit) = &self.cit_glint {
                            cit.draw(&mut pass, *blend, range.clone(), coverage.is_some());
                        }
                        continue;
                    }
                };
                let pipeline = pipeline.get(coverage.is_some());
                match &self.pack_text {
                    Some(pack_text) if *blend == Blend::Alpha => {
                        for (part, text) in pack_text::text_runs(&batch.vertices, range.clone()) {
                            if text {
                                pack_text.draw(&mut pass, part, coverage.is_some());
                                pass.set_bind_group(0, &self.group, &[]);
                            } else {
                                pass.set_pipeline(pipeline);
                                pass.draw(part, 0..1);
                            }
                        }
                    }
                    _ => {
                        pass.set_pipeline(pipeline);
                        pass.draw(range.clone(), 0..1);
                    }
                }
            }
            at = end;
        }
    }
}

#[cfg(test)]
#[path = "hud/coverage_tests.rs"]
mod coverage_tests;

#[cfg(test)]
mod tests;
