//! SNBT item parsing for mods (`item_from_snbt`).
use crate::game::CommonGame;
use sunrise_items::items::ItemStack;

/// The most SNBT text `item_from_snbt` reads: `NbtAccounter`'s default quota, 2 MiB.
const MAX_SNBT_BYTES: usize = 2 * 1024 * 1024;

pub trait NativeGiveStackGameExt1<'__sg>: crate::game::SideGame<'__sg> {
    /// `ItemStack.CODEC` reading the compound `snbt` stands for (`TagParser`): only that
    /// compound and whitespace. At most 2 MiB and 512 levels: vanilla's SNBT parser has
    /// neither limit (deep text ends in a stack overflow); they are this API's bound so
    /// mod-supplied text cannot overflow the parser.
     fn item_from_snbt(&self, snbt: &str) -> Option<ItemStack> {
        if snbt.len() > MAX_SNBT_BYTES {
            return None;
        }
        let (tag, end) = sunrise_core::nbt::parse_snbt(snbt).ok()?;
        if !snbt.as_bytes()[end..].iter().all(u8::is_ascii_whitespace) {
            return None;
        }
        let stack = sunrise_items::item_nbt::item_from_persistent_nbt(&tag)?;
        self.items().get(&stack.item).is_some().then_some(stack)
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> NativeGiveStackGameExt1<'__sg> for G {}


use crate::game::tables::GameItems as _;
