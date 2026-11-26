pub(in crate::game) type ContainerPos = (i32, i32, i32);

/// A menu the player opened, including the block entity instances it opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game) struct HeardMenu {
    pub(in crate::game) window: i32,
    /// The menu type's name; a mount's inventory has none in vanilla.
    pub(in crate::game) menu: String,
    /// The block the menu is of.
    pub(in crate::game) pos: Option<[i32; 3]>,
    /// Each position and its generation when the menu opened; combined menus hold both halves.
    pub(in crate::game) openers: [Option<(ContainerPos, u64)>; 2],
}
