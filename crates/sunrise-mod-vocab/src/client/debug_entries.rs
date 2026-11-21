//! Named entries and world-space renderers a native mod adds to the debug screen (NeoForge's
//! `RegisterDebugEntriesEvent`, `RegisterDebugRenderersEvent`).

use crate::{ClientHost, WorldDrawContext, WorldRenderView};

// no wire: this callable and its captures stay in the client process.
/// Appends lines for one registered F3 text entry. The host is borrowed for this draw.
pub type DebugEntryLines = dyn Fn(&dyn ClientHost, &mut Vec<String>) + Send + Sync + 'static;

// no wire: a text entry owns a process-local callback.
/// An F3 entry registered at client startup: text in a vanilla group or a renderer toggle.
pub struct DebugEntry {
    id: String,
    text: Option<(DebugGroup, Box<DebugEntryLines>)>,
}

impl DebugEntry {
    /// Declare a text entry under a non-vanilla `<namespace>:<path>` such as `mymod:tick_cost`.
    /// Only while the entry is visible, the client calls `lines` as the registering mod.
    /// Append lines to the supplied vector; legacy colour codes (§) are supported.
    pub fn text(
        id: String,
        group: DebugGroup,
        lines: impl Fn(&dyn ClientHost, &mut Vec<String>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            id,
            text: Some((group, Box::new(lines))),
        }
    }

    /// Declare a renderer toggle under the same non-vanilla `<namespace>:<path>` as its renderer.
    /// A toggle has no text group or line callback.
    pub fn renderer(id: String) -> Self {
        Self { id, text: None }
    }

    /// The full identifier used in the debug options and profile.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The vanilla group and line callback of a text entry, or `None` for a renderer toggle.
    pub fn text_lines(&self) -> Option<(DebugGroup, &DebugEntryLines)> {
        self.text.as_ref().map(|(group, lines)| (*group, &**lines))
    }
}

type DebugRendererDraw =
    dyn Fn(&dyn ClientHost, &WorldRenderView, &mut dyn WorldDrawContext) + Send + Sync + 'static;

// no wire: the renderer owns a process-local draw callback.
/// A world-space debug renderer owned by the client until its renderer list next refreshes.
pub struct DebugRenderer {
    id: String,
    draw: Box<DebugRendererDraw>,
}

impl DebugRenderer {
    /// Bind drawing to the ID of a [`DebugEntry::renderer`] toggle, such as `mymod:pipe_network`.
    /// The client calls `draw` for each world view while the toggle is active, under the
    /// registering mod's identity. Capture any renderer-specific state in the callback.
    pub fn new(
        id: String,
        draw: impl Fn(&dyn ClientHost, &WorldRenderView, &mut dyn WorldDrawContext)
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            id,
            draw: Box::new(draw),
        }
    }

    /// The full ID of the renderer's switch in the debug options and profile.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Run this renderer's callback with the current world view and drawing context.
    pub fn draw(
        &self,
        host: &dyn ClientHost,
        view: &WorldRenderView,
        draw: &mut dyn WorldDrawContext,
    ) {
        (self.draw)(host, view, draw);
    }
}

/// The vanilla groups a debug entry's lines can join (`DebugScreenDisplayer.addToGroup`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: vanilla's debug screen text groups, matched without a wildcard
#[derive(sunrise_mod_api_macros::Wire)]
pub enum DebugGroup {
    /// `position`: the player's position, section and speed.
    Position,
    /// `light`: the light levels at the feet.
    Light,
    /// `memory`: the process's memory use.
    Memory,
    /// `looking_at_block`: the targeted block's state and tags.
    LookingAtBlock,
    /// `looking_at_entity`: the targeted entity and its tags.
    LookingAtEntity,
    /// `looking_at_fluid`: the targeted fluid's state and tags.
    LookingAtFluid,
    /// `chunk_generation`: the density of the chunk at the feet.
    ChunkGeneration,
}
