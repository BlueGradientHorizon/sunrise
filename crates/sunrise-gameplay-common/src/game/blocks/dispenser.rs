

#[cfg(any(test, feature = "test-support"))]
pub mod tests {

use super::*;

use std::sync::{Arc, atomic::AtomicI32, mpsc};

#[cfg(test)]
/// What a dispense probe part heard: which part, and the dispenser it was for.
    #[derive(Clone, Default)]
    pub(in crate::game) struct Heard(
        pub(in crate::game) std::sync::Arc<std::sync::Mutex<Vec<(&'static str, [i32; 3], u32)>>>,
    );
#[cfg(test)]
impl Heard {
        pub(in crate::game::server_side) fn record(
            &self,
            part: &'static str,
            pos: [i32; 3],
            state: u32,
        ) {
            self.0.lock().unwrap().push((part, pos, state));
        }
        pub(in crate::game) fn take(&self) -> Vec<(&'static str, [i32; 3], u32)> {
            std::mem::take(&mut *self.0.lock().unwrap())
        }
    }
#[cfg(test)]
/// Answers for the faced block for dirt and cobblestone, whatever the item's own part says.
    pub(in crate::game) struct Front(pub(in crate::game) Heard);
#[cfg(test)]
#[mod_api::sunrise_part]
    impl mod_api::BlockPart for Front {
        fn dispense_method(
            &self,
            _: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::DispenseMethodEvent<'_>,
        ) -> Option<mod_api::Dispensed> {
            let state = crate::game::api_values::TryIntoEngine::try_into_engine(&event.state, &())
                .unwrap()
                .0;
            self.0.record("block", event.pos, state);
            let item = event.stack.item();
            let answers = ["minecraft:dirt", "minecraft:cobblestone"]
                .iter()
                .any(|name| item == mod_api::__private::item_key(name).unwrap());
            answers.then_some((None, FAIL))
        }
    }
#[cfg(test)]
/// Answers for the item it is attached to, if the block in front let it.
    pub(in crate::game) struct Item(pub(in crate::game) Heard);
#[cfg(test)]
#[mod_api::sunrise_part]
    impl mod_api::ItemPart for Item {
        fn dispense_item(
            &self,
            _: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::DispenseItemEvent<'_>,
        ) -> Option<mod_api::Dispensed> {
            self.0.record("item", event.pos, 0);
            Some((None, CLICK))
        }
    }
#[cfg(test)]
pub(in crate::game) struct Probe(pub(in crate::game) Heard);
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for Probe {
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            let stone = r.block_key("minecraft:stone");
            r.block_part(stone, Front(self.0.clone()));
            for item in ["minecraft:stone", "minecraft:cobblestone"] {
                let item = r.item_key(item);
                r.item_part(item, Item(self.0.clone()));
            }
        }
    }
#[cfg(test)]
/// Dresses only the entity type it is told to: what the part of an iron helmet answers to
    /// `can_equip`, and what it was asked as `(slot, entity, kind, position)`, the position read
    /// from the entity's handle through the read-only `Host`.
    #[derive(Clone)]
    pub(in crate::game) struct Fits {
        pub(in crate::game) accepts: Arc<std::sync::Mutex<&'static str>>,
        pub(in crate::game) asked:
            Arc<std::sync::Mutex<Vec<(String, u32, String, Option<[f64; 3]>)>>>,
    }
#[cfg(test)]
#[mod_api::sunrise_part]
    impl mod_api::ItemPart for Fits {
        fn can_equip(&self, host: &dyn mod_api::Host, event: &mut mod_api::CanEquipEvent<'_>) {
            let at = event.entity.entity().position(host);
            let (slot, kind) = (event.slot.to_owned(), event.kind.to_owned());
            (self.asked.lock().unwrap()).push((slot, event.entity.id(), kind, at));
            event.can_equip = event.kind == *self.accepts.lock().unwrap();
        }
    }
#[cfg(test)]
pub(in crate::game) struct Tailor(pub(in crate::game) Fits);
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for Tailor {
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            let helmet = r.item_key("minecraft:iron_helmet");
            r.item_part(helmet, self.0.clone());
        }
    }
#[cfg(test)]
impl Fits {
        pub(in crate::game::server_side) fn new(accepts: &'static str) -> Self {
            Self {
                accepts: Arc::new(std::sync::Mutex::new(accepts)),
                asked: Arc::default(),
            }
        }
        pub(in crate::game::server_side) fn asked(
            &self,
        ) -> Vec<(String, u32, String, Option<[f64; 3]>)> {
            std::mem::take(&mut *self.asked.lock().unwrap())
        }
        /// Runs `test` with a mod whose part of the iron helmet is this one installed.
        pub(in crate::game::server_side) fn installed(&self, test: impl FnOnce()) {
            let tailor = mod_api::ModEntry::new("tailor", Box::new(Tailor(self.clone())));
            let mods: mod_api::Mods = vec![tailor].into();
            crate::entry::install_client_hooks();
            super::super::api_values::install_parts(&mods).unwrap();
            mod_platform::testing::with_mods(mods, test);
        }
    }
/// An iron helmet whose `equippable` is removed (`fields` None) or replaced by `fields`.
    pub fn helmet_with(
        fields: Option<Vec<(&str, sunrise_core::nbt::Tag)>>,
    ) -> ItemStack {
        use sunrise_core::nbt::Tag;
        let mut helmet = ItemStack::new("minecraft:iron_helmet", 1);
        let (id, value) = match fields {
            Some(fields) => ("minecraft:equippable", Tag::compound(fields)),
            None => ("!minecraft:equippable", Tag::Compound(Default::default())),
        };
        helmet.data_mut().set_other(id, Some(value));
        helmet
    }
/// A stand of entity id `id` standing at `x`, 200, `z`.
    pub fn stand_at(
        id: u32,
        x: f64,
        z: f64,
    ) -> sunrise_sim::entities::ArmorStand {
        let at = sunrise_core::geometry::Vec3::new(x, 200.0, z);
        sunrise_sim::entities::ArmorStand::new(id, at, 0.0)
    }
}
