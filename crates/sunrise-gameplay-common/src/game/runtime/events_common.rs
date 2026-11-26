use serde_json::Value;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::mpsc::Sender,
    time::{Duration, Instant},
};

/// Events kept for `events`.
pub const KEPT: usize = 256;

/// What the last turn saw.
#[derive(PartialEq)]
pub struct Snapshot {
    pub(in crate::game) screen: &'static str,
    pub(in crate::game) world: Option<String>,
    pub(in crate::game) dimension: String,
    pub(in crate::game) chat: u64,
    pub(in crate::game) counts: BTreeMap<String, i64>,
    /// The player's health and food level, while there is a player.
    pub(in crate::game) vitals: Option<(f32, i64)>,
    pub(in crate::game) hit: Option<Hit>,
}

/// The newest damage: a number that changes with each, the damage type, the amount and the
/// name of what dealt it.
pub(in crate::game) type Hit = (i64, String, Option<f32>, Option<String>);

pub enum Condition {
    Screen(String),
    Widget(String),
    Loaded,
    Chat(String),
    Event(String),
}

pub struct Wait {
    pub(in crate::game) condition: Condition,
    /// The event sequence number when the wait started.
    pub(in crate::game) since: u64,
    pub(in crate::game) started: Instant,
    pub(in crate::game) timeout: Duration,
    pub(in crate::game) reply: Sender<String>,
}

#[derive(Default)]
pub struct Events {
    /// The sequence number of the newest event (the first is 1).
    pub(in crate::game) seq: u64,
    pub(in crate::game) log: VecDeque<Value>,
    pub(in crate::game) last: Option<Snapshot>,
    pub(in crate::game) waits: Vec<Wait>,
    /// On a server's client (single player included), where the combat tracker is the
    /// server's: the newest `DamageEvent` it sent about this player.
    pub(in crate::game) remote_hit: Option<Hit>,
}

impl Events {
    pub(in crate::game) fn push(&mut self, tick: u64, mut event: Value) {
        self.seq += 1;
        event["seq"] = self.seq.into();
        event["tick"] = tick.into();
        if self.log.len() == KEPT {
            self.log.pop_front();
        }
        self.log.push_back(event);
    }
    /// `ClientboundDamageEventPacket` for this player: the damage type and the causing
    /// entity's type.
    pub(in crate::game) fn note_hit(&mut self, cause: String, by: Option<String>) {
        let n = self.remote_hit.as_ref().map_or(0, |h| h.0) + 1;
        self.remote_hit = Some((n, cause, None, by));
    }
}
