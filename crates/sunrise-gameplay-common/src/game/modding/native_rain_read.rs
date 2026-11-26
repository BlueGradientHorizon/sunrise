use crate::game::{Dim, mod_api_host_common::weather_for_level};
use mod_api::{LevelDimensionShape, LevelDimensionType, LevelRef};
use sunrise_core::{
    dimension::Dimension as LevelDimension, position::BlockPos, random::LegacyRandom,
};
use sunrise_level::world::World;
use sunrise_world::{level::LevelAccess, registry::BlockStateId};

pub fn weather_for_world(world: &World, is_end: bool) -> (f32, f32) {
    weather_for_level(world.shape, is_end, world.weather.levels(1.0))
}

/// `Level.isRaining` and `isThundering` from the rain and thunder levels.
pub fn weather_flags((rain, thunder): (f32, f32)) -> (bool, bool) {
    (rain > 0.2, thunder > 0.9)
}

/// `Level.isRainingAt` over `world`, the column of the level in dimension `dim`.
pub fn rain_at(
    world: &World,
    facts: &sunrise_items::blocks::BlockFacts,
    biomes: &sunrise_level::biome::Biomes,
    behaviours: &sunrise_world::level::Behaviours,
    dim: Dim,
    pos: BlockPos,
) -> bool {
    let view = RainView {
        world,
        facts,
        biomes,
        dimension: dim.level(),
        raining: weather_flags(weather_for_world(world, dim == Dim::End)).0,
        random: LegacyRandom::new(0),
    };
    behaviours.is_raining_at(&view, pos)
}

/// A read-only view of one level's column for `Level.isRainingAt`, so a level parked
/// while the server ticks another answers like the one in place.
struct RainView<'a> {
    world: &'a World,
    facts: &'a sunrise_items::blocks::BlockFacts,
    biomes: &'a sunrise_level::biome::Biomes,
    dimension: LevelDimension,
    raining: bool,
    /// `LevelAccess::random` asks for a generator; the read draws none.
    random: LegacyRandom,
}

impl sunrise_world::level::BlockEntityNeighbors for RainView<'_> {}

impl LevelAccess for RainView<'_> {
    fn block_state(&self, pos: BlockPos) -> BlockStateId {
        self.world.block(pos.x, pos.y, pos.z)
    }
    fn set_raw(&mut self, _pos: BlockPos, _state: BlockStateId) -> Option<BlockStateId> {
        None
    }
    fn is_loaded(&self, pos: BlockPos) -> bool {
        self.world.loaded(pos.x, pos.z)
    }
    fn random(&mut self) -> &mut LegacyRandom {
        &mut self.random
    }
    fn game_time(&self) -> i64 {
        self.world.game_time
    }
    fn min_y(&self) -> i32 {
        self.world.shape.min_y
    }
    fn max_y(&self) -> i32 {
        self.world.shape.max_y()
    }
    fn sky_light(&self, pos: BlockPos) -> i32 {
        i32::from(self.world.light(pos.x, pos.y, pos.z) >> 4)
    }
    fn world_surface(&self, x: i32, z: i32) -> i32 {
        self.world.world_surface(x, z, |s| self.facts.is_air(s))
    }
    fn biome_name(&self, pos: BlockPos) -> Option<&str> {
        let chunk = self.world.chunks.get(&(pos.x >> 4, pos.z >> 4))?;
        Some(&self.biomes.get(chunk.biome(pos.x, pos.y, pos.z)).name)
    }
    fn is_raining(&self) -> bool {
        self.raining
    }
    fn dimension(&self) -> LevelDimension {
        self.dimension
    }
}

pub trait NativeRainReadGameExt1<'__sg>: crate::game::SideGame<'__sg> {
    /// `Level.getSkyDarken` for an active or parked level, without switching dimensions.
     fn level_get_sky_darken(&self, level: LevelRef) -> Option<i32> {
        let world = self.native_level_read_world(level)?;
        if level.dimension() == self.native_level_key() {
            return Some(self.sky_darken());
        }
        Some(self.sky_darken_in(world, Dim::from_id(level.dimension_id())))
    }

    /// Dimension shape of a loaded server level, including parked levels.
     fn level_dimension_shape(
        &self,
        level: LevelRef,
    ) -> Option<LevelDimensionShape> {
        let shape = self.native_level_world(level)?.shape;
        Some(LevelDimensionShape::new(
            shape.min_y,
            shape.height,
            shape.logical_height,
            shape.sky_light,
            shape.has_ceiling,
        ))
    }

    /// The level's dimension type with its gameplay attributes, from the facts the game uses.
     fn level_dimension_type(
        &self,
        level: LevelRef,
    ) -> Option<LevelDimensionType> {
        use sunrise_world::dimension_type::{CardinalLight, Skybox};
        use sunrise_world::worldgen::value_provider::IntProvider;
        self.native_level_read_world(level)?;
        let key = sunrise_core::LevelKey::parse(level.dimension_id()).ok()?;
        let facts = self.dimension_facts_for(key);
        Some(LevelDimensionType {
            id: facts.id.clone(),
            min_y: facts.height.min_y,
            height: facts.height.height,
            logical_height: facts.height.logical_height,
            has_skylight: facts.height.has_skylight,
            has_ceiling: facts.height.has_ceiling,
            has_fixed_time: facts.has_fixed_time,
            has_ender_dragon_fight: facts.has_ender_dragon_fight,
            coordinate_scale: facts.coordinate_scale,
            ambient_light: facts.ambient_light,
            infiniburn: facts.infiniburn.clone(),
            monster_spawn_light_level: match facts.monster_spawn_light_level {
                IntProvider::Constant(n) => Some((n, n)),
                IntProvider::Uniform { min, max } => Some((min, max)),
                _ => None,
            },
            monster_spawn_block_light_limit: facts.monster_spawn_block_light_limit,
            skybox: match facts.skybox {
                Skybox::None => "none".to_owned(),
                Skybox::Overworld => "overworld".to_owned(),
                Skybox::End => "end".to_owned(),
            },
            cardinal_light: match facts.cardinal_light {
                CardinalLight::Default => "default".to_owned(),
                CardinalLight::Nether => "nether".to_owned(),
            },
            water_evaporates: facts.water_evaporates,
            fast_lava: facts.fast_lava,
            respawn_anchor_works: facts.respawn_anchor_works,
            nether_portal_spawns_piglin: facts.nether_portal_spawns_piglin,
            piglins_zombify: facts.piglins_zombify,
            snow_golem_melts: facts.snow_golem_melts,
            can_start_raid: facts.can_start_raid,
            sky_light_level: facts.sky_light_level,
            timelines: facts.timelines.clone(),
            default_clock: facts.default_clock.clone(),
            bed_can_sleep: facts.bed_rule.can_sleep.name().to_owned(),
            bed_can_set_spawn: facts.bed_rule.can_set_spawn.name().to_owned(),
            bed_destroy_on_use: facts.bed_rule.destroy_on_use,
            bed_destroy_on_leave: facts.bed_rule.destroy_on_leave,
            straw_bed_can_sleep: facts.straw_bed_rule.can_sleep.name().to_owned(),
            straw_bed_can_set_spawn: facts.straw_bed_rule.can_set_spawn.name().to_owned(),
            straw_bed_destroy_on_use: facts.straw_bed_rule.destroy_on_use,
            straw_bed_destroy_on_leave: facts.straw_bed_rule.destroy_on_leave,
        })
    }

    /// The rain and thunder levels, each 0.0 to 1.0 (`Level.getRainLevel`,
    /// `getThunderLevel` at partial tick 1.0): both 0 in a level with no weather.
    /// Reads any open server level or the client's current synchronized level;
    /// None for a stale handle, an unavailable level or a client not logged in.
     fn level_weather(&self, level: LevelRef) -> Option<(f32, f32)> {
        let world = self.native_level_read_world(level)?;
        Some(weather_for_world(
            world,
            level.dimension() == Dim::End.key(),
        ))
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> NativeRainReadGameExt1<'__sg> for G {}


use crate::game::weather::WeatherGameExt as _;

use crate::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use crate::game::native_level_refs_common::NativeLevelRefsCommonGameExt1 as _;
