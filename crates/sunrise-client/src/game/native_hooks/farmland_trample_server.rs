#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::farmland_trample::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::block_landing::FarmlandTrample;

/// The hook point. It does nothing in Sunrise.
impl Game<'_, crate::game::server_side::ServerState> {
    /// Apply the owning-host decision inside `FarmlandBlock.fallOn`, while the
    /// landing is paused. The caller resumes fall damage even when this refuses.
    pub(in crate::game::server_side) fn trample_farmland(
        &mut self,
        entity: u32,
        player: bool,
        trample: FarmlandTrample,
    ) {
        let FarmlandTrample {
            position: pos,
            state,
            base,
            distance,
            ..
        } = trample;
        let at = [pos.x, pos.y, pos.z];
        let dimension = self.dim;
        let still = |game: &Self| {
            game.dim == dimension
                && !game.side.world_debug
                && game.world.is_loaded_at(pos.x, pos.y, pos.z)
                && !game.world.carrier_fences.locks_cell(&at)
                && game.world.block(pos.x, pos.y, pos.z) == state
        };
        if !still(self) {
            return;
        }
        let applied = if mod_api::__private::listened(mod_api::HookId::FarmlandTrample) {
            let Some(living) = self.entity(entity).and_then(|e| self.living(e)) else {
                return;
            };
            self.storage_transaction(|game, _| {
                let mut cancel = false;
                game.native_farmland_trample(living, pos, state, distance, &mut cancel);
                let valid =
                    |game: &Self| game.entity(entity) == Some(living.entity()) && still(game);
                (!cancel && game.apply_farmland_trample(player, trample, valid, true))
                    .then_some(())
                    .ok_or(())
            })
            .is_ok()
        } else {
            self.apply_farmland_trample(player, trample, still, false)
        };
        if applied {
            let who = if player {
                Who::Player
            } else {
                Who::Mob(entity)
            };
            self.after_commit(crate::game::transactions::After::BlockGameEvent(
                "minecraft:block_change",
                at,
                who,
                base,
            ));
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_entity_refs::tests::with_game;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::farmland_trample::tests::AfterTrap;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::farmland_trample::tests::Body;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::farmland_trample::tests::Choice;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::farmland_trample::tests::FIELD;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::farmland_trample::tests::Gardener;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::farmland_trample::tests::HEALTHY_WRITE;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::farmland_trample::tests::LANDING;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::farmland_trample::tests::LANDING_Y;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::farmland_trample::tests::LISTENER_WRITE;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::farmland_trample::tests::Moment;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::farmland_trample::tests::OTHER;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::farmland_trample::tests::OUTSIDE;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::farmland_trample::tests::Observe;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::farmland_trample::tests::Trace;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::farmland_trample::tests::WALKING;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::farmland_trample::tests::land;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::farmland_trample::tests::land_with_later_packet;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::farmland_trample::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_item_places::tests::{
        guest, open_to_lan,
    };
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use mod_api::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use std::sync::{Arc, Mutex};
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_client::{
        chunk::Chunk, lan::guest::InboxItem, motion::PlayerMotion,
    };
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_core::geometry::Vec3;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_protocol::packets::{
        Empty, play, play::Serverbound as S,
    };

    #[test]
    #[ignore = "daily"]
    fn ordinary_moves_and_short_farmland_landings_do_not_pause() {
        crate::game::in_own_process(
            "game::server_side::native_hooks_server::farmland_trample::tests::ordinary_moves_and_short_farmland_landings_do_not_pause",
            || {
                use sunrise_client::{
                    health::{DamageLevel, LentLevel},
                    mobs::{Mob, ai},
                    motion::MotionWorld,
                };

                let trace = Trace::default();
                mod_platform::testing::with_mods(
                    vec![ModEntry::new(
                        "gardener",
                        Box::new(Gardener(Choice::Allow, trace.clone())),
                    )]
                    .into(),
                    || {
                        with_game(|game| {
                            game.options.dedicated = Some(Vec::new());
                            game.streaming
                                .insert_remote(&mut game.world, Chunk::empty(0, 0));
                            let registry = game.pack.assets.registry;
                            let farmland = registry.default_state("minecraft:farmland").unwrap();
                            let [x, y, z] = FIELD;
                            game.world.set(x, y, z, farmland).unwrap();
                            let pig = game.res.mob_types().get("minecraft:pig").unwrap().clone();
                            let mut lent = LentLevel::take(&mut game.world);
                            {
                                let view = game.mob_world_base(&[]);
                                let mut level = DamageLevel::lent(
                                    &game.world,
                                    registry,
                                    &mut lent,
                                    Some(game.items()),
                                    Some(&game.pack.block_registries),
                                    &game.damage_hooks,
                                );
                                for walking in [false, true] {
                                    let (start_y, velocity) = if walking {
                                        (LANDING_Y, Vec3::new(0.125, -0.08, 0.0))
                                    } else {
                                        // Even the smallest random draw cannot pass this short fall.
                                        (64.125, Vec3::new(0.0, -0.5, 0.0))
                                    };
                                    let mut mob = Mob::new(
                                        LANDING,
                                        pig.clone(),
                                        Vec3::new(1.5, start_y, 1.5),
                                        0.0,
                                    );
                                    mob.ai.goals = Default::default();
                                    mob.ai.targets = Default::default();
                                    mob.ground.on_ground = walking;
                                    mob.velocity = velocity;
                                    let mut trample = None;
                                    let progress = ai::tick_until_consumption(
                                        &mut mob,
                                        &view,
                                        &mut level,
                                        &mut Vec::new(),
                                        &mut trample,
                                    );
                                    assert!(trample.is_none());
                                    assert!(
                                        matches!(progress, ai::TickProgress::Complete(true)),
                                        "walking={walking}: {progress:?}"
                                    );
                                    assert!(mob.ground.on_ground);
                                    assert_eq!(mob.position.y, LANDING_Y);
                                    assert_eq!(mob.fall_distance, 0.0);
                                    assert_eq!(mob.position.x > 1.5, walking);
                                }
                            }
                            lent.give_back(&mut game.world);

                            // The guest motion pass finishes a short fall by itself:
                            // `None` means no continuation and no return to the owning host.
                            let mut guest =
                                PlayerMotion::new(game.motion, Vec3::new(1.5, LANDING_Y, 1.5))
                                    .unwrap();
                            guest.fall_distance = 0.125;
                            let side = game_side!(game);
                            let mut motion = MotionWorld {
                                side: &side,
                                entities: &[],
                                player: &mut guest,
                                scene: &mut game.world,
                                assets: &game.pack.assets,
                                collision_binding: game.collision,
                                resources: game.motion,
                                health: &game.health_resources,
                                tag_items: Some(world_items!(game)),
                                loop_context: &game.loop_context,
                                damage_hooks: &game.damage_hooks,
                                block_registries: Some(&game.pack.block_registries),
                            };
                            assert!(
                                motion
                                    .check_guest_fall_damage(
                                        -0.125,
                                        true,
                                        farmland,
                                        BlockPos::new(x, y, z)
                                    )
                                    .unwrap()
                                    .is_none()
                            );
                            assert_eq!(guest.fall_distance, 0.0);
                            assert_eq!(game.world.block(x, y, z), farmland);
                            assert!(trace.lock().unwrap().is_empty());
                        });
                    },
                );
            },
        );
    }
}
