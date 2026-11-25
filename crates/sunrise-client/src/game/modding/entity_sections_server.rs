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
}

impl LiveSectionObserver {
    pub(crate) fn new(
        sections: std::rc::Rc<std::cell::RefCell<EntitySections>>,
        scope: Scope,
    ) -> Self {
        Self { sections, scope }
    }
}
