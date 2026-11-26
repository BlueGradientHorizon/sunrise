use crate::game::api_values::{StackReadMetadata, project_stack, stack_read_metadata};
use mod_api::client::first_person::Hand;
use mod_api::{EquipmentSlot, ItemStackView};
use sunrise_sim::mobs::finalize::EquipmentRead;

/// Lend one equipment slot to the API, applying overlays and encoding components only on demand.
pub fn lend_equipment(
    stack: EquipmentRead<'_>,
    read: &mut dyn for<'a> FnMut(ItemStackView<'a>),
) -> Option<()> {
    let metadata = stack_read_metadata(stack.base()).ok()?;
    with_equipment_view(&stack, metadata, read);
    Some(())
}

/// Use checked slot metadata to lend the same lazy view to readers and component editors.
pub(in crate::game) fn with_equipment_view<R>(
    stack: &EquipmentRead<'_>,
    metadata: StackReadMetadata,
    read: impl for<'a> FnOnce(ItemStackView<'a>) -> R,
) -> R {
    let project = |metadata| project_stack(&stack.project(), metadata).ok();
    read(ItemStackView::resident(metadata, &project))
}

/// The equipment slot of a hand (`LivingEntity.getItemInHand`/`setItemInHand`).
pub fn hand_slot(hand: Hand) -> EquipmentSlot {
    match hand {
        Hand::Main => EquipmentSlot::MainHand,
        Hand::Off => EquipmentSlot::OffHand,
    }
}
