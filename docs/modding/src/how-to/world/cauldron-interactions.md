# Registering cauldron interactions

Use `Hooks::register_cauldron_interaction` with a `RegistrationContext` and
`RegisterCauldronInteractionEvent`. Retain typed `BlockKey` and `ItemKey` values
from registration. The hook runs on both
sides when preparing the resource snapshot; publication happens only after the
complete candidate validates. Failed preparation retains the previous snapshot.
A [failed registration callback](../../concepts/load-phases.md#registration-and-setup)
contributes no exact, tag or all-dispatcher registrations. Other callbacks can still register theirs.

## Register an item

Register an exact item on a block's dispatcher:

```rust,ignore
fn register_cauldron_interaction(
    &self,
    _: &RegistrationContext,
    event: &mut RegisterCauldronInteractionEvent<'_>,
) {
    let handler: Arc<dyn Interaction> = Arc::new(Syrup { empty_bottle: self.glass_bottle });
    event.interactions.register(self.empty_cauldron, self.syrup_bottle, handler.clone()).unwrap();
    event.interactions.register(self.syrup_cauldron, self.syrup_bottle, handler).unwrap();
}
```

A custom block opts into dispatch by registration; its name need not end in
`cauldron`. Register the filled target too when a plan replaces the empty block
with a custom cauldron. Its declared state schema must admit the proposed state,
for example `level=1` in a 1..3 level property. `register_to_all(item, handler)`
queues registration over the complete prepared dispatcher set, including custom blocks
registered later in the hook sequence, overwriting any existing entry for that
item on each block in place. Immediate admission errors from a completed
callback still refuse preparation even if that callback ignored the returned error.

Vanilla entries are prebound in the same table. Registering an item that already
has a vanilla or mod handler overwrites it in place (matching
`CauldronInteraction.Dispatcher.put`). Unknown block or item keys refuse
registration with `CauldronRegistrationError::UnknownBlock` or
`CauldronRegistrationError::UnknownItem`. On water cauldrons, vanilla's dye tag
(`minecraft:cauldron_can_remove_dye`) precedes exact-item entries: washing that
returns `Pass` does not fall through to an exact-item handler unless a mod
replaces the dye tag entry itself via `register_tag`.

## Register an item tag

An item tag answers for many items at once (NeoForge's `register(id, TagKey, …)`
and `registerToAll(TagKey, …)`):

```rust,ignore
// One block's dispatcher:
event.interactions.register_tag(self.water_cauldron, "c:buckets/milk", handler.clone()).unwrap();
// Or every dispatcher:
event.interactions.register_tag_to_all("#c:buckets/milk", handler).unwrap();
```

The tag is read when the item is used, against the tags loaded then, so a data
pack that adds an item to it covers that item after `/reload`. Tag entries are
checked before exact item entries (matching `CauldronInteraction.Dispatcher.get`),
in registration order (with vanilla's dye tag first on water cauldrons, and
`register_tag_to_all` expanded after direct block registrations). Registering
the same tag again on one block overwrites its handler in place at its original
position; a string that is not a resource location returns
`CauldronRegistrationError::InvalidTag`.

## Decide and apply an interaction

Implement `Interaction::decide(&dyn Host, &CauldronUse)` for eligibility. It reads
the complete captured held stack, target state, actor, hand, dimension, creative
flag and underwater flag. Return `Pass` to leave normal item use available,
`Consume` to stop it without a plan, or `Success(CauldronPlan)` to propose a block
replacement, one held-item exchange and deferred effects. The exchange uses the
existing survival, creative, inventory insertion and overflow-drop rules.

For example, a handler can propose a syrup-bottle exchange without changing the
world during the decision:

```rust,ignore
struct Syrup { empty_bottle: ItemKey }

impl Interaction for Syrup {
    fn decide(&self, host: &dyn Host, _: &CauldronUse) -> CauldronDecision {
        let Some(filled) = host.block_state_id("example:syrup_cauldron[level=1]") else {
            return CauldronDecision::Pass;
        };
        CauldronDecision::Success(CauldronPlan {
            block: Some(filled),
            exchange: Some(ItemStack::new(self.empty_bottle, 1).unwrap()),
            sound: Some(Identifier::parse("minecraft:item.bottle.empty").unwrap()),
            fluid_event: Some(Identifier::parse("minecraft:fluid_place").unwrap()),
            statistics: vec![
                Identifier::parse("minecraft:use_cauldron").unwrap(),
                Identifier::parse("minecraft:used").unwrap(),
            ],
        })
    }

    fn fire(&self, _: &mut dyn ServerHost, _: &CauldronUse, _: &CauldronPlan) -> CauldronFire {
        // No extra writes: the engine applies the validated plan.
        CauldronFire::Applied
    }
}
```

A transfer can carry **two distinct statistics**: for example
`minecraft:use_cauldron` and `minecraft:used`. The latter awards the used-item
statistic for the original held item's key. Each is awarded once; duplicate
statistics and plans with more than two statistics refuse the action. A plan may
also carry one sound and one fluid event (`minecraft:fluid_place` or
`minecraft:fluid_pickup`). The replacement state and returned stack are validated against the
loaded resources before mutation. The sound must be a registered sound event;
statistics must be vanilla custom-statistic identifiers or `minecraft:used`.

`Interaction::fire(&mut dyn ServerHost, &CauldronUse, &CauldronPlan)` runs only on
the server, inside the shared action transaction. Return `CauldronFire::Applied`
to permit the plan and supported callback writes to commit. A refusal, panic,
stale precondition or enclosing abort discards the action.
Sounds, statistics, events and overflow drops wait for root commit. The action uses the
shared transaction and runtime limits, with no cauldron-specific budget.

## Client prediction

Client prediction selects the same entry, but skips custom handlers entirely:
it does not call `decide` or predict writes and consumes the local action while
waiting for the server result. Vanilla handlers continue to predict normally.
The server owns inventories, statistics, drops and game events.
`CauldronFire` defaults to `Refused` when a generated callback cannot run.
