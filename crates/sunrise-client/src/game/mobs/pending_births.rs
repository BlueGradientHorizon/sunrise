//! The engine side of pending children for `mob_split` and `baby_entity_spawn`:
//! constructs the children on the stack, lends them to listeners via scoped editors
//! (`SplitChildList` / `BabyChildSlot`), and inserts whatever survives in order if not cancelled.
//!
//! Nothing here replays construction or draws randomness: a cancel or empty output keeps every
//! draw construction already made and spawns no child, and unwinding drops pending state via RAII.

use super::Game;
use super::native_pending_children::{BabyChildSlot, ChildConstraint, PendingMob, SplitChildList};
use mod_api::{BabyEntitySpawnChild, MobSplitChildren};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use sunrise_client::mobs::Mob;

static NEXT_CHILD_ACTION: AtomicU64 = AtomicU64::new(1);

/// Result of dispatching `baby_entity_spawn`: distinguishes cancellation (which resets both
/// parents per NeoForge's `Animal.spawnChildFromBreeding` / `Fox.FoxBreedGoal.breed`) from
/// an uncanceled empty slot (which skips breeding finalization without resetting parents).
pub(super) enum BreedOutcome {
    Accepted(Option<Mob>),
    Cancelled,
}

impl Game<'_> {
    /// Run construction with the holder reachable, but retire only this unpublished mob's
    /// metadata if dispatch unwinds. Unrelated Host writes stay in the level.
    pub(super) fn initialize_unpublished_mob(&mut self, id: u32, kind: &str) {
        let run = catch_unwind(AssertUnwindSafe(|| {
            self.initialize_entity(mod_api::Holder::Entity(id), kind);
        }));
        if let Err(panic) = run {
            self.forget_entity(id);
            resume_unwind(panic);
        }
    }

    /// Move constructor state out of the level before lending a proposed child. Dropping
    /// the owned value now discards just this child's unpublished data.
    fn stash_pending_child(&mut self, mut mob: Mob, action: u64) -> PendingMob {
        let id = std::mem::replace(&mut mob.id, 0);
        let uuid = self.persist.uuids.remove(&id);
        let mut attachments = self.entity_attachments.remove(&id).unwrap_or_default();
        attachments.unbind_index();
        let initialized = self
            .entity_initialization
            .initialized
            .remove(&mod_api::Holder::Entity(id));
        self.forget_entity(id);
        PendingMob {
            mob,
            uuid,
            attachments,
            initialized,
            origin_reservation: Some((action, id)),
            dimension: self.dim,
            resources: self.res.clone(),
            attributes: self.motion.attributes.clone(),
        }
    }

    /// Publish one pending child's unpublished state under an entity ID belonging to this world
    /// and complete `EntityConstructing` if it has not already run.
    ///
    /// If construction itself unwinds past dispatch, `initialize_unpublished_mob` retires the
    /// child's unpublished UUID, attachments and initialization marker before resuming the unwind.
    fn issue_pending_child(&mut self, child: PendingMob, action: u64) -> Mob {
        let PendingMob {
            mut mob,
            uuid,
            attachments,
            initialized,
            origin_reservation,
            ..
        } = child;
        // Reuse the ID only in its originating action. Detached children reattached to a
        // different action or world get an ID belonging to that destination.
        let reused = origin_reservation
            .filter(|&(origin, id)| {
                origin == action
                    && !self.persist.uuids.contains_key(&id)
                    && !self.entity_attachments.contains_key(&id)
                    && !self.mobs.iter().any(|m| m.id == id)
            })
            .map(|(_, id)| id);
        mob.id = match reused {
            Some(id) => id,
            None => {
                self.next_entity += 1;
                self.next_entity
            }
        };
        let id = mob.id;
        if let Some(uuid) = uuid {
            self.persist.uuids.insert(id, uuid);
        }
        if attachments.has_index_entries() || !attachments.to_nbt().is_empty() {
            self.entity_attachments
                .entry(id)
                .or_default()
                .replace_contents(attachments);
        }
        if initialized {
            self.entity_initialization
                .initialized
                .insert(mod_api::Holder::Entity(id));
        } else {
            let kind = super::native_hooks::mob_kind(&mob);
            self.initialize_unpublished_mob(id, &kind);
        }
        mob
    }

    /// `AbstractCubeMob.remove`'s children once the parent `dying` is dead: the count is drawn,
    /// then each half-size child is constructed in order with its yaw draw from the parent. With a
    /// `mob_split` listener the whole list is constructed first on the stack and lent via
    /// [`MobSplitChildren`] (even when `drawn == 0`, e.g. a primed sulfur cube above size 1,
    /// matching NeoForge's `AbstractCubeMob.remove` patch); otherwise each child joins as it is
    /// made, with zero extra work when no listener is present.
    pub(super) fn split_cube_children(&mut self, dying: &mut Mob) {
        let drawn = sunrise_client::mobs::cube::split_count(dying);
        if !mod_api::__private::listened(mod_api::HookId::MobSplit) {
            for i in 0..drawn {
                self.next_entity += 1;
                let mut child = Mob::new(self.next_entity, dying.kind.clone(), dying.position, 0.0);
                sunrise_client::mobs::cube::split_child(dying, &mut child, i);
                self.mobs.push(child);
                self.native_mobs_joined(self.mobs.len() - 1, false);
            }
            return;
        }
        let dimension = self.dim;
        let resources = self.res.clone();
        let attributes = self.motion.attributes.clone();
        let action = NEXT_CHILD_ACTION.fetch_add(1, Ordering::Relaxed);
        let (parent, kind, size) = (
            dying.id,
            super::native_hooks::mob_kind(dying),
            dying.extra.size,
        );
        let mut pending = Vec::with_capacity(drawn as usize);
        for i in 0..drawn {
            self.next_entity += 1;
            let mut child = Mob::new(self.next_entity, dying.kind.clone(), dying.position, 0.0);
            // The conversion constructs each proposal even if the split event will later
            // remove it. The constructor runs before this child's split setup draws its yaw.
            self.initialize_unpublished_mob(child.id, &kind);
            sunrise_client::mobs::cube::split_child(dying, &mut child, i);
            pending.push(self.stash_pending_child(child, action));
        }
        let mut cancel = false;
        {
            let mut editor = SplitChildList {
                children: &mut pending,
                resources,
                attributes,
                dimension,
            };
            self.native_mob_split(mod_api::__private::MobSplitCall {
                dimension: dimension.id(),
                parent,
                kind: &kind,
                size,
                child_size: size / 2,
                children: MobSplitChildren::new(&mut editor),
                cancel: &mut cancel,
            });
        }
        #[cfg(test)]
        escaping_fault::reach(escaping_fault::At::SplitDispatched, parent);
        if cancel {
            return;
        }
        // Issue survivors in order, completing construction for mod-made replacements that
        // have not been told yet. On unwind retire only issued children that have not joined.
        let mut issued = Vec::with_capacity(pending.len());
        for child in pending {
            let run = catch_unwind(AssertUnwindSafe(|| self.issue_pending_child(child, action)));
            match run {
                Ok(mob) => issued.push(mob),
                Err(panic) => {
                    for unjoined in issued {
                        self.forget_entity(unjoined.id);
                    }
                    resume_unwind(panic);
                }
            }
        }
        let mut remaining = issued.into_iter();
        while let Some(child) = remaining.next() {
            let run = catch_unwind(AssertUnwindSafe(|| {
                self.mobs.push(child);
                self.native_mobs_joined(self.mobs.len() - 1, false);
            }));
            if let Err(panic) = run {
                for unjoined in remaining {
                    self.forget_entity(unjoined.id);
                }
                resume_unwind(panic);
            }
        }
    }

    /// `Animal.spawnChildFromBreeding` / `Fox.FoxBreedGoal.breed` once the proposed `baby`
    /// (if any) is constructed: lends it in a stack-local optional slot to `baby_entity_spawn`
    /// listeners and distinguishes cancellation from an uncanceled empty slot.
    pub(super) fn breed_with_listeners(
        &mut self,
        parent: u32,
        partner: u32,
        player: (Option<u128>, Option<i32>),
        baby: Option<Mob>,
        constraint: ChildConstraint,
    ) -> BreedOutcome {
        let dimension = self.dim;
        let resources = self.res.clone();
        let attributes = self.motion.attributes.clone();
        let action = NEXT_CHILD_ACTION.fetch_add(1, Ordering::Relaxed);
        let mut slot = baby.map(|baby| self.stash_pending_child(baby, action));
        let mut cancel = false;
        {
            let mut editor = BabyChildSlot {
                slot: &mut slot,
                resources,
                attributes,
                dimension,
                constraint,
            };
            self.native_baby_entity_spawn(mod_api::__private::BabyEntitySpawnCall {
                dimension: dimension.id(),
                parent,
                partner,
                player: player.0,
                player_entity_id: player.1,
                child: BabyEntitySpawnChild::new(&mut editor),
                cancel: &mut cancel,
            });
        }
        #[cfg(test)]
        escaping_fault::reach(escaping_fault::At::BabyDispatched, parent);
        if cancel {
            return BreedOutcome::Cancelled;
        }
        BreedOutcome::Accepted(slot.map(|kept| self.issue_pending_child(kept, action)))
    }
}

#[cfg(test)]
pub(super) mod escaping_fault {
    use std::cell::Cell;
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(in crate::game) enum At {
        Constructed,
        Admission,
        SplitDispatched,
        BabyDispatched,
    }

    thread_local! {
        static ARMED: Cell<Option<(At, u32)>> = const { Cell::new(None) };
    }

    /// Fail at an engine checkpoint, not from inside a mod callback. The unique payload
    /// lets the fixture distinguish an exercised cleanup path from an unrelated assertion.
    pub(in crate::game) fn reach(at: At, id: u32) {
        if ARMED.get() == Some((at, id)) {
            ARMED.set(None);
            resume_unwind(Box::new((at, id)));
        }
    }

    pub(in crate::game) fn expect(at: At, id: u32, run: impl FnOnce()) {
        let previous = ARMED.replace(Some((at, id)));
        let result = catch_unwind(AssertUnwindSafe(run));
        ARMED.set(previous);
        let payload = result.expect_err("engine checkpoint did not unwind");
        assert_eq!(payload.downcast_ref::<(At, u32)>(), Some(&(at, id)));
    }
}
