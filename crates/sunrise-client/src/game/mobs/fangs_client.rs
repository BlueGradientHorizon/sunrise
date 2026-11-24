#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fangs::FangState;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fangs::Fangs;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fangs::Party;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fangs::fang_attack_pitch;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fangs::fang_crit;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fangs::fangs_allied;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fangs::illager_considers;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fangs::is_illager;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fangs::p_zrot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::fangs::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::mobs::{Part, Species, anim::Poses};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::particles::{ParticleOptions, ParticleType};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::random::{LegacyRandom, RandomSource};

impl Game<'_> {
    pub(in crate::game) fn insert_fangs(
        &mut self,
        id: u32,
        at: [f64; 3],
        yaw: f32,
        delay: i32,
        owner: u32,
    ) {
        if self.fangs.model.is_none() {
            self.fangs.model = self.client.entity_models.evoker_fangs.clone();
        }
        self.fangs.list.push(Fangs {
            attachment_lifetime: Default::default(),
            join: Default::default(),
            id,
            pos: at,
            motion: Vec3::ZERO,
            yaw,
            warmup: delay,
            life: 22,
            owner,
            started: false,
            fire_ticks: 0,
            synced_fire: false,
            has_visual_fire: false,
            random: sunrise_client::mobs::entity_random(id, Vec3::new(at[0], at[1], at[2])),
        });
    }
    /// `EvokerFangs.handleEntityEvent`: only event 4 starts a client attack.
    /// `playLocalSound` at `nextFloat() * 0.2 + 0.85`. The fangs this client
    /// tracks carry no Silent flag, so the sound always plays.
    pub(in crate::game) fn remote_fangs_event(&mut self, id: u32, event: i8) {
        let sound = self
            .fangs
            .list
            .iter()
            .position(|f| f.id == id)
            .and_then(|i| {
                self.fangs.list.update(i, |f| {
                    if event != 4 {
                        return None;
                    }
                    f.started = true;
                    let pitch = fang_attack_pitch(&mut f.random);
                    Some((f.pos, pitch))
                })
            });
        if let Some((pos, pitch)) = sound {
            self.audio.0.play_unshared(
                "minecraft:entity.evoker_fangs.attack",
                Some(pos),
                1.0,
                pitch,
            );
        }
    }
    /// `EvokerFangs.tick`: clients only advance animation, never damage or discard.
    /// At life 14, twelve `crit` particles (`ATTACK_TRIGGER_TICKS`).
    pub(in crate::game) fn tick_remote_fangs(&mut self) {
        let mut bursts = Vec::new();
        self.fangs.list.for_each_mut(|f| {
            if f.started {
                f.life -= 1;
                if f.life == 14 {
                    let mut crits = Vec::with_capacity(12);
                    for _ in 0..12 {
                        crits.push(fang_crit(&mut f.random, f.pos));
                    }
                    bursts.push(crits);
                }
            }
        });
        let crit = ParticleOptions::simple(ParticleType::Crit);
        for crits in bursts {
            for (at, velocity) in crits {
                self.particles.0.add(&crit, at, velocity);
            }
        }
    }
}
