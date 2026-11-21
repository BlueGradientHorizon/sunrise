//! The `item_name` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Vanilla's `Item.getName(ItemStack)`: the name of `stack` from its
    /// components, so one mod item has many names (a data model named after
    /// the mob in it, a filled container after its contents). `name` arrives
    /// as `None`, which keeps the default: the stack's `minecraft:item_name`
    /// component, else the item's translated name. Set it to `Some(text)` to
    /// name the stack; a translatable `Text` renders in each viewer's own
    /// language. It replaces the default rather than filling in for a missing
    /// `item_name`, as the vanilla items that override `getName` do. A
    /// `minecraft:custom_name` still wins over it (`ItemStack.getHoverName`).
    /// Read wherever the game names a stack: tooltips, the selected item's
    /// name above the hotbar, command feedback such as `/give`'s, the anvil,
    /// stats, recipe screens and `Host::hover_name`. Asked often, on both
    /// sides: answer from the stack alone, cheaply. Only the `ItemPart`s
    /// attached to `stack`'s item are asked; the last to set `name` wins, and
    /// an item with none costs one table lookup.
    #[hook(
        side = "both",
        contract = "collect",
        out = "name",
        scope = "item",
        key = "stack",
        host = "read",
        neoforge = "Item.getName"
    )]
    fn item_name();
}
