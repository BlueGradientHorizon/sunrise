#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::game::{commands::Target, server_travel::SpawnAction};

impl Game<'_> {
    pub(in crate::game) fn guest_respawn(&mut self, conn: u32) {
        self.after_commit(crate::game::transactions::After::PlayerRespawn(conn));
    }

    /// `handleClientCommand(PERFORM_RESPAWN)` calls PlayerList, not ServerPlayer.teleport.
    pub(in crate::game) fn commit_player_respawn(&mut self, conn: u32) {
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        let Some((end_conquered, dead)) = host.hub.update_guest(conn, |guest| {
            (
                std::mem::take(&mut guest.won_game),
                guest.dead && !guest.respawning,
            )
        }) else {
            return;
        };
        if end_conquered || dead {
            self.queue_spawn_trip(
                conn,
                SpawnAction::Replace {
                    was_death: !end_conquered,
                },
            );
        }
    }

    /// Player.remove in the source level. The close hook still writes the old player's map.
    pub(in crate::game) fn remove_old_player_for_clone(&mut self, conn: u32, was_death: bool) {
        let Some(entity) = self
            .lan
            .as_ref()
            .and_then(|h| h.hub.guest(conn))
            .filter(|g| g.placed)
            .map(|g| g.entity_id as u32)
        else {
            return;
        };
        self.eject_player_riders(Some(entity));
        self.guest_close_menus(
            conn,
            if was_death {
                MenuRemoval::Killed
            } else {
                MenuRemoval::ChangedDimension
            },
        );
        if let Some(host) = self.lan.as_mut()
            && let Some(i) = host.hub.guests.connection_index(conn)
        {
            host.hub.remove_player_for_respawn(i);
            if let Some(map) = host.hub.guests.attachments_mut(conn) {
                map.retire();
                map.unbind_index();
            }
        }
        self.entity_section_forget(entity);
    }

    /// `ServerPlayer.die` → `Entity.awardKillScore` on the entity the dying
    /// guest's combat record credits (`LivingEntity.getKillCredit`): the
    /// guest's `entity_killed_player`, with that entity and the death's
    /// `killing_blow`. Nothing credited, nothing fires.
    pub(in crate::game) fn guest_killed_player_trigger(
        &mut self,
        conn: u32,
        at: [f64; 3],
        blow: Option<(String, Option<u32>)>,
    ) {
        const TRIGGER: &str = "minecraft:entity_killed_player";
        let now = self.world.game_time;
        let Some(id) = self
            .guest_combat_record(conn)
            .and_then(|c| c.kill_credit(now))
        else {
            return;
        };
        let Some(killer) = self.guest_entity_view(id, TRIGGER) else {
            return;
        };
        let blow = self.guest_killing_blow(blow, at, TRIGGER);
        self.with_dying_guest(conn, |g| {
            g.trigger_killed_as(TRIGGER, &killer, blow.as_ref());
        });
    }

    /// The dying guest's last blow (`LivingEntity.getLastDamageSource`), its
    /// damage type and causing entity as the combat record held them, as a
    /// damage source: the projectile as the direct entity.
    pub(in crate::game) fn guest_killing_blow(
        &self,
        blow: Option<(String, Option<u32>)>,
        at: [f64; 3],
        trigger: &str,
    ) -> Option<sunrise_client::advancements::DamageView> {
        let (damage, attacker) = blow?;
        // The damage's causing entity: the player in place, another guest, or
        // a mob (`DamageSource.getEntity`).
        let source = attacker.and_then(|id| self.guest_entity_view(id, trigger));
        Some(sunrise_client::advancements::recorded_blow(
            damage, source, at,
        ))
    }
}
