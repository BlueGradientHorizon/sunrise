#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::chat::ChatTrust;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::chat::Decoration;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::chat::chat_type_decorate;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::chat::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::chat::{ChatSource, Tag as ChatTag};

impl Game<'_> {
    /// Record a visible line, keeping a player message's signature for deletion.
    pub(in crate::game) fn push_chat_line(
        &mut self,
        content: Text,
        source: ChatSource,
        signature: Option<Signature>,
        tag: Option<ChatTag>,
    ) {
        let text = content.plain();
        let focused = self.client.screen == Screen::Chat;
        if source == ChatSource::Player {
            self.ui.chat.add_signed(
                &self.gui.atlas,
                content,
                signature,
                tag,
                self.visual_ticks(),
                focused,
            );
        } else {
            // addServerSystemMessage and addClientSystemMessage both carry
            // GuiMessageTag.systemSinglePlayer: the grey bar at the line's left.
            let indicator = Some(sunrise_client::chat::SYSTEM_INDICATOR);
            self.ui.chat.add(
                &self.gui.atlas,
                content,
                indicator,
                source,
                self.visual_ticks(),
                focused,
            );
        }
        if let Some(r) = self.remote.as_mut() {
            r.chat.push((text, Instant::now()));
            r.chat_received += 1;
            if r.chat.len() > 100 {
                r.chat.remove(0);
            }
        }
    }
}
