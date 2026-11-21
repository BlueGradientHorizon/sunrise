//! Equipment-aware movement helpers for `AbstractContainerMenu`.
use super::*;

impl Menu {
    pub(super) fn move_item_stack_to(
        &mut self,
        env: &Env,
        stack: &mut ItemStack,
        start: usize,
        end: usize,
        backwards: bool,
    ) -> bool {
        self.touch();
        let mut changed = false;
        let order = |_: ()| -> Vec<usize> {
            if backwards {
                (start..end).rev().collect()
            } else {
                (start..end).collect()
            }
        };
        if Self::stackable(env, stack) {
            for dest in order(()) {
                if stack.count == 0 {
                    break;
                }
                // Empty destinations belong only to the insertion pass. Do not ask their
                // placement rule with a different top-up maximum first.
                if !self.has_item(dest) {
                    continue;
                }
                // A mod menu's result (unless a mod's hook opens it) and a
                // hidden slot get no top-up either, as a furnace's quick move
                // never aims at its result.
                let refuses = if self.state.slots[dest].role == Role::Hidden {
                    true
                } else if let Some(id) = self.registry_id() {
                    !self
                        .ask_slot_rules(
                            env,
                            id,
                            dest,
                            stack,
                            (
                                self.state.slots[dest].role != Role::ModResult,
                                true,
                                CONTAINER_MAX,
                            ),
                            Self::max_stack(env, stack),
                        )
                        .0
                } else {
                    false
                };
                let max = match self.slot(dest) {
                    Some(target) if !refuses && target.same_item_same_components(stack) => {
                        self.slot_max(env, dest, target)
                    }
                    _ => continue,
                };
                let slot = self.state.slots[dest];
                let target = self.state.containers[slot.container][slot.index]
                    .as_mut()
                    .expect("checked");
                let total = target.count + stack.count;
                if total <= max {
                    stack.count = 0;
                    target.count = total;
                    changed = true;
                    self.slot_changed(env, dest);
                } else if target.count < max {
                    stack.count -= max - target.count;
                    target.count = max;
                    changed = true;
                    self.slot_changed(env, dest);
                }
            }
        }
        if stack.count > 0 {
            for dest in order(()) {
                if self.has_item(dest) {
                    continue;
                }
                let (allowed, max) = self.placement_rules(env, dest, stack);
                if allowed {
                    let part = split(stack, stack.count.min(max));
                    self.set_by_player(env, dest, Some(part));
                    changed = true;
                    break;
                }
            }
        }
        changed
    }

    pub(super) fn safe_insert_with_placement(
        &mut self,
        env: &Env,
        s: usize,
        input: &mut Option<ItemStack>,
        amount: u32,
        allowed: bool,
        max: u32,
    ) {
        let Some(stack) = input.as_mut() else { return };
        if !allowed {
            return;
        }
        let have = self.slot(s).map_or(0, |c| c.count);
        let transferable = amount.min(stack.count).min(max.saturating_sub(have));
        if transferable == 0 {
            return;
        }
        match self.slot(s).cloned() {
            None => {
                let part = split(stack, transferable);
                self.set_by_player(env, s, Some(part));
            }
            Some(mut current) if current.same_item_same_components(stack) => {
                stack.count -= transferable;
                current.count += transferable;
                self.set_by_player(env, s, Some(current));
            }
            Some(_) => {}
        }
        tidy(input);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobs::equip::CanEquip;
    use std::cell::Cell;

    #[derive(Default)]
    struct FeetOnly {
        asks: Cell<usize>,
    }
    impl CanEquip for FeetOnly {
        fn can_equip(&self, _: &ItemStack, slot: &str, _: u32, _: &str, _: bool) -> bool {
            self.asks.set(self.asks.get() + 1);
            slot == "feet"
        }
    }

    #[test]
    #[ignore = "daily"]
    fn four_slot_menu_transfer_asks_each_armor_slot_until_accepted() {
        super::super::tests::with_menu(Kind::Inventory, |env, menu| {
            let parts = FeetOnly::default();
            let env = Env {
                can_equip: Some(&parts),
                player_entity_id: Some(7),
                ..*env
            };
            let mut helmet = ItemStack::new("minecraft:iron_helmet", 3);
            assert!(menu.move_to(&env, &mut helmet, 5, 9, false));
            assert_eq!(parts.asks.get(), 4);
            assert_eq!(helmet.count, 2);
            assert_eq!(menu.slot(8).unwrap().count, 1);
            assert!((5..8).all(|slot| menu.slot(slot).is_none()));
        });
    }

    #[test]
    #[ignore = "daily"]
    fn menus_without_actor_or_equipment_candidates_do_not_ask_can_equip() {
        super::super::tests::with_menu(Kind::Inventory, |env, menu| {
            let parts = FeetOnly::default();
            let env = Env {
                can_equip: Some(&parts),
                player_entity_id: None,
                ..*env
            };
            let mut helmet = ItemStack::new("minecraft:iron_helmet", 1);
            assert!(menu.move_to(&env, &mut helmet, 5, 6, false));
            assert_eq!(menu.slot(5).unwrap().item.as_ref(), "minecraft:iron_helmet");
            assert_eq!(parts.asks.get(), 0);
            let env = Env {
                player_entity_id: Some(7),
                ..env
            };
            menu.carried = Some(ItemStack::new("minecraft:stone", 3));
            for (slot, button) in [(-999, 0), (9, 1), (10, 1), (-999, 2)] {
                menu.click(&env, slot, button, Input::QuickCraft).unwrap();
            }
            assert_eq!(menu.slot(9).unwrap().count, 1);
            assert_eq!(menu.slot(10).unwrap().count, 1);
            assert_eq!(menu.carried.as_ref().unwrap().count, 1);
            assert_eq!(parts.asks.get(), 0);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn occupied_equipment_cursor_and_hotbar_swaps_ask_once_per_click() {
        for input in [Input::Pickup, Input::Swap] {
            super::super::tests::with_menu(Kind::Inventory, |env, menu| {
                let parts = FeetOnly::default();
                let env = Env {
                    can_equip: Some(&parts),
                    player_entity_id: Some(7),
                    ..*env
                };
                menu.set_slot(&env, 8, Some(ItemStack::new("minecraft:diamond_boots", 1)));
                if input == Input::Pickup {
                    menu.carried = Some(ItemStack::new("minecraft:iron_helmet", 1));
                } else {
                    menu.set_slot(&env, 36, Some(ItemStack::new("minecraft:iron_helmet", 1)));
                }
                menu.click(&env, 8, 0, input).unwrap();
                assert_eq!(menu.slot(8).unwrap().item.as_ref(), "minecraft:iron_helmet");
                let returned = if input == Input::Pickup {
                    menu.carried.as_ref()
                } else {
                    menu.slot(36)
                };
                assert_eq!(returned.unwrap().item.as_ref(), "minecraft:diamond_boots");
                assert_eq!(parts.asks.get(), 1);
            });
        }
    }
}
