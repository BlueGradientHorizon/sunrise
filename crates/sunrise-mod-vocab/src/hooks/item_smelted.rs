//! The `item_smelted` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// `FurnaceResultSlot.checkTakeAchievements` ran for a server player at a
    /// furnace, blast furnace or smoker's result slot with a nonzero
    /// `removeCount` (NeoForge `PlayerEvent.ItemSmeltedEvent`): `player` is
    /// the live player handle, `stack` lends the stack vanilla passed there
    /// (`getSmelting`), and `amount_removed` is `removeCount`
    /// (`getAmountRemoved`). Fires after the furnace awards its used recipes
    /// and pops experience (`AbstractFurnaceBlockEntity.awardUsedRecipesAndPopExperience`).
    /// A pickup or throw passes the taken stack and its count; a shift-click
    /// fires once with the pre-move stack and the count moved (`onQuickCraft`);
    /// a hotbar swap has `removeCount == 0` so it awards experience without
    /// firing `item_smelted`. Server-side.
    #[hook(
        dispatch = "api_carrier",
        side = "server",
        contract = "listen",
        neoforge = "PlayerEvent.ItemSmeltedEvent",
        cancellable = false,
        version = 2
    )]
    fn item_smelted(player: PlayerRef, stack: ItemStackView<'_>, amount_removed: u32);
}
