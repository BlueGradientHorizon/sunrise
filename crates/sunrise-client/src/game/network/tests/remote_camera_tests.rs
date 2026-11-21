//! `ClientboundSetCameraPacket` (`ClientPacketListener.handleSetCamera`) and
//! `Camera.alignWithEntity`: a server's camera follows any entity the level
//! holds, not only mobs and armor stands.
use crate::game::boats::Boat;
use crate::game::remote_camera::CameraEntities;
use crate::game::tnt::PrimedTnt;
use sunrise_client::entities::ItemFrame;
use sunrise_core::geometry::Vec3;

fn boat(id: u32) -> Boat {
    Boat::new(id, "oak".into(), Vec3::new(10.5, 64.0, -3.5), 90.0)
}

fn frame(id: u32, pos: [i32; 3], facing: u8) -> ItemFrame {
    ItemFrame {
        id,
        pos,
        facing,
        glow: false,
        item: None,
        rotation: 0,
        drop_chance: 1.0,
        fixed: false,
        invisible: false,
        check_interval: 0,
        ticks: 0,
        attachment_lifetime: Default::default(),
        join: Default::default(),
        custom_name: None,
        random: sunrise_core::random::LegacyRandom::new(0),
    }
}

fn tnt(id: u32) -> PrimedTnt {
    let at = Vec3::new(4.5, 64.0, 7.5);
    PrimedTnt {
        rotation: [135.0, 0.0],
        join: Default::default(),
        attachment_lifetime: Default::default(),
        id,
        used_portal: false,
        position: at,
        previous: at,
        velocity: Vec3::ZERO,
        fuse: 80,
        ticks: 0,
        ground: Default::default(),
        fall_distance: 0.0,
        stuck: Vec3::ZERO,
        in_water: false,
        owner: None,
        guest: None,
        no_gravity: false,
        power: 4.0,
        block_state: None,
    }
}

/// `ClientboundSetCameraPacket.getEntity` (`Level.getEntity`): the level holds
/// the id whatever kind of entity it names, and `Camera.alignWithEntity` reads
/// that entity — a boat's interpolated position, view rotation and
/// `EntityType.BOAT`'s eyes at 0.5625, with no living entity's scale; a
/// frame's position and zero eyes; primed TNT's 0.15.
#[test]
fn a_server_camera_takes_any_entity_the_level_holds() {
    let mut boat = boat(9);
    boat.previous = Vec3::new(10.0, 64.0, -3.5);
    boat.previous_yaw = 45.0;
    let frame = frame(11, [2, 64, -5], 3);
    let tnt = tnt(13);
    let entities = CameraEntities {
        mobs: &[],
        stands: &[],
        boats: std::slice::from_ref(&boat),
        frames: std::slice::from_ref(&frame),
        tnts: std::slice::from_ref(&tnt),
        vehicles: &std::collections::HashMap::new(),
        player: None,
        living: Box::new(|_| (1.0, 4.0)),
    };

    let view = entities.view(9).expect("a boat is a camera");
    assert_eq!(view.position, [10.5, 64.0, -3.5]);
    assert_eq!(view.previous, [10.0, 64.0, -3.5]);
    assert_eq!((view.y_rot_o, view.y_rot), (45.0, 90.0));
    assert_eq!(view.eye_height, 0.5625);
    assert!(view.living.is_none(), "a boat has no camera distance");

    let view = entities.view(11).expect("an item frame is a camera");
    // `ItemFrame.createBoundingBox`: the block's centre pulled 0.46875 out of the
    // wall it hangs on (south: z - 0.46875), which `HangingEntity` makes its pos.
    assert_eq!(view.position, [2.5, 64.5, -4.96875]);
    assert_eq!((view.eye_height, view.y_rot), (0.0, 0.0));

    let view = entities.view(13).expect("primed TNT is a camera");
    assert_eq!(view.position, [4.5, 64.0, 7.5]);
    assert_eq!((view.eye_height, view.y_rot), (0.15, 135.0));

    // An id the level does not hold changes nothing.
    assert!(!entities.holds(10));
    assert!(entities.view(10).is_none());
}

/// `HangingEntity.setDirection`: a frame turns `get2DDataValue() * 90` in yaw
/// (west 90, north 180, east 270), and on the ceiling or the floor
/// `-90 * getAxisDirection().getStep()` in pitch with no yaw.
#[test]
fn a_frame_camera_turns_with_its_facing() {
    let view = |facing| {
        let frame = frame(1, [0; 3], facing);
        let entities = CameraEntities {
            mobs: &[],
            stands: &[],
            boats: &[],
            frames: std::slice::from_ref(&frame),
            tnts: &[],
            vehicles: &std::collections::HashMap::new(),
            player: None,
            living: Box::new(|_| (1.0, 4.0)),
        };
        let view = entities.view(1).unwrap();
        (view.y_rot, view.x_rot)
    };
    assert_eq!(view(4), (90.0, 0.0));
    assert_eq!(view(2), (180.0, 0.0));
    assert_eq!(view(5), (270.0, 0.0));
    assert_eq!(view(1), (0.0, -90.0));
    assert_eq!(view(0), (0.0, 90.0));
}

/// Minecraft 26.3: the real `Camera.alignWithEntity` (first person) on a real
/// sleeping `Villager` and `RemotePlayer` at each bed facing, recorded by
/// `tools/record_remote_camera_sleeping.py`: the view turns to the bed
/// (`toYRot() - 180`, pitch 0) and drops 0.15.
#[test]
#[ignore = "daily"]
fn a_spectated_sleeping_villager_shows_the_bed_view() {
    use super::mob_camera_entity;
    use sunrise_client::camera::{Camera, CameraType};

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/game/network/tests/fixtures/remote_camera_sleeping");
    let cases = std::fs::read_to_string(dir.join("cases.jsonl")).unwrap();
    let vanilla = std::fs::read_to_string(dir.join("vanilla.txt")).unwrap();
    let frames: Vec<&str> = vanilla.lines().filter(|l| l.starts_with("frame ")).collect();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.local/sunrise-client")
        .join(sunrise_core::VANILLA_VERSION);
    let types = sunrise_client::mobs::MobTypes::load(&root).unwrap();
    let player_kind = crate::game::perspective::player_type(&root).expect("player mob type");
    let bits = |v: &serde_json::Value| u64::from_str_radix(v.as_str().unwrap(), 16).unwrap();
    let mut checked = 0;
    for (case, frame) in cases.lines().zip(&frames) {
        let case: serde_json::Value = serde_json::from_str(case).unwrap();
        let ops = case["ops"].as_array().unwrap();
        let s = &ops[0][1];
        let f = |v: &serde_json::Value| f32::from_bits(bits(v) as u32);
        let d = |v: &serde_json::Value| f64::from_bits(bits(v));
        let v3 = |v: &serde_json::Value| Vec3::new(d(&v[0]), d(&v[1]), d(&v[2]));
        let is_player = s["kind"] == "remote_player";
        let (id, kind) = if is_player {
            ("minecraft:player", player_kind.clone())
        } else {
            (
                "minecraft:villager",
                std::sync::Arc::new(types.types["minecraft:villager"].clone()),
            )
        };
        let mut mob = sunrise_client::mobs::Mob::new(1, kind, v3(&s["pos"]), 0.0);
        mob.previous = v3(&s["prev"]);
        mob.head_pitch = f(&s["xr"]);
        mob.head_yaw = f(&s["yr"]);
        mob.previous_head_pitch = f(&s["xro"]);
        mob.previous_head_yaw = f(&s["yro"]);
        if is_player {
            mob.extra.avatar_pose = sunrise_client::pose::Pose::Sleeping;
        }
        // Villager.sleeping: the bed's facing code (1 north, 2 south, 3 west, 4 east) from Direction.get2DDataValue.
        mob.extra.ints[1] = [2, 3, 1, 4][case["bed"][3].as_u64().unwrap() as usize];
        let entity = mob_camera_entity(&mob, (1.0, 4.0));
        let mut camera = Camera::default();
        camera.tick(Some(entity.eye_height), 1.0);
        camera.align_with_entity(&entity, f(&ops[2][1]), CameraType::FirstPerson, |_, _| None);
        let q = camera.rotation;
        let got = format!(
            "frame {:016x} {:016x} {:016x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x}",
            camera.position[0].to_bits(), camera.position[1].to_bits(), camera.position[2].to_bits(),
            camera.x_rot.to_bits(), camera.y_rot.to_bits(), q.x.to_bits(), q.y.to_bits(), q.z.to_bits(), q.w.to_bits(),
        );
        let want: Vec<&str> = frame.split(' ').take(10).collect();
        assert_eq!(got, want.join(" "), "{id} bed {} case {checked}", case["bed"][3]);
        checked += 1;
    }
    assert_eq!(checked, 40);
}
