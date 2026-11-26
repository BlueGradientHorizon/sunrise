use crate::game::native_hooks::NativeRecipes as Recipes;
use std::sync::Arc;

/// The tables, made once, and the recipe book's known displays with the
/// book's version they are of.
/// The most recipes a server's recipe content may carry.
const MAX_SYNCED_RECIPES: usize = 1_000_000;

/// The parts of one recipe update so far. A part whose serializer could not
/// be decoded refuses the whole update, once, when its last part comes: the
/// parts after it must not publish what is left, and the previous generation
/// stays.
#[derive(Default)]
pub struct IncomingRecipes {
    recipes: Vec<(String, serde_json::Value)>,
    refused: Option<String>,
}
/// A completed update: the recipes and the types they replace, or why it is
/// refused.
pub(in crate::game) type RecipeUpdate =
    Result<(Vec<(String, serde_json::Value)>, Vec<String>), String>;
impl IncomingRecipes {
    /// Take one part; the finished update on the last.
    pub(in crate::game) fn push(
        &mut self,
        part: sunrise_sim::lan::recipes::ContentPart,
    ) -> Option<RecipeUpdate> {
        if self.refused.is_none() {
            self.refused = part.refused;
        }
        if self.refused.is_none() {
            self.recipes.extend(part.recipes);
            // The server is trusted to end its parts; one that never does is
            // dropped, not kept growing.
            if self.recipes.len() > MAX_SYNCED_RECIPES {
                self.recipes = Vec::new();
                self.refused = Some(format!(
                    "more than {MAX_SYNCED_RECIPES} recipes; the rest is dropped"
                ));
            }
        }
        let types = part.last?;
        let recipes = std::mem::take(&mut self.recipes);
        Some(match self.refused.take() {
            Some(why) => Err(why),
            None => Ok((recipes, types)),
        })
    }
}

#[derive(Default)]
pub struct RecipeTables {
    pub(in crate::game) tables: Option<Arc<Recipes>>,
    pub(in crate::game) books: Option<
        Result<sunrise_sim::recipe_book::ModBookIndex, sunrise_sim::recipe_book::DisplayLoadError>,
    >,
    pub(in crate::game) known: (Option<u64>, Arc<[i32]>),
    /// The server's recipes of the types its native mods sent (`sunrise:
    /// recipe_content`) over the window's own: what `Game::recipes` reads on
    /// a client once they came.
    pub(in crate::game) synced: Option<sunrise_sim::crafting::Recipes>,
    /// The display numbers of those recipes: the server numbers its book's
    /// displays over its own recipe set, a world's data pack recipes
    /// included, so the book numbers them the same (`Game::display_index`).
    pub(in crate::game) synced_index: Option<sunrise_sim::recipe_book::RecipeIndex>,
    /// The recipe content's parts so far, until its last.
    pub(in crate::game) incoming: IncomingRecipes,
    /// The types that came before the player did: the mods hear of them
    /// once the helpers can answer (`recipes_received_now`).
    pub(in crate::game) received: Option<Vec<String>>,
    /// The last authoritative property-set and stonecutter generation sent by a server.
    pub(in crate::game) server_properties: sunrise_sim::recipe_book::ServerRecipeProperties,
    /// The station recipes of a client connected to a server (stonecutter order from server).
    pub(in crate::game) server_stations: Option<sunrise_sim::stations::StationRecipes>,
}

impl RecipeTables {
    /// Drop all per-session synced recipe and book-display state on disconnect.
    pub(in crate::game) fn clear_session(&mut self) {
        self.tables = None;
        self.books = None;
        self.known = Default::default();
        self.synced = None;
        self.synced_index = None;
        self.incoming = IncomingRecipes::default();
        self.received = None;
        self.server_properties = Default::default();
        self.server_stations = None;
    }
}
