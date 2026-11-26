//! Client-side entity state reads and client fire-immunity writes.
use super::commands_common::Target;
use mod_api::EntityRef;

impl crate::game::ClientGame<'_> {
    /// `Entity.tickCount` of a client's own copy: the local player's, or that of an entity the
    /// server added to the level the client shows, whatever its kind, counted by the client's
    /// entity pass (`Entity.commonTick`) since the client added it.
    pub(in crate::game) fn client_entity_tick_count(&self, entity: EntityRef) -> Option<i32> {
        let (remote, id) = self.native_client_entity_context(entity)?;
        // The local player from its admission (Login, Respawn), before the server's first
        // teleport gives it a body.
        if id == remote.entity_id {
            return remote.has_logical_player.then_some(remote.own_tick_count);
        }
        remote
            .entities
            .get(&id)
            .map(|tracked| tracked.tick_count)
            .or_else(|| remote.general.rows.get(&id).map(|row| row.age as i32))
    }

    pub(in crate::game) fn client_tracked_player_fire_immunity(
        &self,
        entity: EntityRef,
    ) -> Option<bool> {
        let (kind, _) = self.native_client_tracked(entity)?;
        (kind == "minecraft:player").then_some(())?;
        self.client
            .remote
            .as_ref()?
            .tracked_fire_immunity(i32::try_from(entity.id()).ok()?)
    }

    pub(in crate::game) fn client_entity_is_shift_key_down(
        &self,
        entity: EntityRef,
    ) -> Option<bool> {
        Some(match self.native_client_entity_target(entity)? {
            Target::Player => self.held_mapping(sunrise_client::actions::Action::Sneak),
            _ => false,
        })
    }

    pub(in crate::game) fn client_entity_main_arm(
        &self,
        entity: EntityRef,
    ) -> Option<mod_api::MainHand> {
        let target = self.native_client_entity_target(entity)?;
        let left = match target {
            Target::Player => self.client.settings.main_hand_left,
            Target::Mob(id) => self
                .common.mobs
                .iter()
                .find(|m| m.id == id && !m.removed)?
                .main_arm_left(),
            Target::Stand(_) | Target::Dragon(_) => false,
            _ => return None,
        };
        Some(if left {
            mod_api::MainHand::Left
        } else {
            mod_api::MainHand::Right
        })
    }

    pub(in crate::game) fn entity_set_client_fire_immunity(
        &mut self,
        entity: EntityRef,
        immune: bool,
    ) -> bool {
        if let Some(target) = self.native_client_entity_target(entity) {
            let (grant, ticks) = match target {
                Target::Mob(id) => {
                    let Some(mob) = self.common.mobs.iter_mut().find(|m| m.id == id && !m.removed) else {
                        return false;
                    };
                    (&mut mob.mod_fire_immune, &mut mob.fire_ticks)
                }
                Target::Stand(id) => {
                    let Some(stand) = self.common.armor_stands.iter_mut().find(|s| s.id == id) else {
                        return false;
                    };
                    (&mut stand.mod_fire_immune, &mut stand.fire_ticks)
                }
                Target::Player => {
                    let Some(player) = self.common.player.as_mut() else {
                        return false;
                    };
                    (&mut player.mod_fire_immune, &mut player.fire_ticks)
                }
                _ => return false,
            };
            *grant = immune;
            if immune && matches!(target, Target::Player) {
                *ticks = (*ticks).min(0);
            }
            return true;
        }
        let tracked_player = self
            .native_client_tracked(entity)
            .is_some_and(|(kind, _)| kind == "minecraft:player")
            && self
                .client
                .remote
                .as_ref()
                .and_then(|r| {
                    i32::try_from(entity.id())
                        .ok()
                        .and_then(|id| r.tracked_fire_immunity(id))
                })
                .is_some();
        if !tracked_player {
            return false;
        }
        self.client
            .remote
            .as_mut()
            .is_some_and(|remote| remote.set_tracked_fire_immunity(entity.id() as i32, immune))
    }
}
