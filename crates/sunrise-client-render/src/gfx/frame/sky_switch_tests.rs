//! The sun and moon the game draws itself under a shader pack's `sun` and `moon` (spec 3.2).

use super::vanilla_bodies;
use sunrise_shaders::SkySwitches;

const ALL_ON: SkySwitches = SkySwitches {
    sky: true,
    stars: true,
    sun: true,
    moon: true,
};

/// The Overworld's two quads: the sun's (indices 0..6), then the moon's (6..12).
const SUN_AND_MOON: usize = 8;

#[test]
fn both_bodies_draw_by_default() {
    assert_eq!(vanilla_bodies(SUN_AND_MOON, false, false, ALL_ON), 0..12);
}

#[test]
fn sun_false_leaves_only_the_moon() {
    let switches = SkySwitches {
        sun: false,
        ..ALL_ON
    };
    assert_eq!(vanilla_bodies(SUN_AND_MOON, false, false, switches), 6..12);
}

#[test]
fn moon_false_leaves_only_the_sun() {
    let switches = SkySwitches {
        moon: false,
        ..ALL_ON
    };
    assert_eq!(vanilla_bodies(SUN_AND_MOON, false, false, switches), 0..6);
}

#[test]
fn sun_and_moon_false_leave_nothing() {
    let switches = SkySwitches {
        sun: false,
        moon: false,
        ..ALL_ON
    };
    assert_eq!(vanilla_bodies(SUN_AND_MOON, false, false, switches), 6..6);
}

/// The sky and stars switches don't touch the bodies.
#[test]
fn sky_and_stars_false_keep_both_bodies() {
    let switches = SkySwitches {
        sky: false,
        stars: false,
        ..ALL_ON
    };
    assert_eq!(vanilla_bodies(SUN_AND_MOON, false, false, switches), 0..12);
}

/// A pack sky drawing its own sun skips vanilla's, as before the switches.
#[test]
fn an_own_sun_still_skips_the_sun() {
    assert_eq!(vanilla_bodies(SUN_AND_MOON, false, true, ALL_ON), 6..12);
}

/// The End's flash, its one quad, is neither the sun nor the moon.
#[test]
fn the_end_flash_draws_whatever_sun_and_moon_say() {
    let switches = SkySwitches {
        sun: false,
        moon: false,
        ..ALL_ON
    };
    assert_eq!(vanilla_bodies(4, true, false, switches), 0..6);
}
