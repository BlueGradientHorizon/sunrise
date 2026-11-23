# Saving chunk attachments

This hook saves chunk attachments. To serialize a mod block entity's live state into its own
payload, use [`block_entity_save_additional`](../concepts/parts/block-entity-types.md#saved-data)
on the server thread, before the disk worker sees the capture. If block-entity serializer edits
cannot be written, the worker retries with stored block-entity payloads and keeps the result of
`chunk_data_save`.

`chunk_data_save` receives `&dyn ChunkDiskHost` and a `ChunkDataSaveEvent`.
The event's `data` is a typed, job-local attachment store. Use its `get`, `set`
and `remove` methods with an issued `AttachmentKey<T>` whose descriptor has a
persistence codec (`.serialize(codec)`). Disk operations consume only the
persistence codec and never invoke `sync_codec`. `T` does not need `Wire`. A
codec error, missing persistence codec, stale key, or envelope that the ordinary
saved-record codec cannot write and read returns `Err` locally without modifying
stored envelopes or poisoning unrelated edits. Inspect `err.reason` for the typed
`ChunkDiskFailureKind`.
Use `event.data.transaction(...)` and return an error to undo a group of edits.

The save callback runs on the owning server thread after capture, before handing
the snapshot to background encoding. The load callback runs on that same owner
after a saved full or proto record decodes and before admission.
For example, capturing 7, writing 8 in the callback and later writing 9 in the
live chunk produces disk value 8 without changing live value 9. The host's
`job()` borrows a `&ChunkDiskContext` identifying the world epoch, dimension,
coordinates, full/proto kind and phase. It grants no live chunk lookup,
scheduling, packets or world writes.
`registry(name)` lends `Option<&[String]>` in registry ID order from the frozen generation
retained by the snapshot. `None` means that generation has no such registry. Use the slice within
the callback, or copy it if it must outlive the callback. Neither metadata getter reads files;
`ConfigHost::setting` still returns an owned `Option<String>` from the same snapshot.

A captured record caches its callback result. Disk encoding and pending reads
reuse it; they cannot invoke the callback twice. A panicking listener loses only
its edits and is disabled for this hook for the session. The log and local error
toast name the mod. Edits from the remaining listeners go to the ordinary writer;
a callback panic does not stop the save.

Every captured record runs the save callback once, for full saves (explicit,
forced, late, unload and closing) and proto saves alike. The writer may coalesce
older captures without writing them. Saves are refused, with
no callback, while an action savepoint is open, and chunk unloading waits for
it to close. A record that the writer cannot encode or write is reported with
its coordinates and storage type. A flush
waits for all attempts to finish. An individual record failure does not fail the
command, timer save or shutdown, but a failed region sync fails the flush. The server
does not retry the failed snapshot. A later edit can be saved in a new snapshot.

## Disk NBT validation

Attachment edits use the normal saved-record NBT writer and reader rules, at
`sunrise:attachments`'s actual nesting in the record. A bad envelope does not
stop a valid sibling edit or terrain from saving. There are no separate attachment
budgets. The ordinary writer still encodes the complete record; an ordinary record
failure leaves the prior record intact without dropping neighboring records. As in
vanilla `NbtIo.StringFallbackDataOutput`, strings too long for modified UTF-8 are
written empty. Unknown attachment entries are preserved opaquely.
