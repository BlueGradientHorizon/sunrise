//! Role-neutral mount helpers shared by server riding, client HUD/guide, and remote vehicles.
use super::Game;
use crate::game::server_side::mounts::{seat_of, turned};
use sunrise_core::geometry::Vec3;
use sunrise_sim::{
    mobs::{Species, equine},
    sprint::Keys,
};

impl<__Side: crate::game::Side> Game<'_, __Side> {
    pub(in crate::game) fn mount_index(&self) -> Option<usize> {
        let id = self.riding?;
        self.mobs.iter().position(|m| m.id == id)
    }

    /// `PlayerRideableJumping`: horses, camels and nautili (the dash).
    pub(in crate::game) fn jumps(species: Species) -> bool {
        equine::is_equine(species) || sunrise_sim::mobs::nautilus::is_nautilus(species)
    }

    /// `LocalPlayer.jumpableVehicle`: controlled vehicle when it can jump now.
    pub(in crate::game) fn jumpable_mount(&self) -> Option<u32> {
        if !self.remote.as_ref().is_some_and(|r| r.first_passenger) {
            return None;
        }
        let mount = &self.mobs[self.mount_index()?];
        let can_jump = Self::jumps(mount.species())
            && mount.extra.saddled()
            && !(matches!(mount.species(), Species::Camel | Species::CamelHusk)
                && sunrise_sim::mobs::camel::refuse_to_move(mount));
        can_jump.then_some(mount.id)
    }

    pub(in crate::game) fn charge_mount_jump(&mut self, i: usize, keys: Keys) -> f32 {
        let saddled = self.mobs[i].extra.saddled() && Self::jumps(self.mobs[i].species());
        let mut jump = 0.0;
        if saddled {
            let st = &mut self.mount;
            if st.jump_ticks < 0 {
                st.jump_ticks += 1;
                if st.jump_ticks == 0 {
                    st.jump_scale = 0.0;
                }
            }
            if st.was_jumping && !keys.jump {
                st.jump_ticks = -10;
                let amount = (st.jump_scale * 100.0).floor() as i32;
                jump = equine::jump_pending_scale(amount);
            } else if !st.was_jumping && keys.jump {
                st.jump_ticks = 0;
                st.jump_scale = 0.0;
            } else if st.was_jumping {
                st.jump_ticks += 1;
                st.jump_scale = if st.jump_ticks < 10 {
                    st.jump_ticks as f32 * 0.1
                } else {
                    0.8 + 2.0 / (st.jump_ticks - 9) as f32 * 0.1
                };
            }
            st.was_jumping = keys.jump;
        }
        jump
    }

    /// `positionRider`: the player sits on the mount after it moves.
    pub(in crate::game) fn position_rider(&mut self) {
        let Some(i) = self.mount_index() else {
            return;
        };
        let (mob_id, is_happy_ghast) = {
            let m = &self.mobs[i];
            (m.id, m.species() == Species::HappyGhast)
        };
        let seat_index = if is_happy_ghast {
            self.side_own_passenger_index(mob_id)
        } else {
            0
        };
        let m = &self.mobs[i];
        let mut at = if sunrise_sim::mobs::camel::is_camel(m.species()) {
            seat_of(m, sunrise_sim::mobs::camel::passenger_attachment(m))
        } else {
            seat_of(m, sunrise_sim::mobs::riders::type_attachment(m))
        };
        if is_happy_ghast {
            at = seat_of(
                m,
                sunrise_sim::mobs::riders::passenger_attachment_at(m, seat_index),
            );
        }
        let rear = turned(equine::rear_offset(m), m.yaw);
        at = Vec3::new(at.x + rear.x, at.y + rear.y, at.z + rear.z);
        let first = self.posed_rider != self.riding;
        self.posed_rider = self.riding;
        if let Some(p) = self.player.as_mut() {
            self.previous_position = if first { at } else { p.position };
            p.position = at;
            p.velocity = Vec3::ZERO;
            p.fall_distance = 0.0;
        }
    }
}
