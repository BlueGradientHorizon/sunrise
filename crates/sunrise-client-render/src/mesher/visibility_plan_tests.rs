//! The mesh visibility design's plan rules that need no resources: combining answers, the
//! border from the neighbours, the cells that stop occluding, the raw input and the no-mod path.
use super::super::{FluidState, Info, Padded, SectionMesh, Tint, pi};
use super::*;
use crate::chunk::{Chunk, MIN_Y};

fn info(block_entity: bool, emission: u8) -> Info {
    Info {
        solid: true,
        full: true,
        permeable: false,
        shade: 1.0,
        emission,
        tier: 0,
        emissive: false,
        fluid: FluidState::EMPTY,
        model: true,
        block_entity,
        tints: [Tint::None; 2],
        tint_count: 0,
        colormap: None,
        offset: None,
        legacy_solid: false,
        overlay: false,
        leaves: false,
        pane: false,
        opaque_cube: true,
        blocks_flow: true,
        ice: false,
        opaque_ice: false,
        sturdy: 0,
        face: [0; 6],
        cover: [0.0; 6],
    }
}

fn answer(edit: impl FnOnce(&mut SectionPlanBuilder)) -> Option<SectionAnswer> {
    let mut builder = SectionPlanBuilder::new();
    edit(&mut builder);
    builder.finish()
}

#[test]
fn answers_that_change_nothing_keep_the_no_plan_path() {
    assert_eq!(answer(|_| {}), None);
    assert_eq!(answer(|b| b.restrict([3, 4, 5], true, true, 63)), None);
    assert!(plan_section(None, |_| None).is_none());
    let none = answer(|_| {});
    assert!(plan_section(none.as_ref(), |_| None).is_none());
}

#[test]
fn answers_combine_by_intersection_and_never_restore() {
    let combined = answer(|b| {
        b.restrict([1, 2, 3], true, true, 0b11_1101);
        // A later mod keeps everything: the up face stays removed.
        b.restrict([1, 2, 3], true, true, 63);
        b.restrict([1, 2, 3], false, true, 0b11_1110);
    })
    .unwrap();
    let plan = plan_section(Some(&combined), |_| None).unwrap();
    assert_eq!(plan.kept([1, 2, 3]), FLUID | 0b11_1100);
    assert_eq!(plan.kept([0, 0, 0]), ALL);
    assert!(!plan.hides_terrain());
}

#[test]
fn hide_terrain_wins_over_cell_answers() {
    let hidden = answer(|b| {
        b.restrict([0, 0, 0], true, true, 1);
        b.hide_terrain();
    })
    .unwrap();
    assert!(hidden.hides_terrain());
    assert!(plan_section(Some(&hidden), |_| None).unwrap().hides_terrain());
}

#[test]
fn the_border_reads_the_neighbours_answers() {
    // The east neighbour hides its cell (0, 4, 7); the section above hides all its terrain.
    let east = answer(|b| b.restrict([0, 4, 7], false, false, 0)).unwrap();
    let above = answer(SectionPlanBuilder::hide_terrain).unwrap();
    let plan = plan_section(None, |d| match d {
        [1, 0, 0] => Some(&east),
        [0, 1, 0] => Some(&above),
        _ => None,
    })
    .unwrap();
    assert!(!plan.hides_terrain());
    assert_eq!(plan.kept([16, 4, 7]), 0);
    assert_eq!(plan.kept([16, 4, 8]), ALL);
    assert_eq!(plan.kept([5, 16, 9]), 0, "a hidden section above occludes nothing");
    assert_eq!(plan.kept([16, 16, 7]), ALL, "the diagonal above-east has no answer");
    for local in [[0, 4, 7], [15, 4, 7], [5, 15, 9]] {
        assert_eq!(plan.kept(local), ALL, "{local:?}: the section's own cells");
    }
}

#[test]
fn only_outer_cells_change_the_neighbours() {
    let inner = answer(|b| b.restrict([7, 7, 7], false, false, 0)).unwrap();
    let outer = answer(|b| b.restrict([15, 7, 7], false, false, 0)).unwrap();
    let corner = answer(|b| b.restrict([0, 0, 0], true, true, 0)).unwrap();
    assert!(!SectionAnswer::border_differs(Some(&inner), None));
    assert!(SectionAnswer::border_differs(Some(&outer), None));
    assert!(SectionAnswer::border_differs(None, Some(&corner)));
    assert!(SectionAnswer::border_differs(Some(&outer), Some(&corner)));
    assert!(!SectionAnswer::border_differs(Some(&outer), Some(&outer.clone())));
}

#[test]
fn a_cell_without_model_or_fluid_turns_to_air_and_keeps_its_block_entity() {
    let hidden = answer(|b| {
        b.restrict([2, 3, 4], false, false, 63);
        b.restrict([5, 5, 5], false, true, 63);
    })
    .unwrap();
    let east = answer(|b| b.restrict([0, 0, 0], false, false, 0)).unwrap();
    let plan = plan_section(Some(&hidden), |d| (d == [1, 0, 0]).then_some(&east)).unwrap();
    let mut pad = Padded {
        blocks: vec![1; P * P * P],
        light: Vec::new(),
    };
    pad.blocks[pi(2, 3, 4)] = 2;
    let infos = [info(false, 0), info(false, 0), info(true, 7)];
    let mut out = SectionMesh::default();
    substitute(&plan, &mut pad, &infos, &mut out, [1, MIN_Y, 2]);
    assert_eq!(pad.blocks[pi(2, 3, 4)], 0);
    assert_eq!(pad.blocks[pi(16, 0, 0)], 0, "the neighbour's hidden cell");
    assert_eq!(pad.blocks[pi(5, 5, 5)], 1, "a fluid still drawn keeps the cell");
    assert_eq!(pad.blocks[pi(17, 0, 0)], 1, "beyond the one-block border");
    let world = [16 + 2, MIN_Y + 3, 32 + 4];
    assert_eq!(out.block_entities, [(world, 2)]);
    assert_eq!(out.lights, [(world, 2)]);
}

#[test]
fn a_cell_with_anything_removed_stops_being_solid() {
    let partial = answer(|b| b.restrict([4, 0, 15], true, true, 0b11_1110)).unwrap();
    let plan = plan_section(Some(&partial), |_| None).unwrap();
    let mut rows = vec![u32::MAX; P * P];
    open_masked(&plan, &mut rows);
    let row = |y: i32, z: i32| rows[((y + B) as usize) * P + (z + B) as usize];
    assert_eq!(row(0, 15), u32::MAX & !(1 << (4 + B)));
    assert_eq!(row(0, 14), u32::MAX);
    assert!(occludes(ALL, 3));
    assert!(!occludes(ALL & !MODEL, 3), "a model not drawn occludes nothing");
    assert!(!occludes(ALL & !(1 << 3), 3), "a removed face occludes nothing");
    assert!(occludes(ALL & !(1 << 2), 3));
    let mut visible = [true; 6];
    keep_faces(0b11_1101, &mut visible);
    assert_eq!(visible, [true, false, true, true, true, true]);
    let mut culled = [false; 6];
    keep_faces(63, &mut culled);
    assert_eq!(culled, [false; 6], "a mask never shows a culled face");
}

#[test]
fn the_no_mod_masks_keep_everything() {
    const { assert!(!NoMasks::ACTIVE) };
    assert_eq!(NoMasks.at(0), ALL);
    assert_eq!(NoMasks.at(P * P * P - 1), ALL);
}

#[test]
fn the_raw_input_reads_the_world_and_names_missing_columns() {
    let mut chunk = Chunk::empty(0, 0);
    chunk.set(15, MIN_Y + 16 + 15, 15, 7);
    let mut east = Chunk::empty(1, 0);
    east.set(0, MIN_Y + 16, 0, 9);
    // The column and its east neighbour loaded; the rest missing.
    let mut region: [[Option<&Chunk>; 3]; 3] = [[None; 3]; 3];
    region[1][1] = Some(&chunk);
    region[1][2] = Some(&east);
    let (mut cells, mut light) = (Vec::new(), Vec::new());
    capture_input(&region, 1, &mut cells, &mut light);
    assert_eq!(cells.len(), mod_api::SECTION_INPUT_CELLS);
    assert_eq!(light.len(), mod_api::SECTION_INPUT_CELLS);
    let at = |local: [i32; 3]| {
        cells[mod_api::SectionVisibilityInput::index(local).unwrap()]
    };
    // The state is read as stored: a state a mod hides from the mesher still reads as itself.
    assert_eq!(at([15, 15, 15]), Some(7));
    assert_eq!(at([16, 0, 0]), Some(9));
    assert_eq!(at([0, 0, 0]), Some(0));
    assert_eq!(at([-1, 0, 0]), None, "the west column is missing, not air");
    assert_eq!(at([16, 0, 16]), None, "the south-east diagonal is missing");
}

#[test]
fn the_api_checks_answers_whole() {
    use mod_api::{CellVisibility, SectionVisibility as V, SectionVisibilityRefusal as R};
    let cell = |pos, faces| CellVisibility::new(pos, true, true, faces);
    assert_eq!(V::Keep(vec![cell([0, 0, 0], 63)]).check(), Ok(()));
    assert_eq!(V::HideTerrain.check(), Ok(()));
    assert_eq!(
        V::Keep(vec![cell([1, 1, 1], 61), cell([2, 2, 2], 64)]).check(),
        Err(R::FaceMask {
            pos: [2, 2, 2],
            faces: 64
        })
    );
    assert_eq!(
        V::Keep(vec![cell([1, 1, 1], 0), cell([1, 1, 1], 1)]).check(),
        Err(R::Duplicate { pos: [1, 1, 1] })
    );
    assert_eq!(
        V::Keep(vec![cell([16, 0, 0], 0)]).check(),
        Err(R::OutsideSection { pos: [16, 0, 0] })
    );
    let mut many: Vec<_> = (0..4096u32)
        .map(|i| cell([(i & 15) as u8, (i >> 8) as u8, (i >> 4 & 15) as u8], 0))
        .collect();
    assert_eq!(V::Keep(many.clone()).check(), Ok(()));
    many.push(cell([0, 0, 0], 0));
    assert_eq!(
        V::Keep(many).check(),
        Err(R::TooManyCells { count: 4097 })
    );
    assert_eq!(
        mod_api::SectionVisibilityInput::index([17, 0, 0]),
        Err(R::OutsideInput)
    );
    assert_eq!(
        mod_api::SectionVisibilityInput::new([0; 3], vec![], vec![]),
        Err(R::InputSize)
    );
}
