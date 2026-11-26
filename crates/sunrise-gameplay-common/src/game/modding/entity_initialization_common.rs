//! Shared `entity_constructing` tracking used on both server and client roles.
use crate::game::CommonGame as Game;
use mod_api::Holder;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct EntityInitialization {
    /// The object holders already told (entity IDs and separate player UUIDs).
    pub initialized: HashSet<Holder>,
    /// Each LAN guest's connection when its player was last told.
    pub players: HashMap<u128, u32>,
    /// The holder being told now: its attachments are reachable before it is in the level.
    pub active: Option<Holder>,
}

impl crate::game::CommonGame<'_> {

    pub fn forget_entity_initialization(&mut self, holder: Holder) {
        if let Holder::Entity(id) = holder
            && let Some(mob) = self.mobs.iter_mut().find(|mob| mob.id == id)
        {
            mob.accepted_hit = None;
        }
        self.entity_initialization.initialized.remove(&holder);
    }
}

pub trait EntityInitializationCommonGameExt1<'__sg>: crate::game::SideGame<'__sg> {
    /// Tell the listeners about a new entity of type `kind`, unless it was already told.
     fn initialize_entity(&mut self, holder: Holder, kind: &str) {
        if !mod_api::__private::listened(mod_api::HookId::EntityConstructing) {
            return;
        }
        if !matches!(holder, Holder::Entity(_) | Holder::Player(_)) {
            return;
        }
        if !self.entity_initialization.initialized.insert(holder) {
            return;
        }
        let previous = self.entity_initialization.active.replace(holder);
        self.native_entity_constructing(holder, kind);
        self.entity_initialization.active = previous;
        if let Holder::Entity(id) = holder {
            self.side_bind_constructed_entity_attachments(id);
        }
    }

    /// `initialize_entity` for a holder whose attachments are about to be read.
     fn initialize_entity_on_access(&mut self, holder: Holder) {
        if !mod_api::__private::listened(mod_api::HookId::EntityConstructing) {
            return;
        }
        match holder {
            Holder::Entity(id) => {
                let Some(kind) = self
                    .entity(id)
                    .and_then(|entity| self.entity_kind(entity)?.id(<Self::State as crate::game::Side>::as_host(self)))
                else {
                    return;
                };
                self.initialize_entity(holder, &kind);
            }
            Holder::Player(_) => self.initialize_entity(holder, "minecraft:player"),
            _ => {}
        }
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> EntityInitializationCommonGameExt1<'__sg> for G {}


use crate::game::side_ops::modapi_modding::ModapiModdingSideGameExt as _;

use crate::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;
