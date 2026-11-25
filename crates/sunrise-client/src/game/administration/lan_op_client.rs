#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::owner_link::{ControlRequest, OwnerRequest};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::lan_op::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::Result;

impl Game<'_> {
    /// `sunrise-ctl lan op NAME LEVEL`; `None` for any other command.
    pub(in crate::game::client_side) fn lan_op_control(
        &mut self,
        words: &[&str],
    ) -> Result<Option<serde_json::Value>> {
        use serde_json::json;
        if words.first() != Some(&"lan") || words.get(1) != Some(&"op") {
            return Ok(None);
        }
        let usage = "lan op NAME LEVEL (0 to 4)";
        let (Some(name), Some(level)) = (
            words.get(2),
            words
                .get(3)
                .and_then(|l| l.parse::<u8>().ok())
                .filter(|l| *l <= 4),
        ) else {
            return Err(usage.into());
        };
        let name = (*name).to_owned();
        let found = match &self.integrated {
            Some(server) => server
                .ask(|reply| OwnerRequest::Control(ControlRequest::SetOp { name, level, reply }))
                .ok_or(super::integrated::NO_ANSWER)?,
            None => self.lan_set_op(&name, level),
        };
        if found {
            Ok(Some(json!({ "ok": true })))
        } else {
            Err("no guest of that name is connected".into())
        }
    }
}
