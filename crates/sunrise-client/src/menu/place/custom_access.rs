//! Engine owners of detached placement and publication use the same frozen plan.
use super::{Kind, Menu, custom_plan::MenuPlacement};
impl MenuPlacement {
    /// Physical slots whose old contents the placement operation may return.
    pub fn cleared_slots(&self) -> &[usize] {
        &self.clear
    }
    /// A `Mod` layout's plan before its part names the grid: only `for_mod` plans have no inputs.
    pub fn awaits_grid(&self) -> bool {
        self.inputs.is_empty()
    }
    /// Authoritative ingredient predicates, never display examples.
    pub fn requirements(&self) -> &crate::crafting::Placement {
        &self.placement
    }
}
impl Menu {
    /// Rebuild only visibility from the actual current mod menu. A cached
    /// machine may have hidden slots left over from an earlier linked view.
    pub fn reset_mod_visibility(&mut self, hidden: &[usize]) {
        self.touch();
        if let Kind::Mod(kind) = self.state.kind {
            for (slot, entry) in self.state.slots.iter_mut().take(kind.slots).enumerate() {
                entry.role = if hidden.contains(&slot) {
                    super::super::Role::Hidden
                } else {
                    self.state.kind.slot(slot).unwrap().role
                };
            }
        }
    }
}
