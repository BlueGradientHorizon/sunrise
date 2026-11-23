# Drawing GUI content

## Drawing directly with a canvas

`DrawContext::draw_gui` lends a `GuiCanvas` with an identity pose and the current context's
clip. Call its methods directly inside the callback, in draw order. A nested `scope` applies
pose operations in order to a copy of the current pose and optionally scissors to
`[x, y, w, h]` in the resulting local coordinates. That clip stays fixed in screen space even
when a descendant changes pose. The scissor restricts the pixels drawn, not the original vertices
or their colour and texture interpolation. Returning from a child scope leaves the parent pose
and clip as they were.

```rust,ignore
use mod_api::{GuiDrawError, Text, client::hud_layout::PoseOp};

let result: Result<(), GuiDrawError> = draw.draw_gui(&mut |canvas| {
    canvas.scope(
        &[PoseOp::Translate(20.0, 20.0), PoseOp::Scale(2.0, 2.0)],
        Some([0.0, 0.0, 60.0, 16.0]),
        &mut |panel| {
            panel.fill([0.0, 0.0, 60.0, 16.0], 0xC020_2020)?;
            panel.text(&Text::literal("60 FPS"), [4.0, 4.0], 0xFFFF_FFFF, true)
        },
    )
});
```

A refused primitive draws nothing. Use `?` to discard the whole group on a refusal, or handle
it to retain the other draws. Successful children commit in place, but any error returned by a
parent still discards them. If you handle a child's error, only that child is discarded. Returning
`Err(GuiDrawError::Abort)` or unwinding also discards the current group. Widgets, scenes and
independent host calls are not part of the transaction; use the ordinary `DrawContext` for those
draws, with [`with_scissor`](screen-clipping.md) if you need to clip them.

For a wrapped component, call `canvas.text_with_word_wrap`. It wraps and paints in the current
scope and returns the [label dimensions](screens.md#labels-wrapped-text-without-a-widget). The
canvas walks the wrapped ranges and paints each line using reusable scratch instead of
building a replay list. If a scaled offset or dimension is not finite, the call
returns `GuiDrawError::TextLayout(TextLayoutError::GeometryNotFinite)` without drawing, even
for a blank or fully transparent label.

## Items

Call `canvas.item(&stack, at, seed)` to draw a stack's model in a 16-pixel slot, passing the seed to
its model resolver. Call `canvas.item_decorations(&stack, at, None)` to add its bar, cooldown,
count and custom decoration, in that order. An optional `Some(label)` replaces the count text.
An `item_bar` override supplies a signed width in pixels. As in vanilla, the bar's foreground
endpoint is formed with wrapping integer addition.
The stack is read for display, not written to a slot: it keeps its components and can be over the
storage limit. Empty stacks draw nothing. Head-skin fetches and atlas requests caused by the draw
wait until the outer group commits.

To draw from the local inventory, read it at the calling boundary rather than sending a slot
number to the renderer. Inventory numbers `0..=8` are the hotbar and `40` is the off hand. An
absent stack means nothing to draw. Here the model and decorations both follow the panel's
translation, but only the model gets the pop squeeze, as in vanilla's hotbar:

```rust,ignore
use mod_api::client::hud_layout::PoseOp;

let stack = host.local_player()
    .and_then(|player| player.with_inventory(host, |inv| inv.get(0)?.project()))
    .flatten();
if let Some(stack) = stack {
    draw.draw_gui(&mut |canvas| {
        canvas.scope(&[PoseOp::Translate(20.0, 30.0)], None, &mut |panel| {
            let at = [0.0, 0.0];
            panel.scope(
                &[
                    PoseOp::Translate(8.0, 12.0),
                    PoseOp::Scale(0.5, 1.5),
                    PoseOp::Translate(-8.0, -12.0),
                ],
                None,
                &mut |model| model.item(&stack, at, 1),
            )?;
            panel.item_decorations(&stack, at, None)
        })
    })?;
}
```

`ItemPart::item_decorations` receives a `GuiCanvas` as `event.draw` after the standard
bar, cooldown and count. Use the icon's local `event.x` and `event.y` for drawing. The canvas
already has the caller's pose and scissor (identity for ordinary inventory, hotbar and tooltip
calls). On this canvas and its child scopes, `item_decorations` paints only the standard bar,
cooldown and count; it does not invoke any item decorators, even for a different stack. A
decorator returns `()`, so use `event.draw.scope` to roll back its own primitives together on
failure. Move any widgets, focus, narration or live scenes to ordinary
HUD or screen draw hooks.

For replacing or wrapping a HUD status layer, see [HUD drawing](hud-drawing.md).

## Blits

- `canvas.blit_sprite` looks up a GUI sprite by id, such as `container/slot` or `mymod:panel`.
  Pass `None` to draw the whole sprite or `Some(GuiSpriteRegion { size, offset })` to give the
  whole sprite's logical size and the source offset in its pixels. Only `Textured` and `Crosshair`
  work on this path. A removed GUI sprite is refused, not drawn as a missing texture.
- `canvas.atlas_sprite` looks up the exact packed atlas name, independently of the GUI lookup.
- `canvas.blit` draws part of a raw texture (a path under `textures/`, without `.png`, or a live
  or registered texture id). `GuiBlitRegion { uv, src, tex }` gives the source offset, signed source
  extents and texture-axis divisors in pixels. A negative source extent mirrors that axis; a zero
  divisor is refused. `Textured` blends normally, `Vignette` darkens what is under
  it (`dst * (1 - src)`), `Nausea` is additive and `Crosshair` inverts it.

## Refusals

`GuiDrawError` belongs to the call that returned it. Geometry or pose arithmetic that isn't finite
returns `NotFinite`; text and stack conversion failures carry `Text` or `Stack` errors. The blit
methods distinguish `UnknownSprite`, `UnknownTexture`, `BadSpriteRegion`, `BadBlitRegion` and
`UnsupportedSpriteBlend`. Return a refusal from the callback to discard its whole group, or
handle it there and keep the successful draws.
