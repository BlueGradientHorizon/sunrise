use super::*;

impl Game<'_> {
    /// Collect each listener's attributed sprite-set factories at startup. The particle
    /// engine binds the accepted factories after loading sprites, not during dispatch.
    pub(in crate::game) fn native_register_particle_providers(
        context: &mod_api::RegistrationContext,
        providers: &mut Vec<Answered<ParticleProviderRegistration>>,
    ) {
        Self::fire_register_particle_providers(context, providers);
    }
}
