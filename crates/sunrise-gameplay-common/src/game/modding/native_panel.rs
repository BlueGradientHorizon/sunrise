

#[cfg(any(test, feature = "test-support"))]
pub mod tests {

use super::{
        BrowserStack, BrowserTooltip, DRAG_THRESHOLD, Held, Now, Owner, Panel, RecipePage, Release,
        browser_overlay,
    };

use crate::game::native_hooks::{NativeRecipeLayout, NativeRecipeResult, NativeRecipeView};

use mod_api::FluidStack;

pub fn keys() {
        mod_platform::__private::part_tests::install_parts_fixture(&[]).unwrap();
        mod_api::__private::resolve_items([
            "minecraft:oak_log",
            "minecraft:oak_planks",
            "minecraft:stone",
            "minecraft:dirt",
            "minecraft:crafting_table",
            "recipe_pilot:pressed_fragment",
        ])
        .unwrap();
        mod_api::__private::resolve_fluids(
            &["minecraft:water", "minecraft:lava"],
            &sunrise_world::fluid::FluidKind::ALL.map(|kind| kind.name()),
        )
        .unwrap();
    }
pub fn stack(item: &str) -> BrowserStack {
        BrowserStack {
            item: mod_api::__private::item_key(item).unwrap(),
            count: 1,
            components: String::new(),
        }
    }
fn fluid(id: &str, amount: u32) -> FluidStack {
        FluidStack::new(mod_api::__private::fluid_key(id).unwrap(), amount)
    }
pub fn view() -> NativeRecipeView {
        NativeRecipeView {
            layout: NativeRecipeLayout::Grid,
            width: 5,
            height: 1,
            slots: vec![vec![stack("minecraft:oak_log")]; 30],
            results: (0..6)
                .map(|i| NativeRecipeResult {
                    stacks: vec![stack("minecraft:oak_planks")],
                    chance: (i == 1).then_some(250.0),
                })
                .collect(),
            fluid_inputs: vec![fluid("minecraft:water", 1000); 10],
            fluid_outputs: vec![fluid("minecraft:lava", 250)],
            note: String::new(),
            action: Some(true),
        }
    }
/// The menu every `held` press goes down on.
    pub const MENU: u64 = 7;
/// Each `Search` the panel's mods heard: the mod it was for and the text.
    pub static HEARD: std::sync::Mutex<Vec<(String, String)>> = std::sync::Mutex::new(Vec::new());
#[cfg(test)]
struct Listener;
#[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for Listener {
        fn panel_event(
            &self,
            _: &mut dyn mod_api::ClientHost,
            event: &mut mod_api::PanelEventEvent<'_>,
        ) {
            if let mod_api::PanelEvent::Search(text) = event.event {
                let heard = (event.mod_id.to_owned(), text.clone());
                HEARD.lock().unwrap().push(heard);
            }
        }
    }
pub fn heard(mod_id: &str, text: &str) -> Vec<(String, String)> {
        vec![(mod_id.to_owned(), text.to_owned())]
    }
}
