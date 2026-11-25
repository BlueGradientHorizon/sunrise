#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::commands::Target;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::native_living_effects::{active_effect, mob_effect};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_living_body::ARMOR_STAND;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_living_body::ENDER_DRAGON;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_living_body::PLAYER;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_living_body::body_absorption;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_living_body::body_attribute;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_living_body::body_effect;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_living_body::body_effects;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_living_body::body_equipment;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_living_body::body_health;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_living_body::body_holder;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_living_body::body_kind;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_living_body::body_kind_name;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_living_body::body_max_health;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_living_body::body_no_ai;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_living_body::body_potion_affected;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_living_body::dragon_body;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_living_body::player_attribute;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_living_body::stand_body;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_living_body::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{ActiveEffect, EquipmentSlot};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::Arc;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::health::{
    FixedBody, LivingBody, PlayerBody, PlayerHealthRead,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::mobs::finalize::{self, EquipmentRead};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::attribute_map::ValueKind;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_world::attributes::entity::Catalog;

impl Game<'_, crate::game::server_side::ServerState> {
    /// The entity id `who` names as a hit's causing entity: a mob's own, the player in place's
    /// for `Who::Player`; 0 for none.
    pub(in crate::game) fn causer_id(&self, who: Option<sunrise_client::mobs::Who>) -> i32 {
        match who {
            Some(sunrise_client::mobs::Who::Mob(id)) => id as i32,
            Some(sunrise_client::mobs::Who::Player) => {
                self.player.as_ref().map_or(0, |p| p.entity_id)
            }
            None => 0,
        }
    }
    /// The body a living target names now, where its reads are: the player in place's own
    /// fields, a guest's hub record, a mob that is not removed, an armor stand or the dragon.
    pub(in crate::game) fn native_living_body(&self, target: Target) -> Option<LivingBody<'_>> {
        Some(match target {
            Target::Player => {
                let p = self.player.as_ref()?;
                LivingBody::Player(PlayerBody {
                    id: p.entity_id,
                    uuid: None,
                    health: PlayerHealthRead::Synced(&p.health.data, p.health.ids),
                    attributes: Some(&p.attributes),
                    effects: self.native_living_player_effects(target),
                    equipment: sunrise_client::health::player_equipment(
                        &self.inventory,
                        self.selected,
                        &self.armor,
                        &self.offhand,
                        &self.equipment,
                    ),
                    attachments: None,
                })
            }
            Target::Guest(conn) => {
                // The guest's hub record, the one read a hit's parked causer makes too; an acting
                // guest's effects are the game's.
                let mut body = PlayerBody::of_record(self.command_guest(conn)?);
                body.effects = self.native_living_player_effects(target);
                // The holder and its map are a hit's own loan; outside one the record names
                // none, as before.
                body.uuid = None;
                body.attachments = None;
                LivingBody::Player(body)
            }
            Target::Mob(id) => {
                LivingBody::Mob(self.mobs.iter().find(|m| m.id == id && !m.removed)?)
            }
            Target::Stand(id) => stand_body(self.armor_stands.iter().find(|a| a.id == id)?),
            Target::Dragon(id) => dragon_body(self.dragon.as_ref().filter(|d| d.id == id)?),
            _ => return None,
        })
    }
    /// The guest whose entity id `id` is, when the `LivingRef` reads answer for it from its hub
    /// record now (`Target::Guest`, not the player in place): the causing entity a hit may lend
    /// read-only ([`HitLend::parked_guest`](sunrise_client::health::HitLend::parked_guest)).
    /// None for anything else, so a hit's view never answers where `LivingRef` does not.
    pub(in crate::game) fn native_parked_causer(&self, id: i32) -> Option<u32> {
        let epoch = self.native_lifecycle.epoch()?;
        let living =
            mod_api::LivingRef::new(mod_api::EntityRef::new(epoch, u32::try_from(id).ok()?));
        match self.native_living_target(living)? {
            Target::Guest(conn) if self.side.lan_acting != Some(conn) => Some(conn),
            _ => None,
        }
    }
}
