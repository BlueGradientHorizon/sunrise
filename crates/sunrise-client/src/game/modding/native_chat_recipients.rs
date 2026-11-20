//! The receivers a mod keeps a public chat line from (`chat_recipient`), asked before the hub
//! broadcasts it.
use super::Game;

impl Game<'_> {
    /// The placed players a mod keeps a public line from, asked once each in hub order.
    /// Prepare the sender, name and message only when a mod listens.
    pub(in crate::game) fn chat_hidden_from<N: AsRef<str>, M: AsRef<str>>(
        &mut self,
        fields: impl FnOnce(&Self) -> (u128, N, M),
    ) -> Vec<u128> {
        if !mod_api::__private::listened(mod_api::HookId::ChatRecipient) {
            return Vec::new();
        }
        let Some(host) = self.lan.as_ref() else {
            return Vec::new();
        };
        let receivers: Vec<u128> = host
            .hub
            .guests
            .iter()
            .filter(|g| g.placed)
            .map(|g| g.uuid)
            .collect();
        let (sender, name, message) = fields(self);
        let (name, message) = (name.as_ref(), message.as_ref());
        receivers
            .into_iter()
            .filter(|&recipient| {
                let mut hidden = false;
                self.native_chat_recipient(sender, name, message, recipient, &mut hidden);
                hidden
            })
            .collect()
    }
}
