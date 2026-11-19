//! The uniforms a shader pack's post-processing programs read once per frame (spec 5.2, class
//! F), re-exported from sunrise_render_shaderpack.

pub use sunrise_render_shaderpack::pack_uniforms::*;

pub trait FrameUniformsExt {
    /// Update uniforms from the client's `Frame` and `PackInputs`.
    fn update_from_client_frame(
        &mut self,
        frame: &super::Frame<'_>,
        inputs: &PackInputs,
        size: [u32; 2],
    );
}

impl FrameUniformsExt for FrameUniforms {
    fn update_from_client_frame(
        &mut self,
        frame: &super::Frame<'_>,
        inputs: &PackInputs,
        size: [u32; 2],
    ) {
        let view = sunrise_render_core::UniformFrameView {
            camera: frame.camera,
            yaw: frame.yaw,
            pitch: frame.pitch,
            fov_degrees: frame.fov_degrees,
            eye: frame.eye,
            eye_rotation: frame.eye_rotation,
            first_person: frame.first_person,
            bob: frame.bob,
            spin: frame.spin,
            sky: sunrise_render_core::SkyView {
                sun_angle: frame.sky.sun_angle,
                moon_angle: frame.sky.moon_angle,
                sky_color: frame.sky.sky_color,
                fog_color: frame.sky.fog_color,
            },
            distance: frame.distance,
            detail_distance: frame.detail_distance,
            depth_far: frame.depth_far,
            fog: frame.fog.as_ref().map(|f| sunrise_render_core::FogView {
                color: [f.color[0], f.color[1], f.color[2]],
                environmental_start: f.environmental_start,
                environmental_end: f.environmental_end,
            }),
            night_vision: frame.night_vision,
            darkness: frame.darkness,
            dynamic_lights: &frame.dynamic_lights,
            entities: frame.entities,
        };
        self.update(&view, inputs, size);
    }
}

pub fn shadow_light(frame: &super::Frame<'_>, shadow: &sunrise_shaders::Shadow) -> glam::Vec3 {
    let view = sunrise_render_core::UniformFrameView {
        camera: frame.camera,
        yaw: frame.yaw,
        pitch: frame.pitch,
        fov_degrees: frame.fov_degrees,
        eye: frame.eye,
        eye_rotation: frame.eye_rotation,
        first_person: frame.first_person,
        bob: frame.bob,
        spin: frame.spin,
        sky: sunrise_render_core::SkyView {
            sun_angle: frame.sky.sun_angle,
            moon_angle: frame.sky.moon_angle,
            sky_color: frame.sky.sky_color,
            fog_color: frame.sky.fog_color,
        },
        distance: frame.distance,
        detail_distance: frame.detail_distance,
        depth_far: frame.depth_far,
        fog: None,
        night_vision: frame.night_vision,
        darkness: frame.darkness,
        dynamic_lights: &[],
        entities: &[],
    };
    sunrise_render_shaderpack::pack_uniforms::shadow_light(&view, shadow)
}

pub fn shadow_model_view(
    frame: &super::Frame<'_>,
    shadow: &sunrise_shaders::Shadow,
) -> (glam::Mat4, glam::Mat4) {
    let view = sunrise_render_core::UniformFrameView {
        camera: frame.camera,
        yaw: frame.yaw,
        pitch: frame.pitch,
        fov_degrees: frame.fov_degrees,
        eye: frame.eye,
        eye_rotation: frame.eye_rotation,
        first_person: frame.first_person,
        bob: frame.bob,
        spin: frame.spin,
        sky: sunrise_render_core::SkyView {
            sun_angle: frame.sky.sun_angle,
            moon_angle: frame.sky.moon_angle,
            sky_color: frame.sky.sky_color,
            fog_color: frame.sky.fog_color,
        },
        distance: frame.distance,
        detail_distance: frame.detail_distance,
        depth_far: frame.depth_far,
        fog: None,
        night_vision: frame.night_vision,
        darkness: frame.darkness,
        dynamic_lights: &[],
        entities: &[],
    };
    sunrise_render_shaderpack::pack_uniforms::shadow_model_view(&view, shadow)
}

pub fn shadow_culling(frame: &super::Frame<'_>, shadow: &sunrise_shaders::Shadow) -> glam::Mat4 {
    let view = sunrise_render_core::UniformFrameView {
        camera: frame.camera,
        yaw: frame.yaw,
        pitch: frame.pitch,
        fov_degrees: frame.fov_degrees,
        eye: frame.eye,
        eye_rotation: frame.eye_rotation,
        first_person: frame.first_person,
        bob: frame.bob,
        spin: frame.spin,
        sky: sunrise_render_core::SkyView {
            sun_angle: frame.sky.sun_angle,
            moon_angle: frame.sky.moon_angle,
            sky_color: frame.sky.sky_color,
            fog_color: frame.sky.fog_color,
        },
        distance: frame.distance,
        detail_distance: frame.detail_distance,
        depth_far: frame.depth_far,
        fog: None,
        night_vision: frame.night_vision,
        darkness: frame.darkness,
        dynamic_lights: &[],
        entities: &[],
    };
    sunrise_render_shaderpack::pack_uniforms::shadow_culling(&view, shadow)
}

#[cfg(test)]
pub(super) mod initial_history_tests {
    use crate::hud::HudBatch;

    /// A frame of the game for the tests of what a pack's uniforms are made of: the camera at
    /// (0.5, 100, 0.5) looking 10 degrees down an 85-degree field of view, 12 blocks of
    /// distance. It stayed in this crate when the uniforms moved to `sunrise-render-shaderpack`:
    /// it builds a client `Frame` from a `HudBatch`, which that crate does not have.
    pub(in crate::gfx) fn frame(hud: &HudBatch) -> super::super::Frame<'_> {
        super::super::Frame {
            eye: glam::DVec3::ZERO,
            eye_rotation: [0.0; 2],
            first_person: true,
            shadow_scene_revision: (0, 0),
            shadow_cache: false,
            water_plane: None,
            weather: &[],
            weather_time: crate::weather::Time::new(0, 0.0),
            camera: glam::DVec3::new(0.5, 100.0, 0.5),
            yaw: 0.0,
            pitch: 10.0,
            fov_degrees: 85.0,
            bob: glam::Mat4::IDENTITY,
            spin: glam::Mat4::IDENTITY,
            sky: crate::environment::Sky::default(),
            distance: 12.0,
            detail_distance: 12.0,
            depth_far: 1000.0,
            underwater: false,
            cave_culling: false,
            title: true,
            sky_blocked: false,
            skybox: false,
            end_sky: false,
            end_sky_sprite: None,
            boss_world_fog: false,
            environment_fog: None,
            water_fog: [0.0; 3],
            fog: None,
            dark_disc: false,
            outline: Vec::new(),
            outline_after_terrain: false,
            outline_high_contrast: false,
            pvp: Default::default(),
            debug_lines: &[],
            debug_lines_on_top: &[],
            debug_crosshair: &[],
            hud,
            glint_alpha: 0.75,
            glint_ticks: 0.0,
            nether_lighting: false,
            entities: &[],
            static_block_entities: &[],
            visible_static_block_entity_sections: &[],
            static_block_entity_stats: crate::block_entity_render::StaticCacheFrameStats::default(),
            entity_motion: &[],
            entity_motion_posed: &[],
            colored_lights: &[],
            emissive_layers: &[],
            shadow_only: &[],
            entity_spans: &[],
            skin: None,
            glowing: &[],
            dynamic_lights: &[],
            clouds: None,
            lightning: Default::default(),
            additive: &[],
            see_through: &[],
            world_primitives: Default::default(),
            celestial: &[],
            stars: None,
            hand: &[],
            hand_items: &[],
            hand_fov_degrees: 70.0,
            screen: &[],
            pack: Default::default(),
            border: None,
            game_time: 0.0,
            shader_game_time: 0.0,
            end_portal_sprites: [[0.0; 4]; 2],
            crumbling_sprite: [0.0; 4],
            ticks: 0,
            darkness: (0.0, 0.0),
            night_vision: 0.0,
            block_flicker: 0.0,
            boss_darken: 0.0,
            menu_blur: None,
            chunk_fade: 0.0,
            post_effects: crate::post_chain::Request::none(),
        }
    }
}
