use super::*;

/// The hook point. Does nothing in Sunrise.
impl Game<'_> {
    /// `register_behaviors` of the mob's entity parts, asked with the read host: the moment
    /// vanilla's `LivingEntity` constructor builds the brain (`makeBrain`). Server-side.
    pub(in crate::game) fn native_register_behaviors(
        &self,
        behaviors: &mut mod_api::BehaviorRegistrar<'_>,
    ) {
        mod_api::dispatch_register_behaviors(self, behaviors);
    }
}
