//! The `screen_key_released_pre` hook: declarations the mod API generator reads (see `hooks/mod.rs`).
// catalog-order: 0120

pub trait Hooks {
    /// Before the receiving screen handles a named key's release. Same receiving
    /// screens and modifiers as `screen_key_pressed_pre`. Window releases and the
    /// control socket's `key ... up` and release phase of `key ... tap` fire it.
    /// Cancel skips both screen and mapped key handling (including holds and
    /// toggles); the physical key is still recorded as up. Do not cancel a release
    /// just to bypass the screen when its mapped press was allowed.
    #[hook(
        target = "sunrise_client::game::native_hooks::base::Game<'_>::native_screen_key_released_pre",
        receiver = "read",
        dispatch = "flat",
        side = "client",
        version = 2,
        contract = "cancel",
        out = "canceled",
        neoforge = "ScreenEvent.KeyReleased.Pre",
        cancellable = true
    )]
    fn screen_key_released_pre(
        screen: &ScreenView<'_>,
        key: &str,
        modifiers: u32,
        canceled: &mut bool,
    );
}
