//! `ItemPart::item_name` (`Item.getName(ItemStack)`) behind every place the game names a stack.
use super::native_entity_refs::tests::with_game;
use mod_api::{Hooks, ItemNameEvent, ItemPart, Registrar, RegistrationContext};
use sunrise_client::items::ItemStack;

/// A paper's Part: a stack of more than one is named "Bundle of Notes", one is left to the default.
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
    fn item_name(&self, _: &dyn mod_api::Host, event: &mut ItemNameEvent<'_>) {
        if event.stack.count() > 1 {
            *event.name = Some(mod_api::Text::translate(
                "item.test.bundle_of_notes",
                Vec::new(),
            ));
        }
    }
}

/// The Part's name wins over the item's own for its stacks, in the hover name, the styled name a
/// tooltip shows, the plain name the anvil compares and the menus' name; a custom name still wins
/// over it; a stack it leaves keeps the default; another item isn't asked.
#[test]
#[ignore = "daily"]
fn a_part_names_its_stacks_everywhere_the_game_names_them() {
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new("names", Box::new(Notes))].into();
    mod_platform::testing::with_mods(mods.clone(), || {
        crate::game::api_values::install_parts(&mods).unwrap();
        with_game(|game| {
            let named = ItemStack::new("minecraft:paper", 2);
            let key = |t: sunrise_client::text::Text| t.component().to_string();
            assert!(key(game.stack_hover_name_text(&named)).contains("item.test.bundle_of_notes"));
            assert!(key(game.styled_stack_name(&named)).contains("item.test.bundle_of_notes"));
            assert!(key(game.mod_item_name(&named).unwrap()).contains("item.test.bundle_of_notes"));
            let env = game.menu_env();
            assert_eq!(
                env.hover_name(&named),
                game.stack_hover_name_text(&named).plain()
            );

            // A stack the Part leaves, and another item: the default name, no Part asked.
            let single = ItemStack::new("minecraft:paper", 1);
            assert_eq!(game.mod_item_name(&single), None);
            assert_eq!(
                game.stack_hover_name_text(&single).component(),
                sunrise_client::stations::hover_name_text(&game.pack.names, &single).component()
            );
            assert_eq!(
                game.mod_item_name(&ItemStack::new("minecraft:stone", 2)),
                None
            );

            // `ItemStack.getHoverName`: a custom name wins over the Part's.
            let mut renamed = named.clone();
            renamed.data_mut().custom_name = Some(sunrise_core::nbt::Tag::String("Diary".into()));
            assert_eq!(game.stack_hover_name_text(&renamed).plain(), "Diary");
            assert_eq!(env.hover_name(&renamed), "Diary");
        });
    });
}
