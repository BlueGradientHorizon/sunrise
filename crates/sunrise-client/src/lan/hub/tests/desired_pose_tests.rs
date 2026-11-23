use super::*;
use crate::motion::{MotionWorld, PlayerMotion};
use crate::pose::{Pose, PoseFlags, desired_pose};
use sunrise_core::geometry::Vec3;

/// The poses `Player.getDesiredPose` reads: each flag alone, a spin and a glide while
/// swimming (swimming comes first), a crouch while flying (none) and in bed.
const CASES: [PoseFlags; 8] = [
    PoseFlags {
        sleeping: false,
        swimming: false,
        fall_flying: false,
        auto_spin_attack: false,
        shift_key_down: false,
        flying: false,
    },
    PoseFlags {
        sleeping: true,
        swimming: false,
        fall_flying: false,
        auto_spin_attack: false,
        shift_key_down: true,
        flying: false,
    },
    PoseFlags {
        sleeping: false,
        swimming: true,
        fall_flying: true,
        auto_spin_attack: true,
        shift_key_down: false,
        flying: false,
    },
    PoseFlags {
        sleeping: false,
        swimming: false,
        fall_flying: true,
        auto_spin_attack: true,
        shift_key_down: false,
        flying: false,
    },
    PoseFlags {
        sleeping: false,
        swimming: false,
        fall_flying: false,
        auto_spin_attack: true,
        shift_key_down: true,
        flying: false,
    },
    PoseFlags {
        sleeping: false,
        swimming: false,
        fall_flying: false,
        auto_spin_attack: false,
        shift_key_down: true,
        flying: false,
    },
    PoseFlags {
        sleeping: false,
        swimming: false,
        fall_flying: false,
        auto_spin_attack: false,
        shift_key_down: true,
        flying: true,
    },
    PoseFlags {
        sleeping: false,
        swimming: false,
        fall_flying: false,
        auto_spin_attack: false,
        shift_key_down: false,
        flying: true,
    },
];

/// `Player.getDesiredPose` is answered in one place,
/// [`desired_pose`](crate::pose::desired_pose), and every reader takes its answer:
/// `updatePlayerPose` in the client's prediction (`update_player_pose`) and in the server's
/// copy of a player (`server_player_pose_tick`), under an open sky where every pose fits; the
/// pose the server shows of a guest whose body has not ticked yet (`Guest::pose`); and the
/// pose in the player data the others are sent (`player_state`).
#[test]
#[ignore = "daily"]
fn every_pose_reader_takes_the_desired_pose() {
    let v = crate::test_fixtures::vanilla().expect("prepared resources required");
    let (assets, render, binding, resources) = (&v.assets, &v.render, &v.collision, &v.motion);
    let mut scene = crate::world::World::test_scene(assets, render, 2).unwrap();
    let (mut hub, _rx) = hub_with_a_guest();
    let entity = hub.guests[0].entity_id;
    let mut seen = Vec::new();
    for flags in CASES {
        let desired = desired_pose(entity, flags);
        seen.push(desired);
        for server in [false, true] {
            let mut player = PlayerMotion::new(resources, Vec3::new(20.5, 10.0, 20.5)).unwrap();
            player.entity_id = entity;
            player.sleeping = flags.sleeping;
            player.swimming = flags.swimming;
            player.fall_flying = flags.fall_flying;
            player.auto_spin_attack_ticks = if flags.auto_spin_attack { 2 } else { 0 };
            player.shift_key_down = flags.shift_key_down;
            player.flying = flags.flying;
            let mut w = MotionWorld {
                side: crate::motion::dry_side(),
                tag_items: None,
                block_registries: None,
                loop_context: crate::test_resources::server_loop_context(),
                damage_hooks: &crate::health::DamageHooks::NONE,
                entities: &[],
                player: &mut player,
                scene: &mut scene,
                assets,
                collision_binding: binding,
                resources,
                health: &resources.health,
            };
            if server {
                w.server_player_pose_tick().unwrap();
            } else {
                w.update_player_pose().unwrap();
            }
            assert_eq!(player.pose, desired, "{flags:?}, on the server: {server}");
        }

        // The swim and the spin are the body's: a guest without one shows the rest.
        hub.guests.update(0, |g| {
            g.sleeping = flags.sleeping;
            g.fall_flying = flags.fall_flying;
            g.sneaking = flags.shift_key_down;
            g.abilities.flying = flags.flying;
        });
        let guest = &hub.guests[0];
        assert!(guest.motion.is_none());
        let announced = PoseFlags {
            swimming: false,
            auto_spin_attack: false,
            ..flags
        };
        assert_eq!(guest.pose(), desired_pose(entity, announced), "{flags:?}");

        let body = BodyFlags {
            swimming: flags.swimming,
            gliding: flags.fall_flying,
            spin_attack: flags.auto_spin_attack,
            ..Default::default()
        };
        let state = hub.player_state(
            entity,
            0,
            [0.0; 3],
            (0.0, 0.0, 0.0),
            true,
            (flags.shift_key_down, false, flags.sleeping),
            body,
            0,
            None,
            &guest.info,
            20.0,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let shown = state
            .meta
            .iter()
            .find_map(|(index, _, value)| (*index == 6).then_some(value.clone()));
        // A crouch while flying is still a crouch for the others: the body's
        // crouch under a low ceiling is read the same way.
        let others = desired_pose(
            entity,
            PoseFlags {
                flying: false,
                ..flags
            },
        );
        let expected = (others != Pose::Standing).then_some(Value::Id(others.id()));
        assert_eq!(shown, expected, "{flags:?}");
    }
    assert_eq!(
        seen,
        [
            Pose::Standing,
            Pose::Sleeping,
            Pose::Swimming,
            Pose::FallFlying,
            Pose::SpinAttack,
            Pose::Crouching,
            Pose::Standing,
            Pose::Standing,
        ]
    );
}
