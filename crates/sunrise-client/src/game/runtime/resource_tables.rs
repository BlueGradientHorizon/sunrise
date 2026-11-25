//! The game's shared read-only tables: `Resources` (blocks, items, recipes, mob types, biomes),
//! `UiResources` and `PackResources`.
use super::{api_values::TryIntoApi, *};
use std::path::Path;

pub(in crate::game) fn load_animate_tables(
    root: &Path,
    registry: &sunrise_world::registry::BlockRegistry,
    draws: bool,
    biomes: &Biomes,
) -> Result<(
    sunrise_client::animate_tick::AnimateTables,
    sunrise_client::fog::FogTables,
)> {
    let _step = startup::step("rest.animate_fog");
    let animate = if draws {
        sunrise_client::animate_tick::AnimateTables::load(root, registry)?
    } else {
        Default::default()
    };
    Ok((animate, sunrise_client::fog::FogTables::load(root, biomes)))
}

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
    pub(in crate::game) fn transformers(&self) -> &sunrise_client::block_transform::Transformers {
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
    pub(in crate::game) fn living_type_keys(&self) -> sunrise_client::health::LivingTypeKeys {
        self.rest().living_type_keys
    }
    pub(in crate::game) fn mob_data(&self) -> &sunrise_client::mobs::MobData {
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
    pub fn spawn_registries(&self) -> &sunrise_client::mobs::finalize::SpawnRegistries {
        &self.core.spawn_registries
    }
    pub(in crate::game) fn threads(&self) -> usize {
        self.core.threads
    }
    pub(in crate::game) fn player_type(&self) -> &Option<Arc<sunrise_client::mobs::MobType>> {
        &self.rest().player_type
    }
    pub(in crate::game) fn stations(&self) -> &sunrise_client::stations::StationData {
        &self.rest().stations
    }
}

/// Everything the title and menus draw: the GUI atlas with its font, the
/// title panorama and the splash pool. Kept apart from the world bundle so the
/// title can appear before the block sprites, models and gameplay tables are ready.
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
#[derive(Default)]
#[rustfmt::skip]
pub struct PlainTexts(
    pub(in crate::game)
        std::cell::RefCell<std::collections::HashMap<Box<str>, sunrise_client::text::Text>>,
);

impl PlainTexts {
    pub(in crate::game) const MAX: usize = 256;
}

#[cfg(test)]
thread_local! {
    pub(in crate::game) static DYNAMIC_TEXTURE_POOL_OVERRIDE: std::cell::Cell<Option<bool>> =
        const { std::cell::Cell::new(None) };
}

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
pub(in crate::game) fn dynamic_texture_pool() -> bool {
    #[cfg(test)]
    if let Some(pool) = DYNAMIC_TEXTURE_POOL_OVERRIDE.get() {
        return pool;
    }
    !mod_api::__private::mods().is_empty()
}

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

impl UiResources {
    /// What a server's game holds, since it draws nothing: the font, which books
    /// and chat measure with, and a one-pixel panorama.
    pub fn server(root: &sunrise_assets::resource_source::ResourceSource) -> Result<Self> {
        Ok(Self {
            atlas: HudAtlas::fonts(root)?,
            panorama: Arc::new(sunrise_client::gfx::panorama::Faces::colour([0; 4])),
            splashes: Arc::from(""),
            plain_texts: PlainTexts::default(),
        })
    }
}

pub(in crate::game) struct PackParts {
    pub(in crate::game) assets: Arc<Assets>,
    pub(in crate::game) render: Arc<RenderAssets>,
    pub(in crate::game) tables: Arc<MeshTables>,
    pub(in crate::game) item_renderer: sunrise_client::item_render::ItemRenderer,
    pub(in crate::game) biomes: Arc<Biomes>,
    pub(in crate::game) light: Arc<LightTables>,
}

impl PackResources {
    /// The workers' context over `biomes`: this pack's own when `biomes` is
    /// the table it was built with, else the same parts with that table.
    pub(in crate::game) fn context_with(&self, biomes: Arc<Biomes>) -> Arc<WorkerContext> {
        let old = &self.context;
        if Arc::ptr_eq(&old.biomes, &biomes) {
            return old.clone();
        }
        Arc::new(WorkerContext {
            assets: old.assets.clone(),
            render: old.render.clone(),
            tables: old.tables.clone(),
            biomes,
            light: old.light.clone(),
            hidden_states: old.hidden_states.clone(),
            part_tables: old.part_tables.clone(),
        })
    }
    /// The pack tables over their parts, which `load_resources` builds on
    /// separate workers. A cloud texture vanilla refuses fails it, as it fails vanilla's reload
    /// (`sunrise_client::clouds::load`).
    pub(in crate::game) fn assemble(
        root: &sunrise_assets::resource_source::ResourceRoot,
        parts: PackParts,
        draws: bool,
    ) -> Result<Self> {
        let PackParts {
            assets,
            render,
            tables,
            item_renderer,
            biomes,
            light,
        } = parts;
        let water_overlay_state = assets
            .registry
            .default_state("minecraft:water")
            .ok_or("block registry is missing the default minecraft:water state")?
            .try_into_api()
            .map_err(|e| format!("cannot bind the default minecraft:water state: {e:?}"))?;
        // These pack-local resources do not depend on one another. Decode the
        // images and language map together so a custom-sky pack cannot put all
        // of the pack assembly on the critical path behind its own files.
        let (names, clouds, forcefield, custom_sky, environments, polytone_particles) =
            std::thread::scope(|scope| -> Result<_> {
                let names = named_scope(scope, "pack.names", || Names::load(root.source()));
                let (clouds, forcefield, custom_sky, environments, particles) = if draws {
                    let clouds = named_scope(scope, "pack.clouds", || {
                        sunrise_client::clouds::load(root.source())
                    });
                    let forcefield =
                        named_scope(scope, "pack.forcefield", || load_forcefield(root.source()));
                    let custom_sky = named_scope(scope, "pack.custom_sky", || {
                        sunrise_client::custom_sky::load(root.source())
                    });
                    let environments =
                        named_scope(scope, "pack.environments", || pack_environments(root));
                    let particles = named_scope(scope, "pack.particles", || {
                        pack_particles(root, assets.registry)
                    });
                    (
                        clouds.join().unwrap().map_err(|e| e.to_string())?,
                        forcefield.join().unwrap(),
                        custom_sky.join().unwrap(),
                        environments.join().unwrap(),
                        particles.join().unwrap(),
                    )
                } else {
                    (
                        Default::default(),
                        Default::default(),
                        sunrise_client::custom_sky::empty(),
                        None,
                        None,
                    )
                };
                Ok((
                    names.join().unwrap(),
                    clouds,
                    forcefield,
                    custom_sky,
                    environments,
                    particles,
                ))
            })?;
        let polytone_post = if draws {
            sunrise_client::polytone_post::PolytonePost::read(
                root.source(),
                sunrise_client::packs::polytone_configs(&sunrise_client::packs::Source::Opened(
                    root.source().clone(),
                )),
            )
            .map(Arc::new)
        } else {
            None
        };
        // Each item's own default name, which the hover name reads as `getDescriptionId` does.
        let names = names.with_item_names(&item_renderer.models.prototypes, assets.registry);
        Ok(Self {
            layer_names: Arc::new(sunrise_client::block_sprites::texture_names(
                &render.sprite_layers,
            )),
            context: Arc::new(WorkerContext {
                assets: assets.clone(),
                render: render.clone(),
                tables,
                biomes,
                light,
                hidden_states: Arc::from([]),
                part_tables: Default::default(),
            }),
            block_registries: native_block_states::block_registries(assets.registry),
            assets,
            water_overlay_state,
            render,
            item_renderer: Arc::new(item_renderer),
            source: root.source().clone(),
            data_source: root.data().clone(),
            data_changes: root.data_changes().clone(),
            additional_data: Arc::from([]),
            mod_layers: root.mod_layers().clone(),
            names: Arc::new(names),
            clouds: Arc::new(clouds),
            forcefield: Arc::new(forcefield),
            custom_sky,
            environments,
            polytone_particles,
            polytone_post,
            polytone_colors: if draws {
                Arc::new(read_colors(root))
            } else {
                Arc::default()
            },
        })
    }
}

/// The overworld's, Nether's and End's attribute stacks with the Polytone biome and dimension
/// modifiers of the packs merged at `root` (#11486); `None` when no pack has any. The overlay
/// root links the vanilla data the stacks start from.
pub(in crate::game) fn pack_environments(
    root: &sunrise_assets::resource_source::ResourceRoot,
) -> Option<Arc<[Environment; 3]>> {
    let modifiers = sunrise_client::environment::PackModifiers::read(root.source());
    if modifiers.is_empty() {
        return None;
    }
    let modifiers = modifiers.with_configs(sunrise_client::packs::polytone_configs(
        &sunrise_client::packs::Source::Opened(root.source().clone()),
    ));
    let load = |d: &str| Environment::load_dimension_with(root.data(), d, &modifiers);
    match (load("overworld"), load("the_nether"), load("the_end")) {
        (Ok(o), Ok(n), Ok(e)) => Some(Arc::new([o, n, e])),
        (o, n, e) => {
            for error in [o.err(), n.err(), e.err()].into_iter().flatten() {
                log::warn!("pack environment: {error}");
            }
            None
        }
    }
}

/// The custom particles and block emitters of the packs merged at `root` (#11486); `None` when
/// no pack has any.
pub(in crate::game) fn pack_particles(
    root: &sunrise_assets::resource_source::ResourceRoot,
    blocks: &'static sunrise_world::registry::BlockRegistry,
) -> Option<Arc<sunrise_client::particles::polytone::Registry>> {
    let configs = sunrise_client::packs::polytone_configs(&sunrise_client::packs::Source::Opened(
        root.source().clone(),
    ));
    let registry =
        sunrise_client::particles::polytone::Registry::read(root.source(), configs, blocks);
    (!registry.is_empty()).then(|| Arc::new(registry))
}

/// The packs' custom colours; their dye and sheep colours go to the draw paths' table.
pub(in crate::game) fn read_colors(
    root: &sunrise_assets::resource_source::ResourceRoot,
) -> sunrise_client::pack_palette::PackColors {
    let colors = sunrise_client::pack_palette::read(root.source());
    sunrise_client::pack_palette::set(&colors);
    sunrise_client::brewing::set_pack_colors(colors.effects.clone());
    colors
}

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
