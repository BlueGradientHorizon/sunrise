/// Load-owned particle types and key requests, resolved before any world can emit particles.
pub trait ParticlesRegistrar {
    /// Request an existing particle type by registry ID. Unknown IDs are reported with the
    /// requesting mod when the content registry loads. To introduce a native type, use
    /// [`Self::declare_particle`] instead.
    fn particle_key(&mut self, id: &str) -> crate::ParticleKey;

    /// Publish a native particle type's identity and return its key (NeoForge's particle type
    /// `RegisterEvent`). Declare it on both logical sides. The client separately registers its
    /// sprite-set factory; an absent description does not invalidate the particle type.
    fn declare_particle(&mut self, id: &crate::Identifier) -> crate::ParticleKey;
}
