# Transactions

Use `HostTransaction` on a `ServerHost` to group capability writes. Pass the
borrowed `&TransactionContext` (`tx` below) and the host from the **same** closure
to each handler or place write.

```rust,ignore
use mod_api::{HostTransaction, ServerHost, TransactionError};

fn update(host: &mut dyn ServerHost) -> Result<(), TransactionError<()>> {
    host.transaction(|host, tx| {
        // Pass host and tx to your capability writes.
        Ok(())
    })
}
```

The closure's `Ok(value)` commits and returns `Ok(value)`. Its `Err(error)`
rolls back the transaction's journaled work and returns `TransactionError::Aborted(error)`.
`TransactionError::Unavailable` means the host could not open the transaction and did not
call the closure. Handle that separately from an error in your own work.

Two other entry points use the same result type:

- In `block_entity_tick`, use `event.subject.transaction(|tick, tx| …)` to group writes
  through `tick.host()` and `tick.with_view(…)`. End each borrow before starting the next;
  another view loan can refuse if the original block entity is gone. Return `Err` if a
  required write or view does not succeed.
- In a server main-thread payload handler, use `cx.transaction(|cx, tx| …)` on
  `&mut dyn IPayloadContext` when you need the payload context and world writes in the
  same savepoint. Call `cx.server_host()` for the host and use `tx` for its capability
  writes. End the host borrow before calling `cx.reply` or `cx.finish_configuration_task`.
  The client and the network thread cannot open this scope.

## Commit and rollback

- Writes apply immediately, so subsequent steps in the transaction see them.
- The game snapshots each storage before its first write. Handlers use the game's
  journal rather than keeping their own.
- A nested commit merges into its parent. Only the outermost commit makes storage
  changes final and publishes deferred engine-managed container effects.
- An abort restores recorded storage without publishing those commit effects.
- Direct writes by mods to their own storage and rollback restores issue no notices.
- Explicit changed/comparator/neighbour/data-send notifications run when issued,
  including inside a transaction, and survive abort.
- A panic unwinding out of the closure aborts its transaction. The parent remains
  open if you catch the panic inside the parent closure.

The journal covers writes through capability handlers to these storages:

| Storage | Recorded state |
|---|---|
| Block entity | Item slots, saved compound, custom name and attachments |
| Hopper | Cooldown |
| Cauldron | Block state |
| Composter | Block state and composter-local random stream; fill feedback, game events and maturation scheduling wait for outer commit |
| World block set, removal, and fluid pickup and placement | Block state, and a block entity the new block removes |
| Player | Inventory slots, armour, offhand, body, saddle, cursor, ender chest and persistent data, for host and guest players |
| Entity inventory | Chest or hopper minecart, chest boat, chested donkey, mule or llama inventory |
| Item entity | Stack; an emptied entity remains until the outermost commit removes it |

At the outermost commit, default `ITEMS` and item `STORAGE` writes to vanilla containers
and engine-managed mod slots publish their implicit change notifications.
Other deferred effects include jukebox playback, cauldron and composter block updates and
item drops, including items placed at a block, thrown by a player or entity, or split off an
item entity. Explicit mod-issued world notifications are not deferred with these effects.

A world pickup or placement of fluid (`level.pickup_fluid`, `level.place_fluid`) writes
its block at the call and the rest at the outermost commit; an aborted one leaves the world
unchanged, where NeoForge's may not. `level.play_fluid_effects` queues a fluid's sound and
game event the same way: they play only at the outermost commit and are dropped on abort,
unlike `trigger_sound_and_game_event`, which acts at once. The `block_drops` listeners of a
replaced block see it as it was, and a cancelled drop keeps the replacement.

`LevelRef::set_block` and `LevelRef::remove_block` expose the tentative block state
right away; block updates and replacement effects wait for the outermost commit.
`LevelRef::destroy_block` returns `Ok(None)` while a transaction is open, before
running loot, experience or break effects.

Some direct writes are not journaled: `LivingRef::set_equipment` and the armor-stand
`slot_set` paths take effect immediately, even inside a transaction. Equipment sounds and equip/unequip
events from `LivingRef::set_equipment` also happen immediately. An abort does not
undo these writes or effects. Use journaled capability handlers when you need rollback.

## Spawning and removing entities

Entities a transaction spawns or removes follow the same commit and rollback rules:

- A spawn builds the entity at the call: its handle works, its attachments and Parts exist,
  `entity_constructing` has run and selectors find it. Its `entity_join_level` does not run
  until the outermost commit, in the order the entities were added, before any removal
  of the transaction takes effect. A listener reads the entity at the commit, not during the
  call. A join a listener cancels at the commit means the entity was never announced and the
  handle the spawn returned reads stale from then on.
- An abort takes out every entity the transaction spawned, with its attachments and saved data, and
  tells no mod: neither `entity_join_level` nor `entity_leave_level` runs. An entity id is
  never reused, so the aborted entity's id stays burnt and a stale handle never names a later entity.
- A removal (`EntityRef::discard`, a kill) marks the entity at the call: its handle and selectors
  stop finding it, and a mob reads as removed (it is flagged at the call; any other entity is
  listed as removed until the commit has run its removal, joins told by then included).
  Passengers dismount, the leave
  is told and the other effects of removal wait for the outermost commit; an abort puts the
  entity back as it was and tells no one. An entity spawned and removed in the same
  transaction is never announced, so it never leaves.
- A spawn is refused, inside a transaction as outside one, once the entity ids run out.
- A transaction covers the entities of the level it was opened in; with transactions of several levels open (a command run `in` another level can open one), an entity belongs to the innermost one of its own level. A command it runs in another
  level (`execute in`) spawns or removes there as it would outside a transaction when no
  transaction is open in that level: the join and the leave are told at once, and an abort does
  not take the entity back. With one open there, the innermost of that level journals it, as above. A join listener that
  spawns, removes or commits a transaction while a commit is telling its joins is told about its
  own entities after the commit's batch, in the order it asked.
- Travel between levels is not journaled: an entity that arrives in another level during an aborted
  transaction stays where it arrived.

## Transaction authority

`TransactionContext` is a loan for one transaction on the current thread. It
cannot be cloned, sent or shared across threads (`!Send` and `!Sync`), or encoded
with `Wire`. It cannot outlive the closure, and only the scope ends the transaction.

Writes must use the context for the **innermost** open transaction on that host.
An ancestor context (even from the same host), a context from another host, or a
context passed to a host with no open transaction is a programming error: the
operation panics rather than reporting that nothing moved, even for a zero-amount
write. This follows NeoForge's `TransactionManager.validateCurrentTransaction`.
When opening a nested transaction, use its new `tx` until that closure returns.

Resource helpers that accept `Option<&TransactionContext>` require `Some(tx)` for
that same innermost frame or `None` when opening a root transaction. They also panic
on a mismatch, including `None` while a transaction is already open.

## Inspecting a transaction

These methods are server-only:

| Method | Result |
|---|---|
| `tx.depth()` | `0` for a root transaction, increasing by one per nesting level. Depth does not identify a transaction. |
| `host.transaction_matches(Some(tx))` | Whether `tx` belongs to the innermost open transaction on this host. |
| `host.transaction_matches(None)` | Whether there is no open transaction on this host. |
| `host.transaction_lifecycle()` | A process-local `TransactionLifecycle` describing the open or closing phase. It does not implement `Wire`. |

| `TransactionLifecycle` variant | Meaning |
|---|---|
| `None` | No transaction is open |
| `Open` | A transaction is open and none is closing |
| `Closing` | A nested transaction is committing into its parent or aborting |
| `RootClosing` | The outermost transaction is closing |

Implicit engine-managed container notices run during `RootClosing`, outside the transaction
whose commit triggered them. Explicit mod notices issued during a transaction run during `Open`.

## Entity inventory slots

`ItemPlace::EntitySlot { entity, slot }` addresses an entity inventory slot, like
NeoForge's `Capabilities.Item.ENTITY`. Read and write it like any other item place.
A read uses the resident slots, or the vehicle's live menu slots if open. It does not unpack
pending loot or load chunks. Writes to a minecart's slots unpack its loot table without a player.

The player `set_inventory_slot`, `set_ender_chest_slot`, `give_stack` and
`set_persistent_data_key` writes use the open transaction journal. Aborting restores
the old values; equipment effects and given-stack overflow wait for root commit.

These writes reach a player in another level than the hook's: their inventory and data
change at once, wherever they are. What would land in the world at them does not happen
there: `give_stack` returns what did not fit instead of dropping it (drop it from a hook in
the player's own level if you want), `drop_stack` returns the stack, `destroy_block` breaks
nothing, and no pickup or equip sound plays.
