#![cfg(test)]
//! `DrawContext::fill_gradient` and `GuiCanvas::fill_gradient` through the game's generated
//! dispatch: both draw with the engine's `HudBatch::fill_gradient` in vanilla's colour order,
//! inside the enclosing pose and scissor. Zero area, one-axis inversion and non-finite input
//! draw nothing (`GuiCanvas::fill_gradient` refuses non-finite geometry); both axes inverted
//! keeps the colours on their input y edges.
use mod_api::client::hud_layout::PoseOp;
use mod_api::{ComponentHoverEvent, GuiDrawError, Hooks, ModEntry};
use mod_platform::testing::with_mods;
use sunrise_client::hud::HudBatch;
use sunrise_client::hud_layout::abgr;
use sunrise_client::text::{Style, Text};

const TOP: u32 = 0x8000_FF00;
const BOTTOM: u32 = 0xFF00_00FF;

/// Draws empty, clipped, inverted and posed gradients on both `DrawContext` and `GuiCanvas`.
struct Gradients;
#[mod_api::__subscriptions]
impl Hooks for Gradients {
    fn component_hover(
        &self,
        _: &dyn mod_api::ClientHost,
        event: &mut ComponentHoverEvent<'_>,
    ) -> mod_api::Flow {
        let draw = &mut *event.draw;
        draw.fill_gradient([10.0, 10.0, 0.0, 5.0], TOP, BOTTOM);
        draw.fill_gradient([10.0, 10.0, 5.0, -5.0], TOP, BOTTOM);
        draw.fill_gradient([f32::NAN, 10.0, 5.0, 5.0], TOP, BOTTOM);
        draw.fill_gradient([10.0, 10.0, f32::INFINITY, 5.0], TOP, BOTTOM);
        draw.with_scissor([0.0, 0.0, 100.0, 14.0], &mut |clipped| {
            clipped.fill_gradient([10.0, 10.0, 20.0, 8.0], TOP, BOTTOM);
        })
        .unwrap();
        draw.fill_gradient([130.0, 28.0, -20.0, -8.0], 0x8011_2233, 0xFF44_5566);
        draw.draw_gui(&mut |canvas| {
            assert_eq!(
                canvas.fill_gradient([f32::NAN, 0.0, 4.0, 2.0], TOP, BOTTOM),
                Err(GuiDrawError::NotFinite)
            );
            canvas.scope(
                &[PoseOp::Translate(50.0, 60.0)],
                Some([0.0, 0.0, 10.0, 4.0]),
                &mut |scoped| {
                    // Fully inside the scissor: x in [50, 54], y in [60, 62].
                    scoped.fill_gradient([0.0, 0.0, 4.0, 2.0], TOP, BOTTOM)?;
                    // One-axis-inverted and empty gradients draw nothing.
                    scoped.fill_gradient([4.0, 0.0, -4.0, 2.0], TOP, BOTTOM)?;
                    scoped.fill_gradient([0.0, 0.0, 0.0, 2.0], TOP, BOTTOM)?;
                    // Cut halfway down by the scissor at y = 64: x in [56, 60], y in [62, 64].
                    scoped.fill_gradient([6.0, 2.0, 4.0, 4.0], TOP, BOTTOM)?;
                    Ok(())
                },
            )?;
            // Both axes inverted under a pose keeps the colours on their input y edges.
            canvas.scope(&[PoseOp::Translate(150.0, 60.0)], None, &mut |scoped| {
                scoped.fill_gradient([4.0, 2.0, -4.0, -2.0], 0x8011_2233, 0xFF44_5566)
            })
        })
        .unwrap();
        mod_api::Flow::Cancel
    }
}

#[test]
#[ignore = "daily"]
fn a_mods_gradients_draw_top_to_bottom_in_the_clip_and_pose_and_empty_ones_draw_nothing() {
    let text = Text::from(serde_json::json!({
        "text": "x", "hover_event": { "action": "show_text", "value": "hi" }
    }));
    let style: Style = text.spans[0].style.clone();
    let mods: mod_api::Mods = vec![ModEntry::new("gradients", Box::new(Gradients))].into();
    let mut batch = HudBatch::scratch([320, 240]);
    with_mods(mods, || {
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.component_hover_effect(&mut batch, &style, (40.0, 30.0));
        });
    });
    let drawn: Vec<_> = batch.vertices.iter().map(|v| (v.pos, v.color)).collect();
    let dc_clipped: Vec<_> = drawn
        .iter()
        .copied()
        .filter(|(pos, _)| pos[0] < 40.0)
        .collect();
    let canvas_unclipped: Vec<_> = drawn
        .iter()
        .copied()
        .filter(|(pos, _)| (50.0..=54.0).contains(&pos[0]))
        .collect();
    let canvas_clipped: Vec<_> = drawn
        .iter()
        .copied()
        .filter(|(pos, _)| (56.0..=60.0).contains(&pos[0]))
        .collect();
    let dc_inverted: Vec<_> = drawn
        .iter()
        .copied()
        .filter(|(pos, _)| (100.0..140.0).contains(&pos[0]))
        .collect();
    let canvas_inverted: Vec<_> = drawn
        .iter()
        .copied()
        .filter(|(pos, _)| pos[0] >= 140.0)
        .collect();
    assert_eq!(
        dc_clipped.len()
            + canvas_unclipped.len()
            + canvas_clipped.len()
            + dc_inverted.len()
            + canvas_inverted.len(),
        drawn.len(),
        "empty, one-axis-inverted and refused gradients must draw nothing: {drawn:?}"
    );
    assert_eq!(
        dc_inverted,
        [
            ([110.0, 20.0], 0xFF66_5544),
            ([130.0, 20.0], 0xFF66_5544),
            ([130.0, 28.0], 0x8033_2211),
            ([110.0, 20.0], 0xFF66_5544),
            ([130.0, 28.0], 0x8033_2211),
            ([110.0, 28.0], 0x8033_2211),
        ]
    );
    assert_eq!(
        canvas_inverted,
        [
            ([150.0, 60.0], 0xFF66_5544),
            ([154.0, 60.0], 0xFF66_5544),
            ([154.0, 62.0], 0x8033_2211),
            ([150.0, 60.0], 0xFF66_5544),
            ([154.0, 62.0], 0x8033_2211),
            ([150.0, 62.0], 0x8033_2211),
        ]
    );
    assert!(!dc_clipped.is_empty(), "{drawn:?}");
    assert_eq!(canvas_unclipped.len(), 6, "{drawn:?}");
    assert!(!canvas_clipped.is_empty(), "{drawn:?}");

    let mid = {
        let [t, b] = [abgr(TOP), abgr(BOTTOM)].map(u32::to_le_bytes);
        u32::from_le_bytes(std::array::from_fn(|i| {
            (u16::from(t[i]) + u16::from(b[i])).div_ceil(2) as u8
        }))
    };
    let assert_half_ramp = |color: u32| {
        let (got, want) = (color.to_le_bytes(), mid.to_le_bytes());
        assert!(
            got.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= 1),
            "{got:?} != {want:?}"
        );
    };
    // The DrawContext::with_scissor call: cut at y = 14, halfway down [10, 18].
    for (pos, color) in dc_clipped {
        assert!(pos[0] == 10.0 || pos[0] == 30.0, "{pos:?}");
        match pos[1] {
            10.0 => assert_eq!(color, abgr(TOP)),
            14.0 => assert_half_ramp(color),
            y => panic!("clipped vertex at y {y}"),
        }
    }
    // The unclipped GuiCanvas::scope gradient under Translate(50, 60).
    for (pos, color) in canvas_unclipped {
        assert!(pos[0] == 50.0 || pos[0] == 54.0, "{pos:?}");
        match pos[1] {
            60.0 => assert_eq!(color, abgr(TOP)),
            62.0 => assert_eq!(color, abgr(BOTTOM)),
            y => panic!("canvas gradient vertex at y {y}"),
        }
    }
    // The GuiCanvas::scope gradient clipped halfway down [62, 66] at y = 64.
    for (pos, color) in canvas_clipped {
        assert!(pos[0] == 56.0 || pos[0] == 60.0, "{pos:?}");
        match pos[1] {
            62.0 => assert_eq!(color, abgr(TOP)),
            64.0 => assert_half_ramp(color),
            y => panic!("canvas clipped vertex at y {y}"),
        }
    }
}

#[test]
#[ignore = "daily"]
fn with_scissor_around_posed_draw_gui_clips_once_against_effective_scissor() {
    struct MixedScissor;
    #[mod_api::__subscriptions]
    impl Hooks for MixedScissor {
        fn component_hover(
            &self,
            _: &dyn mod_api::ClientHost,
            event: &mut ComponentHoverEvent<'_>,
        ) -> mod_api::Flow {
            event
                .draw
                .with_scissor([20.0, 20.0, 20.0, 20.0], &mut |clipped| {
                    clipped.fill([0.0, 0.0, 100.0, 100.0], 0xFFFF_0000);
                    clipped
                        .draw_gui(&mut |canvas| {
                            canvas.scope(
                                &[
                                    PoseOp::Translate(30.0, 30.0),
                                    PoseOp::Rotate(std::f32::consts::FRAC_PI_4),
                                ],
                                Some([-6.0, -6.0, 12.0, 12.0]),
                                &mut |scoped| {
                                    scoped.fill_gradient([-16.0, -16.0, 32.0, 32.0], TOP, BOTTOM)
                                },
                            )?;
                            canvas.fill([0.0, 0.0, 100.0, 100.0], 0xFF00_FF00)
                        })
                        .unwrap();
                    clipped.fill([0.0, 0.0, 100.0, 100.0], 0xFF00_00FF);
                })
                .unwrap();
            mod_api::Flow::Cancel
        }
    }

    let text = Text::from(serde_json::json!({
        "text": "x", "hover_event": { "action": "show_text", "value": "hi" }
    }));
    let style: Style = text.spans[0].style.clone();
    let mods: mod_api::Mods = vec![ModEntry::new("mixed_scissor", Box::new(MixedScissor))].into();
    let mut batch = HudBatch::scratch([320, 240]);
    let mut reference = HudBatch::scratch([320, 240]);
    with_mods(mods, || {
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.component_hover_effect(&mut batch, &style, (40.0, 30.0));
            crate::game::host::draw::GameGuiCanvas::draw_with_scissor(
                &mut reference,
                &game.gui.atlas,
                Some(game),
                Some([20.0, 20.0, 40.0, 40.0]),
                &mut |canvas| {
                    canvas.scope(
                        &[
                            PoseOp::Translate(30.0, 30.0),
                            PoseOp::Rotate(std::f32::consts::FRAC_PI_4),
                        ],
                        Some([-6.0, -6.0, 12.0, 12.0]),
                        &mut |scoped| scoped.fill_gradient([-16.0, -16.0, 32.0, 32.0], TOP, BOTTOM),
                    )
                },
            )
            .unwrap();
        });
    });

    // Red parent fill [0..6], rotated gradient [6..6+ref_len], green canvas fill, blue parent fill.
    let ref_len = reference.vertices.len();
    assert!(ref_len > 0);
    assert_eq!(batch.vertices.len(), 18 + ref_len);
    let rect_bounds = |slice: &[sunrise_client::hud::HudVertex]| -> [f32; 4] {
        slice.iter().fold(
            [
                f32::INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::NEG_INFINITY,
            ],
            |[x0, y0, x1, y1], v| {
                [
                    x0.min(v.pos[0]),
                    y0.min(v.pos[1]),
                    x1.max(v.pos[0]),
                    y1.max(v.pos[1]),
                ]
            },
        )
    };
    assert_eq!(rect_bounds(&batch.vertices[0..6]), [20.0, 20.0, 40.0, 40.0]);
    assert_eq!(batch.vertices[0].color, abgr(0xFFFF_0000));
    let actual_grad: Vec<_> = batch.vertices[6..6 + ref_len]
        .iter()
        .map(|v| (v.pos, v.color))
        .collect();
    let expected_grad: Vec<_> = reference
        .vertices
        .iter()
        .map(|v| (v.pos, v.color))
        .collect();
    assert_eq!(
        actual_grad, expected_grad,
        "rotated gradient inside with_scissor + draw_gui must be clipped once, identically to single effective scissor"
    );
    assert_eq!(
        rect_bounds(&batch.vertices[6 + ref_len..12 + ref_len]),
        [20.0, 20.0, 40.0, 40.0]
    );
    assert_eq!(batch.vertices[6 + ref_len].color, abgr(0xFF00_FF00));
    assert_eq!(
        rect_bounds(&batch.vertices[12 + ref_len..18 + ref_len]),
        [20.0, 20.0, 40.0, 40.0]
    );
    assert_eq!(batch.vertices[12 + ref_len].color, abgr(0xFF00_00FF));
}
