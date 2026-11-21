#![cfg(test)]
// Item model callbacks carry owned values and a separate runtime selection.
use crate::*;

struct NoClient;
impl Host for NoClient {}
crate::impl_host_areas!(NoClient);
impl ClientHost for NoClient {}
crate::impl_client_host_areas!(NoClient);

struct Selected;
#[crate::__subscriptions]
impl Hooks for Selected {
    fn item_model_property(&self, _: &dyn ClientHost, event: &mut ItemModelPropertyEvent<'_>) {
        event.value = event.stack.count() == 2 && event.bundle_selected == 3;
    }
    fn register_range_select_item_model_property(
        &self,
        _: &RegistrationContext,
        event: &mut RegisterRangeSelectItemModelPropertyEvent<'_>,
    ) {
        event.register(Identifier::parse("example:selected").unwrap(), |_| {
            Ok(|_, input| input.bundle_selected as f32 / input.stack.count() as f32)
        });
    }
    fn register_select_item_model_property(
        &self,
        _: &RegistrationContext,
        event: &mut RegisterSelectItemModelPropertyEvent<'_>,
    ) {
        event.register(Identifier::parse("example:selected").unwrap(), |_| {
            Ok(|_, input| {
                (input.bundle_selected >= 0)
                    .then(|| format!("{}:{}", input.bundle_selected, input.stack.count()))
            })
        });
    }
    fn register_item_tint_sources(
        &self,
        _: &RegistrationContext,
        event: &mut RegisterItemTintSourcesEvent<'_>,
    ) {
        event.register(Identifier::parse("example:selected").unwrap(), |_| {
            Ok(|_, input| {
                if input.bundle_selected < 0 {
                    u32::MAX
                } else {
                    0xff00_0000 | ((input.bundle_selected as u32) << 16) | input.stack.count()
                }
            })
        });
    }
}

#[test]
fn model_callbacks_receive_owned_counts_and_the_exact_runtime_selection() {
    use crate::__private::game::RegistrationContextNew;
    install_parts(&[], &crate::__private::part_tests::rule_schema(), |_| false).unwrap();
    let stack = crate::__private::part_tests::stack("minecraft:stick", 2, 0);
    let mods = vec![ModEntry::new("selected", Box::new(Selected))].into();
    testing::with_mods(mods, || {
        let mut range_props = Vec::new();
        dispatch_register_range_select_item_model_property(
            &RegistrationContext::new(),
            &mut range_props,
        );
        let mut select_props = Vec::new();
        dispatch_register_select_item_model_property(
            &RegistrationContext::new(),
            &mut select_props,
        );
        let mut tint_sources = Vec::new();
        dispatch_register_item_tint_sources(&RegistrationContext::new(), &mut tint_sources);
        let config = crate::Tag::compound([] as [(&str, crate::Tag); 0]);
        let range_eval = (range_props[0].answer.decode)(&config).unwrap();
        let select_eval = (select_props[0].answer.decode)(&config).unwrap();
        let tint_eval = (tint_sources[0].answer.decode)(&config).unwrap();
        let context = ItemModelContext {
            display: ItemDisplayContext::Gui,
            owner: None,
            level: None,
            seed: 0,
        };
        for (selected, boolean, number, text, tint) in [
            (3, true, 1.5, Some("3:2"), 0xff03_0002),
            (-1, false, -0.5, None, u32::MAX),
        ] {
            let mut value = false;
            dispatch_item_model_property(
                &NoClient,
                "example:selected",
                &stack,
                selected,
                &mut value,
            );
            assert_eq!(value, boolean);
            let prop_input = ItemModelPropertyInput {
                stack: &stack,
                bundle_selected: selected,
                context: &context,
            };
            assert_eq!(range_eval(&NoClient, &prop_input), number);
            assert_eq!(select_eval(&NoClient, &prop_input).as_deref(), text);
            let tint_input = ItemTintInput {
                stack: &stack,
                bundle_selected: selected,
            };
            assert_eq!(tint_eval(&NoClient, &tint_input), tint);
        }
    });
}

#[cfg(test)]
use super::*;

#[cfg(test)]
use crate::testing;
