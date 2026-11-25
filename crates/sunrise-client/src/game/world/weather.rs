//! Weather and sky light for the player's level: whether it can have weather, the rain and thunder
//! levels, where it rains or snows (`Level.isRainingAt`, the motion-blocking height), the weather's
//! effects each tick and `Level.getSkyDarken` with `isDarkOutside`.
#[allow(unused_imports)]
use super::*;

/// `ClientLevel.getPrecipitationAt`: a loaded column's biome and the level's sea level
/// decide whether it snows, rains, or has no precipitation.
pub(in crate::game) fn precipitation_at(
    world: &World,
    biomes: &Biomes,
    zoom: BiomeZoom,
    pos: BlockPos,
    sea_level: i32,
) -> Option<bool> {
    if !world.chunks.contains_key(&(pos.x >> 4, pos.z >> 4)) {
        return None;
    }
    // `LevelReader.getBiome` zooms first. If that quart's chunk is missing,
    // `ClientLevel.getUncachedNoiseBiome` supplies plains.
    let [x, y, z] = zoom.quart_at_block(pos).map(|q| q << 2);
    let id = world
        .noise_biome(x, y, z)
        .unwrap_or_else(|| biomes.id("minecraft:plains"));
    biomes.get(id).snows_at(pos, sea_level)
}

/// What `Level.precipitationAt` reads of the level in place: it rains (the rain level above 0.2),
/// the sky light is 15, the motion-blocking height, and the biome's precipitation there
/// (`ClientLevel.getPrecipitationAt`'s zoomed biome; snow is `Some(true)` there).
pub(in crate::game) struct GamePrecipitation<'a, 'g>(pub(in crate::game) &'a Game<'g>);

impl sunrise_world::level::weather::PrecipitationReads for GamePrecipitation<'_, '_> {
    fn is_raining(&self) -> bool {
        self.0.weather_levels(1.0).0 > 0.2
    }
    fn can_see_sky(&self, p: BlockPos) -> bool {
        self.0.world.light(p.x, p.y, p.z) >> 4 == 15
    }
    fn motion_blocking_y(&self, p: BlockPos) -> i32 {
        self.0.rain_height(p.x, p.z)
    }
    fn biome_precipitation(&self, p: BlockPos) -> sunrise_world::level::PrecipitationKind {
        use sunrise_world::level::PrecipitationKind;
        let game = self.0;
        match precipitation_at(
            &game.world,
            world_biomes!(game),
            game.streaming.biome_zoom,
            p,
            game.weather_sea_level(),
        ) {
            Some(true) => PrecipitationKind::Snow,
            Some(false) => PrecipitationKind::Rain,
            None => PrecipitationKind::None,
        }
    }
}

impl Game<'_> {
    /// The joined `ClientLevel`'s sea level, or the server level's generator setting.
    pub(in crate::game) fn weather_sea_level(&self) -> i32 {
        self.remote
            .as_ref()
            .map_or_else(|| self.dim.sea_level(), |remote| remote.sea_level)
    }

    /// `Level.isDarkOutside`.
    pub(in crate::game) fn dark_outside(&self) -> bool {
        !self.dimension_type().has_fixed_time && self.sky_darken() >= 4
    }

    pub(in crate::game) fn can_have_weather(&self) -> bool {
        level_can_have_weather(
            self.world.shape.sky_light,
            self.world.shape.has_ceiling,
            self.dim == Dim::End,
        )
    }

    /// The rain and thunder levels (`Level.getRainLevel`, `getThunderLevel`):
    /// the world's, where the level can have weather, else none.
    pub(in crate::game) fn weather_levels(&self, partial: f32) -> (f32, f32) {
        if self.can_have_weather() {
            self.world.weather.levels(partial)
        } else {
            (0.0, 0.0)
        }
    }

    /// `Level.getSkyDarken`: the same dimension, time and weather calculation used by gameplay.
    pub(in crate::game) fn sky_darken_in(&self, world: &World, dimension: Dim) -> i32 {
        // A client of a server has the environment its server's type made for the level it is
        // in; a parked level keeps the client's own for its dimension.
        let environment = match self
            .remote
            .as_ref()
            .and_then(|r| r.level_environment.as_deref())
            .filter(|_| dimension == self.dim)
        {
            Some(own) => own,
            None => match dimension {
                Dim::Overworld => self.res.environment(),
                Dim::Extra(_) => sunrise_client::environment::Environment::for_level(
                    dimension.id(),
                    self.res.environment(),
                ),
                Dim::Nether => self.res.nether_environment(),
                Dim::End => self.res.end_environment(),
            },
        };
        let (rain, thunder) = if level_can_have_weather(
            world.shape.sky_light,
            world.shape.has_ceiling,
            dimension == Dim::End,
        ) {
            self.world.weather.levels(1.0)
        } else {
            (0.0, 0.0)
        };
        let day_time = self.command_clock_time(&self.level_clock_name(dimension));
        sunrise_client::environment::sky_darken(environment, day_time, rain, thunder)
    }

    pub(in crate::game) fn sky_darken(&self) -> i32 {
        if self.dim == Dim::Nether {
            return 11;
        }
        if self.dim == Dim::End {
            return sunrise_client::environment::sky_darken(
                self.res.end_environment(),
                self.world.day_time,
                0.0,
                0.0,
            );
        }
        let (rain, thunder) = self.weather_levels(1.0);
        // Sampled once per (dimension, time, weather): every mob asks each tick.
        let key = (
            self.dim,
            self.world.day_time,
            rain.to_bits(),
            thunder.to_bits(),
        );
        if let Some((k, darken)) = self.sky_darken_memo.get()
            && k == key
        {
            return darken;
        }
        let darken = self.sky_darken_in(&self.world, self.dim);
        self.sky_darken_memo.set(Some((key, darken)));
        darken
    }

    /// `Level.isRainingAt`: `Level.precipitationAt`
    /// (`sunrise_world::level::weather::precipitation_at`) is rain.
    pub(in crate::game) fn raining_at(&self, p: BlockPos) -> bool {
        sunrise_world::level::weather::precipitation_at(&GamePrecipitation(self), p)
            == sunrise_world::level::PrecipitationKind::Rain
    }

    /// `getHeight(Heightmap.Types.MOTION_BLOCKING, x, z)`: one above the
    /// highest block in `blocks_motion_in_heightmap` or holding a fluid.
    pub(in crate::game) fn rain_height(&self, x: i32, z: i32) -> i32 {
        render_tick::motion_blocking_height(&self.world, &self.pack.assets.properties, x, z)
    }
}
