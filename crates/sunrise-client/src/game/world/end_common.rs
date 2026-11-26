//! Common End environment and portal-frame aim helpers shared by server and client games.
use super::{Game, Resources, remote::Remote};
use sunrise_level::environment::Environment;

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// The environment attributes of the current dimension.
    pub(in crate::game) fn dim_environment(&self) -> &Environment {
        Self::level_environment(self.remote.as_ref(), &self.res, &self.dimension_type().id)
    }

    /// A field-level borrow for the client tick, which also mutably borrows its particles.
    pub(in crate::game) fn level_environment<'a>(
        remote: Option<&'a Remote>,
        resources: &'a Resources,
        dimension_type: &str,
    ) -> &'a Environment {
        // A client of a server takes the type the server sent, built-in levels included.
        remote
            .and_then(|r| r.level_environment.as_deref())
            .unwrap_or_else(|| resources.environment_of(dimension_type))
    }

    /// EnderEyeItem.use's `getPlayerPOVHitResult`: whether the block aimed
    /// at is an End portal frame (the use passes there).
    pub(in crate::game) fn aims_at_end_portal_frame(&self) -> bool {
        self.pick()
            .is_some_and(|hit| &*self.block_name(hit.pos) == "minecraft:end_portal_frame")
    }
}
