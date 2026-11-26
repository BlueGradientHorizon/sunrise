

#[cfg(test)]
pub(crate) mod join_order_tests {

use sunrise_gameplay_common::game::server_side::entity_server::join_order_tests::at;

use crate::game::client_side::entity_client::{DrawExtras, DrawFamily, DrawRun, draw_runs};

use crate::game::entity::{EntityHandle, EntityLists, JoinOrderError};

use crate::game::{boats_common::Boat, creatures::render_fixture::mob, fireworks::Rocket};

use sunrise_sim::join_seq::{JoinList, JoinSeq};

use sunrise_sim::mobs::{Mob, Species};

fn runs(
        lists: &Lists,
        local_player: Option<JoinSeq>,
    ) -> Vec<(DrawFamily, std::ops::Range<usize>)> {
        let mut runs = Vec::new();
        draw_runs(
            lists.view(),
            DrawExtras {
                local_player,
                ..Default::default()
            },
            &mut runs,
        );
        runs.into_iter().map(|r| (r.family, r.range)).collect()
    }
/// The ordered draw takes the families' entities in join order, not family by family, and
    /// each run names the list range its family draws.
    #[test]
    fn draw_runs_interleave_families_in_join_order() {
        let mut lists = Lists::default();
        let cow = |id| mob(id, "minecraft:cow", Species::Cow, 0.9, 1.4);
        lists.mobs.push(cow(4));
        lists.mobs.push(cow(3));
        let local_player = JoinSeq::next();
        lists.mobs.push(cow(2));
        lists.trident(6);
        lists.rocket(8);
        lists.stand(5);
        lists.mobs.push(cow(1));
        lists.boat(7);
        lists.trident(9);
        lists.stand(11);
        lists.rocket(10);
        assert_eq!(
            runs(&lists, Some(local_player)),
            [
                (DrawFamily::Mob, 0..2),
                (DrawFamily::LocalPlayer, 0..1),
                (DrawFamily::Mob, 2..3),
                (DrawFamily::Trident, 0..1),
                (DrawFamily::Rocket, 0..1),
                (DrawFamily::ArmorStand, 0..1),
                (DrawFamily::Mob, 3..4),
                (DrawFamily::Boat, 0..1),
                (DrawFamily::Trident, 1..2),
                (DrawFamily::ArmorStand, 1..2),
                (DrawFamily::Rocket, 1..2),
            ]
        );
    }
/// Entities of one family that joined back to back are one run.
    #[test]
    fn draw_runs_join_a_family_that_joined_back_to_back() {
        let mut lists = Lists::default();
        lists.stand(1);
        lists.stand(2);
        lists.boat(3);
        lists.stand(4);
        assert_eq!(
            runs(&lists, None),
            [
                (DrawFamily::ArmorStand, 0..2),
                (DrawFamily::Boat, 0..1),
                (DrawFamily::ArmorStand, 2..3),
            ]
        );
    }
/// Entities of an undrawn family do not split a run.
    #[test]
    fn an_undrawn_family_splits_no_run() {
        use sunrise_sim::projectiles::{Projectile, ProjectileKind};
        let mut lists = Lists::default();
        lists.stand(1);
        let mut projectiles = JoinList::<Projectile, false>::default();
        projectiles.push(Projectile::new(ProjectileKind::DragonFireball, at(), None));
        lists.stand(2);
        let mut draw = Vec::new();
        draw_runs(
            EntityLists {
                projectiles: &projectiles,
                ..lists.view()
            },
            DrawExtras::default(),
            &mut draw,
        );
        assert_eq!(
            draw,
            [DrawRun {
                family: DrawFamily::ArmorStand,
                range: 0..2
            }]
        );
    }
}
