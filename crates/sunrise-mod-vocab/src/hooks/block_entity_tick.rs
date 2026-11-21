//! The server block-entity ticker, in vanilla's `Level.tickBlockEntities` order.
// catalog-order: 0024

pub trait Hooks {
    /// A server tick of a resident mod block entity of your registered `kind`.
    /// `event.subject.with_view` lends its live storage. To reach the level or
    /// another object, return from the loan and call `event.subject.host()`.
    /// Ask for another loan afterwards; it returns `None` if the level, state
    /// or generation no longer matches. Keep a handle only to use across calls.
    ///
    /// Setters apply checked writes in place. Read-only and unchanged loans do not
    /// clone the row or dirty the chunk. Edits join the ambient transaction;
    /// nothing opens a per-listener transaction and there is no tick reply.
    /// Use `event.subject.transaction` when one operation must edit the subject
    /// and other storage atomically. Views don't notify neighbors or send a block
    /// update; normal menu synchronization reads the resident data slots.
    /// With no Part subscribed for this type, the game skips the tick dispatch.
    #[hook(
        receiver = "associated",
        dispatch = "flat",
        side = "server",
        contract = "listen",
        scope = "block_entity",
        key = "kind",
        neoforge = "none (vanilla BlockEntityTicker; NeoForge adds no event or extension for it)",
        version = 3
    )]
    fn block_entity_tick(kind: mod_api::BlockEntityTypeKey, subject: mod_api::BlockEntityScope<'_>);
}
