#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Dim, Game,
    commands::{R, Target, err},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::bossbars::BossState;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::bossbars::Offer;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::bossbars::Scope;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::bossbars::Viewer;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::bossbars::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::collections::{HashMap, HashSet};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    boss_overlay,
    bossbar::{self, BossBars},
    command::{Arg, Context},
    mobs::Species,
    save,
    scoreboard::{self as sb, Comp, canonical_component, json_to_tag},
    scoreboard_cmd::format_list,
    text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    identifier::Identifier, java_hash_map::JavaHashMap, nbt::Tag,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::packets::play::{
    self, BossAction, Clientbound as C,
};

impl Game<'_> {
    /// `ClientPacketListener.handleBossUpdate`.
    pub(in crate::game::client_side) fn apply_boss_event(&mut self, packet: &play::BossEvent) {
        self.client
            .boss_overlay
            .apply(packet, boss_overlay::now_millis());
    }
    #[inline(always)]
    pub(in crate::game) fn publish_boss_bars_client(&mut self, event: &play::BossEvent, now: u128) {
        self.client.boss_overlay.apply(event, now);
    }
}
