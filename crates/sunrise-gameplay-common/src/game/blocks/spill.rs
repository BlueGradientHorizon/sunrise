

#[cfg(test)]
pub(crate) mod tests {

use super::*;

struct RemovalMod(u8);
struct RemovalPart(u8, mod_api::ItemKey);
#[mod_api::sunrise_part]
    impl mod_api::BlockPart for RemovalPart {}
#[mod_api::sunrise_part]
    impl mod_api::BlockEntityPart for RemovalPart {
        fn pre_remove_side_effects(
            &self,
            _: &mut dyn mod_api::LevelRandomHost,
            event: &mut mod_api::PreRemoveSideEffectsEvent<'_>,
        ) {
            match self.0 {
                1 => event.keep_contents = true,
                2 => {
                    let key = self.1;
                    event.spawns.push((
                        mod_api::ItemStack::new(key, 1).unwrap(),
                        [1.25, 65.75, -2.5],
                    ));
                }
                3 => event.experience.push((10, [1.25, 65.75, -2.5])),
                4 => {
                    event.drops.push(None);
                    let key = self.1;
                    event
                        .drops
                        .push(Some(mod_api::ItemStack::new(key, 1).unwrap()));
                }
                _ => {}
            }
        }
    }
#[mod_api::__subscriptions]
    impl mod_api::Hooks for RemovalMod {
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            let kind = r.block_entity_type("machine").blocks(&[]).build();
            let diamond = r.item_key("minecraft:diamond");
            r.block_entity_part(&kind, RemovalPart(self.0, diamond))
                .unwrap();
        }
    }
/// Listens to `block_drops`: takes any break of dirt, adds 2 points to the rest.
    struct DropsMod;
#[mod_api::__subscriptions]
    impl mod_api::Hooks for DropsMod {
        fn block_drops(
            &self,
            _: &dyn mod_api::LevelHost,
            event: &mut mod_api::BlockDropsEvent<'_>,
        ) -> mod_api::Flow {
            if event.drops.iter().any(|d| {
                d.stack().is_some_and(|s| {
                    mod_api::__private::item_key_id(s.item()) == Some("minecraft:dirt")
                })
            }) {
                return mod_api::Flow::Cancel;
            }
            event.experience += 2;
            mod_api::Flow::Continue
        }
    }
}
