#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    /// The mod block at `pos` when its container opens menu type `menu`, the
    /// `block` its screen factory is told. Only when this is the menu that
    /// block opens: a position outlives the use that set it, so another menu
    /// opened later (a command's, another mod's) is not the last clicked
    /// machine's. Read the declaration directly, as a client's world has no
    /// server-side mod container table.
    pub(in crate::game) fn menu_opener_block(&self, pos: Pos, menu: &str) -> Option<&str> {
        let block = &self
            .res
            .facts()
            .get(self.world.block(pos.0, pos.1, pos.2))
            .name;
        if block.starts_with("minecraft:") {
            return None;
        }
        let container = mod_api::__private::parts_installation::block_container(block)?;
        let (opens, _) = mod_menu(container.menu.as_deref()?)?;
        (opens == menu).then_some(block.as_ref())
    }
}
