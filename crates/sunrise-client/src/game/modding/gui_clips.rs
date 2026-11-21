//! One full-context scissor scope, cut on the batch like the game's own screens.
use crate::hud::HudBatch;
use mod_api::GuiClipRefusal;

/// Where a clip began and its rectangle (`[x0, y0, x1, y1]`), intersected with its parent.
/// The scoped draw context owns the clip along with the borrowed batch, and closes it on drop.
pub(crate) struct GuiClip {
    start: usize,
    rect: [f32; 4],
}

impl GuiClip {
    pub(crate) fn open(
        batch: &HudBatch,
        rect: [f32; 4],
        outer: Option<&Self>,
    ) -> Result<Self, GuiClipRefusal> {
        if !rect.iter().all(|v| v.is_finite()) {
            return Err(GuiClipRefusal::NotFinite);
        }
        let [x, y, w, h] = rect;
        let (x1, y1) = (x + w.max(0.0), y + h.max(0.0));
        if ![x1, y1].iter().all(|v| v.is_finite()) {
            return Err(GuiClipRefusal::NotFinite);
        }
        let mut clip = [x, y, x1, y1];
        if let Some(outer) = outer {
            clip = [
                clip[0].max(outer.rect[0]),
                clip[1].max(outer.rect[1]),
                clip[2].min(outer.rect[2]),
                clip[3].min(outer.rect[3]),
            ];
            // Disjoint from the outer clip: an empty one, clipping everything.
            clip[2] = clip[2].max(clip[0]);
            clip[3] = clip[3].max(clip[1]);
        }
        Ok(Self {
            start: batch.vertices.len(),
            rect: clip,
        })
    }

    pub(crate) fn close(self, batch: &mut HudBatch) {
        let [x0, y0, x1, y1] = self.rect;
        batch.clip_since(self.start, x0, y0, x1, y1);
    }
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
        let clip = GuiClip::open(&batch, [10.0, 10.0, 100.0, 100.0], None).unwrap();
        square(&mut batch, 0.0, 0.0, 300.0);
        clip.close(&mut batch);
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
            let outer = GuiClip::open(&batch, outer, None).unwrap();
            let inner = GuiClip::open(&batch, inner, Some(&outer)).unwrap();
            square(&mut batch, 0.0, 0.0, 300.0);
            inner.close(&mut batch);
            outer.close(&mut batch);
            assert_eq!(
                (!batch.vertices.is_empty()).then(|| bounds(&batch)),
                expected
            );
        }
    }

    #[test]
    fn non_finite_rects_are_refused_and_nesting_is_unbounded() {
        let mut batch = HudBatch::scratch([640, 480]);
        let outer = GuiClip::open(&batch, [0.0, 0.0, 200.0, 200.0], None).unwrap();
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
                GuiClip::open(&batch, rect, Some(&outer)).err(),
                Some(GuiClipRefusal::NotFinite)
            );
        }

        let mut stack = vec![outer];
        for i in 1..=100 {
            let offset = i as f32;
            let next = GuiClip::open(&batch, [offset, offset, 200.0, 200.0], stack.last()).unwrap();
            stack.push(next);
        }
        assert!(stack.len() > 64);
        square(&mut batch, 0.0, 0.0, 300.0);
        while let Some(clip) = stack.pop() {
            clip.close(&mut batch);
        }
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
