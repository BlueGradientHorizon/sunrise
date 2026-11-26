//! Dual-role menu handle reads shared across server and client roles.
use mod_api::{MenuRef, PlayerRef};
use sunrise_items::items::ItemStack;

pub trait NativeMenuRefsCommonGameExt1<'__sg>: crate::game::SideGame<'__sg> {
    /// A player's current supported menu.
     fn player_container_menu(&self, player: PlayerRef) -> Option<MenuRef> {
        self.side_player_container_menu(player)
    }

    /// Registry menu type (`AbstractContainerMenu.getType`); outer None means unavailable.
     fn menu_kind(&self, menu: MenuRef) -> Option<Option<String>> {
        self.side_menu_kind(menu)
    }

    /// Number of slots in `AbstractContainerMenu.slots`.
     fn menu_slot_count(&self, menu: MenuRef) -> Option<usize> {
        self.side_menu_slot_count(menu)
    }

    /// Carried (cursor) stack; outer None means unavailable.
     fn menu_carried(&self, menu: MenuRef) -> Option<Option<ItemStack>> {
        self.side_menu_carried(menu)
    }

    /// Full stack at a menu index; outer None means unavailable or out of range.
     fn menu_slot(
        &self,
        menu: MenuRef,
        slot: usize,
    ) -> Option<Option<ItemStack>> {
        self.side_menu_slot(menu, slot)
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> NativeMenuRefsCommonGameExt1<'__sg> for G {}


use crate::game::side_ops::modapi_modding::ModapiModdingSideGameExt as _;
