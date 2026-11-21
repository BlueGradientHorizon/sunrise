//! The `anvil_craft_post` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0237

pub trait Hooks {
    /// A player took an anvil's result at `pos` and the levels, inputs and anvil wear are done (`AnvilCraftEvent.Post`); a broken anvil is already air. `craft` holds the inputs and the result as they were before. Not called when `anvil_craft_pre` cancelled.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_anvil_craft_post",
        dispatch = "flat",
        side = "server",
        contract = "listen",
        neoforge = "AnvilCraftEvent.Post"
    )]
    fn anvil_craft_post(
        player: u128,
        name: &str,
        pos: [i32; 3],
        craft: &mod_api::item::menu::AnvilCraft,
    );
}
