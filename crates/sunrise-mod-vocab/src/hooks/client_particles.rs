//! Upstream prose, retained with fork routing described in the modding book.
pub trait Notes {
    #[notes = "`ClientHost::add_particle(&ParticleOptions, position, velocity) -> bool` submits one request to the loaded client world. It preserves the typed options and raw velocity slots, including encoded color/size arguments. Normal distance, Particles settings and group admission apply. True means submitted, not spawned: a bound `ParticleProvider::create_particle` can return `None`, and a missing description or failed bind refuses native spawns unless a pack overrides the type. Without a loaded client player/level or with invalid numeric input it returns false. Use `RegisterParticleProvidersEvent::register_sprite_set` to give a registered type owned native instances. Bundles need \"hooks\": @API:client_particles@."]
    fn client_particles();
}
