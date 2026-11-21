//! Concrete authoring extracted verbatim from the vocabulary layer.
use super::*;
#[cfg(test)]
#[path = "menus/book_tests.rs"]
mod book_tests;
#[cfg(test)]
#[path = "menus/screen_tests.rs"]
mod screen_tests;

impl PartRegistrar {
    fn is_builtin_menu(&self, menu: &str) -> bool {
        mod_vocab::__private::is_vanilla_menu_type(menu)
            || self.has_builtin_entry("minecraft:menu", menu)
    }
}

impl MenusRegistrar for PartRegistrar {
    fn register_menu_screen(
        &mut self,
        menu: &str,
        factory: Arc<dyn crate::MenuScreenFactory>,
    ) -> Result<(), String> {
        let result = (|| {
            if mod_vocab::__private::parts_installation::parts_installed() {
                return Err("registry is frozen");
            }
            crate::Identifier::parse_namespaced(menu)
                .ok_or("menu must be a namespaced identifier")?;
            let stock = self.is_builtin_menu(menu);
            let rows = self.menu_screen_factories.entry(menu.into()).or_default();
            if (!stock && !rows.is_empty()) || rows.contains_key(&self.mod_index) {
                return Err("duplicate menu screen factory");
            }
            rows.insert(
                self.mod_index,
                MenuScreenRegistration {
                    owner: self.mod_id,
                    index: self.mod_index,
                    factory,
                    panicked: Default::default(),
                },
            );
            Ok(())
        })();
        result.map_err(|reason| {
            let message = format!("mod {} menu {menu:?}: {reason}", self.mod_id);
            self.refused.push(message.clone());
            message
        })
    }

    fn menu_slot_exclusions(
        &mut self,
        menu: &str,
        block: Option<&str>,
        excluded: &[u16],
    ) -> Result<(), String> {
        let result = (|| {
            if mod_vocab::__private::parts_installation::parts_installed() {
                return Err("registry is frozen");
            }
            crate::Identifier::parse_namespaced(menu)
                .ok_or("menu must be a namespaced identifier")?;
            if let Some(block) = block {
                crate::Identifier::parse_namespaced(block)
                    .ok_or("opener must be a namespaced mod block identifier")?;
                if self.has_builtin_entry("minecraft:block", block) {
                    return Err("opener must be a mod block, not a built-in block");
                }
            }
            if self.is_builtin_menu(menu) && block.is_none() {
                return Err("a stock menu requires an explicit opener block");
            }
            let scopes = self.menu_slot_exclusions.entry(menu.into()).or_default();
            let scope = block.unwrap_or_default();
            if scopes.contains_key(scope) {
                return Err("duplicate menu slot exclusions");
            }
            let mut excluded = excluded.to_vec();
            excluded.sort_unstable();
            excluded.dedup();
            scopes.insert(
                scope.into(),
                MenuSlotExclusion {
                    owner: self.mod_id,
                    slots: excluded.into_boxed_slice(),
                },
            );
            Ok(())
        })();
        result.map_err(|reason| {
            let message = format!(
                "mod {} menu {menu:?} opener {block:?}: {reason}",
                self.mod_id,
            );
            self.refused.push(message.clone());
            message
        })
    }

    fn menu_recipe_book(
        &mut self,
        menu: &str,
        book: &crate::RecipeBookTypeKey,
        recipe_type: &str,
        layout: crate::RecipeBookLayout,
    ) -> Result<(), String> {
        let result: Result<(), String> = (|| {
            if mod_vocab::__private::parts_installation::parts_installed() {
                return Err("registry is frozen".into());
            }
            crate::Identifier::parse_namespaced(menu)
                .ok_or_else(|| "menu must be a namespaced identifier".to_owned())?;
            if self.is_builtin_menu(menu) {
                return Err("cannot bind a recipe book to a built-in menu".into());
            }
            if !self.recipe_book_types.contains_key(book.as_str()) {
                return Err(format!("unknown recipe book {}", book.as_str()));
            }
            if self.menu_recipe_books.contains_key(menu) {
                return Err("duplicate menu recipe book".into());
            }
            crate::Identifier::parse_namespaced(recipe_type)
                .ok_or_else(|| "recipe type must be a namespaced identifier".to_owned())?;
            layout.validate(256)?;
            self.menu_recipe_books.insert(
                menu.into(),
                MenuBookDeclaration {
                    owner: self.mod_id,
                    book: book.clone(),
                    recipe_type: recipe_type.into(),
                    layout,
                },
            );
            Ok(())
        })();
        result.map_err(|reason| {
            let message = format!("mod {} menu {menu:?}: {reason}", self.mod_id);
            self.refused.push(message.clone());
            message
        })
    }
    fn attach_menu_part(&mut self, menu: &str, kind: &'static str, part: Arc<dyn MenuPart>) {
        self.attach(|p| &mut p.menus, kind, menu, part);
    }
}
