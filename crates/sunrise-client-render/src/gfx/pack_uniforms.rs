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
