use super::{Dim, Game};
use crate::game::server_side::end::CrystalDrawFrame;
#[cfg(test)]
use crate::game::server_side::end::*;
use sunrise_client::{
    environment::Environment,
    gfx::CelestialBody,
    items::ItemStack,
    mesher::Vertex,
    particles::{ParticleOptions, ParticleType},
};
use sunrise_core::random::RandomSource;

/// SkyRenderer.renderEndFlash's quad: the end_flash `sprite` at height 100, scale 60, turned by
/// `pose`, at `intensity`; none when the flash is out or has no sprite. Each quad comes with what
/// it is (`Frame::celestial_bodies`).
pub(in crate::game) fn end_flash_quad(
    sprite: Option<sunrise_client::hud::Rect>,
    intensity: f32,
    pose: glam::Mat4,
) -> (Vec<Vertex>, Vec<CelestialBody>) {
    let mut out = Vec::new();
    if intensity > 1e-5
        && let Some(r) = sprite
    {
        let level = (intensity.clamp(0.0, 1.0) * 255.0) as u32;
        let color = level << 24 | level << 16 | level << 8 | level;
        let corners = [
            (-1.0, -1.0, [r.u0, r.v0]),
            (1.0, -1.0, [r.u1, r.v0]),
            (1.0, 1.0, [r.u1, r.v1]),
            (-1.0, 1.0, [r.u0, r.v1]),
        ];
        for (x, z, uv) in corners {
            let p = pose.transform_point3(glam::Vec3::new(x * 60.0, 100.0, z * 60.0));
            out.push(Vertex {
                normal: [0; 4],
                pos: p.to_array(),
                uv,
                layer: 2,
                color,
                light: 0,

                glint_uv: [0.0, 0.0],
            });
        }
    }
    let bodies = vec![CelestialBody::EndFlash; out.len() / 4];
    (out, bodies)
}

impl Game<'_, crate::game::client_side::ClientState> {
    /// The environment attributes the frame draws: the packs' Polytone layers over the
    /// current dimension's, when a pack has some. Gameplay keeps `dim_environment`. A client of a
    /// server draws the type its server sent, with the packs' layers over it where they have a
    /// stack of that type.
    pub(in crate::game) fn view_environment(&self) -> &Environment {
        if let Some(own) = self.remote.as_ref().and_then(|r| {
            r.level_pack_environment
                .as_deref()
                .or(r.level_environment.as_deref())
        }) {
            return own;
        }
        match (&self.side.client_pack.environments, self.dim) {
            (Some(e), Dim::Overworld) => &e[0],
            (Some(e), Dim::Nether) => &e[1],
            (Some(e), Dim::End) => &e[2],
            (None, _) | (_, Dim::Extra(_)) => self.dim_environment(),
        }
    }

    /// SkyRenderer.renderEndFlash, drawn after the renderer's resident sky mesh: its quad, if
    /// any, with what it is.
    pub(in crate::game) fn end_flash_geometry(&self) -> (Vec<Vertex>, Vec<CelestialBody>) {
        // The old combined sky/flash draw also skipped the flash without a sky sprite.
        if !self
            .side
            .gui
            .atlas
            .sprites
            .contains_key("environment/end_sky")
        {
            return (Vec::new(), Vec::new());
        }
        let intensity = self.end.flash.intensity(self.clock.fraction() as f32);
        let sprite = self
            .side
            .gui
            .atlas
            .sprites
            .get("environment/celestial/end_flash");
        end_flash_quad(sprite.copied(), intensity, self.end.flash.pose())
    }

    /// Iris's `endFlashIntensity` and the flash's direction (camera-relative, length 100: the
    /// sprite's centre), for shader packs; no flash outside the End.
    pub(in crate::game) fn end_flash_for_pack(&self, partial: f32) -> (f32, glam::Vec3) {
        if self.dimension_type().skybox != sunrise_world::dimension_type::Skybox::End {
            return (0.0, glam::Vec3::ZERO);
        }
        let flash = &self.end.flash;
        (
            flash.intensity(partial).clamp(0.0, 1.0),
            flash
                .pose()
                .transform_point3(glam::Vec3::new(0.0, 100.0, 0.0)),
        )
    }

    /// `EndCrystalRenderer.submit`: the crystal and its beam to the respawn target.
    pub(in crate::game) fn end_crystal_geometry(
        &self,
        crystal: &super::dragon::Crystal,
        frame: &CrystalDrawFrame<'_>,
        camera: glam::DVec3,
        out: &mut Vec<Vertex>,
    ) {
        if self.hides_as_camera(crystal.id) {
            return;
        }
        let feet = glam::DVec3::new(crystal.position.x, crystal.position.y, crystal.position.z);
        let rel = feet - camera;
        if rel.length_squared() > 256.0 * 256.0 {
            return;
        }
        let age = crystal.time as f32 + frame.partial;
        let light = self.world.light(
            feet.x.floor() as i32,
            feet.y.floor() as i32,
            feet.z.floor() as i32,
        );
        let packed = (u32::from(light >> 4) * 16) | ((u32::from(light & 15) * 16) << 8);
        let poses = sunrise_client::end_crystal::poses(frame.model, age, crystal.show_bottom);
        // poseStack.scale(2) then translate(0, -0.5, 0), not flipped.
        let base = glam::Mat4::from_translation(rel.as_vec3())
            * glam::Mat4::from_scale(glam::Vec3::splat(2.0))
            * glam::Mat4::from_translation(glam::Vec3::new(0.0, -0.5, 0.0));
        let start = out.len();
        self.mark_entity(start, "minecraft:end_crystal");
        self.mark_motion(start, 0);
        sunrise_client::mobs::render::draw(
            frame.model,
            base,
            &poses,
            frame.uv,
            0xFFFF_FFFF,
            packed,
            0,
            out,
        );
        // HDR output: the crystal shines; its tier's window leaves the dark base alone.
        for v in &mut out[start..] {
            v.layer |= u32::from(sunrise_client::mesher::BRIGHT) << 16;
        }
        #[cfg(test)]
        super::native_render_pair_tests::witness_body(crystal.id, start, out.len());
        if let Some(target) = crystal.beam {
            // The beam from the target's centre up to the bobbing crystal.
            let from = glam::DVec3::new(
                f64::from(target[0]) + 0.5,
                f64::from(target[1]) + 0.5,
                f64::from(target[2]) + 0.5,
            );
            let delta = feet - from
                + glam::DVec3::new(0.0, f64::from(sunrise_client::end_crystal::y(age)), 0.0);
            self.crystal_beam(from, delta, age, camera, out);
        }
    }

    /// ThrownItemRenderer: the ender eye item facing the camera.
    pub(in crate::game) fn eye_geometry(&self, camera: glam::DVec3, out: &mut Vec<Vertex>) {
        let Some(r) = self.side.gui.atlas.sprites.get("item/ender_eye").copied() else {
            return;
        };
        let alpha = self.clock.fraction();
        let (yaw, pitch) = (self.yaw.to_radians(), self.pitch.to_radians());
        let right = glam::Vec3::new(-yaw.cos(), 0.0, -yaw.sin()) * 0.25;
        let up = glam::Vec3::new(
            -yaw.sin() * pitch.sin(),
            pitch.cos(),
            yaw.cos() * pitch.sin(),
        ) * 0.25;
        for eye in &self.end.eyes {
            if self.hides_as_camera(eye.id) {
                continue;
            }
            let p = eye.previous + (eye.position - eye.previous) * alpha - camera;
            out.extend(crate::game::end::eye_quad(p.as_vec3(), right, up, r));
        }
    }

    pub(in crate::game::client_side) fn begin_crystal_geometry(
        &self,
    ) -> Option<CrystalDrawFrame<'_>> {
        if self.crystals.is_empty() {
            return None;
        }
        let model = self.side.crystal_model.as_ref()?;
        let r = self
            .side
            .gui
            .atlas
            .sprites
            .get("entity/end_crystal/end_crystal")?;
        Some(CrystalDrawFrame {
            model,
            uv: [r.u0, r.v0, r.u1, r.v1],
            partial: self.clock.fraction() as f32,
        })
    }
    /// LightmapRenderStateExtractor: End flashes add to the sky light factor
    /// unless Hide Lightning Flashes is on (the sky sprite is not gated), a
    /// third of it under a boss bar's world fog.
    pub(in crate::game::client_side) fn end_flash_sky(
        &self,
        sky: &mut sunrise_client::environment::Sky,
        partial: f64,
    ) {
        if self.dimension_type().skybox == sunrise_world::dimension_type::Skybox::End
            && !self.side.settings.file.bool("hideLightningFlashes")
        {
            let intensity = self.end.flash.intensity(partial as f32);
            sky.sky_light_factor += if self.side.boss_overlay.should_create_world_fog() {
                intensity / 3.0
            } else {
                intensity
            };
        }
    }
    /// `LevelEventHandler` 2003: a shattered eye's item crumbs and two rings
    /// of portal particles drawn in towards it.
    pub(in crate::game) fn eye_break_particles(&mut self, pos: [i32; 3]) {
        let (x, y, z) = (
            f64::from(pos[0]) + 0.5,
            f64::from(pos[1]),
            f64::from(pos[2]) + 0.5,
        );
        let crumbs = self.item_particle(&ItemStack::new("minecraft:ender_eye", 1));
        for _ in 0..8 {
            let r = self.side.particles.0.random();
            let v = [
                r.next_gaussian() * 0.15,
                r.next_double() * 0.2,
                r.next_gaussian() * 0.15,
            ];
            self.side.particles.0.add(&crumbs, [x, y, z], v);
        }
        let portal = ParticleOptions::simple(ParticleType::Portal);
        let mut angle = 0.0f64;
        while angle < std::f64::consts::PI * 2.0 {
            let (c, s) = (angle.cos(), angle.sin());
            let at = [x + c * 5.0, y - 0.4, z + s * 5.0];
            self.side
                .particles
                .0
                .add(&portal, at, [c * -5.0, 0.0, s * -5.0]);
            self.side
                .particles
                .0
                .add(&portal, at, [c * -7.0, 0.0, s * -7.0]);
            angle += std::f64::consts::PI / 20.0;
        }
    }
    /// `EyeOfEnder.tick` on a client: behind each eye (a quarter of its last
    /// move back), four bubbles in water, else a portal particle.
    pub(in crate::game::client_side) fn eye_particles(&mut self) {
        for i in 0..self.end.eyes.len() {
            let e = &self.end.eyes[i];
            let movement = (e.position - e.previous).to_array();
            let origin = (e.position - (e.position - e.previous) * 0.25).to_array();
            let block = origin.map(|v| v.floor() as i32);
            let water = self
                .res
                .facts()
                .get(self.world.block(block[0], block[1], block[2]))
                .fluid
                == sunrise_client::blocks::Fluid::Water;
            if water {
                let bubble = ParticleOptions::simple(ParticleType::Bubble);
                for _ in 0..4 {
                    self.side.particles.0.add(&bubble, origin, movement);
                }
            } else {
                let r = self.side.particles.0.random();
                let x = origin[0] + r.next_double() * 0.6 - 0.3;
                let z = origin[2] + r.next_double() * 0.6 - 0.3;
                let portal = ParticleOptions::simple(ParticleType::Portal);
                self.side
                    .particles
                    .0
                    .add(&portal, [x, origin[1] - 0.5, z], movement);
            }
        }
    }
    /// `ClientLevel.tick`'s End flash, on every client in the End:
    /// `EndFlashState.tick`, and when a flash starts (not under the credits)
    /// its `DirectionalSoundInstance`, played 30 ticks later from the camera
    /// toward the flash.
    pub(in crate::game::client_side) fn tick_end_flash(&mut self) {
        if self.dimension_type().skybox != sunrise_world::dimension_type::Skybox::End {
            return;
        }
        // getDefaultClockTime: the End's own clock (the client keeps the
        // dimension's default clock in day_time), not the game time.
        self.end.flash.tick(self.world.day_time);
        if self.end.flash.started_this_tick() && self.side.win.is_none() {
            let flash = &self.end.flash;
            let instance = sunrise_client::sound_engine::SoundInstance::directional(
                "minecraft:weather.end_flash",
                sunrise_client_core::sound_source::WEATHER,
                sunrise_client::random_source::create(),
                self.side.audio.0.listener,
                flash.x_angle,
                flash.y_angle,
            );
            self.side.audio.0.play_instance_after(instance, 30);
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::*;

    /// The End's flash quad says it is the End's flash, and there is no entry without the quad.
    #[test]
    fn the_end_flash_quad_is_the_end_flash() {
        let sprite = sunrise_client::hud::Rect {
            u0: 0.25,
            v0: 0.0,
            u1: 0.375,
            v1: 0.125,
            w: 16,
            h: 16,
        };
        let pose = flash_pose(-60.0, 30.0);
        let (quad, bodies) = end_flash_quad(Some(sprite), 1.0, pose);
        assert_eq!((quad.len(), bodies), (4, vec![CelestialBody::EndFlash]));
        for (sprite, intensity) in [(Some(sprite), 0.0), (None, 1.0)] {
            let (quad, bodies) = end_flash_quad(sprite, intensity, pose);
            assert!(quad.is_empty() && bodies.is_empty());
        }
    }
}
