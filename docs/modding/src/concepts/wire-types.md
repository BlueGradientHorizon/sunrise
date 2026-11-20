# Wire types

`mod_api::wire::{Wire, Reader, Writer, WireError}` defines the codec used by
[typed payload tokens](../how-to/client/networking.md#typed-payloads) and
[recipe placement and recipe-book sync](../crafting-recipe-kinds.md). Derive `mod_api::Wire` explicitly on a type
that needs this encoding. The build never adds the derive. Every field of a derived type must also
implement `Wire`.

```rust,ignore
#[derive(mod_api::Wire)]
pub struct ChemicalAmount {
    pub chemical: String,
    pub amount: u64,
}
```

Records encode fields in declaration order. Enums encode a case index followed by
its fields. Lengths and numbers are little-endian; collections are length-prefixed.
Decoding returns `WireError` for invalid data rather than accepting an unknown tag,
invalid UTF-8 or an unsupported length.

## Nesting and stacks

Ordinary Wire decoding and encoding allow **32** nested records, payload-carrying
variants and `Box`/`Arc` values (`MAX_DEPTH`). Collection iteration, `Option`
and tuples do not consume a level. The reader returns `WireError::Depth` before
entering level 33; the writer records the same error. Always check `Writer::error`
before sending its bytes. This is a transport budget for unbounded mod-defined
trees, including ingredients, item places, loot vehicles and recipe slot displays.

Typed payloads use the same encoding with their own limits. They count list, present
option and box levels, not records, and enforce `PAYLOAD_MAX_DEPTH`. String and list
lengths are bounded by `PAYLOAD_MAX_STRING_BYTES` and `PAYLOAD_MAX_LIST_ENTRIES`.
The reader and writer both charge `PAYLOAD_DECODE_BUDGET` for decoded storage, apart
from the transport's byte limit. A failed encode is never queued.

Vanilla trees are not restricted by that structural budget: NBT `Tag` uses an
explicit work stack with **512** nested containers; `Text` crosses as the existing
validated flat `TextForm` table, preserving its **512** component-depth contract.
Text's layout identity is the flat table's identity.

JSON-backed `serde_json::Value`, JSON maps and `ModelPart` also refuse beyond
32 JSON containers. Their encoders preflight the in-memory tree iteratively
before serde recursion; their decoders scan the JSON's nesting outside quoted
strings before parsing. A model part's JSON includes object, children-list and
named-child-tuple containers, so one part can consume several JSON levels.

Hand-written recursive `Wire` implementations must enforce their own stack contract; the
reader cannot police code outside its codec.

Encoding borrows the input (`writer.put(&value)`); the caller still owns it.
Constructing an arbitrarily deep Rust tree and then dropping it can overflow a
thread independently of Wire. Use explicit work stacks when disposing of hostile
trees (`discard_tag` for NBT).

## Checked representations

Use `#[wire(via = String)]` (or another `Wire` type) when a checked constructor owns
validation. Implement `From<&YourType> for String` and `TryFrom<String> for YourType`.
The derive writes the representation and maps a failed conversion to
`WireError::Invalid`. Its layout is the representation's layout.

## Layout identity

A type's layout describes the fields, enum cases and child types that Wire reads and writes.
The derive declares the layout from the Rust fields. Layouts do not include Rust type paths.
They feed the SDK's surface digests and `Wire::describe`, which provides the fingerprint used
in typed payload negotiation. `payload_schema::<T>()` computes that fingerprint; it is not
written into the payload body. Changing the encoding changes the negotiated fingerprint.

## Hand-written implementations

The derive handles this automatically and is preferred. A hand-written implementation must
supply `put`, `take`, `MIN` and `describe`. For a new encoding, use a versioned `WireSchema::leaf`
and combine it with the fingerprints of its parts using `WireSchema::mix`. Use
`WireSchema::named::<Self>` when describing recursive fields so the walk can recognize a
reference to an ancestor. If the implementation delegates completely to an existing encoding,
its `describe` can delegate to the same type.

A hand decoder owns tag validation and bounds for any storage it allocates itself. Decode
indirections through their `Wire` types so they count against the typed-payload limits. Keep
validation at the checked constructor boundary.
