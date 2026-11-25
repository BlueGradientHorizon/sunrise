//! `item_stacked_on_other` on the game's side: a pickup click, decided, fired and applied as one
//! action (the design's decide/fire/apply).
//!
//! The click reads what the hook is told (the cursor and the clicked slot's stack) and freezes
//! it. With no listener the plain click runs at once, with nothing allocated. With one, the game
//! opens an action savepoint, fires the hook with `HostSide` and the frozen stacks, validates the
//! typed answer once, checks that nothing the click read was written meanwhile, and only then
//! runs the click with the answer, inside the same savepoint. A stale read, an invalid answer, a
//! panicking listener or a refusal to admit leaves the click undone and every write the listener
//! made rolled back; the click then does nothing (the existing no-change result) and the reason
//! goes to the log.
use super::*;
use crate::game::api_values::{TryIntoApi, TryIntoEngine};
use crate::game::transactions::decision::{
    self, ActionTarget, GameHost, Identity, InvalidReason, MenuId, Outcome, Reply, Side,
    StaleReason,
};
use mod_api::item::menu::{ClickedStackResult, StackResult, StackedOnOtherResult};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use sunrise_client::menu::{
    StackedClick, StackedClickedEdit, StackedEditPolicy, StackedOutcome, StackedResume,
};

/// What a pickup click did: it ran, or the decision refused it and nothing changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum Pickup {
    Ran,
    Refused,
}

/// The click, run with the mods' answer (`None`: no listener, the plain click).
type Perform<'p> = Box<dyn FnOnce(&mut Game<'_>, Option<&StackedOutcome>) + 'p>;
/// What a click reads now, to compare with what it froze.
type Current<'p> = Box<dyn Fn(&Game<'_>) -> Option<StackedClick> + 'p>;

struct Shared<'p> {
    frozen: StackedClick,
    current: Current<'p>,
    perform: RefCell<Option<Perform<'p>>>,
    answer: RefCell<Option<StackedOutcome>>,
    /// A slot rule or a stale input refused the mods' edit (`StackedOutcome::refused`).
    refused: Cell<bool>,
}

/// The decision's target: an id that names the route's menu (a second click on it while this one
/// is active is refused as busy), and what the click froze.
#[derive(Clone)]
struct PickupTarget<'p> {
    id: MenuId,
    shared: Rc<Shared<'p>>,
}

/// What the click read: the journaled-write epoch before the listeners ran.
struct PickupReads {
    writes: u64,
}

/// The one thing a pickup decision plans: that the click runs. The write set is the click's own.
struct PickupPlan;

impl<'a, 'g> ActionTarget<GameHost<'a, 'g>> for PickupTarget<'_> {
    type Reads = PickupReads;
    type View = PickupReads;
    type Plan = PickupPlan;
    type Frozen = PickupPlan;

    fn id(&self) -> MenuId {
        self.id
    }
    fn capture(&self, host: &GameHost<'a, 'g>) -> std::result::Result<PickupReads, Outcome> {
        Ok(PickupReads {
            writes: host.game.transactions.writes(),
        })
    }
    fn view(reads: &PickupReads) -> &PickupReads {
        reads
    }
    fn freeze(
        &self,
        _: &PickupReads,
        plan: PickupPlan,
    ) -> std::result::Result<PickupPlan, InvalidReason> {
        Ok(plan)
    }
    fn empty(_: &PickupPlan) -> bool {
        false
    }
    /// The conservative read set: any write the journals were asked to keep since the click
    /// froze its stacks, even one that restored what was there, makes it stale; and what the
    /// click would read now must still be what it froze.
    fn validate(
        &self,
        host: &GameHost<'a, 'g>,
        reads: &PickupReads,
    ) -> std::result::Result<(), Outcome> {
        if host.game.transactions.writes() != reads.writes {
            return Err(Outcome::Stale(StaleReason::Mutation));
        }
        match (self.shared.current)(host.game) {
            Some(now) if now == self.shared.frozen => Ok(()),
            _ => Err(Outcome::Stale(StaleReason::Snapshot)),
        }
    }
    fn apply(&self, host: &mut GameHost<'a, 'g>, _: PickupPlan) {
        let perform = self.shared.perform.take().expect("a pickup runs once");
        let answer = self.shared.answer.take();
        perform(host.game, answer.as_ref());
        self.shared
            .refused
            .set(answer.is_some_and(|a| a.refused.get()));
    }
}

/// Where a pickup decision's menu is named: a route's own id, stable for as long as the route's
/// menu lives, so that a click made from inside the action finds it busy.
pub(in crate::game) fn local_pickup_id() -> MenuId {
    static ID: std::sync::OnceLock<MenuId> = std::sync::OnceLock::new();
    *ID.get_or_init(MenuId::fresh)
}

impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// The pickup click whose frozen stacks are `frozen`, decided by the mods and then run by
    /// `perform`, which is given their answer. `current` reads the same stacks again, for the
    /// check that nothing changed them. `probe_carried` runs the shared vanilla carried override
    /// (`vanilla_carried_override`) on a throwaway copy with the working stacks after any carried
    /// `ItemPart` declines, so `OtherStackedOnMe` only runs when the real carried override
    /// returned `false`. `route` names the menu clicked.
    ///
    /// With no listener on this side this is `perform(self, None)` and nothing else: no
    /// transaction, no decision, no allocation.
    pub(in crate::game) fn stacked_pickup<'p>(
        &mut self,
        route: MenuId,
        frozen: StackedClick,
        current: impl Fn(&Game<'_, __Side>) -> Option<StackedClick> + 'p,
        probe_carried: impl FnOnce(&mut Game<'_, __Side>, Option<ItemStack>, Option<ItemStack>) -> bool,
        perform: impl FnOnce(&mut Game<'_, __Side>, Option<&StackedOutcome>) + 'p,
    ) -> Pickup {
        let server = self.is_logical_server();
        if !stacked_listened_for_side(server) {
            perform(self, None);
            return Pickup::Ran;
        }
        let Some(epoch) = self.native_lifecycle.epoch() else {
            return Pickup::Refused;
        };
        let Some(entity) = frozen
            .player_entity_id
            .and_then(|id| u32::try_from(id).ok())
        else {
            return Pickup::Refused;
        };
        let actor = mod_api::EntityRef::new(epoch, entity);
        let identity = Identity {
            side: if server { Side::Server } else { Side::Client },
            world: epoch,
            actor,
            target: actor,
        };
        let shared = Rc::new(Shared {
            frozen,
            current: Box::new(current),
            perform: RefCell::new(Some(Box::new(perform))),
            answer: RefCell::new(None),
            refused: Cell::new(false),
        });
        let target = PickupTarget {
            id: route,
            shared: shared.clone(),
        };
        // The action's savepoint needs a transaction to nest in: the open one, or this one.
        let (outcome, ran) = self.storage_scope(|game, _| {
            let mut host = GameHost { game, identity };
            let fired = shared.clone();
            let outcome = decision::run(
                &mut host,
                &target,
                |_, _| PickupPlan,
                move |host| fire(host.game, &fired, probe_carried),
            );
            // A replacement the slot's rules refused is undone with everything the listeners wrote.
            let ran = matches!(outcome, Ok(Outcome::Applied)) && !shared.refused.get();
            ((outcome, ran), ran)
        });
        match outcome {
            Ok(Outcome::Applied) if ran => Pickup::Ran,
            Ok(Outcome::Applied) => {
                log::warn!("item_stacked_on_other: the menu refused the edit; nothing changed");
                Pickup::Refused
            }
            Ok(other) => {
                log::warn!(
                    "item_stacked_on_other: the click was refused ({other:?}); nothing changed"
                );
                Pickup::Refused
            }
            Err(admission) => {
                log::warn!(
                    "item_stacked_on_other: the click was not admitted ({admission:?}); nothing changed"
                );
                Pickup::Refused
            }
        }
    }
}

/// Whether any listener active on `server` side answers `item_stacked_on_other`.
pub(in crate::game) fn stacked_listened_for_side(server: bool) -> bool {
    (server && mod_api::__private::listened(mod_api::HookId::ItemStackedOnOther))
        || mod_api::__private::has_part_hook(mod_api::HookId::ItemStackedOnOther)
}

/// Whether any listener active on `server` side subscribes to `frozen`: lets clicks on
/// unrelated items skip the decision transaction.
pub(in crate::game) fn stacked_listens_for_side(server: bool, frozen: &StackedClick) -> bool {
    (server && mod_api::__private::listened(mod_api::HookId::ItemStackedOnOther))
        || frozen.carried.as_ref().is_some_and(|stack| {
            mod_api::__private::item_listens(mod_api::HookId::ItemStackedOnOther, &stack.item)
        })
        || frozen.clicked.as_ref().is_some_and(|stack| {
            mod_api::__private::item_listens(mod_api::HookId::ItemStackedOnOther, &stack.item)
        })
}

/// Whether any server-side listener (global `Hooks::item_stacked_on_other` or `ItemPart`)
/// answers `item_stacked_on_other`.
pub(in crate::game) fn stacked_listened() -> bool {
    stacked_listened_for_side(true)
}

/// Whether a server-side global listener or an `ItemPart` on either frozen stack subscribes to
/// `item_stacked_on_other`.
pub(in crate::game) fn stacked_listens_for(frozen: &StackedClick) -> bool {
    stacked_listens_for_side(true, frozen)
}

/// Fire the global hook (server side only) and item-part overrides in vanilla
/// `AbstractContainerMenu.tryItemClickBehaviourOverride` order, validating each phase's answer.
/// The accepted answer is kept for the click; any bad answer refuses it.
fn fire<__Side: crate::game::Side>(
    game: &mut Game<'_, __Side>,
    shared: &Shared<'_>,
    probe_carried: impl FnOnce(&mut Game<'_, __Side>, Option<ItemStack>, Option<ItemStack>) -> bool,
) -> Reply {
    let frozen = &shared.frozen;
    let make_event =
        |direction: mod_api::item::menu::StackedOnOtherDirection,
         carried: &Option<ItemStack>,
         clicked: &Option<ItemStack>|
         -> std::result::Result<mod_api::item::menu::StackedOnOther, InvalidReason> {
            let (Ok(carried), Ok(clicked)) = (carried.try_into_api(), clicked.try_into_api())
            else {
                return Err(InvalidReason::Plan);
            };
            Ok(mod_api::item::menu::StackedOnOther::new(
                frozen.player,
                frozen.player_entity_id,
                frozen.slot,
                frozen.container,
                frozen.container_slot,
                frozen.primary,
                carried,
                clicked,
            )
            .with_direction(direction))
        };
    let finish = |_game: &mut Game<'_, __Side>, outcome: StackedOutcome| -> Reply {
        *shared.answer.borrow_mut() = Some(outcome);
        Reply::Allow
    };

    // 1. Global NeoForge `ItemStackedOnOtherEvent`, authoritative server side only, before either
    // item override. Preserves the documented global listener contract unchanged.
    if game.is_logical_server() && mod_api::__private::listened(mod_api::HookId::ItemStackedOnOther)
    {
        let base = match make_event(
            mod_api::item::menu::StackedOnOtherDirection::Event,
            &frozen.carried,
            &frozen.clicked,
        ) {
            Ok(event) => event,
            Err(reason) => return Reply::Invalid(reason),
        };
        let mut result = StackedOnOtherResult::default();
        let mut canceled = false;
        game.native_item_stacked_on_other(&base, &mut result, &mut canceled);
        let outcome = match answer_global(game, canceled, &result) {
            Ok(outcome) => outcome,
            Err(reason) => return Reply::Invalid(reason),
        };
        if outcome.canceled {
            return finish(game, outcome);
        }
    }

    let mut working_carried = frozen.carried.clone();
    let mut working_clicked = frozen.clicked.clone();
    let mut carried_part_cursor = None;
    let mut carried_part_took = false;
    let mut clicked_part_cursor = None;
    let mut clicked_edit: Option<StackedClickedEdit> = None;
    let mut any_part_ran = false;

    let apply_clicked_part = |working: &mut Option<ItemStack>,
                              edit: &mut Option<StackedClickedEdit>,
                              next: PartClickedEdit| {
        match next {
            PartClickedEdit::Set(stack) => {
                *working = stack.clone();
                *edit = match edit.take() {
                    Some(StackedClickedEdit::Take(take)) => {
                        Some(StackedClickedEdit::TakeThenSet { take, then: stack })
                    }
                    _ => Some(StackedClickedEdit::Set(stack)),
                };
            }
            PartClickedEdit::Take(count) => {
                if let Some(cur) = working.as_mut() {
                    cur.count = cur.count.saturating_sub(count);
                    if cur.count == 0 {
                        *working = None;
                    }
                }
                *edit = match edit.take() {
                    Some(StackedClickedEdit::Take(first)) => {
                        Some(StackedClickedEdit::TakeThenTake {
                            first,
                            second: count,
                        })
                    }
                    Some(StackedClickedEdit::Set(Some(set))) => {
                        Some(StackedClickedEdit::SetThenTake { set, take: count })
                    }
                    None => Some(StackedClickedEdit::Take(count)),
                    Some(other) => Some(other),
                };
            }
        }
    };

    // 2. Carried stack's `Item.overrideStackedOnOther`.
    if let Some(carried_stack) = working_carried.clone()
        && mod_api::__private::item_listens(
            mod_api::HookId::ItemStackedOnOther,
            &carried_stack.item,
        )
    {
        any_part_ran = true;
        let event = match make_event(
            mod_api::item::menu::StackedOnOtherDirection::CarriedOnOther,
            &working_carried,
            &working_clicked,
        ) {
            Ok(event) => event,
            Err(reason) => return Reply::Invalid(reason),
        };
        let mut result = StackedOnOtherResult::default();
        let mut canceled = false;
        game.item_stacked_on_other_part(&carried_stack.item, &event, &mut result, &mut canceled);
        let part = match answer_part(game, canceled, &result, working_clicked.as_ref()) {
            Ok(part) => part,
            Err(reason) => return Reply::Invalid(reason),
        };
        carried_part_cursor = part.carried;
        if let Some(next_carried) = &carried_part_cursor {
            working_carried = next_carried.clone();
        }
        if let Some(next_clicked) = part.clicked {
            carried_part_took = matches!(next_clicked, PartClickedEdit::Take(_));
            apply_clicked_part(&mut working_clicked, &mut clicked_edit, next_clicked);
        }
        if part.handled {
            return finish(
                game,
                StackedOutcome {
                    canceled: true,
                    handled: true,
                    policy: StackedEditPolicy::Part,
                    resume: StackedResume::Done,
                    carried_part_cursor,
                    carried_part_took,
                    clicked: clicked_edit,
                    ..StackedOutcome::default()
                },
            );
        }
    }

    // 3. Real vanilla carried override (`BundleItem.overrideStackedOnOther`): run through the
    // single shared implementation before deciding whether the clicked ItemPart should fire.
    let vanilla_carried_takes =
        probe_carried(game, working_carried.clone(), working_clicked.clone());
    if vanilla_carried_takes {
        return finish(
            game,
            StackedOutcome {
                policy: if any_part_ran {
                    StackedEditPolicy::Part
                } else {
                    StackedEditPolicy::Global
                },
                resume: StackedResume::VanillaCarried,
                carried_part_cursor,
                carried_part_took,
                clicked: clicked_edit,
                ..StackedOutcome::default()
            },
        );
    }

    // 4. Clicked stack's `Item.overrideOtherStackedOnMe`.
    if let Some(clicked_stack) = working_clicked.clone()
        && mod_api::__private::item_listens(
            mod_api::HookId::ItemStackedOnOther,
            &clicked_stack.item,
        )
    {
        any_part_ran = true;
        let event = match make_event(
            mod_api::item::menu::StackedOnOtherDirection::OtherStackedOnMe,
            &working_carried,
            &working_clicked,
        ) {
            Ok(event) => event,
            Err(reason) => return Reply::Invalid(reason),
        };
        let mut result = StackedOnOtherResult::default();
        let mut canceled = false;
        game.item_stacked_on_other_part(&clicked_stack.item, &event, &mut result, &mut canceled);
        let part = match answer_part(game, canceled, &result, working_clicked.as_ref()) {
            Ok(part) => part,
            Err(reason) => return Reply::Invalid(reason),
        };
        clicked_part_cursor = part.carried;
        if let Some(next_clicked) = part.clicked {
            apply_clicked_part(&mut working_clicked, &mut clicked_edit, next_clicked);
        }
        if part.handled {
            return finish(
                game,
                StackedOutcome {
                    canceled: true,
                    handled: true,
                    policy: StackedEditPolicy::Part,
                    resume: StackedResume::Done,
                    carried: clicked_part_cursor,
                    carried_part_cursor,
                    carried_part_took,
                    clicked: clicked_edit,
                    ..StackedOutcome::default()
                },
            );
        }
    }

    finish(
        game,
        StackedOutcome {
            policy: if any_part_ran {
                StackedEditPolicy::Part
            } else {
                StackedEditPolicy::Global
            },
            resume: StackedResume::VanillaClicked,
            carried: clicked_part_cursor,
            carried_part_cursor,
            carried_part_took,
            clicked: clicked_edit,
            ..StackedOutcome::default()
        },
    )
}

/// The global listener's typed answer, validated once: a stack may be set only by a cancelled
/// click that is also handled, `ClickedStackResult::Take` is refused, and a set stack must be a
/// valid one (a known item, a count within its maximum).
fn answer_global<__Side: crate::game::Side>(
    game: &Game<'_, __Side>,
    canceled: bool,
    result: &StackedOnOtherResult,
) -> std::result::Result<StackedOutcome, InvalidReason> {
    if matches!(result.clicked, ClickedStackResult::Take(_)) {
        return Err(InvalidReason::Plan);
    }
    let carried_set = matches!(result.carried, StackResult::Set(_));
    let clicked_set = matches!(result.clicked, ClickedStackResult::Set(_));
    if !(canceled && result.handled) && (carried_set || clicked_set) {
        return Err(InvalidReason::Plan);
    }
    let items = game.items();
    let carried = match &result.carried {
        StackResult::Keep => None,
        StackResult::Set(stack) => Some(
            stack
                .try_into_engine(items)
                .map_err(|_| InvalidReason::Count)?,
        ),
    };
    let clicked = match &result.clicked {
        ClickedStackResult::Keep => None,
        ClickedStackResult::Set(stack) => Some(StackedClickedEdit::Set(
            stack
                .try_into_engine(items)
                .map_err(|_| InvalidReason::Count)?,
        )),
        ClickedStackResult::Take(_) => return Err(InvalidReason::Plan),
    };
    let resume = if !canceled {
        StackedResume::VanillaAll
    } else if result.handled {
        StackedResume::Done
    } else {
        StackedResume::Plain
    };
    Ok(StackedOutcome {
        canceled,
        handled: result.handled,
        policy: StackedEditPolicy::Global,
        resume,
        carried,
        clicked,
        ..StackedOutcome::default()
    })
}

enum PartClickedEdit {
    Set(Option<ItemStack>),
    Take(u32),
}

struct PartAnswer {
    handled: bool,
    carried: Option<Option<ItemStack>>,
    clicked: Option<PartClickedEdit>,
}

/// An `ItemPart` override's typed answer, representing both vanilla outcomes inside the
/// transaction:
/// - Handled (stop): `Flow::Cancel` with `result.handled == true` (optional stack edits).
/// - Not handled (continue): `Flow::Continue` with `result.handled == false` (optional stack
///   edits visible to later overrides and plain pickup).
///
/// Contradictory combinations (`canceled != result.handled`), `ClickedStackResult::Take` exceeding
/// the clicked stack (or on an empty slot), or invalid stacks refuse the click.
fn answer_part<__Side: crate::game::Side>(
    game: &Game<'_, __Side>,
    canceled: bool,
    result: &StackedOnOtherResult,
    current_clicked: Option<&ItemStack>,
) -> std::result::Result<PartAnswer, InvalidReason> {
    if canceled != result.handled {
        return Err(InvalidReason::Plan);
    }
    let items = game.items();
    let carried = match &result.carried {
        StackResult::Keep => None,
        StackResult::Set(stack) => Some(
            stack
                .try_into_engine(items)
                .map_err(|_| InvalidReason::Count)?,
        ),
    };
    let clicked = match &result.clicked {
        ClickedStackResult::Keep => None,
        ClickedStackResult::Set(stack) => Some(PartClickedEdit::Set(
            stack
                .try_into_engine(items)
                .map_err(|_| InvalidReason::Count)?,
        )),
        ClickedStackResult::Take(count) => {
            let Some(current) = current_clicked else {
                return Err(InvalidReason::Plan);
            };
            if *count == 0 || *count > current.count {
                return Err(InvalidReason::Count);
            }
            Some(PartClickedEdit::Take(*count))
        }
    };
    Ok(PartAnswer {
        handled: result.handled,
        carried,
        clicked,
    })
}
