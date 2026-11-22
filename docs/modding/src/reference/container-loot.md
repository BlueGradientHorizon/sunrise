# Container loot

`container_loot_filled` is a server-side Sunrise addition. It exposes the rolled list in roll order, after table-load changes and before shuffle, splitting and placement. Listeners run in dispatch order and may add, remove or replace stacks. Empty stacks disappear; oversized stacks are trimmed to the effective item maximum before placement, and excess stacks that cannot fit are discarded.

The context names the table, dimension, block position or entity id, and the block centre or vehicle position. `opener` is present only when a player's open starts the fill. Hopper, comparator, break, explosion, command and `ItemPlace::EntitySlot` accesses have no opener. Reading the vehicle slot during a player action still has no opener. Each half of a double chest fills separately. Already-filled containers and block or mob drops do not fire this hook. With no listener, no API values are built and random draws are unchanged.

Listeners use their own randomness. `drops` is a `HookList`, which validates each edit that supplies a stack. Non-negative damage that fits a signed 32-bit integer is accepted even above the item's maximum. An invalid edit returns an error and logs a hook refusal without undoing other accepted edits. If a listener panics, only that listener's edits are undone.
