use mod_api::game::{PhantomSpawnResult, SleepProblem};
use mod_api::{
    ActiveEffect, BlockClick, BlockEntityRef, BlockFace, ChunkRef, ClockAdjustment, EffectKey,
    EntityRef, ExplosionStart, GameMode, GameRuleActor, GameTests, Identifier, ItemEntityRef,
    ItemFished, LevelRef, LifecycleContext, LivingRef, LootDrops, PathComputationType,
    PermissionLevel, Placement, PlayerRef, ServerTickPhase, TicketController, TriState, WorldPhase,
};
use sunrise_core::{
    dimension::Dimension as LevelDimension,
    geometry::Direction,
    nbt::{Compound, Tag},
    position::BlockPos,
};
use sunrise_game_data::{gamerules::Value as GameRuleValue, suggestion::Suggestion, text::Text};
use sunrise_gameplay_common::game::{
    item_snapshot::ItemSnapshotGameExt1 as _, native_configs::ConfigFile,
    native_data_maps_common::DataMapType,
};
use sunrise_items::items::ItemStack;
use sunrise_level::{biome::SpawnEntry, particle_options::ParticleOptions};
use sunrise_sim::{first_person_sim::Hand, health::HurtSource};
use sunrise_world::registry::BlockStateId;

include!(concat!(env!("OUT_DIR"), "/mod_api_fire_generated.rs"));
