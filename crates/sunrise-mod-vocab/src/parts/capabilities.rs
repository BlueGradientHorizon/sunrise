use super::*;
use std::any::TypeId;

pub(crate) type Capabilities<P> = HashMap<&'static str, HashMap<Box<str>, Vec<CapabilityPart<P>>>>;

/// A capability provider a mod registered, with its dispatch metadata.
#[derive(Clone)]
pub(crate) struct CapabilityPart<P> {
    pub(crate) mod_id: &'static str,
    pub(crate) mod_index: usize,
    /// Asked ahead of the game's own provider.
    pub(crate) before_game: bool,
    /// Its place among every mod's registrations, which orders a block's providers and those of
    /// the block entity type standing there as one list.
    pub(crate) order: usize,
    pub(crate) provider: P,
    /// Set once this registered callback panicked: it is disabled for the rest of the session.
    pub(crate) panicked: Arc<std::sync::atomic::AtomicBool>,
}

/// Opaque native provider bound by the typed capability methods on `dyn Registrar`.
// hidden: the registrar's erased provider, not a mod construction surface
// exhaustive: its fields are private, so mods cannot build or destructure it
// no wire: it retains an in-process typed callback.
#[doc(hidden)]
#[derive(Clone)]
pub struct NativeCapabilityProvider {
    provider: Arc<dyn Any + Send + Sync>,
    handler: &'static str,
}

impl NativeCapabilityProvider {
    pub(crate) fn of<T: ?Sized + 'static, P: Send + Sync + 'static>(provider: P) -> Self {
        Self {
            provider: Arc::new(provider),
            handler: type_name::<T>(),
        }
    }

    pub(crate) fn binding(&self) -> (TypeId, &'static str) {
        (self.provider.as_ref().type_id(), self.handler)
    }

    pub(crate) fn get<P: 'static>(&self) -> Option<&P> {
        self.provider.downcast_ref::<P>()
    }
}

/// The registrar's table, as the game reads it.
pub(crate) struct InstalledProviders;

/// Visits two sorted provider slices (a block entity type's and a block's) as one sequence in
/// dispatch order: those asked ahead of the game, then the others, each group in registration
/// order across both slices.
pub(crate) struct MergedProviders<'a, P> {
    first: &'a [CapabilityPart<P>],
    second: &'a [CapabilityPart<P>],
}

impl<'a, P> MergedProviders<'a, P> {
    pub(crate) fn new(first: &'a [CapabilityPart<P>], second: &'a [CapabilityPart<P>]) -> Self {
        Self { first, second }
    }
}

impl<'a, P> Iterator for MergedProviders<'a, P> {
    type Item = &'a CapabilityPart<P>;

    fn next(&mut self) -> Option<Self::Item> {
        // Each slice is already in this order: those ahead of the game first, by registration.
        let rank = |part: &CapabilityPart<P>| (!part.before_game, part.order);
        match (self.first.split_first(), self.second.split_first()) {
            (Some((a, _)), Some((b, rest))) if rank(b) < rank(a) => {
                self.second = rest;
                Some(b)
            }
            (Some((a, rest)), _) => {
                self.first = rest;
                Some(a)
            }
            (None, Some((b, rest))) => {
                self.second = rest;
                Some(b)
            }
            (None, None) => None,
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/builtin_capability_keys.rs"));

/// Who answers a capability: a mod's provider, the game's own, or nobody.
pub(crate) enum Answer<T: ?Sized> {
    Mod(Capability<T>),
    Game,
    Nobody,
}

/// The handler `answer` names: a mod's, or `game`, the game's own when it answered.
pub(crate) fn answered<T: ?Sized>(
    answer: Answer<T>,
    game: Option<Box<T>>,
) -> Option<Capability<T>> {
    match answer {
        Answer::Mod(found) => Some(found),
        Answer::Game => game.map(|handler| Capability {
            handler,
            owner: None,
        }),
        Answer::Nobody => None,
    }
}

/// Asks the borrowed `providers` in dispatch order (those registered `before_game`, then the
/// others), checking each row's mod and callback enablement right before it is used, and the
/// game's own (`game`: whether it answers) once at the boundary between the two groups or after
/// exhaustion, in the fork's chosen order (NeoForge's javadoc doesn't state one between mods'
/// providers and the game's); the first to answer wins. Each mod callback runs under its own
/// per-registration panic containment; `game` runs outside mod containment.
pub(crate) fn ask_providers<'a, T: ?Sized>(
    host: &mut dyn ServerHost,
    providers: impl Iterator<Item = &'a CapabilityPart<NativeCapabilityProvider>>,
    call: impl Fn(&NativeCapabilityProvider, &mut dyn ServerHost) -> Option<Box<T>>,
    game: impl FnOnce(&mut dyn ServerHost) -> bool,
) -> Answer<T> {
    let mut game = Some(game);
    for part in providers {
        if !crate::callback_owner::enabled(part.mod_index, &part.panicked) {
            continue;
        }
        if !part.before_game
            && let Some(game) = game.take()
            && game(host)
        {
            return Answer::Game;
        }
        if let Ok(Some(handler)) = crate::callback_owner::call(
            part.mod_id,
            part.mod_index,
            &part.panicked,
            "capability provider",
            "capability_provider",
            || call(&part.provider, host),
        ) {
            return Answer::Mod(Capability {
                handler,
                owner: Some(part.mod_id),
            });
        }
    }
    match game.is_some_and(|game| game(host)) {
        true => Answer::Game,
        false => Answer::Nobody,
    }
}

/// CapabilityProviders operations for capabilities.
// hidden: per-kind lookup implementation for native provider bindings
#[doc(hidden)]
pub trait CapabilitiesCapabilityProviders {
    /// Known block capability names in declaration/discovery order.
    fn block_capability_names(&self) -> Vec<String>;
    /// Whether this capability was explicitly marked safe to forward.
    fn block_capability_proxyable(&self, name: &str) -> bool;
    /// Known block capability names explicitly marked safe to forward, in discovery order.
    fn block_capability_proxyable_names(&self) -> Vec<String>;
    /// How many times a mod was switched on or off, as the game counts it: a kept capability
    /// compares it to tell that a provider may have come or gone (`BlockCapabilityCache`).
    fn mods_switched(&self) -> u64;
}

impl CapabilitiesCapabilityProviders for InstalledProviders {
    fn block_capability_names(&self) -> Vec<String> {
        match installed() {
            Some(installed) => installed
                .keys
                .block_capability_order
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
            None => builtin_block_capabilities()
                .into_iter()
                .map(|(name, _)| name.to_owned())
                .collect(),
        }
    }
    fn block_capability_proxyable(&self, name: &str) -> bool {
        installed()
            .and_then(|installed| installed.keys.block_capability_metadata.get(name))
            .is_some_and(|(_, proxyable)| *proxyable == Some(true))
    }
    fn block_capability_proxyable_names(&self) -> Vec<String> {
        let Some(installed) = installed() else {
            return Vec::new();
        };
        installed
            .keys
            .block_capability_order
            .iter()
            .filter(|name| self.block_capability_proxyable(name))
            .map(|name| (*name).to_owned())
            .collect()
    }
    fn mods_switched(&self) -> u64 {
        crate::runtime::mods_switched()
    }
}
