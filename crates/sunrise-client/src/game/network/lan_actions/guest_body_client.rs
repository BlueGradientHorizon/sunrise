#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::guest_body::initialize;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_actions::guest_body::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    lan::{guest::Guest, hub::CommittedMove},
    landing::PlayerLandingPause,
    motion::{MotionResources, PlayerMotion, ServerTravel},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::Vec3;

impl Game<'_> {
    pub(in crate::game) fn guest_motion_events(
        &mut self,
        conn: u32,
        at: [f64; 3],
        events: Vec<MotionEvent>,
    ) {
        for event in events {
            match event {
                MotionEvent::Health(sunrise_client::health::HealthEvent::DamageBroadcast {
                    source,
                }) => self.guest_damage_broadcast(conn, source),
                // LivingEntity.playHurtSound: the server's `Entity.playSound`
                // reaches every player around but the hurt one, whose own
                // client plays it from handleDamageEvent (#4149).
                MotionEvent::HurtSound(sound) => self.guest_hurt_sound(conn, sound),
                // LivingEntity.checkFallDamage: the crumbs reach every player.
                MotionEvent::Fall(sunrise_client::landing::FallEvent::Particles {
                    state,
                    position,
                    count,
                    speed,
                }) => self.landing_dust(state, position, count, speed),
                // A block's own sound names no player: the guest hears it too.
                MotionEvent::Fall(sunrise_client::landing::FallEvent::Sound {
                    sound,
                    position: Some(pos),
                    volume,
                    pitch,
                }) if sound.block_sound().is_some() => {
                    if let Some(name) = sound.block_sound() {
                        let at = [
                            f64::from(pos.x) + 0.5,
                            f64::from(pos.y) + 0.5,
                            f64::from(pos.z) + 0.5,
                        ];
                        self.send_level_sound(LevelSound::new(name, at, volume, pitch), None);
                    }
                }
                // LivingEntity.causeFallDamage, and HoneyBlock.fallOn's
                // `entity.playSound`: Player.playSound skips the guest, whose
                // own client plays it; everyone else hears it.
                MotionEvent::Fall(sunrise_client::landing::FallEvent::Sound {
                    sound,
                    volume,
                    pitch,
                    ..
                }) => {
                    if let Some(name) = sound.player_fall_sound() {
                        self.send_level_sound(LevelSound::new(name, at, volume, pitch), Some(conn));
                    }
                }
                // Player.causeFallDamage, mayfly: the mods hear the landing.
                MotionEvent::Fall(sunrise_client::landing::FallEvent::FlyableFall {
                    distance,
                    multiplier,
                }) => {
                    if let Some((uuid, id, name)) = self
                        .lan
                        .as_ref()
                        .and_then(|host| host.hub.guest(conn))
                        .map(|g| (g.uuid, g.entity_id, g.name.clone()))
                    {
                        self.as_guest(conn, |game| {
                            game.native_player_flyable_fall(uuid, id, &name, distance, multiplier);
                        });
                    }
                }
                // HoneyBlock.fallOn: entity event 54, including the player who
                // landed (`broadcastEntityEvent` has no excluded player).
                MotionEvent::Fall(sunrise_client::landing::FallEvent::Broadcast(event))
                    if sunrise_client::landing::honey_particle_count(event as i8).is_some() =>
                {
                    if let Some(host) = self.lan.as_ref()
                        && let Some(id) = host.hub.guest(conn).map(|g| g.entity_id)
                    {
                        host.hub.entity_event(id, event as i8);
                    }
                }
                MotionEvent::Fall(sunrise_client::landing::FallEvent::BlockFallSound {
                    position,
                    state,
                }) => {
                    let entity = self
                        .lan
                        .as_ref()
                        .and_then(|host| host.hub.guest(conn))
                        .map(|g| (g.entity_id as u32, "minecraft:player"));
                    let sound = self.block_sound_type(
                        [position.x, position.y, position.z],
                        state,
                        entity,
                        mod_api::BlockSoundCause::Fall,
                    );
                    self.send_level_sound(
                        // `Player.playSound` plays in the players' source.
                        LevelSound::new(&sound.fall, at, sound.volume * 0.5, sound.pitch * 0.75)
                            .in_source(mod_api::SoundSource::Players),
                        Some(conn),
                    );
                }
                // Farmland a mod listens to: asked and applied as the landing guest.
                MotionEvent::Fall(event) => {
                    let player = self
                        .lan
                        .as_ref()
                        .and_then(|host| host.hub.guest(conn))
                        .map(|g| g.uuid);
                    // The rule's own game event goes out as the player whose
                    // landing it was (`FarmlandBlock.turnToBaseBlock`'s change,
                    // `TurtleEggBlock.decreaseEggs`' destroy): a sculk sensor, a
                    // calibrated sensor and a warden hear it.
                    if let Some(vibration) = self.landing_block_updates(player, &event) {
                        self.as_guest(conn, |game| game.post_landing_vibration(vibration));
                    }
                }
                // PowderSnowBlock.entityInside: every player around sees the
                // snowflake (`ServerLevel.sendParticles`).
                MotionEvent::Snowflake { position, velocity } => {
                    if let Some(host) = self.lan.as_ref() {
                        crate::game::lan_particles::send_snowflake_particle(
                            host,
                            [position.x, position.y, position.z],
                            [velocity.x, velocity.y, velocity.z],
                        );
                    }
                }
                // LivingEntity.baseTick's entity event 67, to every client
                // tracking the guest and its own; this window draws it too.
                // A mod's other bubble count goes out as particles instead.
                MotionEvent::Drown(bubbles) => {
                    if !self.drown_bubbles(at, [0.0; 3], bubbles) {
                        continue;
                    }
                    if let Some(host) = self.lan.as_ref()
                        && let Some(g) = host.hub.guest(conn)
                    {
                        host.hub
                            .entity_event(g.entity_id, crate::game::combat::DROWN_EVENT);
                    }
                }
                // ServerPlayer.onInsideBlock: the enter_block trigger, for the
                // player who moved.
                MotionEvent::EnterBlock { state } => {
                    self.as_guest(conn, |game| game.trigger_enter_block(state));
                }
                // HoneyBlock.maybeDoSlideEffects: the sound reaches the
                // others (Player.playSound excludes the player, whose client
                // plays it), the entity event 53 everyone around.
                MotionEvent::HoneySlide {
                    pos,
                    sound,
                    particles,
                } => {
                    if self.world.game_time % 20 == 0 {
                        self.as_guest(conn, |game| game.trigger_honey_slide([pos.x, pos.y, pos.z]));
                    }
                    let Some((id, at)) = self
                        .lan
                        .as_ref()
                        .and_then(|host| host.hub.guest(conn))
                        .map(|g| (g.entity_id, g.position))
                    else {
                        continue;
                    };
                    if sound {
                        let name = "minecraft:block.honey_block.slide";
                        self.send_level_sound(LevelSound::new(name, at, 1.0, 1.0), Some(conn));
                    }
                    if particles {
                        if let Some(host) = self.lan.as_ref() {
                            host.hub.entity_event(id, 53);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
