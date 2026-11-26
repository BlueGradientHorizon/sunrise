//! Client-side entity state reads and client fire-immunity writes.
use super::{Game, commands::Target};
use mod_api::EntityRef;

impl Game<'_, crate::game::client_side::ClientState> {
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
            Target::Player => self.side.settings.main_hand_left,
            Target::Mob(id) => self
                .mobs
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
                    let Some(mob) = self.mobs.iter_mut().find(|m| m.id == id && !m.removed) else {
                        return false;
                    };
                    (&mut mob.mod_fire_immune, &mut mob.fire_ticks)
                }
                Target::Stand(id) => {
                    let Some(stand) = self.armor_stands.iter_mut().find(|s| s.id == id) else {
                        return false;
                    };
                    (&mut stand.mod_fire_immune, &mut stand.fire_ticks)
                }
                Target::Player => {
                    let Some(player) = self.player.as_mut() else {
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
        self.remote
            .as_mut()
            .is_some_and(|remote| remote.set_tracked_fire_immunity(entity.id() as i32, immune))
    }
}
