//! The receivers a mod keeps a public chat line from (`chat_recipient`), asked before the hub
//! broadcasts it.
use super::Game;

impl Game<'_> {
    /// Prepare the sender and text only if there are subscribers. Use the same eligibility
    /// as the hub's public chat delivery; take the identities before lending the Host, so a
    /// listener's own chat command can run independently through the same delivery path.
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
            .chat_recipients()
            .map(|(_, guest)| guest.uuid)
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
