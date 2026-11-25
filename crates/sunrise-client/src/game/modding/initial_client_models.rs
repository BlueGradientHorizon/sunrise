//! First client reload: the initial game owns no published model generation until apply.
use super::{Game, PackResources, Renderer, UiResources, client_reload};
use std::sync::Arc;
use sunrise_client::gfx::PreparedModels;

pub(in crate::game) struct InitialModelGeneration {
    pub(in crate::game) pack: PackResources,
    pub(in crate::game) gui: UiResources,
}

pub(in crate::game) struct PendingClientStart {
    pub(in crate::game) models: InitialModelGeneration,
    pub(in crate::game) applies: Vec<client_reload::PreparedClientReload>,
    pub(in crate::game) pack_error: Option<String>,
    pub(in crate::game) handoff: bool,
}

impl Game<'_, crate::game::client_side::ClientState> {
    /// AtlasManager.apply: adopt the uploaded textures and the GUI sprites, without
    /// admitting the new model store or its GPU buffers.
    pub(in crate::game::client_side) fn publish_client_atlases(
        &mut self,
        atlas: &Arc<super::RenderAssets>,
        gui: UiResources,
        renderer: Option<Box<Renderer>>,
    ) -> Option<PreparedModels> {
        let mut prepared = None;
        if let Some(mut renderer) = renderer {
            prepared = renderer.take_prepared_models();
            if let Some(old) = &self.side.renderer.0 {
                renderer.resize(old.config.width, old.config.height);
            }
            self.side.renderer.0 = Some(renderer);
        }
        self.side.stitching_block_atlas.0 = Some(atlas.clone());
        self.native_texture_atlas_stitched(mod_api::TextureAtlas::Blocks);
        self.gui = gui;
        let (pool, mut names) = self.gui.atlas.dynamic_names();
        self.side.dynamic_textures.rebind(pool, &mut names);
        sunrise_client::ui_theme::reset_atlas();
        self.native_texture_atlas_stitched(mod_api::TextureAtlas::Gui);
        prepared
    }

    pub(in crate::game::client_side) fn install_model_upload(
        &mut self,
        prepared: Option<PreparedModels>,
    ) {
        self.side.stitching_block_atlas.0 = None;
        if let Some(renderer) = &mut self.side.renderer.0 {
            renderer.publish_prepared_models(prepared);
        }
    }

    pub(in crate::game::client_side) fn apply_initial_model_generation(
        &mut self,
        models: InitialModelGeneration,
        client_applies: Vec<client_reload::PreparedClientReload>,
        renderer: Option<Box<Renderer>>,
    ) -> sunrise_client::Result<()> {
        self.join_world_rest()?;
        let InitialModelGeneration { pack, gui } = models;
        let item_extensions = Self::prepare_client_item_extensions(&gui)?;
        let geometry = super::native_block_entity_draw::StandaloneGeometry::new(&pack, &gui);
        // The atlas listener applies before the model manager. Both use the same
        // prepared atlas/model bundle, but each publishes only its own store.
        let upload = self.publish_client_atlases(&pack.render, gui, renderer);
        self.apply_client_reload(client_applies, |game| {
            game.pack = pack;
            game.side.standalone_geometry = geometry;
            let context = game.worker_context();
            game.streaming.reload_resources(&mut game.world, context);
            game.install_model_upload(upload);
            game.side.armor_stand_assets.0.extensions = item_extensions;
            game.resolve_entity_renderers();
            game.native_model_baking_completed();
        });
        self.native_client_setup();
        Ok(())
    }

    /// Fixtures with an already constructed Game drive the same application path.
    #[cfg(test)]
    pub(in crate::game) fn publish_initial_client_models(
        &mut self,
        client_applies: Vec<client_reload::PreparedClientReload>,
    ) -> sunrise_client::Result<()> {
        let pack = self.pack.clone();
        let metrics = UiResources::server(&pack.source)?;
        let gui = std::mem::replace(&mut self.gui, metrics);
        self.apply_initial_model_generation(
            InitialModelGeneration { pack, gui },
            client_applies,
            None,
        )
    }
}
