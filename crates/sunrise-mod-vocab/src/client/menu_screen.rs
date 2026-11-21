use crate::client::types::MenuScreenLayout;

// no wire: a window-owned presentation assembled on the client.
/// Presentation of one open menu. The window owns it until that opening closes.
pub struct MenuScreen {
    layout: MenuScreenLayout,
}

impl MenuScreen {
    /// Give the open window its layout. Sunrise validates it against the active menu
    /// before displaying it. Use positive finite dimensions and finite slot positions.
    pub fn new(layout: MenuScreenLayout) -> Self {
        Self { layout }
    }
}

/// Read the validated layout from the window-owned screen.
pub fn menu_screen_layout(screen: &MenuScreen) -> &MenuScreenLayout {
    &screen.layout
}

/// Access the layout during open-time validation and normalization.
pub fn menu_screen_layout_mut(screen: &mut MenuScreen) -> &mut MenuScreenLayout {
    &mut screen.layout
}

// no wire: the factory borrows the active client's menu metadata only for construction.
/// The active menu contract, offered once before the window shows its screen.
// exhaustive: the game constructs this view; mods only read it
pub struct MenuScreenOpen<'a> {
    /// The server's container id for this opening. It can be reused after close.
    pub window: i32,
    /// The active menu type's namespaced identifier.
    pub menu: &'a str,
    /// The verified mod block whose container opens this menu, if any.
    pub block: Option<&'a str>,
    /// The position supplied for this opening, or its verified clicked opener.
    pub pos: Option<[i32; 3]>,
    /// The server's title as an API text component, if available.
    pub title: Option<&'a crate::Text>,
    /// The number of physical container slots, including permanently excluded ones.
    pub backing_slots: usize,
    /// The number of appended player inventory slots, including the hotbar.
    pub player_slots: usize,
    /// The custom menu's live data slot count, or the capacity for dynamic data
    /// on a stock menu opened by a mod. This does not count opening bytes.
    pub data_slots: usize,
    /// Permanent container backing indices left empty by the layout. Indices do not move.
    pub excluded_slots: &'a [u16],
    /// The server's one-time menu payload, empty if absent. Decode it here; retain only
    /// owned values needed by the screen. No borrowed data survives the factory call.
    pub extra: &'a [u8],
}

/// Constructs a fresh presentation for an opening, not a shared screen instance.
/// Register it with [`crate::MenusRegistrar::register_menu_screen`].
pub trait MenuScreenFactory: Send + Sync + 'static {
    /// Build the screen for `open`, or return `None` to decline the opening.
    /// Called on the client before the screen is installed. `host` reads client state;
    /// controls that require a shown screen are not available yet.
    fn create(&self, host: &dyn crate::ClientHost, open: MenuScreenOpen<'_>) -> Option<MenuScreen>;
}

impl<F> MenuScreenFactory for F
where
    F: Fn(&dyn crate::ClientHost, MenuScreenOpen<'_>) -> Option<MenuScreen> + Send + Sync + 'static,
{
    fn create(&self, host: &dyn crate::ClientHost, open: MenuScreenOpen<'_>) -> Option<MenuScreen> {
        self(host, open)
    }
}
