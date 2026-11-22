//! Register and request particle identities independently of client sprite descriptions.
use super::*;

impl ParticlesRegistrar for PartRegistrar {
    fn particle_key(&mut self, id: &str) -> crate::ParticleKey {
        match self
            .registry_keys
            .borrow_mut()
            .particles
            .request(self.mod_id, id)
        {
            Ok(slot) => mod_vocab::__private::parts_installation::particle_key(slot),
            Err(error) => {
                self.refused.push(error);
                mod_vocab::__private::parts_installation::particle_key(u32::MAX)
            }
        }
    }

    fn declare_particle(&mut self, id: &crate::Identifier) -> crate::ParticleKey {
        let mut keys = self.registry_keys.borrow_mut();
        match keys.particles.request_identifier(self.mod_id, id) {
            Ok(slot) => {
                keys.declared_particles.insert(id.to_string());
                mod_vocab::__private::parts_installation::particle_key(slot)
            }
            Err(error) => {
                self.refused.push(error);
                mod_vocab::__private::parts_installation::particle_key(u32::MAX)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Hooks, ModEntry, RegistrationContext};
    use std::sync::Mutex;
    #[test]
    fn particle_registration_retains_requested_slots_until_atomic_publication() {
        struct Keys(Arc<Mutex<Vec<crate::ParticleKey>>>);
        #[mod_api::__subscriptions]
        impl Hooks for Keys {
            fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
                let custom = registrar.particle_key("test:spark");
                let smoke = registrar.particle_key("smoke");
                let native = registrar
                    .declare_particle(&crate::Identifier::parse_namespaced("test:native").unwrap());
                assert_eq!(native, registrar.particle_key("test:native"));
                assert_eq!(
                    mod_vocab::__private::parts_installation::validate_particle_key(
                        mod_vocab::__private::parts_installation::particle_key_slot(custom)
                    ),
                    Ok(custom)
                );
                assert_eq!(smoke, registrar.particle_key("minecraft:smoke"));
                self.0.lock().unwrap().extend([custom, smoke, native]);
            }
        }
        let keys = Arc::new(Mutex::new(Vec::new()));
        install_parts(
            &[ModEntry::new("keys", Box::new(Keys(keys.clone())))],
            &crate::parts::tests::rule_schema(),
            |_| false,
        )
        .unwrap();
        let keys = keys.lock().unwrap();
        assert_eq!(
            mod_vocab::__private::resolve_particles(["minecraft:smoke"], |_| Some(
                sunrise_mod_runtime::registry_keys::ParticleKind::Custom
            )),
            Err(vec![
                "mod keys requested unknown particle test:spark".to_owned()
            ]),
        );
        assert!(!mod_vocab::__private::particles_resolved());
        mod_vocab::__private::resolve_particles(["minecraft:smoke", "test:spark"], |id| {
            Some(if id == "minecraft:smoke" {
                sunrise_mod_runtime::registry_keys::ParticleKind::Vanilla(5)
            } else {
                sunrise_mod_runtime::registry_keys::ParticleKind::Custom
            })
        })
        .unwrap();
        assert_eq!(
            mod_vocab::__private::particle_name(keys[0]),
            Some("test:spark")
        );
        assert_eq!(mod_vocab::__private::particle_vanilla_key(5), Some(keys[1]));
        assert_eq!(
            mod_vocab::__private::particle_named_key("test:native"),
            Some(keys[2])
        );
        assert_eq!(
            mod_vocab::__private::particle_kind(keys[0]),
            Some(sunrise_mod_runtime::registry_keys::ParticleKind::Custom)
        );
    }
}
