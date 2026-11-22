//! Sounds and particles in a level for native server mods, through a
//! `LevelRef` (docs/modding/src/how-to/native-mods.md, "Entity and level handles", #7387).
//! Every player in range hears or sees them, as `ServerLevel.playSound` and
//! `sendParticles` send them. They return false on a client, for a handle of
//! an earlier open, or for arguments the network can't carry.
use super::journal_level_write::PendingWrite;
use super::native_hooks::{mod_particles_ok, sound_id};
use super::{
    Game, creatures::NativeSound, lan_particles::QueuedParticle, level_sounds::LevelSound,
};
use mod_api::{LevelRef, PlayerRef, PlayerWorldRefusal, SoundSource};
use sunrise_client::particles::ParticleOptions;
use sunrise_world::registry::BlockStateId;

/// A level or entity-bound native sound request, before dispatch or queueing.
pub(super) struct NativeSoundRequest<'a> {
    pub(super) level: LevelRef,
    pub(super) sound: &'a str,
    pub(super) at: [f64; 3],
    pub(super) source: SoundSource,
    pub(super) volume: f32,
    pub(super) pitch: f32,
    pub(super) except: Option<u32>,
    pub(super) entity: Option<i32>,
    /// A direct holder, with or without a fixed broadcast radius.
    pub(super) inline_range: Option<Option<f32>>,
}

impl Game<'_> {
    /// The level's dimension id, for a handle of this open on the server.
    fn native_level_dimension(&self, level: LevelRef) -> Option<&'static str> {
        self.native_level_world(level)?;
        Some(level.dimension_id())
    }
    /// Plays sound event `sound` at `at` for the players in this level
    /// within range (16 blocks, times the volume above 1), as
    /// `ServerLevel.playSound(except, ...)` does, under `source`'s volume
    /// slider, leaving out `except`'s client (a fake player has none, so it
    /// leaves out no one; a stale one queues nothing). Registry entries are sent by ID, and
    /// other valid keys by name. True means queued, not heard.
    pub(super) fn level_play_sound(
        &mut self,
        level: LevelRef,
        sound: &str,
        at: [f64; 3],
        source: SoundSource,
        volume: f32,
        pitch: f32,
        except: Option<PlayerRef>,
    ) -> bool {
        let except = match except {
            None => None,
            Some(player) => match self.player_connection(player) {
                Ok(conn) => Some(conn),
                // A fake player has no client to leave out.
                Err(PlayerWorldRefusal::NoConnection) => None,
                Err(_) => return false,
            },
        };
        self.native_queue_sound(NativeSoundRequest {
            level,
            sound,
            at,
            source,
            volume,
            pitch,
            except,
            entity: None,
            inline_range: None,
        })
    }

    /// Accept a level or entity-bound sound. In a transaction, ask the hooks at the call and
    /// defer only the resolved effect to root commit; outside one, queue it with its seed.
    pub(super) fn native_queue_sound(&mut self, request: NativeSoundRequest<'_>) -> bool {
        let NativeSoundRequest {
            level,
            sound,
            at,
            source,
            volume,
            pitch,
            except,
            entity,
            inline_range,
        } = request;
        let Some(dimension) = self.native_level_dimension(level) else {
            return false;
        };
        if self.lan.is_none() || !sound_id(sound) {
            return false;
        }
        self.emit_level_effect(PendingWrite::Sound(NativeSound {
            dimension,
            sound: sound.to_owned(),
            inline_range,
            at,
            source: source.index() as i32,
            volume,
            pitch,
            seed: 0,
            except,
            entity,
            skip_hooks: false,
        }));
        true
    }

    /// Queues a gameplay sound in its own dimension, even when another level is in place.
    pub(in crate::game) fn queue_level_sound(
        &mut self,
        dimension: &'static str,
        sound: LevelSound<'_>,
        except: Option<u32>,
    ) -> bool {
        if self.lan.is_none() {
            return false;
        }
        self.emit_level_effect(PendingWrite::Sound(NativeSound {
            dimension,
            sound: sound.name.to_owned(),
            inline_range: None,
            at: sound.at,
            source: sound.source.index() as i32,
            volume: sound.volume,
            pitch: sound.pitch,
            seed: 0,
            except,
            entity: sound.emitter.map(|id| id as i32),
            skip_hooks: false,
        }));
        true
    }
    /// Sends `count` particles around `at` to the players in this level
    /// within 32 blocks, as `ServerLevel.sendParticles` does: each offset is
    /// a Gaussian times `spread`, and `speed` scales their motion (a zero
    /// count sends one particle moving along `spread`). Block particle types
    /// need `state`; item particles need a validated stack; trail and vibration aren't supported.
    /// True means queued, not drawn.
    pub(super) fn level_send_particles(
        &mut self,
        level: LevelRef,
        options: &ParticleOptions,
        state: Option<BlockStateId>,
        at: [f64; 3],
        spread: [f64; 3],
        speed: f64,
        count: i32,
    ) -> bool {
        let Some(dimension) = self.native_level_dimension(level) else {
            return false;
        };
        let registry = self.pack.assets.registry;
        let state_ok = |id| registry.state(id).is_some();
        if !mod_particles_ok(options, state, state_ok, at, spread, speed) {
            return false;
        }
        self.emit_level_effect(PendingWrite::Particles(QueuedParticle {
            dimension,
            options: options.clone(),
            state,
            at,
            spread,
            speed: [speed; 3],
            count,
            randomization: 0,
        }));
        true
    }
}
