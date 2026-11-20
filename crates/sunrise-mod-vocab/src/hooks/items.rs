//! The `items` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0153

pub trait Hooks {
    /// Items this mod adds, declared in code (NeoForge's
    /// `DeferredRegister.Items`), as `blocks` declares blocks: pass a
    /// `mod_api::Item` per item to `event.register`, a content mod's
    /// `data/<id>/sunrise/item/<path>.json` (a vanilla `template`,
    /// `max_stack_size`, `rarity`, `components`, `crafting_remainder`). A
    /// block's item comes with its block (`item: true`), not from here.
    /// IDs must use a namespace owned by the mod's manifest (bare paths use its
    /// mod ID). Invalid IDs and unowned namespaces return a typed error and refuse
    /// startup, even if the caller ignores the error.
    /// Asked once at start, on every side.
    #[hook(
        side = "both",
        contract = "collect",
        out = "items",
        context = "registration",
        neoforge = "RegisterEvent",
        version = 2
    )]
    fn items();
}
