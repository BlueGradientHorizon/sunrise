#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::chat_received::QueuedChat;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::chat_received::QueuedChatKind;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::chat_received::player_chat_info;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::chat_received::*;

impl Game<'_, crate::game::client_side::ClientState> {
    /// NeoForge's `ClientChatReceivedEvent` (`ClientHooks.onClientChat`,
    /// `onClientPlayerChat` and `onClientSystemChat`): mods may replace
    /// `line` or cancel it. False when cancelled.
    pub(in crate::game::client_side) fn chat_received(
        &mut self,
        system: bool,
        overlay: bool,
        sender: u128,
        chat_type: Option<i32>,
        player: Option<&mod_api::PlayerChatInfo>,
        line: &mut sunrise_client::text::Text,
    ) -> bool {
        if !mod_api::__private::listened(mod_api::HookId::ClientChatReceived) {
            return true;
        }
        let registries = self.remote.as_ref().map(|r| Arc::clone(&r.registries));
        let name = chat_type
            .zip(registries.as_deref())
            .and_then(|(id, registries)| registries.name("minecraft:chat_type", id));
        // A line the API's text form cannot hold is not told, as before.
        let Ok(mut message) = sunrise_client_lib::api_text::to_api(line) else {
            return true;
        };
        let mut cancel = false;
        self.native_client_chat_received(mod_api::__private::ClientChatReceivedCall {
            system,
            overlay,
            sender,
            chat_type: name,
            player_message: player,
            message: &mut message,
            cancel: &mut cancel,
        });
        if let Ok(text) = sunrise_client_lib::api_text::to_engine(&message) {
            *line = text;
        }
        !cancel
    }
}
