impl Game<'_> {
    fn unscoped_game_stack_source(&self, place: &ItemPlace) -> Option<EquipmentRead<'_>> {
        let stack = match place {
            ItemPlace::ItemEntity { id } => {
                return self
                    .unscoped_item_entity_stack(*id)
                    .map(EquipmentRead::resident);
            }
            ItemPlace::EntitySlot { entity, slot } => {
                let vehicle = self
                    .carts
                    .iter()
                    .find(|c| c.id == *entity && c.kind.slots() > 0)
                    .map(|cart| cart.items.as_slice())
                    .or_else(|| {
                        self.boats
                            .iter()
                            .find(|b| b.id == *entity && b.chest)
                            .map(|boat| boat.items.as_slice())
                    });
                let slots = if let Some(slots) = vehicle {
                    // The placeholder owns the live slots while a vehicle's menu is open.
                    let key = (i32::MAX - 15, super::minecarts::CART_KEY_Y, *entity as i32);
                    match self.world.block_entities.get(&key) {
                        Some(super::BlockEntity::Chest(open)) => open.as_slice(),
                        _ => slots,
                    }
                } else {
                    let mob = self
                        .mobs
                        .iter()
                        .find(|m| m.id == *entity && m.extra.chested)?;
                    let size = super::mount_menu::mount_columns(mob) * 3;
                    &mob.extra.items[..size.min(mob.extra.items.len())]
                };
                slots.get(*slot)?.as_ref()
            }
            ItemPlace::Equipment { entity, slot } => {
                return self
                    .equipment_stack_source(*entity, *slot)?
                    .filter(|stack| stack.base().count > 0);
            }
            ItemPlace::PlayerSlot { player, .. } | ItemPlace::PlayerHand { player, .. } => {
                let (inventory, armor, selected, offhand) =
                    match self.native_item_place_player(*player)? {
                        PlayerRead::Acting { .. } => {
                            (&self.inventory, &self.armor, self.selected, &self.offhand)
                        }
                        PlayerRead::Guest(guest) => (
                            &guest.inventory,
                            &guest.armor,
                            guest.selected,
                            &guest.offhand,
                        ),
                    };
                return PlayerStackSlot::borrow(place, inventory, armor, selected, offhand)
                    .map(|(_, read)| read);
            }
            ItemPlace::PlayerCursor { menu } => {
                let guest = self.native_menu_guest(*menu)?;
                if self.lan_acting == Some(guest.conn) {
                    self.native_live_acting_body()?;
                    self.carried.as_ref()
                } else if !guest.dead {
                    guest.carried.as_ref()
                } else {
                    None
                }
            }
            _ => None,
        }?;
        (stack.count > 0).then_some(EquipmentRead::resident(stack))
    }

    #[cold]
    #[inline(never)]
    fn scoped_game_stack_source(&self, place: &ItemPlace) -> Option<EquipmentRead<'_>> {
        if let ItemPlace::ItemEntity { id } = place {
            return self
                .scoped_item_entity_stack(*id)
                .map(EquipmentRead::resident);
        }
        self.unscoped_game_stack_source(place)
    }
}
