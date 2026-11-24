use super::*;

/// The hook point. Does nothing in Sunrise.
impl Game<'_> {
    /// `register_goals` of the mob's entity parts, asked with the read host: the moment
    /// vanilla's `Mob` constructor runs `registerGoals`. Server-side.
    pub(in crate::game) fn native_register_goals(&self, goals: &mut mod_api::GoalRegistrar<'_>) {
        mod_api::dispatch_register_goals(self, goals);
    }
    /// A mob as the game constructs it: the species' own goals and brain, then every entity
    /// part's `register_goals` and `register_behaviors` with the read host, as vanilla's
    /// constructors run `registerGoals` and `makeBrain`.
    pub(in crate::game) fn new_mob(
        &self,
        id: u32,
        kind: std::sync::Arc<sunrise_client::mobs::MobType>,
        position: sunrise_core::geometry::Vec3,
        yaw: f32,
    ) -> sunrise_client::mobs::Mob {
        let mut mob = sunrise_client::mobs::Mob::new(id, kind, position, yaw);
        mob.attach_mod_parts(self);
        mob
    }
}
