

#[cfg(any(test, feature = "test-support"))]
pub mod tests {

use super::*;

use sunrise_items::items::ItemStack;

use mod_api::{AttributeModifier, AttributeOperation, Identifier, ModifierLifetime};

pub fn stack(item: &str) -> Option<ItemStack> {
        Some(ItemStack::new(item, 1))
    }
pub fn modifier(id: &str, amount: f64, operation: AttributeOperation) -> AttributeModifier {
        AttributeModifier::new(Identifier::parse(id).unwrap(), amount, operation)
    }
/// The ids of the modifiers a saved `attributes` list holds.
    pub fn saved_ids<'a>(list: impl IntoIterator<Item = &'a sunrise_core::nbt::Tag>) -> Vec<String> {
        use sunrise_core::nbt::Tag;
        use sunrise_game_data::nbt_value::text;
        list.into_iter()
            .flat_map(|entry| match entry.get("modifiers") {
                Some(Tag::List(l)) => l.iter().filter_map(|m| text(m.get("id"))).collect(),
                _ => Vec::new(),
            })
            .collect()
    }
#[cfg(test)]
/// Plates every chestplate with +0.4 knockback resistance and no armor
    /// toughness (`item_attribute_modifier`).
    struct Plated;
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for Plated {
        fn item_attribute_modifier(&self, event: &mut mod_api::ItemAttributeModifierEvent<'_>) {
            if !mod_api::__private::item_key_id(event.stack.item())
                .unwrap()
                .ends_with("_chestplate")
            {
                return;
            }
            let id = |text: &str| Identifier::parse(text).unwrap();
            event.modifiers.add(
                id("minecraft:knockback_resistance"),
                modifier("test:plated", 0.4, AttributeOperation::AddValue),
                mod_api::EquipmentSlotGroup::Chest,
            );
            event
                .modifiers
                .remove_all_for(&id("minecraft:armor_toughness"));
        }
    }
}
