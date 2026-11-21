# Scoped GUI drawing (`GuiCanvas`)

`DrawContext::draw_gui(&mut |canvas| { ... })` lends a [`GuiCanvas`] that appends vanilla's
`GuiGraphicsExtractor` primitives directly to the HUD batch inside lexical pose and scissor scopes
(`GuiCanvas::scope`). Each primitive validates its own inputs before emitting vertices, so a
refused call adds no output while draws already emitted stay in the batch. Pose and scissor are
always restored when a scope returns, returns `Err`, or unwinds.

```rust,ignore
use mod_api::client::hud_layout::PoseOp;

// Draw an item model at twice its size about (28, 28), then draw its decorations unscaled
// outside the squeeze scope.
draw.draw_gui(&mut |canvas| {
    canvas.scope(
        &[
            PoseOp::Translate(28.0, 28.0),
            PoseOp::Scale(2.0, 2.0),
            PoseOp::Translate(-28.0, -28.0),
        ],
        None,
        &mut |squeezed| squeezed.item(&stack, [20.0, 20.0], 0),
    )?;
    canvas.item_decorations(&stack, [20.0, 20.0], None)
})?;
```

## Scopes

`GuiCanvas::scope(ops, scissor, draw)` copies the current pose, applies `ops` (`PoseOp::Translate`,
`PoseOp::Scale`, `PoseOp::Rotate`) in order, transforms `scissor` (`Some([x, y, w, h])`) into
screen space, and intersects it with any enclosing scissor (`None` inherits the enclosing scissor).
Only the transformed, ordered corners determine emptiness; a reflected negative extent can be visible.
Non-finite pose or scissor geometry refuses the scope with `GuiDrawError::NotFinite` before calling
`draw`.

## Items and item decorations

- `canvas.item(&stack, [x, y], seed)` draws the stack's 16-pixel GUI item model at `(x, y)` and
  forwards `seed` to the item model resolver (`GuiGraphicsExtractor.item`). Empty or air stacks
  draw nothing and return `Ok(())`.
- `canvas.item_decorations(&stack, [x, y], count)` draws the durability bar, cooldown shade,
  count label (or `count` override; `Some("")` hides it), and custom `item_decorations` item parts
  at `(x, y)` in the enclosing pose and scissor. Call it outside a model-only squeeze scope so
  decorations follow the surrounding pose.

## Sprites and blits

- `canvas.blit_sprite(name, dst, region, argb, blend)` draws a present GUI sprite by ID, honouring
  its metadata scaling (stretch, tile or nine-slice) and an optional `GuiSpriteRegion { size, offset }`
  for partial blits. Tiled regions repeat from the original tile origin under clipping and poses,
  sampling at the render target's pixel centers (including scaled captures). A sample just before
  a tile seam stays in the ending tile; one on an interior texel edge selects the texel that
  starts there, even when the sprite sits at an offset in the atlas. Accepts `Blend::Textured`
  and `Blend::Crosshair`.
- `canvas.atlas_sprite(name, dst, argb)` draws a sprite by its exact packed atlas name.
- `canvas.blit(texture, dst, GuiBlitRegion { uv, src, tex }, argb, blend)` draws part of a raw
  texture (path under `textures/`, without `.png`, or a live/registered texture ID) with any
  `Blend`: `Textured`, `Vignette` (`dst * (1 - src)`), `Nausea` (additive), or `Crosshair`
  (invert). Zero or negative `src` extents mirror or repeat the sampled edge, matching vanilla.

## Refusals

| `GuiDrawError` | When |
| --- | --- |
| `Unavailable` | This context cannot lend a GUI canvas (or item drawing has no game). |
| `NotFinite` | A coordinate, pose, scissor edge, UV, or draw endpoint is NaN or infinite. |
| `Text(TextFormError)` | Styled text cannot cross to the game (for example, deeper than 512 component nodes). |
| `Stack(StackWriteError)` | An item stack cannot be read for rendering (storage count and damage limits do not apply). |
| `UnknownSprite` | The requested GUI sprite (including removed sprites) or packed-atlas sprite is absent. |
| `UnsupportedSpriteBlend` | `blit_sprite` was called with `Blend::Vignette` or `Blend::Nausea`. |
| `BadSpriteRegion` | `GuiSpriteRegion::size` is not positive on both axes. |
| `UnknownTexture` | The requested raw texture is not in the GUI atlas. |
| `BadBlitRegion` | `GuiBlitRegion::tex` is not positive on both axes. |

The enum is `#[non_exhaustive]`. A refused call emits no vertices while earlier draws remain in the
batch.

- **Cost.** Primitives write straight to the active batch without building a command buffer; a mod
  that does not draw pays nothing.
