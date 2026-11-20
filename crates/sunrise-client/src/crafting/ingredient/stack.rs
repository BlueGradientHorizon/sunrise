//! Allocation-free projection of a live engine stack into an ingredient predicate.
use mod_api::{__private, IngredientStack};
use sunrise_items::{
    item_data::ingredient_components::removed,
    items::{ItemInfo, ItemStack},
};

#[allow(
    dead_code,
    reason = "the recipe matcher switches to bound predicates in the next step"
)]
pub(super) fn with_ingredient_stack<R>(
    stack: &ItemStack,
    info: Option<&ItemInfo>,
    read: impl FnOnce(IngredientStack<'_>) -> R,
) -> R {
    let damage = stack
        .carries_component(info, "minecraft:damage")
        .then(|| stack.effective_damage(info) as u32);
    read(__private::ingredient_stack(
        &stack.item,
        __private::item_key(&stack.item),
        stack.count,
        damage,
        &|id| !id.starts_with('!') && stack.carries_component(info, id),
        &|id| removed(stack, id),
        &|id| {
            if id.starts_with('!') {
                return None;
            }
            stack.data().and_then(|data| data.saved_other(id))
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sunrise_core::nbt::Tag;
    use sunrise_items::item_data::decode_components;

    #[test]
    fn ingredient_stack_lends_live_component_data() {
        let mut stack = ItemStack::new("fixture:ingredient_stack", 3);
        with_ingredient_stack(&stack, None, |view| assert_eq!(view.damage(), None));

        let patch = Tag::compound([
            ("minecraft:damage", Tag::Int(0)),
            (
                "!minecraft:max_stack_size",
                Tag::Compound(Default::default()),
            ),
            (
                "minecraft:custom_data",
                Tag::compound([("grade", Tag::Int(7))]),
            ),
        ]);
        (stack.damage, stack.extra) = decode_components(Some(&patch));
        let stored = stack
            .data()
            .unwrap()
            .saved_other("minecraft:custom_data")
            .unwrap();

        with_ingredient_stack(&stack, None, |view| {
            assert_eq!(view.item_id(), "fixture:ingredient_stack");
            assert_eq!(view.item_key(), None);
            assert_eq!(view.count(), 3);
            assert_eq!(view.damage(), Some(0));
            assert!(view.has_component("minecraft:damage"));
            assert_eq!(view.stored_component("minecraft:damage"), None);
            assert!(!view.has_component("minecraft:max_stack_size"));
            assert!(view.component_removed("minecraft:max_stack_size"));
            assert!(!view.component_removed("minecraft:unbreakable"));
            assert_eq!(view.stored_component("minecraft:max_stack_size"), None);
            assert!(view.has_component("minecraft:custom_data"));
            let lent = view.stored_component("minecraft:custom_data").unwrap();
            assert_eq!(lent.get("grade"), Some(&Tag::Int(7)));
            assert!(std::ptr::eq(lent, stored));
        });
    }
}
