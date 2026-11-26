//! #1340: the general rows' base `Entity` physics, run on plain rows: `Entity.commonTick` and
//! `baseTick` through [`GeneralRow::base_tick`] with a stand-in for the level's fluid scan, and
//! the end of `Entity.move` through [`GeneralRow::finish_move`]. No game, world or resources.
use super::*;
use sunrise_client::mobs::living::Fluids;

fn row() -> GeneralRow {
    GeneralRow {
        id: 1,
        uuid: [1, 2, 3, 4],
        join: JoinSeq::UNJOINED,
        type_slot: 0,
        position: Vec3::new(0.5, 64.0, 0.5),
        previous: Vec3::new(0.5, 64.0, 0.5),
        velocity: Vec3::ZERO,
        rotation: [10.0, 20.0],
        previous_rotation: [10.0, 20.0],
        owner: None,
        fall_distance: 0.0,
        custom_name: None,
        custom_name_visible: false,
        no_gravity: false,
        on_ground: false,
        tick_count: 0,
        no_physics: false,
        horizontal_collision: false,
        vertical_collision: false,
        vertical_collision_below: false,
        fluids: Fluids::default(),
        under_water: false,
        payload_version: 1,
        payload_bytes: Vec::new(),
        mount: None,
        vehicle_order: 0,
        boarding_cooldown: 0,
        attachment_lifetime: Default::default(),
    }
}

const MIN_Y: i32 = -64;

/// A scan that finds nothing and leaves the velocity alone.
fn dry(_: Aabb, _: Vec3, _: f64, _: &mut Vec3) -> Fluids {
    Fluids::default()
}

/// A scan that finds water around the row, its eye in it, and pushes the velocity as a current.
fn water(_: Aabb, _: Vec3, _: f64, velocity: &mut Vec3) -> Fluids {
    velocity.x += 0.014;
    Fluids {
        water: 0.9,
        in_water: true,
        eye_in_water: true,
        ..Fluids::default()
    }
}

/// A scan that finds lava around the row and pushes the velocity as a current.
fn lava(_: Aabb, _: Vec3, _: f64, velocity: &mut Vec3) -> Fluids {
    velocity.z += 0.007;
    Fluids {
        lava: 0.5,
        in_lava: true,
        ..Fluids::default()
    }
}

/// `Entity.commonTick`: the tick count goes up by one and wraps as Java's `int` does, the old
/// position and rotation are the ones the tick began with, and the boarding cooldown counts
/// down to zero and no further.
#[test]
fn the_base_tick_counts_remembers_and_cools_down() {
    let mut row = row();
    row.tick_count = i32::MAX;
    row.boarding_cooldown = 1;
    row.position = Vec3::new(3.0, 70.0, 4.0);
    row.rotation = [30.0, 40.0];
    assert!(row.base_tick(false, MIN_Y, dry));
    assert_eq!(row.tick_count, i32::MIN);
    assert_eq!(row.previous, Vec3::new(3.0, 70.0, 4.0));
    assert_eq!(row.previous_rotation, [30.0, 40.0]);
    assert_eq!(row.boarding_cooldown, 0);
    assert!(row.base_tick(false, MIN_Y, dry));
    assert_eq!(row.tick_count, i32::MIN + 1);
    assert_eq!(row.boarding_cooldown, 0);
}

/// A passenger's velocity is zero when its tick begins, and no current pushes it.
#[test]
fn a_riding_row_has_no_velocity_and_no_current() {
    let mut row = row();
    row.velocity = Vec3::new(1.0, 2.0, 3.0);
    assert!(row.base_tick(true, MIN_Y, water));
    assert_eq!(row.velocity, Vec3::ZERO);
    assert!(row.fluids.in_water, "its fluids are still measured");
}

/// `updateFluidInteraction`: a current in water pushes the velocity, water resets the fall and
/// the row reads as in water; the eye in water makes it under water on the tick after.
#[test]
fn water_pushes_the_row_and_resets_its_fall() {
    let mut row = row();
    row.fall_distance = 5.0;
    assert!(row.base_tick(false, MIN_Y, water));
    assert!((row.velocity.x - 0.014).abs() < 1.0e-12);
    assert_eq!(row.fall_distance, 0.0);
    assert!(row.fluids.in_water && !row.fluids.in_lava);
    assert_eq!(row.fluids.water, 0.9);
    assert!(
        !row.under_water,
        "wasEyeInWater is the sample taken before this tick's scan"
    );
    assert!(row.base_tick(false, MIN_Y, water));
    assert!(row.under_water);
    assert!((row.velocity.x - 0.028).abs() < 1.0e-12);
    assert!(row.base_tick(false, MIN_Y, dry));
    assert!(!row.fluids.in_water);
    assert!(
        !row.under_water,
        "out of the water it is no longer under it"
    );
}

/// The current is `Entity.isPushedByFluid`'s: a `noPhysics` row is measured but not pushed.
#[test]
fn a_no_physics_row_is_not_pushed_by_a_current() {
    let mut row = row();
    row.no_physics = true;
    row.velocity = Vec3::new(0.0, 1.0, 0.0);
    assert!(row.base_tick(false, MIN_Y, water));
    assert_eq!(row.velocity, Vec3::new(0.0, 1.0, 0.0));
    assert!(row.fluids.in_water);
}

/// `Entity.baseTick`: lava halves the fall each tick, except on the first, where `isInLava` is
/// false (`firstTick`).
#[test]
fn lava_halves_the_fall_but_not_on_the_first_tick() {
    let mut row = row();
    row.fall_distance = 8.0;
    assert!(row.base_tick(false, MIN_Y, lava));
    assert_eq!(row.fall_distance, 8.0);
    assert!(!row.fluids.in_lava);
    assert!((row.velocity.z - 0.007).abs() < 1.0e-12);
    assert!(row.base_tick(false, MIN_Y, lava));
    assert_eq!(row.fall_distance, 4.0);
    assert!(row.fluids.in_lava);
    assert!(row.base_tick(false, MIN_Y, lava));
    assert_eq!(row.fall_distance, 2.0);
}

/// `Entity.checkBelowWorld`: below `min_y - 64` the row is discarded, at it it is not.
#[test]
fn a_row_below_the_world_is_discarded() {
    let mut row = row();
    row.position.y = f64::from(MIN_Y - 64);
    assert!(row.base_tick(false, MIN_Y, dry));
    row.position.y = f64::from(MIN_Y - 64) - 0.001;
    assert!(!row.base_tick(false, MIN_Y, dry));
}

/// The base tick applies no gravity and makes no move: what the type's own tick does is its own.
#[test]
fn the_base_tick_neither_falls_nor_moves() {
    let mut row = row();
    row.velocity = Vec3::new(0.2, -0.3, 0.1);
    row.position = Vec3::new(1.0, 80.0, 1.0);
    assert!(row.base_tick(false, MIN_Y, dry));
    assert_eq!(row.velocity, Vec3::new(0.2, -0.3, 0.1));
    assert_eq!(row.position, Vec3::new(1.0, 80.0, 1.0));
}

/// A downward move stopped by the floor: the row stands, has collided vertically below, loses
/// only its vertical speed, and its fall (accumulated by the distance moved) is forgotten.
#[test]
fn landing_grounds_the_row_and_forgets_its_fall() {
    let mut row = row();
    row.fall_distance = 3.0;
    row.velocity = Vec3::new(0.1, -0.5, 0.2);
    row.finish_move(Vec3::new(0.1, -0.5, 0.2), Vec3::new(0.1, -0.25, 0.2));
    assert!(row.on_ground && row.vertical_collision && row.vertical_collision_below);
    assert!(!row.horizontal_collision);
    assert_eq!(row.velocity, Vec3::new(0.1, 0.0, 0.2));
    assert_eq!(row.fall_distance, 0.0);
}

/// A downward move in the air adds its distance to the fall; one in water does not.
#[test]
fn a_fall_adds_up_outside_water_only() {
    let mut row = row();
    row.on_ground = true;
    let fall = Vec3::new(0.0, -0.5, 0.0);
    row.finish_move(fall, fall);
    assert!(!row.on_ground && !row.vertical_collision && !row.vertical_collision_below);
    assert_eq!(row.fall_distance, 0.5);
    row.finish_move(fall, fall);
    assert_eq!(row.fall_distance, 1.0);
    row.fluids.in_water = true;
    row.finish_move(fall, fall);
    assert_eq!(row.fall_distance, 1.0);
    row.finish_move(Vec3::new(0.0, 0.25, 0.0), Vec3::new(0.0, 0.25, 0.0));
    assert_eq!(row.fall_distance, 1.0, "a rise never counts");
}

/// A wall stops one horizontal axis: only that axis of the velocity is lost.
#[test]
fn a_wall_zeroes_the_blocked_horizontal_axis() {
    let mut row = row();
    row.velocity = Vec3::new(0.5, 0.25, 0.5);
    row.finish_move(Vec3::new(0.5, 0.0, 0.5), Vec3::new(0.1, 0.0, 0.5));
    assert!(row.horizontal_collision);
    assert!(!row.vertical_collision);
    assert_eq!(row.velocity, Vec3::new(0.0, 0.25, 0.5));
    row.finish_move(Vec3::new(0.5, 0.0, 0.5), Vec3::new(0.5, 0.0, 0.0));
    assert_eq!(row.velocity, Vec3::new(0.0, 0.25, 0.0));
    row.finish_move(Vec3::new(0.5, 0.0, 0.5), Vec3::new(0.5, 0.0, 0.5));
    assert!(!row.horizontal_collision);
}

/// A ceiling is a vertical collision, but not one below: the row does not stand, and its
/// upward speed is lost.
#[test]
fn a_ceiling_blocks_the_rise_without_grounding_the_row() {
    let mut row = row();
    row.on_ground = true;
    row.velocity = Vec3::new(0.0, 0.5, 0.0);
    row.finish_move(Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.0, 0.125, 0.0));
    assert!(row.vertical_collision);
    assert!(!row.vertical_collision_below);
    assert!(!row.on_ground);
    assert_eq!(row.velocity.y, 0.0);
}

/// `noPhysics` skips the end of the move: no flag, no fall and no velocity changes.
#[test]
fn a_no_physics_row_skips_the_end_of_the_move() {
    let mut row = row();
    row.no_physics = true;
    row.velocity = Vec3::new(0.5, -0.5, 0.5);
    row.finish_move(Vec3::new(0.5, -0.5, 0.5), Vec3::ZERO);
    assert!(!row.horizontal_collision && !row.vertical_collision && !row.on_ground);
    assert_eq!(row.fall_distance, 0.0);
    assert_eq!(row.velocity, Vec3::new(0.5, -0.5, 0.5));
}

/// A `noPhysics` row's placement clears the flags a move before it left, as `Entity.move` does.
#[test]
fn a_no_physics_placement_clears_the_collision_flags() {
    let mut row = row();
    row.horizontal_collision = true;
    row.vertical_collision = true;
    row.vertical_collision_below = true;
    row.no_physics = true;
    let mut gate = MoveGate::new();
    gate.arm(
        1,
        sunrise_world::entity_move::MoverType::SelfMovement,
        Vec3::new(1.0, 0.0, 0.0),
    );
    row.move_entity(&mut gate, Vec3::new(1.0, 0.0, 0.0));
    assert_eq!(row.position, Vec3::new(1.5, 64.0, 0.5));
    assert!(!row.horizontal_collision && !row.vertical_collision && !row.vertical_collision_below);
}
