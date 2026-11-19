//! A shader pack's post-processing over Sunrise's own world (spec 2.1 rows 4, 6, 8, 9; 6.1; 6.2;
//! 6.4): the world renders into its own target as with a resolution scale, its colour becomes
//! colortex0 and its depth `depthtex0`, then the pack's `prepare`, `deferred`, `composite` and
//! `final` programs run in order, and `final` draws onto the screen under the hand and HUD.
//!
//! Every colortex buffer a program reads or writes has two textures, A and B. Programs read
//! "main" and write "alt", and each buffer a program wrote flips afterwards (6.1.6). Every
//! frame starts with A as "main".
//!
//! The crate split by what each part does:
//
// - `types`: the pack's entries and exits (what it is handed, what it makes, what it refuses)
//   and the puzzle of passes, buffers and bindings they are built from.
// - `construction`: `PackPost` itself, `PackPost::new` and the pieces it builds them with.
//
// The frame's own work (`execution.rs`) stays in the client's `gfx::pack_post`, which uses
// these names where they have always been.

pub mod construction;
mod image_formats;
mod packed;
pub mod types;

pub use image_formats::{image_format, storage_formats_of};

pub use construction::*;
pub use types::*;

#[cfg(test)]
mod tests;
