#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::pick_accumulator::{
    PickAccumulator, PickCandidate, pick_ray_segment,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::pick_entity::AttackRangePickResult;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::pick_entity::CrosshairDecision;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::pick_entity::CrosshairEntityHit;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::pick_entity::aabb_extents;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::pick_entity::knot_bounds;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::pick_entity::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{BlockPos, CrosshairTarget, EntityRef};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{items::ItemStack, save::uuid_text};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::geometry::{Aabb, Vec3};

impl Game<'_> {
    /// The fence of a natural knot or an explicitly summoned/loaded leash knot.
    pub(in crate::game) fn pick_knot_fence(&self, id: u32) -> Option<[i32; 3]> {
        self.natural_knot(id).or_else(|| {
            let knot = self
                .others
                .0
                .iter()
                .find(|o| o.id == id && o.knot_check.is_some())?;
            if self.level_entity(id as i32) != Some(super::commands::Target::Other(id)) {
                return None;
            }
            let block = sunrise_core::position::BlockPos::containing(knot.position);
            Some([block.x, block.y, block.z])
        })
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::Game;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::client_side::pick_entity::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::commands::Target;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::creatures::SpawnSite;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::dragon::dragon_part_id;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_entity_refs::tests::with_game;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_client::{items::ItemStack, motion::PlayerMotion};
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_core::geometry::Vec3;

    #[test]
    #[ignore = "daily"]
    fn server_entity_pick_returns_lead_and_delegates_dragon_parts() {
        crate::game::in_own_process(
            "game::server_side::pick_entity_server::tests::server_entity_pick_returns_lead_and_delegates_dragon_parts",
            || {
                with_game(|game| {
                    game.player =
                        Some(PlayerMotion::new(game.motion, Vec3::new(0.5, 65.0, 0.5)).unwrap());
                    let knot = game.tie_knot([0, 65, 2]);
                    let far_knot = game.tie_knot([0, 65, 10]);
                    let Some(Target::Other(explicit_knot)) = game
                        .summon_with_nbt("minecraft:leash_knot", [1.5, 65.0, 2.5], "{}")
                        .unwrap()
                    else {
                        panic!("expected an explicit leash knot");
                    };
                    for id in [knot, explicit_knot] {
                        assert!(
                            game.within_entity_range(id as i32),
                            "knot {id} is reachable"
                        );
                        assert_eq!(
                            game.entity_pick_result(id as i32),
                            Some(ItemStack::new("minecraft:lead", 1))
                        );
                    }
                    assert!(
                        !game.within_entity_range(far_knot as i32),
                        "far knot outside reach+3 must fail server reach"
                    );
                    // Cross the signed i32 boundary so later dragon parts carry negative wire IDs.
                    game.server.next_entity = (i32::MAX as u32) - 3;
                    let dragon = game
                        .spawn_dragon_facing([0.5, 65.0, 3.5], SpawnSite::Plain, Some(0.0))
                        .unwrap();
                    let parent_result = game.entity_pick_result(dragon as i32);
                    assert_eq!(
                        parent_result,
                        Some(ItemStack::new("minecraft:ender_dragon_spawn_egg", 1))
                    );
                    let body_part_id = dragon_part_id(dragon, 2) as i32;
                    assert!(body_part_id < 0, "expected negative signed part wire ID");
                    assert!(game.within_entity_range(body_part_id));
                    for part in 0..8 {
                        assert_eq!(
                            game.entity_pick_result(dragon_part_id(dragon, part) as i32),
                            parent_result
                        );
                    }

                    // Exercise the real PickItemFromEntity server packet handler through a creative guest.
                    let (mut hub, _rx, _other) =
                        crate::game::lan_particles::tests::test_hub("PickEntity", Vec::new(), None);
                    let gi = hub.guests.connection_index(1).unwrap();
                    hub.guests.update(gi, |g| {
                        g.game_mode = 1;
                        g.position = [0.5, 65.0, 0.5];
                        g.placed = true;
                        g.dimension = "minecraft:overworld".into();
                        g.inventory[0] = None;
                        g.selected = 0;
                    });
                    game.lan = Some(crate::game::lan::LanHost::for_test(hub));

                    for (id, expected) in [
                        (far_knot as i32, None),
                        (knot as i32, Some("minecraft:lead")),
                        (explicit_knot as i32, Some("minecraft:lead")),
                        (body_part_id, Some("minecraft:ender_dragon_spawn_egg")),
                    ] {
                        game.lan.as_mut().unwrap().hub.guests.update(gi, |g| {
                            g.inventory[0] = None;
                            g.selected = 0;
                        });
                        game.lan_guest_packet(
                            1,
                            sunrise_protocol::packets::play::Serverbound::PickItemFromEntity(
                                sunrise_protocol::packets::play::PickItemFromEntity {
                                    id,
                                    include_data: false,
                                },
                            ),
                        );
                        let held = game.lan.as_ref().unwrap().hub.guests[gi].inventory[0]
                            .as_ref()
                            .map(|s| &*s.item);
                        assert_eq!(held, expected, "PickItemFromEntity({id})");
                    }
                });
            },
        );
    }
}
