//! The `section_visibility` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// A chunk section is about to mesh: push one `SectionVisibility` for it into `answers`, or
    /// nothing to leave it as it is. `input` is the raw section with its one-block border, read
    /// before any mod hid a state. Asked on the client world thread before the section goes to a
    /// meshing worker, every time it meshes (its blocks or a neighbour's change, a column beside
    /// it arrives or unloads, the resources reload) and for every loaded section when a mod calls
    /// `ClientHost::refresh_render_visibility`; never per frame and never on a worker. Answers
    /// only take away, and every mod's are combined, so no mod brings back what another hid. An
    /// answer that breaks a bound is refused whole, logged once, and the section draws without
    /// it. With no mod listening, the mesher pays nothing.
    #[notes = r#"## Section visibility

`section_visibility` decides, per chunk section, what the terrain mesh of that section draws: the whole section
(`SectionVisibility::HideTerrain`) or, per cell, the block model, the fluid and a six-bit face mask
(`SectionVisibility::Keep` with `CellVisibility` records). It extends `block_render_visibility`, which hides a
state everywhere, for decisions that depend on where a block is: a cutaway above the player, an x-ray of one
area, a structure preview that hides the terrain under it.

```rust,ignore
fn section_visibility(&self, _: &dyn ClientHost, event: &mut SectionVisibilityEvent<'_>) {
    if event.input.origin[1] >= 64 {
        event.answers.push(SectionVisibility::HideTerrain);
        return;
    }
    // Hide every cell of the mod's marker block, wherever it is in the section.
    let mut cells = Vec::new();
    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                if event.input.cell([x, y, z]) == Ok(SectionCell::State(self.marker)) {
                    cells.push(CellVisibility::hidden([x as u8, y as u8, z as u8]));
                }
            }
        }
    }
    event.answers.push(SectionVisibility::Keep(cells));
}
```

The input is the 16-cubed section and a one-block border on all six sides, edges and corners included (5832
cells), with each cell's light. A cell is the state the world holds, before any visibility answer: an ore a
`block_render_visibility` part hides still reads as the ore. A neighbouring column not loaded reads `Missing`:
not air, but neither occluding nor matching; the section is asked again when the column arrives or unloads.
`SectionVisibilityInput::cell` refuses reads beyond the border.

Answers only take away. A face mask removes faces after normal culling and never forces a culled face to show. A
cell with neither model nor fluid no longer occludes, so its neighbours' faces toward it draw (also across a
section border); a cell with faces removed occludes only through the faces it keeps, and the section's
see-through graph is worked out again from what still occludes, erring toward drawing what is behind. Unculled
quads go with the model. `HideTerrain` drops the section's block models, fluids and section geometry quads and
hides nothing behind it; block entities that draw themselves still draw. All mods' answers are combined by
intersection.

An answer is checked whole before any of it is used: a cell outside 0 to 15, a face mask above 63, a cell named
twice or more than 4096 records refuses that mod's whole answer for the section with one log line
(`E_SECTION_VISIBILITY_...`), and the section draws as if it had not answered. A listener that panics or traps is
not asked again until the next `refresh_render_visibility`. The decision is asked on the client world thread, so
it may read the `ClientHost`; it changes when the section meshes again, which a mod asks for with
`ClientHost::refresh_render_visibility`, never by polling."#]
    #[hook(
        side = "client",
        contract = "collect",
        out = "answers",
        answered_by = "mod",
        neoforge = "none (Sunrise-only; NeoForge has no counterpart)"
    )]
    fn section_visibility(
        input: &SectionVisibilityInput,
        answers: &mut Vec<Answered<SectionVisibility>>,
    );
}
