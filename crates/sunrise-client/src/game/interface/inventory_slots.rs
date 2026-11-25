//! Container screen slots: item and decoration drawing, the carried stack and quick craft drag,
//! slot positions and hit tests, highlights, and the crafting result slot.
use super::*;

/// Disjoint internal slot base for mod menu backing slots (`ModMenuType`), well above
/// vanilla container (`300..500`), armor/offhand (`500..=504`), station (`600..`), and
/// mount (`700..`) slot ranges so large mod menus never alias unrelated inventory stores.
pub(in crate::game) const MOD_MENU_SLOT_BASE: usize = 100_000;

/// Decode a container backing index from either a vanilla container slot (`300..500`)
/// or a mod menu backing slot (`MOD_MENU_SLOT_BASE..`).
pub(in crate::game) fn container_slot_index(slot: usize) -> Option<usize> {
    if slot >= MOD_MENU_SLOT_BASE {
        Some(slot - MOD_MENU_SLOT_BASE)
    } else if (300..500).contains(&slot) {
        Some(slot - 300)
    } else {
        None
    }
}

#[cfg(test)]
thread_local! { pub(in crate::game) static SLOT_POSITIONS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// The stock hopper/dispenser slot layout, unless a mod screen is shown for
    /// that menu: a mod screen places its own container slots
    /// (`native_screen_slot`) and the player's inventory (`native_screen_inventory_y`),
    /// and this layout answers first in `slot_position`, which kept a mod's screen
    /// on a hopper or a dispenser on the stock grid (#5141). The rest of those
    /// menus is already the mod's: `draw_native_screen` owns the background and
    /// `screen_height` its height.
    pub(in crate::game) fn stock_machine_slot(
        stock: Option<(f32, f32)>,
        mod_screen_shown: bool,
    ) -> Option<(f32, f32)> {
        if mod_screen_shown { None } else { stock }
    }
}

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// Slot ids: 0..36 player inventory, 100..109 crafting grid, 200 result.
    pub(in crate::game) fn container_kind(&self) -> Option<&BlockEntity> {
        self.container
            .and_then(|k| self.world.block_entities.get(&k))
    }
}
