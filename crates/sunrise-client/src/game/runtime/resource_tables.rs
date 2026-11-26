//! The game's shared read-only tables: `Resources` (blocks, items, recipes, mob types, biomes),
//! `UiResources` and `PackResources`.
use super::*;
use sunrise_level::chunk_streamer::ChunkWorkerContext;

impl Resources {
    pub(in crate::game) fn rest(&self) -> &WorldRest {
        self.rest
            .get()
            .expect("world resources joined before entry")
    }
    pub(in crate::game) fn facts(&self) -> &BlockFacts {
        &self.core.facts
    }
    pub(in crate::game) fn item_defaults(&self) -> &sunrise_world::item_defaults::ItemDefaults {
        self.core.item_data.defaults()
    }
    pub(in crate::game) fn items(&self) -> &Items {
        &self.rest().items
    }
    pub(in crate::game) fn shared_items(&self) -> Arc<Items> {
        self.rest().items.clone()
    }
    pub(in crate::game) fn recipes(&self) -> &Recipes {
        &self.rest().recipes
    }
    pub(in crate::game) fn transformers(&self) -> &sunrise_sim::block_transform::Transformers {
        &self.rest().transformers
    }
    pub(in crate::game) fn biomes(&self) -> &Arc<Biomes> {
        &self.core.biomes
    }
    pub(in crate::game) fn environment(&self) -> &Environment {
        &self.core.environment
    }
    /// The environment of a dimension type id; a type with no table of its own gets the overworld's.
    pub(in crate::game) fn environment_of(&self, dimension_type: &str) -> &Environment {
        match dimension_type {
            "minecraft:the_nether" => self.nether_environment(),
            "minecraft:the_end" => self.end_environment(),
            _ => self.environment(),
        }
    }
    pub(in crate::game) fn mob_types(&self) -> &HashMap<String, Arc<MobType>> {
        &self.rest().mob_types
    }
    /// The player's, the armor stand's and the dragon's registry keys, resolved with these
    /// resources.
    pub(in crate::game) fn living_type_keys(&self) -> sunrise_sim::health::LivingTypeKeys {
        self.rest().living_type_keys
    }
    pub(in crate::game) fn mob_data(&self) -> &sunrise_sim::mobs::MobData {
        &self.rest().mob_data
    }
    pub(in crate::game) fn providers(&self) -> &Providers {
        &self.rest().providers
    }
    pub(in crate::game) fn nether_environment(&self) -> &Environment {
        &self.rest().nether_environment
    }
    pub(in crate::game) fn end_environment(&self) -> &Environment {
        &self.rest().end_environment
    }
    pub(in crate::game) fn behaviours(&self) -> &Behaviours {
        &self.rest().behaviours
    }
    pub(in crate::game) fn collision(&self) -> &BlockCollisionRegistry<'static> {
        &self.core.collision
    }
    pub(in crate::game) fn suffocation(
        &self,
    ) -> &sunrise_world::suffocation::SuffocationRegistry<'static> {
        &self.core.suffocation
    }
    pub(in crate::game) fn motion(&self) -> &MotionResources {
        &self.core.motion
    }
    /// The variant registries finalizeSpawn selects from
    /// (`data/minecraft/<registry>`), shared with world generation.
    pub fn spawn_registries(&self) -> &sunrise_sim::mobs::finalize::SpawnRegistries {
        &self.core.spawn_registries
    }
    pub(in crate::game) fn threads(&self) -> usize {
        self.core.threads
    }
    pub(in crate::game) fn stations(&self) -> &sunrise_sim::stations::StationData {
        &self.rest().stations
    }
}

/// Everything the title and menus draw: the GUI atlas with its font, the
/// title panorama and the splash pool. Kept apart from the world bundle so the
/// title can appear before the block sprites, models and gameplay tables are ready.
#[cfg(feature = "client")]
pub struct UiResources {
    /// The GUI, font, particle, map and skin atlas.
    pub atlas: HudAtlas,
    /// The title screen's panorama faces.
    pub panorama: Arc<sunrise_client::gfx::panorama::Faces>,
    /// The title screen's splashes (`sunrise_client::splashes::pool`).
    pub splashes: Arc<str>,
    /// The `Text` of each plain string mods draw (`DrawContext::text`).
    pub plain_texts: PlainTexts,
}

/// The `Text` of each plain string a mod draws by `&str` (`DrawContext::text`), kept so drawing
/// the same string again builds no new `Text`. It holds at most [`PlainTexts::MAX`] strings and
/// starts over past that, so text that changes every frame can't grow it.
#[cfg(feature = "client")]
#[derive(Default)]
#[rustfmt::skip]
pub struct PlainTexts(
    pub(in crate::game)
        std::cell::RefCell<std::collections::HashMap<Box<str>, sunrise_client_core::text::Text>>,
);

#[cfg(feature = "client")]
impl PlainTexts {
    pub(in crate::game) const MAX: usize = 256;
}

#[cfg(feature = "client")]
#[cfg(test)]
thread_local! {
    pub(in crate::game) static DYNAMIC_TEXTURE_POOL_OVERRIDE: std::cell::Cell<Option<bool>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(feature = "client")]
#[cfg(test)]
pub(in crate::game) fn with_dynamic_texture_pool_override<R>(
    pool: bool,
    f: impl FnOnce() -> R,
) -> R {
    struct Restore(Option<bool>);
    impl Drop for Restore {
        fn drop(&mut self) {
            DYNAMIC_TEXTURE_POOL_OVERRIDE.set(self.0);
        }
    }
    let _restore = Restore(DYNAMIC_TEXTURE_POOL_OVERRIDE.replace(Some(pool)));
    f()
}

/// Whether the HUD atlas packs the pool of dynamic texture slots: whenever a client mod is
/// loaded, since any of them can call `ClientHost::create_dynamic_texture`, with
/// no mixin and no opt-in (#4437). A game with no mod packs no slot and its atlas does not grow.
/// Read where the atlas is built: at the first load and on a resource reload.
#[cfg(feature = "client")]
pub(in crate::game) fn dynamic_texture_pool() -> bool {
    #[cfg(test)]
    if let Some(pool) = DYNAMIC_TEXTURE_POOL_OVERRIDE.get() {
        return pool;
    }
    !mod_api::__private::mods().is_empty()
}

#[cfg(feature = "client")]
#[cfg(test)]
impl UiResources {
    pub(in crate::game) fn clone_for_tests(&self) -> Self {
        Self {
            atlas: self.atlas.clone_for_tests(),
            panorama: self.panorama.clone(),
            splashes: self.splashes.clone(),
            plain_texts: PlainTexts::default(),
        }
    }
}

pub(in crate::game) struct PackParts {
    pub(in crate::game) assets: Arc<Assets>,
    pub(in crate::game) biomes: Arc<Biomes>,
    pub(in crate::game) light: Arc<LightTables>,
    pub(in crate::game) item_models: Arc<sunrise_assets::item_model::ItemModels>,
    pub(in crate::game) item_ids: Arc<HashMap<String, i32>>,
}

impl PackResources {
    pub(in crate::game) fn chunk_context_with(
        &self,
        biomes: Arc<Biomes>,
    ) -> Arc<ChunkWorkerContext> {
        let old = &self.chunk_context;
        if Arc::ptr_eq(&old.biomes, &biomes) {
            return old.clone();
        }
        Arc::new(ChunkWorkerContext {
            assets: old.assets.clone(),
            biomes,
            light: old.light.clone(),
        })
    }

    pub(in crate::game) fn assemble(
        root: &sunrise_assets::resource_source::ResourceRoot,
        parts: PackParts,
    ) -> Result<Self> {
        let PackParts {
            assets,
            biomes,
            light,
            item_models,
            item_ids,
        } = parts;
        let names =
            Names::load(root.source()).with_item_names(&item_models.prototypes, assets.registry);
        Ok(Self {
            chunk_context: Arc::new(ChunkWorkerContext {
                assets: assets.clone(),
                biomes,
                light,
            }),
            block_registries: native_block_states::block_registries(assets.registry),
            assets,
            source: root.source().clone(),
            data_source: root.data().clone(),
            data_changes: root.data_changes().clone(),
            additional_data: Arc::from([]),
            mod_layers: root.mod_layers().clone(),
            names: Arc::new(names),
            item_models,
            item_ids,
        })
    }

    pub(in crate::game) fn item_registry_id(&self, item: &str) -> Option<u32> {
        sunrise_sim::item_models::item_registry_id(&self.item_ids, item)
    }
}

#[cfg(feature = "client")]
#[cfg(test)]
pub(in crate::game) mod plain_text_tests {
    use super::PlainTexts;

    #[test]
    fn a_string_drawn_again_reuses_its_text_and_the_store_stays_bounded() {
        let texts = PlainTexts::default();
        let first = texts.with("Jump", |t| t as *const _);
        assert_eq!(texts.with("Jump", |t| t as *const _), first);
        assert_eq!(texts.with("Jump", |t| t.plain()), "Jump");
        for i in 0..PlainTexts::MAX * 2 {
            texts.with(&i.to_string(), |_| ());
        }
        assert!(texts.0.borrow().len() <= PlainTexts::MAX);
    }
}
