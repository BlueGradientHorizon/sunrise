#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    pub(in crate::game) fn send_captured_level_sound(
        &mut self,
        mut captured: sunrise_client::audio::CapturedSound,
        except: Option<u32>,
    ) {
        let source = mod_api::SoundSource::ALL
            .get(captured.source)
            .copied()
            .unwrap_or(mod_api::SoundSource::Master);
        let sound = LevelSound {
            name: &captured.event,
            at: captured.position,
            volume: captured.volume,
            pitch: captured.pitch,
            source,
            emitter: captured.entity,
        };
        let Some(heard) = self.ask_level_sound(self.dim.id(), sound) else {
            return;
        };
        let name = heard.name.into_owned();
        captured.source = heard.source as usize;
        captured.volume = heard.volume;
        captured.pitch = heard.pitch;
        captured.event = name;
        if let Some(host) = self.lan.as_ref() {
            host.hub.captured_sound(&captured, except);
        }
    }

    /// The guests' items in use and Riptide spins, for the players around
    /// (`LivingEntity.startUsingItem`, `stopUsingItem` and `startAutoSpinAttack`
    /// set DATA_LIVING_ENTITY_FLAGS).
    pub(in crate::game) fn lan_living_flags(&mut self) {
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        host.hub.guests.update_all(|g| {
            let using = host.play.get(&g.conn).and_then(GuestPlay::using);
            let spinning = g
                .motion
                .as_ref()
                .is_some_and(|m| m.auto_spin_attack_ticks > 0);
            g.living_flags = crate::game::lan::living_flags(using, spinning);
        });
    }

    /// A use the server ran swung the guest's hand: their own client is told
    /// too (`swing(hand, true)` for a SwingSource.SERVER result). Sunrise's
    /// client does not run uses itself, so it does not predict the swing; a
    /// client that did (a block placed) is still in the first half of its
    /// swing and ignores it (`SwingState.startIfAble`).
    pub(in crate::game) fn lan_swing_back(&mut self, conn: u32, swung: Option<bool>) {
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        if swung != Some(true) {
            return;
        }
        let (Some(g), Some(play)) = (host.hub.guest(conn), host.play.get(&conn)) else {
            return;
        };
        if let Some(c) = play.swing.current {
            host.hub
                .send_to(conn, &swing_packet(g.entity_id, c.hand, c.animation));
        }
    }

    /// The tick's arm swings for the players around (each guest's to everyone
    /// else), and `LivingEntity.updateSwingTime` for the guests' bodies here.
    pub(in crate::game) fn lan_swings(&mut self) {
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        let conns: Vec<u32> = host.play.keys().copied().collect();
        for conn in conns {
            let Some(g) = host.hub.guests.iter().find(|g| g.conn == conn) else {
                continue;
            };
            let (id, at, here) = (g.entity_id, g.position, host.hub.here(g));
            let play = host.play.get_mut(&conn).expect("play");
            let swung = std::mem::take(&mut play.swings) > 0;
            if let (true, true, Some(c)) = (swung, here, play.swing.current) {
                let packet = swing_packet(id, c.hand, c.animation);
                host.hub.broadcast_near(at, 128.0, &packet, Some(conn));
            }
            play.swing.tick();
        }
    }
}
