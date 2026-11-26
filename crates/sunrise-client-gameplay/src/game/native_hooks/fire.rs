use crate::game::native_mob_draw::{MobRender, PlayerRender};
use mod_api::game::Shake;
use mod_api::{
    ActiveEffect, Answered, BlockEntityDataSource, BlockRim, ClientMiningPrediction, ClientPacket,
    ClientReloadListeners, ColoredLight, EntityLayers, EntityRef, FrameModel, FrameModelEdit,
    GuiLayerRegistrar, HandDrawContext, HandRender, Identifier, InteractionAttempt, InteractionKey,
    LevelRef, ModBlockEntity, PanelEvent, RandomSource, ScreenOpening, ScreenView, ScreenWidgets,
    ScreenshotImage, TooltipElement, TriState,
};
use std::time::Duration;
use sunrise_client::{
    extras_hud::Corners,
    music::{Music, MusicLayer},
};
use sunrise_core::{geometry::Direction, position::BlockPos};
use sunrise_game_data::text::Text;
use sunrise_items::items::ItemStack;
use sunrise_level::biome::BiomeColors;
use sunrise_sim::first_person_sim::Hand;
use sunrise_world::registry::BlockStateId;

include!(concat!(env!("OUT_DIR"), "/mod_api_fire_generated.rs"));
