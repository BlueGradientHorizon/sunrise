//! A hub container click the game owns. `Hub::container_click` used to run inside
//! `self.lan.as_mut()`, so nothing it called could reach the game: a hook fired in the middle
//! of a click had no `&Game` to be given. Here the game lifts the host out of itself for the
//! click, so the click code and the game are separate values and the game is whole while the
//! hub runs.
#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::game::{lan_menus::ClickReply, stacked_on_other::Pickup};
#[allow(unused_imports)]
use std::cell::RefCell;
#[allow(unused_imports)]
use sunrise_client::menu::StackedOutcome;

impl Game<'_> {
    /// Run `f` with the LAN host taken out of `self.lan`, then put it back. `f` is given the whole
    /// game (`self.lan` is `None` while it runs) and the host. The host is restored on every
    /// path, including a panic in `f`, which is then propagated: no borrow of the host outlives
    /// the call and the game is not left without it. `None` when there is no host to lift,
    /// which includes a call made while the host is already lifted.
    pub(in crate::game) fn with_lan_lifted<R>(
        &mut self,
        f: impl FnOnce(&mut Self, &mut LanHost) -> R,
    ) -> Option<R> {
        let mut host = self.lan.take()?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self, &mut host)));
        debug_assert!(self.lan.is_none(), "a lifted LAN host was replaced");
        self.lan = Some(host);
        match result {
            Ok(result) => Some(result),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}
