#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{
    Game, PackResources, Resources, UiResources, build_renderer, prepare_renderer_textures,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::packs::OVERLAY_HOLD;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::packs::PackReload;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::packs::PackRoots;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::packs::RELOAD_LINES;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::packs::ReloadKind;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::packs::ReloadOverlay;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::packs::StageResult;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::packs::StagedPack;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::packs::cache_dir;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::packs::clear_selection;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::packs::layer_client_packs;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::packs::open_uri;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::packs::pack_infos;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::packs::reload_files;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::packs::stage_pack;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::packs::startup_root;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::packs::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::__private::game::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
    __private::reload::ReloadFiles, FileToIdConverter, Identifier,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_assets::resource_source::ResourceRoot;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    assets::Assets,
    datapack::DataPack,
    gfx::PreparedTextures,
    hud::HudAtlas,
    lang,
    level_loading::OverlayFade,
    pack_browser,
    pack_screen::{self, Compat, PackInfo, Source as PackSource},
    packs::{self, Pack, Position},
    streaming::WorkerContext,
    text::{Style, Text},
};

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn apply_staged_pack_server(&mut self) {
        self.random_tick_sections.0.clear();
    }
}
