use mod_api::*;
use sunrise_gameplay_common::game::api_values::TryIntoEngine;

impl RenderAssets for crate::game::ClientGame<'_> {
    fn model(&self, id: &Identifier) -> Result<ModelRef, ModelError> {
        self.native_model(id)
    }
    fn block_state_models(&self, state: BlockState) -> Result<BakedModelKeys<'_>, ModelError> {
        let state = state
            .try_into_engine(&())
            .map_err(|_| ModelError::UnknownModel)?;
        Ok(self.client.client_pack.render.block_state_models(state))
    }
    fn block_state_model(&self, state: BlockState) -> Result<ModelRef, ModelError> {
        let state = state
            .try_into_engine(&())
            .map_err(|_| ModelError::UnknownModel)?;
        self.native_block_state_model(state)
    }
    fn baked_model(&self, key: BakedModelKey) -> Result<BakedModelView<'_>, ModelError> {
        self.client.client_pack.render.baked_model(key)
    }
    fn render_pipeline(&self, id: &Identifier) -> Result<RenderPipelineRef, RenderPipelineError> {
        self.client.client_pack.render.render_pipeline(id)
    }
    fn animation(&self, id: &Identifier) -> Option<AnimationHolder> {
        self.client.client_pack.render.animation(id)
    }
}
