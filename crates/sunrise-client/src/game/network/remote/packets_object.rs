#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    /// `C::RemoveEntities`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_remove_entities_packet(&mut self, p: play::RemoveEntities) {
        for &id in &p.ids {
            self.forget_remote_entity_construction(id);
            self.forget_client_collider(id);
            if let Some(r) = self.remote.as_mut() {
                if let Some(uuid) = r.level_players.remove(&id) {
                    r.attachments.remove(&Holder::Player(uuid));
                }
                r.level_player_names.remove(&id);
                r.level_player_textures.remove(&id);
                r.level_player_infos.remove(&id);
                r.attachments.remove(&Holder::Entity(id as u32));
            }
            self.pick_section_remove(id as u32);
        }
        self.remove_entities(&p.ids);
    }

    /// `C::EntityPositionSync`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_entity_position_sync_packet(
        &mut self,
        p: play::EntityPositionSync,
    ) {
        let path = match p.path {
            play::PositionPath::Linear(at) => Path::Linear(at),
            play::PositionPath::Stepped(steps) => Path::Stepped(steps),
        };
        // handleEntityPositionSync: the position codec's base is set.
        if let Some(t) = self.remote.as_mut().and_then(|r| r.entities.get_mut(&p.id)) {
            t.codec_base = path.end();
        }
        self.remote_position_sync_and_file(p.id, path, p.yaw, p.pitch, p.on_ground);
    }

    /// `C::MoveVehicle`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_move_vehicle_packet(&mut self, p: play::MoveVehicle) {
        // handleMoveVehicle: the vehicle this client moves goes where the server says,
        // unless it is within 1e-5 already, and is reported back at once.
        if let Some(at) = self.driven_client_position() {
            let to = Vec3::new(p.position[0], p.position[1], p.position[2]);
            if to.subtract(at).length() > f64::from(1.0e-5_f32) {
                self.remote_move_vehicle_and_file(p.position, (p.yaw, p.pitch));
            }
            if let Some(report) = self.remote_vehicle_packets().into_iter().next() {
                self.send(report);
            }
        }
    }

    /// `C::ProjectilePower`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_projectile_power_packet(&mut self, p: play::ProjectilePower) {
        // ClientPacketListener.handleProjectilePowerPacket: the power a
        // hurting projectile accelerates by, sent bundled with its motion.
        super::super::remote_projectiles::remote_projectile_power(
            &mut self.creatures.projectiles,
            p.id,
            p.power,
        );
    }

    /// `C::SwingAnimation`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_swing_animation_packet(&mut self, p: play::SwingAnimation) {
        // ClientPacketListener.handleSwingAnimation: the entity swings
        // (LivingEntity.swing, which a swing in its first half ignores).
        let hand = if p.hand == 1 { Hand::Off } else { Hand::Main };
        let animation = sunrise_client::first_person::SwingAnimation {
            kind: p.kind.clamp(0, 255) as u8,
            duration: p.duration,
        };
        if self.remote.as_ref().is_some_and(|r| r.entity_id == p.id) {
            self.swing_hand(hand, animation);
        } else if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32) {
            m.swing_hand_modified(hand == Hand::Off, animation);
        }
    }

    /// `C::HurtAnimation`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_hurt_animation_packet(&mut self, p: play::HurtAnimation) {
        if self.remote.as_ref().is_some_and(|r| r.entity_id == p.id) {
            // Player.animateHurt: the tilt turns around the yaw.
            if let Some(pl) = self.player.as_mut() {
                pl.health.animate_hurt_dir(p.yaw);
            }
        } else if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32) {
            m.hurt_time = 10;
            if &*m.kind.id == "minecraft:player" {
                m.extra.hurt_dir = p.yaw;
            }
        } else {
            self.remote_dragon_hurt(p.id);
        }
    }

    /// `C::RotateHead`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_rotate_head_packet(&mut self, p: play::RotateHead) {
        // ClientPacketListener.handleRotateMob: lerpHeadTo(yHeadRot, 3).
        if let Some(r) = self.remote.as_mut() {
            let yaw = sunrise_protocol::buf::unpack_degrees(p.head_yaw);
            r.head_lerps.insert(p.id, (yaw, 3));
        }
    }

    /// `C::SetEntityMotion`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_set_entity_motion_packet(
        &mut self,
        p: play::SetEntityMotion,
    ) -> Result<()> {
        // ClientPacketListener.handleSetEntityMotion: knockback on this player.
        if self.remote.as_ref().is_some_and(|r| r.entity_id == p.id) {
            if let Some(pl) = self.player.as_mut() {
                pl.velocity = Vec3::new(p.velocity[0], p.velocity[1], p.velocity[2]);
            }
            return Ok(());
        }
        // The same packet for another entity: Entity.lerpMotion.
        self.remote_motion(p.id, p.velocity);
        Ok(())
    }
}
