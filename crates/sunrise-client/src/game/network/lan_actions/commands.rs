#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    /// The guests' queued commands, run with the hub in `self.lan` so that
    /// they reach the other players too (`/give Bob`, `/tp Bob`, `/kick`).
    pub(in crate::game) fn run_guest_commands(&mut self) {
        loop {
            let Some(host) = self.lan.as_mut() else {
                return;
            };
            if host.commands.is_empty() {
                return;
            }
            let c = host.commands.remove(0);
            self.run_guest_command(c);
        }
    }

    /// One guest's command as that guest, its feedback sent to them; false
    /// when it failed (or the guest is gone).
    pub(in crate::game) fn run_guest_command(&mut self, c: GuestCommand) -> bool {
        self.run_guest_command_told(c).is_some_and(|(ok, _)| ok)
    }

    /// ServerGamePacketListenerImpl.handleCustomCommandSuggestions: the
    /// completions of the command typed so far (after an optional slash), for
    /// the nodes the tree marks `ask_server`.
    /// ServerCommandSuggestionsProvider.tryProcessRequest: answer the latest request if this
    /// tick's answer is free. One client asking a hundred times a tick costs one parse.
    pub(in crate::game) fn guest_try_suggestions(&mut self, conn: u32) {
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        let play = host.play.entry(conn).or_default();
        if !play.suggestion_budget {
            return;
        }
        let Some(payload) = play.suggestion.take() else {
            return;
        };
        play.suggestion_budget = false;
        self.guest_suggestions(conn, &payload);
    }
}
