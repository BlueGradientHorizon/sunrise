//! `lightningBoltPosition` (spec 5.2): the newest drawn bolt in player space, w = 1.

use glam::DVec3;

use super::initial_history_tests::frame;
use super::{FrameUniforms, PackInputs, Value, index, zero};
use crate::gfx::lightning::{Bolt, Input};
use crate::hud::HudBatch;

fn bolt_position(u: &FrameUniforms) -> [f32; 4] {
    match u.values[index("lightningBoltPosition").unwrap()] {
        Value::F(v) => [v[0], v[1], v[2], v[3]],
        Value::I(_) => panic!("lightningBoltPosition is a float vector"),
    }
}

/// No bolt reads zeros, w included; a bolt reads its position minus the camera's with w = 1,
/// worked out in doubles: at x = 29 999 000.25 a float can't hold the quarter, so subtracting
/// floats would give 10 or 11, not 10.25. Of two bolts, the one drawn last (the newest) counts.
#[test]
fn the_newest_drawn_bolt_reaches_the_pack_in_player_space() {
    let hud = HudBatch::scratch([800, 450]);
    let mut f = frame(&hud);
    f.camera = DVec3::new(29_999_000.25, 64.5, -30.75);
    let mut u = FrameUniforms::default();
    u.update(&f, &PackInputs::default(), [800, 450]);
    assert_eq!(bolt_position(&u), [0.0; 4]);

    let bolts = [
        Bolt {
            at: [29_999_010.5, 63.0, -20.25],
            seed: 7,
        },
        Bolt {
            at: [29_998_990.0, 70.25, -40.5],
            seed: 8,
        },
    ];
    f.lightning = Input {
        bolts: &bolts[..1],
        uv: [0.0; 2],
    };
    u.update(&f, &PackInputs::default(), [800, 450]);
    assert_eq!(bolt_position(&u), [10.25, -1.5, 10.5, 1.0]);

    f.lightning.bolts = &bolts;
    u.update(&f, &PackInputs::default(), [800, 450]);
    assert_eq!(bolt_position(&u), [-10.25, 5.75, -9.75, 1.0]);

    // Gone again: zeros.
    f.lightning.bolts = &[];
    u.update(&f, &PackInputs::default(), [800, 450]);
    assert_eq!(bolt_position(&u), [0.0; 4]);
}

/// It is a `vec4`: a program declaring it so reads it, one declaring a `vec3` reads 0 as for
/// the other built-ins, and custom uniforms see four components before the first frame.
#[test]
fn the_bolt_position_is_a_vec4() {
    let v = Value::f(&[1.0, 2.0, 3.0, 1.0]);
    assert!(v.read_as("lightningBoltPosition", "vec4"));
    assert!(!v.read_as("lightningBoltPosition", "vec3"));
    assert_eq!(zero("lightningBoltPosition").unwrap().n, 4);
}
