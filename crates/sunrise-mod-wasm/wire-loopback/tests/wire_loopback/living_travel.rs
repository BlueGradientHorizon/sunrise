//! `living_travel` crosses to the guest and back with its decision intact
//! (#2414): the same hook the native listener sees, through the generator's
//! own wire proxies — no second route.
use super::*;

thread_local! {
    static HEARD: std::cell::RefCell<Option<(u32, String, mod_api::TravelMode, [f64; 3], [f64; 3])>> =
        const { std::cell::RefCell::new(None) };
}

/// What the guest's generated `living_travel` listener heard on
/// its last call: entity, kind, mode, and the input and velocity as they
/// arrived.
pub(super) fn take_heard() -> Option<(u32, String, mod_api::TravelMode, [f64; 3], [f64; 3])> {
    HEARD.with_borrow_mut(std::mem::take)
}

/// The guest side of the loopback answers `living_travel` with the flow the
/// test set in `ANSWER`, replacing the input and velocity when it says so.
pub(super) enum GuestAnswer {
    Continue,
    Cancel,
    Replace { input: [f64; 3], velocity: [f64; 3] },
}
thread_local! {
    static ANSWER: std::cell::RefCell<Option<GuestAnswer>> = const { std::cell::RefCell::new(None) };
}

pub(super) fn set_answer(answer: GuestAnswer) {
    ANSWER.with_borrow_mut(|slot| *slot = Some(answer));
}

pub(super) fn guest_listener(
    _: &dyn LoopContext,
    event: &mut mod_api::LivingTravelEvent<'_>,
) -> mod_api::Flow {
    HEARD.with_borrow_mut(|heard| {
        *heard = Some((
            event.entity,
            event.kind.to_owned(),
            event.mode,
            *event.input,
            *event.velocity,
        ))
    });
    match ANSWER.with_borrow_mut(std::mem::take) {
        None => mod_api::Flow::Continue,
        Some(GuestAnswer::Continue) => mod_api::Flow::Continue,
        Some(GuestAnswer::Cancel) => mod_api::Flow::Cancel,
        Some(GuestAnswer::Replace { input, velocity }) => {
            *event.input = input;
            *event.velocity = velocity;
            mod_api::Flow::Continue
        }
    }
}

/// One guest dispatch, by the argument shape the generator's own proxy uses
/// (`WasmHandler::living_travel` in `wire_host_generated.rs`): the fields
/// cross one by one, and the answer returns the flow with the written-back
/// fields.
fn call_guest(
    module: &WasmMod,
    event: &mut mod_api::LivingTravelEvent<'_>,
) -> (mod_api::Flow, [f64; 3], [f64; 3]) {
    let id = boundary::export_id("Hooks", "living_travel").unwrap();
    let (flow, input, velocity) = module
        .call(
            CallClass::Hook,
            boundary::Frame::default(),
            id,
            None,
            |w| {
                w.put(&event.entity);
                w.put(event.kind);
                w.put(&event.mode);
                w.put(&*event.input);
                w.put(&*event.velocity);
                w.put(&event.cancelled);
            },
            |r| {
                let flow: mod_api::Flow = r.take()?;
                let _entity: u32 = r.take()?;
                let _mode: mod_api::TravelMode = r.take()?;
                let _cancelled: bool = r.take()?;
                let input: [f64; 3] = r.take()?;
                let velocity: [f64; 3] = r.take()?;
                Ok((flow, input, velocity))
            },
        )
        .unwrap();
    (flow, input, velocity)
}

#[test]
fn wire_loopback_living_travel_carries_the_guests_decision() {
    mod_api::__private::initialize().unwrap();
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let module = WasmMod {
        group: 0,
        subscribed: &[],
        listeners: &[],
        corrupt: false,
    };
    let context = Context;
    let kind = "minecraft:zombie";
    for (answer, expected_flow, expected_input, expected_velocity) in [
        (
            GuestAnswer::Continue,
            mod_api::Flow::Continue,
            [0.216, 0.0, 0.0],
            [1.0, -0.078_4, 0.0],
        ),
        (
            GuestAnswer::Replace {
                input: [-1.0, 0.0, 0.0],
                velocity: [64.0, 0.0, 0.0],
            },
            mod_api::Flow::Continue,
            [-1.0, 0.0, 0.0],
            [64.0, 0.0, 0.0],
        ),
        (
            GuestAnswer::Cancel,
            mod_api::Flow::Cancel,
            [0.216, 0.0, 0.0],
            [1.0, -0.078_4, 0.0],
        ),
    ] {
        set_answer(answer);
        let mut input = [0.216, 0.0, 0.0];
        let mut velocity = [1.0, -0.078_4, 0.0];
        let mut event = mod_api::LivingTravelEvent {
            entity: 7,
            kind,
            mode: mod_api::TravelMode::Water,
            input: &mut input,
            velocity: &mut velocity,
            cancelled: false,
        };
        let (flow, got_input, got_velocity) = call_guest(&module, &mut event);
        assert_eq!(flow, expected_flow);
        assert_eq!(got_input, expected_input);
        assert_eq!(got_velocity, expected_velocity);
        assert_eq!(
            take_heard(),
            Some((
                7,
                kind.to_owned(),
                mod_api::TravelMode::Water,
                [0.216, 0.0, 0.0],
                [1.0, -0.078_4, 0.0]
            )),
            "the guest heard the entity, kind, mode and resolved values"
        );
        let _ = &context;
    }
}

/// A native listener that replaces every travel's input and velocity: the
/// earlier listener of the trap test below.
struct NativeReplacer;

#[mod_api::__subscriptions]
impl mod_api::Hooks for NativeReplacer {
    fn living_travel(
        &self,
        _: &dyn LoopContext,
        event: &mut mod_api::LivingTravelEvent<'_>,
    ) -> mod_api::Flow {
        *event.input = [0.5, 0.0, 0.0];
        *event.velocity = [0.0, 0.0, 2.0];
        mod_api::Flow::Continue
    }
}

/// A native listener that only counts its calls: the later listener of the
/// trap test, which must not run on the trapped invocation.
struct NativeTally(std::sync::Arc<std::sync::atomic::AtomicUsize>);

#[mod_api::__subscriptions]
impl mod_api::Hooks for NativeTally {
    fn living_travel(
        &self,
        _: &dyn LoopContext,
        _: &mut mod_api::LivingTravelEvent<'_>,
    ) -> mod_api::Flow {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        mod_api::Flow::Continue
    }
}

/// A wasm listener whose call fails after an earlier listener replaced the
/// values (the round-1 review requirement, as for `entity_move`):
/// `APPEND_ATTACK` 3 stands in for a trapped or refused guest call — the
/// reply comes back short, decodes as nothing, and the generated proxy sees
/// `None`. The isolated dispatch gives it the same treatment a native panic
/// gets: the **resolved** input and velocity are restored — not the
/// replacer's — the trapped listener is switched off for this hook alone,
/// and no later listener runs on that invocation.
#[test]
fn a_trapped_wasm_listener_after_a_replacement_keeps_the_resolved_values() {
    mod_api::__private::initialize().unwrap();
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mods = [
        mod_api::ModEntry::new("replacer", Box::new(NativeReplacer)),
        mod_api::ModEntry::new(
            "wire_test",
            Box::new(WasmMod {
                group: 0,
                subscribed: &[Some(mod_api::HookId::LivingTravel)],
                listeners: &[],
                corrupt: false,
            }),
        ),
        mod_api::ModEntry::new("tally", Box::new(NativeTally(calls.clone()))),
    ];
    mod_api::testing::with_mods(std::sync::Arc::from(mods), || {
        let dispatch = |input: &mut [f64; 3], velocity: &mut [f64; 3], canceled: &mut bool| {
            mod_api::dispatch_living_travel(
                &Context,
                7,
                "minecraft:zombie",
                mod_api::TravelMode::Air,
                input,
                velocity,
                canceled,
            );
        };
        let (mut input, mut velocity, mut canceled) =
            ([0.216, 0.0, 0.0], [1.0, -0.078_4, 0.0], false);
        // The guest's reply loses its last byte: as a trapped or refused
        // call, nothing decodes.
        APPEND_ATTACK.set(3);
        dispatch(&mut input, &mut velocity, &mut canceled);
        APPEND_ATTACK.set(0);
        assert_eq!(
            input,
            [0.216, 0.0, 0.0],
            "the resolved input survived the trap, not the replacer's [0.5, 0.0, 0.0]"
        );
        assert_eq!(
            velocity,
            [1.0, -0.078_4, 0.0],
            "the resolved velocity survived the trap, not the replacer's [0.0, 0.0, 2.0]"
        );
        assert!(!canceled, "a trapped listener cancels nothing");
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::Relaxed),
            0,
            "no listener after the trapped one ran on that invocation"
        );
        dispatch(&mut input, &mut velocity, &mut canceled);
        assert_eq!(
            input,
            [0.5, 0.0, 0.0],
            "the trapped listener is off: the replacer's input stands again"
        );
        assert_eq!(velocity, [0.0, 0.0, 2.0]);
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::Relaxed),
            1,
            "the later listener runs once the trapped one is switched off"
        );
    });
}

pub(super) struct LivingTravelHooks;
impl LivingTravelHooks {
    pub(super) fn living_travel(
        &self,
        context: &dyn LoopContext,
        event: &mut mod_api::LivingTravelEvent<'_>,
    ) -> mod_api::Flow {
        guest_listener(context, event)
    }
}
