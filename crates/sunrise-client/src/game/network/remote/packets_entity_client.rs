#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::packets_entity::*;

impl Game<'_> {
    /// `C::EntityEvent`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game::client_side) fn apply_entity_event_packet(
        &mut self,
        p: play::EntityEvent,
    ) -> Result<()> {
        if p.event == super::super::fireworks::FIREWORK_EXPLODE_EVENT {
            self.remote_firework_explosion(p.id);
        }
        self.remote_fangs_event(p.id as u32, p.event);
        self.remote_boat_event(p.id, p.event);
        // MinecartTNT.handleEntityEvent 70: prime the fuse the window draws.
        self.remote_tnt_cart_event(p.id, p.event);
        // Arrow.handleEntityEvent 0: a tipped arrow's burst of particles.
        if p.event == 0 {
            self.remote_arrow_puff(p.id as u32);
        }
        // LivingEntity.handleEntityEvent 2: a spear's hit felt.
        if p.event == 2 {
            self.remote_kinetic_hit(p.id);
        }
        // LocalPlayer.setPermissions also refreshes an open World Options screen.
        if (24..=28).contains(&p.event) && self.remote.as_ref().is_some_and(|r| r.entity_id == p.id)
        {
            self.ui.registries.permission = (p.event - 24) as u8;
            self.remote_world_options_permission(p.event >= 26);
        }
        // Player.handleEntityEvent 22 / 23: reduced debug info on / off.
        if (22..=23).contains(&p.event)
            && let Some(r) = self.remote.as_mut().filter(|r| r.entity_id == p.id)
        {
            r.reduced_debug_info = p.event == 22;
            self.debug_entries.0.invalidate();
        }
        // Ravager.handleEntityEvent 69: the roar's puff and push.
        if p.event == 69 {
            self.remote_ravager_roar(p.id);
        }
        // ClientPacketListener.handleEntityEvent 63: a sniffer's digging sound.
        if p.event == 63 {
            self.start_sniffer_digging_sound(p.id as u32);
        }
        // ClientPacketListener.handleEntityEvent 21: a guardian's hum.
        if p.event == 21 {
            self.start_guardian_sound(p.id as u32);
        }
        // ZombieVillager.handleEntityEvent 16: the cure starts.
        if p.event == 16 {
            self.zombie_villager_cure_sound(p.id as u32);
        }
        // Witch.handleEntityEvent 15: the magic puffs over its hat.
        if p.event == 15 {
            self.witch_magic_particles(p.id as u32);
        }
        // MinecartTNT.handleEntityEvent 70: primeFuse on the client.
        if p.event == 70 {
            self.remote_cart_primed(p.id as u32);
        }
        // Fox.handleEntityEvent 45: the crumbs of the food in its mouth.
        if p.event == 45 {
            self.fox_eat_particles(p.id as u32);
        }
        // Rabbit.handleEntityEvent(1): the hop clocks, and
        // Entity.spawnSprintParticle (one block-dust crumb).
        if p.event == 1 {
            let hop = {
                let m = self.mob_index.get_mut(&mut self.mobs, p.id as u32);
                if let Some(m) = m
                    && m.species() == sunrise_client::mobs::Species::Rabbit
                {
                    let sample = (
                        m.position,
                        m.velocity,
                        f64::from(m.width()),
                        m.ground.supporting_block.map(|p| [p.x, p.y, p.z]),
                    );
                    sunrise_client::mobs::rabbit::hop_event(m);
                    Some(sample)
                } else {
                    None
                }
            };
            if let Some((position, movement, width, support)) = hop {
                let id = p.id as u32;
                self.sprint_particle(position, movement, support, width, |game| {
                    // this.random: the rabbit's own, x then z.
                    game.mob_index
                        .get_mut(&mut game.mobs, id)
                        .map(|m| (m.random.next_double(), m.random.next_double()))
                });
            }
        }
        if p.event == 19
            && let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32)
        {
            sunrise_client::mobs::aquatic::squid_cycle_event(m);
        }
        // LivingEntity.handleEntityEvent 55: another player swapped
        // its main and off hand (`swapHandItems`).
        if p.event == 55
            && let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32)
        {
            m.extra.swap_hands();
        }
        // LivingEntity.handleEntityEvent 47 to 52, 65 and 68: an
        // equipped item broke (onEquippedItemBroken).
        if let Some(slot) = sunrise_client::mobs::item_break::equipment_break_slot(p.event) {
            self.remote_equipment_break(p.id, slot);
        }
        // EntityEvent.DEATH: start the death animation.
        if p.event == 3
            && let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32)
        {
            m.health = 0.0;
        }
        // LivingEntity.handleEntityEvent 3 on this player: its death
        // sound, which Player.playSound plays for this client alone
        // (the server's goes to everyone else).
        if p.event == 3
            && self.remote.as_ref().is_some_and(|r| r.entity_id == p.id)
            && self.player.is_some()
        {
            let sound = "minecraft:entity.player.death";
            let pitch = player_voice_pitch();
            self.play_client_player_sound(sound, 1.0, pitch);
        }
        // The events that start a mob's animation (the warden's).
        if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32) {
            sunrise_client::mobs::sync::entity_event(m, p.event as u8);
        }
        // LivingEntity.handleEntityEvent 46: portal particles from the
        // old position to the new one (chorus fruit, an enderman).
        if p.event == 46 {
            self.entity_teleport_particles(p.id);
        }
        // LivingEntity.handleEntityEvent 60: the poof as the dead body goes.
        if p.event == 60
            && let Some(m) = self.mob_index.get(&self.mobs, p.id as u32)
        {
            let (at, w, h) = (m.position, f64::from(m.width()), f64::from(m.height()));
            self.death_poof([at.x, at.y, at.z], w, h);
        }
        if p.event == super::super::totem::TOTEM_EVENT {
            self.remote_totem(p.id);
        }
        // Mob.handleEntityEvent 20: the spawn poof.
        if p.event == 20 && self.mobs.iter().any(|m| m.id == p.id as u32) {
            self.poof_particles(p.id as u32);
        }
        // The particle bursts of Villager 12 to 14 and 42, TamableAnimal
        // and AbstractHorse 6 and 7, Animal and Allay 18, Ocelot 40 and
        // 41, Dolphin 38: the table in `entity_event_burst` is the one
        // list of them; any other event draws nothing here.
        self.entity_event_particles(p.id as u32, p.event);
        // Snowball and ThrownEgg.handleEntityEvent 3: the crumbs where
        // the projectile is (the server removes it right after).
        if p.event == 3 {
            let crumbs = self
                .creatures
                .projectiles
                .iter()
                .find(|pr| pr.id == p.id as u32)
                .and_then(|pr| {
                    let item = match pr.kind {
                        sunrise_client::projectiles::ProjectileKind::Snowball => {
                            "minecraft:snowball"
                        }
                        sunrise_client::projectiles::ProjectileKind::Egg => "minecraft:egg",
                        sunrise_client::projectiles::ProjectileKind::Thrown(t) => t.item,
                        _ => return None,
                    };
                    Some(([pr.position.x, pr.position.y, pr.position.z], item))
                });
            if let Some((at, item)) = crumbs {
                self.item_burst(at, item);
            }
        }
        if p.event == super::super::combat::DROWN_EVENT {
            self.remote_drown(p.id);
        }
        // Entity events 53 and 54: honey crumbs while sliding, and the
        // ten from a landing (`HoneyBlock.showSlideParticles` /
        // `showJumpParticles`).
        if sunrise_client::landing::honey_particle_count(p.event).is_some() {
            let at = if self.remote.as_ref().is_some_and(|r| r.entity_id == p.id) {
                self.player
                    .as_ref()
                    .map(|q| [q.position.x, q.position.y, q.position.z])
            } else {
                self.mobs
                    .iter()
                    .find(|m| m.id == p.id as u32)
                    .map(|m| [m.position.x, m.position.y, m.position.z])
            };
            if let Some(at) = at {
                self.honey_entity_particles(at, p.event);
            }
        }
        if p.event == super::super::consume::USE_ITEM_COMPLETE_EVENT
            && self.remote.as_ref().is_some_and(|r| r.entity_id == p.id)
        {
            self.remote_complete_using();
        }
        Ok(())
    }
    /// `C::UpdateAttributes`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game::client_side) fn apply_update_attributes_packet(
        &mut self,
        p: play::UpdateAttributes,
    ) -> Result<()> {
        // The player's own instances: its maximum health (the hearts the
        // HUD draws and the health it may hold), reach, speed and so on.
        // Then the received health again, which the old maximum clamped.
        if self.remote.as_ref().is_some_and(|r| r.entity_id == p.id) {
            let nameplate = self.remote_nameplate_values(&p.values);
            self.apply_own_attributes(p.values);
            if let Some(m) = self.client.player_model.as_mut() {
                if let Some(value) = nameplate.name_tag {
                    m.set_name_tag_distance(value);
                }
                if let Some(value) = nameplate.below_name {
                    m.set_below_name_distance(value);
                }
            }
            self.apply_remote_health();
            return Ok(());
        }
        self.remote_mob_attributes(p.id, &p.values);
        Ok(())
    }
}
