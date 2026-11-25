#[allow(unused_imports)]
pub(in crate::game::client_side) use super::stats;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::villager_client::unhappy_counter;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game,
    native_living_effects::{active_effect, mob_effect},
    shield::OFFHAND_SLOT,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::api_values::TryIntoApi;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::HELP;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::LanHost;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::LanSettings;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::Owner;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::baby_index;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::client_info;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::event_timings::{
    EventTimings, UseDetail, describe_burst, event_kind, note_event,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::living_flags;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::mob_freeze_meta;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::mob_meta;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::mob_raider_meta;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::lan::*;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::client_side) use level_event_targets::level_event_except;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{collections::HashMap, sync::Arc, time::Instant};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    Result,
    effects::{
        EffectInstance, ModifiedEffectParticles, ShownEffect, effect_particle,
        visible_effect_particles,
    },
    lan::{
        self, NetConfig,
        chunks::ChunkEncoder,
        configuration_tasks::{Configured, PreparedSpawn},
        data::{BIOME_REGISTRY, SyncData},
        entity::{EntityState, States, avatar_main_arm_meta},
        hub::{HostEvent, Hub, HubConfig, TickCtx, stuck_meta},
        playerdata,
    },
    mobs::{Mob, Species, Who, leash::Holder},
    text::Text,
    world::World,
};
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::client_side) use sunrise_gameplay::lan::level_event_targets;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::{
    entity_data::{Particles, Value, serializer},
    packets::play::{self, Clientbound as C},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::block_collision::BlockCollisionRegistry;

impl Game<'_> {
    pub(in crate::game) fn client_lan_open(&mut self, settings: LanSettings) -> Result<()> {
        let server = self
            .integrated
            .as_ref()
            .ok_or("no world of this game to open")?;
        let port = server
            .ask(|reply| super::owner_link::OwnerRequest::Publish(settings, reply))
            .ok_or(super::integrated::NO_ANSWER)??;
        self.client_system_message(Text::translate(
            "menu.multiplayerOptions.publish.started.lan",
            &[Text::literal(port.to_string())],
        ));
        Ok(())
    }

    pub(in crate::game) fn client_lan_close(&mut self, announce: bool) {
        let Some(server) = &self.integrated else {
            self.friends_lan_closed();
            return;
        };
        let closed = server
            .ask(super::owner_link::OwnerRequest::Unpublish)
            .unwrap_or(false);
        if closed {
            self.friends_lan_closed();
            if announce {
                self.client_system_message(Text::translate(
                    "menu.multiplayerOptions.publish.stopped",
                    &[],
                ));
            }
        }
    }

    /// `sunrise-ctl lan ...`.
    pub(in crate::game::client_side) fn lan_control(
        &mut self,
        words: &[&str],
    ) -> Result<Option<serde_json::Value>> {
        use serde_json::json;
        if words[0] != "lan" {
            return Ok(None);
        }
        if let Some(server) = &self.integrated
            && words.get(1) == Some(&"status")
        {
            // The hub is the server thread's.
            let status = server
                .ask(super::owner_link::OwnerRequest::LanStatus)
                .ok_or(super::integrated::NO_ANSWER)?;
            return Ok(Some(status));
        }
        match words.get(1).copied() {
            Some("open") => {
                if self.title.0.is_some() {
                    return Err("open a world first".into());
                }
                let port = words.get(2).map_or(Ok(0), |p| p.parse::<u16>())?;
                self.client_lan_open(LanSettings {
                    port: if port == 0 {
                        lan::available_port()
                    } else {
                        port
                    },
                    guest_commands: words.contains(&"commands"),
                    force_game_mode: words.contains(&"force"),
                })?;
                Ok(Some(json!({ "ok": true, "port": self.lan_port() })))
            }
            Some("close") => {
                self.client_lan_close(true);
                Ok(Some(json!({ "ok": true })))
            }
            Some("status") => Ok(Some(json!({ "open": false }))),
            _ => Err("lan open|close|status|op".into()),
        }
    }
    pub(in crate::game) fn lan_port(&self) -> Option<u16> {
        self.integrated
            .as_ref()
            .and_then(|server| server.published())
    }
}

#[cfg(test)]
#[path = "tracking_hook_tests_client.rs"]
pub(in crate::game) mod tracking_hook_tests;
