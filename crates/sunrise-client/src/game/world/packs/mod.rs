//! Resource pack selection bound to the game: the pack screen lists, and a
//! full resource reload after a change, while playing.
#[cfg(feature = "client")]
use super::{
    ClientPackResources, ClientResources, Game, PackResources, Resources, UiResources,
    build_renderer, prepare_renderer_textures,
};
use mod_api::__private::game::*;
use mod_api::{__private::reload::ReloadFiles, FileToIdConverter, Identifier};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use sunrise_assets::resource_source::ResourceRoot;
#[cfg(feature = "client")]
use sunrise_client::gfx::PreparedTextures;
#[cfg(feature = "client")]
use sunrise_client::{hud::HudAtlas, level_loading::OverlayFade, pack_browser};
use sunrise_client::{
    pack_screen::{self, Compat, PackInfo, Source as PackSource},
    packs::{self, Pack, Position},
};
#[cfg(feature = "client")]
use sunrise_client_core::lang;
use sunrise_client_core::text::{Style, Text};
#[cfg(feature = "client")]
use sunrise_client_render::streaming::WorkerContext;
#[cfg(feature = "client")]
use sunrise_level::assets::Assets;
use sunrise_level::datapack::DataPack;

/// Where overlay roots are built (beside options.txt).
pub fn cache_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_default()
        .join(".sunrise-cache/resourcepacks")
}
pub(in crate::game::client_side) fn packs_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_default()
        .join("resourcepacks")
}
/// `Blaze3D.openPath`: the desktop's handler on the folder (the default file manager).
pub(in crate::game::client_side) fn open_folder(dir: &Path) {
    if let Err(e) = sunrise_client::desktop::open_path(dir) {
        log::warn!("Sunrise: could not open {}: {e}", dir.display());
    }
}
/// `Blaze3D.openUri`: the system browser on the link.
pub fn open_uri(uri: &str) {
    if let Err(error) = sunrise_client::desktop::open_uri(uri) {
        log::warn!("Sunrise: could not open URI {uri}: {error}");
    }
}
/// The accepted content root and the result of layering selected client packs over it.
/// Recovery keeps the required packs and uses the same content root.
pub(in crate::game) struct PackRoots {
    pub(in crate::game) base: ResourceRoot,
    pub(in crate::game) selected: std::io::Result<ResourceRoot>,
    pub(in crate::game) cleared: Option<Vec<Pack>>,
}

impl PackRoots {
    pub(in crate::game::client_side) fn for_selection(
        base: ResourceRoot,
        all: &[Pack],
        ids: &[String],
        server: &[Pack],
    ) -> Self {
        let mut enabled = packs::enabled(all, ids);
        let cleared = packs::has_optional_pack(&enabled).then(|| packs::enabled(all, &[]));
        enabled.extend_from_slice(server);
        let selected = layer_client_packs(&base, &enabled);
        Self {
            base,
            selected,
            cleared,
        }
    }
}

impl From<ResourceRoot> for PackRoots {
    fn from(base: ResourceRoot) -> Self {
        Self {
            selected: Ok(base.clone()),
            base,
            cleared: None,
        }
    }
}

/// The selection and a server's packs (fixed at the top, `Pack.Position.TOP`) are laid over
/// [`super::mods::content_root`], so required mod resources sit below the player's packs.
/// Built-in packs are found in `vanilla`.
pub(in crate::game) fn startup_root(
    vanilla: &Path,
    selected: &[String],
    server: &[Pack],
    mod_sources: &super::mods::ModsState,
) -> sunrise_client_core::Result<PackRoots> {
    let mut all = packs::discover(vanilla, &packs_dir());
    all.extend(super::pack_finders::discover(
        mod_api::PackType::ClientResources,
        mod_sources,
    ));
    let base = super::mods::content_root(vanilla, mod_sources)?;
    Ok(PackRoots::for_selection(base, &all, selected, server))
}

/// `Minecraft.clearResourcePacksOnError`: save the empty selection before rebuilding its layers.
pub(in crate::game) fn clear_selection(settings: &mut super::Settings) {
    log::info!("Caught error loading resourcepacks, removing all selected resourcepacks");
    settings.resource_packs.clear();
    settings.file.set(
        "incompatibleResourcePacks",
        sunrise_client::options_file::V::List(Vec::new()),
    );
    settings.save(&crate::game::options_path());
}

/// Client packs overlay assets only, never the prepared tables or passive server data.
pub(in crate::game) fn layer_client_packs(
    base: &ResourceRoot,
    enabled: &[Pack],
) -> std::io::Result<ResourceRoot> {
    let source = packs::resources(base.source().clone(), enabled)?;
    Ok(
        ResourceRoot::new(base.prepared().to_path_buf(), cache_dir(), source)
            .with_data(base.data().clone(), base.data_changes().clone())
            .with_mod_layers(base.mod_layers().clone()),
    )
}
pub(in crate::game::client_side) fn info(p: &Pack) -> PackInfo {
    if let Some(info) = &p.selection {
        return info.clone();
    }
    let built_in = !p.id.starts_with("file/");
    PackInfo {
        id: p.id.clone(),
        title: p.title.clone(),
        description: p.raw_description.clone(),
        description_style: p.description_style.clone(),
        source: if built_in {
            PackSource::BuiltIn
        } else {
            PackSource::Default
        },
        compat: p.compatibility,
        icon: p.icon_png().as_deref().and_then(pack_screen::load_icon),
        required: false,
        position: Position::Top,
        fixed: false,
        hidden: false,
    }
}
/// A pack from `DownloadedPackSource.loadRequestedPacks`: fixed at the top of
/// the selected list (`DOWNLOADED_PACK_SELECTION`), labelled as a server pack
/// (or a world's own pack when its id is the local-world uuid).
pub(in crate::game::client_side) fn downloaded_info(p: &Pack) -> PackInfo {
    // `WorldOpenFlows.WORLD_PACK_ID` as hex in `server/{uuid}/…`.
    const WORLD: &str = "server/640a6a92b6cb48a0b391831586500359/";
    let world = p.id.starts_with(WORLD);
    PackInfo {
        id: p.id.clone(),
        title: p.title.clone(),
        description: p.raw_description.clone(),
        description_style: p.description_style.clone(),
        source: if world {
            PackSource::World
        } else {
            PackSource::Server
        },
        compat: p.compatibility,
        icon: p.icon_png().as_deref().and_then(pack_screen::load_icon),
        required: true,
        position: Position::Top,
        fixed: true,
        hidden: false,
    }
}
/// The vanilla pack: required, at the bottom (`BuiltInPackSource`), with the
/// client JAR's `pack.png` that the prepared resources hold.
pub(in crate::game::client_side) fn vanilla_info(vanilla: &Path) -> PackInfo {
    PackInfo {
        id: "vanilla".into(),
        title: Text::translate("resourcePack.vanilla.name", &[]),
        description: Text::translate("resourcePack.vanilla.description", &[]),
        description_style: Style::default(),
        source: PackSource::BuiltIn,
        compat: Compat::Compatible,
        icon: std::fs::read(vanilla.join("pack.png"))
            .ok()
            .as_deref()
            .and_then(pack_screen::load_icon),
        required: true,
        position: Position::Bottom,
        fixed: false,
        hidden: false,
    }
}
/// Packs the Resource Packs screen lists: folder and built-in packs, then any
/// active downloaded server (or world) packs, then vanilla.
pub(in crate::game) fn pack_infos(
    vanilla: &Path,
    server: &[Pack],
    additional: &[Pack],
) -> Vec<PackInfo> {
    let mut infos: Vec<PackInfo> = packs::discover(vanilla, &packs_dir())
        .iter()
        .map(info)
        .collect();
    infos.extend(additional.iter().map(info));
    infos.extend(server.iter().map(downloaded_info));
    infos.push(vanilla_info(vanilla));
    infos
}
/// The folder packs' Polytone config entries (#11486), by pack id, for those that have some:
/// what the Pack Options page edits.
pub(in crate::game::client_side) fn pack_configs(
    vanilla: &Path,
) -> std::collections::BTreeMap<String, packs::Configs> {
    packs::discover(vanilla, &packs_dir())
        .into_iter()
        .filter(|p| !p.polytone_configs.entries().is_empty())
        .map(|p| (p.id, p.polytone_configs))
        .collect()
}

/// The confirmation's callback, shared by the startup menu and in-game options.
pub(in crate::game::client_side) fn import_resource_packs(
    ui: &mut sunrise_client::options_ui::OptionsUi,
    vanilla: &Path,
    server: &[Pack],
    paths: &[PathBuf],
    additional: &[Pack],
) {
    let dir = packs_dir();
    let mut failed = false;
    for path in paths {
        match packs::copy_pack(path, &dir) {
            Ok(()) => log::info!("Sunrise: added resource pack {}", path.display()),
            Err(error) => {
                log::warn!(
                    "Sunrise: could not copy resource pack {}: {error}",
                    path.display()
                );
                failed = true;
            }
        }
    }
    ui.update_pack_infos(pack_infos(vanilla, server, additional));
    ui.pack_configs = pack_configs(vanilla);
    if failed {
        ui.resource_error = Some(Text::translate("pack.copyFailure", &[]).plain());
    }
}
/// The player changed Pack Options: `chosen` replaces their choices for `entries`, kept in
/// `polytone-configs.json`.
pub(in crate::game::client_side) fn save_pack_options(
    entries: &[String],
    chosen: std::collections::BTreeMap<String, packs::ConfigValue>,
) {
    let mut all = packs::polytone_choices();
    all.retain(|id, _| !entries.contains(id));
    all.extend(chosen);
    if let Err(e) = packs::save_polytone_choices(&packs::polytone_choices_file(), &all) {
        log::warn!("could not save the pack options: {e}");
    }
    packs::set_polytone_choices(all);
}
#[cfg(feature = "client")]
impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game::client_side) fn drop_pack_file(&mut self, path: &Path) {
        let size = self.gui_size();
        let handled = self.side.screen == super::Screen::Options
            && self.side.chat_ui.options.drop_resource_pack(
                path.to_path_buf(),
                &self.side.gui.atlas,
                size,
            );
        if !handled
            && self.title_input_active()
            && let Some(title) = &mut self.side.title.0
        {
            title.drop_file(path.to_path_buf());
        }
    }
    pub(in crate::game::client_side) fn resource_pack_infos(&mut self) -> Vec<PackInfo> {
        let additional = self.discover_mod_packs(mod_api::PackType::ClientResources);
        pack_infos(
            &self.vanilla_root(),
            &self.side.server_packs.0.active,
            &additional,
        )
    }
    /// Whether all active packs preserve vanilla's block models. The server
    /// overlay sits above the local selection and must be included too.
    pub(in crate::game::client_side) fn model_packs_are_vanilla(&self) -> bool {
        self.side.settings.sunrise.vanilla_models_with_server_packs(
            self.side.settings.resource_packs.iter().map(String::as_str),
            self.side
                .server_packs
                .0
                .active
                .iter()
                .map(|pack| pack.id.as_str()),
        )
    }
    /// The leaf mode that is safe for the current local and server packs.
    pub(in crate::game::client_side) fn effective_leaf_culling(
        &self,
    ) -> sunrise_client::graphics::LeafCullingMode {
        self.side
            .settings
            .sunrise
            .leaf_culling_effective_with_server_packs(
                self.side.settings.resource_packs.iter().map(String::as_str),
                self.side
                    .server_packs
                    .0
                    .active
                    .iter()
                    .map(|pack| pack.id.as_str()),
            )
    }
    /// The packs the pack screen offers and the current selection
    /// (`PackSelectionScreen` makes its `PackSelectionModel` from them).
    pub(in crate::game) fn refresh_pack_lists(&mut self) {
        let infos = self.resource_pack_infos();
        let configs = pack_configs(&self.vanilla_root());
        let browsing = self.side.settings.extras.pack_browser;
        let o = &mut self.side.chat_ui.options;
        o.pack_browser = browsing;
        o.pack_infos = infos;
        o.pack_configs = configs;
        o.pack_selection = self.side.settings.resource_packs.clone();
        o.pack_screen = None;
    }
    /// `Minecraft.reloadResourcePacks`: rebuild the overlay and reload every
    /// client resource from it, as vanilla's reload listeners do: block
    /// models and sprites (every section is meshed again,
    /// `LevelRenderer.allChanged`), the shared atlas (GUI, item, entity,
    /// particle and font textures), item models, sounds, languages, GUI
    /// metadata and the cloud and world border textures. The loading runs on
    /// a worker under the progress overlay (`LoadingOverlay` over
    /// `ReloadableResourceManager.createReload`'s background executor), so
    /// the window keeps drawing; `drive_pack_reload` applies it.
    /// Sunrise's pack browser: what its worker finished since the last
    /// frame. A pack that landed becomes an entry in the available list.
    pub(in crate::game::client_side) fn drive_pack_browser(&mut self) {
        let Some(screen) = &mut self.side.chat_ui.options.pack_screen else {
            return;
        };
        let Some(browser) = &mut screen.browser else {
            return;
        };
        let Some(message) = browser.poll() else {
            return;
        };
        if let pack_browser::Message::Results(_) = &message {
            // The rows draw their icons as they land.
            browser.fetch_icons();
        }
        if let pack_browser::Message::Icon(id, png) = &message {
            let key = pack_screen::icon_texture(id);
            if let Some(icon) = pack_screen::load_fetched_icon(png) {
                // Tracked, so closing the browser frees it again.
                screen.keep_browser_icon(key, icon);
            }
            return;
        }
        if let pack_browser::Message::Downloaded(name) = message {
            log::info!("Sunrise: added the resource pack {name}");
            // Rebuilding the screen would shut the list; keep it and only
            // refresh what the lists read.
            let selection = self.side.settings.resource_packs.clone();
            let infos = self.resource_pack_infos();
            self.side.chat_ui.options.pack_infos = infos;
            self.side.chat_ui.options.pack_configs = pack_configs(&self.vanilla_root());
            self.side.chat_ui.options.pack_selection = selection;
            self.side.chat_ui.options.pack_screen = None;
        }
    }
    pub(in crate::game) fn reload_packs(&mut self) {
        if let Err(e) = self.spawn_pack_reload(ReloadKind::Selection) {
            self.selection_reload_failed(&format!("could not start the pack reload: {e}"));
        }
    }
    /// NeoForge's `Minecraft.rollbackResourcePacks`: reset the selection and
    /// reload only if an optional pack was selected; otherwise show the error.
    pub(in crate::game::client_side) fn selection_reload_failed(&mut self, error: &str) {
        log::warn!("Sunrise: resource reload failed: {error}");
        let mut all = packs::discover(&self.vanilla_root(), &packs_dir());
        all.extend(self.discover_mod_packs(mod_api::PackType::ClientResources));
        if packs::has_optional_pack(&packs::enabled(&all, &self.side.settings.resource_packs)) {
            clear_selection(&mut self.side.settings);
            if self.spawn_pack_reload(ReloadKind::Selection).is_ok() {
                // addResourcePackLoadFailToast(null), SystemToast.addOrUpdate.
                let title =
                    sunrise_client_core::text::Text::translate("resourcePack.load_fail", &[]);
                self.add_or_update_system_toast(
                    super::toast_manager::SystemToastId::PackLoadFailure,
                    sunrise_client::toasts::SystemToast::new(title, None),
                );
                return;
            }
        }
        if self.side.screen != super::Screen::Options {
            self.open_options_at(
                self.side.title.0.is_some(),
                sunrise_client::options_ui::Page::Packs,
            );
        }
        self.side.chat_ui.options.resource_error = Some(error.to_string());
    }
    /// Stages the selection with the server's active packs on top on a
    /// worker (`createReload`'s background executor); the frame draws its
    /// progress meanwhile, and the overlay takes no input (`LoadingOverlay`),
    /// so what is held is released (`KeyMapping.releaseAll`).
    pub(in crate::game::client_side) fn spawn_pack_reload(
        &mut self,
        kind: ReloadKind,
    ) -> std::io::Result<()> {
        // A selection reload, including startup rollback, needs tail-owned mob metadata.
        // An in-flight reload finishes first (vanilla's `pendingReload`).
        if kind == ReloadKind::Selection
            && reload_waits(
                self.rest_loader.is_some(),
                self.side.pack_reload.0.is_some(),
            )
        {
            self.side.pending_pack_reload = true;
            return Ok(());
        }
        self.join_world_rest()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        self.clear_device_keys();
        self.side.physical_keys.clear();
        self.set_mapping_held("key.attack", false);
        self.set_mapping_held("key.use", false);
        let vanilla = self.vanilla_root();
        if self.ui.vanilla_root.as_os_str().is_empty() {
            self.ui.vanilla_root = vanilla.clone();
        }
        let enabled = self.applied_mods();
        // Only content-only packages are selectable on reload. Keep code packages' validated
        // startup manifests: changing their content must not install code or registries live.
        self.mods.selected = sunrise_mod_package::selection::content_packages(
            &crate::game::game_dir(),
            &enabled,
            &self.mods.dev,
        )
        .filter(|p| {
            !self
                .mods
                .native
                .built()
                .iter()
                .any(|built| built.id == p.package.manifest.id)
        })
        .collect();
        self.mods.refused = Default::default();
        let mod_sources = self.mods.clone();
        let (selected, server) = (
            self.side.settings.resource_packs.clone(),
            self.side.server_packs.0.active.clone(),
        );
        let (assets, res, context) = (
            self.pack.assets.clone(),
            self.res.clone(),
            self.side.client_pack.context.clone(),
        );
        let client_res_slot = self.side.client_res.clone();
        let (unicode, japanese) = (
            self.side.settings.force_unicode,
            self.side.settings.japanese_glyphs,
        );
        // The reload's mod listeners register here, on the thread that owns
        // the game, before the staging worker starts: each listener keeps its
        // registering mod for crash attribution.
        let client_listeners = self.register_client_reload_listeners();
        let texture_atlases = super::client_reload::collect_texture_atlases();
        super::mods::register_fluid_models(self.options.execution_role);
        let stage = move |progress: &crate::game::Progress| {
            let Some(client_res) = client_res_slot.get() else {
                return Err("client resources not loaded".to_owned());
            };
            stage_pack(
                &vanilla,
                &selected,
                &server,
                &mod_sources,
                client_listeners,
                &texture_atlases,
                &assets,
                &res,
                client_res,
                &context,
                unicode,
                japanese,
                progress,
            )
            .map_err(|e| e.to_string())
        };
        #[cfg(test)]
        let stage = mod_api::testing::inherit_mods(stage);
        self.side.pack_reload.0 = Some(PackReload::spawn(kind, stage)?);
        self.side.reload_overlay = Some(ReloadOverlay::default());
        Ok(())
    }
    /// One frame of the pack reload's `LoadingOverlay`: its bar eased toward
    /// the reload's progress (all of it once applied) and how far it has
    /// faded. `None`, and the overlay dropped, once it has faded out.
    pub(in crate::game::client_side) fn reload_overlay_frame(
        &mut self,
    ) -> Option<(f32, OverlayFade)> {
        let overlay = self.side.reload_overlay.as_mut()?;
        let now = Instant::now();
        let since = |at: Instant| now.duration_since(at).as_millis() as u64;
        let fade_in = since(*overlay.fade_in.get_or_insert(now));
        let Some(fade) = OverlayFade::at(fade_in, overlay.fade_out.map(since)) else {
            self.side.reload_overlay = None;
            return None;
        };
        let actual = self
            .side
            .pack_reload
            .0
            .as_ref()
            .map_or(1.0, |reload| reload.progress.fraction());
        overlay.bar = sunrise_client::level_loading::ease_overlay(overlay.bar, actual);
        Some((overlay.bar, fade))
    }
    /// `LoadingOverlay.tick` once the reload is done: the staged pack is
    /// applied on the render thread, and its outcome goes to whoever asked
    /// (`reloadResourcePacks`' callback, or `PackReloadConfig.Callbacks`).
    pub(in crate::game::client_side) fn drive_pack_reload(&mut self) {
        let Some((kind, staged)) = self
            .side
            .pack_reload
            .0
            .as_mut()
            .and_then(PackReload::finished)
        else {
            return;
        };
        self.side.pack_reload.0 = None;
        let result =
            staged.and_then(|staged| self.apply_staged_pack(staged).map_err(|e| e.to_string()));
        if let Some(overlay) = &mut self.side.reload_overlay {
            overlay.fade_out = Some(Instant::now());
        }
        match kind {
            ReloadKind::Selection => {
                self.mods_reloaded(result.is_ok());
                if let Err(e) = result {
                    self.selection_reload_failed(&e);
                }
            }
            ReloadKind::Server => {
                if let Err(e) = &result {
                    log::warn!("Sunrise: server resource pack reload failed: {e}");
                }
                self.finish_pack_reload(result.is_ok());
            }
        }
    }
    /// The render-thread half: the renderer on the new pack's textures, then
    /// the swap (`ReloadInstance`'s application listeners).
    pub(in crate::game) fn apply_staged_pack(
        &mut self,
        staged: StagedPack,
    ) -> sunrise_client_core::Result<()> {
        let StagedPack {
            root,
            client_applies,
            gui,
            pack,
            client_pack,
            textures,
            cem,
            block_entity_shapes,
            waypoint_styles,
            armor_stand_assets,
            frame_models,
            sounds,
            language,
        } = staged;
        // All fallible preflight finishes before any listener or the game
        // publishes the candidate. In particular, registration can refuse it.
        let loop_context = self.prepare_loop_context(&root)?;
        let professions = self.prepare_client_professions()?;
        let item_extensions = Self::prepare_client_item_extensions(&gui)?;
        // A reload from the title can apply before the start's world-rest worker is done.
        self.join_world_rest()?;
        let tab_modifiers = sunrise_client::polytone_creative_tabs::TabModifier::read_all(&root);
        let standalone_geometry =
            super::native_block_entity_draw::StandaloneGeometry::new(&pack, &client_pack, &gui);
        // Keep the working GPU resources, and hand its window surface over.
        if let Some(renderer) = &mut self.side.renderer.0 {
            renderer.release_surface();
        }
        let renderer = match self
            .side
            .window
            .0
            .clone()
            .map(|window| {
                build_renderer(
                    window,
                    self.options.offscreen,
                    &pack,
                    &client_pack,
                    &gui,
                    self.side.renderer.0.as_deref(),
                    Some(textures),
                    self.side.settings.sunrise.api,
                    self.side.settings.sunrise.translucent_batching,
                    self.side.settings.vsync,
                    &self.side.settings.sunrise,
                    super::shader_packs::picked_pack_shape(&self.side.settings),
                    self.side
                        .renderer
                        .0
                        .as_ref()
                        .map(|r| r.display_reading())
                        .unwrap_or_default(),
                    self.side.settings.texture_atlas_settings(),
                )
                .and_then(|(renderer, clouds)| {
                    clouds?;
                    Ok(Box::new(renderer))
                })
            })
            .transpose()
        {
            Ok(renderer) => renderer,
            Err(error) => {
                if let Some(renderer) = &mut self.side.renderer.0 {
                    renderer.recreate_surface().map_err(|restore| {
                        format!(
                            "{error}; could not restore the previous rendering surface: {restore}"
                        )
                    })?;
                }
                return Err(error);
            }
        };
        let skip = self
            .side
            .settings
            .extras
            .skip_modded_splashes_if_vanilla_modified;
        let gui = gui.with_splashes(&pack.source, &self.vanilla_root(), skip);
        let has_renderer = renderer.is_some();
        let upload = self.publish_client_atlases(&client_pack.render, gui, renderer);
        // With the atlas visible, admit the models at the model manager's point
        // in the listener order. All renderer and model validation is finished.
        self.apply_client_reload(client_applies, |game: &mut super::ClientGame<'_>| {
            // Refresh modifiers from the newly staged packs, including removals.
            game.side
                .creative_screen
                .0
                .set_pack_modifiers(tab_modifiers);
            game.refresh_creative_search_names();
            [
                game.side.frame_model,
                game.side.glow_frame_model,
                game.side.map_frame_model,
                game.side.glow_map_frame_model,
            ] = frame_models;
            game.side.waypoint_styles = waypoint_styles;
            game.side
                .armor_stand_assets
                .0
                .apply_resources(armor_stand_assets);
            game.side.armor_stand_assets.0.extensions = item_extensions;
            game.side_clear_random_tick_sections();
            game.portals.sections.clear();
            game.options.resources = root.clone();
            game.side.falling_vertices.get_mut().clear();
            game.pack = pack;
            game.side.client_pack = client_pack;
            // The joined level's environment for the frame has the packs' layers over its type.
            game.build_level_environments();
            game.side.standalone_geometry = standalone_geometry;
            game.side.instance_groups.borrow_mut().clear();
            // The new atlas's skin and map slots are blank: players and maps
            // upload into it again instead of naming sprites it does not have.
            game.side.skin_slots.forget();
            game.side
                .skin_downloads
                .0
                .borrow_mut()
                .set_resource_root(game.pack.source.clone());
            game.side.map_slots.forget();
            game.side.map_patches.clear();
            game.side.text_patches.get_mut().clear();
            // Kept mod block entities name the old atlas's sprites, and their view distances the old
            // registry's states.
            game.side.kept_block_entities.get_mut().clear();
            game.side.live_scenes.0.get_mut().clear();
            game.side.mod_view_distances.take();
            // The atlas's sprites moved: the particle engine re-reads every rect
            // and the packs' sprite sets, or every particle draws a stale region.
            if !game.is_logical_server() {
                game.side.particles.0.reload(
                    &game.pack.source,
                    &game.side.gui.atlas.sprites,
                    &game.side.gui.atlas.translucent,
                );
            }
            // Villager hats come from the pack's texture metadata (shared data, swapped in place).
            *game
                .client_res()
                .client_mob_data()
                .villager_hats
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                sunrise_client::mobs::villager_hats::VillagerHats::load(&game.pack.source);
            game.install_client_professions(&professions);
            *game
                .client_res()
                .client_mob_data()
                .cem
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = cem;
            game.client_res()
                .block_entity_render()
                .set_shapes(block_entity_shapes);
            // The random entity textures and emissive overlays the new atlas packs. A new scan is
            // a new generation: every mob chooses again on its next tick.
            *game
                .client_res()
                .client_mob_data()
                .random_textures
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                std::mem::take(&mut game.side.gui.atlas.random_textures);
            *game
                .client_res()
                .client_mob_data()
                .emissive
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                game.side.gui.atlas.emissive.clone();
            *game
                .client_res()
                .client_mob_data()
                .blinks
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                std::mem::take(&mut game.side.gui.atlas.blinks);
            // Read with the language in use when the reload started: one picked since then wins.
            let code = lang::code_snapshot();
            lang::set(if language.code == code {
                language
            } else {
                lang::Language::load(&[&game.pack.source], &code)
            });
            // Book search terms and resolved result icons belong to these resources.
            game.side.recipe_ui.0.reset_screen();
            game.refresh_remote_language();
            game.side.audio.0.reload_with(&game.pack.source, sounds);
            game.clear_lod();
            // A pack that may replace leaf models falls back to `Off` before the
            // reload marks every section dirty.
            game.side.streaming.leaf_culling = game.effective_leaf_culling();
            let context = game.worker_context();
            game.side
                .streaming
                .reload_resources(&mut game.world, context);
            game.side.pending_uploads.0.clear();
            game.install_model_upload(upload);
            if has_renderer {
                game.apply_ui_settings();
            }
            // Sunrise's own shader pack is built again: its sprite tags name the old atlas's
            // layers, and a new renderer runs no pack.
            game.side.native_shaders = super::native_shaders::NativeShaders::picking(
                &std::mem::take(&mut game.side.native_shaders.picked),
            );
            game.side.chat_ui.chat.rescale(&game.side.gui.atlas);
            game.install_loop_context(loop_context);
            game.resolve_entity_renderers();
            game.native_model_baking_completed();
        });
        self.native_client_resources_reloaded();
        Ok(())
    }
}

/// The CPU half of a pack reload (`PackReload` runs it on a worker;
/// `apply_staged_pack` does the GPU half and the swap): the overlay
/// root, the models and sprites, the menu bundle, the pack's tables and its
/// CPU mip chains.
#[cfg(feature = "client")]
pub(in crate::game) fn stage_pack(
    vanilla_root: &Path,
    selected: &[String],
    server: &[Pack],
    mod_sources: &super::mods::ModsState,
    client_listeners: mod_api::ClientListenerRegistry,
    texture_atlases: &[mod_api::TextureAtlasRegistration],
    assets: &Arc<Assets>,
    res: &Resources,
    client_res: &ClientResources,
    context: &WorkerContext,
    force_unicode: bool,
    japanese: bool,
    progress: &super::Progress,
) -> sunrise_client_core::Result<StagedPack> {
    let resource_root = startup_root(vanilla_root, selected, server, mod_sources)?.selected?;
    let resource_root = super::client_reload::stage_texture_atlases(resource_root, texture_atlases);
    let root = resource_root.prepared().to_path_buf();
    let source = resource_root.source().clone();
    progress.set(1);
    let (staged, client_applies) = super::client_reload::stage_client_reload(
        &source,
        client_listeners,
        || -> sunrise_client_core::Result<_> {
            // The world's models and sprites and the GUI atlas read the same root and nothing of
            // each other: about a second each, side by side.
            let pool = super::resource_tables::dynamic_texture_pool();
            // The sound events and the language are read here too, not in
            // the frame that applies the reload (about 20 ms of it).
            let (render, atlas, panorama, (sounds, language)) = std::thread::scope(|s| {
                let atlas = super::resource_load::named_scope(s, "pack-assets", || {
                    let game = sunrise_assets::resource_source::ResourceSource::directory(
                        vanilla_root.to_path_buf(),
                    );
                    HudAtlas::build_over_with_dynamic_textures(&source, &source, &game, pool)
                        .map_err(|e| e.to_string())
                });
                let panorama = super::resource_load::named_scope(s, "pack-assets", || {
                    sunrise_client::gfx::panorama::Faces::load(&source, &source)
                });
                let small = super::resource_load::named_scope(s, "pack-assets", || {
                    (
                        sunrise_level::sound_events::SoundRegistry::load(&source),
                        lang::Language::load(&[&source], &lang::code()),
                    )
                });
                (
                    sunrise_client::assets::RenderAssets::load(&resource_root, assets),
                    atlas.join(),
                    panorama.join(),
                    small
                        .join()
                        .unwrap_or_else(|p| std::panic::resume_unwind(p)),
                )
            });
            let render = Arc::new(render?);
            let mut atlas = atlas.unwrap_or_else(|p| std::panic::resume_unwind(p))?;
            atlas.font.uniform = force_unicode;
            atlas.font.japanese = japanese;
            let gui = UiResources {
                atlas,
                panorama: Arc::new(panorama.unwrap_or_else(|p| std::panic::resume_unwind(p))),
                // `Game::apply_staged_pack` fills it with the client's setting.
                splashes: Arc::from(""),
                plain_texts: Default::default(),
            };
            progress.set(2);
            let (mut pack, client_pack) = PackResources::load(
                &resource_root,
                assets.clone(),
                render,
                res.facts(),
                context.chunk.biomes.clone(),
                context.chunk.light.clone(),
                &res.core.item_data,
            )?;
            pack.additional_data = super::pack_finders::data_packs(mod_sources).into();
            let textures = prepare_renderer_textures(&client_pack);
            let cem = Arc::new(sunrise_client::mobs::cem::CemModels::load(
                &source,
                client_res.client_mob_models(),
            ));
            let block_entity_shapes = client_res.block_entity_render().shapes_for(&source);
            let waypoint_styles = sunrise_client::locator_bar::WaypointStyles::load(&source);
            let armor_stand_assets = super::armor_stands::ArmorStandAssets::load(&source, None);
            let frame_models =
                super::frames::bake_frames(client_pack.render.model_library.as_deref().ok());
            progress.set(3);
            Ok(move |client_applies| StagedPack {
                root,
                client_applies,
                gui,
                pack,
                client_pack,
                textures,
                cem,
                block_entity_shapes,
                waypoint_styles,
                armor_stand_assets,
                frame_models,
                sounds,
                language,
            })
        },
    )
    .map_err(|error| {
        error
            .downcast::<mod_api::__private::reload::ReloadStageError>()
            .map_or_else(|error| error.to_string(), |error| error.diagnostic())
    })?;
    Ok(staged(client_applies))
}

/// Everything a pack reload builds on the CPU (`apply_staged_pack` finishes
/// it on the render thread).
#[cfg(feature = "client")]
pub(in crate::game) struct StagedPack {
    pub root: PathBuf,
    pub waypoint_styles: sunrise_client::locator_bar::WaypointStyles,
    pub armor_stand_assets: super::armor_stands::ArmorStandAssets,
    pub frame_models: [super::frames::FrameModel; 4],
    /// The mod reload listeners' prepared applies, ordered around publication
    /// of the staged models.
    pub client_applies: Vec<super::client_reload::PreparedClientReload>,
    pub gui: UiResources,
    pub pack: PackResources,
    pub(in crate::game) client_pack: ClientPackResources,
    pub textures: PreparedTextures,
    /// The packs' custom entity models (OptiFine CEM).
    pub cem: Arc<sunrise_client::mobs::cem::CemModels>,
    /// The block entities' models, the packs' custom entity models over vanilla's.
    pub block_entity_shapes: Arc<sunrise_client::block_entity_render::BlockEntityShapes>,
    /// The root's sound events and current language.
    pub sounds: sunrise_level::sound_events::SoundRegistry,
    pub language: lang::Language,
}

/// A pack reload's stages, indexed by `progress`: the overlay draws how far
/// through them it is (`ReloadInstance.getActualProgress`), not their names.
pub(in crate::game) const RELOAD_LINES: [&str; 4] = [
    "Preparing resources",
    "Loading assets",
    "Loading pack tables",
    "Applying resources",
];
/// Whether a reload asked for now waits: for the world's resources, or for
/// the reload already under its overlay (`Minecraft.reloadResourcePacks`
/// hands back `pendingReload` while a `LoadingOverlay` is up).
pub(in crate::game::client_side) fn reload_waits(world_loading: bool, in_flight: bool) -> bool {
    world_loading || in_flight
}
/// Who asked for a pack reload: its outcome goes back there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum ReloadKind {
    /// The player's own selection (`Minecraft.reloadResourcePacks`).
    Selection,
    /// A server's packs (`DownloadedPackSource.startReload`).
    Server,
}
#[cfg(feature = "client")]
pub(in crate::game) type StageResult = std::result::Result<StagedPack, String>;
/// A pack reload whose CPU half runs on a worker while `progress` feeds the
/// loading overlay; `Game::drive_pack_reload` applies it on the render thread.
#[cfg(feature = "client")]
pub(in crate::game) struct PackReload {
    pub(in crate::game) progress: super::Progress,
    pub(in crate::game) kind: ReloadKind,
    pub(in crate::game) handle: Option<std::thread::JoinHandle<StageResult>>,
    /// When the overlay went up. `LoadingOverlay.fadeInStart`, set on the
    /// first frame it draws.
    pub(in crate::game) started: Instant,
    /// How long the overlay stays up before the reload may be applied.
    pub(in crate::game) hold: Duration,
}
/// `LoadingOverlay` over a pack reload, from its first frame until it has
/// faded out (`Minecraft.setOverlay(null)`), two seconds after the reload.
#[derive(Default)]
#[cfg(feature = "client")]
pub(in crate::game) struct ReloadOverlay {
    /// `fadeInStart`: the overlay's first frame.
    pub(in crate::game) fade_in: Option<Instant>,
    /// `fadeOutStart`: when the reload was applied.
    pub(in crate::game) fade_out: Option<Instant>,
    /// `currentProgress`: the bar as drawn.
    pub(in crate::game) bar: f32,
}
/// `LoadingOverlay.isReadyToFadeOut`: a reload started from inside the game
/// (`fadeIn`) is not ready while `Util.getMillis() - fadeInStart < 1000L`, so
/// a pack that stages in a few frames still shows its overlay for a second
/// instead of flashing.
pub(in crate::game) const OVERLAY_HOLD: Duration = Duration::from_millis(1000);
#[cfg(feature = "client")]
impl PackReload {
    /// Starts `stage` on its own thread and returns at once.
    pub(in crate::game::client_side) fn spawn(
        kind: ReloadKind,
        stage: impl FnOnce(&super::Progress) -> StageResult + Send + 'static,
    ) -> std::io::Result<Self> {
        Self::spawn_holding(kind, OVERLAY_HOLD, stage)
    }
    /// `spawn` with the overlay's hold spelled out, so a test need not wait it.
    pub(in crate::game::client_side) fn spawn_holding(
        kind: ReloadKind,
        hold: Duration,
        stage: impl FnOnce(&super::Progress) -> StageResult + Send + 'static,
    ) -> std::io::Result<Self> {
        let progress = super::Progress::with(&RELOAD_LINES);
        let worker = progress.clone();
        let handle = std::thread::Builder::new()
            .stack_size(sunrise_threads::GAME_THREAD_STACK_BYTES)
            .name("pack-reload".into())
            .spawn(move || stage(&worker))?;
        Ok(Self {
            progress,
            kind,
            handle: Some(handle),
            started: Instant::now(),
            hold,
        })
    }
    pub(in crate::game::client_side) fn kind(&self) -> ReloadKind {
        self.kind
    }
    /// The staged pack once the worker is done and the overlay has had its
    /// second (taken once); none while either is outstanding.
    pub(in crate::game::client_side) fn finished(&mut self) -> Option<(ReloadKind, StageResult)> {
        if self.started.elapsed() < self.hold || !self.handle.as_ref()?.is_finished() {
            return None;
        }
        let staged = self
            .handle
            .take()?
            .join()
            .unwrap_or_else(|_| Err("the pack reload worker panicked".into()));
        Some((self.kind, staged))
    }
}

/// Reads one converter's directory from `packs`, later packs winning, and
/// collects the converted files: the shared read-and-log loop of both sides'
/// reloads. `top` is `"data"` or `"assets"`.
pub(in crate::game) fn reload_files(
    top: &str,
    packs: &[&DataPack],
    listener: &Identifier,
    converter: &FileToIdConverter,
) -> ReloadFiles {
    let mut selected = std::collections::BTreeMap::new();
    for pack in packs {
        for file in pack.list_under(top, &[converter.directory()]) {
            if let Some(id) = file
                .strip_prefix(&format!("{top}/"))
                .and_then(|path| converter.file_to_id(path))
            {
                selected.insert(id, (*pack, file));
            }
        }
    }
    let mut files = std::collections::BTreeMap::new();
    for (id, (pack, path)) in selected {
        // A file that vanishes or fails to read between listing and reading is one bad
        // entry, as vanilla's `scanDirectory` treats a parse failure: log it, skip it.
        match pack.read(&path) {
            Some(bytes) => {
                files.insert(id, bytes);
            }
            None => log::error!(
                "Couldn't read file '{id}' from '{}/{}' for listener {}",
                pack.id,
                path,
                listener
            ),
        }
    }
    ReloadFiles::new(files)
}

#[cfg(feature = "client")]
#[cfg(test)]
pub(in crate::game) mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};

    #[test]
    #[ignore = "daily"]
    fn fluid_models_reload_recollects_before_baking() {
        // Concurrent resource loads replace the process-wide fluid looks and models.
        if super::super::native_entity_refs::tests::run_fresh_cases(
            "game::client_side::packs::tests::fluid_models_reload_recollects_before_baking",
            "SUNRISE_FLUID_RELOAD_CHILD",
            &["recollect"],
        ) {
            return;
        }
        use mod_api::{FluidModel, Hooks, ModEntry, RegisterFluidModelsEvent, RegistrationContext};
        use mod_platform::testing::with_mods;
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        #[derive(Default)]
        struct Models(Arc<AtomicUsize>);
        #[mod_api::sunrise_mod]
        impl Hooks for Models {
            fn register_fluid_models(
                &self,
                _: &RegistrationContext,
                event: &mut RegisterFluidModelsEvent<'_>,
            ) {
                if self.0.load(Ordering::SeqCst) == 2 {
                    return;
                }
                let mut model = FluidModel::default();
                model.fluid = "reload_boundary:oil".into();
                model.still = Some(format!(
                    "reload_boundary:block/oil_{}",
                    self.0.load(Ordering::SeqCst)
                ));
                event.register(model);
            }
        }
        let dir = std::env::temp_dir().join(format!("sunrise-fluid-reload-{}", std::process::id()));
        let fluids = dir.join("data/reload_boundary/sunrise/fluid");
        std::fs::create_dir_all(&fluids).unwrap();
        std::fs::write(dir.join("sunrise-mod.toml"),
                "id = \"reload_boundary\"\nname = \"Reload boundary\"\nversion = \"1\"\napi = \"0.2\"\ncapabilities = [\"registries\"]\n").unwrap();
        std::fs::write(fluids.join("oil.json"), br#"{"world":{}}"#).unwrap();
        let vanilla = PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let mut sources = super::super::mods::ModsState {
            dev: vec![dir.clone()],
            ..Default::default()
        };
        let enabled = ["reload_boundary".to_owned()].into();
        sources.select(&std::env::current_dir().unwrap(), &enabled);
        let generation = Arc::new(AtomicUsize::new(0));
        with_mods(
            vec![ModEntry::new(
                "reload_boundary",
                Box::new(Models(generation.clone())),
            )]
            .into(),
            || {
                let root = super::super::mods::content_root(&vanilla, &sources).unwrap();
                super::super::native_entity_refs::tests::with_game_role_client(
                    root.prepared().to_path_buf(),
                    super::super::execution_role::ExecutionRole::Client,
                    |game| {
                        game.ui.vanilla_root = vanilla;
                        game.mods = sources;
                        game.side.settings.mods.enabled = enabled;
                        // Drive the real entry point and join its CPU staging worker, without
                        // applying the staged textures (which would need a window/GPU).
                        let mut reload = |expected: Option<&str>| {
                            game.spawn_pack_reload(ReloadKind::Selection).unwrap();
                            let mut pending = game.side.pack_reload.0.take().unwrap();
                            let staged = pending.handle.take().unwrap().join().unwrap().unwrap();
                            let layers = &staged.client_pack.render.sprite_layers;
                            for id in ["reload_boundary:block/oil_0", "reload_boundary:block/oil_1"]
                            {
                                assert_eq!(layers.contains_key(id), expected == Some(id), "{id}");
                            }
                        };
                        mod_platform::__private::with_test_packages(&["reload_boundary"], || {
                            reload(Some("reload_boundary:block/oil_0"));
                            generation.store(1, Ordering::SeqCst);
                            reload(Some("reload_boundary:block/oil_1"));
                            // A provider that stops contributing must clear its old models.
                            generation.store(2, Ordering::SeqCst);
                            reload(None);
                        });
                        sunrise_client::fluid_models::register(vec![]);
                        sunrise_client::fluid_models::set_world_looks([]);
                        std::fs::remove_dir_all(dir).unwrap();
                    },
                );
            },
        );
    }

    fn wait(reload: &mut PackReload) -> (ReloadKind, StageResult) {
        for _ in 0..500 {
            if let Some(done) = reload.finished() {
                return done;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("the reload worker never finished");
    }

    #[test]
    #[ignore = "daily"]
    fn resource_pack_overrides_mod_and_vanilla() {
        use std::fs;
        let vanilla =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.local/sunrise-client/26.3");
        assert!(
            vanilla.join("facts.json").is_file(),
            "prepared vanilla data required"
        );
        let dir =
            std::env::temp_dir().join(format!("sunrise-pack-mod-order-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mod_dir = dir.join("mod");
        let pack_dir = dir.join("packs/override");
        let asset = "assets/order_test/textures/block/example.png";
        for root in [&mod_dir, &pack_dir] {
            fs::create_dir_all(root.join(asset).parent().unwrap()).unwrap();
        }
        fs::write(
            mod_dir.join("sunrise-mod.toml"),
            "id = \"order_test\"\nname = \"Order test\"\nversion = \"1\"\napi = \"0.2\"\ncapabilities = [\"registries\"]\n",
        )
        .unwrap();
        fs::write(mod_dir.join(asset), b"mod").unwrap();
        fs::write(pack_dir.join(asset), b"pack").unwrap();
        fs::write(
            pack_dir.join("pack.mcmeta"),
            br#"{"pack":{"description":"test","pack_format":97}}"#,
        )
        .unwrap();
        {
            use std::io::Write;
            let mut archive =
                zip::ZipWriter::new(fs::File::create(dir.join("packs/override.zip")).unwrap());
            for name in ["pack.mcmeta", asset] {
                archive
                    .start_file(name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                archive
                    .write_all(&fs::read(pack_dir.join(name)).unwrap())
                    .unwrap();
            }
            archive.finish().unwrap();
            fs::remove_dir_all(&pack_dir).unwrap();
        }
        let mut sources = super::super::mods::ModsState {
            dev: vec![mod_dir],
            ..Default::default()
        };
        sources.select(&std::env::current_dir().unwrap(), &Default::default());
        let without_pack = startup_root(&vanilla, &[], &[], &sources)
            .unwrap()
            .selected
            .unwrap();
        assert_eq!(without_pack.source().read(asset).unwrap(), b"mod");
        let pack = packs::folder(&dir.join("packs"), packs::RESOURCE_FORMAT);
        assert_eq!(pack.len(), 1);
        let with_pack = startup_root(&vanilla, &[], &pack, &sources)
            .unwrap()
            .selected
            .unwrap();
        assert_eq!(with_pack.source().read(asset).unwrap(), b"pack");
        // The selected archive changes resource lookup without materializing
        // its override beside generated tables.
        assert_eq!(with_pack.prepared(), without_pack.prepared());
        assert_ne!(
            fs::read(with_pack.prepared().join(asset)).ok().as_deref(),
            Some(b"pack".as_slice())
        );
        assert!(!pack_dir.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_reload_stages_on_a_worker_while_the_caller_goes_on() {
        // The stage blocks until the caller lets it go: `spawn` returning at
        // all means the window thread is free to draw the overlay.
        let (release, blocked) = mpsc::channel::<()>();
        let (thread, ran_on) = mpsc::channel();
        let mut reload = PackReload::spawn(ReloadKind::Selection, move |progress| {
            blocked.recv().unwrap();
            progress.set(2);
            thread
                .send(std::thread::current().name().map(str::to_owned))
                .unwrap();
            Err("staged".into())
        })
        .unwrap();
        assert!(reload.finished().is_none(), "still staging");
        assert_eq!(reload.progress.line(), "Preparing resources");
        release.send(()).unwrap();
        let (kind, staged) = wait(&mut reload);
        assert_eq!(kind, ReloadKind::Selection);
        assert_eq!(staged.err().as_deref(), Some("staged"));
        assert_eq!(reload.progress.line(), "Loading pack tables");
        assert_eq!(ran_on.recv().unwrap().as_deref(), Some("pack-reload"));
        assert!(reload.finished().is_none(), "the outcome is taken once");
    }

    #[test]
    fn a_panicking_stage_fails_the_reload() {
        let mut reload = PackReload::spawn(ReloadKind::Server, |_| panic!("bad pack")).unwrap();
        let (kind, staged) = wait(&mut reload);
        assert_eq!(kind, ReloadKind::Server);
        assert_eq!(
            staged.err().as_deref(),
            Some("the pack reload worker panicked")
        );
    }

    /// `LoadingOverlay.isReadyToFadeOut`: a reload that stages at once still
    /// keeps its overlay up - the 1000 ms `fadeInStart` gate - so a small pack
    /// does not flash the Mojang logo for a frame or two.
    #[test]
    fn a_fast_reload_keeps_the_overlay_up_before_it_applies() {
        assert_eq!(OVERLAY_HOLD, Duration::from_millis(1000));
        let mut quick = PackReload::spawn_holding(ReloadKind::Selection, Duration::ZERO, |_| {
            Err("staged".into())
        })
        .unwrap();
        assert_eq!(wait(&mut quick).0, ReloadKind::Selection);
        // A hold no test machine outlasts, however loaded: the worker is done long
        // before the overlay has had its time.
        let hold = Duration::from_secs(3600);
        let mut held =
            PackReload::spawn_holding(ReloadKind::Selection, hold, |_| Err("staged".into()))
                .unwrap();
        for _ in 0..500 {
            if held.handle.as_ref().is_some_and(|h| h.is_finished()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(held.handle.as_ref().is_some_and(|h| h.is_finished()));
        assert!(held.finished().is_none(), "the overlay is still up");
        // Once the hold has passed, the staged pack is handed over.
        held.hold = Duration::ZERO;
        assert_eq!(wait(&mut held).0, ReloadKind::Selection);
    }

    #[test]
    fn a_reload_waits_for_world_resources_and_the_one_in_flight() {
        assert!(!reload_waits(false, false));
        assert!(reload_waits(true, false));
        assert!(reload_waits(false, true));
        assert!(reload_waits(true, true));
    }
}

#[cfg(feature = "client")]
#[cfg(test)]
#[path = "clouds_reload_tests.rs"]
pub(in crate::game) mod clouds_reload_tests;
