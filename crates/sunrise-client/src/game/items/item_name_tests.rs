//! `ItemPart::item_name` (`Item.getName(ItemStack)`) behind every place the game names a stack.
use super::native_entity_refs::tests::{run_fresh_cases, with_game};
use mod_api::{Hooks, ItemNameEvent, ItemPart, LoopContext, Registrar, RegistrationContext};
use sunrise_client::items::ItemStack;
use sunrise_core::nbt::Tag;

const NAME: &str = "game::item_name_tests::a_part_names_its_stacks_everywhere_the_game_names_them";
const KEY: &str = "item.test.bundle_of_notes";

/// A paper's Part: a stack of more than one is named by `KEY`, one is left to the default.
#[derive(Clone)]
struct Notes;

#[mod_api::__subscriptions]
impl Hooks for Notes {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let key = r.item_key("minecraft:paper");
        r.item_part(key, self.clone());
    }
}
#[mod_api::sunrise_part]
impl ItemPart for Notes {
    fn item_name(&self, _: &dyn LoopContext, event: &mut ItemNameEvent<'_>) {
        if event.stack.count() > 1 {
            *event.name = Some(mod_api::Text::translate(KEY, Vec::new()));
        }
    }
}

/// The Part's name wins over the item's own for its stacks, `item_name` component included, in the
/// hover name, the styled tooltip name and the anvil; a stack it leaves keeps the default, its
/// `item_name` included; a custom name wins over it; another item isn't asked. The anvil keeps an
/// unchanged Part name and renames to the item's base name. In its own process: the game it opens
/// publishes process-wide tables.
#[test]
#[ignore = "daily"]
fn a_part_names_its_stacks_everywhere_the_game_names_them() {
    if run_fresh_cases(NAME, "SUNRISE_ITEM_NAME_CHILD", &["names"]) {
        return;
    }
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new("names", Box::new(Notes))].into();
    mod_platform::testing::with_mods(mods.clone(), || {
        crate::game::api_values::install_parts(&mods).unwrap();
        crate::entry::install_client_hooks();
        with_game(|game| {
            game.resolve_parts().unwrap();
            let plain = |t: sunrise_client::text::Text| t.plain();
            // An untranslated key reads as itself: the Part's name, independent of the code.
            let named = ItemStack::new("minecraft:paper", 2);
            assert_eq!(plain(game.stack_hover_name_text(&named)), KEY);
            assert_eq!(plain(game.styled_stack_name(&named)), KEY);

            // The Part replaces an explicit `item_name`; a stack it leaves keeps it.
            let mut labelled = named.clone();
            labelled
                .data_mut()
                .set_other("minecraft:item_name", Some(Tag::String("Label".into())));
            assert_eq!(plain(game.stack_hover_name_text(&labelled)), KEY);
            let mut single = labelled.clone();
            single.count = 1;
            assert_eq!(plain(game.stack_hover_name_text(&single)), "Label");
            assert_eq!(game.mod_item_name(&single), None);

            // A plain single sheet and another item: vanilla's name, no Part asked.
            let sheet = ItemStack::new("minecraft:paper", 1);
            assert_eq!(
                game.stack_hover_name_text(&sheet).component(),
                sunrise_client::stations::hover_name_text(&game.pack.names, &sheet).component()
            );
            assert_eq!(
                game.mod_item_name(&ItemStack::new("minecraft:stone", 2)),
                None
            );

            // `ItemStack.getHoverName`: a custom name wins over the Part's.
            let mut renamed = named.clone();
            renamed.data_mut().custom_name = Some(Tag::String("Diary".into()));
            assert_eq!(plain(game.stack_hover_name_text(&renamed)), "Diary");

            // The anvil (`AnvilMenu.createResult`): its box holds the Part's name, which leaves
            // the stack as it is; the item's base name typed in is a rename.
            let hover = plain(game.stack_hover_name_text(&named));
            let anvil = |name: &str| {
                sunrise_client::stations::anvil_result(
                    &game.stations().enchantments,
                    game.items(),
                    Some(&named),
                    None,
                    Some(name),
                    &hover,
                    true,
                )
            };
            assert!(
                anvil(KEY).result.is_none(),
                "an unchanged name renames nothing"
            );
            let renamed = anvil("Paper").result.expect("the base name is a rename");
            assert_eq!(renamed.custom_name().as_deref(), Some("Paper"));
        });
    });
}
