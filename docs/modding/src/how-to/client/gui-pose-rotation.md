# Turning what a GUI canvas draws

Inside `DrawContext::draw_gui`, `GuiCanvas::scope(ops, scissor, draw)` moves what the child closure
draws with a pose: it applies `ops` to a copy of the current pose (`pose().pushMatrix()`, then the
ops) and restores the previous pose automatically when `draw` returns, returns `Err`, or unwinds.
Besides `PoseOp::Translate(x, y)` and `PoseOp::Scale(x, y)`, `PoseOp::Rotate(angle)` turns it by
`angle` radians, as vanilla's `Matrix3x2fStack.rotate` does: for a compass needle, a minimap arrow,
or a tilted label.

```rust,ignore
use mod_api::client::hud_layout::PoseOp;

// A needle 20 pixels long turned by `heading` about (cx, cy).
draw.draw_gui(&mut |canvas| {
    canvas.scope(
        &[PoseOp::Translate(cx, cy), PoseOp::Rotate(heading)],
        None,
        &mut |turned| turned.fill([-1.0, -20.0, 2.0, 20.0], 0xFFFF_0000),
    )
})?;
```

- **About the origin.** The turn is about the current pose's origin, so translate to the pivot first, as above.
  The ops of a `scope` apply in order, each after the ones before it, as vanilla composes them:
  `Translate` then `Rotate` turns about the translated point; `Rotate` then `Translate` moves along the turned axes.
- **Direction.** GUI y grows downwards, so a positive angle turns clockwise on screen.
- **Scope.** Leaving the child closure restores the pose (and any scissor) automatically, even on
  `Err` or unwind. Scopes nest without an artificial depth cap.
- **What follows it.** Everything drawn through the child canvas: fills, gradients, sprites, blits,
  items, item decorations and text. Pass `Some([x, y, w, h])` for `scissor` to clip in the same scope,
  or `None` to inherit the enclosing scissor.
- **Exactness.** The matrix is JOML's, float for float: a quarter turn of `FRAC_PI_2` leaves the same tiny
  non-zero cosine vanilla's does.
- **Refusal.** A NaN or infinite angle (or non-finite composed pose) refuses the scope with
  `GuiDrawError::NotFinite` before invoking the child closure, leaving the enclosing pose untouched.
