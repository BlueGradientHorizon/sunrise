//! The `screen_input_post` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// A mouse button went down over the open screen and the screen has run (`ScreenEvent.MouseButtonPressed.Post`, vanilla's `Screen.mouseClicked` returning): `screen`, `button`, `modifiers` and `double_click` as in `screen_mouse_button_pressed_pre`. Not cancellable; `event.handled` enters with whether the screen (or a mounted widget) handled the click, and a listener may set `event.handled = true` to mark it handled (OR-merged across listeners as NeoForge's `setResult(DEFAULT/HANDLED)`). Fires on the screen that received the click (even if handling changed screens), on the window only, for real mouse input and not a pad's or keyboard focus's, and only when the matching Pre did not cancel it.
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        version = 2,
        contract = "collect",
        out = "handled",
        neoforge = "ScreenEvent.MouseButtonPressed.Post",
        cancellable = false
    )]
    fn screen_mouse_button_pressed_post(
        screen: &ScreenView<'_>,
        button: i32,
        modifiers: u32,
        double_click: bool,
        handled: &mut bool,
    );
    /// A mouse button went up over the open screen and the screen has run (`ScreenEvent.MouseButtonReleased.Post`, vanilla's `Screen.mouseReleased` returning), with the fields of `screen_mouse_button_released_pre`. Not cancellable; `event.handled` enters with whether the screen (or a mounted widget) handled the release, and a listener may set `event.handled = true` to mark it handled. Fires on the screen that received the release (even if handling changed screens), on the window only, for real mouse input and not a pad's or keyboard focus's, and only when the matching Pre did not cancel it.
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        version = 2,
        contract = "collect",
        out = "handled",
        neoforge = "ScreenEvent.MouseButtonReleased.Post",
        cancellable = false
    )]
    fn screen_mouse_button_released_post(
        screen: &ScreenView<'_>,
        button: i32,
        modifiers: u32,
        handled: &mut bool,
    );
    /// A held-button pointer movement that the receiving screen (and its mounted widgets) did not handle (`ScreenEvent.MouseDragged.Post`, vanilla's `Screen.mouseDragged` returning false): `button`, `delta` (GUI pixels, `screen.mouse` the new position) and `modifiers` as in `screen_mouse_dragged_pre`. Not cancellable. Fires on the receiving screen only when unhandled and when the matching Pre did not cancel it, on the window only, for real mouse input and not a pad's or keyboard focus's.
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        version = 2,
        contract = "listen",
        neoforge = "ScreenEvent.MouseDragged.Post",
        cancellable = false
    )]
    fn screen_mouse_dragged_post(
        screen: &ScreenView<'_>,
        button: i32,
        delta: [f32; 2],
        modifiers: u32,
    );
    /// A mouse wheel scroll that the receiving screen (and its mounted widgets) did not handle (`ScreenEvent.MouseScrolled.Post`, vanilla's `Screen.mouseScrolled` returning false): `scroll` and `modifiers` as in `screen_mouse_scrolled_pre`. Not cancellable. Fires on the receiving screen only when unhandled and when the matching Pre did not cancel it, on the window only, for real mouse input and not a pad's or keyboard focus's.
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        version = 2,
        contract = "listen",
        neoforge = "ScreenEvent.MouseScrolled.Post",
        cancellable = false
    )]
    fn screen_mouse_scrolled_post(screen: &ScreenView<'_>, scroll: [f64; 2], modifiers: u32);
    /// A key press that neither the receiving screen nor its mounted widgets took (`ScreenEvent.KeyPressed.Post`, vanilla's `Screen.keyPressed` returning false): `key`, `repeat` and `modifiers` as in `screen_key_pressed_pre`. Set `canceled` and the press counts as handled: the game's own key handling (its key bindings, hotbar and toggles) never sees it. A key that has no vanilla name fires nothing. The control socket's `key` fires it too, for a press (never a release). Fires on the window only, on the receiving screen, for real keyboard input and not a pad's, and only when the matching Pre did not cancel it.
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        version = 2,
        contract = "cancel",
        out = "canceled",
        neoforge = "ScreenEvent.KeyPressed.Post",
        cancellable = true
    )]
    fn screen_key_pressed_post(
        screen: &ScreenView<'_>,
        key: &str,
        repeat: bool,
        modifiers: u32,
        canceled: &mut bool,
    );
    /// A key release that neither the receiving screen nor its mounted widgets took (`ScreenEvent.KeyReleased.Post`, vanilla's `Screen.keyReleased` returning false): `key` and `modifiers` as in `screen_key_released_pre`. Set `canceled` and the release counts as handled: the game's own key handling never sees it, so a held mapping stays down; cancel a release only when you cancelled the press it ends. Fires on the window only, on the receiving screen, for real keyboard input and not a pad's, and only when the matching Pre did not cancel it.
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        version = 2,
        contract = "cancel",
        out = "canceled",
        neoforge = "ScreenEvent.KeyReleased.Post",
        cancellable = true
    )]
    fn screen_key_released_post(
        screen: &ScreenView<'_>,
        key: &str,
        modifiers: u32,
        canceled: &mut bool,
    );
    /// A typed character that neither the receiving screen nor its mounted widgets took (`ScreenEvent.CharacterTyped.Post`, vanilla's `Screen.charTyped` returning false), once per printable character that `screen_char_typed_pre` let through: `c` and `modifiers` as there. Not cancellable (matching NeoForge's `ScreenEvent.CharacterTyped.Post`, which is an unhandled post-dispatch notice). A held edit modifier makes the key a shortcut, so its character is not taken and fires it. Fires on the window only, on the receiving screen, for real keyboard input and not a pad's, and only when the matching Pre did not cancel it.
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        version = 2,
        contract = "listen",
        neoforge = "ScreenEvent.CharacterTyped.Post",
        cancellable = false
    )]
    fn screen_char_typed_post(screen: &ScreenView<'_>, c: char, modifiers: u32);
}
