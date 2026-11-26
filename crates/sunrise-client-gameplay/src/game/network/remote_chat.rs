//! Secure chat on a server's client (ClientPacketListener): the chat
//! session made from the account's chat key pair (setKeyPair), signed chat
//! and signed command arguments (sendChat, sendCommand), and the messages
//! the client acknowledges as seen (LastSeenMessagesTracker,
//! markMessageAsProcessed). Without a signed-in
//! account, or on a server that plays the account under another profile
//! (offline mode), there is no session and everything goes unsigned.
use super::{Screen, books::BookScreen, dialogs::ClosedDialog};
use std::sync::Arc;
use sunrise_client::{
    auth::{self, ChatKeyPair},
    confirm_screen::Choice,
    hud::HudBatch,
};
use sunrise_command::{Dispatcher, Registries};
use sunrise_game_data::text::Text;
use sunrise_java_properties::now_millis;
use sunrise_protocol::{
    packets::play::{self, Serverbound as S},
    secure_chat::{Body, Encoder, LastSeenTracker, Signature},
};
use winit::keyboard::KeyCode;

/// ClientPacketListener.sendChatAcknowledgement's threshold.
const ACK_AFTER: i32 = 64;

/// ClientPacketListener.CommandCheckResult.
#[derive(Clone, Copy, Debug, PartialEq)]
enum CommandCheck {
    NoIssues,
    ParseErrors,
    SignatureRequired,
    PermissionsRequired,
}

/// ClientPacketListener.verifyCommand over the tree the server sent.
fn verify_command(tree: &Dispatcher, registries: &Registries, command: &str) -> CommandCheck {
    let parse = tree.parse(command, registries);
    if !parse.is_valid() {
        return CommandCheck::ParseErrors;
    }
    if !parse.signable_arguments(tree).is_empty() {
        return CommandCheck::SignatureRequired;
    }
    if !tree
        .without_restricted()
        .parse(command, registries)
        .is_valid()
    {
        return CommandCheck::PermissionsRequired;
    }
    CommandCheck::NoIssues
}

/// What the confirmation's first button does.
#[derive(Clone, Copy, Debug, PartialEq)]
enum CommandAccept {
    /// openCommandSendConfirmationWindow: the command goes as it is.
    Run,
    /// openSignedCommandSendConfirmationWindow: into the chat box, for the
    /// player to read and send.
    Suggest,
    /// The same with a screen to come back to: on the clipboard.
    Copy,
}

/// The screen a clicked command left, which answering no returns to
/// (openSendConfirmationWindow's `currentScreen`).
pub(in crate::game) enum Behind {
    Book(Box<BookScreen>),
    Dialog(ClosedDialog),
}

/// The `ConfirmScreen` of ClientPacketListener.openSendConfirmationWindow.
pub(in crate::game) struct CommandPrompt {
    command: String,
    message: &'static str,
    accept: CommandAccept,
    /// A screen is open behind it: the second button says Back, not Cancel.
    over_screen: bool,
    behind: Option<Behind>,
    /// The button Tab or an arrow key moved to.
    focus: Option<usize>,
}

/// `FriendlyByteBuf.readUtf(256)` counts UTF-16 code units, not Unicode scalar
/// values. Keep a whole scalar at the boundary so the wire string is valid.
fn truncate_utf16(content: &str, max_units: usize) -> String {
    let mut units = 0;
    content
        .chars()
        .take_while(|ch| {
            let next = units + ch.len_utf16();
            (next <= max_units).then(|| units = next).is_some()
        })
        .collect()
}

/// A connection's secure chat state, new with every login packet.
#[derive(Default)]
pub struct SecureChat {
    /// LocalChatSession: the key pair it signs with, and its chain.
    session: Option<(Arc<ChatKeyPair>, Encoder)>,
    tracker: LastSeenTracker,
}

impl SecureChat {
    pub(in crate::game::client_side) fn signed(&self) -> bool {
        self.session.is_some()
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn tracker_mut(&mut self) -> &mut LastSeenTracker {
        &mut self.tracker
    }
    /// handleDeleteChat: a deleted message that is still pending is not
    /// acknowledged.
    pub(in crate::game) fn ignore_pending(&mut self, signature: &Signature) {
        self.tracker.ignore_pending(signature);
    }
}

/// Crypt.SaltSupplier.getLong: a SecureRandom long.
fn salt() -> i64 {
    i64::from_be_bytes(random())
}

/// Bytes from the OS. A failure is logged, not hidden: what it leaves is not random.
fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    if let Err(e) = sunrise_protocol::crypto::random_bytes(&mut bytes) {
        log::error!("Secure chat has no randomness: {e}");
    }
    bytes
}

/// Encoder.pack; a message that can't be signed goes without, and says so.
fn sign(keys: &ChatKeyPair, encoder: &mut Encoder, body: &Body) -> Option<Signature> {
    let signature = encoder.sign(&keys.private, body);
    if signature.is_none() {
        log::error!("Failed to sign a chat message");
    }
    signature
}

/// UUID.randomUUID: version 4, IETF variant.
fn random_uuid() -> u128 {
    let mut bytes: [u8; 16] = random();
    bytes[6] = bytes[6] & 0x0f | 0x40;
    bytes[8] = bytes[8] & 0x3f | 0x80;
    u128::from_be_bytes(bytes)
}

#[cfg(feature = "client")]
impl crate::game::ClientGame<'_> {
    /// handleLogin: a new session state and signature cache, and prepareKeyPair.
    /// The session itself goes out from `tick_chat_session`, never from here:
    /// vanilla's tick sends it once the key future is done, after the login's
    /// teleport confirm.
    pub(in crate::game::client_side) fn remote_chat_login(&mut self) {
        if let Some(r) = self.client.remote.as_mut() {
            r.secure_chat = SecureChat::default();
            r.signatures = Default::default();
        }
        // Only when the server says it is in online mode (handleLogin).
        if self.client.remote.as_ref().is_some_and(|r| r.online_mode)
            && let Some(account) = self.auth()
        {
            auth::prepare_chat_keys_for(account, true);
        }
    }

    /// ClientPacketListener.tick's key refresh (shouldRefreshKeyPair), then
    /// setKeyPair with whatever key pair is ready.
    pub(in crate::game::client_side) fn tick_chat_session(&mut self) {
        // shouldRefreshKeyPair is asked only once there is a chat session; a
        // first key pair arrives through the join's prepareKeyPair.
        if self
            .client
            .remote
            .as_ref()
            .is_some_and(|r| r.logged_in && r.online_mode)
        {
            if self
                .client
                .remote
                .as_ref()
                .is_some_and(|r| r.secure_chat.signed())
                && let Some(account) = self.auth()
            {
                auth::prepare_chat_keys_for(account, false);
            }
            self.update_chat_session();
        }
    }

    /// setKeyPair: only for the signed-in account's own profile
    /// (isLocalPlayer), and only when the key pair changed: a new session
    /// (random id, chain from link 0) the server hears of at once.
    fn update_chat_session(&mut self) {
        let Some(account) = self.auth() else {
            return;
        };
        let Some(r) = self.client.remote.as_mut() else {
            return;
        };
        if !r.logged_in || r.profile() != account.uuid {
            return;
        }
        let Some(keys) = auth::chat_keys_for(account.uuid) else {
            return;
        };
        if r.secure_chat
            .session
            .as_ref()
            .is_some_and(|(k, _)| Arc::ptr_eq(k, &keys))
        {
            return;
        }
        let id = random_uuid();
        let packet = S::ChatSessionUpdate(play::ChatSessionUpdate {
            session: id,
            expires_at: keys.public.expires_at,
            key: keys.public.key.clone(),
            key_signature: keys.public.signature.clone(),
        });
        r.secure_chat.session = Some((keys, Encoder::new(account.uuid, id)));
        self.send(packet);
    }

    /// ClientPacketListener.sendChat: the last-seen update goes with every
    /// message; the signature only with a chat session.
    pub(in crate::game::client_side) fn send_signed_chat(&mut self, content: &str) {
        let content = truncate_utf16(content, 256);
        let (timestamp, salt) = (now_millis(), salt());
        let Some(r) = self.client.remote.as_mut() else {
            return;
        };
        let chat = &mut r.secure_chat;
        let seen = chat.tracker.update();
        let signature = chat.session.as_mut().and_then(|(keys, encoder)| {
            sign(
                keys,
                encoder,
                &Body {
                    content: &content,
                    timestamp,
                    salt,
                    last_seen: &seen.last_seen,
                },
            )
        });
        self.send(S::Chat(play::Chat {
            message: content,
            timestamp,
            salt,
            signature: signature.map(|s| s.to_vec()),
            last_seen: seen.update,
        }));
    }

    /// ClientPacketListener.sendCommand: a command with message arguments
    /// (SignableCommand.of over the chat's tree, the server's once it sent one) has each of them signed,
    /// all with the same time, salt and last seen (without a chat session the
    /// signature list is empty, as in vanilla); any other command goes as the
    /// unsigned packet.
    pub fn send_command(&mut self, command: &str) {
        let Some(r) = self.client.remote.as_mut() else {
            return;
        };
        let arguments: Vec<(String, String)> = if r.chat_tree_received {
            r.chat_tree
                .parse(command, &self.common.ui.registries)
                .signable_arguments(&r.chat_tree)
        } else {
            Vec::new()
        };
        if arguments.is_empty() {
            self.send(S::ChatCommand(play::ChatCommand {
                command: command.to_owned(),
            }));
            return;
        }
        let (timestamp, salt) = (now_millis(), salt());
        let chat = &mut r.secure_chat;
        let seen = chat.tracker.update();
        let mut signatures = Vec::new();
        if let Some((keys, encoder)) = chat.session.as_mut() {
            for (name, value) in arguments {
                let body = Body {
                    content: &value,
                    timestamp,
                    salt,
                    last_seen: &seen.last_seen,
                };
                if let Some(signature) = sign(keys, encoder, &body) {
                    signatures.push((name, signature.to_vec()));
                }
            }
        }
        self.send(S::ChatCommandSigned(play::ChatCommandSigned {
            command: command.to_owned(),
            timestamp,
            salt,
            arguments: signatures,
            last_seen: seen.update,
        }));
    }

    /// ClientPacketListener.sendUnattendedCommand: a command the player
    /// didn't type (a clicked chat line, a dialog's button, a mod's command)
    /// goes unsigned when `verify_command` has no issue with it. Any other
    /// asks first, and one with a message argument is never sent from here:
    /// the player can only send it themselves, from the chat box. So neither
    /// a server nor a mod can have the player sign a message they never saw.
    /// Returns true when sent without a confirmation, so a dialog can open
    /// its screen after the command.
    pub(in crate::game::client_side) fn send_unattended_command(&mut self, command: &str) -> bool {
        let command = command.trim();
        let command = command.strip_prefix('/').unwrap_or(command);
        if command.is_empty() || !self.chat_abilities().allows(&format!("/{command}")) {
            return false;
        }
        let Some(r) = self.client.remote.as_ref() else {
            return false;
        };
        // Vanilla's dispatcher is empty until the server sends its tree.
        let check = if r.chat_tree_received {
            verify_command(&r.chat_tree, &self.common.ui.registries, command)
        } else {
            CommandCheck::ParseErrors
        };
        // screenAfterCommand: what stays once it has run, a dialog included.
        let over_screen = self.client.screen != Screen::Playing || self.dialog_open();
        let (message, accept) = match check {
            CommandCheck::NoIssues => {
                // Verification above excludes message arguments. Use the ordinary sender so
                // releases without an unsigned command packet still acknowledge seen chat.
                self.send_command(command);
                return true;
            }
            CommandCheck::ParseErrors => (
                "multiplayer.confirm_command.parse_errors",
                CommandAccept::Run,
            ),
            CommandCheck::PermissionsRequired => (
                "multiplayer.confirm_command.permissions_required",
                CommandAccept::Run,
            ),
            CommandCheck::SignatureRequired => (
                "multiplayer.confirm_command.signature_required",
                if over_screen {
                    CommandAccept::Copy
                } else {
                    CommandAccept::Suggest
                },
            ),
        };
        self.open_screen_overlay("command_prompt", |game| {
            game.client.command_prompt.0 = Some(CommandPrompt {
                command: command.to_owned(),
                message,
                accept,
                over_screen,
                behind: None,
                focus: None,
            });
            game.capture(false);
        });
        false
    }

    /// The command just clicked left this screen: if it asks, no returns there.
    pub(in crate::game::client_side) fn command_prompt_returns_to(&mut self, behind: Behind) {
        if let Some(prompt) = self.client.command_prompt.0.as_mut() {
            prompt.behind = Some(behind);
            prompt.over_screen = true;
        }
    }

    /// The translation key of the open confirmation's message.
    pub(in crate::game::client_side) fn command_prompt_message(&self) -> Option<&'static str> {
        self.client.command_prompt.0.as_ref().map(|p| p.message)
    }

    /// The confirmation's title, the command in yellow in its message, and
    /// its two buttons.
    fn command_prompt_layout(&self, prompt: &CommandPrompt, size: (f32, f32)) -> Choice {
        let command = Text::untrusted(&prompt.command).with_color(0xFF_FF55);
        let mut choice = Choice::new(
            &self.client.gui.atlas.font,
            size,
            &Text::translate("multiplayer.confirm_command.title", &[]),
            &Text::translate(prompt.message, &[command]),
            2,
            150.0,
        );
        choice.focus = prompt.focus;
        choice
    }

    pub(in crate::game::client_side) fn draw_command_prompt(&self, batch: &mut HudBatch) {
        let Some(prompt) = &self.client.command_prompt.0 else {
            return;
        };
        sunrise_client::widgets::cover_frame();
        sunrise_client::widgets::begin_screen(|| {
            super::screen_info::confirm(
                "multiplayer/confirm_command",
                &super::screen_info::tr("multiplayer.confirm_command.title", &[]),
                &prompt.command,
            )
        });
        let atlas = &self.client.gui.atlas;
        let (w, h) = (batch.width, batch.height);
        sunrise_client::ui::menu_background(batch, atlas, 0.0, 0.0, w, h, true);
        let labels = [
            Text::translate(
                match prompt.accept {
                    CommandAccept::Run => "multiplayer.confirm_command.run_command",
                    CommandAccept::Suggest => "multiplayer.confirm_command.suggest_command",
                    CommandAccept::Copy => "chat.copy",
                },
                &[],
            ),
            Text::translate(
                if prompt.over_screen {
                    "gui.back"
                } else {
                    "gui.cancel"
                },
                &[],
            ),
        ];
        let mut layout = self.command_prompt_layout(prompt, (w, h));
        layout.declare_no(1);
        layout.draw(batch, atlas, self.client.cursor, &labels);
        self.draw_overlay_mounted_widgets(batch, "command_prompt");
    }

    /// The confirmation's answer; on no, what was behind it is back.
    pub(in crate::game::client_side) fn answer_command_prompt(&mut self, yes: bool) {
        let Some(prompt) = self.client.command_prompt.0.take() else {
            return;
        };
        match (yes, prompt.accept, prompt.behind) {
            (true, CommandAccept::Run, _) => {
                if self.client.remote.is_some() {
                    self.send(S::ChatCommand(play::ChatCommand {
                        command: prompt.command,
                    }));
                }
            }
            // Gui.openChatAndAddText.
            (true, CommandAccept::Suggest, _) => {
                self.begin_chat_screen(&format!("/{}", prompt.command), false)
            }
            (true, CommandAccept::Copy, _) => {
                use sunrise_client::clipboard::Clipboard;
                self.client.clipboard.0.set(&format!("/{}", prompt.command));
            }
            (false, _, Some(Behind::Book(book))) => self.open_book(*book),
            (false, _, Some(Behind::Dialog(dialog))) => self.reopen_dialog(dialog),
            (false, _, None) => {}
        }
        self.refresh_screen_widgets();
        if self.client.screen == Screen::Playing && !self.screen_over_playing() {
            self.capture(true);
        }
    }

    /// ConfirmScreen.keyPressed, and what its buttons take from the keyboard
    /// (Screen's focus navigation, AbstractButton's Enter and Space).
    pub(in crate::game::client_side) fn command_prompt_key(&mut self, key: KeyCode) -> bool {
        let (size, shift) = (self.gui_size(), self.has_shift_down());
        let Some(prompt) = &self.client.command_prompt.0 else {
            return false;
        };
        let mut choice = self.command_prompt_layout(prompt, size);
        match key {
            KeyCode::Escape => {
                self.answer_command_prompt(false);
                return true;
            }
            KeyCode::Tab => choice.tab(shift),
            KeyCode::ArrowLeft | KeyCode::ArrowRight => choice.arrow(key == KeyCode::ArrowLeft),
            KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space => {
                if let Some(button) = choice.pressed() {
                    self.client.audio.0.play_button_click();
                    self.answer_command_prompt(button == 0);
                    return true;
                }
                return false;
            }
            _ => return false,
        }
        let focus = choice.focus;
        if let Some(prompt) = self.client.command_prompt.0.as_mut() {
            prompt.focus = focus;
        }
        false
    }

    pub(in crate::game::client_side) fn command_prompt_click(&mut self) -> bool {
        let Some(prompt) = &self.client.command_prompt.0 else {
            return false;
        };
        let pressed = self
            .command_prompt_layout(prompt, self.gui_size())
            .pressed_at(self.client.cursor);
        if let Some(button) = pressed {
            self.client.audio.0.play_button_click();
            self.answer_command_prompt(button == 0);
        }
        pressed.is_some()
    }

    /// markMessageAsProcessed, when the chat takes a signed message (past
    /// any Chat Delay): whether it was shown, and an acknowledgement once more
    /// than 64 wait (sendChatAcknowledgement).
    pub(in crate::game) fn mark_message_processed(&mut self, signature: &Signature, shown: bool) {
        let Some(r) = self.client.remote.as_mut() else {
            return;
        };
        let tracker = &mut r.secure_chat.tracker;
        if tracker.add_pending(signature, shown) && tracker.offset() > ACK_AFTER {
            let offset = tracker.take_offset();
            self.send(S::ChatAck(play::ChatAck { offset }));
        }
    }

    /// `ClientPacketListener.sendChatAcknowledgement`, used after a
    /// configuration transition flushes the delayed chat queue.
    pub(in crate::game::client_side) fn send_chat_acknowledgement(&mut self) {
        let offset = self
            .client
            .remote
            .as_mut()
            .map(|r| r.secure_chat.tracker.take_offset())
            .unwrap_or(0);
        if offset > 0 {
            self.send(S::ChatAck(play::ChatAck { offset }));
        }
    }
}

#[cfg(feature = "client")]
#[cfg(test)]
mod tests {
    use super::truncate_utf16;

    #[test]
    fn chat_limit_counts_utf16_units() {
        let emojis = "😀".repeat(256);
        assert_eq!(truncate_utf16(&emojis, 256).encode_utf16().count(), 256);
        assert_eq!(truncate_utf16(&emojis, 255).encode_utf16().count(), 254);
        let mixed = format!("{}😀tail", "a".repeat(255));
        assert_eq!(truncate_utf16(&mixed, 256), "a".repeat(255));
    }
}
