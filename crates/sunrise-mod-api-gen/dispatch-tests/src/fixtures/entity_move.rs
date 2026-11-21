//! The global cancel dispatcher keeps accepted outputs across listener failures.

/// What the fixture listener heard: each requested delta and cancel flag.
type Heard = Vec<([f64; 3], bool)>;

struct Mover {
    answer: Answer,
    heard: Arc<Mutex<Heard>>,
}

#[derive(Clone, Copy)]
enum Answer {
    Replace([f64; 3]),
    Panic,
    Refuse,
    Invalid,
    Cancel,
    Continue,
}

#[mod_api::__subscriptions]
impl Hooks for Mover {
    fn entity_move(&self, _: &dyn LoopContext, event: &mut EntityMoveEvent<'_>) -> Flow {
        self.heard
            .lock()
            .unwrap()
            .push((*event.delta, event.cancelled));
        match self.answer {
            Answer::Replace(delta) => *event.delta = delta,
            Answer::Panic => {
                *event.delta = [9.0; 3];
                event.cancelled = true;
                panic!("the fixture's move listener unwinds")
            }
            Answer::Refuse => {
                *event.delta = [8.0; 3];
                event.cancelled = true;
                std::panic::resume_unwind(Box::new(runtime::Refusal(
                    "unavailable fixture operation",
                )))
            }
            Answer::Invalid => {
                *event.delta = [f64::NAN, 0.0, 0.0];
                event.cancelled = true;
                return Flow::Cancel;
            }
            Answer::Cancel => return Flow::Cancel,
            Answer::Continue => {}
        }
        Flow::Continue
    }
}

fn mover(id: &'static str, answer: Answer) -> (Arc<Mutex<Heard>>, ModEntry) {
    let heard = Arc::new(Mutex::new(Vec::new()));
    let entry = ModEntry::new(
        id,
        Box::new(Mover {
            answer,
            heard: Arc::clone(&heard),
        }),
    );
    (heard, entry)
}

fn dispatch(mut delta: [f64; 3]) -> ([f64; 3], bool) {
    let mut canceled = false;
    mod_api::dispatch_entity_move(
        &Loop("server"),
        7,
        "minecraft:zombie",
        MoveCause::SelfMovement,
        &mut delta,
        &mut canceled,
    );
    (delta, canceled)
}

#[test]
fn a_panicking_move_listener_loses_its_proposal_but_the_walk_continues() {
    let accepted = [3.0, 4.0, 5.0];
    let (first_heard, first) = mover("accepted", Answer::Replace(accepted));
    let (panic_heard, panicker) = mover("panicker", Answer::Panic);
    let (later_heard, later) = mover("later", Answer::Continue);
    install(vec![first, panicker, later].into());

    assert_eq!(dispatch([1.0, 0.0, 0.0]), (accepted, false));
    assert_eq!(*later_heard.lock().unwrap(), [(accepted, false)]);
    assert_eq!(dispatch([2.0, 0.0, 0.0]), (accepted, false));
    assert_eq!(
        *first_heard.lock().unwrap(),
        [([1.0, 0.0, 0.0], false), ([2.0, 0.0, 0.0], false)]
    );
    assert_eq!(*panic_heard.lock().unwrap(), [(accepted, false)]);
    assert_eq!(*later_heard.lock().unwrap(), [(accepted, false); 2]);
    assert!(runtime::poisoned(HookId::EntityMove, "panicker"));
    assert!(!runtime::poisoned(HookId::EntityMove, "accepted"));
    assert!(!runtime::poisoned(HookId::EntityMove, "later"));
}

#[test]
fn a_failed_receiver_cannot_clear_an_accepted_move_veto() {
    struct ClearThenPanic(Arc<Mutex<Heard>>);
    #[mod_api::__subscriptions]
    impl Hooks for ClearThenPanic {
        #[listener(receive_cancelled)]
        fn entity_move(&self, _: &dyn LoopContext, event: &mut EntityMoveEvent<'_>) -> Flow {
            self.0.lock().unwrap().push((*event.delta, event.cancelled));
            event.cancelled = false;
            *event.delta = [9.0; 3];
            panic!("the veto stands")
        }
    }

    let (_, veto) = mover("veto", Answer::Cancel);
    let heard = Arc::new(Mutex::new(Vec::new()));
    let failed = ModEntry::new("failed_receiver", Box::new(ClearThenPanic(heard.clone())));
    let (skipped_heard, skipped) = mover("skipped", Answer::Continue);
    install(vec![veto, failed, skipped].into());
    assert_eq!(dispatch([1.0; 3]), ([1.0; 3], true));
    assert_eq!(*heard.lock().unwrap(), [([1.0; 3], true)]);
    assert!(runtime::poisoned(HookId::EntityMove, "failed_receiver"));
    assert!(skipped_heard.lock().unwrap().is_empty());
}

#[test]
fn a_move_phase_refusal_restores_the_proposal_and_is_reported_on_each_call() {
    let accepted = [3.0, 4.0, 5.0];
    let (_, first) = mover("accepted", Answer::Replace(accepted));
    let (refused_heard, refused) = mover("refused", Answer::Refuse);
    let (later_heard, later) = mover("later", Answer::Continue);
    install(vec![first, refused, later].into());

    runtime::OUTSIDE_WORLD.set(true);
    let outputs = [dispatch([1.0; 3]), dispatch([2.0; 3])];
    runtime::OUTSIDE_WORLD.set(false);
    assert_eq!(outputs, [(accepted, false); 2]);
    assert_eq!(*refused_heard.lock().unwrap(), [(accepted, false); 2]);
    assert_eq!(*later_heard.lock().unwrap(), [(accepted, false); 2]);
    assert!(!runtime::poisoned(HookId::EntityMove, "refused"));
    assert_eq!(
        runtime::REFUSALS.with(|reports| std::mem::take(&mut *reports.borrow_mut())),
        ["unavailable fixture operation"; 2]
    );
}

#[test]
fn an_invalid_move_discards_its_delta_and_veto_and_reports_each_invocation() {
    let delta = [3.0, 4.0, 5.0];
    let (_, first) = mover("accepted", Answer::Replace(delta));
    let (invalid_heard, invalid) = mover("invalid", Answer::Invalid);
    let (later_heard, later) = mover("later", Answer::Continue);
    install(vec![first, invalid, later].into());

    let warnings = capture_warnings(|| {
        assert_eq!(dispatch([1.0; 3]), (delta, false));
        assert_eq!(dispatch([2.0; 3]), (delta, false));
    });
    assert_eq!(
        warnings,
        ["mod invalid hook entity_move output delta refused: its outputs are dropped and the preceding ones stand";
            2]
    );
    assert_eq!(*invalid_heard.lock().unwrap(), [(delta, false); 2]);
    assert_eq!(*later_heard.lock().unwrap(), [(delta, false); 2]);
    assert!(!runtime::poisoned(HookId::EntityMove, "invalid"));
}

mod root {
    mod runtime {
        pub(super) struct Refusal(pub(super) &'static str);
        impl Refusal {
            pub(super) fn report(&self) {
                REFUSALS.with(|reports| reports.borrow_mut().push(self.0));
            }
        }

        pub(super) enum DispatchOutcome<T> {
            Ok(T),
            Refused(Refusal),
            Panicked,
        }

        pub(super) fn outside_world() -> bool {
            OUTSIDE_WORLD.get()
        }
        pub(super) fn refusable<T>(
            contained: bool,
            call: impl FnOnce() -> T,
        ) -> Result<T, Refusal> {
            if !contained {
                return Ok(call());
            }
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(call)) {
                Ok(value) => Ok(value),
                Err(payload) => match payload.downcast::<Refusal>() {
                    Ok(refusal) => Err(*refusal),
                    Err(payload) => std::panic::resume_unwind(payload),
                },
            }
        }
        pub(super) fn invoke<T>(
            hook: super::HookId,
            _: &'static str,
            mod_id: &'static str,
            contained: bool,
            call: impl FnOnce() -> T,
        ) -> DispatchOutcome<T> {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                refusable(contained, call)
            })) {
                Ok(Ok(value)) => DispatchOutcome::Ok(value),
                Ok(Err(refusal)) => DispatchOutcome::Refused(refusal),
                Err(_) => {
                    poison(hook, mod_id);
                    DispatchOutcome::Panicked
                }
            }
        }

        pub(super) fn listening_kind_owner(
            hook: super::HookId,
            kind: &str,
        ) -> Option<&'static super::ModEntry> {
            let (namespace, _) = kind.split_once(':')?;
            super::listening(hook)
                .map(|(_, entry)| entry)
                .find(|entry| entry.id == namespace)
        }

        pub(super) fn entered_with_id_matching<'a>(
            hook_id: super::HookId,
            hook: &'static str,
            mut allow: impl FnMut(&str) -> bool + 'a,
        ) -> impl Iterator<Item = (&'static str, &'static dyn super::Hooks)> + 'a {
            super::listening(hook_id).filter_map(move |(_, entry)| {
                if !allow(entry.id) {
                    return None;
                }
                let ((), hooks) = super::enter_listener(entry, hook);
                Some((entry.id, hooks))
            })
        }

        pub(super) struct ListenerScope;
        impl ListenerScope {
            pub(super) fn new() -> Self {
                Self
            }
            pub(super) fn enter(
                &mut self,
                entry: &'static super::ModEntry,
                hook: &'static str,
            ) -> &'static dyn super::Hooks {
                super::enter_listener(entry, hook).1
            }
        }
        thread_local! {
            pub(super) static OUTSIDE_WORLD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
            pub(super) static REFUSALS: std::cell::RefCell<Vec<&'static str>> = const { std::cell::RefCell::new(Vec::new()) };
            /// The listeners dispatch poisoned, as (hook ordinal, id) pairs — the stand-in
            /// for the real runtime's per-hook sets.
            pub(super) static POISONED: std::cell::RefCell<std::collections::HashSet<(usize, &'static str)>> =
                std::cell::RefCell::new(std::collections::HashSet::new());
        }
        pub(super) fn poison(hook: super::HookId, id: &'static str) {
            POISONED.with(|poisoned| poisoned.borrow_mut().insert((hook as usize, id)));
        }
        pub(super) fn poisoned(hook: super::HookId, id: &str) -> bool {
            POISONED.with(|poisoned| {
                poisoned
                    .borrow()
                    .iter()
                    .any(|(poisoned_hook, poisoned_id)| {
                        *poisoned_hook == hook as usize && *poisoned_id == id
                    })
            })
        }
    }
}
