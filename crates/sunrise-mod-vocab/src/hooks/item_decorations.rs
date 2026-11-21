//! The `item_decorations` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0100

pub trait Hooks {
    /// Append an item's custom decorations after its default bar, cooldown and count.
    /// `draw` is the borrowed `GuiCanvas`; `(x, y)` is the icon's top-left in its local
    /// GUI units. The canvas includes the caller's decoration pose and scissor, not the
    /// model-only squeeze. Inventory, hotbar and tooltip draws lend an identity canvas.
    /// Appended draws belong to the enclosing group, if any; independent host calls do not.
    /// The NeoForge counterpart is `IItemDecorator.render`; registration through
    /// `RegisterItemDecorationsEvent` is a separate moment.
    #[hook(
        side = "client",
        contract = "listen",
        scope = "item",
        key = "stack",
        neoforge = "IItemDecorator.render",
        version = 2
    )]
    fn item_decorations();
}
