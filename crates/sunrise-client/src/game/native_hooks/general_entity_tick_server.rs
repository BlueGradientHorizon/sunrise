#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::general_entity_tick::GeneralTickAccess;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::general_entity_tick::ResidentGeneralRow;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::general_entity_tick::native_general_entity_tick;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_hooks::general_entity_tick::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::{
    commands::{Target, entity_pitch},
    general::GeneralRow,
    transactions::OpenTransactions,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
    EntityRef, EntityServerAccess, EntityServerScope, EntitySubjectView, ResidentGeneralEntity,
    ServerHost,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::lan::playerdata::ints_uuid;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::geometry::Vec3;

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_hooks::general_entity_tick::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::{
        api_values::test_snapshot, native_entity_refs::tests::with_game,
    };

    #[test]
    #[ignore = "daily"]
    fn general_subject_and_host_writes_commit_or_abort_together() {
        crate::game::in_own_process(
            "game::server_side::native_hooks_server::general_entity_tick::tests::general_subject_and_host_writes_commit_or_abort_together",
            || {
                with_game(|game| {
                    game.options.dedicated = Some(Vec::new());
                    game.spawn_item(ItemStack::new("minecraft:stick", 1), [0.0; 3], [0.0; 3], 0);
                    let peer = game
                        .item_entity(game.entity(game.dropped[0].id).unwrap())
                        .unwrap();
                    game.server.next_entity += 1;
                    let id = game.server.next_entity;
                    let epoch = game.native_lifecycle.epoch().unwrap();
                    game.resident_entities.general.rows.push(GeneralRow {
                        id,
                        uuid: [1, 2, 3, 4],
                        join: Default::default(),
                        type_slot: 0,
                        position: Vec3::ZERO,
                        previous: Vec3::ZERO,
                        velocity: Vec3::new(1.0, 2.0, 3.0),
                        rotation: [4.0, 5.0],
                        owner: None,
                        fall_distance: 6.0,
                        payload_version: 1,
                        payload_bytes: Vec::new(),
                        mount: None,
                        vehicle_order: 0,
                        boarding_cooldown: 0,
                        attachment_lifetime: Default::default(),
                    });
                    let stack = test_snapshot(&ItemStack::new("minecraft:stick", 2));
                    for commit in [false, true] {
                        let mut access = GeneralTickAccess {
                            game: &mut *game,
                            subject: EntityRef::new(epoch, id),
                            kind: "test:robot",
                        };
                        let mut scope = EntityServerScope::new(&mut access);
                        let result = scope.transaction(|scope, _| {
                            assert_eq!(
                                scope.with_view(|mut view| view.set_velocity([7.0; 3])),
                                Some(true)
                            );
                            scope
                                .transaction(|scope, _| {
                                    scope
                                        .with_view(|mut view| {
                                            assert!(view.set_velocity([8.0; 3]));
                                            assert!(view.set_rotation(9.0, 10.0));
                                            view.reset_fall_distance();
                                        })
                                        .unwrap();
                                    assert_eq!(
                                        peer.set_stack(scope.host(), stack.clone()),
                                        Ok(true)
                                    );
                                    Ok::<_, ()>(())
                                })
                                .unwrap();
                            commit.then_some(()).ok_or(())
                        });
                        assert_eq!(
                            result,
                            commit
                                .then_some(())
                                .ok_or(mod_api::TransactionError::Aborted(()))
                        );
                        let row = game.resident_entities.general.row(id).unwrap();
                        assert_eq!(
                            (
                                row.velocity.components(),
                                row.rotation,
                                row.fall_distance,
                                game.dropped[0].stack.count
                            ),
                            if commit {
                                ([8.0; 3], [9.0, 10.0], 0.0, 2)
                            } else {
                                ([1.0, 2.0, 3.0], [4.0, 5.0], 6.0, 1)
                            }
                        );
                    }
                })
            },
        );
    }
}
