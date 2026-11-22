# Turning what a GUI canvas draws

Pass `PoseOp::Rotate(angle)` to [the canvas's scope](gui-paint.md#drawing-directly-with-a-canvas)
to turn by `angle` radians, as vanilla's `Matrix3x2fStack.rotate` does. Combine it with
`PoseOp::Translate(x, y)` and `PoseOp::Scale(x, y)` to draw a compass needle, a minimap arrow,
or a tilted label.

```rust,ignore
use mod_api::client::hud_layout::PoseOp;

// A needle 20 pixels long turned by `heading` about (cx, cy).
draw.draw_gui(&mut |canvas| {
    canvas.scope(
        &[PoseOp::Translate(cx, cy), PoseOp::Rotate(heading)],
        None,
        &mut |needle| needle.fill([-1.0, -20.0, 2.0, 20.0], 0xFFFF_0000),
    )
})?;
```

- **Order and direction.** Operations apply in order. `Translate` then `Rotate` turns about the
  translated point; `Rotate` then `Translate` moves along the turned axes. GUI y grows downwards,
  so a positive angle turns clockwise on screen.
- **Scope.** The pose applies to everything drawn through the child canvas, including its nested
  scopes. Returning to the parent restores the earlier pose and scissor. If this scope supplies
  a scissor, it transforms the rectangle's two ordered corners at entry, not the bounding box
  of a rotated rectangle. That screen-space clip does not turn with a later child.
- **Exactness.** The matrix is JOML's, float for float: a quarter turn of `FRAC_PI_2` leaves the
  same tiny non-zero cosine vanilla's does.
- **Refusal.** A NaN or infinite angle returns `GuiDrawError::NotFinite` before the callback
  runs. Handle that error to keep drawing in the parent, or propagate it to discard the group.
