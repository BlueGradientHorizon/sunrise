# Hiding terrain per section and per cell

`block_render_visibility` hides a block state everywhere. `section_visibility` decides where: for each chunk
section about to mesh, a client mod reads the raw section and answers what of it draws. A cutaway above the
player, an x-ray of one area or a structure preview that hides the terrain under it are such answers.

```rust,ignore
fn section_visibility(&self, _: &dyn ClientHost, event: &mut SectionVisibilityEvent<'_>) {
    if event.input.origin[1] > self.cut_above {
        event.answers.push(SectionVisibility::HideTerrain);
        return;
    }
    let mut cells = Vec::new();
    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                if event.input.cell([x, y, z]) == Ok(SectionCell::State(self.stone)) {
                    // Only the stone's top face draws.
                    cells.push(CellVisibility::new([x as u8, y as u8, z as u8], true, true, 1 << 1));
                }
            }
        }
    }
    event.answers.push(SectionVisibility::Keep(cells));
}
```

## What the mod reads

`event.input` is the section and a one-block border on every side, edges and corners too: 18 cubed cells, local
-1 to 16 on each axis, with each cell's sky and block light. A cell is the state the world holds, before any
visibility answer, so an ore a `block_render_visibility` part hides still reads as the ore. A column that is not
loaded reads `SectionCell::Missing`: not air, but neither occluding nor matching anything. `cell` refuses a read
beyond the border with `SectionVisibilityRefusal::OutsideInput`.

## What the answer does

- `HideTerrain`: none of the section's block models, fluids or section geometry quads draw, and the section hides
  nothing behind it. Block entities that draw themselves, entities and collision are untouched.
- `Keep(cells)`: each `CellVisibility` names a cell (0 to 15 on each axis) and whether its model and its fluid
  draw, and a face mask: bit 0 down, 1 up, 2 north, 3 south, 4 west, 5 east, 63 for all.

Answers only take away. A face mask removes faces after the normal culling and never shows a face the game
culled. A cell with neither model nor fluid stops occluding, as a hidden state does, so its neighbours' faces
toward it draw, across a section border too; its block entity still draws. A cell with faces removed occludes only
through the faces it keeps, and the section's see-through graph is worked out again from what still occludes.
Unculled quads (a torch's) go with the model. Translucent faces stay in the translucent pass. Every mod's answer
is combined with the others' by intersection: no mod brings back what another hid.

## Bounds and refusals

The game checks each mod's answer whole before it uses any of it. More than 4096 cell records
(`E_SECTION_VISIBILITY_TOO_MANY`), a cell outside 0 to 15 (`E_SECTION_VISIBILITY_OUTSIDE_SECTION`), a face mask
above 63 (`E_SECTION_VISIBILITY_FACE_MASK`) or a cell named twice (`E_SECTION_VISIBILITY_DUPLICATE`) refuses that
mod's answer for the section, logged once per mod and code, and the section draws as if the mod had not answered.
A listener that panics (`E_SECTION_VISIBILITY_PANIC`) is switched off for this hook for the session, alone: the
other listeners are asked as before, and every loaded section is asked again without it, so no mask it gave earlier
stays in a neighbour's border or in a queued mesh.

`SectionVisibility::check` is the same check, for a mod's own tests.

## When it is asked

On the client world thread, before the section goes to a meshing worker, every time the section meshes: its
blocks or a neighbour's change, a column beside it arrives or unloads, the resources reload. The worker only reads
the frozen result. A mod whose decision changes calls `ClientHost::refresh_render_visibility`, which meshes every
loaded section again and asks anew; nothing is polled per tick or per frame. With no mod listening, nothing is
captured or asked and the mesher runs exactly as before.
