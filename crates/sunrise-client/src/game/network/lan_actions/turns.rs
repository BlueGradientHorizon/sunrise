#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::game::stats::Stats;
#[allow(unused_imports)]
use sunrise_client::{actions::Action, lan::guest::Guest};

/// The player that was in place before this guest's turn. Stats stay with the
/// displaced body so a synchronous death can credit it before the put-back.
pub(in crate::game) struct DisplacedPlayer {
    pub(in crate::game) body: Option<PlayerMotion>,
    pub(in crate::game) name: Option<String>,
    pub(in crate::game) uuid: Option<u128>,
    pub(in crate::game) fake: bool,
    pub(in crate::game) mode: GameMode,
    pub(in crate::game) input_sneak: bool,
    /// The borrower temporarily holds the outer player's equipment, effects,
    /// advancement progress and recipe book in its hub row.
    pub(in crate::game) record_conn: u32,
    pub(in crate::game) dimension: Dim,
    pub(in crate::game) selected: usize,
    pub(in crate::game) experience: Experience,
    pub(in crate::game) adv_loaded: bool,
    pub(in crate::game) riding: Option<u32>,
    pub(in crate::game) yaw: f32,
    pub(in crate::game) pitch: f32,
    pub(in crate::game) stats: Stats,
}

pub(in crate::game) enum CombatPlayer<'a> {
    InPlace,
    Displaced(usize, &'a PlayerMotion),
    Parked(&'a Guest),
}

impl Game<'_> {
    /// Run `f` as the guest `conn`: the host's player state is swapped for theirs.
    /// Returns `None` if the guest is missing or already active in a nested turn.
    pub(in crate::game) fn as_guest<R>(
        &mut self,
        conn: u32,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Option<R> {
        let gi = self.guest_index(conn)?;
        self.as_guest_at(gi, conn, f).ok()
    }

    /// `as_guest` for the guest at `gi` in the hub's list, which a caller that already found it
    /// passes with its `conn`: the turn starts without another scan, and is refused when the
    /// guest there isn't `conn`.
    pub(in crate::game) fn as_guest_at<R>(
        &mut self,
        gi: usize,
        conn: u32,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Result<R, TurnRefused> {
        let result = self.with_guest_swap_at(gi, conn, self.dim, |game| {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let dimension = (game.serving.is_some() || game.options.dedicated.is_some())
                    .then_some(game.dim.id());
                let result =
                    sunrise_client::block_contact::in_level(game.tick, dimension, || f(game));
                game.drain_entity_inside();
                result
            }));
            // The restore clears the acting marker, so teardown learns of all interrupted turns
            // from the record (#2704).
            if result.is_err() {
                game.record_open_turns();
            }
            result
        })?;
        // The guest's progress and the outer player were restored before propagating a panic.
        match result {
            Ok(result) => Ok(result),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    /// Where guest `conn` is in the hub's list.
    pub(in crate::game) fn guest_index(&self, conn: u32) -> Option<usize> {
        self.lan.as_ref()?.hub.guests.connection_index(conn)
    }

    /// Run a command or fixture with the guest's player state in place, without the
    /// packet/tick turn's block-contact scope or handling a panic from `f`. When the caller
    /// has changed levels, the outer body is still in `outer_dimension`.
    pub(in crate::game) fn with_guest_swap<R>(
        &mut self,
        conn: u32,
        outer_dimension: Dim,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Option<R> {
        let i = self.guest_index(conn)?;
        self.with_guest_swap_at(i, conn, outer_dimension, f).ok()
    }

    pub(in crate::game) fn with_guest_swap_at<R>(
        &mut self,
        i: usize,
        conn: u32,
        outer_dimension: Dim,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Result<R, TurnRefused> {
        let mut f = Some(f);
        let mut result = None;
        self.guest_turn(i, conn, outer_dimension, &mut |game| {
            result = Some(f.take().unwrap()(game));
        })?;
        Ok(result.unwrap())
    }

    /// `ServerLevel.broadcastDamageEvent` for a guest that took an unblocked hit.
    pub(in crate::game) fn guest_damage_broadcast(
        &mut self,
        conn: u32,
        source: sunrise_world::damage_source::DamageTypeId,
    ) {
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        let Some(g) = host.hub.guest(conn) else {
            return;
        };
        let (id, at) = (g.entity_id, g.position);
        let kind = self
            .health_resources
            .damage
            .definition(source)
            .map_or("minecraft:generic", |d| d.name.as_str());
        host.hub.damage_event(id, kind, None, None, at);
    }

    /// The game mode fields without `set_mode`'s side effects on the host.
    pub(in crate::game) fn set_mode_raw(&mut self, id: i32) {
        self.mode = crate::game::game_mode::mode_from_id(id);
        self.creative = self.mode == GameMode::Creative;
    }
}

impl Game<'_> {
    /// `LivingEntity.playHurtSound` for a guest: the server's `Entity.playSound`
    /// reaches every player around but the hurt one, whose own client plays it
    /// from `handleDamageEvent` (#4149).
    pub(in crate::game) fn guest_hurt_sound(&mut self, conn: u32, sound: &str) {
        let Some(at) = self
            .lan
            .as_ref()
            .and_then(|host| host.hub.guest(conn))
            .map(|g| g.position)
        else {
            return;
        };
        self.send_level_sound(LevelSound::new(sound, at, 1.0, 1.0), Some(conn));
    }
}
