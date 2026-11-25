#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::lan_actions::effects::*;

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn lan_broken_block_positions(&self) -> Vec<[f64; 3]> {
        let Some(host) = self.lan.as_ref() else {
            return Vec::new();
        };
        host.broken
            .iter()
            .copied()
            .chain(
                self.server
                    .lan_events
                    .iter()
                    .filter(|event| event.kind == 2001)
                    .map(|event| event.pos.map(f64::from)),
            )
            .collect()
    }
    /// `LivingEntity.tick`'s arrows and bee stingers falling out of the
    /// players' bodies, and their counts for the players around
    /// (`DATA_ARROW_COUNT_ID`, `DATA_STINGER_COUNT_ID`).
    pub(in crate::game::server_side) fn lan_stuck(&mut self, paused: bool) {
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        if !paused {
            self.combat.0.stuck.tick();
        }
        host.hub.guests.update_all(|g| {
            g.shoulders = crate::game::shoulders::shoulder_variants(g.saved.as_ref());
            g.stuck = match self.guest_combat.0.get_mut(&g.conn) {
                Some(combat) => {
                    if !paused {
                        combat.stuck.tick();
                    }
                    (combat.stuck.arrows, combat.stuck.stingers)
                }
                None => (0, 0),
            };
        });
    }
    /// `InstrumentItem.play`'s `level.playSound(player, ...)`: the others hear
    /// a guest's horn now, the guest's own client played it as it used it.
    /// The horn is bound to the guest (`playSound(player, player, ...)`).
    pub(in crate::game) fn guest_instrument_sound(
        &mut self,
        conn: u32,
        heard: usize,
        horn: Option<String>,
    ) {
        let Some(player) = self
            .lan
            .as_ref()
            .and_then(|host| host.hub.guest(conn))
            .and_then(|g| u32::try_from(g.entity_id).ok())
        else {
            return;
        };
        let Some(horn) = horn else {
            return;
        };
        if !self.server.audio_capture_active {
            return;
        }
        let capture = &mut self.server.pending_sounds;
        let mut sounds = Vec::new();
        let mut i = heard.min(capture.len());
        while i < capture.len() {
            if capture[i].event == horn {
                sounds.push(capture.remove(i));
            } else {
                i += 1;
            }
        }
        for mut captured in sounds {
            captured.entity = Some(player);
            self.send_captured_level_sound(captured, Some(conn));
        }
    }
    /// `level.playSound(player, ...)` of a guest's use it predicted (`remote::hand_operated`
    /// blocks only, as the guest decides): the others hear it now, the guest already did.
    pub(in crate::game) fn guest_use_sounds(&mut self, conn: u32, logged: usize, heard: usize) {
        if self.lan.is_none() {
            return;
        }
        let changed: Vec<(i32, i32, i32)> = self
            .world
            .block_log
            .as_ref()
            .map_or(&[][..], |log| log.get(logged..).unwrap_or(&[]))
            .to_vec();
        // BlockItem.place: `playSound(player, ...)`, for the others only. The
        // placer's client plays it as its block appears. The place sound is the
        // one the block's parts answered the placement with (`place_sound_type`).
        let placer = self
            .lan
            .as_ref()
            .and_then(|host| host.hub.guest(conn))
            .map(|g| (g.entity_id as u32, "minecraft:player"));
        let placed: Vec<([f64; 3], String)> = changed
            .iter()
            .map(|&(x, y, z)| {
                let at = [f64::from(x) + 0.5, f64::from(y) + 0.5, f64::from(z) + 0.5];
                let state = self.world.block(x, y, z);
                let place = mod_api::BlockSoundCause::Place;
                let name = self.block_sound_type([x, y, z], state, placer, place).place;
                (at, name.trim_start_matches("minecraft:").to_owned())
            })
            .collect();
        let mut sounds = Vec::new();
        if self.server.audio_capture_active {
            let capture = &mut self.server.pending_sounds;
            let mut i = heard.min(capture.len());
            while i < capture.len() {
                let (name, at) = (&capture[i].event, &capture[i].position);
                let place = placed
                    .iter()
                    .any(|(p, n)| p == at && name.trim_start_matches("minecraft:") == n.as_str());
                if place {
                    sounds.push(capture.remove(i));
                } else {
                    i += 1;
                }
            }
        }
        let behaviours = self.res.behaviours();
        let hand_operated = !changed.is_empty()
            && changed.iter().all(|&(x, y, z)| {
                crate::game::remote::hand_operated(behaviours, self.world.block(x, y, z))
            });
        if hand_operated && self.server.audio_capture_active {
            let capture = &mut self.server.pending_sounds;
            let start = heard.min(capture.len());
            sounds.extend(capture.drain(start..));
        }
        for sound in sounds {
            self.send_captured_level_sound(sound, Some(conn));
        }
    }
    /// Positional sounds and level events of the tick, for the guests.
    pub(in crate::game) fn lan_broadcast_effects(&mut self) {
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        host.broken.clear();
        let sounds = std::mem::take(&mut self.server.pending_sounds);
        for sound in sounds {
            self.send_captured_level_sound(sound, None);
        }
        self.send_bound_sounds();
        if let Some(host) = self.lan.as_ref() {
            host.hub.finish_captured_sounds();
        }
        // A stop whose removal waited for these sounds follows that removal (#4522).
        self.run_tracking_after_sounds();
    }
    /// A level sound's old emitter must still be known at playback. Keep
    /// sounds needing a new pairing queued, with their later removals held.
    pub(in crate::game) fn lan_prepare_captured_sounds(
        &mut self,
        entities: &[sunrise_client::lan::entity::EntityState],
    ) {
        if !self.server.audio_capture_active {
            return;
        }
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        host.hub
            .prepare_captured_sounds(&mut self.server.pending_sounds, entities);
    }
}
