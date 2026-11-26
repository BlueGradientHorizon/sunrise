use crate::game::ServerGame;
use sunrise_gameplay_common::game::lan_menu_data::GuestStation;
use sunrise_gameplay_common::game::stations_common::Kind as StationKind;
use sunrise_level::containers::BlockEntity;

impl ServerGame<'_> {
    /// `LecternMenu.stillValid` (`Container.stillValidBlockEntity`): the block
    /// entity still holds a book and the guest is within the interaction
    /// range and its buffer.
    pub(in crate::game) fn lan_lectern_valid(
        &self,
        g: &sunrise_sim::lan::guest::Guest,
        key: (i32, i32, i32),
    ) -> bool {
        matches!(
            self.world.block_entities.get(&key),
            Some(BlockEntity::Lectern(l)) if l.book.is_some()
        ) && g.within_block_range([key.0, key.1, key.2], 4.0)
    }
    /// `AbstractContainerMenu.removed` of the guest's station (`window`;
    /// None closes it whichever it is): the carried stack and the inputs go
    /// back to the inventory, what doesn't fit is dropped. False when no such
    /// window is open.
    pub(in crate::game) fn lan_station_closed(&mut self, conn: u32, window: Option<i32>) -> bool {
        self.lan_station_removed(conn, window, super::lan_crafting::MenuRemoval::Live)
    }
    /// How many slots of its own the guest's open station menu has before the
    /// player's inventory (`slot_count - 36`), if one is open.
    pub(in crate::game::server_side) fn lan_station_own_slots(&self, conn: u32) -> Option<usize> {
        let gs = self.server.lan.as_ref()?.menus.get(&conn)?.station.as_ref()?;
        Some(gs.menu.slot_count() - 36)
    }
    /// A station menu's `stillValid`: its block is still that station and the
    /// guest is within `isWithinBlockInteractionRange(pos, 4)`.
    pub fn lan_station_valid(
        &self,
        g: &sunrise_sim::lan::guest::Guest,
        gs: &GuestStation,
    ) -> bool {
        let [x, y, z] = gs.pos;
        let name = &self.common.res.facts().get(self.common.world.block(x, y, z)).name;
        StationKind::from_block(name) == Some(gs.kind) && g.within_block_range(gs.pos, 4.0)
    }
}
