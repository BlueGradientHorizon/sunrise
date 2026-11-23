# Attachments

Register every attachment once during `Hooks::register` with an
`AttachmentDescriptor<T>` (mirroring NeoForge's `AttachmentType.Builder` and
`RegisterEvent.register` for `NeoForgeRegistries.Keys.ATTACHMENT_TYPES`), and
keep the issued `AttachmentKey<T>` on your mod or Part:

```rust,ignore
use mod_api::{
    AttachmentCodec, AttachmentDescriptor, AttachmentError, AttachmentKey, Compound, Holder,
    Registrar, Tag,
};

const COUNTER_CODEC: AttachmentCodec<i32> = AttachmentCodec {
    encode: |n| Ok([("count".into(), Tag::Int(*n))].into()),
    decode: |data: &Compound| match data.get(&"count".into()) {
        Some(Tag::Int(n)) => Ok(*n),
        _ => Err(AttachmentError::Decode),
    },
};

// Inside Hooks::register(&self, registrar: &mut dyn Registrar, _) (returns `()`):
// The registrar records any refusal on the load even if the Err branch is not handled.
if let Ok(counter) = registrar.register_attachment_type(
    AttachmentDescriptor::new("example:counter", || 0).serialize(COUNTER_CODEC),
) {
    *self.counter.write().unwrap() = Some(counter);
}

// Inside a server hook, or mixin code, with `host: &mut dyn ServerHost`:
if !counter.has(host, Holder::Chunk([0, 0]))? { /* no counter stored */ }
let old = counter.get(host, Holder::Chunk([0, 0]), |value| *value)?;
counter.set(host, Holder::Chunk([0, 0]), old + 1)?;
Ok::<(), AttachmentError>(())
```

Configure optional persistence with `.serialize(codec)`, client synchronization
with `.sync(sync_codec)` (independent of `.serialize`), and death copying with
`.copy_on_death()` (which requires `.serialize`).

The registrar records any refusal even if the returned result is ignored. Any
attachment ID must be unique and use a namespace the registering package owns or
[serves](../reference/manifest.md). The registering package is
recorded as the attachment's owner (`key.owner()`) and named on duplicate
declarations. Another package that depends on the owner looks up the
same typed handle during `Hooks::register` with
`registrar.attachment_key::<T>("namespace:name")`, which returns `Err(AttachmentError::Undeclared)`
for missing IDs and `Err(AttachmentError::TypeMismatch)` for mismatched Rust types.

Cloning an `AttachmentKey<T>` is cheap (it shares the underlying registration)
and preserves its installation identity. Keys retained from a prior installation
are refused with `AttachmentError::StaleKey` even if a later installation
registers the same ID and Rust type; `key.is_live()` checks whether a key still
belongs to the active installation.

Synced declarations refuse a `side = "server"` package before registration is
installed, whether the declaring package itself is server-only or serves the
attachment namespace on behalf of a server-only package. Here “carries a synced attachment” means **declares** one: a server
package may look up and use A's synced attachment if A is a client-compared
package. If A is itself a server package, A is refused. Load checks registration
refusals (invalid descriptors, duplicate IDs, and stale part-state keys) first,
then server-package synced declarations.

## Reading and writing

Use an issued `AttachmentKey<T>` to store mod data on a player, entity, block
entity, chunk or level. Call `key.get`, `key.set`, `key.remove`, `key.has`,
`key.get_existing` and `key.sync_data` with `&mut dyn Host` (or `get_in`,
`set_in`, `remove_in`, `has_in`, `get_existing_in`, `sync_data_in` with
`&mut dyn LevelRandomHost`) and a `Holder`. `get` inserts the default on first
access, then lends `&T` to the read closure. `has` and `get_existing` read without
inserting (NeoForge's `hasData` and `getExistingDataOrNull`): `has` says whether a
value is stored, even before it decodes, and `get_existing` lends `Option<&T>`,
with `None` instead of the default. `with_decoded` lends the same option but never
decodes a saved or synced value. The read returns only what the closure projects;
copy or clone the data you need beyond the call. Reads and `set` need
`T: Send + Sync + 'static`; `remove` also needs `Clone` to return the previous value. `set` runs the optional persistence codec
before changing the value, returning an `AttachmentError` if persistence encoding
fails. Saving and serialized copies encode the current canonical object, including
changes made through its interior mutability; unread saved entries remain opaque.
The sync codec runs only when there is a value to send to tracking clients.
Keys are identified by `(namespace, name)` and bound to the installation that
issued them; a key already decoded as another Rust type is an error, and a key
from an earlier installation fails with `AttachmentError::StaleKey`. On a server,
a missing holder is an error, not a new phantom holder; a client keeps a copy for
any holder it is asked about (see the sync paragraph below).
`AttachmentError` distinguishes `MissingHolder`, `TypeMismatch`, `Decode`, `Encode`,
`InvalidNamespace` (a malformed saved compound), `InvalidCopyPolicy` (death-copy
without a persistence codec), `InvalidSyncPolicy` (sync operation on an unsynced
key or a policy mismatch), `Undeclared` (missing attachment registration),
`StaleKey` (key from an earlier installation), `HolderBusy` (storage temporarily
unavailable; retry after the pass) and `UnsupportedHolder` (the holder family has
no attachment storage; retrying does not help).
`host.attachments()` exposes the host's `AttachmentStore` for custom storage
adapters. Its `store` method accepts only an `AttachmentStoreWrite` prepared by
the installed descriptor, not a caller-supplied name, erased value or NBT form.
Ordinary mods write with `key.set(host, holder, value)`. Reading and copying a holder's
data share its stored values without cloning them.

`AttachmentHolder` keeps an existing typed handle when a system needs to store a holder
identity: `Entity`, `Player`, `BlockEntity`, `Chunk` or `Level`. It is `Copy`, does not keep
anything loaded, and follows each handle's ordinary stale-world and unload rules. Unlike
the storage-relative `Holder`, it carries the handle's world scope.

## Querying holders

Import `AttachmentQueries` to use `host.query(&key)` on either host side. The
returned batch is a **snapshot** of membership and stored value bindings in the
host's current dimension. Create a cursor with `query.iter()` and step it with
`rows.try_next()?` until it returns `Ok(None)`. Borrow each value with
`row.with_value(scope, |value| ...)` without cloning it. Get `scope` from
`host.attachment_read_scope()?`. Iteration decodes saved entries lazily; malformed
or incompatible values produce row errors, not defaults. Later sets, removals and unloads appear only in fresh queries. This
is a residency view: an entity queued for removal can appear until the removal sweep. Captured handles do not keep holders loaded, and stale writes fail.
Interior mutability inside `T` is not deep-frozen by a snapshot.

Batches, holder selectors and write tokens stay in the running game; they are
not `Wire` values. Persist or sync the attachment value through its registered
`AttachmentCodec`, not by encoding the query or a holder's residency.

On a server, import `AttachmentQueriesMut` for `host.query_mut(&key)` (`T: Clone`).
Each borrowed row has `holder()`, `with_value(...)`, and explicit `set(host, replacement)` /
`remove(host)` methods. These validate residency against the host's current
dimension and then use the ordinary attachment journal. There is no `&mut T`,
implicit transaction, or commit on drop: wrap a system in `host.transaction` for
all-or-nothing writes. Unload or replacement produces `AttachmentRowError::StaleHolder`.
Rows retain captured values even after successful write-through updates. A capture
made during a transaction can only be used while its capturing frame is current.
A value read borrows the capturing host for the whole call, so the frame cannot
close or open a child during that read. After commit or rollback, create a fresh
query; using the old one returns `AttachmentRowError::OutsideTransaction`. If a
child frame temporarily makes the capture unavailable, that refusal does not
advance the cursor. Continue with the same cursor when the parent is current again.

Traversal follows dense index order: insertion appends, and removal fills its slot
with the last row. The same ordered mutation history is deterministic, but the
order is neither sorted nor stable across structural changes. Retaining a batch
can make the next live mutation copy row metadata and shared pointers once;
retaining many batches keeps their old bindings alive. Values from an incompatible Rust type domain report `TypeMismatch` rather than
falling back to saved decoding.


## Holders and persistence

Use `Holder::Level`, `Holder::Chunk([x, z])`, `Holder::BlockEntity([x, y, z])`,
`Holder::Entity(id)`, or `Holder::Player(uuid)`. Entity IDs cover every entity in
the level apart from players: mobs, items, experience orbs, projectiles, tridents,
falling blocks, primed TNT, fireworks, boats, minecarts, armor stands, cushions, item
frames, paintings, End crystals, the ender dragon, ominous item spawners and the
server's `OtherEntity` records. An entity keeps its UUID through saves, so its saved
attachments load back with it; an orb saved with a `count` above one loads as several
orbs, and only the first keeps the attachments. Player UUIDs cover online guests and
the acting player. Mutable level
hosts can access their level, loaded chunks and block entities; read-only level hosts
cannot access attachments. Unserialized values disappear on unload or restart. A saved
value is stored under `sunrise:attachments`, then the mod namespace and attachment name,
as `{value: <compound>, copy_on_death: 1b}` (omitting `copy_on_death` when false;
client sync policy and sync projections are resolved from the active registered
descriptor rather than stored in disk NBT).
Unknown mods' saved entries survive load and save without being decoded. Level
attachments use the dimension's `data/sunrise/attachments.dat` SavedData
file, not `level.dat`.

## Synchronization

You cannot supply a holder-aware default, custom copy handler, conditional serialization,
recipient predicate or partial synchronization updates. The default is `fn() -> T`;
copying uses the declared policy. Synchronization is configured with `.sync(sync_codec)`
on `AttachmentDescriptor`, independent of disk persistence (`.serialize(codec)`): an
attachment may sync without persisting to disk, persist without syncing, or use distinct
codecs for disk and network (`AttachmentCodec` is `Copy`, so the same codec may be passed
to both).

A descriptor configured with `.sync(sync_codec)` sends its value to Sunrise clients when
`get` creates the default, on `set` and `remove`, and on demand with
`key.sync_data(host, holder)` (or `key.sync_data_in(level, holder)`); changes made in one
tick are sent once, at its end. The recipients are the
players who track the holder: guests who were sent a chunk for a chunk or block entity,
who track an entity for an entity, the player and those tracking them for a player, and
everyone in the level for the level. A client also gets all of a holder's synced values
when it starts seeing it: a chunk sent (with its block entities), a level joined or
re-entered, an entity tracked, and its own player. The wire form is encoded and decoded
with the sync codec passed to `.sync(...)`, without requiring `.serialize(...)`.
The send pass encodes the current committed value, including on explicit resync.
A codec refusal is logged each time with the holder, attachment ID and declaring
mod; other keys can still be sent. If a projection succeeds but its NBT cannot
be written to the network, the send pass reports the holder and key and omits
only that change. A panic disables the failing sync callback for the
session and reports the mod in the log and in a local error toast.

Values travel on `sunrise:attachment_sync`, an engine declaration in the negotiated
payload table (play, to the client, version 1, optional): only a client whose connection
offered it is sent values, so a vanilla client is sent none. A message is prepared against
the player's current connection epoch and published at the root commit, in order with the
player's other packets; a rolled-back edit sends nothing, and a message prepared before a
reconfiguration never reaches the new one. Its size is bounded by the connection's frame
limits. A change with a namespace or name over 256 UTF-16 code units or a value
that cannot be written is omitted without dropping its siblings. A message past the
connection's frame limit is not sent. Refusals are logged; the player stays joined. A full outgoing
queue closes that player's connection.

On a client, `Host::attachments` returns a copy of what the server synced, which `get`
decodes; `set` and `remove` change only that copy, which is dropped when the chunk,
entity or player goes. A client keeps no copy for the level hosts (`LevelRandomHost`):
they stay server-only there, though their changes on a server are synced.

A general entity row (`declare_general_entity`) has no data of its own on a client: its
synced attachments are what a Sunrise client knows of it beyond its type, position and
rotation. They arrive with the row, when the client starts tracking it, and go with it. Its
[renderer](../how-to/client/entity-renderers.md#a-general-entitys-frame) reads the last
published whole generation rather than consulting the client holder.

## Copying on respawn and conversion

On death respawn, the installed descriptor must have a persistence codec and opt in
with `.copy_on_death()` for its attachment to reach the replacement. On return from
the End credits (`was_death = false`), the installed descriptor still needs a persistence
codec, even if an older save contains data for that key. Raw entries for absent mods stay
opaque and are copied on End return; on death, they use the flag stored in the envelope.
In both cases the copy runs before `player_clone` (version 3), even without a listener,
and the replacement starts from fresh encoded entries rather than sharing
decoded instances with the old player. A mob's conversion (a villager struck into a witch,
a zombie villager cured, a piglin zombified, a tadpole grown) copies every entry with a
codec, raw entries of an absent mod included, to the new mob through its encoded form,
over what its construction set; entries without a codec stay on the removed mob. A
cancelled `living_conversion_pre` copies nothing, `living_conversion_post` sees the copy,
and the copy is not synced on its own (the new mob's spawn carries its synced entries).

## Reading the original player

`player_clone` supplies `event.original: OriginalPlayer<'_>` alongside the replacement's
`event.player` UUID. During that hook, `event.original.attachment(&counter, |value| *value)`
reads the old player's counter even when `copy_on_death` is false or the type has no codec,
lending its default without inserting it if absent.
`counter.get(host, Holder::Player(event.player), |value| *value)` reads the replacement's
counter, inserting the default if absent. This works on death and on return from
the End. The original's `has_attachment`, `existing_attachment` (decoding and caching a
saved entry once without inserting a default), `with_decoded_attachment`, `set_attachment`,
and `remove_attachment` work on the retained old player without touching the replacement.
Cache writes, explicit writes and removals on the original are journaled to `host.transaction`,
so an aborted transaction restores both holders together. The replacement is already assigned its final respawn level, including
any `player_respawn_position` redirect. It has not been placed or published in the bulk
attachment index yet. Requery after placement: a bulk write token for the old player is
stale even though the replacement reuses that player's UUID.

The same `OriginalPlayer<'_>` view exposes `uuid`, `position`, `dimension`, `level`,
`health`, `experience`, `effects`, `persistent_data`, and `with_inventory` for the pre-copy
player. `with_inventory` borrows an `InventoryView` using vanilla `Inventory` slot numbering:
0–35 for the main inventory (0–8 hotbar), 36–39 for boots through helmet, 40 for the off
hand, 41 for body, and 42 for saddle. Rust's borrow checker binds `event.original` to the
callback lifetime, preventing the view from outliving `player_clone`. With no `player_clone`
listener, no original player state is retained.
