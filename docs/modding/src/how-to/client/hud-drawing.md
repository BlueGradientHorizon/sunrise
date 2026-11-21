# Drawing on the HUD and in screens

A HUD layer, a screen or a tooltip hook draws through the `DrawContext` it is handed, in GUI pixels. Each call
is one of vanilla's `GuiGraphics` calls: `fill`, `text`, `sprite`, `item_stack` and the others.
[Draw through a canvas](gui-paint.md#drawing-directly-with-a-canvas) with `draw_gui` to group
fills, styled text, blits and seeded items with their decorations in lexical pose and scissor scopes.

`ClientHost::debug_overlay_visible()` reports whether the client draws its debug overlay this frame. In
`render_gui_post`, use it to leave space for the overlay. Always-on debug entries can make it visible with
F3 off. F1 hides the overlay unless a screen is open. The read is available only on the client host.

## Gradient fills

`DrawContext::fill_gradient(rect, top, bottom)` is vanilla's `GuiGraphics.fillGradient`, the call screen
backgrounds and tooltips are drawn with. `rect` is `[x, y, w, h]` in GUI pixels, as `fill` takes it. `top` and
`bottom` are straight-alpha ARGB colours (`0xAARRGGBB`): `top` at `y`, blending to `bottom` at `y + h`.
With positive extents these are the visual top and bottom edges. Alpha blends with the colours, so a gradient can
fade out. The current HUD shader discards interpolated alpha below 0.004, not just exact zero; near-transparent
fragments can therefore differ from vanilla's exact-zero discard.

```rust,ignore
fn gui_render_post(&self, _: &dyn ClientHost, event: &mut GuiRenderPostEvent<'_>) {
    // A panel shaded from dark purple at the top to black at the bottom.
    event.batch.fill_gradient([20.0, 20.0, 120.0, 60.0], 0xF010_0010, 0xF000_0000);
    // A shade that fades out downwards.
    event.batch.fill_gradient([20.0, 80.0, 120.0, 10.0], 0x8000_0000, 0x0000_0000);
}
```

`GuiCanvas::fill_gradient` takes the same arguments and follows the canvas's current pose and scissor.

- **Clip.** A clip from `DrawContext::with_scissor` or `GuiCanvas::scope` keeps the colour ramp:
  the cut edge has the colour that was there.
- **Empty or inverted.** Zero width or height draws nothing, as does inversion on exactly one axis.
  Inverting both axes (`w < 0` and `h < 0`) draws sorted bounds with the colours flipped: the first
  colour stays on its input y edge, now the lower edge. This preserves vanilla's visible winding
  and colour placement. A non-finite rectangle draws nothing; the canvas reports `NotFinite`.
  Zero-area calls are culled here; vanilla can retain their layering bounds without drawing pixels.
- **Cost.** One quad on the GUI batch, as `fill`; nothing is drawn or kept when a mod doesn't call it.

## Scaled text

`DrawContext::text_scaled(text, at, scale, argb, shadow)` draws `text` as `text` does, scaled by `scale` about
`at`, its top-left corner: what vanilla's HUD draws with `PoseStack.scale` around a line. A half-size line
under a label is `scale` 0.5, starting a quarter of its unscaled width left of where it is centred.

```rust,ignore
fn gui_layer_draw(&self, _: &dyn ClientHost, event: &mut GuiLayerDrawEvent<'_>) {
    let width = event.draw.text_width("12 CPS");
    // Centred at (40, 30), half size.
    event.draw.text_scaled("12 CPS", [40.0 - width / 4.0, 30.0], 0.5, 0xFFFF_FFFF, false);
}
```
