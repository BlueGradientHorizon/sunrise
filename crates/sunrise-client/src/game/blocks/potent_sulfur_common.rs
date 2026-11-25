//! Common potent sulfur state/source queries and sound constants.
use super::Game;
use sunrise_client::{
    block_entities::Pos, particles::ParticleLevel, potent_sulfur::find_noxious_gas_source_block,
};
use sunrise_gameplay::potent_sulfur::Geyser;
pub(in crate::game) use sunrise_gameplay::potent_sulfur::PassMobs;
use sunrise_world::registry::BlockStateId;

/// `SoundEvents.GEYSER_ERUPTION_ACTIVE`.
pub(in crate::game) const GEYSER_ERUPTION_ACTIVE: &str =
    "minecraft:block.potent_sulfur.geyser_eruption_active";
/// `SoundEvents.GEYSER_CONTINUOUS_ACTIVE`.
pub(in crate::game) const GEYSER_CONTINUOUS_ACTIVE: &str =
    "minecraft:block.potent_sulfur.geyser_continuous_eruption_active";

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// The `potent_sulfur_state` of the block at `pos`, with its block state.
    pub(in crate::game) fn potent_sulfur_state(&self, pos: Pos) -> Option<(BlockStateId, Geyser)> {
        let state = self.world.block(pos.0, pos.1, pos.2);
        let value = self
            .pack
            .assets
            .registry
            .state(state)?
            .properties
            .get("potent_sulfur_state")?;
        let geyser = match value.as_str() {
            "dry" => Geyser::Dry,
            "wet" => Geyser::Wet,
            "dormant" => Geyser::Dormant,
            "erupting" => Geyser::Erupting,
            "continuous" => Geyser::Continuous,
            _ => return None,
        };
        Some((state, geyser))
    }

    /// `PotentSulfurBlockEntity.findNoxiousGasSourceBlock` of the sulfur at `pos`.
    pub(in crate::game) fn potent_sulfur_source(&self, pos: Pos) -> Option<[i32; 3]> {
        let level = ParticleLevel {
            world: &self.world,
            facts: self.res.facts(),
            collision: self.collision,
            player: None,
            cached_chunk: std::cell::Cell::new(None),
        };
        find_noxious_gas_source_block(&level, [pos.0, pos.1, pos.2])
    }
}
