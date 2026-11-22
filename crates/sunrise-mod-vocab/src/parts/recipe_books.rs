//! Custom book identities, independent of recipe categories and vanilla ordinals.

use crate::item::recipe_book::RecipeBookTypeKey;

/// Operations available during the existing shared mod registration phase.
pub trait RecipeBooksRegistrar {
    /// Register a book with an explicit `namespace:path` accepted by
    /// [`crate::Identifier::parse_namespaced`] (including slash-separated paths, with no default
    /// namespace or whitespace trimming) in a manifest-owned namespace.
    /// Registration imposes no separate book-count or 256-byte ID cap.
    /// Invalid or duplicate declarations refuse installation even if the caller ignores
    /// the error. The registry freezes before player state is loaded. Saved compound names
    /// and synchronized IDs are subject only to ordinary NBT modified UTF-8 and wire
    /// `Identifier.STREAM_CODEC` (32,767-byte) encodings, respectively.
    fn recipe_book_type(&mut self, id: &str) -> Result<RecipeBookTypeKey, String>;
    /// Register a book whose flags use explicit, case-sensitive fields in the player's
    /// `recipeBook` compound, like vanilla's `RecipeBookSettings.TypeSettings` codec.
    /// The book ID follows the same rules as [`Self::recipe_book_type`].
    /// Each true flag is saved as a byte; false flags are omitted. This book is not
    /// also saved in the engine extension. Names may contain any characters, must be
    /// nonempty, and must differ, with no separate byte-length cap beyond the NBT
    /// writer's modified UTF-8 encoding.
    /// Vanilla flag fields, `recipes`, `toBeDisplayed`, `sunrise:book_settings`, and
    /// other books' fields are reserved. Invalid declarations refuse installation even if ignored.
    fn recipe_book_type_with_fields(
        &mut self,
        id: &str,
        open_field: &str,
        filtering_field: &str,
    ) -> Result<RecipeBookTypeKey, String>;
    /// Give a book of this mod its own filter button look: four sprites and a tooltip key. Each
    /// sprite is an explicit GUI sprite ID accepted by [`crate::Identifier::parse_namespaced`]
    /// (including slash-separated paths, with no default namespace or whitespace trimming).
    /// The tooltip must be a nonempty translation key; neither sprites nor tooltips have a
    /// separate 256-byte registration cap. A book takes one look.
    /// A second declaration, a book registered by another mod, or a malformed ID
    /// refuses installation even if the caller ignores the error.
    fn recipe_book_filter_look(
        &mut self,
        book: &RecipeBookTypeKey,
        look: crate::RecipeBookFilterLook,
    ) -> Result<(), String>;
}
