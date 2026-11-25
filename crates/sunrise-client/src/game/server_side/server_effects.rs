//! Server-owned sound queue, capture lifecycle, and portable effect helpers.
use super::Game;
use sunrise_client_core::sound_source::ServerSound;

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn start_audio_capture(&mut self) {
        assert!(
            self.is_logical_server(),
            "start_audio_capture called on Client role"
        );
        self.server.audio_capture_active = true;
        self.server.pending_sounds.clear();
    }

    #[inline(always)]
    pub(in crate::game) fn stop_audio_capture(&mut self) {
        self.server.audio_capture_active = false;
        self.server.pending_sounds.clear();
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub(in crate::game) fn audio_capture_active(&self) -> bool {
        self.server.audio_capture_active
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub(in crate::game) fn audio_capture_len(&self) -> usize {
        self.server.pending_sounds.len()
    }

    pub(in crate::game) fn server_play_sound(
        &mut self,
        event: &str,
        at: [f64; 3],
        volume: f32,
        pitch: f32,
    ) {
        assert!(
            self.is_logical_server(),
            "server_play_sound called on Client role"
        );
        if self.server.audio_capture_active {
            self.server
                .pending_sounds
                .push(ServerSound::new(event, at, volume, pitch));
        }
    }

    pub(in crate::game) fn server_play_sound_on(
        &mut self,
        source: usize,
        event: &str,
        at: [f64; 3],
        volume: f32,
        pitch: f32,
    ) {
        assert!(
            self.is_logical_server(),
            "server_play_sound_on called on Client role"
        );
        if self.server.audio_capture_active {
            self.server.pending_sounds.push(ServerSound {
                event: event.to_owned(),
                source,
                position: at,
                volume,
                pitch,
                seed: None,
                entity: None,
                except_entity: None,
            });
        }
    }

    pub(in crate::game) fn server_play_sound_seeded(
        &mut self,
        event: &str,
        at: [f64; 3],
        volume: f32,
        pitch: f32,
        seed: i64,
    ) {
        assert!(
            self.is_logical_server(),
            "server_play_sound_seeded called on Client role"
        );
        if self.server.audio_capture_active {
            self.server.pending_sounds.push(ServerSound {
                seed: Some(seed),
                ..ServerSound::new(event, at, volume, pitch)
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::game) fn server_play_bound_sound(
        &mut self,
        event: &str,
        source: usize,
        at: [f64; 3],
        volume: f32,
        pitch: f32,
        emitter: u32,
        silent: bool,
        except: Option<u32>,
    ) {
        assert!(
            self.is_logical_server(),
            "server_play_bound_sound called on Client role"
        );
        if silent {
            return;
        }
        if self.server.audio_capture_active {
            self.server.pending_sounds.push(ServerSound {
                event: event.to_owned(),
                source,
                position: at,
                volume,
                pitch,
                seed: None,
                entity: Some(emitter),
                except_entity: except,
            });
        }
    }

    /// `Level.playSound(except, mob, ...)`: the category and actor exclusion
    /// belong to the call site, not to the event's name.
    pub(in crate::game) fn play_mob_bound_sound(
        &mut self,
        index: usize,
        event: &str,
        source: usize,
        volume: f32,
        pitch: f32,
        except_actor: bool,
    ) {
        let mob = &self.mobs[index];
        let (at, id, silent) = (
            [mob.position.x, mob.position.y, mob.position.z],
            mob.id,
            mob.silent,
        );
        let except = except_actor.then(|| self.native_acting_entity());
        self.server_play_bound_sound(event, source, at, volume, pitch, id, silent, except);
    }

    pub(in crate::game) fn server_particle_at(
        &mut self,
        particle: crate::lan::server_effects::ServerParticle,
        at: [f64; 3],
        velocity: [f64; 3],
    ) {
        assert!(
            self.is_logical_server(),
            "server_particle_at called on Client role"
        );
        let speed = if velocity == [0.0; 3] {
            [0.0; 3]
        } else {
            [1.0; 3]
        };
        self.server_send_particles(crate::lan::server_effects::ServerQueuedParticle {
            dimension: self.dim.id(),
            particle,
            at,
            spread: velocity,
            speed,
            count: 0,
            randomization: 0,
            except: None,
        });
    }
}
