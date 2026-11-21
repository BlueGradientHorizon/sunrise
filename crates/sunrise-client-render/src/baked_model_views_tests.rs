//! Geometry a mod borrows from a baked model, before resolving its draw-time state.
use super::*;
use crate::models::catalog::BakedFace;
use sunrise_assets::{bake::ModelQuad, model::Material};

fn quad(facing: FaceDirection, cull: Option<FaceDirection>, sprite: &str) -> ModelQuad {
    ModelQuad {
        positions: [
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
        ],
        uv: [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]],
        facing,
        cull,
        material: Material {
            sprite: sprite.into(),
            force_translucent: false,
        },
        tint_index: 0,
        shade_direction_override: None,
        light_emission: 7,
        light: [7, 0],
        color: None,
        ambient_occlusion: None,
        render_type: None,
        shade: false,
    }
}

#[test]
fn a_view_lends_quads_in_bake_order_with_baked_argb_separate_from_tint() {
    let mut top = quad(
        FaceDirection::Up,
        Some(FaceDirection::Up),
        "minecraft:block/stone",
    );
    top.color = Some(0xc012_3456u32 as i32);
    top.tint_index = 3;
    top.render_type = Some(sunrise_assets::bake::RenderType::Translucent);
    let mut west = quad(FaceDirection::West, None, "minecraft:block/dirt");
    west.tint_index = -1;
    let model = BakedModel {
        ambient_occlusion: false,
        particle: Some(3),
        faces: vec![
            BakedFace::new(top, 3, false, true, None),
            BakedFace::new(west, 4, true, false, None),
        ],
        cube: None,
    };
    let particle = String::from("minecraft:block/stone");
    let view = built::baked_model_view(false, Some(&particle), &model);
    assert!(!view.ambient_occlusion);
    let named = view.particle.unwrap();
    assert_eq!(named, "minecraft:block/stone");
    assert!(std::ptr::eq(named, particle.as_str()));

    let mut quads = view.quads();
    let top = quads.next().unwrap();
    assert_eq!(
        top.positions,
        &[
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0]
        ],
    );
    assert_eq!(top.uvs, &[[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]]);
    let baked_top = &model.faces[0].quad;
    assert!(std::ptr::eq(top.positions, &baked_top.positions));
    assert!(std::ptr::eq(top.uvs, &baked_top.uv));
    assert!(std::ptr::eq(top.sprite, baked_top.material.sprite.as_str()));
    assert_eq!(top.sprite, "minecraft:block/stone");
    assert_eq!(top.facing, Direction::Up);
    assert_eq!(top.cull, Some(Direction::Up));
    assert_eq!(top.tint_index, 3);
    assert_eq!(top.baked_argb, Some(0xc012_3456));
    assert!(!top.shade);
    assert_eq!(top.light_emission, 7);
    let west = quads.next().unwrap();
    assert_eq!(west.facing, Direction::West);
    assert_eq!(west.cull, None);
    assert_eq!(west.sprite, "minecraft:block/dirt");
    assert_eq!(west.tint_index, -1);
    assert_eq!(west.baked_argb, None);
    assert!(quads.next().is_none());
}

#[test]
fn quad_iteration_reports_the_remaining_count_and_reads_both_ends() {
    let model = BakedModel {
        ambient_occlusion: true,
        particle: None,
        faces: [
            "fixture:block/first",
            "fixture:block/middle",
            "fixture:block/last",
        ]
        .into_iter()
        .map(|sprite| BakedFace::new(quad(FaceDirection::Up, None, sprite), 0, false, false, None))
        .collect(),
        cube: None,
    };
    let view = built::baked_model_view(true, None, &model);
    let mut quads = view.quads();
    assert_eq!(quads.size_hint(), (3, Some(3)));
    assert_eq!(quads.next_back().unwrap().sprite, "fixture:block/last");
    assert_eq!(quads.len(), 2);
    assert_eq!(quads.next().unwrap().sprite, "fixture:block/first");
    assert_eq!(quads.len(), 1);
    assert_eq!(quads.next_back().unwrap().sprite, "fixture:block/middle");
    assert_eq!(quads.len(), 0);
    assert!(quads.next().is_none());
    assert!(quads.next_back().is_none());
}

#[test]
fn quad_count_and_unequal_lengths_do_not_read_geometry() {
    struct UnreadGeometry(usize);
    impl BakedQuadSource for UnreadGeometry {
        fn len(&self) -> usize {
            self.0
        }

        fn quad(&self, index: usize) -> Option<BakedQuad<'_>> {
            panic!("unexpected geometry lookup at {index}");
        }
    }

    let two_quads = UnreadGeometry(2);
    let three_quads = UnreadGeometry(3);
    let left = built::baked_model_view(true, None, &two_quads);
    let right = built::baked_model_view(true, None, &three_quads);
    assert_eq!(left.quads().count(), 2);
    assert_ne!(left, right);
}

#[test]
fn a_model_can_have_no_quads() {
    let model = BakedModel {
        ambient_occlusion: true,
        particle: None,
        faces: Vec::new(),
        cube: None,
    };
    let view = built::baked_model_view(true, None, &model);
    assert!(view.quads().next().is_none());
}

#[test]
fn baked_model_keys_are_a_sized_sequence_in_one_generation() {
    let keys = built::baked_model_keys(4, &[7, 3]);
    let empty = built::baked_model_keys(4, &[]);
    assert_eq!((keys.len(), keys.is_empty()), (2, false));
    assert_eq!((empty.len(), empty.is_empty()), (0, true));
    assert_eq!(keys, built::baked_model_keys(4, &[7, 3]));
    assert_ne!(keys, built::baked_model_keys(4, &[3, 7]));
    assert_ne!(keys, built::baked_model_keys(5, &[7, 3]));

    let mut iter = keys.into_iter();
    assert_eq!(iter.len(), 2);
    assert_eq!(
        iter.next(),
        Some(BakedModelKey::from(built::model_ref(4, 7)))
    );
    assert_eq!(iter.len(), 1);
    assert_eq!(
        iter.next(),
        Some(BakedModelKey::from(built::model_ref(4, 3)))
    );
    assert_eq!(iter.next(), None);
    assert_eq!(empty.into_iter().next(), None);
}

#[test]
fn every_face_direction_maps_to_the_api_direction_of_the_same_side() {
    for (face, side) in [
        (FaceDirection::Down, Direction::Down),
        (FaceDirection::Up, Direction::Up),
        (FaceDirection::North, Direction::North),
        (FaceDirection::South, Direction::South),
        (FaceDirection::West, Direction::West),
        (FaceDirection::East, Direction::East),
    ] {
        assert_eq!(direction(face), side);
    }
}
