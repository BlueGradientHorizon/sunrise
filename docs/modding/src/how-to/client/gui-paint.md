# Painting a GUI command buffer

## Drawing directly with a canvas

`DrawContext::draw_gui` lends a `GuiCanvas` with an identity pose. Draw inside its callback rather
than building a list of commands. Everything the callback draws commits in order on `Ok(())`;
returning an error (including `GuiDrawError::Abort`) discards the group. A nested `scope` applies pose
operations in order and optionally scissors to `[x, y, w, h]` in the resulting local coordinates.
Its clip stays fixed in screen space even when a descendant changes pose. A refused call draws
nothing. Propagate its error with `?` or, if you have an alternative, handle it and continue.

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

A nested scope that returns an error leaves its parent unchanged. Widgets, scenes and other
host state are not part of the transaction; use the ordinary `DrawContext` for those draws.

## Items

Call `canvas.item(&stack, at, seed)` to draw a stack's model in a 16-pixel slot, passing the seed to
its model resolver. Call `canvas.item_decorations(&stack, at, None)` to add its bar, cooldown,
count and custom decoration, in that order. An optional `Some(label)` replaces the count text.
The stack is read for display, not written to a slot: it keeps its components and can be over the
storage limit. Empty stacks draw nothing. Head-skin fetches and atlas requests caused by the draw
wait until the outer group commits.

To draw from the local inventory, read it at the calling boundary rather than sending a slot
number to the renderer. Inventory numbers `0..=8` are the hotbar and `40` is the off hand. An
absent stack means nothing to draw. As in vanilla's hotbar, apply the pop squeeze only in the
model's child scope, so it doesn't distort the decorations:

```rust,ignore
use mod_api::client::hud_layout::PoseOp;

let slots = host.local_player().and_then(|player| player.inventory_slots(host));
if let Some((_, stack)) = slots.into_iter().flatten().find(|(slot, _)| *slot == 0) {
    draw.draw_gui(&mut |canvas| {
        let at = [20.0, 20.0];
        canvas.scope(
            &[
                PoseOp::Translate(28.0, 32.0),
                PoseOp::Scale(0.5, 1.5),
                PoseOp::Translate(-28.0, -32.0),
            ],
            None,
            &mut |model| model.item(&stack, at, 1),
        )?;
        canvas.item_decorations(&stack, at, None)
    })?;
}
```

`ItemPart::item_decorations` receives a `GuiCanvas` as `event.draw` after the standard
bar, cooldown and count. Use the icon's local `event.x` and `event.y` for drawing. The canvas
already has the caller's pose and scissor (identity for ordinary inventory, hotbar and tooltip
calls). Use `event.draw.scope` to give a decorator's own draws a separate rollback boundary.

## Command buffer

`DrawContext::gui_paint(&[GuiDraw])` draws a buffer of vanilla's `GuiGraphics` calls in order, under their poses
(`GuiDraw::Push` / `Pop`). Every op in it is drawn, or none is: an op the paint cannot draw refuses the whole buffer
with a `GuiPaintError` that names it and its index, so a mod can draw it another way.

## Blits

`GuiDraw::Blit` draws part of a texture (`texture` is the path under `textures/`, without `.png`) with its
`blend`: `Textured` blends normally, `Vignette` darkens what is under it by the texture (`dst * (1 - src)`),
`Nausea` adds to it and `Crosshair` inverts it.

## Refusals

| `GuiPaintError` | When |
| --- | --- |
| `Text(TextFormError)` | A `Text` op's text cannot cross to the game (deeper than 512 nodes, malformed). |
| `ItemSlot { index, slot }` | An `Item` of a slot other than 0-8 or 40. |
| `ItemNotFinite { index }` | An `Item` whose `scale` or `pivot` is NaN or infinite. |
| `BlitTexture { index, texture }` | A `Blit` of a texture the GUI atlas does not hold. |
| `BlitSize { index }` | A `Blit` whose `tex` size is not positive, or whose `uv` is NaN or infinite. |
| `Sprite { index, sprite }` | A `Sprite` or `AtlasSprite` the GUI atlas does not hold, a mob effect sprite (`mob_effect/...`, namespaced or not) included. |
| `SpriteRegion { index }` | A `Sprite` whose `region` has a sprite width or height of zero (or less). |
| `SpriteBlend { index, blend }` | A `Sprite` with the `Nausea` or `Vignette` blend, which sprites cannot be drawn with. |

The enum is `#[non_exhaustive]`. `index` is the op's place in the buffer; nothing of the buffer was drawn, the
ops before it included. A `TextFormError` converts into it with `?`.

- **Cost.** The check reads the buffer once before drawing it; a mod that doesn't paint pays nothing.
