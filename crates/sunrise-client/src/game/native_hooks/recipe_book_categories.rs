use super::*;

/// The hook points. Each does nothing in Sunrise.
impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// Declare categories for custom recipe types, with registered item icons.
    /// Asked lazily once per recipe table by `recipe_book_category`.
    /// The complete batch is validated atomically; any invalid declaration
    /// refuses the generation. Multiple categories may share a recipe type.
    /// This does not yet add tabs or book displays.
    pub(in crate::game) fn native_recipe_book_categories(
        &mut self,
        categories: &mut Vec<RecipeBookCategory>,
    ) {
        self.fire_recipe_book_categories(categories);
    }
}
