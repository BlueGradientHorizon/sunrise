//! The `screen_init_post` hooks and helpers: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// A screen initialized or resized (`ScreenEvent.Init.Post`, vanilla's `Screen.init` /
    /// `Screen.rebuildWidgets`): use `event.widgets` (`ScreenWidgets`) to mount buttons, text
    /// fields, or custom widgets onto the screen. Widgets mounted here are cleared and rebuilt on
    /// every subsequent init or resize.
    #[hook(
        receiver = "read",
        dispatch = "flat",
        side = "client",
        version = 2,
        contract = "listen",
        neoforge = "ScreenEvent.Init.Post",
        cancellable = false
    )]
    fn screen_init_post(screen: &ScreenView<'_>, widgets: &mut dyn ScreenWidgets);
}
