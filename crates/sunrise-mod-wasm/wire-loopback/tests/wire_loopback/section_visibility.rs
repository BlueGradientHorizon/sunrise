//! `section_visibility` through the generated host/guest wire against the same listener called
//! natively: the guest reads the same raw section (states, `Missing` columns, light) and its
//! answers, a malformed one too, come back as the native listener gives them, so the game checks
//! and combines identical masks on native and wasm.
use super::*;
use mod_api::{
    CellVisibility, SectionCell, SectionVisibility, SectionVisibilityEvent, SectionVisibilityInput,
    SectionVisibilityRefusal,
};

/// Hides state 5, keeps only the up face of state 6 with no fluid, answers a face mask of 64 for
/// state 7 (refused by the game's check), hides all terrain above y 256, and nothing for a section
/// whose west border is not loaded.
fn answer(input: &SectionVisibilityInput) -> Option<SectionVisibility> {
    if input.origin[1] >= 256 {
        return Some(SectionVisibility::HideTerrain);
    }
    if input.cell([-1, 0, 0]) == Ok(SectionCell::Missing) {
        return None;
    }
    let mut cells = Vec::new();
    for y in 0..16u8 {
        for z in 0..16u8 {
            for x in 0..16u8 {
                let local = [i32::from(x), i32::from(y), i32::from(z)];
                let pos = [x, y, z];
                match input.cell(local) {
                    Ok(SectionCell::State(mod_api::BlockStateId(5))) => {
                        cells.push(CellVisibility::hidden(pos))
                    }
                    Ok(SectionCell::State(mod_api::BlockStateId(6))) => {
                        let lit = input.light(local).unwrap()[0] > 7;
                        cells.push(CellVisibility::new(pos, true, false, 2 | u8::from(lit)))
                    }
                    Ok(SectionCell::State(mod_api::BlockStateId(7))) => {
                        cells.push(CellVisibility::new(pos, true, true, 64))
                    }
                    _ => {}
                }
            }
        }
    }
    Some(SectionVisibility::Keep(cells))
}

pub(super) struct SectionVisibilityHooks;
impl SectionVisibilityHooks {
    pub(super) fn section_visibility(
        &self,
        _: &dyn mod_api::ClientHost,
        event: &mut mod_api::SectionVisibilityEvent<'_>,
    ) {
        assert!(
            event.answers.is_empty(),
            "the guest starts from its own batch"
        );
        event.answers.extend(answer(event.input));
    }
}

/// An input whose cells read `state(i)` and light `light(i)`, with the west border missing when
/// `west_missing`.
fn input(origin: [i32; 3], west_missing: bool) -> SectionVisibilityInput {
    let mut cells = Vec::new();
    let mut light = Vec::new();
    for y in -1i32..=16 {
        for z in -1i32..=16 {
            for x in -1i32..=16 {
                let id = ((x * 3 + y * 5 + z * 7).rem_euclid(9)) as u32;
                cells.push(if west_missing && x == -1 {
                    SectionCell::Missing
                } else {
                    SectionCell::State(mod_api::BlockStateId(id))
                });
                light.push(((y.rem_euclid(16) as u8) << 4) | (x.rem_euclid(16) as u8));
            }
        }
    }
    SectionVisibilityInput::new(origin, cells, light).unwrap()
}

#[test]
fn wire_loopback_section_visibility_answers_match_native() {
    mod_api::__private::initialize().unwrap();
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    struct Client;
    impl mod_api::Host for Client {}
    mod_api::impl_host_areas!(Client);
    impl mod_api::ClientHost for Client {}
    mod_api::impl_client_host_areas!(Client);
    install_counter();
    let module = WasmMod {
        group: 0,
        subscribed: &[],
        listeners: &[],
        corrupt: false,
    };
    let mut kinds = Vec::new();
    for (origin, west_missing) in [
        ([16, 0, -32], false),
        ([16, 0, -32], true),
        ([0, 256, 0], false),
    ] {
        let input = input(origin, west_missing);
        let mut answers = Vec::new();
        module.section_visibility(
            &Client,
            &mut SectionVisibilityEvent {
                input: &input,
                answers: &mut answers,
            },
        );
        let native: Vec<_> = answer(&input).into_iter().collect();
        assert_eq!(answers, native, "{origin:?} {west_missing}");
        kinds.push(answers.first().map(SectionVisibility::check));
    }
    assert_eq!(
        kinds,
        [
            Some(Err(SectionVisibilityRefusal::FaceMask {
                pos: [0, 0, 1],
                faces: 64
            })),
            None,
            Some(Ok(())),
        ]
    );
}
