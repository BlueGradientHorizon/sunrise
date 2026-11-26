use sunrise_items::items::ItemStack;

pub const CROSSBOW: &str = "minecraft:crossbow";

pub const FIREWORK_ROCKET: &str = "minecraft:firework_rocket";

pub const ARROW: &str = "minecraft:arrow";

/// What `getProjectile` returned: the ammunition and, while it is still the
/// stack found in the inventory, the slot it is taken from.
pub(in crate::game) struct Ammo {
    pub(in crate::game) stack: ItemStack,
    pub(in crate::game) slot: Option<usize>,
}
