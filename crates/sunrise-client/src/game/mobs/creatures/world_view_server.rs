#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::creatures::world_view::*;

impl Game<'_, crate::game::server_side::ServerState> {
    pub(in crate::game) fn mob_world_base<'s>(&'s self, others: &'s [Seen]) -> MobWorld<'s> {
        self.world.enable_path_type_cache();
        self.mob_world_base_with(
            others,
            MobWorldSideParams {
                side: self,
                trading_players: Some(self),
                player: self.player_view(),
                player_identity: self.player_identity(),
                leash_boats: &self.side.leash_boats,
                teams: std::sync::Arc::new(self.mob_teams()),
                lid_openers: &self.side.lid_openers,
                mod_effects: Some(self),
                mob_hooks: (self.native_authoritative()
                    && ([
                        mod_api::HookId::EndermanAnger,
                        mod_api::HookId::EntityMobGriefing,
                        mod_api::HookId::LivingVisibility,
                        mod_api::HookId::LivingChangeTarget,
                    ]
                    .into_iter()
                    .any(mod_api::__private::listened)
                        || [
                            mod_api::HookId::CanEntityDestroy,
                            mod_api::HookId::BlockPathType,
                            mod_api::HookId::AdjacentBlockPathType,
                        ]
                        .into_iter()
                        .any(mod_api::__private::has_scoped_hook)))
                .then_some(self as &dyn sunrise_client::mobs::MobHooks),
                goal_host: (self.native_authoritative()
                    && (mod_api::__private::has_entity_parts()
                        || mod_api::__private::listened(mod_api::HookId::LivingChangeTarget)))
                .then_some(self as &dyn sunrise_client::mobs::part_goals::GoalHost),
                asks_animal_tame: mod_api::__private::listened(mod_api::HookId::AnimalTame),
                asks_living_destroy_block: mod_api::__private::listened(
                    mod_api::HookId::LivingDestroyBlock,
                ),
                asks_baby_spawn: mod_api::__private::listened(mod_api::HookId::BabyEntitySpawn),
                // Server owns entity UUIDs so warden away suspects resolve here.
                who_lookup: Some(self),
                // First player in the level: a server has no player of its own.
                difficulty: self.local_difficulty(
                    self.level_players_iter()
                        .next()
                        .map_or([0.0; 3], |p| [p.position.x, p.position.y, p.position.z]),
                ),
            },
        )
    }
}
