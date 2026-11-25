#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::golems::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::Result;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::block_pattern::{
    IRON_GOLEM_BASE, IRON_GOLEM_FULL, SNOW_GOLEM_BASE, SNOW_GOLEM_FULL,
};

impl Game<'_> {
    /// The copper golem (`CarvedPumpkinBlock.trySpawnGolem`, after the snow
    /// and iron golems): both blocks clear, the golem appears where the
    /// pumpkin was, the copper block becomes the matching copper chest facing
    /// the pumpkin's way (`replaceCopperBlockWithChest`, flags 2) and the
    /// golem takes the block's weathering (`CopperGolem.spawn`).
    pub(in crate::game) fn try_spawn_copper_golem(&mut self, head: [i32; 3]) -> Result<()> {
        if !self.copper_golem_base(head) {
            return Ok(());
        }
        let below = [head[0], head[1] - 1, head[2]];
        let copper = self.world.block(below[0], below[1], below[2]);
        let pumpkin = self.world.block(head[0], head[1], head[2]);
        let registry = self.pack.assets.registry;
        let facing = registry
            .state(pumpkin)
            .and_then(|s| s.properties.get("facing").cloned())
            .unwrap_or_else(|| "north".into());
        // getWeatherStateFromPattern: waxed blocks weather like their unwaxed form.
        let base = self
            .golem_block(below)
            .trim_start_matches("minecraft:")
            .trim_start_matches("waxed_");
        let weather = ["exposed", "weathered", "oxidized"]
            .iter()
            .position(|w| base.starts_with(w))
            .map_or(0, |i| i as u8 + 1);
        let cells = [head, below];
        self.clear_pattern_blocks(&cells)?;
        let at = [
            f64::from(head[0]) + 0.5,
            f64::from(head[1]) + 0.05,
            f64::from(head[2]) + 0.5,
        ];
        let id = self.create_triggered("minecraft:copper_golem", at, 0.0);
        self.trigger_summoned(id, 5.0);
        self.update_pattern_blocks(&cells)?;
        let direction = match facing.as_str() {
            "south" => sunrise_core::geometry::Direction::South,
            "west" => sunrise_core::geometry::Direction::West,
            "east" => sunrise_core::geometry::Direction::East,
            _ => sunrise_core::geometry::Direction::North,
        };
        let pos = sunrise_core::position::BlockPos::new(below[0], below[1], below[2]);
        let chest = self.with_level(|b, level, ticks| {
            b.copper_chest_from_block(level, ticks, copper, direction, pos)
        });
        if let Some(chest) = chest {
            self.with_level(|b, level, ticks| b.set_block(level, ticks, pos, chest, 2));
            if let Some(entity) = sunrise_client::containers::BlockEntity::for_block(
                &self.res.facts().get(chest).name,
            ) {
                self.world
                    .block_entities
                    .insert((below[0], below[1], below[2]), entity);
            }
        }
        if let Some(m) = id.and_then(|id| self.mobs.iter_mut().find(|m| m.id == id)) {
            m.extra.variant = weather;
            let p = [m.position.x, m.position.y, m.position.z];
            self.server_play_sound("minecraft:entity.copper_golem.spawn", p, 1.0, 1.0);
        }
        self.process_level_events()
    }
}
