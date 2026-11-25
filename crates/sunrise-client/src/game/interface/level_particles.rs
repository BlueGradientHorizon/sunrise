//! Level events whose client handler (`LevelEventHandler`) spawns particles
//! on a block: bone meal's growth (1505), dragon egg teleportation (2015), a
//! consumable's teleport (2017), an enderman's (2018), electric sparks (3002),
//! waxing (3003), wax off (3004) and scraping (3005), plus the smoke, drips,
//! egg and dragon fireball particles of 1501, 1502, 1504, 2006, 2009, 2012
//! and 3009.
#[allow(unused_imports)]
use super::Game;
#[allow(unused_imports)]
use sunrise_client::items::ItemStack;
#[allow(unused_imports)]
use sunrise_client::particles::{self, ParticleOptions, ParticleType};
#[allow(unused_imports)]
use sunrise_core::{geometry::trig, position::BlockPos, random::RandomSource};
#[allow(unused_imports)]
use sunrise_world::level::LevelEvent;
#[allow(unused_imports)]
use sunrise_world::registry::BlockStateId;

impl Game<'_> {
    /// `BoneMealItem.addGrowthParticles` (event 1505, `count` the data):
    /// happy villager sparks over a spreading block's neighbourhood (grass,
    /// nylium, netherrack, moss) or water, or inside a growing block.
    pub(in crate::game::server_side) fn growth_particles(&mut self, pos: BlockPos, count: i32) {
        let p = [pos.x, pos.y, pos.z];
        let state = self.world.block(p[0], p[1], p[2]);
        let facts = self.res.facts();
        // ParticleUtils.spawnParticles(level, pos, count, xzSpread, ySpread,
        // allowFloatingParticles, particle).
        let (at, count, xz, height, floating) = match self.res.behaviours().bone_meal_spreads(state)
        {
            Some(spreads) => {
                if spreads {
                    // BonemealableBlock.getParticlePos: a NEIGHBOR_SPREADER
                    // (grass, nylium, netherrack, moss) sparks over the block
                    // above it, where its spread lands (#11310).
                    ([p[0], p[1] + 1, p[2]], count * 3, 3.0, 1.0, false)
                } else {
                    // getParticlePos of a GROWER is the block itself, below
                    // for rooted dirt (its hanging roots) and mangrove leaves
                    // (their propagule).
                    let at = match &*facts.get(state).name {
                        "minecraft:rooted_dirt" | "minecraft:mangrove_leaves" => {
                            [p[0], p[1] - 1, p[2]]
                        }
                        _ => p,
                    };
                    // ParticleUtils.spawnParticleInBlock: up to the
                    // block's shape (a full block when air).
                    (at, count, 0.5, self.block_shape_top(at), true)
                }
            }
            None if &*facts.get(state).name == "minecraft:water" => (p, count * 3, 3.0, 1.0, false),
            // A mod block the level answered bone meal for: the port knows no
            // `BonemealableBlock.Type` for it, and vanilla's `GROWER` is the
            // one a block growing itself has, so the sparks stay inside it
            // rather than spreading over its neighbours.
            None if !facts.get(state).name.starts_with("minecraft:") => {
                (p, count, 0.5, self.block_shape_top(p), true)
            }
            None => return,
        };
        self.spawn_particles(at, count, xz, height, floating, ParticleType::HappyVillager);
    }
    /// `BlockState.getShape(..).max(Y)` of the block at `at`, 1.0 for air.
    pub(in crate::game::server_side) fn block_shape_top(&self, at: [i32; 3]) -> f64 {
        let facts = self.res.facts();
        let inside = self.world.block(at[0], at[1], at[2]);
        if facts.is_air(inside) {
            1.0
        } else {
            facts
                .get(inside)
                .outline
                .iter()
                .map(|b| b[4])
                .fold(0.0, f64::max)
        }
    }
    /// `LevelEventHandler` 2011 (a bee grew a crop) and 2012 (a turtle egg
    /// laid on sand): `ParticleUtils.spawnParticleInBlock` with happy villager
    /// sparks.
    pub(in crate::game::server_side) fn turtle_egg_particles(&mut self, pos: BlockPos, count: i32) {
        let at = [pos.x, pos.y, pos.z];
        let height = self.block_shape_top(at);
        self.spawn_particles(at, count, 0.5, height, true, ParticleType::HappyVillager);
    }
    /// `SpeleothemBlock.isStalactite`: a speleothem whose tip points down.
    pub(in crate::game) fn is_stalactite(&self, state: BlockStateId) -> bool {
        self.res.facts().get(state).has_class("SpeleothemBlock")
            && self
                .pack
                .assets
                .registry
                .state(state)
                .and_then(|s| s.properties.get("vertical_direction"))
                .is_some_and(|d| d == "down")
    }
    /// `ServerLevel.globalLevelEvent`: heard by every player on the server
    /// (an end portal opening, the dragon's death roar): here, and sent to
    /// every guest whatever their dimension, each at their own position
    /// (`Hub::global_level_event`).
    pub(in crate::game::server_side) fn global_level_event(
        &mut self,
        id: i32,
        pos: [i32; 3],
        data: i32,
    ) {
        // ServerLevel.globalLevelEvent: without global_sound_events it is a
        // plain level event (`levelEvent(null, ...)`), sent to the players
        // near `pos`; the vanilla client has no plain sound for 1023, 1028
        // and 1038 (they are only in `LevelEventHandler.globalLevelEvent`).
        if !self
            .ui
            .rules
            .bool_at(sunrise_client::gamerules::vanilla::GLOBAL_SOUND_EVENTS)
        {
            self.level_event_at(id, pos, data);
            return;
        }
        self.global_level_event_sound(id, pos);
        if self.audio_capture_active() {
            self.server
                .lan_global_events
                .push((self.dim.id(), id, pos, data));
        }
    }
    /// Deliver one committed global level event immediately without touching unrelated
    /// `lan_global_events` or `lan_events`, preserving packet order across effect kinds.
    pub(in crate::game::server_side) fn deliver_global_level_event(
        &mut self,
        id: i32,
        pos: [i32; 3],
        data: i32,
    ) {
        if !self
            .ui
            .rules
            .bool_at(sunrise_client::gamerules::vanilla::GLOBAL_SOUND_EVENTS)
        {
            self.deliver_level_event(id, pos, data);
            return;
        }
        self.global_level_event_sound(id, pos);
        if let Some(host) = self.lan.as_ref() {
            host.hub.global_level_event(self.dim.id(), id, pos, data);
        } else if self.audio_capture_active() {
            self.server
                .lan_global_events
                .push((self.dim.id(), id, pos, data));
        }
    }
    /// `ClientLevel.addBreakingBlockEffects`: the block's hit sound (only for
    /// `play_sound`) and one crumb on the face being mined; nothing for air.
    pub(in crate::game) fn breaking_block_effects(
        &mut self,
        pos: [i32; 3],
        face: [i32; 3],
        play_sound: bool,
    ) {
        let state = self.world.block(pos[0], pos[1], pos[2]);
        if self.res.facts().is_air(state) {
            return;
        }
        if play_sound {
            self.play_mining_hit_sound(state, pos, face, true);
        }
        self.crack_particle(pos, face, state);
    }
    /// Queue and play level event `id` at `pos` (`Level.levelEvent`).
    pub(in crate::game) fn level_event_at(&mut self, id: i32, pos: [i32; 3], data: i32) {
        self.level_event_from(None, id, pos, data);
    }

    /// Deliver one committed level event immediately without draining unrelated `level_events` or
    /// `lan_events`, sending its packet directly when a LAN host is active so packet order matches
    /// commit order across effect kinds.
    pub(in crate::game::server_side) fn deliver_level_event(
        &mut self,
        id: i32,
        pos: [i32; 3],
        data: i32,
    ) {
        let saved_level_events = std::mem::take(&mut self.level_events);
        let saved_lan_events = std::mem::take(&mut self.server.lan_events);
        self.level_events.push(LevelEvent::LevelEvent {
            id,
            pos: BlockPos::new(pos[0], pos[1], pos[2]),
            data,
            source: None,
        });
        let _ = self.process_level_events();
        let recorded = std::mem::replace(&mut self.server.lan_events, saved_lan_events);
        let extra_level_events = std::mem::replace(&mut self.level_events, saved_level_events);
        self.level_events.extend(extra_level_events);
        if let Some(host) = self.lan.as_mut() {
            if id == 2001 {
                host.broken
                    .push([f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2])]);
            }
            host.hub.level_event(id, pos, data, None);
        } else {
            self.server.lan_events.extend(recorded);
        }
    }

    pub(in crate::game::server_side) fn level_event_from(
        &mut self,
        source: Option<sunrise_world::level::LevelEventPlayer>,
        id: i32,
        pos: [i32; 3],
        data: i32,
    ) {
        self.level_events.push(LevelEvent::LevelEvent {
            id,
            pos: BlockPos::new(pos[0], pos[1], pos[2]),
            data,
            source,
        });
        let _ = self.process_level_events();
    }
}

#[cfg(test)]
#[path = "tests/phantom_bite_tests.rs"]
pub(in crate::game) mod phantom_bite_tests;
