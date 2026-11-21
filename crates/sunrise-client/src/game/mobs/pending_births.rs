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
use sunrise_mod_runtime::attachments::Attachments;

static NEXT_BREEDING_ACTION: AtomicU64 = AtomicU64::new(1);

/// Result of dispatching `baby_entity_spawn`: distinguishes cancellation (which resets both
/// parents per NeoForge's `Animal.spawnChildFromBreeding` / `Fox.FoxBreedGoal.breed`) from
/// an uncanceled empty slot (which skips breeding finalization without resetting parents).
pub(super) enum BreedOutcome {
    Accepted(Option<Mob>),
    Cancelled,
}

impl Game<'_> {
    /// Publish one pending child's unpublished state under an entity ID belonging to this world
    /// and complete `EntityConstructing` if it has not already run.
    ///
    /// If `EntityConstructing` unwinds, the child's unpublished ID metadata (UUID, attachments,
    /// initialization marker) is cleaned up via `forget_entity` before resuming the unwind;
    /// listener `Host` writes are not rolled back.
    fn issue_pending_child(&mut self, child: PendingMob, breeding_action: Option<u64>) -> Mob {
        let PendingMob {
            mut mob,
            uuid,
            attachments,
            initialized,
            origin_reservation,
            ..
        } = child;
        // Reuse a pre-allocated breeding ID only for the exact originating action when free;
        // every other child (newly constructed, split, or detached/reattached across dispatches
        // or worlds) gets a fresh entity ID belonging to the destination world.
        let reused = breeding_action.and_then(|action| {
            origin_reservation
                .filter(|&(origin, id)| {
                    origin == action
                        && id != 0
                        && !self.persist.uuids.contains_key(&id)
                        && !self.entity_attachments.contains_key(&id)
                        && !self.mobs.iter().any(|m| m.id == id)
                })
                .map(|(_, id)| id)
        });
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
            let run = catch_unwind(AssertUnwindSafe(|| {
                self.initialize_entity(mod_api::Holder::Entity(id), &kind);
            }));
            if let Err(panic) = run {
                self.forget_entity(id);
                resume_unwind(panic);
            }
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
        // The numbers are issued when the children commit, so a listener's own spawns cannot
        // collide; the constructor still seeds from the number the child would have.
        let first = self.next_entity + 1;
        let mut pending: Vec<PendingMob> = (0..drawn)
            .map(|i| {
                let mut child = Mob::new(first + i, dying.kind.clone(), dying.position, 0.0);
                sunrise_client::mobs::cube::split_child(dying, &mut child, i);
                child.id = 0;
                PendingMob {
                    mob: child,
                    uuid: None,
                    attachments: Attachments::default(),
                    initialized: false,
                    origin_reservation: None,
                    dimension,
                    resources: resources.clone(),
                    attributes: attributes.clone(),
                    constraint: ChildConstraint::Split,
                }
            })
            .collect();
        let (parent, kind, size) = (
            dying.id,
            super::native_hooks::mob_kind(dying),
            dying.extra.size,
        );
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
        if cancel {
            return;
        }
        // Complete construction for every surviving child before joining the first, matching
        // NeoForge's `AbstractCubeMob.remove`. On unwind, clean up every issued child that has
        // not yet entered `self.mobs` without rolling back already-joined children or Host writes.
        let mut issued = Vec::with_capacity(pending.len());
        for child in pending {
            let run = catch_unwind(AssertUnwindSafe(|| self.issue_pending_child(child, None)));
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
        let action_token = NEXT_BREEDING_ACTION.fetch_add(1, Ordering::Relaxed);
        // Move all unpublished state (UUID, populated constructor attachments, initialization
        // marker) out of Game onto the owned PendingMob before lending it, so taking, dropping,
        // unwinding, or reattaching in another dispatch/world leaves zero leaked state and
        // preserves every constructor attachment without double construction or ID collision.
        let mut slot = baby.map(|mut baby| {
            let reserved_id = (baby.id != 0).then_some(baby.id);
            let uuid = reserved_id.and_then(|id| self.persist.uuids.remove(&id));
            let mut attachments = reserved_id
                .and_then(|id| self.entity_attachments.remove(&id))
                .unwrap_or_default();
            attachments.unbind_index();
            let initialized = reserved_id.is_some_and(|id| {
                self.entity_initialization
                    .initialized
                    .remove(&mod_api::Holder::Entity(id))
            });
            if let Some(id) = reserved_id {
                self.forget_entity(id);
            }
            baby.id = 0;
            PendingMob {
                mob: baby,
                uuid,
                attachments,
                initialized,
                origin_reservation: reserved_id.map(|id| (action_token, id)),
                dimension,
                resources: resources.clone(),
                attributes: attributes.clone(),
                constraint,
            }
        });
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
        if cancel {
            return BreedOutcome::Cancelled;
        }
        match slot.take() {
            Some(kept) => {
                BreedOutcome::Accepted(Some(self.issue_pending_child(kept, Some(action_token))))
            }
            None => BreedOutcome::Accepted(None),
        }
    }
}
