#[allow(unused_imports)]
use crate::game::commands::data::DataRef;
#[allow(unused_imports)]
use crate::game::dragon::Breath;
#[allow(unused_imports)]
use crate::game::native_entity_refs::tests::{
    bind_local_player, prepare_entity_ticking_neighborhood, with_game,
};
#[allow(unused_imports)]
use serde_json::Value;
#[allow(unused_imports)]
use sunrise_client::motion::PlayerMotion;
#[allow(unused_imports)]
use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
use sunrise_core::position::ChunkPos;

pub(in crate::game) fn read_witness() -> String {
    let raw = std::env::var_os("SUNRISE_CLOUD_WATER_WITNESS").unwrap_or_else(|| {
        panic!(
            "SUNRISE_CLOUD_WATER_WITNESS is unset; run through tools/test_cloud_water_parity.py, which records the 26.3 witness"
        )
    });
    let path = std::path::PathBuf::from(raw);
    let file = if path.is_dir() {
        path.join("cloud_water.json")
    } else {
        path
    };
    std::fs::read_to_string(&file).unwrap_or_else(|err| {
        panic!(
            "failed to read {} (run through tools/test_cloud_water_parity.py): {err}",
            file.display()
        )
    })
}

pub(in crate::game) fn offset_coord(base: i32, offset: &str) -> String {
    match offset.split_once('.') {
        Some((whole, frac)) => format!("{}.{frac}", base + whole.parse::<i32>().unwrap()),
        None => (base + offset.parse::<i32>().unwrap()).to_string(),
    }
}

pub(in crate::game) fn parse_double_bits(hex: &str) -> Tag {
    Tag::Double(u64::from_str_radix(hex, 16).unwrap())
}

pub(in crate::game) fn parse_motion_bits(obs: &Value) -> Tag {
    let bits = obs["motion_bits"].as_array().unwrap();
    assert_eq!(
        bits.len(),
        3,
        "Motion witness must have 3 double components"
    );
    Tag::List(
        bits.iter()
            .map(|b| parse_double_bits(b.as_str().unwrap()))
            .collect(),
    )
}

/// #2359 (vanilla 26.3): area_effect_cloud fall_distance and Motion in water,
/// lava, and at the radius-32 boundary match the 26.3 witness recorded by
/// `tools/test_cloud_water_parity.py` bit-for-bit.
#[test]
#[ignore = "run by tools/test_cloud_water_parity.py"]
fn a_cloud_in_water_resets_its_fall_distance_and_a_dry_one_keeps_it() {
    run_a_cloud_in_water_resets_its_fall_distance_and_a_dry_one_keeps_it();
}

#[allow(unused_imports)]
pub(in crate::game) use crate::game::server_side::potions_server::cloud_water_tests::run_a_cloud_in_water_resets_its_fall_distance_and_a_dry_one_keeps_it;
