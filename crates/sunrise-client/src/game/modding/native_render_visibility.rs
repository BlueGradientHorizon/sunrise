//! States a client mod hides from the mesher (`native_block_render_visibility`): ore
//! scanners, builder cutaways, cinematic shots. The answer is per state and resolved here, so
//! meshing reads one table and never a mod.

use super::{BlockStateId, Game};
use std::sync::Arc;

impl Game<'_, crate::game::client_side::ClientState> {
    /// Asks `native_block_render_visibility` again for every state, and the fluid-face parts
    /// for every state and fluid (`resolve_part_tables`), and, when the answers differ from
    /// those in use, remeshes every loaded section with them. While a mod listens to
    /// `section_visibility`, rebuilds the player's level's meshes and prepares their visibility
    /// again. Call it when a decision changes and when the player joins, not every frame.
    pub(in crate::game) fn refresh_render_visibility(&mut self) {
        // With no section listener now or before, the section policy does not change.
        let listening = mod_api::__private::listened(mod_api::HookId::SectionVisibility);
        self.side
            .streaming
            .refresh_section_visibility(&mut self.world, listening);
        let asked = mod_api::__private::has_scoped_hook(mod_api::HookId::BlockRenderVisibility);
        let mut hidden: Arc<[bool]> = if asked {
            (0..self.pack.assets.registry.len())
                .map(|state| {
                    let mut hidden = false;
                    self.native_block_render_visibility(BlockStateId(state as u32), &mut hidden);
                    hidden
                })
                .collect()
        } else {
            Arc::from([])
        };
        // A table that hides nothing is no table at all.
        if !hidden.contains(&true) {
            hidden = Arc::from([]);
        }
        let part_tables = self.resolve_part_tables();
        if hidden == self.side.client_pack.context.mesh.hidden_states
            && part_tables == *self.side.client_pack.context.mesh.part_tables
        {
            return;
        }
        let old = &self.side.client_pack.context;
        self.side.client_pack.context = Arc::new(sunrise_client_render::streaming::WorkerContext {
            chunk: old.chunk.clone(),
            mesh: sunrise_client_render::streaming::MeshWorkerContext {
                render: old.mesh.render.clone(),
                tables: old.mesh.tables.clone(),
                hidden_states: hidden,
                part_tables: Arc::new(part_tables),
            },
        });
        let context = self.worker_context();
        self.side
            .streaming
            .reload_resources(&mut self.world, context);
    }
}
