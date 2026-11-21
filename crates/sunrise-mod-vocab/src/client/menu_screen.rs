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

pub fn menu_screen_layout(screen: &MenuScreen) -> &MenuScreenLayout {
    &screen.layout
}

pub fn menu_screen_layout_mut(screen: &mut MenuScreen) -> &mut MenuScreenLayout {
    &mut screen.layout
}
