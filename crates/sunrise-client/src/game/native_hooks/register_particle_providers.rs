use super::*;

impl Game<'_> {
    /// Gather attributed provider factories at startup, before the client loads sprite sets.
    pub(in crate::game) fn native_register_particle_providers(
        context: &mod_api::RegistrationContext,
        providers: &mut Vec<Answered<ParticleProviderRegistration>>,
    ) {
        Self::fire_register_particle_providers(context, providers);
    }
}
