use super::*;
use crate::client::menu_screen::OwnedMenuScreen;

/// Built-in menu types from vanilla 26.3 `net.minecraft.world.inventory.MenuType`.
const VANILLA_MENU_TYPES: &[&str] = &[
    "minecraft:generic_9x1",
    "minecraft:generic_9x2",
    "minecraft:generic_9x3",
    "minecraft:generic_9x4",
    "minecraft:generic_9x5",
    "minecraft:generic_9x6",
    "minecraft:generic_3x3",
    "minecraft:crafter_3x3",
    "minecraft:anvil",
    "minecraft:beacon",
    "minecraft:blast_furnace",
    "minecraft:brewing_stand",
    "minecraft:crafting",
    "minecraft:enchantment",
    "minecraft:furnace",
    "minecraft:grindstone",
    "minecraft:hopper",
    "minecraft:lectern",
    "minecraft:loom",
    "minecraft:merchant",
    "minecraft:shulker_box",
    "minecraft:smithing",
    "minecraft:smoker",
    "minecraft:cartography_table",
    "minecraft:stonecutter",
];

/// Whether `menu` names one of vanilla 26.3's built-in `MenuType` entries.
// hidden: engine/registrar menu classification; authors pass menu ids directly.
#[doc(hidden)]
pub fn is_vanilla_menu_type(menu: &str) -> bool {
    VANILLA_MENU_TYPES.contains(&menu)
}

/// One registered permanent slot exclusion mask and its declaring mod.
// hidden: engine staging; mods register masks through `MenusRegistrar::menu_slot_exclusions`.
// exhaustive: engine-only staging carries the full owner and sorted slot indices.
#[doc(hidden)]
// no wire: process-local registration metadata with a static mod id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuSlotExclusion {
    /// The package id that declared this exclusion mask.
    pub owner: &'static str,
    /// Sorted, deduplicated backing slot indices excluded from menu interaction.
    pub slots: Box<[u16]>,
}

// no wire: frozen registration storage; each boundary value has its own Wire derive.
#[derive(Clone)]
pub(crate) struct MenuBookDeclaration {
    pub(crate) owner: &'static str,
    pub(crate) book: crate::RecipeBookTypeKey,
    pub(crate) recipe_type: String,
    pub(crate) layout: crate::RecipeBookLayout,
}

// no wire: frozen registration and panic state, never a window instance.
/// Engine lookup record for a registered constructor.
#[derive(Clone)]
pub(crate) struct MenuScreenRegistration {
    pub(crate) owner: &'static str,
    pub(crate) index: usize,
    pub(crate) factory: Arc<dyn crate::MenuScreenFactory>,
    pub(crate) panicked: Arc<std::sync::atomic::AtomicBool>,
}

/// Look up the factory for the resolved custom-menu owner or the actual stock opener.
fn menu_screen_factory(
    menu: &str,
    menu_owner: Option<&str>,
    opener: Option<&str>,
) -> Option<&'static MenuScreenRegistration> {
    let installed = installed()?;
    let rows = installed.menu_screen_factories.get(menu)?;
    if let Some(owner) = menu_owner {
        if let Some(index) = crate::__private::mod_index(owner)
            && let Some(reg) = rows.get(&index)
        {
            return Some(reg);
        }
        return (!is_vanilla_menu_type(menu) && rows.len() == 1)
            .then(|| rows.values().next())
            .flatten();
    }
    let opener = opener?;
    if let Some(index) = crate::runtime::content_owner_index(opener)
        && let Some(reg) = rows.get(&index)
    {
        return Some(reg);
    }
    let owner = installed.menu_slot_exclusions.get(menu)?.get(opener)?.owner;
    let index = crate::__private::mod_index(owner)?;
    rows.get(&index)
}

/// Whether the content menu's owner or the verified stock opener's owner supplies a constructor.
pub fn has_menu_screen_factory(menu: &str, menu_owner: Option<&str>, opener: Option<&str>) -> bool {
    menu_screen_factory(menu, menu_owner, opener).is_some()
}

/// Build a window's screen. The outer option distinguishes an unregistered factory
/// from one that declined or was refused. The returned instance is not part of registration.
pub fn create_menu_screen(
    host: &dyn crate::ClientHost,
    open: crate::MenuScreenOpen<'_>,
    menu_owner: Option<&str>,
) -> Option<(&'static str, Result<Option<OwnedMenuScreen>, String>)> {
    let registration = menu_screen_factory(open.menu, menu_owner, open.block)?;
    let screen = crate::callback_owner::call(
        registration.owner,
        registration.index,
        &registration.panicked,
        "menu screen factory",
        "menu_screen_factory",
        || {
            registration
                .factory
                .create(host, open)
                .map(|screen| OwnedMenuScreen::new(registration.owner, registration.index, screen))
        },
    );
    Some((registration.owner, screen))
}

impl HookParts {
    pub(crate) fn menu<'k>(
        &self,
        id: impl FnOnce() -> Option<&'k str>,
    ) -> &[Attached<dyn MenuPart>] {
        attached(&self.menus, id)
    }
    /// The menu's owner lookup: only the part whose package owns the menu type, or the block
    /// opening a built-in vanilla menu, receives callbacks. A bound recipe book, a custom
    /// menu screen factory, or an opener-scoped slot exclusion identifies its declaring mod
    /// directly without inferring ownership from the key's namespace.
    pub(crate) fn menu_for_opener(
        &self,
        menu: &str,
        block: Option<&str>,
    ) -> &[Attached<dyn MenuPart>] {
        let parts = attached(&self.menus, || Some(menu));
        if parts.is_empty() {
            return &[];
        }
        let owner = crate::RecipeBookLayout::declaring_mod(menu)
            .or_else(|| {
                (!is_vanilla_menu_type(menu))
                    .then(|| installed()?.menu_screen_factories.get(menu))
                    .flatten()
                    .filter(|rows| rows.len() == 1)
                    .and_then(|rows| rows.values().next())
                    .map(|reg| reg.owner)
            })
            .or_else(|| {
                let block = block?;
                installed()?
                    .menu_slot_exclusions
                    .get(menu)?
                    .get(block)
                    .map(|exclusion| exclusion.owner)
            });
        if let Some(owner) = owner {
            return match parts.iter().position(|p| p.mod_id == owner) {
                Some(i) => std::slice::from_ref(&parts[i]),
                None => &[],
            };
        }
        if is_vanilla_menu_type(menu) {
            let Some(block) = block else {
                return &[];
            };
            let Some(owner_index) = crate::runtime::content_owner_index(block) else {
                return &[];
            };
            return match parts.iter().position(|p| p.mod_index == owner_index) {
                Some(i) => std::slice::from_ref(&parts[i]),
                None => &[],
            };
        }
        if let Some(owner_index) = crate::runtime::content_owner_index(menu)
            && let Some(i) = parts.iter().position(|p| p.mod_index == owner_index)
        {
            return std::slice::from_ref(&parts[i]);
        }
        parts
    }
}

/// Whether the Parts attached to the menu type `menu` subscribe to `hook`; the game
/// asks once when a menu opens, so a menu with none skips the hook while it is open.
pub fn menu_listens(hook: HookId, menu: &str) -> bool {
    if !has_part_hook(hook) {
        return false;
    }
    parts(hook, "menu routing")
        .for_hook(hook)
        .is_some_and(|table| !table.menu(|| Some(menu)).is_empty())
}

/// Authoritative exclusions for the validated opener, or the menu type's default.
/// The caller checks these backing indices against the count of its actual menu.
pub fn menu_slot_exclusions(menu: &str, block: Option<&str>) -> &'static [u16] {
    let Some(scopes) = installed().and_then(|installed| installed.menu_slot_exclusions.get(menu))
    else {
        return &[];
    };
    // An empty scope key denotes the default; explicit block IDs cannot be empty.
    block
        .and_then(|block| scopes.get(block))
        .or_else(|| scopes.get(""))
        .map_or(&[], |exclusion| exclusion.slots.as_ref())
}

/// All permanent masks for a menu, including the default and each exact opener override.
/// Content-load validation checks recipe roles against each mask before the menu can open.
pub fn menu_slot_exclusion_masks(
    menu: &str,
) -> impl Iterator<Item = (Option<&'static str>, &'static [u16])> + use<> {
    installed()
        .and_then(|installed| installed.menu_slot_exclusions.get(menu))
        .into_iter()
        .flatten()
        .map(|(scope, exclusion)| {
            (
                (!scope.is_empty()).then_some(scope.as_str()),
                exclusion.slots.as_ref(),
            )
        })
}

impl dyn Registrar + '_ {
    /// Attach gameplay behavior to `menu`: a mod's, or one of vanilla's that
    /// a mod block opens. Register the client screen separately with
    /// [`MenusRegistrar::register_menu_screen`].
    pub fn menu_part<P: MenuPart>(&mut self, menu: &str, part: P) {
        self.attach_menu_part(menu, type_name::<P>(), Arc::new(part));
    }
}

/// Registrar operations for menus.
pub trait MenusRegistrar {
    /// Register one client constructor for a namespaced menu type. Any valid
    /// namespaced custom menu accepts at most one factory; its declaring mod is
    /// recorded directly without requiring namespace ownership. For a built-in
    /// vanilla menu, each mod may register one constructor, called only when a
    /// verified mod block opened for that mod triggers the menu.
    /// The factory builds a fresh screen on the client for each opening. The
    /// window owns its callbacks and local slot moves until close or replacement.
    /// If a custom menu or a stock opener with excluded slots lacks its factory,
    /// the client warns and leaves the current menu unchanged.
    /// A duplicate or malformed registration refuses installation, even if its
    /// error is ignored. A package with a factory cannot declare `side = "server"`.
    fn register_menu_screen(
        &mut self,
        menu: &str,
        factory: Arc<dyn crate::MenuScreenFactory>,
    ) -> Result<(), String>;

    /// Exclude container backing indices from server pickup and quick-move and
    /// from the client layout. Indices stay in place; the player inventory is unaffected.
    /// For a custom `menu`, use `None` for its default or `Some(block)` to replace it
    /// when that exact mod block's container opens this menu type.
    /// For a built-in vanilla menu, `block` must name an explicit non-built-in mod block.
    /// Both IDs must be namespaced; declaring-mod attribution is recorded directly rather
    /// than inferred from either key's namespace. Indices are sorted and deduplicated.
    /// An empty slice is an explicit empty mask.
    /// A duplicate or invalid declaration refuses installation even if the error
    /// is ignored. An index outside the actual container backing refuses the open.
    /// Screen layouts leave these positions empty, without renumbering later slots.
    /// Local hides never change this mask or server-side slot validation.
    fn menu_slot_exclusions(
        &mut self,
        menu: &str,
        block: Option<&str>,
        excluded: &[u16],
    ) -> Result<(), String>;

    /// Declare this custom menu's book, authoritative recipe type and destination roles.
    /// Any valid namespaced non-built-in menu id is accepted and attributed to the
    /// declaring mod. The menu remains content-owned; loading checks these roles against
    /// its actual container slots, crafting declaration and every permanent mask
    /// declared with [`Self::menu_slot_exclusions`].
    /// To show this menu, register a factory with [`Self::register_menu_screen`]
    /// whose layout sets [`crate::MenuScreenLayout::recipe_book`] to this `book`.
    /// Content can load without the factory. The client checks for it when the
    /// menu opens and validates geometry when the factory constructs the screen. The
    /// declaring mod owns the screen's presentation callbacks and slot controls.
    /// This does not turn display candidates into ingredient predicates.
    /// Built-in menus, unknown books, duplicate menus or malformed roles
    /// refuse installation, even when the caller ignores the error.
    fn menu_recipe_book(
        &mut self,
        menu: &str,
        book: &crate::RecipeBookTypeKey,
        recipe_type: &str,
        layout: crate::RecipeBookLayout,
    ) -> Result<(), String>;
    /// Attach `part`, of kind `kind`, to the menu type `menu`.
    fn attach_menu_part(&mut self, menu: &str, kind: &'static str, part: Arc<dyn MenuPart>);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_menu_list_matches_stock_menus_not_custom_minecraft_ids() {
        assert!(is_vanilla_menu_type("minecraft:hopper"));
        assert!(is_vanilla_menu_type("minecraft:generic_9x6"));
        assert!(is_vanilla_menu_type("minecraft:stonecutter"));
        assert!(!is_vanilla_menu_type("minecraft:custom_machine"));
        assert!(!is_vanilla_menu_type("demo:hopper"));
    }
}
