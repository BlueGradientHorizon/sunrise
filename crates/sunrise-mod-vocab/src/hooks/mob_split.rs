//! The `mob_split` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// A slime, magma cube or sulfur cube is dying and about to split into
    /// smaller ones (NeoForge's `MobSplitEvent`, vanilla's
    /// `AbstractCubeMob.remove`): `dimension` is the level it dies in (a
    /// `PendingMobSpec` for a new child names it), `parent` its entity id, `kind` its type,
    /// `size` its size and `child_size` the size of the children (half of
    /// it). The children vanilla made (2 to 4 for a slime or magma cube, 2
    /// for an unprimed sulfur cube, or 0 for a primed sulfur cube above size 1)
    /// are already constructed, in order, and wait in an unbounded mutable list
    /// lent as `children` (`MobSplitChildren<'_>`, matching NeoForge's
    /// `MobSplitEvent.getChildren()`): inspect a child (`event.children.get(i)`),
    /// edit one in place (`event.children.get_mut(i)`), construct a detached
    /// [`UnspawnedMob`] (`event.children.construct(spec)?`) and add or swap it in
    /// (`event.children.push(child)?`, `event.children.insert(i, child)?`,
    /// `event.children.replace(i, child)?`), or remove children
    /// (`event.children.remove(i)`, `event.children.clear()`). Each child is a
    /// complete mob that is in no level yet: it has no entity number, no join
    /// and nothing a player or another mod can see, and any detached
    /// [`UnspawnedMob`] not left in the list is cleanly discarded when dropped.
    /// Set `cancel` (or return `Flow::Cancel`, or leave the list empty) to
    /// spawn no children: the children's construction draws stay spent and the
    /// parent still dies. What the list holds when the hook returns completes
    /// `entity_constructing` for every surviving child in order, then joins the
    /// level once each in order through the usual join. Only for a parent above
    /// size 1, once its children are made. With no listener nothing is allocated
    /// for the list. On the server.
    #[notes = r#"## Mob split

- **`mob_split(dimension, parent, kind, size, child_size, children, cancel)`** (NeoForge's `MobSplitEvent`, version 3): fires when a slime, magma cube or sulfur cube above size 1 dies and splits, after vanilla has constructed its children (`AbstractCubeMob.remove`: the count, then each half-size child and the parent's yaw draw for it; a primed sulfur cube above size 1 dispatches with an empty list). `children` (`MobSplitChildren<'_>`) lends an unbounded mutable child list directly on the stack, matching NeoForge's `MobSplitEvent.getChildren()`.
- The list editor offers `len`, `is_empty`, `get` (read-only `UnspawnedMobView<'_>`), `get_mut` (mutable `PendingChildEditor<'_>` with `set_position`, `set_yaw`, `set_baby`, `set_size` and all unspawned-mob edits), `construct` (taking a `PendingMobSpec` and returning an owned `UnspawnedMob`), `push`, `insert`, `replace`, `remove` and `clear`. Any detached `UnspawnedMob` not left in the list is cleanly discarded on normal Rust drop with no arena or sweep. There is no 64-child cap, no lease token and no `Expired`/`Ownership`/`Capacity`/`Budget` errors; an out-of-bounds index on `insert`/`replace`, an unknown kind, a wrong dimension, a non-finite position or yaw, or an invalid size answers `Invalid` without mutating the list.
- Cancelling (`Flow::Cancel`), or removing every child, spawns none. The construction draws stay spent and the parent still dies. Listeners see earlier listeners' edits, and on completion every surviving child completes construction (`entity_constructing`) in list order before joining the level once each in list order.
- Not fired for a parent of size 1. With no listener the split runs exactly as vanilla and allocates nothing for the list."#]
    #[hook(
        dispatch = "api_carrier",
        side = "server",
        version = 3,
        contract = "cancel",
        out = "cancel",
        neoforge = "MobSplitEvent",
        paper = "SlimeSplitEvent",
        cancellable = true
    )]
    fn mob_split(
        dimension: &str,
        parent: u32,
        kind: &str,
        size: i32,
        child_size: i32,
        children: MobSplitChildren<'_>,
        cancel: &mut bool,
    );
}
