# Clipping what a screen draws

A scrolling list or a viewport into a larger page shows only the part inside a box.
`DrawContext::with_scissor(rect, &mut |clipped| { ... })` cuts draws made through `clipped` to
`[x, y, w, h]` in GUI pixels, like vanilla's `GuiGraphics.enableScissor` and `disableScissor`
around the callback. The full drawing context is available inside, including widgets, scenes,
items and text. The callback returns `()` and the clip does not roll back drawing; handle errors
from draw methods inside the callback.

```rust,ignore
fn render_gui_post(&self, _: &dyn ClientHost, event: &mut RenderGuiPostEvent<'_>) {
    let [x, y, w, h] = self.list_box;
    let _ = event.draw.with_scissor([x, y, w, h], &mut |clipped| {
        for (row, line) in self.lines.iter().enumerate() {
            let top = y + row as f32 * 12.0 - self.scroll;
            clipped.text(line, [x + 2.0, top], 0xFFFF_FFFF, false);
        }
    });
}
```

- **Nesting.** An inner clip is intersected with the outer clip, so it never shows more than that
  one. A nonpositive width or height, or a disjoint inner clip, clips everything. The enclosing
  clip is restored on return or unwind, before any subsequent draws.
- **Depth.** Scenes keep their own box too. Cutting an item or scene preserves depth: a cut face
  still draws in front of or behind its neighbours as it did uncut.
- **Refusals.** `GuiClipRefusal::NotFinite` (a coordinate, size or computed endpoint is NaN or
  infinite) or `Unavailable` (no scissor drawing surface) leaves the enclosing clip as it was and
  does not call the callback.

For a fallible, transactional group of GUI primitives [on a canvas](gui-paint.md#drawing-directly-with-a-canvas),
pass `Some(rect)` to `canvas.scope`. That scissor uses the child scope's pose at entry; later
pose changes do not move it.
