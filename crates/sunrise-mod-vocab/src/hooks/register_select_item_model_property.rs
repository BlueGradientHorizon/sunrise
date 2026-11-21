//! The `register_select_item_model_property` hooks: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Register the categorical `minecraft:select` item model properties a mod adds (NeoForge's
    /// `RegisterSelectItemModelPropertyEvent`). Asked once at start on the client, before resources
    /// load. Use [`RegisterSelectItemModelPropertyEvent::register`] to bind each property
    /// identifier directly to its decode-time factory, which parses the `select` node's JSON object
    /// once on each load/reload and returns both the per-case `when` value decoder
    /// (`&Tag -> Result<SelectCaseKey, String>`) and the per-stack categorical callback
    /// (`-> Option<SelectCaseKey>`). Any valid [`Identifier`] (including `minecraft:`) is accepted;
    /// a later registration for the same id replaces the earlier one (last-writer-wins), and
    /// registering a built-in vanilla property id overrides that built-in when decoding item
    /// definitions.
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
`decode` parses the `select` node's JSON object (as a `Tag`) once on each load and reload and returns a pair
`(decode_case, eval)`. `decode_case` decodes each `when` entry from a `&Tag` into a typed `SelectCaseKey`
once at load (rejecting invalid `when` entries with `Err(reason)`), and `eval` answers `Option<SelectCaseKey>`
per stack.

```rust,ignore
fn register_select_item_model_property(
    &self,
    _context: &RegistrationContext,
    event: &mut RegisterSelectItemModelPropertyEvent<'_>,
) {
    event.register(Identifier::parse("example:mode").unwrap(), |_config| {
        Ok((
            |when: &Tag| match when {
                Tag::String(mode) => Ok(SelectCaseKey::new(mode.clone())),
                _ => Err("expected string mode".into()),
            },
            |_host, _input| Some(SelectCaseKey::new("dig".to_owned())),
        ))
    });
}
```

Each `when` value in the item's `minecraft:select` on `example:mode` (single JSON value or array of values)
is decoded by `decode_case` into a `SelectCaseKey` (strings, numbers, compounds, or any custom `Eq + Hash`
type); the case whose key equals the answer is drawn, else `fallback` (also when the callback answers `None`).
Any valid `Identifier` (including `minecraft:`) is accepted; a later registration for the same id replaces
the earlier one (last-writer-wins, matching NeoForge's `LateBoundIdMapper`), and registering a built-in
vanilla property id overrides that built-in when decoding item definitions. Returning `Err(reason)` from the
decoder or `decode_case` rejects the enclosing item definition without falling back to a built-in; panicking
in the decoder, `decode_case`, or per-stack callback disables that mod's callback for the session and surfaces
an error toast. A select on a foreign property no mod registers draws its fallback, decided once at load."##]
    fn register_select_item_model_property(
        properties: &mut Vec<Answered<SelectPropertyRegistration>>,
    );
}
