//! Read-only equipment host: cached target facts, ordinary game reads elsewhere.
use super::Game;
use mod_api::__private::game::BackendRefOf;
use mod_api::{Backend, BackendRef, EquipReadRow, EquipmentSlot, Host, ItemStackView};
pub(in crate::game) struct EquipHost<'g, 'a> {
    pub(in crate::game) game: &'g Game<'a>,
    pub(in crate::game) row: &'g EquipReadRow,
}
impl EquipHost<'_, '_> {
    pub(in crate::game) fn with_equipment(
        &self,
        slot: EquipmentSlot,
        read: &mut dyn for<'a> FnMut(ItemStackView<'a>),
    ) -> Option<()> {
        read(ItemStackView::from_stack(
            self.row.equipment[slot as usize].as_ref()?,
        ));
        Some(())
    }
}
impl Host for EquipHost<'_, '_> {
    fn backend(&self) -> Option<BackendRef<'_>> {
        Some(BackendRef::of(self))
    }
}
impl Backend for EquipHost<'_, '_> {}
