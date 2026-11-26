//! Adapter to actual game-owned menu values. No synthetic slot translation and
//! no rules, recipe lookup or callbacks during validation or frozen write-back.
//!
//! The owners live in `LanHost::menus` (`lan_menus.rs`), outside the `GuestPlay` that
//! `as_guest` lifts, so they stay reachable during a guest's turn. This adapter is not
//! yet a complete LAN read set: guest/backing-storage and registry versions still need
//! to be bound before any subscribed production route may use it.
use super::MenuId;
use sunrise_sim::menu::Menu as EngineMenu;

/// Names the guest's inventory owner, never a borrowed menu moved out for dispatch.
#[derive(Clone, Copy)]
pub(in crate::game) struct Target {
    pub(in crate::game) conn: u32,
    pub(in crate::game) id: MenuId,
}

pub(in crate::game) struct KeptOwner {
    pub(in crate::game) target: Target,
    pub(in crate::game) value: Option<Box<EngineMenu>>,
}

impl Target {
    /// The owner's stable identity, which no take, restore or lift changes.
    pub(in crate::game) fn owner_id(&self) -> MenuId {
        self.id
    }
}

#[cfg(test)]
#[path = "decision_engine_tests.rs"]
mod tests;
