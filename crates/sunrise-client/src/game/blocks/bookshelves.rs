//! Client-side enchanting particle power cache; shared queries live in `bookshelves_common.rs`.
use super::Game;
use sunrise_core::position::BlockPos;

impl Game<'_> {
    /// Snapshot at world entry/parts reload, or after received tags and tag-update listeners
    /// are published. Particle workers receive state answers, never a host or mod callback.
    pub(in crate::game::client_side) fn resolve_enchant_particles(&self) {
        if self.is_logical_server() {
            return;
        }
        self.client.ambience.enchant_power.rebuild(
            self.pack.assets.registry.len(),
            |state| {
                let origin = BlockPos::new(0, 0, 0);
                // An already-active provider supplies no particle source.
                self.enchant_power_for_table(None, state, origin, origin)
                    .unwrap_or_default()
            },
            |state| self.enchant_transmits(state),
        );
    }
}
