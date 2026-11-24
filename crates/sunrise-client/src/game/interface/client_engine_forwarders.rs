//! Single-call forwarders from server-side `Game` methods to client-owned engines.
use super::Game;
use sunrise_client::particles::{ExplosionParticle, ParticleOptions, TrackingBody, TrackingTarget};

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn client_audio_play(
        &mut self,
        event: &str,
        at: Option<[f64; 3]>,
        volume: f32,
        pitch: f32,
    ) {
        self.audio.0.play(event, at, volume, pitch);
    }

    #[inline(always)]
    pub(in crate::game) fn client_audio_play_bound(
        &mut self,
        event: &str,
        source: usize,
        volume: f32,
        pitch: f32,
        entity: sunrise_client::sound_engine::SoundEntity,
        except_entity: Option<u32>,
    ) {
        self.audio
            .0
            .play_bound(event, source, volume, pitch, entity, except_entity);
    }

    #[inline(always)]
    pub(in crate::game) fn client_audio_play_on(
        &mut self,
        source: usize,
        event: &str,
        at: Option<[f64; 3]>,
        volume: f32,
        pitch: f32,
    ) {
        self.audio.0.play_on(source, event, at, volume, pitch);
    }

    #[inline(always)]
    pub(in crate::game) fn client_audio_play_unshared(
        &mut self,
        event: &str,
        at: Option<[f64; 3]>,
        volume: f32,
        pitch: f32,
    ) {
        self.audio.0.play_unshared(event, at, volume, pitch);
    }

    #[inline(always)]
    pub(in crate::game) fn client_audio_set_world(&mut self, world: &str) {
        self.audio.0.set_world(world);
    }

    #[inline(always)]
    pub(in crate::game) fn client_audio_stop_sounds(
        &mut self,
        event: Option<&str>,
        source: Option<usize>,
    ) {
        self.audio.0.stop_sounds(event, source);
    }

    #[inline(always)]
    pub(in crate::game) fn client_particles_add(
        &mut self,
        options: &ParticleOptions,
        p: [f64; 3],
        v: [f64; 3],
    ) {
        self.particles.0.add(options, p, v);
    }

    #[inline(always)]
    pub(in crate::game) fn client_particles_add_with(
        &mut self,
        options: &ParticleOptions,
        p: [f64; 3],
        v: [f64; 3],
        override_limiter: bool,
        always_show: bool,
    ) {
        self.particles
            .0
            .add_with(options, p, v, override_limiter, always_show);
    }

    #[inline(always)]
    pub(in crate::game) fn client_particles_elder_guardian(&mut self) {
        self.particles.0.elder_guardian();
    }

    #[inline(always)]
    pub(in crate::game) fn client_particles_explosion(
        &mut self,
        center: [f64; 3],
        emitter: &ParticleOptions,
        block_count: usize,
        radius: f32,
        particles: &[ExplosionParticle],
    ) {
        self.particles
            .0
            .explosion(center, emitter, block_count, radius, particles);
    }

    #[inline(always)]
    pub(in crate::game) fn client_particles_llama_spit_burst(
        &mut self,
        at: [f64; 3],
        movement: [f64; 3],
    ) {
        self.particles.0.llama_spit_burst(at, movement);
    }

    #[inline(always)]
    pub(in crate::game) fn client_particles_send_particles(
        &mut self,
        options: &ParticleOptions,
        at: [f64; 3],
        spread: [f64; 3],
        speed: f64,
        count: u32,
    ) {
        self.particles
            .0
            .send_particles(options, at, spread, speed, count);
    }

    #[inline(always)]
    pub(in crate::game) fn client_particles_tracking_emitter(
        &mut self,
        options: ParticleOptions,
        target: TrackingTarget,
        body: TrackingBody,
    ) {
        self.particles.0.tracking_emitter(options, target, body);
    }

    #[inline(always)]
    pub(in crate::game) fn client_recipe_ui_reset_screen(&mut self) {
        self.recipe_ui.0.reset_screen();
    }
}
