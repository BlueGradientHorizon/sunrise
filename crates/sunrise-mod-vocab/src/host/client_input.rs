//! Host method declarations; access defaults and game forwarders are generated.
#![allow(unused_variables)]
use crate::*;
/// The client host's methods of the `client_input` area, implemented by the game and reached through [`crate::ClientHost`].
#[sunrise_mod_api_macros::host_area]
pub trait ClientInputClientHost {
    /// Side: client.
    /// Right-clicks with these items (at most 64 ids) are mod `mod_id`'s; a later call replaces the list.
    #[access(WorldOnly)]
    fn claim_items(&mut self, mod_id: &str, ids: &[String]);
    /// Side: client.
    /// Mod `mod_id`'s events since its last poll (at most 256 wait).
    #[access(WorldOnly)]
    fn poll(&mut self, mod_id: &str) -> Vec<ModEvent>;
    /// Side: client.
    /// Where the mouse is, in GUI pixels.
    #[access(ClientSetup)]
    fn cursor(&self) -> [f32; 2];
    /// Side: client.
    /// The saved key and modifiers for an owner-bound registration, or its declared default.
    #[access(ClientSetup)]
    fn mod_key(&self, mod_id: &str, name: &str) -> Option<(String, ClientModifiers)>;
    /// Side: client.
    /// Binds mod `mod_id`'s registered mapping `name` to `key` (`key.keyboard.z`,
    /// `key.mouse.left`, `key.keyboard.unknown` for none) with `modifiers` held, as the player
    /// binding it in Controls does (`KeyMapping.setKey` with NeoForge's
    /// `setKeyModifierAndCode`, then `Options.save`): the mapping answers to the new key from the
    /// next input, and the binding is saved to `sunrise-mods.txt`. A modifier key bound on its own
    /// is the key, not a modifier held with itself, as Controls stores it. Each refusal changes
    /// nothing and is named by `ModKeyRefusal`: `NotRegistered`, `NotOwner` (a native mod asked
    /// for another mod's mapping), `UnknownKey`, `UnknownModifiers` and `Unavailable`.
    #[access(ClientSetup)]
    fn set_mod_key(
        &mut self,
        mod_id: &str,
        name: &str,
        key: &str,
        modifiers: ClientModifiers,
    ) -> Result<(), ModKeyRefusal>;
    /// Side: client.
    /// Gives mod `mod_id`'s registered mapping `name` its declared default key and modifiers
    /// back, as Controls' Reset does (`KeyMapping.setToDefault`), and saves `sunrise-mods.txt`.
    /// Refusals as for `set_mod_key`, without `UnknownKey` and `UnknownModifiers`.
    #[access(ClientSetup)]
    fn reset_mod_key(&mut self, mod_id: &str, name: &str) -> Result<(), ModKeyRefusal>;
    /// Side: client.
    /// Whether mod `mod_id`'s registered mapping `id` is held (`KeyMapping.isDown`): its key or
    /// mouse button is down and its conflict context and modifiers are active. False for an
    /// unregistered id.
    #[access(ClientSetup)]
    fn is_down(&self, mod_id: &str, id: &str) -> bool;
    /// Side: client.
    /// Takes one click off mod `mod_id`'s registered mapping `id` (`KeyMapping.consumeClick`):
    /// true when it had one. A press, each auto-repeat while its key is held, and a mouse press
    /// add one, with no screen open and its conflict context and modifiers active, from the
    /// window or the control socket alike; a screen opening drops them all. False for an
    /// unregistered id.
    #[access(ClientSetup)]
    fn consume_click(&mut self, mod_id: &str, id: &str) -> bool;
    /// Side: client.
    /// Playing, with nothing over the world: keys and zoom apply.
    #[access(ClientSetup)]
    fn playing(&self) -> bool;
    /// Side: client.
    /// Resolve a vanilla mapping name such as `key.jump`, not a binding name such as
    /// `key.keyboard.space`. Keep the handle for subsequent reads and writes. An unknown name returns
    /// [`MappingError::Unknown`]; a host or phase without client input returns
    /// [`MappingError::Unavailable`].
    #[access(ClientSetup)]
    fn vanilla_key_mapping(&self, name: &str) -> Result<VanillaKeyMappingRef, MappingError>;
    /// Side: client.
    /// Milliseconds on the game's own clock (`Util.getMillis`), counting from the game's start:
    /// what a text field's cursor blinks by and what tells a double click from two clicks.
    /// Only differences between two readings mean anything.
    #[access(ClientSetup)]
    fn millis(&self) -> u64;
    /// Side: client.
    /// The current window modifier keys in the same GLFW encoding as input hooks.
    /// Cleared when the window loses focus; available even outside a world.
    #[access(ClientSetup)]
    fn modifiers(&self) -> ClientModifiers;
    /// Side: client.
    /// The clipboard's text (`KeyboardHandler.getClipboard`), empty when it holds none,
    /// while the game handles a key, mouse or typed-text event: a mod's key or screen hook
    /// pasting into its own field. None at any other moment (a tick, a frame, a packet),
    /// so no mod reads the clipboard without the player doing something, and None on a
    /// server.
    #[access(ClientSetup)]
    fn clipboard_text(&self) -> Option<String>;
    /// Side: client.
    /// Puts `text` on the clipboard (`KeyboardHandler.setClipboard`) on the same terms as
    /// `native_clipboard_text` reads it. False, with nothing copied, at any other moment or
    /// on a server; empty text copies nothing, as vanilla's.
    #[access(ClientSetup)]
    fn set_clipboard_text(&self, text: &str) -> bool;
}

/// The live client reads behind [`VanillaKeyMappingRef`].
// hidden: a handle's backend, reached through `Backend`; mods call the handle, never this trait
#[doc(hidden)]
#[sunrise_mod_api_macros::host_area]
pub trait ClientInputClientBackend {
    /// Side: client.
    /// The current binding name (`key.keyboard.w`, `key.mouse.left`, or
    /// `key.keyboard.unknown` when unbound), borrowed for the life of the host borrow.
    /// Reads through a foreign client return [`MappingError::Stale`]; an unavailable
    /// backend or phase returns [`MappingError::Unavailable`].
    #[access(ClientSetup)]
    fn vanilla_key_mapping_binding(
        &self,
        mapping: VanillaKeyMappingRef,
    ) -> Result<&str, MappingError>;
    /// Side: client.
    /// The logical held state (`KeyMapping.isDown`) of movement, attack or use, including
    /// controller input, toggles and logical overrides. Other vanilla mappings resolve but
    /// return [`MappingError::NoLogicalState`]; this never substitutes a physical read.
    /// Stale and unavailable handles are refused as for [`VanillaKeyMappingRef::binding`].
    #[access(ClientSetup)]
    fn vanilla_key_mapping_is_down(
        &self,
        mapping: VanillaKeyMappingRef,
    ) -> Result<bool, MappingError>;
    /// Side: client.
    /// Whether the current binding's key or mouse button is physically down. Ignores
    /// toggles, conflict context, modifiers and logical overrides. False when the binding
    /// has no physical key or button. Stale and unavailable handles are refused as for
    /// [`VanillaKeyMappingRef::binding`].
    #[access(ClientSetup)]
    fn vanilla_key_mapping_physical_down(
        &self,
        mapping: VanillaKeyMappingRef,
    ) -> Result<bool, MappingError>;
    /// Side: client.
    /// Builds the current binding's Controls label on demand (`KeyMapping.getTranslatedKeyMessage`).
    /// An invalid component returns [`MappingError::Label`]; stale and unavailable handles
    /// are refused as for [`VanillaKeyMappingRef::binding`].
    #[access(ClientSetup)]
    fn vanilla_key_mapping_translated_key_message(
        &self,
        mapping: VanillaKeyMappingRef,
    ) -> Result<Text, MappingError>;
}

/// The live client writes behind [`VanillaKeyMappingRef`].
// hidden: a handle's backend, reached through `ClientBackendMut`; mods call the handle, never this trait
#[doc(hidden)]
#[sunrise_mod_api_macros::host_area]
pub trait ClientInputClientBackendMut {
    /// Side: client.
    /// Holds this vanilla gameplay mapping down or lets it go (`KeyMapping.setDown`), without
    /// adding a click. For a hold-style mapping this replaces its logical state until the next
    /// input for that mapping. It does not press or release the physical key or add a separate
    /// hold that can outlast its release. Sneak, sprint, attack and use with their toggle option
    /// on flip only on `true`, as `ToggleKeyMapping.setDown` does.
    ///
    /// The client tick's key step acts on the mapping with all its checks: a held use repeats
    /// every four ticks, and a held attack goes on breaking the block. With a screen or overlay
    /// open a press does nothing, and opening one releases the mappings (`KeyMapping.releaseAll`).
    /// `key_mapping_pre` is not asked: it is about physical keys.
    ///
    /// Accepts movement, gameplay and inventory mappings only ([`MappingError::NotGameplay`]).
    /// `key.pickItem` is click-only ([`MappingError::ClickOnly`]); use [`VanillaKeyMappingRef::click`].
    /// An unavailable backend or phase returns [`MappingError::Unavailable`], a handle from
    /// another client returns [`MappingError::Stale`], and no player returns [`MappingError::NoWorld`].
    #[access(WorldOnly)]
    fn vanilla_key_mapping_set_down(
        &mut self,
        mapping: VanillaKeyMappingRef,
        down: bool,
    ) -> Result<(), MappingError>;
    /// Side: client.
    /// Adds one click to this vanilla gameplay mapping (`KeyMapping.click`), without holding it.
    /// An attack, use or pick click acts on a subsequent client tick, through the checks of
    /// `startAttack` and `startUseItem` (the miss cooldown, busy hands, the item and the target
    /// under the crosshair), sending the same packets as the player's own click. Inventory
    /// mappings select a hotbar slot, drop an item, open the inventory or swap hands as usual.
    /// With a screen or overlay open the click is dropped. `interaction_key_mapping_triggered`
    /// fires at the action as for the player's click; `key_mapping_pre` is not asked.
    ///
    /// Queued actions drain oldest first, at most 64 entries per queue per drain, with at most
    /// 1024 pending. Excess entries wait for later drains (normally later client ticks). Each
    /// hotbar key has its own click counter, up to 1024 pending clicks per key. A selection drain
    /// takes one click per key, in order from key 1 to key 9, so the highest key clicked wins.
    /// Selection can drain more than once per tick. A full queue refuses the new entry and
    /// returns normally, logging only its first refusal for that queue's lifetime. These bounds
    /// also apply to physical input queued through the same routes.
    ///
    /// Refusals are as for [`VanillaKeyMappingRef::set_down`], except that `key.pickItem` can be clicked.
    #[access(WorldOnly)]
    fn vanilla_key_mapping_click(
        &mut self,
        mapping: VanillaKeyMappingRef,
    ) -> Result<(), MappingError>;
}
