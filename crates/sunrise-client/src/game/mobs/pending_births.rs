//! The engine side of pending children for `mob_split` and `baby_entity_spawn`:
//! constructs the children on the stack, lends them to listeners via scoped editors
//! (`SplitChildList` / `BabyChildSlot`), and inserts whatever survives in order if not cancelled.
//!
//! Nothing here replays construction or draws randomness: a cancel or empty output keeps every
//! draw construction already made and spawns no child, and unwinding drops pending state via RAII.

use super::Game;
use super::native_pending_children::{BabyChildSlot, PendingMob, SplitChildList};
use mod_api::{BabyEntitySpawnChild, MobSplitChildren};
use sunrise_client::mobs::Mob;
use sunrise_mod_runtime::attachments::Attachments;

impl Game<'_> {
    /// The child as an ordinary unjoined mob. A child a mod constructed gets its entity number
    /// now, so breeding's finalization and its advancement see the real one. Its UUID and
    /// attachments, and those an engine-made child kept out of the tables while it waited, are
    /// published here: the first moment it can join.
    fn issue_pending_child(&mut self, child: PendingMob) -> Mob {
        let PendingMob {
            mut mob,
            uuid,
            attachments,
            ..
        } = child;
        let fresh = mob.id == 0;
        if fresh {
            self.next_entity += 1;
            mob.id = self.next_entity;
        }
        if let Some(uuid) = uuid {
            self.persist.uuids.insert(mob.id, uuid);
        }
        if !attachments.to_nbt().is_empty() {
            self.entity_attachments
                .entry(mob.id)
                .or_default()
                .replace_contents(attachments);
        }
        if fresh {
            let kind = super::native_hooks::mob_kind(&mob);
            self.initialize_entity(mod_api::Holder::Entity(mob.id), &kind);
        }
        mob
    }

    /// `AbstractCubeMob.remove`'s children once the parent `dying` is dead: the count is drawn,
    /// then each half-size child is constructed in order with its yaw draw from the parent. With a
    /// `mob_split` listener the whole list is constructed first on the stack and lent via
    /// [`MobSplitChildren`]; otherwise each child joins as it is made, exactly as vanilla does.
    pub(super) fn split_cube_children(&mut self, dying: &mut Mob) {
        let drawn = sunrise_client::mobs::cube::split_count(dying);
        if drawn == 0 {
            return;
        }
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
        // The numbers are issued when the children join, so a listener's own spawns cannot
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
                    dimension,
                    resources: resources.clone(),
                    attributes: attributes.clone(),
                    breeding: false,
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
        for child in pending {
            let issued = self.issue_pending_child(child);
            self.mobs.push(issued);
            self.native_mobs_joined(self.mobs.len() - 1, false);
        }
    }

    /// `Animal.spawnChildFromBreeding` once `baby` is constructed: lends it in a stack-local
    /// optional slot to `baby_entity_spawn` listeners and returns the baby that remains (the
    /// listener's replacement when it swapped one in), or `None` when cancelled or cleared.
    pub(super) fn breed_with_listeners(
        &mut self,
        parent: u32,
        partner: u32,
        player: (Option<u128>, Option<i32>),
        baby: Mob,
    ) -> Option<Mob> {
        use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
        let dimension = self.dim;
        let resources = self.res.clone();
        let attributes = self.motion.attributes.clone();
        let reserved_id = (baby.id != 0).then_some(baby.id);
        let uuid = reserved_id.and_then(|id| self.persist.uuids.remove(&id));
        let attachments = match reserved_id.and_then(|id| self.entity_attachments.get(&id)) {
            Some(held) if !held.has_index_entries() && !held.has_index_binding() => reserved_id
                .and_then(|id| self.entity_attachments.remove(&id))
                .unwrap_or_default(),
            _ => Attachments::default(),
        };
        let mut slot = Some(PendingMob {
            mob: baby,
            uuid,
            attachments,
            dimension,
            resources: resources.clone(),
            attributes: attributes.clone(),
            breeding: true,
        });
        let mut cancel = false;
        let run = catch_unwind(AssertUnwindSafe(|| {
            let mut editor = BabyChildSlot {
                slot: &mut slot,
                resources,
                attributes,
                dimension,
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
        }));
        if let Err(panic) = run {
            drop(slot);
            if let Some(id) = reserved_id {
                self.forget_entity(id);
            }
            resume_unwind(panic);
        }
        if cancel {
            drop(slot);
            if let Some(id) = reserved_id {
                self.forget_entity(id);
            }
            return None;
        }
        match slot.take() {
            Some(kept) => {
                if let Some(id) = reserved_id.filter(|&id| id != kept.mob.id) {
                    self.forget_entity(id);
                }
                Some(self.issue_pending_child(kept))
            }
            None => {
                if let Some(id) = reserved_id {
                    self.forget_entity(id);
                }
                None
            }
        }
    }
}
