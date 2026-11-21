use super::*;
use crate::client::menu_screen::OwnedMenuScreen;

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

/// Look up the screen factory registered for `menu`.
fn menu_screen_factory(menu: &str) -> Option<&'static MenuScreenRegistration> {
    installed()?.menu_screen_factories.get(menu)
}

/// Whether `menu` has a registered screen factory.
pub fn has_menu_screen_factory(menu: &str) -> bool {
    menu_screen_factory(menu).is_some()
}

/// Build a window's screen. The outer option distinguishes an unregistered factory
/// from one that declined or was refused. The returned instance is not part of registration.
pub fn create_menu_screen(
    host: &dyn crate::ClientHost,
    open: crate::MenuScreenOpen<'_>,
) -> Option<(&'static str, Result<Option<OwnedMenuScreen>, String>)> {
    let registration = menu_screen_factory(open.menu)?;
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
    /// opening a vanilla menu, receives callbacks. A bound recipe book keeps ownership with
    /// its declaring mod.
    pub(crate) fn menu_for_opener(
        &self,
        menu: &str,
        block: Option<&str>,
    ) -> &[Attached<dyn MenuPart>] {
        let parts = attached(&self.menus, || Some(menu));
        let owner = crate::RecipeBookLayout::declaring_mod(menu);
        let namespace = match owner {
            Some(owner) => owner,
            None if menu.starts_with("minecraft:") => {
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
    /// Register one client constructor for a namespaced menu type, matching NeoForge's
    /// `RegisterMenuScreensEvent.register` and vanilla's `MenuScreens.SCREENS`.
    /// Any mod may register a factory for a custom menu type (including another mod's
    /// or a non-built-in `minecraft:*` menu type). Built-in vanilla menu types that
    /// already have a screen in `MenuScreens.SCREENS` and duplicate registrations for
    /// the same menu type refuse installation, even if the error is ignored.
    /// The factory builds a fresh screen on the client for each opening. The
    /// window owns its callbacks and local slot moves until close or replacement.
    /// A package with a factory cannot declare `side = "server"`.
    fn register_menu_screen(
        &mut self,
        menu: &str,
        factory: Arc<dyn crate::MenuScreenFactory>,
    ) -> Result<(), String>;

    /// Declare this menu's book, authoritative recipe type and destination roles.
    /// The menu remains content-owned; loading checks these roles against its
    /// actual container slots and crafting declaration.
    /// Register a screen factory for this menu with [`Self::register_menu_screen`].
    /// Its layout must set [`crate::MenuScreenLayout::recipe_book`] to this `book`.
    /// Geometry is checked when the client constructs the screen, not here.
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
