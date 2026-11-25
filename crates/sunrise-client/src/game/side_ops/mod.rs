//! Role-dispatched side operations (pre-switch runtime dispatchers rewritten to `SideOps` traits at switch).
pub(in crate::game) mod events;
pub(in crate::game) mod modapi;
pub(in crate::game) mod player;
pub(in crate::game) mod presentation;
