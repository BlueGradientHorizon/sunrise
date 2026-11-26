use crate::game::api_values::TryIntoApi;
use crate::game::api_values::TryIntoEngine;
use crate::game::api_values::engine_display_context;
use sunrise_items::items::Items;

impl TryIntoApi for (sunrise_game_data::text::Text, u32) {
    type Api = (mod_api::Text, u32);
    type Error = mod_api::text::TextFormError;
    fn try_into_api(&self) -> Result<Self::Api, Self::Error> {
        Ok((self.0.try_into_api()?, self.1))
    }
}

impl TryIntoApi for sunrise_client::boss_overlay::ModBar {
    type Api = mod_api::ClientBossBar;
    type Error = mod_api::text::TextFormError;

    fn try_into_api(&self) -> Result<Self::Api, Self::Error> {
        Ok(mod_api::ClientBossBar {
            uuid: self.uuid,
            name: self.name.try_into_api()?,
            progress: self.progress,
            color: self.color,
            overlay: self.overlay,
            darken_screen: self.darken_screen,
            play_boss_music: self.play_boss_music,
            create_world_fog: self.create_world_fog,
        })
    }
}

impl TryIntoEngine for mod_api::ItemDisplayContext {
    type Engine = sunrise_assets::item_model::DisplayContext;
    type Error = ();
    fn try_into_engine(&self, _: &Items) -> Result<Self::Engine, Self::Error> {
        engine_display_context(*self).ok_or(())
    }
}

impl<T: TryIntoApi, E: Clone> TryIntoApi for Result<T, E> {
    type Api = Result<T::Api, E>;
    type Error = T::Error;
    fn try_into_api(&self) -> Result<Self::Api, Self::Error> {
        match self {
            Ok(value) => value.try_into_api().map(Ok),
            Err(error) => Ok(Err(error.clone())),
        }
    }
}

#[cfg(feature = "client")]
impl super::Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn api_block_entity_submit(
        &self,
        pos: [i32; 3],
        block: &str,
        state: sunrise_world::registry::BlockStateId,
        entity: &sunrise_level::containers::ModBlockEntity,
        draw: &mut super::native_block_entity_draw::BlockEntityDraw,
    ) {
        if !mod_api::__private::has_part_hook(mod_api::HookId::BlockEntitySubmit) {
            return;
        }
        if let Ok(api) = entity.try_into_api() {
            self.native_block_entity_submit(pos, block, state, &api, draw);
        }
    }
}
