#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    /// `C::ForgetLevelChunk`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_forget_level_chunk_packet(&mut self, p: play::ForgetLevelChunk) {
        // `poll_remote` forgets a run of them at once.
        let [x, z] = p.pos;
        self.forget_remote_packets(&mut [(x, z)].into_iter().collect());
    }

    /// `C::BlockDestruction`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_block_destruction_packet(&mut self, p: play::BlockDestruction) {
        // ClientPacketListener.handleBlockDestruction: another player's crack.
        self.destroy_block_progress(p.id, p.pos, i32::from(p.progress));
    }

    /// `C::SectionBlocksUpdate`: apply verified states in packet order.
    pub(in crate::game) fn apply_section_blocks_update_packet(
        &mut self,
        p: &play::SectionBlocksUpdate,
        mut groups: crate::game::native_connection_groups::GroupUpdates,
    ) {
        for (pos, state) in super::super::remote_chunks::section_blocks(p) {
            self.server_block(pos, state, groups.remove(&pos));
        }
    }

    /// `C::Sound`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_sound_packet(&mut self, p: play::Sound) {
        if let Some(name) = self.remote_sound_name(&p.sound)
            && !self.pvp_mutes_sound(&name)
        {
            let at = [
                f64::from(p.x) / 8.0,
                f64::from(p.y) / 8.0,
                f64::from(p.z) / 8.0,
            ];
            let mut sound = LevelSound::new(&name, at, p.volume, p.pitch);
            if let Ok(source) = usize::try_from(p.source)
                && let Some(source) = mod_api::SoundSource::ALL.get(source)
            {
                sound.source = *source;
            }
            self.play_client_level_sound(sound, p.seed);
        }
    }

    /// `C::SoundEntity`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_sound_entity_packet(&mut self, p: play::SoundEntity) {
        if let Some(name) = self.remote_sound_name(&p.sound)
            && !self.pvp_mutes_sound(&name)
        {
            let mut sound = LevelSound::new(&name, [0.0; 3], p.volume, p.pitch);
            sound.emitter = Some(p.id as u32);
            if let Ok(source) = usize::try_from(p.source)
                && let Some(source) = mod_api::SoundSource::ALL.get(source)
            {
                sound.source = *source;
            }
            self.play_client_level_sound(sound, p.seed);
        }
    }

    /// `C::SetTime`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_set_time_packet(&mut self, p: play::SetTime) {
        // ClientPacketListener.handleSetTime / ClientClockManager.handleUpdates:
        // advance all clocks to the packet's game time before applying its
        // (possibly partial) updates. The sky still uses only the default clock.
        self.world.game_time = p.game_time;
        self.remote_clock_tick();
        if let Some(r) = self.remote.as_mut() {
            for clock in p.clocks {
                if r.registries
                    .name("minecraft:world_clock", clock.clock)
                    .is_some()
                {
                    r.clocks.insert(clock.clock, clock);
                }
            }
            self.world.day_time = r.clock_time(r.default_clock);
        }
    }

    /// `C::InitializeBorder`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_initialize_border_packet(&mut self, p: play::InitializeBorder) {
        let b = &mut self.border;
        b.set_center([p.center_x, p.center_z]);
        if p.lerp_time > 0 {
            b.lerp_between(p.old_size, p.new_size, p.lerp_time);
        } else {
            b.set_size(p.new_size);
        }
        b.set_absolute_max_size(p.absolute_max_size);
        b.warning_distance = p.warning_blocks;
        b.warning_time = p.warning_time;
    }

    /// `C::SetBorderLerpSize`: its arms of `apply_packet`, guard and order kept.
    pub(in crate::game) fn apply_set_border_lerp_size_packet(
        &mut self,
        p: play::SetBorderLerpSize,
    ) {
        self.border
            .lerp_between(p.old_size, p.new_size, p.lerp_time);
    }

    /// `ClientPacketListener.handleSetSpawn` / `ClientLevel.setRespawnData`: store the
    /// border-adjusted spawn position with the packet's dimension for the compass.
    pub(in crate::game) fn apply_set_default_spawn_position_packet(
        &mut self,
        p: play::SetDefaultSpawnPosition,
    ) {
        let [mut x, mut y, mut z] = p.pos.pos;
        // Level.getWorldBorderAdjustedRespawnData tests the integer BlockPos against
        // the active level's border, even when the packet names a different level.
        if let Some(remote) = &self.remote
            && !self
                .border
                .geometry()
                .is_within_bounds(f64::from(x), f64::from(z), 0.0)
        {
            [x, z] = self.border.center().map(sunrise_core::mth::floor);
            // Level.getHeight: the horizontal range is half-open; outside it use
            // this level's sea level, and for missing chunks use its build minimum.
            const WORLD_RANGE: std::ops::Range<i32> = -30_000_000..30_000_000;
            y = if WORLD_RANGE.contains(&x) && WORLD_RANGE.contains(&z) {
                self.world
                    .chunks
                    .get(&(x >> 4, z >> 4))
                    .and_then(|chunk| chunk.packet_motion_blocking_height(x, z))
                    .unwrap_or(remote.dimension.min_y)
            } else {
                remote.sea_level.wrapping_add(1)
            };
        }
        self.spawn = Some(Vec3::new(
            f64::from(x) + 0.5,
            f64::from(y),
            f64::from(z) + 0.5,
        ));
        self.level.world_spawn_dimension =
            (p.pos.dimension != "minecraft:overworld").then_some(p.pos.dimension);
    }
}
