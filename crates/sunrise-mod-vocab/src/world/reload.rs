//! Shared reload listener registration, prepare/apply, errors and crash attribution.
//! `server_reload` and `client_reload` define each side's context, listener trait
//! and vanilla anchors.
use crate::{
    FileToIdConverter, Identifier,
    world::reload_order::{ReloadOrder, ReloadOrderError},
};
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

/// An error loading a resource, with its ID and the original cause available
/// through [`Self::resource`] and [`std::error::Error::source`]. Its display
/// text names the resource; reload diagnostics report the complete source chain.
// no wire: retains the native error and its source chain in this process.
#[derive(Debug)]
// exhaustive: its fields are private, so mods cannot build or destructure it
pub struct ReloadResourceError {
    resource: Identifier,
    source: Box<dyn std::error::Error + Send + Sync>,
}
impl ReloadResourceError {
    /// Attach the resource ID to a parsing or validation error.
    pub fn new(
        resource: Identifier,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            resource,
            source: Box::new(source),
        }
    }

    /// The converted ID of the resource that failed to load.
    pub fn resource(&self) -> &Identifier {
        &self.resource
    }
}
impl std::fmt::Display for ReloadResourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.resource)
    }
}
impl std::error::Error for ReloadResourceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

/// The stateless boundary listener each side's anchors are built from; each
/// side's file implements its listener trait for it.
pub(crate) struct Boundary;

/// Crash attribution for one listener call. A returned `Err` is recovered by the
/// reload; a panic in `prepare` or `apply` is fatal, and the report names the
/// registering mod (through the native-mod mark) and the listener (through the
/// report's details).
pub(crate) struct ReloadMark {
    _guard: Option<crate::content::crash::ModGuard>,
}

impl ReloadMark {
    pub(crate) fn enter(
        owner: Option<&'static str>,
        listener: &Identifier,
        phase: &'static str,
    ) -> Self {
        let _guard = owner.map(|id| {
            crate::content::crash::enter_native_mod(crate::content::crash::ModCall {
                id,
                hook: phase,
                part: None,
            })
        });
        crate::content::crash::set_detail("Reload listener", listener.to_string());
        Self { _guard }
    }
}

impl Drop for ReloadMark {
    fn drop(&mut self) {
        crate::content::crash::clear_detail("Reload listener");
    }
}

// no wire: process-local listener or callback storage; trait objects are invoked through host dispatch, not serialized.
/// One registered listener of either side, between registration and ordering.
#[non_exhaustive]
pub struct RegisteredListener<L: ?Sized> {
    /// The listener's name.
    pub id: Identifier,
    /// Maps the listener's files to ids, if it reads a folder.
    pub converter: Option<FileToIdConverter>,
    /// The listener.
    pub listener: Arc<L>,
    /// The registering mod's id, for crash attribution on its later calls; the
    /// vanilla anchors have none.
    pub owner: Option<&'static str>,
}

/// Registration both sides share: unique names and objects, dependencies, the
/// vanilla anchor, and the ordering sort. A side adds what its moment needs
/// (the server retains listeners) around it.
pub(crate) struct ReloadCore<L: ?Sized> {
    order: ReloadOrder<RegisteredListener<L>>,
}

impl<L: ?Sized + 'static> ReloadCore<L> {
    /// Seed the registry with the side's vanilla anchor; `boundary` builds its
    /// stateless placeholder listener.
    pub(crate) fn new(anchor: Identifier, boundary: impl Fn() -> Arc<L>) -> Self {
        let mut order = ReloadOrder::default();
        order
            .add_listener(
                anchor.clone(),
                RegisteredListener {
                    id: anchor,
                    converter: None,
                    listener: boundary(),
                    owner: None,
                },
            )
            .unwrap();
        Self { order }
    }
    /// Register a unique listener and converter, e.g.
    /// `FileToIdConverter::json("machines")`.
    pub(crate) fn add_listener(
        &mut self,
        id: Identifier,
        converter: FileToIdConverter,
        listener: Arc<L>,
    ) -> Result<(), ReloadOrderError> {
        let owner = crate::content::crash::current_native_mod();
        self.insert(id, converter, listener, owner)
    }
    fn insert(
        &mut self,
        id: Identifier,
        converter: FileToIdConverter,
        listener: Arc<L>,
        owner: Option<&'static str>,
    ) -> Result<(), ReloadOrderError> {
        if let Some(registered_as) = self.find_name(&*listener) {
            return Err(ReloadOrderError::DuplicateObject {
                id,
                registered_as: registered_as.clone(),
            });
        }
        self.order.add_listener(
            id.clone(),
            RegisteredListener {
                id,
                converter: Some(converter),
                listener,
                owner,
            },
        )
    }
    /// Require `first` before `second`; both names must already be registered.
    pub(crate) fn add_dependency(
        &mut self,
        first: Identifier,
        second: Identifier,
    ) -> Result<(), ReloadOrderError> {
        self.order.add_dependency(first, second)
    }
    /// Registration order, vanilla anchor included.
    pub(crate) fn registry(&self) -> impl Iterator<Item = (&Identifier, &RegisteredListener<L>)> {
        self.order.registry()
    }
    /// Read-only dependency edges as `(first, second)` names.
    pub(crate) fn graph(&self) -> impl Iterator<Item = (&Identifier, &Identifier)> {
        self.order.graph()
    }
    /// Find a listener object's registered name, or None for an unregistered
    /// object.
    pub(crate) fn find_name(&self, listener: &L) -> Option<&Identifier> {
        self.registry().find_map(|(id, entry)| {
            std::ptr::addr_eq(entry.listener.as_ref(), listener).then_some(id)
        })
    }
    /// The ordered listeners; `anchor` is the vanilla listener unrelated mods
    /// sort after.
    pub(crate) fn finish(
        self,
        anchor: &Identifier,
    ) -> Result<Vec<RegisteredListener<L>>, ReloadOrderError> {
        self.order.sort(Some(anchor))
    }
}

/// A failed reload: staging its inputs failed, listeners could not be ordered,
/// or a listener's `prepare` returned an error. Both sides report listeners alike.
// no wire: local staging preserves the typed ordering and listener errors.
#[derive(Debug)]
#[non_exhaustive]
pub enum ReloadStageError {
    /// The next resource generation or its enabled-package inventory could not be built.
    DataSource(String),
    /// An accepted root's original jars or input record could not be read.
    OriginalJars(crate::JarInputsError),
    /// The listeners could not be ordered.
    Order(ReloadOrderError),
    /// A listener's `prepare` failed.
    Listener {
        /// The failing listener's name.
        listener: Identifier,
        /// Why it failed.
        source: ReloadResourceError,
    },
}
impl std::fmt::Display for ReloadStageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DataSource(error) => f.write_str(error),
            Self::OriginalJars(error) => error.fmt(f),
            Self::Order(error) => error.fmt(f),
            Self::Listener { listener, source } => {
                write!(f, "reload listener {listener}: {source}")
            }
        }
    }
}
impl ReloadStageError {
    /// Format a staging failure for the log or command reply. Each listener
    /// error contributes its own message once, even across nested resources.
    pub fn diagnostic(&self) -> String {
        use std::{error::Error, fmt::Write};

        let mut message = self.to_string();
        let Self::Listener { source, .. } = self else {
            return message;
        };
        let mut seen = HashSet::new();
        seen.insert(std::ptr::from_ref(source as &dyn Error));
        let mut next = source.source();
        while let Some(cause) = next {
            // Keep the trait metadata: distinct zero-sized errors can share an address.
            if !seen.insert(std::ptr::from_ref(cause)) {
                message.push_str(": [circular error source]");
                break;
            }
            let _ = write!(message, ": {cause}");
            next = cause.source();
        }
        message
    }
}
impl std::error::Error for ReloadStageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::DataSource(_) => return None,
            Self::OriginalJars(error) => error,
            Self::Order(error) => error,
            Self::Listener { source, .. } => source,
        })
    }
}

// no wire: process-local listener or callback storage; trait objects are invoked through host dispatch, not serialized.
/// One mod listener's prepared step, between its `prepare` and its `apply`.
/// The vanilla anchor stands for the function library or model manager.
#[non_exhaustive]
pub enum PreparedStep<V = ()> {
    /// A mod listener's prepared step.
    Mod {
        /// Publishes the prepared result, consuming the action.
        apply: Box<dyn FnOnce() + Send + 'static>,
        /// The owning mod's id, or `None` for the game's own.
        owner: Option<&'static str>,
        /// The listener's name.
        id: Identifier,
    },
    /// The side's prepared vanilla anchor.
    Vanilla(V),
}

/// Orders the registered listeners, then prepares each with a converter
/// against the reload's own view: the shared loop of both sides' reloads.
/// `context_of` builds the side's context for one listener from its converter,
/// `prepare_of` calls the side's listener trait, and `vanilla` prepares a
/// converter-less vanilla anchor. Runs on the side's staging thread.
/// If any prepare fails, the already prepared actions are dropped without running.
pub fn prepare_listeners<L: ?Sized, C, V, E: From<ReloadStageError>>(
    ordered: Vec<RegisteredListener<L>>,
    phase: &'static str,
    mut vanilla: impl FnMut() -> Result<V, E>,
    context_of: impl Fn(&Identifier, &FileToIdConverter) -> C,
    prepare_of: impl Fn(&L, &C) -> Result<Box<dyn FnOnce() + Send + 'static>, ReloadResourceError>,
) -> Result<Vec<PreparedStep<V>>, E> {
    let mut prepared = Vec::new();
    for listener in ordered {
        let Some(converter) = &listener.converter else {
            prepared.push(PreparedStep::Vanilla(vanilla()?));
            continue;
        };
        let context = context_of(&listener.id, converter);
        let _mark = ReloadMark::enter(listener.owner, &listener.id, phase);
        let apply = prepare_of(&*listener.listener, &context).map_err(|source| {
            ReloadStageError::Listener {
                listener: listener.id.clone(),
                source,
            }
        })?;
        prepared.push(PreparedStep::Mod {
            apply,
            owner: listener.owner,
            id: listener.id,
        });
    }
    Ok(prepared)
}

/// Consumes the prepared steps in order on the thread that owns the game,
/// attributing each mod listener's apply to its registering mod. `vanilla`
/// publishes the side's own step.
pub fn apply_listeners<V, E>(
    prepared: Vec<PreparedStep<V>>,
    phase_owner: &crate::world::phase::PhaseOwner,
    phase: &'static str,
    mut vanilla: impl FnMut(V) -> Result<(), E>,
) -> Result<(), E> {
    for step in prepared {
        match step {
            PreparedStep::Mod { apply, owner, id } => {
                let _reload = phase_owner
                    .reloading()
                    .expect("reload apply on its owning thread");
                let _mark = ReloadMark::enter(owner, &id, phase);
                apply();
            }
            PreparedStep::Vanilla(step) => vanilla(step)?,
        }
    }
    Ok(())
}

/// The files one converter selected from a reload's packs, shared by both
/// sides' contexts; each side keeps its own context type so a wrong-side read
/// does not compile.
// exhaustive: its fields are private, so mods cannot build or destructure it
// no wire: owns the local bytes the prepare context lends to a listener.
pub struct ReloadFiles(BTreeMap<Identifier, Vec<u8>>);
impl ReloadFiles {
    pub(crate) fn new(files: BTreeMap<Identifier, Vec<u8>>) -> Self {
        Self(files)
    }
    /// Converted IDs, across all namespaces, in deterministic resource-ID order.
    pub(crate) fn list(&self) -> impl Iterator<Item = &Identifier> {
        self.0.keys()
    }
    /// The winning bytes for a converted ID, or None outside this directory.
    pub(crate) fn read(&self, id: &Identifier) -> Option<&[u8]> {
        self.0.get(id).map(Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, sync::Mutex, thread};

    #[test]
    fn reload_moves_send_only_actions_to_the_applying_thread_in_order() {
        let trace = Arc::new(Mutex::new(Vec::new()));
        let applying_thread = thread::current().id();
        let prepared = thread::scope(|scope| {
            scope
                .spawn(|| {
                    assert_ne!(thread::current().id(), applying_thread);
                    let ordered = ["first", "vanilla", "last"]
                        .into_iter()
                        .map(|name| RegisteredListener {
                            id: Identifier::parse(&format!("test:{name}")).unwrap(),
                            converter: (name != "vanilla")
                                .then(|| FileToIdConverter::json("data").unwrap()),
                            listener: Arc::new(name),
                            owner: None,
                        })
                        .collect();
                    prepare_listeners(
                        ordered,
                        "reload_prepare",
                        || Ok::<_, ReloadStageError>("apply vanilla"),
                        |_, _| (),
                        |&name, _| {
                            trace.lock().unwrap().push(format!("prepare {name}"));
                            let payload = Cell::new(format!("apply {name}"));
                            let trace = trace.clone();
                            Ok(Box::new(move || {
                                assert_eq!(thread::current().id(), applying_thread);
                                trace.lock().unwrap().push(payload.into_inner());
                            }))
                        },
                    )
                    .unwrap()
                })
                .join()
                .unwrap()
        });

        assert_eq!(
            trace.lock().unwrap().as_slice(),
            ["prepare first", "prepare last"]
        );
        let phase = crate::world::phase::PhaseOwner::new(crate::world::phase::ModPhase::World);
        apply_listeners(prepared, &phase, "reload_apply", |vanilla| {
            trace.lock().unwrap().push(vanilla.into());
            Ok::<_, std::convert::Infallible>(())
        })
        .unwrap();
        assert_eq!(
            trace.lock().unwrap().as_slice(),
            [
                "prepare first",
                "prepare last",
                "apply first",
                "apply vanilla",
                "apply last"
            ]
        );
    }

    #[test]
    fn reload_diagnostic_visits_each_nested_resource_and_cause_once() {
        #[derive(Debug)]
        struct Inner;
        impl std::fmt::Display for Inner {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("inner")
            }
        }
        impl std::error::Error for Inner {}

        #[derive(Debug)]
        struct Middle(Inner);
        impl std::fmt::Display for Middle {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("middle")
            }
        }
        impl std::error::Error for Middle {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&self.0)
            }
        }

        #[derive(Debug)]
        struct Outer(Middle);
        impl std::fmt::Display for Outer {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("outer")
            }
        }
        impl std::error::Error for Outer {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&self.0)
            }
        }

        let resource_error = ReloadResourceError::new(
            Identifier::parse("test:machine").unwrap(),
            ReloadResourceError::new(
                Identifier::parse("test:recipe").unwrap(),
                Outer(Middle(Inner)),
            ),
        );
        assert_eq!(resource_error.to_string(), "test:machine");
        let stage_error = ReloadStageError::Listener {
            listener: Identifier::parse("test:listener").unwrap(),
            source: resource_error,
        };
        assert_eq!(
            stage_error.diagnostic(),
            "reload listener test:listener: test:machine: test:recipe: outer: middle: inner"
        );
    }

    #[test]
    fn reload_diagnostic_stops_cycles_that_cross_a_resource_wrapper() {
        #[derive(Debug)]
        struct Circular(&'static str);
        static CYCLE: std::sync::LazyLock<ReloadResourceError> = std::sync::LazyLock::new(|| {
            ReloadResourceError::new(Identifier::parse("test:loop").unwrap(), Circular("repeat"))
        });
        impl std::fmt::Display for Circular {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.0)
            }
        }
        impl std::error::Error for Circular {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&*CYCLE)
            }
        }
        let stage_error = ReloadStageError::Listener {
            listener: Identifier::parse("test:listener").unwrap(),
            source: ReloadResourceError::new(
                Identifier::parse("test:machine").unwrap(),
                Circular("enter"),
            ),
        };
        assert_eq!(
            stage_error.diagnostic(),
            "reload listener test:listener: test:machine: enter: test:loop: repeat: [circular error source]"
        );
    }
}
