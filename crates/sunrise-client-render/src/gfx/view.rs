//! The views a frame is drawn from. A flat screen has one; a headset has
//! one per eye. Every view shares the frame's camera position, so the
//! camera-relative world (section origins, entity offsets) is built once and
//! only each view's rotation and lens differ.

use super::{Cull, Frame, Frustum, JoinedFrusta, Renderer, frustum, taa_frustum};
#[cfg(feature = "test-support")]
use glam::Vec4;
use glam::{DVec3, Mat4, Vec3};

/// The most views one frame draws.
const MAX_VIEWS: usize = 2;

/// One eye's view of the camera-relative world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// Camera-relative world to eye: a rotation, plus the eye's offset from
    /// the camera on a headset.
    pub view: Mat4,
    /// Reversed-Z perspective: the near plane at depth 1, far at 0.
    pub projection: Mat4,
    /// Width over height of the image this view fills.
    pub aspect: f32,
    /// `tan(fov_y / 2)`, for shaders rebuilding view rays from NDC.
    pub tan_half_fov: f32,
}

impl View {
    /// The flat-screen view: vanilla's camera rotation and a symmetric
    /// vertical field of view.
    pub fn screen(yaw: f32, pitch: f32, fov_degrees: f32, aspect: f32, far: f32) -> Self {
        Self {
            view: crate::camera::view_matrix(yaw, pitch),
            projection: super::world_projection(fov_degrees, aspect, far),
            aspect,
            tan_half_fov: (fov_degrees.to_radians() / 2.0).tan(),
        }
    }

    /// Clip space from the camera-relative world, with `pose` (view bobbing,
    /// nausea spin) between the lens and the rotation.
    pub fn view_proj(&self, pose: Mat4) -> Mat4 {
        self.projection * pose * self.view
    }
}

/// Every view of a frame, and the one geometry is culled with.
#[derive(Clone, Copy, Debug)]
pub struct ViewSet {
    views: [View; MAX_VIEWS],
    len: usize,
    cull: View,
}

impl ViewSet {
    /// A frame drawn from `view` alone, culled with it.
    pub fn single(view: View) -> Self {
        Self {
            views: [view; MAX_VIEWS],
            len: 1,
            cull: view,
        }
    }

    /// The view the HUD, hand and screen-space effects follow.
    pub fn primary(&self) -> &View {
        &self.views[0]
    }

    pub fn views(&self) -> &[View] {
        &self.views[..self.len]
    }

    /// The view whose frustum covers every other view's.
    pub fn cull(&self) -> &View {
        &self.cull
    }
}

/// Collects canonical primary-camera visible sections (including compiled empty air sections
/// tracked in `detail_columns` without a `SectionGpu` mesh) into reusable `out` and `scratch`
/// storage, ordered front-to-back by distance then section key.
#[allow(clippy::too_many_arguments)]
pub(super) fn collect_primary_renderable_sections_into<T>(
    detail_columns: &crate::fast_hash::FxHashMap<(i32, i32), crate::section_mask::SectionMask>,
    sections: &super::resident::Resident<T>,
    is_empty: impl Fn(&T) -> bool,
    reachable: impl Fn(super::SectionKey) -> bool,
    lod_cutout: &crate::fast_hash::FxHashSet<(i32, i32)>,
    lod_leaving: &[(super::SectionKey, std::time::Instant)],
    lod_leaving_columns: &crate::fast_hash::FxHashSet<(i32, i32)>,
    camera: DVec3,
    radius: i32,
    frustum: &Frustum,
    cave_culling: bool,
    scratch: &mut Vec<(u64, super::SectionKey, bool)>,
    out: &mut Vec<mod_api::RenderableSection>,
) {
    out.clear();
    scratch.clear();
    let min_y = sections.min_y();
    let from = super::resident::camera_column(camera);
    let beyond = |x: i32, z: i32| {
        (!lod_cutout.is_empty() && lod_cutout.contains(&(x, z)))
            || (!lod_leaving_columns.is_empty() && lod_leaving_columns.contains(&(x, z)))
    };
    let mut emit_section = |cx: i32, sy: i32, cz: i32, near: bool, inside: bool| {
        let key = (cx, sy, cz);
        if near && cave_culling && !reachable(key) {
            return;
        }
        let ox = f64::from(cx * 16) - camera.x;
        let oy = f64::from(min_y + sy * 16) - camera.y;
        let oz = f64::from(cz * 16) - camera.z;
        let min = Vec3::new(ox as f32, oy as f32, oz as f32);
        let resident = sections.find_with_bounds(key);
        if !inside {
            let bounds = resident.and_then(|(_, b)| b).filter(|_| frustum.tight());
            let (bmin, bmax) = super::resident::section_box(min, bounds);
            if !frustum.intersects(bmin, bmax) {
                return;
            }
        }
        let dx8 = ox + 8.0;
        let dy8 = oy + 8.0;
        let dz8 = oz + 8.0;
        let distance = (dx8 * dx8 + dy8 * dy8 + dz8 * dz8).to_bits();
        let empty = resident.is_none_or(|(gpu, _)| is_empty(gpu));
        scratch.push((distance, key, empty));
    };
    for (&(cx, cz), &mask) in detail_columns {
        if mask.is_empty() {
            continue;
        }
        let near = crate::streaming::in_view_distance(from, radius, cx, cz);
        if !near && !beyond(cx, cz) {
            continue;
        }
        let mut it = mask.iter();
        let Some(low) = it.next() else { continue };
        let high = it.last().unwrap_or(low);
        let ox = f64::from(cx * 16) - camera.x;
        let oz = f64::from(cz * 16) - camera.z;
        let min_x = ox as f32;
        let min_z = oz as f32;
        let low_oy = (f64::from(min_y + low as i32 * 16) - camera.y) as f32;
        let high_oy = (f64::from(min_y + high as i32 * 16) - camera.y) as f32;
        let cmin = Vec3::new(min_x, low_oy, min_z);
        let cmax = Vec3::new(min_x + 16.0, high_oy + 16.0, min_z + 16.0);
        let inside = frustum.contains(cmin, cmax);
        if !inside && !frustum.intersects(cmin, cmax) {
            continue;
        }
        for y in mask.iter() {
            emit_section(cx, y as i32, cz, near, inside);
        }
    }
    if !lod_leaving_columns.is_empty() {
        for &(key, _) in lod_leaving {
            let (cx, sy, cz) = key;
            if detail_columns
                .get(&(cx, cz))
                .is_some_and(|m| m.contains(sy as usize))
            {
                continue;
            }
            let near = crate::streaming::in_view_distance(from, radius, cx, cz);
            emit_section(cx, sy, cz, near, false);
        }
    }
    scratch.sort_unstable_by_key(|&(distance, key, _)| (distance, key));
    out.reserve(scratch.len());
    for &(_, (cx, sy, cz), empty) in scratch.iter() {
        let origin = [cx * 16, min_y + sy * 16, cz * 16];
        let [ox, oy, oz] = origin.map(f64::from);
        out.push(mod_api::RenderableSection::new(
            origin,
            [[ox, oy, oz], [ox + 16.0, oy + 16.0, oz + 16.0]],
            empty,
        ));
    }
}

impl Renderer {
    /// Whether order-independent transparency is active for the given option value on this
    /// renderer (no shader pack post-chain, no native pack, and device supports OIT).
    pub fn oit_active_for(&self, improved_transparency: bool) -> bool {
        improved_transparency
            && self.pack_post.as_ref().is_none_or(|p| p.1.is_none())
            && self.native.is_none()
            && super::oit::supported(&self.device)
    }

    /// Populate `self.visible_renderable_sections` with the primary camera's canonical visible
    /// sections (including compiled air sections), reusing backing storage across frames.
    pub(super) fn collect_primary_renderable_sections(
        &mut self,
        camera: DVec3,
        radius: i32,
        frustum: &Frustum,
        cave_culling: bool,
    ) {
        let occlusion = &self.occlusion;
        collect_primary_renderable_sections_into(
            &self.detail_columns,
            &self.sections,
            |gpu| gpu.solid == 0 && gpu.translucent == 0,
            |k| occlusion.contains(k),
            &self.lod_cutout,
            &self.lod_leaving,
            &self.lod_leaving_scratch,
            camera,
            radius,
            frustum,
            cave_culling,
            &mut self.visible_renderable_scratch,
            &mut self.visible_renderable_sections,
        );
    }

    /// The views `render` draws `frame` from.
    pub fn views(&self, frame: &Frame) -> ViewSet {
        ViewSet::single(self.screen_view(
            frame.fov_degrees,
            frame.yaw,
            frame.pitch,
            frame.depth_far,
        ))
    }
    /// The frusta `render` will cull with from `camera`, for culling geometry built before the
    /// frame (entities, block entities) the same way: the camera's and this frame's world
    /// views', which it asks the mods for (once a frame, `view_sections`).
    pub fn camera_frustum(
        &mut self,
        camera: DVec3,
        fov_degrees: f32,
        yaw: f32,
        pitch: f32,
        bob: Mat4,
        depth_far: f32,
    ) -> JoinedFrusta {
        let views = ViewSet::single(self.screen_view(fov_degrees, yaw, pitch, depth_far));
        let planes = frustum(views.cull().view_proj(bob));
        self.ask_world_views(camera, depth_far, Frustum::from_planes(planes))
    }
    /// A conservative view for geometry built before TAA chooses this frame's
    /// jitter. Cover a whole rendered pixel on each side, at the configured
    /// resolution scale/upscale, rather than borrowing the previous frame's size.
    /// With `camera_frustum`'s world views.
    pub fn camera_frustum_with_jitter(
        &self,
        fov_degrees: f32,
        yaw: f32,
        pitch: f32,
        bob: Mat4,
        depth_far: f32,
    ) -> JoinedFrusta {
        let views = ViewSet::single(self.screen_view(fov_degrees, yaw, pitch, depth_far));
        let size = self
            .world_post
            .taa_render_size([self.config.width, self.config.height]);
        self.asked_world_views(taa_frustum(views.cull().view_proj(bob), size))
    }
    fn screen_view(&self, fov_degrees: f32, yaw: f32, pitch: f32, far: f32) -> View {
        let aspect = self.config.width as f32 / self.config.height.max(1) as f32;
        View::screen(yaw, pitch, fov_degrees, aspect, far)
    }
}

/// Where a perspective `view_proj` sees from, in its own (camera-relative)
/// space: the point whose clip x, y and w are all 0. `None` for a
/// projection without one (orthographic).
pub(super) fn projection_center(view_proj: Mat4) -> Option<Vec3> {
    let m = view_proj.as_dmat4();
    let rows = [m.row(0), m.row(1), m.row(3)];
    let a = glam::DMat3::from_cols(
        DVec3::new(rows[0].x, rows[1].x, rows[2].x),
        DVec3::new(rows[0].y, rows[1].y, rows[2].y),
        DVec3::new(rows[0].z, rows[1].z, rows[2].z),
    );
    let det = a.determinant();
    if !det.is_normal() {
        return None;
    }
    let b = DVec3::new(rows[0].w, rows[1].w, rows[2].w);
    Some((a.inverse() * -b).as_vec3())
}

/// `view_proj` mirrored about the horizontal plane at the camera-relative height `plane`:
/// with F the mirror (y flips and moves to 2 * plane), `view_proj * F` negates column 1
/// and moves column 3 by 2 * plane times column 1.
///
/// `mirror_inverse_view` is the inverse's other side: F * `inverse_view`.
pub(super) fn mirror_view_projection(view_proj: Mat4, plane: f32) -> Mat4 {
    let mut m = view_proj.to_cols_array_2d();
    let col1 = m[1];
    m[1] = col1.map(|v| -v);
    m[3] = std::array::from_fn(|i| m[3][i] + 2.0 * plane * col1[i]);
    Mat4::from_cols_array_2d(&m)
}

/// The mirror's `inverse_view` (the sky's rays): what the view sees, mirrored about the
/// horizontal plane at the camera-relative height `plane`, so F * `inverse_view`.
pub(super) fn mirror_inverse_view(inverse_view: [[f32; 4]; 4], plane: f32) -> [[f32; 4]; 4] {
    inverse_view.map(|[x, y, z, w]| [x, 2.0 * plane * w - y, z, w])
}

#[cfg(feature = "test-support")]
pub fn test_frustum(planes: [Vec4; 6]) -> JoinedFrusta {
    JoinedFrusta::new(super::Frustum::from_planes(planes), [])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The formulas `Renderer` used before views existed, kept to pin the
    /// screen view to them bit for bit.
    fn legacy_view_proj(fov: f32, yaw: f32, pitch: f32, pose: Mat4, aspect: f32, far: f32) -> Mat4 {
        super::super::world_projection(fov, aspect, far)
            * pose
            * crate::camera::view_matrix(yaw, pitch)
    }

    fn poses() -> [Mat4; 3] {
        [
            Mat4::IDENTITY,
            Mat4::from_translation(glam::Vec3::new(0.013, -0.027, 0.0))
                * Mat4::from_rotation_z(0.021),
            Mat4::from_rotation_y(0.4) * Mat4::from_scale(glam::Vec3::new(1.1, 0.9, 1.0)),
        ]
    }

    #[test]
    fn the_screen_view_matches_the_legacy_projection_bit_for_bit() {
        for (fov, yaw, pitch, aspect, far) in [
            (70.0, 0.0, 0.0, 16.0 / 9.0, 512.0),
            (110.0, -137.25, 61.5, 21.0 / 9.0, 8192.0),
            (30.0, 359.9, -90.0, 4.0 / 3.0, 96.0),
            (82.333, 12.5, 7.75, 1.0, 1024.0),
        ] {
            let view = View::screen(yaw, pitch, fov, aspect, far);
            for pose in poses() {
                let old = legacy_view_proj(fov, yaw, pitch, pose, aspect, far);
                let new = view.view_proj(pose);
                assert_eq!(
                    old.to_cols_array().map(f32::to_bits),
                    new.to_cols_array().map(f32::to_bits),
                    "fov {fov} yaw {yaw} pitch {pitch}"
                );
            }
            assert_eq!(view.view, crate::camera::view_matrix(yaw, pitch));
            assert_eq!(
                view.tan_half_fov.to_bits(),
                (fov.to_radians() / 2.0).tan().to_bits()
            );
            assert_eq!(view.aspect.to_bits(), aspect.to_bits());
        }
    }

    #[test]
    fn a_single_view_set_draws_and_culls_with_its_view() {
        let view = View::screen(45.0, 10.0, 70.0, 16.0 / 9.0, 512.0);
        let set = ViewSet::single(view);
        assert_eq!(set.views(), &[view]);
        assert_eq!(*set.primary(), view);
        assert_eq!(*set.cull(), view);
    }

    #[test]
    fn primary_renderable_sections_include_compiled_air_and_apply_canonical_culling() {
        use crate::fast_hash::{FxHashMap, FxHashSet};
        use crate::section_mask::SectionMask;

        let mut detail_columns: FxHashMap<(i32, i32), SectionMask> = FxHashMap::default();
        // Camera sits at (8, 8, 8) looking toward -Z (yaw 180, pitch 0).
        // min_y = 0 so section index 0 covers y = [0, 16).
        // (0, 0, -1): compiled air section in front of camera -> included with empty = true.
        // (0, 0, -2): non-empty resident section in front -> included with empty = false.
        // (0, 0, 2): behind the primary camera -> excluded by primary frustum.
        // (0, 0, -3): in front, within detail radius, but unreachable under cave culling -> excluded.
        // (0, 0, -6): past detail radius (radius = 3), not in lod_cutout -> excluded.
        // (0, 0, -7): past detail radius, in lod_cutout -> included.
        let mut mask = SectionMask::EMPTY;
        mask.insert(0);
        detail_columns.insert((0, -1), mask);
        detail_columns.insert((0, -2), mask);
        detail_columns.insert((0, 2), mask);
        detail_columns.insert((0, -3), mask);
        detail_columns.insert((0, -6), mask);
        detail_columns.insert((0, -7), mask);

        let mut sections = super::super::resident::Resident::<bool>::default();
        sections.set_min_y(0);
        // Only (0, 0, -2) and (0, 0, -7) have non-empty meshes (`false` means non-empty here).
        sections.insert((0, 0, -2), false);
        sections.insert((0, 0, 2), false);
        sections.insert((0, 0, -3), false);
        sections.insert((0, 0, -6), false);
        sections.insert((0, 0, -7), false);

        let mut lod_cutout: FxHashSet<(i32, i32)> = FxHashSet::default();
        lod_cutout.insert((0, -7));
        let lod_leaving: Vec<(super::super::SectionKey, std::time::Instant)> = Vec::new();
        let lod_leaving_columns: FxHashSet<(i32, i32)> = FxHashSet::default();

        let camera = DVec3::new(8.0, 8.0, 8.0);
        let view = View::screen(180.0, 0.0, 70.0, 16.0 / 9.0, 512.0);
        let frustum = super::super::Frustum::from_planes(super::super::frustum(
            view.view_proj(Mat4::IDENTITY),
        ));

        let mut scratch = Vec::new();
        let mut out = Vec::new();
        collect_primary_renderable_sections_into(
            &detail_columns,
            &sections,
            |&empty| empty,
            |key| key != (0, 0, -3),
            &lod_cutout,
            &lod_leaving,
            &lod_leaving_columns,
            camera,
            3,
            &frustum,
            true,
            &mut scratch,
            &mut out,
        );

        let origins_and_empty: Vec<([i32; 3], bool)> =
            out.iter().map(|s| (s.origin, s.empty)).collect();
        assert_eq!(
            origins_and_empty,
            vec![
                ([0, 0, -16], true),
                ([0, 0, -32], false),
                ([0, 0, -112], false),
            ],
            "canonical primary walk keeps compiled air, excludes behind/cave-culled/out-of-range, and honours lod_cutout"
        );
        assert_eq!(
            out[0].bounds,
            [[0.0, 0.0, -16.0], [16.0, 16.0, 0.0]],
            "compiled air section reports full 16x16x16 world bounds"
        );
    }
}
