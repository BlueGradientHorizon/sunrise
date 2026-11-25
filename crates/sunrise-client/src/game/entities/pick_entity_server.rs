#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::pick_ray::{
    PickAccumulator, PickCandidate, pick_ray_segment,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::pick_entity_common::{
    AttackRangePickResult, CrosshairDecision, CrosshairEntityHit, PickKind, aabb_extents,
    knot_bounds,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{BlockPos, CrosshairTarget, EntityRef};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{items::ItemStack, save::uuid_text};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::geometry::{Aabb, Vec3};

fn hanging_center((lo, hi): ([f64; 3], [f64; 3])) -> [f64; 3] {
    [
        (lo[0] + hi[0]) * 0.5,
        (lo[1] + hi[1]) * 0.5,
        (lo[2] + hi[2]) * 0.5,
    ]
}

fn knot_position(block: [i32; 3]) -> [f64; 3] {
    let p = sunrise_client::mobs::leash::knot_state(block).position;
    [p.x, p.y, p.z]
}

/// Server-only entity section walk without client stores (`remote`, `client.remote_knots`, `client.interactions`).
pub(in crate::game) fn visit_server_section_members(
    game: &Game<'_>,
    visit: &mut impl FnMut(PickKind, usize, u32, [f64; 3]),
) {
    use crate::game::entity::Entity as _;
    for (slot, e) in game.mobs.iter().enumerate() {
        let p = e.position();
        visit(PickKind::Mob, slot, e.id(), [p.x, p.y, p.z]);
    }
    for (slot, e) in game.boats.iter().enumerate() {
        let p = e.position();
        visit(PickKind::Boat, slot, e.id(), [p.x, p.y, p.z]);
    }
    for (slot, e) in game.carts.iter().enumerate() {
        let p = e.position();
        visit(PickKind::Cart, slot, e.id(), [p.x, p.y, p.z]);
    }
    for (slot, e) in game.armor_stands.iter().enumerate() {
        let p = e.position();
        visit(PickKind::ArmorStand, slot, e.id(), [p.x, p.y, p.z]);
    }
    for (slot, e) in game.tnts.iter().enumerate() {
        let p = e.position();
        visit(PickKind::Tnt, slot, e.id(), [p.x, p.y, p.z]);
    }
    for (slot, e) in game.falling.iter().enumerate() {
        let p = e.position();
        visit(PickKind::Falling, slot, e.id(), [p.x, p.y, p.z]);
    }
    for (slot, e) in game.creatures.projectiles.iter().enumerate() {
        let p = e.position();
        visit(PickKind::Projectile, slot, e.id(), [p.x, p.y, p.z]);
    }
    for (slot, e) in game.tridents.iter().enumerate() {
        let p = e.position();
        visit(PickKind::Trident, slot, e.id(), [p.x, p.y, p.z]);
    }
    for (slot, e) in game.rockets.iter().enumerate() {
        let p = e.position();
        visit(PickKind::Rocket, slot, e.id(), [p.x, p.y, p.z]);
    }
    for (slot, e) in game.resident_entities.general.rows.iter().enumerate() {
        let p = e.position();
        visit(PickKind::General, slot, e.id(), [p.x, p.y, p.z]);
    }
    for (slot, p) in game.paintings.iter().enumerate() {
        visit(PickKind::Painting, slot, p.id, hanging_center(p.bounds()));
    }
    for (slot, f) in game.item_frames.iter().enumerate() {
        visit(PickKind::ItemFrame, slot, f.id, hanging_center(f.bounds()));
    }
    for (slot, c) in game.cushions.iter().enumerate() {
        visit(
            PickKind::Cushion,
            slot,
            c.id,
            [c.position.x, c.position.y, c.position.z],
        );
    }
    for (slot, k) in game.resident_entities.knots.iter().enumerate() {
        visit(PickKind::LocalKnot, slot, k.id, knot_position(k.fence));
    }
    for (slot, c) in game.crystals.iter().enumerate() {
        if c.id != 0 {
            visit(
                PickKind::Crystal,
                slot,
                c.id,
                [c.position.x, c.position.y, c.position.z],
            );
        }
    }
    for (slot, e) in game.dropped.iter().enumerate() {
        visit(PickKind::Item, slot, e.id(), e.position().components());
    }
    for (slot, e) in game.orbs.iter().enumerate() {
        visit(PickKind::Orb, slot, e.id(), e.position().components());
    }
    if let Some(d) = game.dragon.as_ref().filter(|d| d.id != 0) {
        visit(PickKind::Dragon, 0, d.id, d.position.components());
    }
}

impl Game<'_> {
    /// Authoritative server entity hit from `server_pick_ray_decision`.
    pub(in crate::game) fn server_crosshair_entity_hit(&self) -> Option<CrosshairEntityHit> {
        let CrosshairDecision::Entity(hit) = self.server_pick_ray_decision() else {
            return None;
        };
        let is_synthetic_dragon_part =
            (super::dragon::SPECIAL..super::dragon::SPECIAL + 8).contains(&hit.id);
        let id = hit.id as i32;
        let pickable = !is_synthetic_dragon_part
            && crate::game::pick_entity_common::has_pick_result(self.level_entity(id));
        let suggest_uuid = !is_synthetic_dragon_part && self.resident_dragon_part(hit.id).is_none();
        Some(CrosshairEntityHit {
            id,
            distance: hit.distance_sq.sqrt(),
            point: hit.point,
            pickable,
            suggest_uuid,
        })
    }

    pub(in crate::game) fn server_entity_is_taken(&self, id: u32) -> bool {
        matches!(
            self.server_resident_entity(id),
            sunrise_client::join_seq::residency::table::EntityLookup::Taken
        )
    }

    /// The fence of a natural knot or an explicitly summoned/loaded leash knot.
    pub(in crate::game) fn server_pick_knot_fence(&self, id: u32) -> Option<[i32; 3]> {
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
