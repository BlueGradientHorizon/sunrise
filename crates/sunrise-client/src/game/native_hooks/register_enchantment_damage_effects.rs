use super::*;

/// Register damage value effects before loading enchantment content.
pub(in crate::game) fn native_register_enchantment_damage_effects(
    context: &mod_api::RegistrationContext,
    effects: &mut mod_api::EnchantmentDamageRegistry,
) {
    crate::game::native_hooks::fire::fire_register_enchantment_damage_effects(context, |listen| {
        effects.register_listener(listen)
    });
}

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::*;
    use mod_api::{
        EnchantmentDamageContext, EnchantmentDamageEffect, Hooks,
        RegisterEnchantmentDamageEffectsEvent, ServerHost,
    };
    use std::sync::{Arc, Mutex};

    pub(in crate::game) type Observations = Arc<Mutex<Vec<(u32, u32, String, u32, f32)>>>;
    pub(in crate::game) struct Effect {
        pub(in crate::game) reads: Observations,
        pub(in crate::game) scale: bool,
    }
    impl EnchantmentDamageEffect for Effect {
        fn apply(
            &self,
            host: &dyn ServerHost,
            context: &EnchantmentDamageContext<'_>,
            value: f32,
        ) -> f32 {
            assert_eq!(
                host.entity(context.attacker.id()),
                Some(context.attacker.entity())
            );
            assert_eq!(host.entity(context.victim.id()), Some(context.victim));
            assert_eq!(context.weapon.count(), 1);
            assert_eq!(context.weapon.damage(), 11);
            self.reads.lock().unwrap().push((
                context.attacker.id(),
                context.victim.id(),
                mod_api::__private::item_key_id(context.weapon.item())
                    .unwrap()
                    .to_string(),
                context.level,
                value,
            ));
            if self.scale {
                value * 2.0
            } else {
                value + context.level as f32
            }
        }
    }
    pub(in crate::game) struct TestMod(pub(in crate::game) Observations);
    #[mod_api::__subscriptions]
    impl Hooks for TestMod {
        fn register_enchantment_damage_effects(
            &self,
            _: &mod_api::RegistrationContext,
            event: &mut RegisterEnchantmentDamageEffectsEvent<'_>,
        ) {
            for (id, scale) in [("damage_test:double", true), ("damage_test:level", false)] {
                event
                    .effects
                    .register(
                        mod_api::Identifier::parse(id).unwrap(),
                        Arc::new(Effect {
                            reads: self.0.clone(),
                            scale,
                        }),
                    )
                    .unwrap();
            }
        }
    }
}
