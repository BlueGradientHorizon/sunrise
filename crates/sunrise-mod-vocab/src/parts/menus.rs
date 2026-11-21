use super::*;

// no wire: frozen registration storage; each boundary value has its own Wire derive.
#[derive(Clone)]
pub(crate) struct MenuBookDeclaration {
    pub(crate) owner: &'static str,
    pub(crate) book: crate::RecipeBookTypeKey,
    pub(crate) recipe_type: String,
    pub(crate) layout: crate::RecipeBookLayout,
}

impl HookParts {
    pub(crate) fn menu<'k>(
        &self,
        id: impl FnOnce() -> Option<&'k str>,
    ) -> &[Attached<dyn MenuPart>] {
        attached(&self.menus, id)
    }
    /// The menu's owner lookup: only the part whose package owns the menu type, or the block
    /// opening a vanilla menu, receives callbacks. A bound recipe book keeps ownership with
    /// its declaring mod.
    pub(crate) fn menu_screen(&self, menu: &str, block: Option<&str>) -> &[Attached<dyn MenuPart>] {
        let parts = attached(&self.menus, || Some(menu));
        let owner = crate::RecipeBookLayout::declaring_mod(menu);
        let namespace = match owner {
            Some(owner) => owner,
            None if vanilla_menu(menu) => {
                let Some((namespace, _)) = block.and_then(|b| b.split_once(':')) else {
                    return &[];
                };
                namespace
            }
            None => {
                let Some((namespace, _)) = menu.split_once(':') else {
                    return &[];
                };
                namespace
            }
        };
        match parts
            .iter()
            .position(|p| owns_namespace(p.mod_id, namespace))
        {
            Some(i) => std::slice::from_ref(&parts[i]),
            None => &[],
        }
    }
}

/// The hooks a menu type's screen is made of, one part per screen answering them all.
const SCREEN_HOOKS: [HookId; 4] = [
    HookId::Screen,
    HookId::ScreenDraw,
    HookId::ScreenClick,
    HookId::ScreenTooltip,
];

/// Whether `part` answers a screen hook: then it owns the screens it is asked for.
pub fn lays_out_screen(part: &dyn MenuPart) -> bool {
    let mut subscribed = part.menu_subscribed().iter().flatten();
    subscribed.any(|&hook| {
        SCREEN_HOOKS
            .iter()
            .any(|&screen| screen as usize == hook as usize)
    })
}

/// Whether screen parts of mods `a` and `b` on menu type `menu` would own the same screens,
/// which registration refuses: always on a mod's menu type, and on one of vanilla's only for
/// one mod, as each mod's blocks get their own mod's part (`HookParts::menu_screen`).
pub fn screens_shared(menu: &str, a: &str, b: &str) -> bool {
    !vanilla_menu(menu) || a == b
}

fn vanilla_menu(menu: &str) -> bool {
    menu.starts_with("minecraft:")
}

/// The mod that bound the menu's recipe book or owns the screen of `menu`
/// opened as `block`'s menu.
pub fn menu_screen_owner(menu: &str, block: Option<&str>) -> Option<&'static str> {
    if let Some(owner) = crate::RecipeBookLayout::declaring_mod(menu) {
        return Some(owner);
    }
    SCREEN_HOOKS
        .into_iter()
        .filter(|&hook| has_part_hook(hook))
        .find_map(|hook| {
            let parts = parts(hook, "screen owner");
            parts
                .for_hook(hook)?
                .menu_screen(menu, block)
                .first()
                .map(|p| p.mod_id)
        })
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
        .map_or(&[], |slots| slots.as_ref())
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
        .map(|(scope, slots)| {
            (
                (!scope.is_empty()).then_some(scope.as_str()),
                slots.as_ref(),
            )
        })
}

impl dyn Registrar + '_ {
    /// Attach `part` to the menu type `menu`: a mod's, or one of vanilla's that a mod block opens.
    pub fn menu_part<P: MenuPart>(&mut self, menu: &str, part: P) {
        self.attach_menu_part(menu, type_name::<P>(), Arc::new(part));
    }
}

/// Registrar operations for menus.
pub trait MenusRegistrar {
    /// Exclude container backing indices from server pickup and quick-move and
    /// from the client layout. Indices stay in place; the player inventory is unaffected.
    /// The owning mod of a custom `menu` may use `None` for its default or `Some(block)`
    /// to replace it when that exact mod block's container opens this menu type.
    /// For a stock menu, `block` must name an explicit mod block owned by the registrar.
    /// Both IDs must be namespaced. Indices are sorted and deduplicated.
    /// An empty slice is an explicit empty mask.
    /// A duplicate or out-of-scope declaration refuses installation even if the error
    /// is ignored. An index outside the actual container backing refuses the open.
    /// Screen layouts leave these positions empty; local hides never change this mask.
    fn menu_slot_exclusions(
        &mut self,
        menu: &str,
        block: Option<&str>,
        excluded: &[u16],
    ) -> Result<(), String>;

    /// Declare this menu's book, authoritative recipe type and destination roles.
    /// The menu remains content-owned; loading checks these roles against its
    /// actual container slots, crafting declaration and every permanent mask
    /// declared with [`Self::menu_slot_exclusions`].
    /// Attach a `MenuPart` whose `screen` hook supplies geometry on the client
    /// and sets `MenuScreenLayout::recipe_book` to this `book`. The declaring mod
    /// owns the screen's callbacks and slot controls; a foreign screen part is refused.
    /// This does not turn display candidates into ingredient predicates.
    /// Invalid ownership, unknown books, duplicate menus or malformed roles
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
