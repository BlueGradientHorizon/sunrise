//! The `screen_mouse_button_pressed_pre` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0093

pub trait Hooks {
    /// A mouse button went down over the open screen
    /// (`ScreenEvent.MouseButtonPressed.Pre`, vanilla's
    /// `Screen.mouseClicked`): `screen` is the screen it is, `screen.mouse`
    /// where the pointer is (so there is no position of its own), `button` the
    /// button's GLFW index (0 left, 1 right, 2 middle, 3 back, 4 forward; a
    /// button past those is the platform's own number, which differs between
    /// systems), `modifiers` GLFW's bits (shift 1, control 2, alt 4, super 8),
    /// and `double_click` matches vanilla's `MouseHandler` rule (a second press
    /// of the same button on the same screen within 250 ms). Set `canceled` and
    /// the press goes no further: no mounted widget, no slot, no screen button,
    /// no drag, no hotbar key, no sort or header button.
    ///
    /// Fires before the receiving screen acts, including the title's screens and
    /// a dialog, confirmation prompt, book, lectern or sign editor in front of
    /// another screen. It does not fire with no screen, on the connection or
    /// world-loading screen itself, or during a closing save. Only window mouse
    /// input (including a control-socket click sent through the window), not a
    /// gamepad's or keyboard focus's synthesized click.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_screen_mouse_button_pressed_pre",
        receiver = "read",
        dispatch = "flat",
        side = "client",
        version = 2,
        contract = "cancel",
        out = "canceled",
        neoforge = "ScreenEvent.MouseButtonPressed.Pre",
        cancellable = true
    )]
    fn screen_mouse_button_pressed_pre(
        screen: &ScreenView<'_>,
        button: i32,
        modifiers: u32,
        double_click: bool,
        canceled: &mut bool,
    );
}
