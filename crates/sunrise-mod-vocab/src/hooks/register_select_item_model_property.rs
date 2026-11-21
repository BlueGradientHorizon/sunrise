//! The `register_select_item_model_property` hooks: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Register the categorical `minecraft:select` item model properties a mod adds (NeoForge's
    /// `RegisterSelectItemModelPropertyEvent`). Asked once at start on the client, before resources
    /// load. Use [`RegisterSelectItemModelPropertyEvent::register`] to bind each property
    /// identifier directly to its decode-time factory, which parses the `select` node's JSON object
    /// once on each load/reload and returns the per-stack categorical callback. Any valid
    /// `<namespace>:<path>` identifier (including `minecraft:`) is accepted; a later registration
    /// for the same id replaces the earlier one (last-writer-wins), and registering a built-in
    /// vanilla property id overrides that built-in when decoding item definitions.
    #[hook(
        receiver = "associated",
        dispatch = "flat",
        side = "client",
        version = 2,
        contract = "collect",
        out = "properties",
        context = "registration",
        answered_by = "mod",
        registration_append = "custom",
        neoforge = "RegisterSelectItemModelPropertyEvent"
    )]
    #[notes = r##"## Categorical item model properties

A categorical `minecraft:select` property (NeoForge's `RegisterSelectItemModelPropertyEvent`) is registered
once at client startup in `Hooks::register_select_item_model_property` via `event.register(id, decode)`:
`decode` parses the `select` node's JSON object (as a `Tag`) once on each load and reload and returns the
bound per-stack callback answering `Option<String>`.

```rust,ignore
fn register_select_item_model_property(
    &self,
    _context: &RegistrationContext,
    event: &mut RegisterSelectItemModelPropertyEvent<'_>,
) {
    event.register(Identifier::parse("example:mode").unwrap(), |_config| {
        Ok(|_host, _input| Some("dig".into()))
    });
}
```

Each `when` value in the item's `minecraft:select` on `example:mode` is a string (or a list of them); the
case whose value equals the answer is drawn, else `fallback` (also when the callback answers `None`). Any
valid `<namespace>:<path>` identifier (including `minecraft:`) is accepted; a later registration for the same
id replaces the earlier one (last-writer-wins, matching NeoForge's `LateBoundIdMapper`), and registering a
built-in vanilla property id overrides that built-in when decoding item definitions. Returning `Err(reason)`
from the decoder rejects the enclosing item definition without falling back to a built-in; panicking in the
decoder or per-stack callback disables that mod's callback for the session and surfaces an error toast. A
select on a foreign property no mod registers draws its fallback, decided once at load."##]
    fn register_select_item_model_property(
        properties: &mut Vec<Answered<SelectPropertyRegistration>>,
    );
}
