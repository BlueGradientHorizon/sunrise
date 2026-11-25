use super::*;

/// The hook points. Each does nothing in Sunrise.



/// Gather attributed numeric `minecraft:range_dispatch` item model property providers at
/// client startup, before resources load (NeoForge's
/// `RegisterRangeSelectItemModelPropertyEvent`).
pub(in crate::game) fn native_register_range_select_item_model_property(
    context: &mod_api::RegistrationContext,
    properties: &mut Vec<Answered<RangeSelectPropertyRegistration>>,
) {
    crate::game::native_hooks::fire::fire_register_range_select_item_model_property(context, properties);
}

