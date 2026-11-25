//! `sunrise-ctl lan op`: a connected guest's operator level, which remote profiling checks.
#[allow(unused_imports)]
use super::Game;
#[allow(unused_imports)]
use super::owner_link::{ControlRequest, OwnerRequest};
#[allow(unused_imports)]
use sunrise_client::Result;

impl Game<'_> {
    /// `sunrise-ctl lan op`: the connected guest `name` gets operator level `level`; false when none is called that.
    pub(in crate::game) fn lan_set_op(&mut self, name: &str, level: u8) -> bool {
        let Some(host) = self.lan.as_mut() else {
            return false;
        };
        let Some(uuid) = host
            .hub
            .guests
            .iter()
            .find(|g| g.participates_online() && g.name == name)
            .map(|g| g.uuid)
        else {
            return false;
        };
        host.hub.ops.insert(uuid, level);
        host.hub.resend_commands();
        true
    }
}
