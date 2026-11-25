#[allow(unused_imports)]
pub(in crate::game::client_side) use super::native_ground::alter_ground;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::native_worldgen::prepare as prepare_block_states;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::item_snapshot::ItemBoundary;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::client_side) use crate::game::item_snapshot::ItemSnapshot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::IntoWriteResult;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::LiveLevelRandom;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::OfferWriteError;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::RecipeListing;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::STACK_PROJECTIONS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::StackReadError;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::StackReadMetadata;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::TryIntoApi;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::TryIntoEngine;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::behavior_block;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::bind_worker_blocks;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::block_entity_declarations;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::block_jump_factor;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::block_metadata_table;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::block_speed_factor;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::call_stack_write;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::call_state_write;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::call_write;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::checked_merchant_offer;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::content_declarations::{
    api_push_reaction, push_reaction,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::decode_stack_record;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::default_state;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::encode_stack;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::engine_display_context;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::game_rule_registry;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::group_block_states;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::install_block_states;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::install_parts;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::install_resolved_parts;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::install_stack_hooks;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::install_test_platform;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::is_ladder;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::menu_still_valid;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::mod_tint_values;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::package_snapshot::install_packages;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::placement_refusal_into_api;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::placement_source_into_engine;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::prepare_stack_boundary_tests;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::project_stack;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::projected_stack;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::register_game_keys;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::resolve_block_definitions;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::resolve_block_keys_from_registry;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::resolve_cached_block_definitions;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::resolve_item_registry;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::resolve_stack_boundary_test_items;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::scalar_values;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::stack_count::TryIntoEnginePreservingCount;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::stack_read_metadata;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::stack_value;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::test_snapshot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::with_live_level_random;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::with_rule_schema;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::write_answers;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::api_values::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::item::item_stack::{
    ItemStack as ApiStack, StackWriteError,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::num::NonZeroU32;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::sync::Arc;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::client_side) use sunrise_client::parallel::ListenSnapshot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::parallel::TickThread;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::{Compound, JavaString, Tag};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::random::{
    LegacyRandom, RandomSource as EngineRandom,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_items::items::{ItemStack as EngineStack, Items};

impl TryIntoApi for (sunrise_client::text::Text, u32) {
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

impl super::Game<'_> {
    pub(in crate::game) fn api_block_entity_submit(
        &self,
        pos: [i32; 3],
        block: &str,
        state: sunrise_world::registry::BlockStateId,
        entity: &crate::containers::ModBlockEntity,
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
