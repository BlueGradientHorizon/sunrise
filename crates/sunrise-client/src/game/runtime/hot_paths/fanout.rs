//! Fan-out over native dispatch, with one counted call per subscribed mod.
use super::{Game, Recorder};
use std::{cell::Cell, num::NonZeroU16};
use sunrise_client::Result;

const CALLS: u64 = 100_000;

thread_local! {
    // Callbacks and measurements run on the same server thread. Counting does not allocate
    // or add a contended atomic operation to any listener.
    static LISTENER_CALLS: Cell<u64> = const { Cell::new(0) };
}

/// Observable work performed by each of the benchmark executable's fan-out listeners.
#[doc(hidden)]
pub fn note_hot_paths_listener_call() {
    LISTENER_CALLS.set(LISTENER_CALLS.get() + 1);
}

/// Which native benchmark listeners the executable is asked to install for this process.
#[doc(hidden)]
pub enum HotPathsMods {
    None,
    Native,
    Fanout(NonZeroU16),
}

impl HotPathsMods {
    pub fn from_env() -> Result<Self> {
        let mods = std::env::var("SUNRISE_HOT_PATHS_MODS").unwrap_or_default();
        if mods == "native" {
            Ok(Self::Native)
        } else if let Some(count) = mods.strip_prefix("fanout-") {
            Ok(Self::Fanout(count.parse().map_err(|error| {
                format!("invalid SUNRISE_HOT_PATHS_MODS={mods}: {error}")
            })?))
        } else {
            Ok(Self::None)
        }
    }
}

impl Game<'_> {
    pub(super) fn hot_hook_fanout(
        &mut self,
        recorder: &mut Recorder,
        listeners: NonZeroU16,
    ) -> Result<()> {
        let entity_tick = format!("mod.hook.fanout.entity_tick.{listeners}");
        let server_started = format!("mod.hook.fanout.server_started.{listeners}");
        if recorder.wants(&entity_tick) {
            self.hot_level(5, 2)?;
            self.hot_mobs(128, 2)?;
            let dimension = self.dim.id().to_owned();
            let entities: Vec<_> = self.mobs.iter().map(|mob| mob.id).collect();
            let mut canceled = Vec::new();
            measure(recorder, &entity_tick, listeners, || {
                for call in 0..CALLS as usize {
                    self.native_entity_tick_pre(
                        &dimension,
                        std::slice::from_ref(&entities[call % entities.len()]),
                        &["minecraft:cow"],
                        &mut canceled,
                    );
                    canceled.clear();
                }
            })?;
        }
        if recorder.wants(&server_started) {
            self.hot_level(5, 2)?;
            // One notification per successful server start in play. Repeat just the
            // notification to measure dispatch, not the cost of opening a world.
            measure(recorder, &server_started, listeners, || {
                for _ in 0..CALLS {
                    self.native_server_started(true);
                }
            })?;
        }
        Ok(())
    }
}

fn measure(
    recorder: &mut Recorder,
    name: &str,
    listeners: NonZeroU16,
    dispatch: impl FnOnce(),
) -> Result<()> {
    LISTENER_CALLS.set(0);
    recorder.measure(name, CALLS, || {
        dispatch();
        LISTENER_CALLS.get()
    })?;
    let observed = LISTENER_CALLS.get();
    let expected = u64::from(listeners.get()) * CALLS;
    if observed != expected {
        return Err(format!("{name}: expected {expected} listener calls, got {observed}").into());
    }
    Ok(())
}
