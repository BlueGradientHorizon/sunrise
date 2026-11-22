//! Refusals of level and registry reads.

/// Why a `LevelRef` read cannot return its requested state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub enum LevelReadError {
    /// The handle's level is gone or has been reopened.
    Stale,
    /// The state is not readable from this side or phase.
    Unavailable,
}

impl crate::AccessRefusal for LevelReadError {
    fn unavailable() -> Self {
        Self::Unavailable
    }
}
impl crate::NoBackend for LevelReadError {
    fn no_backend() -> Self {
        Self::Unavailable
    }
}

/// Why the current side's registry could not be visited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
// no wire: this refusal is returned only to the in-process registry visitor
pub enum RegistryReadError {
    /// No registry has this key, or this side does not expose it.
    UnknownRegistry,
    /// There is no live registry backend or the phase does not allow world access.
    Unavailable,
}

impl crate::AccessRefusal for RegistryReadError {
    fn unavailable() -> Self {
        Self::Unavailable
    }
}
impl crate::NoBackend for RegistryReadError {
    fn no_backend() -> Self {
        Self::Unavailable
    }
}
