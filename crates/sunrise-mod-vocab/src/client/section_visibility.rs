//! Per-section and per-cell mesh visibility (`section_visibility`, the mesh visibility design):
//! the raw section a client mod reads and the frozen answer it gives before the section meshes.
//!
//! The game captures the input and asks on the client world thread, never on a meshing worker; the
//! workers read only the checked answers. Every value here is a bounded integer value, the same
//! on native and wasm.
#[allow(unused_imports)]
use crate::*;

/// Cells along each axis of a [`SectionVisibilityInput`]: the section's 16 and one each side.
pub const SECTION_INPUT_SIDE: usize = 18;
/// Cells in a [`SectionVisibilityInput`]: 18 cubed, edges and corners included.
pub const SECTION_INPUT_CELLS: usize = SECTION_INPUT_SIDE * SECTION_INPUT_SIDE * SECTION_INPUT_SIDE;
/// The most cell records one [`SectionVisibility::Keep`] may hold: one per cell of a section.
pub const MAX_CELL_RECORDS: usize = 4096;
/// A face mask with every face kept: bits down, up, north, south, west, east (0 to 5).
pub const ALL_FACES: u8 = 0b11_1111;

/// One cell of a [`SectionVisibilityInput`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub enum SectionCell {
    /// The block state there as the world holds it, before any mod hid it: a state a
    /// `block_render_visibility` part hides still reads here as itself. Above or below the
    /// world it is air.
    State(BlockStateId),
    /// A neighbouring column that is not loaded. It is not air: decide as if it neither occludes
    /// nor matches anything, and expect the section to be asked again when the column arrives.
    Missing,
}

/// The raw section `section_visibility` reads: the 16-cubed section and a one-block border on
/// every side (edges and corners too), with its light, captured once on the client world thread
/// before the section meshes. Read it with [`cell`](Self::cell) and [`light`](Self::light);
/// nothing beyond the border is available.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub struct SectionVisibilityInput {
    /// The block position of the section's lowest corner: a multiple of 16 on each axis.
    pub origin: [i32; 3],
    /// [`SECTION_INPUT_CELLS`] cells, x fastest, then z, then y, from local -1 to 16 on each axis.
    pub cells: Vec<SectionCell>,
    /// Each cell's light, in the order of `cells`: sky light in the high four bits, block light
    /// in the low four. A missing cell reads 0.
    pub light: Vec<u8>,
}

impl SectionVisibilityInput {
    /// The input of the section at `origin` from its cells and light, in the order
    /// [`cells`](Self::cells) documents. Refused with
    /// [`InputSize`](SectionVisibilityRefusal::InputSize) unless both hold
    /// [`SECTION_INPUT_CELLS`] values.
    pub fn new(
        origin: [i32; 3],
        cells: Vec<SectionCell>,
        light: Vec<u8>,
    ) -> Result<Self, SectionVisibilityRefusal> {
        if cells.len() != SECTION_INPUT_CELLS || light.len() != SECTION_INPUT_CELLS {
            return Err(SectionVisibilityRefusal::InputSize);
        }
        Ok(Self {
            origin,
            cells,
            light,
        })
    }
    /// The index in [`cells`](Self::cells) of `local` (from the section's lowest corner), each
    /// axis -1 to 16; refused with [`OutsideInput`](SectionVisibilityRefusal::OutsideInput)
    /// beyond that border.
    pub fn index(local: [i32; 3]) -> Result<usize, SectionVisibilityRefusal> {
        if local.iter().any(|v| !(-1..=16).contains(v)) {
            return Err(SectionVisibilityRefusal::OutsideInput);
        }
        let [x, y, z] = local.map(|v| (v + 1) as usize);
        Ok((y * SECTION_INPUT_SIDE + z) * SECTION_INPUT_SIDE + x)
    }
    /// The cell at `local` (from the section's lowest corner, -1 to 16 on each axis).
    pub fn cell(&self, local: [i32; 3]) -> Result<SectionCell, SectionVisibilityRefusal> {
        let index = Self::index(local)?;
        self.cells
            .get(index)
            .copied()
            .ok_or(SectionVisibilityRefusal::InputSize)
    }
    /// The sky and block light at `local`, 0 to 15 each.
    pub fn light(&self, local: [i32; 3]) -> Result<[u8; 2], SectionVisibilityRefusal> {
        let index = Self::index(local)?;
        let packed = *self
            .light
            .get(index)
            .ok_or(SectionVisibilityRefusal::InputSize)?;
        Ok([packed >> 4, packed & 15])
    }
}

/// What one cell of a section draws. Every field only takes away: a face a neighbour hides stays
/// hidden, and what one mod hides no other mod brings back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub struct CellVisibility {
    /// The cell, from the section's lowest corner: 0 to 15 on each axis.
    pub pos: [u8; 3],
    /// Whether its block model draws. A model that does not draw occludes nothing, and its
    /// unculled quads go with it.
    pub model: bool,
    /// Whether its fluid draws.
    pub fluid: bool,
    /// The faces kept, [`ALL_FACES`] for all: bit 0 down, 1 up, 2 north, 3 south, 4 west, 5
    /// east. A removed face is not drawn, for the model's and the fluid's faces toward it, and
    /// no longer hides the neighbour's face against it. Only faces that would draw can stay.
    pub faces: u8,
}

impl CellVisibility {
    /// The cell at `pos` with its model, its fluid and the faces in `faces` kept.
    pub fn new(pos: [u8; 3], model: bool, fluid: bool, faces: u8) -> Self {
        Self {
            pos,
            model,
            fluid,
            faces,
        }
    }
    /// The cell at `pos` drawn not at all: as with a hidden state, it no longer occludes, so its
    /// neighbours' faces toward it show. Its block entity still draws.
    pub fn hidden(pos: [u8; 3]) -> Self {
        Self::new(pos, false, false, 0)
    }
}

/// A mod's answer for one section in `section_visibility`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub enum SectionVisibility {
    /// The section draws, with these cells changed, each named at most once and at most
    /// [`MAX_CELL_RECORDS`] of them. `Keep(vec![])` changes nothing.
    Keep(Vec<CellVisibility>),
    /// None of the section's terrain draws: no block model, fluid or section geometry quads,
    /// and it hides nothing behind it. Block entities that draw themselves, entities and
    /// collision are untouched.
    HideTerrain,
}

impl SectionVisibility {
    /// Checks the answer as the game does before it uses any of it: the first fault refuses the
    /// whole answer, and the game then keeps the section as if this mod had not answered.
    pub fn check(&self) -> Result<(), SectionVisibilityRefusal> {
        let Self::Keep(cells) = self else {
            return Ok(());
        };
        if cells.len() > MAX_CELL_RECORDS {
            return Err(SectionVisibilityRefusal::TooManyCells {
                count: u32::try_from(cells.len()).unwrap_or(u32::MAX),
            });
        }
        let mut seen = [0u64; MAX_CELL_RECORDS / 64];
        for cell in cells {
            let [x, y, z] = cell.pos;
            if x > 15 || y > 15 || z > 15 {
                return Err(SectionVisibilityRefusal::OutsideSection { pos: cell.pos });
            }
            if cell.faces > ALL_FACES {
                return Err(SectionVisibilityRefusal::FaceMask {
                    pos: cell.pos,
                    faces: cell.faces,
                });
            }
            let index = usize::from(y) << 8 | usize::from(z) << 4 | usize::from(x);
            let bit = 1u64 << (index & 63);
            if seen[index >> 6] & bit != 0 {
                return Err(SectionVisibilityRefusal::Duplicate { pos: cell.pos });
            }
            seen[index >> 6] |= bit;
        }
        Ok(())
    }
}

/// Why a section visibility read or answer was refused. An answer refused for any of these is
/// refused whole, logged once with the mod named, and the section draws as if it had not answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub enum SectionVisibilityRefusal {
    /// `E_SECTION_VISIBILITY_OUTSIDE_INPUT`: a read beyond the one-block border.
    OutsideInput,
    /// `E_SECTION_VISIBILITY_INPUT_SIZE`: an input that does not hold 5832 cells and lights.
    InputSize,
    /// `E_SECTION_VISIBILITY_OUTSIDE_SECTION`: a cell record outside 0 to 15.
    OutsideSection {
        /// The record's position.
        pos: [u8; 3],
    },
    /// `E_SECTION_VISIBILITY_FACE_MASK`: a face mask above [`ALL_FACES`].
    FaceMask {
        /// The record's position.
        pos: [u8; 3],
        /// The mask given.
        faces: u8,
    },
    /// `E_SECTION_VISIBILITY_DUPLICATE`: two records for one cell.
    Duplicate {
        /// The cell named twice.
        pos: [u8; 3],
    },
    /// `E_SECTION_VISIBILITY_TOO_MANY`: more than [`MAX_CELL_RECORDS`] records.
    TooManyCells {
        /// How many were given.
        count: u32,
    },
    /// `E_SECTION_VISIBILITY_PANIC`: the mod's listener panicked or trapped. It is not asked again
    /// until the next `ClientHost::refresh_render_visibility`.
    Failed,
}

impl SectionVisibilityRefusal {
    /// The refusal's stable code, `E_SECTION_VISIBILITY_...`.
    pub fn code(&self) -> &'static str {
        match self {
            Self::OutsideInput => "E_SECTION_VISIBILITY_OUTSIDE_INPUT",
            Self::InputSize => "E_SECTION_VISIBILITY_INPUT_SIZE",
            Self::OutsideSection { .. } => "E_SECTION_VISIBILITY_OUTSIDE_SECTION",
            Self::FaceMask { .. } => "E_SECTION_VISIBILITY_FACE_MASK",
            Self::Duplicate { .. } => "E_SECTION_VISIBILITY_DUPLICATE",
            Self::TooManyCells { .. } => "E_SECTION_VISIBILITY_TOO_MANY",
            Self::Failed => "E_SECTION_VISIBILITY_PANIC",
        }
    }
}

impl std::fmt::Display for SectionVisibilityRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = self.code();
        match self {
            Self::OutsideInput => write!(f, "{code}: a read beyond the section's one-block border"),
            Self::InputSize => write!(f, "{code}: the input does not hold 5832 cells"),
            Self::OutsideSection { pos } => write!(f, "{code}: cell {pos:?} is outside 0 to 15"),
            Self::FaceMask { pos, faces } => {
                write!(f, "{code}: cell {pos:?} keeps faces {faces}, above 63")
            }
            Self::Duplicate { pos } => write!(f, "{code}: cell {pos:?} is named twice"),
            Self::TooManyCells { count } => {
                write!(f, "{code}: {count} cell records, more than 4096")
            }
            Self::Failed => write!(f, "{code}: the listener panicked or trapped"),
        }
    }
}

impl std::error::Error for SectionVisibilityRefusal {}
