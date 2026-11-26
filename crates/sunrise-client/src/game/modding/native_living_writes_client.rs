use super::{Game, commands::Target};
use mod_api::LivingRef;

impl Game<'_, crate::game::client_side::ClientState> {
    pub(in crate::game) fn client_living_read_target(&self, living: LivingRef) -> Option<Target> {
        match self.native_client_entity_target(living.entity())? {
            Target::Player => Some(Target::Player),
            _ => None,
        }
    }

    pub(in crate::game) fn client_living_target_alive(&self, _id: u32) -> bool {
        false
    }

    pub(in crate::game) fn client_living_absorption(&self, living: LivingRef) -> Option<f32> {
        let Target::Player = self.client_living_read_target(living)? else {
            return None;
        };
        Some(self.player.as_ref()?.health.absorption())
    }

    pub(in crate::game) fn client_living_attribute_value(
        &self,
        living: LivingRef,
        id: &str,
    ) -> Option<f64> {
        let Target::Player = self.client_living_read_target(living)? else {
            return None;
        };
        let player = self.player.as_ref()?;
        let key = player.attributes.key(id)?;
        player
            .attributes
            .value(Some(key), sunrise_world::attribute_map::ValueKind::Computed)
            .ok()
    }

    pub(in crate::game) fn client_living_equipment_stack(
        &self,
        living: LivingRef,
        slot: mod_api::EquipmentSlot,
    ) -> Option<sunrise_sim::mobs::finalize::EquipmentRead<'_>> {
        use mod_api::EquipmentSlot;
        let Target::Player = self.client_living_read_target(living)? else {
            return None;
        };
        let stack = match slot {
            EquipmentSlot::MainHand => self.inventory.get(self.selected)?.as_ref(),
            EquipmentSlot::OffHand => self.offhand.as_ref(),
            EquipmentSlot::Head => self.armor[0].as_ref(),
            EquipmentSlot::Chest => self.armor[1].as_ref(),
            EquipmentSlot::Legs => self.armor[2].as_ref(),
            EquipmentSlot::Feet => self.armor[3].as_ref(),
            EquipmentSlot::Body | EquipmentSlot::Saddle => self.equipment.get(slot as usize),
        }?;
        Some(sunrise_sim::mobs::finalize::EquipmentRead::resident(stack))
    }

    pub(in crate::game) fn client_living_use_ticks(
        &self,
        living: LivingRef,
    ) -> Option<Option<u32>> {
        let Target::Player = self.client_living_read_target(living)? else {
            return None;
        };
        Some(self.using.map(|(_, ticks)| ticks))
    }

    pub(in crate::game) fn client_living_used_item_hand(
        &self,
        living: LivingRef,
    ) -> Option<mod_api::client::first_person::Hand> {
        let Target::Player = self.client_living_read_target(living)? else {
            return None;
        };
        self.used_hand()
    }

    pub(in crate::game) fn client_item_combat_actor(
        &self,
        actor: LivingRef,
    ) -> Option<(bool, bool, f64, bool)> {
        let Target::Player = self.client_living_read_target(actor)? else {
            return None;
        };
        let p = self.player.as_ref()?;
        Some((true, self.creative, p.fall_distance, p.fall_flying))
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::super::native_entity_refs::tests::with_game;
    use crate::game::server_side::native_living_writes::tests::Shared;
    use crate::game::server_side::native_living_writes::tests::UseMod;
    use mod_api::{LivingRef, ModEntry, Mods};
    use sunrise_core::geometry::Vec3;

    /// Runs `test` on a game whose acting player stands at the origin, the use
    /// button down, under a mod that fills the returned record.
    pub(in crate::game) fn with_uses(
        test: impl FnOnce(&mut crate::game::ServerGame<'_>, &Shared, LivingRef),
    ) {
        let uses = Shared::default();
        let mods: Mods = vec![ModEntry::new("uses", Box::new(UseMod(uses.clone())))].into();
        crate::game::api_values::install_parts(&mods).unwrap();
        mod_platform::testing::with_mods(mods, || {
            with_game(|game| {
                crate::game::native_entity_refs::tests::resolve_parts(game).unwrap();
                game.player =
                    Some(sunrise_sim::motion::PlayerMotion::new(game.motion, Vec3::ZERO).unwrap());
                crate::game::native_entity_refs::tests::bind_local_player(game, 20, 123);
                game.server_set_acting_use(true);
                let player = game.living(game.entity(20).unwrap()).unwrap();
                test(game, &uses, player);
            });
        });
    }
}
