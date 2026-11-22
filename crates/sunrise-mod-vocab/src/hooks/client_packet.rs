//! The `client_packet` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// A play packet from the server reached the window's game, decoded,
    /// just before the game handles it (`Connection.genericsFtw` handing it
    /// to `ClientPacketListener`): every packet of the play phase, custom
    /// payloads included, in the order the server sent them. Packets the
    /// network thread handles itself are not heard: keep-alives, chunk batch
    /// starts and finishes, bundle delimiters, resource pack pushes and pops,
    /// cookies, server links, transfers, disconnects, the configuration switch,
    /// `minecraft:brand` and far terrain tiles; neither is the configuration
    /// phase.
    /// Read-only: the game handles the packet as it would without the mod.
    /// Client-side, on any server, single player's own included. With no
    /// listener, the receive path checks one flag and builds no view.
    /// Every packet exposes its protocol type id (`&Identifier`). Custom payloads also expose
    /// their channel (`&str`) and raw bytes (`&[u8]`); no other decoded fields or mutable
    /// bytes cross the API.
    /// The immutable `ClientPacket<'_>` view is valid only for the callback; copy what you
    /// store (`id.clone()`, `channel.to_owned()`, `data.to_vec()`). A panic is logged and
    /// shown in an error toast naming the mod, disables only that mod's `client_packet`
    /// callback for the session, and does not stop later listeners. Host effects are not
    /// rolled back. A phase refusal is logged each time without disabling the callback.
    #[notes = r#"## Decoded client packets

`Hooks::client_packet` receives `&mut dyn ClientHost` and a `ClientPacketEvent<'_>`
before the window handles a decoded server play packet. `event.packet` borrows
an immutable callback-lifetime `mod_api::ClientPacket<'a>`: `id: &'a Identifier`
identifies the packet type (for example, `minecraft:custom_payload`); `custom_payload`
borrows `Option<ClientPayload<'a>>` (`channel: &'a str`, `data: &'a [u8]`) for a custom
payload, or `None` otherwise. An id outside the protocol's packet table is reported as
`sunrise:unknown`. Other decoded fields and mutable packet bytes are not exposed. Copy
only what you retain after the callback returns (`id.clone()`, `channel.to_owned()`,
`data.to_vec()`). With no listener, the receive path checks one flag and builds nothing.

The hook cannot cancel or replace a packet. Each listener sees the original
borrowed packet; reassigning `event.packet` affects only that listener's local event.
If a listener panics, the error is logged and shown in a toast naming the mod, and only
that mod's callback for this hook is disabled for the session. Later listeners and the
normal packet handler still run, and effects already made on the Host are not rolled
back. A phase refusal is logged each time without disabling the callback. The hook also
observes chunk packets handled by the batching path, once each, in server order.
Configuration packets and the network thread's own handling (keep-alives, chunk batch
starts and finishes, bundle delimiters, resource pack pushes and pops, cookies, server
links, transfers, disconnects, the configuration switch, `minecraft:brand`, and far
terrain tiles) do not fire it. The chunks between a batch's start and finish still fire it.

`ClientHost::hold_packets` retains uncancelled play packets, unmodified and in order, up to 256
packets or 1,048,576 encoded bytes. Release sends oldest first; drop discards them. Overflow ends
and flushes the hold; its next release/drop returns `HoldError::Limit` with those configured bounds.
A failed release reports `HoldError::Send { sent, error }` and discards the unsent tail.
Starting another hold while one is active returns `HoldError::Limit` with the configured bounds.
Disconnect, protocol incarnation changes
and aborted travel invalidate holds. Network-thread keep-alives, chunk batch acknowledgements,
configuration, cookies and pack answers bypass the queue. Held packets expose no additional inspection or editing API.

`Hooks::client_packet_send` is the other direction: a `ClientPacketSendEvent<'_>` with the same
immutable callback-lifetime `ClientPacket<'a>` view for each play packet the window's game is
about to send (`Connection.send`), in order, including the selected slot sent before an attack
or a use. Returning `Flow::Cancel` drops the packet, and the game goes on as if it had been
sent, unless a later `#[listener(receive_cancelled)]` listener sets `event.cancelled = false`
and returns `Flow::Continue`. Play-phase pongs pass through this hook.
Packets the network thread sends itself (keep-alives, chunk batch acknowledgements, configuration
pongs, cookies, configuration), the phase-dependent answers (resource pack status, a dialog's
custom click action) and far terrain requests do not fire it. With no listener the send path
checks one flag and builds nothing. Copy `id.clone()`, `channel.to_owned()`, or `data.to_vec()`
to retain data past the call. The same per-callback panic policy applies: a panic disables only
that mod's send callback for the session, leaves the last accepted cancellation unchanged (even
if `event.cancelled` was written before panicking), does not roll back Host effects, and lets
later eligible listeners run; a phase refusal is logged each time without disabling the callback
or changing cancellation."#]
    #[hook(
        dispatch = "flat",
        side = "client",
        contract = "listen",
        cancellable = false,
        contain_panics = true,
        version = 3,
        neoforge = "none (NeoForge has no raw play-packet receive event)"
    )]
    fn client_packet(packet: &ClientPacket<'_>);
}
