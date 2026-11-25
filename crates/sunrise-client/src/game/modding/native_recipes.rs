//! Every item and recipe the game knows, made once for the mods that read
//! them through `native_recipes`, and the recipe book's known displays; placing a
//! recipe and creative gives. The recipes are the window's own data — the
//! content root's — with the server's recipes of the types its native mods
//! had it send (`sunrise:recipe_content`) in their place, while the known
//! displays follow the server's book as the recipe book does.
use super::api_values::{TryIntoApi, TryIntoEngine};
use super::{
    Game,
    native_hooks::{
        NativeItem as RecipeItem, NativeRecipe as Recipe, NativeRecipeKind as RecipeKind,
        NativeRecipeStack as RecipeStack, NativeRecipes as Recipes,
    },
};
use mod_api::{CreativeGiveError, LevelRef, RecipeInput, RecipeLookupError, RecipeMatch};
use std::{collections::HashMap, sync::Arc};
use sunrise_client::{
    crafting::{Ingredient, qualified},
    items::ItemStack,
    recipe_book::{Category, Display},
    stations::Smithing,
};

/// The tables, made once, and the recipe book's known displays with the
/// book's version they are of.
/// The most recipes a server's recipe content may carry.
const MAX_SYNCED_RECIPES: usize = 1_000_000;

/// The parts of one recipe update so far. A part whose serializer could not
/// be decoded refuses the whole update, once, when its last part comes: the
/// parts after it must not publish what is left, and the previous generation
/// stays.
#[derive(Default)]
struct IncomingRecipes {
    recipes: Vec<(String, serde_json::Value)>,
    refused: Option<String>,
}
/// A completed update: the recipes and the types they replace, or why it is
/// refused.
type RecipeUpdate = Result<(Vec<(String, serde_json::Value)>, Vec<String>), String>;
impl IncomingRecipes {
    /// Take one part; the finished update on the last.
    fn push(&mut self, part: sunrise_client::lan::recipes::ContentPart) -> Option<RecipeUpdate> {
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
pub(in crate::game) struct RecipeTables {
    tables: Option<Arc<Recipes>>,
    pub(in crate::game) books: Option<
        Result<
            sunrise_client::recipe_book::ModBookIndex,
            sunrise_client::recipe_book::DisplayLoadError,
        >,
    >,
    known: (Option<u64>, Arc<[i32]>),
    /// The server's recipes of the types its native mods sent (`sunrise:
    /// recipe_content`) over the window's own: what `Game::recipes` reads on
    /// a client once they came.
    pub(in crate::game) synced: Option<sunrise_client::crafting::Recipes>,
    /// The display numbers of those recipes: the server numbers its book's
    /// displays over its own recipe set, a world's data pack recipes
    /// included, so the book numbers them the same (`Game::display_index`).
    pub(in crate::game) synced_index: Option<sunrise_client::recipe_book::RecipeIndex>,
    /// The recipe content's parts so far, until its last.
    incoming: IncomingRecipes,
    /// The types that came before the player did: the mods hear of them
    /// once the helpers can answer (`recipes_received_now`).
    received: Option<Vec<String>>,
    /// The last authoritative property-set and stonecutter generation sent by a server.
    pub(in crate::game) server_properties: crate::recipe_book::ServerRecipeProperties,
    /// The station recipes of a client connected to a server (stonecutter order from server).
    pub(in crate::game) server_stations: Option<sunrise_client::stations::StationRecipes>,
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

#[cfg(test)]
impl RecipeTables {
    pub(in crate::game) fn install_test_recipes(
        &mut self,
        recipes: sunrise_client::crafting::Recipes,
    ) {
        self.synced = Some(recipes);
    }
}

/// Item id -> its index in [`Recipes::items`].
struct Index(HashMap<Arc<str>, u32>);
impl Index {
    fn stack(&self, item: &str, count: u32, components: String) -> Option<RecipeStack> {
        Some(RecipeStack {
            item: *self.0.get(item)?,
            count,
            components,
        })
    }
    fn of(&self, s: &ItemStack) -> Option<RecipeStack> {
        self.stack(&s.item, s.count, components(s))
    }
    fn any(&self, i: &Ingredient) -> Vec<RecipeStack> {
        i.order
            .iter()
            .filter_map(|id| self.stack(id, 1, String::new()))
            .collect()
    }
}

/// A stack's components beyond its item's, as vanilla's item syntax writes
/// them (`[minecraft:potion_contents={potion:"minecraft:swiftness"}]`), or "".
pub(in crate::game::client_side) fn components(s: &ItemStack) -> String {
    use sunrise_core::nbt::Tag;
    let Some(Tag::Compound(entries)) = sunrise_client::item_data::component_snapshot(s) else {
        return String::new();
    };
    let parts: Vec<String> = entries
        .iter()
        .map(|(k, v)| {
            format!(
                "{}={}",
                k.to_string_lossy(),
                sunrise_client::snbt::to_snbt(v)
            )
        })
        .collect();
    format!("[{}]", parts.join(","))
}

fn recipe(id: String, kind: RecipeKind) -> Recipe {
    Recipe {
        id,
        kind,
        category: String::new(),
        group: String::new(),
        width: 0,
        height: 0,
        slots: Vec::new(),
        results: Vec::new(),
        cooking_time: 0,
        experience: 0.0,
        book_id: -1,
    }
}

impl Game<'_> {
    pub(in crate::game) fn registered_recipe_book_category(
        &mut self,
        kind: &str,
    ) -> Option<mod_api::item::recipe_book::RecipeBookCategory> {
        self.ensure_mod_book_displays();
        self.recipe_tables
            .books
            .as_ref()?
            .as_ref()
            .ok()?
            .categories()
            .find(|category| category.recipe_type == kind)
            .cloned()
    }
    /// `place_recipe`: refused as `recipes.place` refuses it.
    pub(in crate::game) fn recipe_place(&mut self, book_id: i32, all: bool) -> Result<(), String> {
        if !self.mod_recipe_screen() {
            return Err("no screen that takes recipes is open".into());
        }
        if !self.known_displays().contains(&book_id) {
            return Err(format!(
                "recipe {book_id} is not in the player's recipe book"
            ));
        }
        self.mod_place_recipe(book_id, all);
        Ok(())
    }
    /// `give_stack`: `Inventory.add` of `stack` for a creative player, components kept and up
    /// to the stack's own maximum, then `MultiPlayerGameMode.handleCreativeModeItemAdd` for
    /// every slot it changed, sent now: the end of the tick's sync skips creative slots while
    /// a server menu is open. What did not fit comes back and is not dropped.
    pub(in crate::game::client_side) fn creative_give_stack(
        &mut self,
        stack: &mod_api::ItemStack,
    ) -> Result<Option<mod_api::ItemStack>, CreativeGiveError> {
        if self.is_logical_server() || self.player.is_none() || self.res.rest.get().is_none() {
            return Err(CreativeGiveError::Unavailable);
        }
        if !self.creative {
            return Err(CreativeGiveError::NotCreative);
        }
        let mut stack = stack.try_into_engine(self.items())?;
        if self.remote.is_some()
            && !self
                .creative_screen
                .0
                .catalog()
                .item_enabled(&stack, self.enabled_feature_ids())
        {
            return Err(CreativeGiveError::FeatureDisabled);
        }
        let max = self.stack_max_size(&stack);
        let inventory = self.inventory[..36].to_vec();
        let offhand = self.offhand.clone();
        self.insert_stack_up_to(&mut stack, max);
        for i in 0..36 {
            if self.inventory[i] != inventory[i] {
                self.remote_creative_slot_add(i);
            }
        }
        if self.offhand != offhand {
            self.remote_creative_slot_add(36);
        }
        if stack.count == 0 {
            return Ok(None);
        }
        stack
            .try_into_api()
            .map(Some)
            .map_err(|error| error.into_write_error().into())
    }
    /// The tables, made on the first call; none while the world resources
    /// they come from are still loading behind the title on a slow start.
    pub(in crate::game) fn made_recipes(&mut self) -> Option<Arc<Recipes>> {
        self.res.rest.get()?;
        if self.recipe_tables.tables.is_none() {
            self.recipe_tables.tables = Some(Arc::new(self.make_mod_recipes()));
        }
        self.recipe_tables.tables.clone()
    }
    /// The `book_id`s the player's recipe book holds now.
    pub(in crate::game) fn known_displays(&mut self) -> Arc<[i32]> {
        if self.res.rest.get().is_none() {
            return Arc::from([]);
        }
        let (version, known) = self.recipe_book_displays();
        if self.recipe_tables.known.0 != Some(version) {
            self.recipe_tables.known = (Some(version), known.into());
        }
        self.recipe_tables.known.1.clone()
    }

    /// `RecipesClientHost::recipe_book_ids`: the display ids the recipe book
    /// gives `recipe_id`, as `transfer_recipe` looks them up.
    pub(in crate::game) fn recipe_book_ids(&mut self, recipe_id: &str) -> Arc<[i32]> {
        if self.player.is_none() || self.res.rest.get().is_none() {
            return Arc::from([]);
        }
        self.viewer_book_ids(recipe_id)
    }

    fn make_mod_recipes(&mut self) -> Recipes {
        let names = &self.pack.names;
        let items = self.items();
        let mut tags: HashMap<&str, Vec<String>> = HashMap::new();
        for (tag, members) in &items.item_tags {
            for m in members {
                tags.entry(m).or_default().push(tag.clone());
            }
        }
        let table: Vec<RecipeItem> = items
            .registry_order
            .iter()
            .map(|id| RecipeItem {
                id: id.to_string(),
                name: names.display_item(id),
                tags: tags.remove(&**id).map_or_else(Vec::new, |mut t| {
                    t.sort();
                    t
                }),
            })
            .collect();
        let index = Index(
            items
                .registry_order
                .iter()
                .enumerate()
                .map(|(i, id)| (id.clone(), i as u32))
                .collect(),
        );
        let mut out = Vec::new();
        let cooking: HashMap<String, (u32, f32)> = self
            .recipes()
            .cooking
            .iter()
            .map(|c| (qualified(&c.id), (c.time, c.experience)))
            .collect();
        for e in self.recipe_book_entries() {
            let furnace = |c: Category| match c.name().split('_').next() {
                Some("blast") => RecipeKind::Blasting,
                Some("smoker") => RecipeKind::Smoking,
                _ => RecipeKind::Smelting,
            };
            let kind = match &e.display {
                Display::Shaped { .. } => RecipeKind::CraftingShaped,
                Display::Shapeless(_) => RecipeKind::CraftingShapeless,
                Display::Furnace(_) => furnace(e.category),
            };
            let mut r = recipe(e.id.clone(), kind);
            r.category = e.category.name().to_owned();
            r.group = e.group.clone().unwrap_or_default();
            r.book_id = e.display_id;
            r.slots = match &e.display {
                Display::Shaped {
                    width,
                    height,
                    cells,
                } => {
                    (r.width, r.height) = (*width as u32, *height as u32);
                    cells
                        .iter()
                        .map(|c| c.as_ref().map_or_else(Vec::new, |i| index.any(i)))
                        .collect()
                }
                Display::Shapeless(list) => list.iter().map(|i| index.any(i)).collect(),
                Display::Furnace(i) => vec![index.any(i)],
            };
            // Requirements shown as other stacks (an arrow's potions): the
            // requirement's index counts the filled slots only.
            let filled: Vec<usize> = (0..r.slots.len())
                .filter(
                    |&k| !matches!(&e.display, Display::Shaped { cells, .. } if cells[k].is_none()),
                )
                .collect();
            for (req, stacks) in &e.stacks {
                if let Some(&k) = filled.get(*req) {
                    r.slots[k] = stacks.iter().filter_map(|s| index.of(s)).collect();
                }
            }
            r.results = e.results().iter().filter_map(|s| index.of(s)).collect();
            if let Some(&(time, xp)) = cooking.get(&e.id) {
                (r.cooking_time, r.experience) = (time, xp);
            }
            out.push(r);
        }
        for c in &self.recipes().cooking {
            if c.kind == "minecraft:campfire_cooking" {
                let mut r = recipe(qualified(&c.id), RecipeKind::Campfire);
                r.group = c.group.clone();
                r.slots = vec![index.any(&c.ingredient)];
                r.results = index.of(&c.result).into_iter().collect();
                (r.cooking_time, r.experience) = (c.time, c.experience);
                out.push(r);
            }
        }
        let stations = self.stations();
        for s in &stations.recipes.stonecutting {
            let mut r = recipe(s.id.clone(), RecipeKind::Stonecutting);
            r.slots = vec![index.any(&s.input)];
            r.results = index.of(&s.result).into_iter().collect();
            out.push(r);
        }
        for s in &stations.recipes.smithing {
            let (id, slots, results) = match s {
                Smithing::Transform {
                    id,
                    template,
                    base,
                    addition,
                    result,
                } => (
                    id,
                    vec![
                        template.as_ref().map(|i| index.any(i)).unwrap_or_default(),
                        index.any(base),
                        addition.as_ref().map(|i| index.any(i)).unwrap_or_default(),
                    ],
                    index.of(&result.create()).into_iter().collect(),
                ),
                Smithing::Trim {
                    id,
                    template,
                    base,
                    addition,
                    ..
                } => (
                    id,
                    vec![index.any(template), index.any(base), index.any(addition)],
                    index.any(base),
                ),
            };
            let mut r = recipe(id.clone(), RecipeKind::Smithing);
            r.slots = slots;
            r.results = results;
            out.push(r);
        }
        for b in &stations.brewing.recipes {
            let potion = |p: &sunrise_client::brewing::PotionIngredient| -> Vec<RecipeStack> {
                let mut item_ids: Vec<&String> = p.items.iter().collect();
                item_ids.sort();
                let mut potions: Vec<&String> = p.potions.iter().flatten().collect();
                potions.sort();
                item_ids
                    .into_iter()
                    .flat_map(|item| {
                        let one = |c: String| index.stack(item, 1, c);
                        if potions.is_empty() {
                            one(String::new()).into_iter().collect::<Vec<_>>()
                        } else {
                            potions
                                .iter()
                                .filter_map(|p| {
                                    one(format!("[minecraft:potion_contents={{potion:\"{p}\"}}]"))
                                })
                                .collect()
                        }
                    })
                    .collect()
            };
            let mut r = recipe(b.id.clone(), RecipeKind::Brewing);
            r.slots = vec![potion(&b.input), potion(&b.reagent)];
            r.results = index.of(&b.output).into_iter().collect();
            out.push(r);
        }
        Recipes {
            items: table,
            recipes: out,
        }
    }
}

impl Game<'_> {
    /// A new play session (`Login`): what the last server sent is gone.
    pub(in crate::game::client_side) fn forget_synced_recipes(&mut self) {
        self.recipe_tables.synced = None;
        self.recipe_tables.synced_index = None;
        self.recipe_tables.incoming = IncomingRecipes::default();
        self.recipe_tables.received = None;
        self.recipe_tables.tables = None;
        self.forget_book();
    }
    /// `native_recipes_received` for the types that came, once the window
    /// has its player: the recipes of a join come before it, and the helpers
    /// answer nothing until then. Once a frame.
    pub(in crate::game::client_side) fn recipes_received_now(&mut self) {
        if self.player.is_some()
            && let Some(types) = self.recipe_tables.received.take()
        {
            self.native_recipes_received(&types);
            self.build_screen();
        }
    }
    /// A `sunrise:recipe_content` part the server sent the window. On the
    /// last, the window's tables take the server's recipes of those types in
    /// place of its own, and the mods hear of it (`RecipesReceivedEvent`).
    pub(in crate::game::client_side) fn remote_recipe_content(&mut self, data: &[u8]) {
        let part = match sunrise_client::lan::recipes::ContentPart::decode(data) {
            Ok(part) => part,
            Err(e) => {
                log::warn!("Sunrise: bad recipe content from the server: {e}");
                return;
            }
        };
        let (received, types) = match self.recipe_tables.incoming.push(part) {
            None => return,
            Some(Ok(update)) => update,
            Some(Err(why)) => {
                log::warn!("Sunrise: the server's recipes were not taken: {why}");
                return;
            }
        };
        // No type any more: back on the window's own recipes.
        if types.is_empty() {
            self.recipe_tables.synced_index = None;
            if self.recipe_tables.synced.take().is_some() {
                self.recipe_tables.tables = None;
                self.forget_book();
                self.recipe_tables.received = Some(types);
                self.recipes_received_now();
            }
            return;
        }
        let built = self
            .res
            .recipes()
            .with_recipe_content(self.items(), received, &types);
        match built {
            Ok((recipes, index)) => {
                self.recipe_tables.synced = Some(recipes);
                self.recipe_tables.synced_index = Some(index);
                // What was made of the window's own recipes is made again.
                self.recipe_tables.tables = None;
                self.forget_book();
                self.recipe_tables.received = Some(types);
                self.recipes_received_now();
            }
            Err(e) => log::warn!("Sunrise: the server's recipes were not taken: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    /// A part with a malformed serializer refuses the whole update: the
    /// valid last part after it publishes nothing, one refusal is reported
    /// when the update completes, and the next update is taken whole.
    #[test]
    fn a_bad_part_refuses_the_whole_update_at_its_last_part() {
        use sunrise_client::lan::recipes::ContentPart;
        let part = |text: &str| ContentPart::decode(text.as_bytes()).unwrap();
        let mut incoming = super::IncomingRecipes::default();
        assert!(
            incoming
                .push(part(
                    r#"{"recipes":[["a:one",{"type":"smelting"}]],"last":null}"#
                ))
                .is_none()
        );
        assert!(
            incoming
                .push(part(
                    r#"{"recipes":[["a:bad",{"type":"x:y:z"}]],"last":null}"#
                ))
                .is_none()
        );
        let refused = incoming.push(part(
            r#"{"recipes":[["a:two",{"type":"smelting"}]],"last":["minecraft:smelting"]}"#,
        ));
        assert!(refused.unwrap().unwrap_err().contains("a:bad"));
        let (recipes, types) = incoming
            .push(part(
                r#"{"recipes":[["a:three",{"type":"smelting"}]],"last":["minecraft:smelting"]}"#,
            ))
            .unwrap()
            .unwrap();
        assert_eq!(recipes.len(), 1);
        assert_eq!(recipes[0].0, "a:three");
        assert_eq!(types, ["minecraft:smelting"]);
    }

    use super::super::native_entity_refs::tests::{with_client_game, with_game, with_game_at};
    use mod_api::{ItemStack, RecipeInput, RecipeLookupError};

    fn stack(item: &str) -> Option<ItemStack> {
        ItemStack::new(
            mod_api::__private::item_key(&format!("minecraft:{item}")).unwrap(),
            1,
        )
    }

    /// With the server's crafting recipes synced, the book still holds the
    /// vanilla ones under their own display ids (a viewer's "+" reads them,
    /// `recipe_book_ids`), and a recipe only the server has gets one too.
    #[test]
    #[ignore = "daily"]
    fn synced_vanilla_recipes_keep_their_book_ids() {
        fn book_ids(game: &mut crate::game::Game<'_>, id: &str) -> Vec<i32> {
            (game.recipe_book_entries().into_iter())
                .filter(|e| e.id == id && e.display_id >= 0)
                .map(|e| e.display_id)
                .collect()
        }
        with_client_game(|game| {
            let ids = [
                "minecraft:stick",
                "minecraft:chest",
                "minecraft:iron_ingot_from_smelting_raw_iron",
            ];
            let own = ids.map(|id| book_ids(game, id));
            assert!(own.iter().all(|ids| !ids.is_empty()), "{own:?}");
            let types = ["minecraft:crafting", "minecraft:smelting"].map(str::to_owned);
            let pack = serde_json::json!({
                "type": "minecraft:crafting_shaped",
                "pattern": ["#", "#"],
                "key": {"#": "#minecraft:logs"},
                "result": {"id": "minecraft:stick", "count": 16},
            });
            let overlay = sunrise_world::loot::PackOverlay::NONE;
            let mut files =
                sunrise_items::recipe_sources::sources(&game.options.resources, &overlay).unwrap();
            files.push(("zzz:sticks_from_logs".to_owned(), pack));
            let server = sunrise_client::crafting::Recipes::from_sources(
                &game.options.resources,
                game.items(),
                &overlay,
                files,
                game.res.core.item_data.context(),
            )
            .unwrap();
            let sources = server.recipe_content(&types);
            for part in sunrise_client::lan::recipes::encode_content(&types, &sources, false) {
                game.remote_recipe_content(&part);
            }
            assert!(game.synced_recipes().is_some());
            assert_eq!(ids.map(|id| book_ids(game, id)), own);
            assert_eq!(book_ids(game, "zzz:sticks_from_logs").len(), 1);
        });
    }

    /// A content mod that ships files under `minecraft` (tags, as most do)
    /// doesn't make vanilla's namespace one of the root's content namespaces:
    /// listed, `recipe_sources::sources` read every vanilla recipe twice, as
    /// `stick` and `minecraft:stick`, and the book held two entries of each,
    /// numbered `n` and `n + 1`, so a viewer's "+" sent the neighbour's id. A
    /// vanilla recipe has exactly one book id, the server's, synced or not.
    #[test]
    #[ignore = "daily"]
    fn a_synced_vanilla_recipe_has_one_book_id_the_servers() {
        use std::path::{Path, PathBuf};
        fn book_ids(game: &mut crate::game::Game<'_>, id: &str) -> Vec<i32> {
            (game.recipe_book_entries().into_iter())
                .filter(|e| e.id == id && e.display_id >= 0)
                .map(|e| e.display_id)
                .collect()
        }
        let base = PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let scratch =
            std::env::temp_dir().join(format!("sunrise-one-book-id-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        let dir = scratch.join("tagger");
        for (name, text) in [
            (
                "sunrise-mod.toml",
                "id = \"tagger\"\nname = \"tagger\"\nversion = \"1.0\"\napi = \"0.2\"\ncapabilities = [\"registries\"]\n",
            ),
            (
                "data/minecraft/tags/item/tagger_extra.json",
                r#"{"values": ["minecraft:stick"]}"#,
            ),
        ] {
            let path = dir.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let package = sunrise_mod_package::Package::read(&dir).unwrap();
        let tagger = sunrise_client::mod_content::ContentMod {
            id: package.manifest.id.clone(),
            content: sunrise_mod_package::content::Content::read(&dir, &package.manifest).unwrap(),
            jars: Vec::new(),
        };
        let root = scratch.join("root");
        let inventory = sunrise_mod_package::enabled::EnabledMods::new([(
            "tagger".to_owned(),
            mod_api::Side::Both,
        )]);
        sunrise_client::mod_content::build(&base, Path::new(""), &[tagger], &root, &inventory)
            .unwrap();
        assert_eq!(sunrise_client::mod_content::namespaces(&root), ["tagger"]);
        with_game_at(root, |game| {
            let ids = [
                "minecraft:stick",
                "minecraft:stick_from_bamboo_item",
                "minecraft:chest",
                "minecraft:iron_ingot_from_smelting_raw_iron",
            ];
            // The integrated server numbers the book's displays by its index.
            let server = ids.map(|id| {
                let index = &game.stations().recipe_index;
                vec![index.first_display(id).unwrap()]
            });
            assert_eq!(ids.map(|id| book_ids(game, id)), server);
            let types = ["minecraft:crafting", "minecraft:smelting"].map(str::to_owned);
            let sources = game.recipes().recipe_content(&types);
            for part in sunrise_client::lan::recipes::encode_content(&types, &sources, false) {
                game.remote_recipe_content(&part);
            }
            assert!(game.synced_recipes().is_some());
            assert_eq!(ids.map(|id| book_ids(game, id)), server);
        });
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
