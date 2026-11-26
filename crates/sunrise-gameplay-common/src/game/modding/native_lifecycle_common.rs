//! Shared lifecycle wrappers and world-close cleanup shared across server and client roles.
use super::scheduled_tasks::ScheduledTasks;
use mod_api::{Authority, LifecycleContext, ServerTickPhase, TickId, WorldEpoch};
use std::sync::atomic::{AtomicU64, Ordering};

pub trait NativeLifecycleCommonGameExt1<'__sg>: crate::game::SideGame<'__sg> {
    fn finish_world_closed_common(&mut self) {
        self.attachment_indexes.clear();
        self.passages.retire_world();
        self.resident_entities.table.get_mut().reset();
        crate::game::cauldron_interactions::clear();
        self.clear_world_parts();
    }

    fn native_lifecycle_open_started(&mut self) {
        self.side_lifecycle_open_started();
    }

    fn native_lifecycle_world_ready(&mut self) {
        self.side_lifecycle_world_ready();
    }

    fn native_lifecycle_world_closed(&mut self) {
        self.side_lifecycle_world_closed();
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> NativeLifecycleCommonGameExt1<'__sg> for G {}

/// Epoch allocation spans server and client `Game` instances and client level
/// replacements, so the counter is process-wide.
pub(in crate::game) static NEXT_EPOCH: AtomicU64 = AtomicU64::new(1);

/// A process-wide epoch no handle has carried yet: a server open's, or a
/// client level's (`native_entity_refs.rs`), so a handle never crosses sides.
pub(in crate::game) fn fresh_epoch() -> WorldEpoch {
    WorldEpoch::new(NEXT_EPOCH.fetch_add(1, Ordering::Relaxed))
}

/// Internal state for deciding which events can fire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenState {
    Opening,
    Ready,
    Closing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenAttempt {
    pub authority: Authority,
    pub(in crate::game) epoch: u64,
    pub session: OpenState,
    pub(in crate::game) ticks: u64,
    /// A tick started and its end has not fired (or the tick errored).
    pub(in crate::game) tick_open: bool,
    pub(in crate::game) tick_world_running: bool,
}

impl OpenAttempt {
    pub fn context(&self) -> LifecycleContext {
        LifecycleContext::from(mod_api::__private::built::LifecycleContext {
            authority: self.authority,
            epoch: WorldEpoch::new(self.epoch),
            tick: TickId::new(self.ticks),
        })
    }

    pub(in crate::game) fn server_context(&self) -> Option<LifecycleContext> {
        (self.authority != Authority::RemoteClient).then(|| self.context())
    }
}

/// The lifecycle bookkeeping: the current attempt's epoch, session phase
/// and non-paused tick count. It only decides whether a phase fires; every
/// hook body is empty, so with no native mods nothing changes.
#[derive(Default)]
pub struct NativeLifecycle {
    pub attempt: Option<OpenAttempt>,
    pub read_scope: bool,
    /// The mods' delayed tasks (`schedule_task`): dropped whenever a world opens or closes.
    pub tasks: ScheduledTasks,
    /// The section `entity_entering_section` files each entity under, for this epoch's world.
    pub entity_sections: super::entity_sections::EntitySections,
    pub section_observer: super::entity_sections::observer_cache::ObserverCache,
    pub(in crate::game) player_section_observer:
        Option<std::rc::Rc<super::entity_sections::PlayerSectionObserver>>,
    /// Tossed item entities `item_toss` is still asking about, innermost last: a join flush
    /// stops before them, so a cancel never takes back an entity already announced.
    pub held_joins: Vec<u32>,
    /// Item notices waiting for their join decision.
    pub pending_item_spawns: Vec<(u32, String, i32, [f64; 3])>,
    /// The log has named the entity id exhaustion (`entity_ids_exhausted`).
    pub ids_exhausted_told: bool,
}

impl NativeLifecycle {
    /// A read is valid only during a tick hook with its current tick context.
    pub fn permits_read(&self, context: &LifecycleContext) -> bool {
        self.read_scope
            && self
                .attempt
                .as_ref()
                .is_some_and(|a| a.session == OpenState::Ready && a.context() == *context)
    }

    /// The epoch of the world open now, from the start of its open to the end
    /// of its close: what a mod's entity and level handles carry.
    pub fn epoch(&self) -> Option<WorldEpoch> {
        self.attempt.map(|a| WorldEpoch::new(a.epoch))
    }

    /// The open starts: a fresh process-wide epoch, never reused.
    pub fn begin_open(&mut self, authority: Authority) {
        self.attempt = Some(OpenAttempt {
            authority,
            epoch: fresh_epoch().get(),
            session: OpenState::Opening,
            ticks: 0,
            tick_open: false,
            tick_world_running: false,
        });
        self.tasks.clear();
    }

    pub fn world_open_ready(&mut self) -> Option<LifecycleContext> {
        let attempt = self.attempt.as_mut()?;
        if attempt.session != OpenState::Opening {
            return None;
        }
        attempt.session = OpenState::Ready;
        attempt.server_context()
    }

    pub fn is_opening(&self) -> bool {
        self.attempt
            .as_ref()
            .is_some_and(|attempt| attempt.session == OpenState::Opening)
    }

    /// `WorldOpenFailed`: only while opening; the attempt is invalidated.
    pub fn world_open_failed(&mut self) -> Option<LifecycleContext> {
        let attempt = self.attempt.as_mut()?;
        if attempt.session != OpenState::Opening {
            return None;
        }
        let context = attempt.server_context();
        self.attempt = None;
        self.tasks.clear();
        context
    }

    pub fn begin_close(&mut self) {
        if let Some(attempt) = self.attempt.as_mut() {
            attempt.session = OpenState::Closing;
        }
    }

    /// A failure after readiness retires the epoch, including during shutdown.
    pub fn world_close_failed(&mut self) -> Option<LifecycleContext> {
        let attempt = self.attempt.as_ref()?;
        if attempt.session == OpenState::Opening {
            return None;
        }
        let context = attempt.server_context();
        self.attempt = None;
        self.tasks.clear();
        context
    }

    /// `WorldCloseFinal`: at most once per epoch; the epoch retires.
    pub fn world_close_final(&mut self) -> Option<LifecycleContext> {
        let attempt = self.attempt.as_mut()?;
        if attempt.session != OpenState::Closing {
            return None;
        }
        let context = attempt.server_context();
        self.attempt = None;
        self.tasks.clear();
        context
    }

    /// Non-paused ready ticks only; a paused pass observes nothing, does not count.
    pub fn server_tick_start(
        &mut self,
        world_running: bool,
    ) -> Option<(LifecycleContext, ServerTickPhase)> {
        let attempt = self.attempt.as_mut()?;
        if attempt.session != OpenState::Ready || attempt.authority == Authority::RemoteClient {
            return None;
        }
        attempt.ticks += 1;
        // "Now" for the mods' delayed tasks moves before any hook of this tick runs.
        self.tasks.begin_tick();
        attempt.tick_open = true;
        attempt.tick_world_running = world_running;
        Some((
            attempt.context(),
            ServerTickPhase::ServerTickStart { world_running },
        ))
    }

    /// Discard the pending end when a tick fails.
    pub fn cancel_server_tick(&mut self) {
        if let Some(attempt) = &mut self.attempt {
            attempt.tick_open = false;
        }
    }

    /// Pairs the last start; an errored tick never reaches it.
    pub fn server_tick_end(&mut self, paused: bool) -> Option<(LifecycleContext, ServerTickPhase)> {
        let attempt = self.attempt.as_mut()?;
        if paused || attempt.session != OpenState::Ready || !attempt.tick_open {
            return None;
        }
        attempt.tick_open = false;
        Some((
            attempt.context(),
            ServerTickPhase::ServerTickEnd {
                world_running: attempt.tick_world_running,
            },
        ))
    }
}

use crate::game::side_ops::modapi_modding::ModapiModdingSideGameExt as _;

use crate::game::tables::WorldTablesExt as _;
