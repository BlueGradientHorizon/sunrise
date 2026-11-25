use super::*;
use crate::game::resource_tables::PackParts;
use crate::game::resource_tables::PlainTexts;
use crate::game::resource_tables::UiResources;
use crate::game::resource_tables::dynamic_texture_pool;
use std::path::Path;

#[cfg(feature = "client")]
impl Game<'_, crate::game::client_side::ClientState> {
    /// A title screen over the current resources and their splash pool.
    pub(crate) fn new_title(&self, saves: PathBuf) -> Title {
        Title::new(saves, &self.options.resources, &self.gui.splashes)
    }
}

impl PackResources {
    /// Everything a pack overlay at `root` provides, over `assets` and `render`
    /// loaded from it.
    pub(in crate::game) fn load(
        root: &sunrise_assets::resource_source::ResourceRoot,
        assets: Arc<Assets>,
        render: Arc<RenderAssets>,
        facts: &BlockFacts,
        biomes: Arc<Biomes>,
        light: Arc<LightTables>,
        item_data: &sunrise_world::item_defaults::ItemData,
    ) -> Result<Self> {
        let visibility = models::load_visibility(root.prepared(), assets.registry)?;
        api_values::resolve_block_definitions(assets.registry)?;
        mods::extend_poi_types(assets.registry);
        mods::register_cauldron_fluid_contents(assets.registry);
        let tables = Arc::new(
            MeshTables::new(
                &assets,
                &render,
                facts,
                visibility,
                api_values::mod_tint_values(assets.registry)?,
            )?
            .with_polytone_colormaps(root.source(), assets.registry),
        );
        let item_renderer = sunrise_client::item_render::ItemRenderer::load_with_library(
            root,
            render.model_library.clone(),
            item_data,
        )?;
        Self::assemble(
            root,
            PackParts {
                assets,
                render,
                tables,
                item_renderer,
                biomes,
                light,
            },
            true,
        )
    }
}

impl PlainTexts {
    /// `draw` with the `Text` of `text`, made the first time it is asked for.
    pub(in crate::game) fn with<R>(
        &self,
        text: &str,
        draw: impl FnOnce(&sunrise_client::text::Text) -> R,
    ) -> R {
        let mut texts = self.0.borrow_mut();
        if !texts.contains_key(text) {
            if texts.len() >= Self::MAX {
                texts.clear();
            }
            texts.insert(text.into(), sunrise_client::text::Text::literal(text));
        }
        draw(&texts[text])
    }
}

impl Resources {
    pub(in crate::game) fn ambient(&self) -> &sunrise_client::ambience::AmbientTables {
        &self.rest().ambient
    }
    pub(in crate::game) fn animate(&self) -> &sunrise_client::animate_tick::AnimateTables {
        &self.rest().animate
    }
    pub(in crate::game) fn block_entity_render(&self) -> &BlockEntityRenderer {
        &self.rest().block_entity_render
    }
    pub(in crate::game::client_side) fn fog(&self) -> &sunrise_client::fog::FogTables {
        &self.rest().fog
    }
    pub(in crate::game) fn music(&self) -> &sunrise_client::music::MusicTables {
        &self.rest().music
    }
    pub(in crate::game::client_side) fn player_arm(
        &self,
        slim: bool,
        left: bool,
    ) -> &Option<sunrise_client::mobs::Part> {
        let arms = if slim {
            &self.rest().player_arm_slim
        } else {
            &self.rest().player_arm
        };
        &arms[usize::from(left)]
    }
    pub(in crate::game::client_side) fn spin_attack(&self) -> &Option<sunrise_client::mobs::Part> {
        &self.rest().spin_attack
    }
}

impl UiResources {
    /// The resolved resource generation, including the base layer and pack filters, over the
    /// game's own resources at `vanilla` (which tell the packs' core shaders from the game's).
    /// Its splash pool is empty until `with_splashes`.
    pub fn load(
        root: &sunrise_assets::resource_source::ResourceSource,
        vanilla: &Path,
    ) -> Result<Self> {
        // The title screen waits for both: the panorama's six faces decode
        // beside the atlas.
        std::thread::scope(|scope| {
            let panorama = named_scope(scope, "panorama", || {
                startup::time("ui.panorama", || {
                    sunrise_client::gfx::panorama::Faces::load(root, root)
                })
            });
            let pool = dynamic_texture_pool();
            let game = sunrise_assets::resource_source::ResourceSource::directory(vanilla.into());
            let atlas = startup::time("ui.atlas", || {
                HudAtlas::build_over_with_dynamic_textures(root, root, &game, pool)
            });
            let panorama = panorama
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
            Ok(Self {
                atlas: atlas?,
                panorama: Arc::new(panorama),
                splashes: Arc::from(""),
                plain_texts: PlainTexts::default(),
            })
        })
    }
    /// The title's splash pool from `stack` (`sunrise_client::splashes::pool`).
    pub fn with_splashes(
        mut self,
        stack: &sunrise_assets::resource_source::ResourceSource,
        vanilla: &std::path::Path,
        skip_if_modified: bool,
    ) -> Self {
        self.splashes = sunrise_client::splashes::pool(stack, vanilla, skip_if_modified).into();
        self
    }
}
