# Clipping what a screen draws

A scrolling list, a label that scrolls when it is too long, or a viewport into a larger page draws more than fits
and shows only the part inside a box. `DrawContext::with_scissor(rect, draw)` runs `draw` inside such a box
(vanilla's `GuiGraphicsExtractor.enableScissor` / `disableScissor`). Everything drawn through the scoped
`DrawContext` is cut to `rect`, `[x, y, w, h]` in GUI pixels as `fill` takes it: the visible part of a sprite,
a glyph or an item stays, and the scissor always closes when `draw` returns or unwinds.

```rust,ignore
fn gui_render_post(&self, _: &dyn ClientHost, event: &mut GuiRenderPostEvent<'_>) {
    let [x, y, w, h] = self.list_box;
    let _ = event.batch.with_scissor([x, y, w, h], &mut |clipped| {
        for (row, line) in self.lines.iter().enumerate() {
            let top = y + row as f32 * 12.0 - self.scroll;
            clipped.text(line, [x + 2.0, top], 0xFFFF_FFFF, false);
        }
    });
}
```

- **Nesting.** Scissors nest without a fixed depth limit: an inner scissor is cut to the scissor around it,
  so it never shows more than that one. A negative width or height, or an inner scissor outside the outer
  one, clips everything.
- **Lexical scope.** A scissor belongs to the `with_scissor` closure (or `GuiCanvas::scope` inside
  `draw_gui`) and is always restored on return or unwind, so a listener cannot leave a clip active for
  later drawing or other listeners.
- **Depth.** Item models and scenes from `draw_scene` are cut too, to both their own box and the scissor, and
  keep their depth: a cut face still draws in front of or behind its neighbours as it did uncut.
- **Refusals.** `with_scissor` returns `Result<(), GuiClipRefusal>` and does not run `draw` when it refuses:
  `GuiClipRefusal::NotFinite` (a coordinate, size, or computed corner is NaN or infinite). The enum is
  `#[non_exhaustive]`.
- **Cost.** A call that never clips allocates nothing; a scissor is cut on the GUI batch when its scope
  closes, as the game's own scrolling screens cut theirs.
