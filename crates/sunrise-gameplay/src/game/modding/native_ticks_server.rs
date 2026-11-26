//! Server-only entity tick helpers (`others` family, server players, section-scope gate).
use crate::game::ServerGame;
use sunrise_gameplay_common::game::native_ticks::{KindNames, PLAYER, TickBuffers, TickKind, TickRound, Told};
use sunrise_sim::join_seq::JoinSeq;

/// Whether an entity the server keeps without simulating it (`others`) is told its tick here.
pub(in crate::game) fn other_ticks(kind: &str) -> bool {
    !matches!(kind, "minecraft:leash_knot" | "minecraft:eye_of_ender")
        && !super::general::quarantined_kind(kind)
}

impl ServerGame<'_> {
    pub(in crate::game) fn server_tick_uses_server_sections(&self) -> bool {
        self.common.entity_section_scope().is_some()
    }

    /// The entity the server keeps unsimulated at `others[i]` (`other_ticks`), told alone.
    pub(in crate::game) fn native_other_tick_alone(&mut self, told: Told, i: usize) {
        let mut buffers = std::mem::take(&mut self.common.creatures.tick_hooks);
        let (id, join) = (self.server.others.0[i].id, self.server.others.0[i].join);
        buffers.names.push(&self.server.others.0[i].kind);
        if let Some(kind) = buffers.names.names().next() {
            let mut round = TickRound {
                told,
                ..TickRound::default()
            };
            if self.common.native_tick_pre_one(&mut round, id, kind)
                && self.server.others.0.locate(join).is_some()
            {
                self.common.native_tick_post_one(&round, id, kind);
            }
        }
        buffers.names.clear();
        self.common.creatures.tick_hooks = buffers;
    }

    /// The server's unsimulated entities (`other_ticks`) of the types no mod named.
    pub(in crate::game) fn native_others_tick_family(&mut self, told: Told, born: JoinSeq) {
        if !told.listening {
            return;
        }
        let ticking = self.entity_ticking_area();
        let buffers = std::mem::take(&mut self.common.creatures.tick_hooks);
        let (mut ids, mut names) = (buffers.ids, buffers.names);
        for o in self.server.others.0.iter() {
            if o.join < born
                && other_ticks(&o.kind)
                && !told.alone_kind(&o.kind)
                && ticking(o.position.x.floor() as i32, o.position.z.floor() as i32)
            {
                ids.push(o.id);
                names.push(&o.kind);
            }
        }
        let mut kinds: Vec<&str> = buffers.kinds;
        if !ids.is_empty() {
            kinds.extend(names.names());
            let mut canceled = Vec::new();
            self.native_entity_tick_pre(self.common.dim.id(), &ids, &kinds, &mut canceled);
            if !canceled.is_empty() {
                let mut kept = 0;
                for at in 0..ids.len() {
                    if !canceled.contains(&ids[at]) {
                        ids.swap(kept, at);
                        kinds.swap(kept, at);
                        kept += 1;
                    }
                }
                ids.truncate(kept);
                kinds.truncate(kept);
            }
            if !ids.is_empty() {
                self.native_entity_tick_post(self.common.dim.id(), &ids, &kinds);
            }
        }
        let mut back = TickBuffers::emptied(ids, kinds, KindNames::default());
        names.clear();
        back.names = names;
        self.common.creatures.tick_hooks = back;
    }

    /// `native_entity_tick_pre` for the players about to run `ServerPlayer.tick`.
    pub(in crate::game) fn native_players_tick_pre(&mut self) -> TickRound {
        self.common.creatures.canceled_players.clear();
        let mut round = self.common.native_tick_round();
        if !round.told.listening {
            return round;
        }
        let Some(host) = self.server.lan.as_ref() else {
            return round;
        };
        if round.alone_kind(PLAYER) {
            for gi in 0..host.hub.guests.len() {
                let Some((conn, id)) = self.server.lan.as_ref().and_then(|host| {
                    let g = host.hub.guests.get(gi).filter(|g| host.hub.here(g))?;
                    Some((g.conn, g.entity_id as u32))
                }) else {
                    continue;
                };
                if !self.common.native_tick_pre_one(&mut round, id, PLAYER) {
                    self.common.creatures.canceled_players.push(conn);
                }
            }
        } else {
            for g in host.hub.guests.iter().filter(|g| host.hub.here(g)) {
                round.add(g.entity_id as u32, TickKind::Fixed(PLAYER));
            }
            self.common.native_tick_pre_batch(&mut round);
            if let Some(host) = self.server.lan.as_ref() {
                let canceled = &mut self.common.creatures.canceled_players;
                canceled.extend(
                    host.hub
                        .guests
                        .iter()
                        .filter(|g| round.canceled(g.entity_id as u32))
                        .map(|g| g.conn),
                );
            }
        }
        round
    }

    /// `native_entity_tick_post` for the player told alone whose turn just ended, if any.
    pub(in crate::game) fn native_player_tick_post_alone(
        &mut self,
        round: &TickRound,
        told: &mut Option<u32>,
    ) {
        if let Some(id) = told.take() {
            self.common.native_tick_post_one(round, id, PLAYER);
        }
    }

    /// Whether a mod's Pre canceled guest `conn`'s `ServerPlayer.tick` this tick.
    pub(in crate::game) fn player_tick_canceled(&self, conn: u32) -> bool {
        let canceled = &self.common.creatures.canceled_players;
        !canceled.is_empty() && canceled.contains(&conn)
    }
}

use sunrise_gameplay_common::game::native_hooks::base::GameHooks as _;
