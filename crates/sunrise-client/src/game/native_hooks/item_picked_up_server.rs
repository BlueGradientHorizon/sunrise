use super::*;
use sunrise_sim::entities::ItemEntity;
use sunrise_sim::join_seq::JoinList;

impl Game<'_, crate::game::server_side::ServerState> {
    /// Whether the acting player, `player`, takes the ground `item` they
    /// touch (`ItemEntity.playerTouch`).
    pub(in crate::game::server_side) fn native_pickup_allowed(
        &mut self,
        item: &mut ItemEntity,
        player: [i32; 4],
    ) -> bool {
        let answer = self.native_pickup_answer(item.id);
        Self::pickup_decision(&mut item.pickup_delay, item.target, player, answer)
    }
    /// `item_picked_up` for the acting player touching the item entity
    /// `item`: the mods' answer, `Default` when no mod listens.
    pub(in crate::game) fn native_pickup_answer(&mut self, item: u32) -> TriState {
        let mut can_pickup = TriState::Default;
        if mod_api::__private::listened(mod_api::HookId::ItemPickedUp)
            && let Some(item) = self.native_item_handle(item)
        {
            let player = self.native_acting_id();
            self.native_item_picked_up(player, item, &mut can_pickup);
        }
        can_pickup
    }
    /// `Inventory.add` of the ground `stack` the acting player picks up: what
    /// did not fit. A player with infinite materials (creative) takes all of it.
    pub(in crate::game) fn native_pickup_insert(&mut self, stack: &ItemStack) -> Option<ItemStack> {
        let rest = self.add_stack_to_inventory(stack.clone());
        rest.filter(|_| !self.creative)
    }
    /// `notice` with the lifted `items[i]` lent back to the level, holding
    /// `during`, where mods reach it: the notice's answer, and the stack the
    /// item held before when a mod removed it meanwhile. An item that is still
    /// there goes back to its place in `items` holding that stack.
    pub(in crate::game::server_side) fn native_pickup_lent(
        &mut self,
        items: &mut JoinList<ItemEntity>,
        i: usize,
        during: ItemStack,
        notice: impl FnOnce(&mut Self, &ItemStack) -> Option<ItemStack>,
    ) -> (Option<ItemStack>, Option<ItemStack>) {
        use crate::game::commands::Target;
        use sunrise_sim::join_seq::Joined;
        let join = items[i].join_seq();
        let mut lent = items.extract(join).expect("in its pass");
        let (id, original) = (lent.id, std::mem::replace(&mut lent.stack, during));
        self.dropped.put_back(self.dropped.len(), lent);
        let answer = notice(self, &original);
        let removed = self.persist.removals.contains(&Target::Item(id));
        if let Some(mut back) = self.dropped.extract(join) {
            back.stack = original.clone();
            items.put_back(i, back);
            if !removed {
                return (answer, None);
            }
        }
        (answer, Some(original))
    }
    /// `item_picked_up_post` for the acting player, who took all or part of
    /// `original` from the item entity `item`, leaving `remaining` (`None`
    /// when all of it went in): the stack the entity then holds, `None` when
    /// it is empty, `remaining` when no mod listens.
    pub(in crate::game) fn native_pickup_post(
        &mut self,
        item: u32,
        original: &ItemStack,
        mut remaining: Option<ItemStack>,
    ) -> Option<ItemStack> {
        if mod_api::__private::listened(mod_api::HookId::ItemPickedUpPost)
            && let Some(item) = self.native_item_handle(item)
        {
            let player = self.native_acting_id();
            self.native_item_picked_up_post(player, item, original, &mut remaining);
        }
        remaining
    }
    /// `ItemEntity.playerTouch` for the host's player, at `feet`, and each of
    /// the lifted `items` whose box meets `reach` (its low and high corners):
    /// `item_picked_up`, `Inventory.add`, `item_picked_up_post`, then the
    /// stat, the criterion and the take animation. An item whose notice a mod
    /// hears is lent back to the level meanwhile, so the mod reaches it, and
    /// one that removes it keeps it removed: no take animation, the stat and
    /// criterion still run.
    pub(in crate::game::server_side) fn native_player_pickups(
        &mut self,
        items: &mut JoinList<ItemEntity>,
        (min, max): ([f64; 3], [f64; 3]),
        _feet: [f64; 3],
        own: [i32; 4],
    ) {
        let listened = mod_api::__private::listened;
        let mut i = 0;
        while i < items.len() {
            let b = items[i].bounds();
            let touching = b.min_position().x < max[0]
                && b.max_position().x > min[0]
                && b.min_position().y < max[1]
                && b.max_position().y > min[1]
                && b.min_position().z < max[2]
                && b.max_position().z > min[2];
            // ItemEntity.playerTouch: `item_picked_up`, then vanilla's delay and target rule.
            if !touching || !self.native_pickup_allowed(&mut items[i], own) {
                i += 1;
                continue;
            }
            let before = items[i].stack.count;
            let kind = items[i].stack.item.clone();
            let thrower = items[i].thrower;
            let rows = items.len();
            // The entity holds the remainder while mods hear it, as on the guest path.
            let (left, removed) = match self.native_pickup_insert(&items[i].stack) {
                Some(rest) if rest.count >= before => (Some(rest), None),
                rest if listened(mod_api::HookId::ItemPickedUpPost) => {
                    let id = items[i].id;
                    let during = rest.clone().unwrap_or_else(|| {
                        let mut empty = items[i].stack.clone();
                        empty.count = 0;
                        empty
                    });
                    self.native_pickup_lent(items, i, during, |game, original| {
                        game.native_pickup_post(id, original, rest)
                    })
                }
                left => (left, None),
            };
            if let Some(original) = removed {
                self.award_stat("picked_up", &kind, before as i32);
                if let Some(thrower) = thrower {
                    self.trigger_thrown_picked_up_by_player(&original, thrower);
                }
                // Still listed until the level's sweep of departed entities, or already gone.
                i += usize::from(items.len() == rows);
                continue;
            }
            let Some(left) = left else {
                self.award_stat("picked_up", &kind, before as i32);
                if let Some(thrower) = thrower {
                    self.trigger_thrown_picked_up_by_player(&items[i].stack, thrower);
                }
                items.remove(i);
                continue;
            };
            let taken = before.saturating_sub(left.count);
            if taken > 0 {
                self.award_stat("picked_up", &kind, taken as i32);
                if let Some(thrower) = thrower {
                    self.trigger_thrown_picked_up_by_player(&left, thrower);
                }
            }
            items[i].stack = left;
            i += 1;
        }
    }
    /// `ItemEntity.playerTouch`'s rule for `player`: no pickup delay, and no
    /// target or `player` as the target, unless `answer` forces it either
    /// way. A refusal sets a vanilla eligibility delay of at least 10 ticks;
    /// later touches still dispatch the hook, and `True` overrides the delay.
    pub(in crate::game) fn pickup_decision(
        pickup_delay: &mut i64,
        target: Option<[i32; 4]>,
        player: [i32; 4],
        answer: TriState,
    ) -> bool {
        if answer == TriState::False {
            *pickup_delay = (*pickup_delay).max(10);
            return false;
        }
        answer.or(*pickup_delay == 0 && target.is_none_or(|t| t == player))
    }
}
