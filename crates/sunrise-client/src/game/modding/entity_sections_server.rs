#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::entity_sections::Crossing;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::entity_sections::EntitySections;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::entity_sections::LiveSectionHost;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::entity_sections::LiveSectionObserver;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::entity_sections::PlayerMoveState;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::entity_sections::Scope;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::entity_sections::SectionWriter;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::entity_sections::crossed;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::entity_sections::player::PlayerSectionObserver;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::entity_sections::section_coord;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::entity_sections::section_of;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::entity_sections::tell_live;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::entity_sections::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::mobs::living::section_writes::PositionWrite;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::__private::game::BackendRefOf;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{DimensionKey, EntityRef, WorldEpoch};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::collections::HashMap;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::collections::hash_map::Entry;

#[path = "entity_sections/observer_cache_server.rs"]
pub(in crate::game) mod observer_cache;

impl Game<'_> {
    /// The projectiles tick ([`Game::tick_projectiles_now`]), and each one's move is filed.
    pub(in crate::game) fn tick_projectiles(&mut self) -> super::Result<()> {
        self.tick_projectiles_now()
    }

    /// `Entity.teleport` placed as `/tp` places it ([`Game::place_teleport_in_level`]), then filed
    /// and told as one position update (`/tp`, a mob's ender pearl).
    pub(in crate::game) fn place_teleport_filed(
        &mut self,
        t: super::commands::Target,
        pos: [f64; 3],
        rotation: Option<[f32; 2]>,
        relative: [bool; 3],
        turn_riders: bool,
    ) -> Option<[f64; 3]> {
        let from = self.place_teleport_in_level(t, pos, rotation, relative, turn_riders);
        if let (Some(entity), Some(to)) = (self.network_entity_id(t), self.target_position(t)) {
            self.entity_section_moved(entity as u32, from, to);
        }
        from
    }

    pub(in crate::game) fn server_pick_section_move(&mut self, _id: u32, _to: [f64; 3]) {}

    pub(in crate::game) fn server_pick_section_sync_moved_id(&mut self, _id: u32) {}

    pub(in crate::game) fn server_pick_section_remove(&mut self, _id: u32) {}
}

impl LiveSectionObserver {
    pub(crate) fn new(
        sections: std::rc::Rc<std::cell::RefCell<EntitySections>>,
        scope: Scope,
    ) -> Self {
        Self { sections, scope }
    }
}
