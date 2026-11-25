//! Clicks on an open container menu: `AbstractContainerMenu.clicked` run by the menu engine
//! (`menu_session`, `click_open_inventory`, `menu_ops`), a shift-click's quick move, the creative screen's own
//! display and throw clicks, and a mouse button over the inventory (`click_inventory`).
#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    /// `quickMoveStack` of the open plain menu for a shift-click.
    #[allow(dead_code)]
    pub(in crate::game) fn inventory_quick_move(&mut self, slot: usize) -> bool {
        self.click_open_inventory(Some(slot), 0, sunrise_client::menu::Input::QuickMove)
    }
    /// The open menu as the verified engine (`sunrise_client::menu`): the
    /// game's slots are copied in, `run` acts on it, and the slots, carried
    /// stack, menu data and drops are copied back when it returns Some(true)
    /// (Some(false): handled, nothing kept; None: not handled). False when the
    /// open screen is not one the engine models or `run` did not handle it.
    pub(in crate::game) fn menu_session(
        &mut self,
        run: impl FnOnce(
            &mut sunrise_client::menu::Menu,
            &sunrise_client::menu::Env,
            &dyn Fn(usize) -> Option<usize>,
        ) -> Option<bool>,
    ) -> bool {
        self.menu_session_with_game(|_, menu, env, to_menu| run(menu, env, to_menu))
    }

    /// `CrafterBlockEntity.setSlotState`: the menu's slot states land on the
    /// block entity, whose `setChanged` marks the chunk for saving.
    pub(in crate::game) fn apply_crafter_slot_states(&mut self, menu: &sunrise_client::menu::Menu) {
        let Some(key) = self.container else {
            return;
        };
        let Some(BlockEntity::Crafter(c)) = self.world.block_entities.get_mut(&key) else {
            return;
        };
        let mut changed = false;
        for (i, d) in c.disabled.iter_mut().enumerate() {
            let disabled = menu.data[i] == 1;
            changed |= *d != disabled;
            *d = disabled;
        }
        if changed {
            self.side_container_changed(key);
        }
    }

    /// A sound a menu click made the player play (a bundle's insert or
    /// remove): `Entity.playSound` at the player.
    pub(in crate::game) fn player_menu_sound(&mut self, sound: &str) {
        let Some(p) = self.player.as_ref() else {
            return;
        };
        let at = [p.position.x, p.position.y, p.position.z];
        let (volume, pitch) = if sound == sunrise_client::bundle::INSERT_FAIL {
            (1.0, 1.0)
        } else {
            (0.8, 0.8 + self.world.random.next_float() * 0.4)
        };
        if self.is_logical_server() {
            self.server_play_sound(sound, at, volume, pitch);
        } else {
            self.play_client_player_sound(sound, volume, pitch);
        }
    }

    pub(in crate::game) fn creative_throw_carrying(&mut self, slot: usize, button: i32) {
        let Some(old) = self
            .slot_mut(slot)
            .as_ref()
            .filter(|s| !s.is_empty())
            .cloned()
        else {
            return;
        };
        let count = if button == 0 {
            1
        } else if self.creative_inventory_open() {
            let max = world_items!(self)
                .get(&old.item)
                .map_or(64, |i| i.max_stack);
            old.count.min(max)
        } else {
            old.count
        };
        let left = old.count - count;
        *self.slot_mut(slot) = (left > 0).then(|| ItemStack {
            count: left,
            ..old.clone()
        });
        self.apply_equipment();
        self.creative_screen_drop(ItemStack { count, ..old });
        self.send_creative_slots();
    }
}
