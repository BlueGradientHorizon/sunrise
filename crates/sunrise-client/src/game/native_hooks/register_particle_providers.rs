use super::*;

impl Game<'_> {
    /// Bind each logical-client sprite-set factory to its common registered particle type.
    /// Registration runs once; resource reloads rebind sprites without replaying the hook.
    pub(in crate::game) fn native_register_particle_providers(
        context: &mod_api::RegistrationContext,
        providers: &mut dyn ParticleProviders,
    ) {
        Self::fire_register_particle_providers(context, providers);
    }
}
