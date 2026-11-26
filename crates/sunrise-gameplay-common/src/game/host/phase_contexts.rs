//! Phase-scoped context trait implementations for [`Game`].

use super::ServerGame;
use mod_api::*;
use std::sync::Arc;

macro_rules! impl_shared_phase_contexts {
    ($game:ty) => {
        impl ConfigContext for $game {
            fn setting(&self, mod_id: &str, key: &str) -> Option<String> {
                <Self as prelude::ConfigHost>::setting(self, mod_id, key)
            }

            fn config_files(&self) -> &'static dyn ConfigFiles {
                <Self as prelude::ConfigHost>::config_files(self)
            }
        }

        impl ReloadContext for $game {
            fn dist(&self) -> Dist {
                <Self as prelude::ConfigHost>::dist(self)
            }
            fn game_version(&self) -> &'static str {
                <Self as prelude::ConfigHost>::game_version(self)
            }
            fn server_data_version(&self) -> i32 {
                <Self as prelude::ConfigHost>::server_data_version(self)
            }
            fn sunrise_version(&self) -> &'static str {
                <Self as prelude::ConfigHost>::sunrise_version(self)
            }
            fn on(&self, mod_id: &str) -> bool {
                <Self as prelude::ConfigHost>::on(self, mod_id)
            }
            fn setting(&self, mod_id: &str, key: &str) -> Option<String> {
                <Self as prelude::ConfigHost>::setting(self, mod_id, key)
            }
            fn game_tests_enabled(&self) -> bool {
                sunrise_level::runtime_env::runtime_environment::get().game_tests_enabled()
            }
            fn is_loaded(&self, id: &str) -> bool {
                <Self as prelude::LevelsHost>::is_loaded(self, id)
            }
            fn item_cooldown_group(&self, stack: &ItemStack) -> String {
                <Self as prelude::PlayersHost>::item_cooldown_group(self, stack)
            }
            fn max_stack_size(&self, item: ItemKey) -> Option<u32> {
                <Self as prelude::ItemsHost>::max_stack_size(self, item)
            }
            fn max_damage(&self, item: ItemKey) -> Option<u32> {
                <Self as prelude::ItemsHost>::max_damage(self, item)
            }
            fn item_key(&self, id: &Identifier) -> Option<ItemKey> {
                <Self as prelude::ItemsHost>::item_key(self, id)
            }
            fn item_default_component(&self, item: ItemKey, component: &str) -> Option<Tag> {
                <Self as prelude::ItemsHost>::item_default_component(self, item, component)
            }
            fn hover_name(&self, stack: &ItemStack) -> Result<Text, TextFormError> {
                <Self as prelude::ItemsHost>::hover_name(self, stack)
            }
            fn item_burn_time(&self, item: ItemKey) -> Option<i32> {
                <Self as prelude::ItemsHost>::item_burn_time(self, item)
            }
            fn item_compost_chance(&self, item: ItemKey) -> Option<f32> {
                <Self as prelude::ItemsHost>::item_compost_chance(self, item)
            }
            fn entity_type_in_tag(&self, entity_type: &str, tag: &str) -> bool {
                <Self as prelude::ItemsHost>::entity_type_in_tag(self, entity_type, tag)
            }
            fn data_map_value(
                &self,
                registry: &str,
                entry: &str,
                map: &str,
            ) -> Result<Option<&Tag>, DataMapError> {
                <Self as prelude::DataMapsBackend>::data_map_value(self, registry, entry, map)
            }
            fn data_map_entries(
                &self,
                registry: &str,
                map: &str,
            ) -> Result<&[(String, Tag)], DataMapError> {
                <Self as prelude::DataMapsBackend>::data_map_entries(self, registry, map)
            }
            fn game_event_key_id(
                &self,
                key: GameEventKey,
            ) -> Result<RegistryName, GameEventKeyError> {
                <Self as prelude::KeysBackend>::game_event_key_id(self, key)
            }
            fn block_key_id(&self, key: BlockKey) -> Option<&str> {
                <Self as prelude::KeysBackend>::block_key_id(self, key)
            }
            fn item_key_id(&self, key: ItemKey) -> Option<&str> {
                <Self as prelude::KeysBackend>::item_key_id(self, key)
            }
            fn item_registry_id(&self, key: ItemKey) -> Option<u32> {
                <Self as prelude::KeysBackend>::item_registry_id(self, key)
            }
            fn effect_key_id(&self, key: EffectKey) -> Option<&str> {
                <Self as prelude::KeysBackend>::effect_key_id(self, key)
            }
            fn effect_category(&self, key: EffectKey) -> Option<EffectCategory> {
                <Self as prelude::KeysBackend>::effect_category(self, key)
            }
            fn fluid_key_id(&self, key: FluidKey) -> Option<&str> {
                <Self as prelude::KeysBackend>::fluid_key_id(self, key)
            }
            fn block_entity_type_key_id(&self, key: BlockEntityTypeKey) -> Option<&str> {
                <Self as prelude::KeysBackend>::block_entity_type_key_id(self, key)
            }
            fn entity_type_key_id(&self, key: EntityTypeKey) -> Option<RegistryName> {
                <Self as prelude::KeysBackend>::entity_type_key_id(self, key)
            }
            fn menu_key_id(&self, key: MenuKey) -> Option<RegistryName> {
                <Self as prelude::KeysBackend>::menu_key_id(self, key)
            }
            fn enchantment_key_id(&self, key: EnchantmentKey) -> Option<RegistryName> {
                <Self as prelude::KeysBackend>::enchantment_key_id(self, key)
            }
            fn biome_key_id(&self, key: BiomeKey) -> Result<&str, BiomeKeyError> {
                <Self as prelude::KeysBackend>::biome_key_id(self, key)
            }
            fn dimension_key_id(&self, key: DimensionKey) -> Option<&str> {
                <Self as prelude::LevelDimensionBackend>::dimension_key_id(self, key)
            }
            fn recipe_ingredient_item_keys(
                &self,
                ingredient: &Ingredient,
            ) -> Result<Arc<[ItemKey]>, IngredientKeysError> {
                <Self as prelude::RecipesBackend>::recipe_ingredient_item_keys(self, ingredient)
            }
        }
    };
}

impl_shared_phase_contexts!(ServerGame<'_>);
