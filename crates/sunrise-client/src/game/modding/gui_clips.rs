//! Screen-space scissor intersection shared by `DrawContext::with_scissor` and `GuiCanvas::scope`.
use crate::hud::HudBatch;
use crate::hud_layout::{IDENTITY, Pose, pose_point};
use mod_api::GuiClipRefusal;

/// Intersect `rect` (`[x, y, w, h]`) in screen space with `enclosing` (or the viewport when
/// `enclosing` is `None`).
pub(crate) fn intersect_scissor(
    batch: &HudBatch,
    rect: [f32; 4],
    enclosing: Option<[f32; 4]>,
) -> Result<[f32; 4], GuiClipRefusal> {
    intersect_posed_scissor(batch, IDENTITY, rect, enclosing)
}

/// Transform `rect` (`[x, y, w, h]`) by `pose` and intersect it with `enclosing` (or the viewport
/// `[0, 0, batch.width, batch.height]` when `enclosing` is `None`).
///
/// Matches vanilla's `ScreenRectangle.transformAxisAligned` and `GuiGraphicsExtractor.ScissorStack`:
/// the two ordered corners are transformed, origin and extents are floored separately, and an
/// empty, inverted, reflected, or disjoint result collapses to `[0.0; 4]`.
pub(crate) fn intersect_posed_scissor(
    batch: &HudBatch,
    pose: Pose,
    rect @ [x, y, w, h]: [f32; 4],
    enclosing: Option<[f32; 4]>,
) -> Result<[f32; 4], GuiClipRefusal> {
    let end = [x + w, y + h];
    if !rect.into_iter().chain(end).all(f32::is_finite) {
        return Err(GuiClipRefusal::NotFinite);
    }
    let [tx, ty] = pose_point(pose, [x, y]);
    let [tright, tbottom] = pose_point(pose, end);
    let [left, top] = [tx.floor(), ty.floor()];
    let [width, height] = [(tright - tx).floor(), (tbottom - ty).floor()];
    let mut clip = [left, top, left + width, top + height];
    if !clip.into_iter().all(f32::is_finite) {
        return Err(GuiClipRefusal::NotFinite);
    }
    let [vx0, vy0, vx1, vy1] = enclosing.unwrap_or([0.0, 0.0, batch.width, batch.height]);
    clip = [
        clip[0].max(vx0),
        clip[1].max(vy0),
        clip[2].min(vx1),
        clip[3].min(vy1),
    ];
    if w <= 0.0 || h <= 0.0 || clip[2] <= clip[0] || clip[3] <= clip[1] {
        clip = [0.0; 4];
    }
    Ok(clip)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(batch: &mut HudBatch, x: f32, y: f32, size: f32) {
        let p = [[x, y], [x + size, y], [x + size, y + size], [x, y + size]];
        batch.quad(
            p,
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            u32::MAX,
        );
    }
    fn bounds(batch: &HudBatch) -> [f32; 4] {
        batch.vertices.iter().fold(
            [
                f32::INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::NEG_INFINITY,
            ],
            |[a, b, c, d], v| {
                [
                    a.min(v.pos[0]),
                    b.min(v.pos[1]),
                    c.max(v.pos[0]),
                    d.max(v.pos[1]),
                ]
            },
        )
    }

    #[test]
    fn a_clip_cuts_only_what_was_drawn_inside_it() {
        let mut batch = HudBatch::scratch([640, 480]);
        square(&mut batch, 0.0, 0.0, 300.0);
        let before: Vec<_> = batch.vertices.iter().map(|v| v.pos).collect();
        let [x0, y0, x1, y1] = intersect_scissor(&batch, [10.0, 10.0, 100.0, 100.0], None).unwrap();
        let start = batch.vertices.len();
        square(&mut batch, 0.0, 0.0, 300.0);
        batch.clip_since(start, x0, y0, x1, y1);
        let kept: Vec<_> = batch.vertices[..before.len()]
            .iter()
            .map(|v| v.pos)
            .collect();
        assert_eq!(kept, before, "drawn before the clip");
        let mut inside = HudBatch::scratch([640, 480]);
        inside.vertices = batch.vertices[before.len()..].to_vec();
        assert_eq!(bounds(&inside), [10.0, 10.0, 110.0, 110.0]);
    }

    #[test]
    fn nested_clips_intersect_and_a_disjoint_one_clips_everything() {
        let mut batch = HudBatch::scratch([640, 480]);
        for (outer, inner, expected) in [
            (
                [0.0, 0.0, 100.0, 100.0],
                [50.0, 50.0, 200.0, 200.0],
                Some([50.0, 50.0, 100.0, 100.0]),
            ),
            ([0.0, 0.0, 10.0, 10.0], [20.0, 20.0, 10.0, 10.0], None),
            ([0.0, 0.0, 10.0, 10.0], [0.0, 0.0, -5.0, 10.0], None),
            ([0.0, 0.0, 10.0, 10.0], [0.0, 0.0, 10.0, -5.0], None),
        ] {
            batch.vertices.clear();
            let outer = intersect_scissor(&batch, outer, None).unwrap();
            let [x0, y0, x1, y1] = intersect_scissor(&batch, inner, Some(outer)).unwrap();
            square(&mut batch, 0.0, 0.0, 300.0);
            batch.clip_since(0, x0, y0, x1, y1);
            assert_eq!(
                (!batch.vertices.is_empty()).then(|| bounds(&batch)),
                expected
            );
        }
    }

    #[test]
    fn non_finite_rects_are_refused_and_nesting_is_unbounded() {
        let mut batch = HudBatch::scratch([640, 480]);
        let outer = intersect_scissor(&batch, [0.0, 0.0, 200.0, 200.0], None).unwrap();
        for rect in [
            [f32::NAN, 0.0, 1.0, 1.0],
            [0.0, 0.0, f32::INFINITY, 1.0],
            [0.0, 0.0, f32::NAN, 1.0],
            [0.0, 0.0, f32::NEG_INFINITY, 1.0],
            [0.0, 0.0, 1.0, f32::NAN],
            [0.0, 0.0, 1.0, f32::NEG_INFINITY],
            [f32::MAX, 0.0, f32::MAX, 1.0],
        ] {
            assert_eq!(
                intersect_scissor(&batch, rect, Some(outer)).err(),
                Some(GuiClipRefusal::NotFinite)
            );
        }

        let mut stack = vec![outer];
        for i in 1..=100 {
            let offset = i as f32;
            let next = intersect_scissor(
                &batch,
                [offset, offset, 200.0, 200.0],
                stack.last().copied(),
            )
            .unwrap();
            stack.push(next);
        }
        assert!(stack.len() > 64);
        let [x0, y0, x1, y1] = *stack.last().unwrap();
        square(&mut batch, 0.0, 0.0, 300.0);
        batch.clip_since(0, x0, y0, x1, y1);
        assert_eq!(bounds(&batch), [100.0, 100.0, 200.0, 200.0]);
    }
}

/// The generated dispatch lends the full context: a nested scope cannot leave a clip active
/// after it returns or unwinds, either in its caller or in another listener.
#[cfg(test)]
mod dispatch_tests {
    use mod_api::{ComponentHoverEvent, Flow, GuiClipRefusal, Hooks, ModEntry};
    use mod_platform::testing::with_mods;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use sunrise_client::hud::HudBatch;
    use sunrise_client::text::Text;

    struct Clipper;
    #[mod_api::__subscriptions]
    impl Hooks for Clipper {
        fn component_hover(
            &self,
            _: &dyn mod_api::ClientHost,
            event: &mut ComponentHoverEvent<'_>,
        ) -> Flow {
            let square = [0.0, 0.0, 100.0, 100.0];
            event
                .draw
                .with_scissor([10.0, 10.0, 10.0, 10.0], &mut |outer| {
                    assert_eq!(
                        outer.with_scissor([0.0, f32::NAN, 5.0, 5.0], &mut |_| {
                            panic!("a refused rectangle must not invoke the callback")
                        }),
                        Err(GuiClipRefusal::NotFinite)
                    );
                    outer
                        .with_scissor([12.0, 12.0, 5.0, 5.0], &mut |inner| {
                            inner.fill(square, 0xFF00_FF00);
                        })
                        .unwrap();
                    let unwind = catch_unwind(AssertUnwindSafe(|| {
                        outer.with_scissor([15.0, 15.0, 10.0, 10.0], &mut |inner| {
                            inner.fill(square, 0xFF00_00FF);
                            panic!("leave the nested scope by unwinding")
                        })
                    }));
                    assert!(unwind.is_err());
                    outer.fill(square, 0xFFFF_0000);
                })
                .unwrap();
            event.draw.fill([8.0, 8.0, 1.0, 1.0], 0xFFFF_FF00);
            Flow::Continue
        }
    }

    struct Unclipped;
    #[mod_api::__subscriptions]
    impl Hooks for Unclipped {
        fn component_hover(
            &self,
            _: &dyn mod_api::ClientHost,
            event: &mut ComponentHoverEvent<'_>,
        ) -> Flow {
            event.draw.fill([24.0, 24.0, 4.0, 4.0], u32::MAX);
            Flow::Cancel
        }
    }

    #[test]
    #[ignore = "daily"]
    fn a_hook_call_clips_what_its_mod_draws_and_closes_what_it_leaves_open() {
        let text = Text::from(serde_json::json!({
            "text": "x", "hover_event": { "action": "show_text", "value": "hi" }
        }));
        let mods: mod_api::Mods = vec![
            ModEntry::new("clipper", Box::new(Clipper)),
            ModEntry::new("unclipped", Box::new(Unclipped)),
        ]
        .into();
        let mut batch = HudBatch::scratch([320, 240]);
        with_mods(mods, || {
            crate::game::native_entity_refs::tests::with_game(|game| {
                game.component_hover_effect(&mut batch, &text.spans[0].style, (40.0, 30.0));
            });
        });
        assert_eq!(batch.vertices.len(), 30);
        for (quad, (color, [x0, y0, x1, y1])) in batch.vertices.as_chunks::<6>().0.iter().zip([
            (0xFF00_FF00, [12.0, 12.0, 17.0, 17.0]),
            (0xFFFF_0000, [15.0, 15.0, 20.0, 20.0]),
            (0xFF00_00FF, [10.0, 10.0, 20.0, 20.0]),
            (0xFF00_FFFF, [8.0, 8.0, 9.0, 9.0]),
            (u32::MAX, [24.0, 24.0, 28.0, 28.0]),
        ]) {
            assert_eq!(quad.each_ref().map(|v| v.color), [color; 6]);
            assert_eq!(
                quad.each_ref().map(|v| v.pos),
                [[x0, y0], [x1, y0], [x1, y1], [x0, y0], [x1, y1], [x0, y1]]
            );
        }
    }
}
