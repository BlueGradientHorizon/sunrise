# Applying a consume effect

Apply one effect from the stack's effective consumable on the server with
`living.apply_consume_effect(host, &stack, index)`. A stack patch can override or remove
its prototype consumable. The index is zero-based in the effective list; this does not
eat food, shrink the stack or run consume triggers. The return
type is `Result<Option<bool>, StackWriteError>`; handle stack-validation errors
from `Err`. The following outcomes are inside `Ok`: `Some(true)` or
`Some(false)` is the effect's result; `None` means the item, index or target is unavailable
or the effect is unsupported. Mob random teleport, armor stands and dragons are not
supported. Player random teleport uses the level random as normal.

Mod-defined kinds dispatch through the ordinary `consume_effect` hook. For these
kinds, `Some(true)` means the bound callback completed, not that it changed game
state. `Some(false)` means the callback was missing, disabled or failed. Nested
applications of the same kind or another kind run immediately and return their
own result. Do not rely on the engine to stop a cycle in your own effects.
