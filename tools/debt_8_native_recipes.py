#!/usr/bin/env python3
"""Replay the coupled native recipe cutover on the debt integration base.

Check every exact edit before writing any files. Run once on the original tree
with `python3 tools/debt_8_native_recipes.py` from any directory.
"""
from hashlib import sha256
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VOCAB = "crates/sunrise-mod-vocab/src/"
API = "crates/sunrise-mod-api/src/"
CLIENT = "crates/sunrise-client/src/"


class Migration:
    def __init__(self):
        self.files = {}

    def replace(self, path, old, new):
        source = self.files.get(path)
        if source is None:
            source = (ROOT / path).read_text(encoding="utf-8")
        count = source.count(old)
        if count != 1:
            raise SystemExit(f"{path}: expected one match for {old.splitlines()[0]!r}, found {count}")
        self.files[path] = source.replace(old, new)

    def span(self, path, start, end, expected, replacement):
        # Fingerprint long removals so an intervening edit cannot be lost.
        source = self.files[path]
        if source.count(start) != 1 or source.count(end) != 1:
            raise SystemExit(f"{path}: expected unique span boundaries {start!r} and {end!r}")
        first = source.index(start)
        last = source.index(end)
        old = source[first:last]
        if first >= last or sha256(old.encode("utf-8")).hexdigest() != expected:
            raise SystemExit(f"{path}: unexpected contents for span starting at {start!r}")
        self.replace(path, old, replacement)

    def write(self):
        for path, source in self.files.items():
            (ROOT / path).write_text(source, encoding="utf-8")


def main():
    migration = Migration()
    replace = migration.replace
    span = migration.span

    path = VOCAB + "parts/crafting_recipes.rs"
    replace(path, """//! Startup registration and portable values for mod crafting recipe kinds.
use super::*;
use crate::wire::{Reader, Wire, Writer};
""", """//! Startup registration and native values for mod crafting recipe kinds.
use super::*;
""")
    span(path, "/// A kind's opaque Wire-encoded value.", "// no wire: borrowed per-craft reader",
         "2030cd6a71b2e26f59dc55c1becf36cee3e6e361de98dba60ad6ab6ba0d7a99e", "")
    replace(path, """/// the file or craft; do not panic. Values must have an explicit Wire codec.
pub trait CraftingRecipeKind: Send + Sync + 'static {
    /// Mod-owned decoded recipe data.
    type Value: Send + Sync + 'static;
    /// Portable codec; the default refuses registration by kind name.
    fn codec(&self) -> Option<CraftingRecipeCodec<Self::Value>> {
        None
    }
""", """/// the file or craft; do not panic. The engine retains the decoded value with its
/// serializer for the lifetime of the loaded recipe generation.
pub trait CraftingRecipeKind: Send + Sync + 'static {
    /// Mod-owned recipe data decoded once and borrowed by subsequent callbacks.
    type Value: Send + Sync + 'static;
""")
    span(path, "/// Portable dispatch to the sole owner of a registered kind.", "impl dyn Registrar + '_ {",
         "7aded7dcc847ce122e66633e0ff4bfe8f6156ff8353c90eee01e195d3c1a8319", """// no wire: process-local registration owns a native JSON decoder.
/// Engine-owned registration of a recipe serializer.
#[derive(Clone)]
pub struct RegisteredRecipeSerializer(Arc<dyn RecipeDecoder>);

impl RegisteredRecipeSerializer {
    /// Decode a compact JSON object whose type field the loader has removed.
    pub fn decode(&self, json: &str) -> Result<LoadedCraftingRecipe, String> {
        Arc::clone(&self.0).load(json)
    }
}

trait RecipeDecoder: Send + Sync {
    fn load(self: Arc<Self>, json: &str) -> Result<LoadedCraftingRecipe, String>;
}

impl<K: CraftingRecipeKind> RecipeDecoder for K {
    fn load(self: Arc<Self>, json: &str) -> Result<LoadedCraftingRecipe, String> {
        let value = self.decode(json)?;
        Ok(LoadedCraftingRecipe(Arc::new(TypedRecipe {
            kind: self,
            value,
        })))
    }
}

// no wire: a recipe and its serializer are native values; only their JSON is synced.
/// Engine-owned dispatch to one loaded recipe and its owning serializer.
#[derive(Clone)]
pub struct LoadedCraftingRecipe(Arc<dyn RecipeDispatch>);

impl LoadedCraftingRecipe {
    /// Write the retained value for synchronization.
    pub fn write(&self) -> Result<String, String> {
        self.0.write()
    }

    /// Test the retained value against the grid snapshot.
    pub fn matches(&self, context: &dyn crate::LoopContext, grid: CraftingGrid) -> bool {
        self.0.matches(context, grid)
    }

    /// Assemble the result for this recipe.
    pub fn assemble(
        &self,
        context: &dyn crate::LoopContext,
        grid: CraftingGrid,
    ) -> Option<crate::ItemStack> {
        self.0.assemble(context, grid)
    }

    /// Return custom remainders for the pre-take grid, or request defaults.
    pub fn remaining_items(
        &self,
        context: &dyn CraftingRemainders,
        grid: CraftingGrid,
    ) -> Option<Vec<Option<crate::ItemStack>>> {
        self.0.remaining_items(context, grid)
    }
}

trait RecipeDispatch: Send + Sync {
    fn write(&self) -> Result<String, String>;
    fn matches(&self, context: &dyn crate::LoopContext, grid: CraftingGrid) -> bool;
    fn assemble(
        &self,
        context: &dyn crate::LoopContext,
        grid: CraftingGrid,
    ) -> Option<crate::ItemStack>;
    fn remaining_items(
        &self,
        context: &dyn CraftingRemainders,
        grid: CraftingGrid,
    ) -> Option<Vec<Option<crate::ItemStack>>>;
}

struct TypedRecipe<K: CraftingRecipeKind> {
    kind: Arc<K>,
    value: K::Value,
}

impl<K: CraftingRecipeKind> RecipeDispatch for TypedRecipe<K> {
    fn write(&self) -> Result<String, String> {
        self.kind.write(&self.value)
    }

    fn matches(&self, context: &dyn crate::LoopContext, grid: CraftingGrid) -> bool {
        self.kind.matches(context, &self.value, grid)
    }

    fn assemble(
        &self,
        context: &dyn crate::LoopContext,
        grid: CraftingGrid,
    ) -> Option<crate::ItemStack> {
        self.kind.assemble(context, &self.value, grid)
    }

    fn remaining_items(
        &self,
        context: &dyn CraftingRemainders,
        grid: CraftingGrid,
    ) -> Option<Vec<Option<crate::ItemStack>>> {
        self.kind.remaining_items(context, &self.value, grid)
    }
}

""")
    replace(path, """    /// Register a crafting kind once at startup. Duplicate names, foreign namespaces,
    /// and values without an explicit Wire codec refuse startup, naming the kind.
""", """    /// Register a crafting kind once at startup. Duplicate names and foreign namespaces
    /// refuse startup, naming the kind.
""")
    replace(path, "    /// Register one owner, or refuse a value without Wire support.\n",
            "    /// Register one serializer owner for a recipe type.\n")
    replace(path, "        handler: Option<Arc<dyn CraftingRecipeHandler>>,\n",
            "        handler: RegisteredRecipeSerializer,\n")
    replace(path, """/// Resolve an installed owner without scanning mods.
pub(crate) fn installed_crafting_recipe(name: &str) -> Option<Arc<dyn CraftingRecipeHandler>> {
    let (owner, recipe_type, handler) = installed()?.keys.recipe_serializers.get(name)?;
    if recipe_type != "minecraft:crafting" {
        return None;
    }
    crate::mod_index_on(*owner).then(|| handler.clone())
}
""", "")
    replace(path, ") -> Option<(String, Arc<dyn CraftingRecipeHandler>)> {\n",
            ") -> Option<(String, RegisteredRecipeSerializer)> {\n")
    replace(path, """pub(super) fn typed_handler<K: CraftingRecipeKind>(
    kind: K,
) -> Option<Arc<dyn CraftingRecipeHandler>> {
    kind.codec()
        .map(|codec| Arc::new(TypedKind { kind, codec }) as Arc<dyn CraftingRecipeHandler>)
}
""", """pub(super) fn typed_handler<K: CraftingRecipeKind>(kind: K) -> RegisteredRecipeSerializer {
    RegisteredRecipeSerializer(Arc::new(kind))
}
""")

    path = VOCAB + "parts/shared.rs"
    replace(path, """    pub(crate) recipe_serializers:
        HashMap<Box<str>, (usize, String, Arc<dyn crate::CraftingRecipeHandler>)>,
""", """    pub(crate) recipe_serializers:
        HashMap<Box<str>, (usize, String, crate::__private::RegisteredRecipeSerializer)>,
""")

    path = VOCAB + "exports/parts_crafting_recipes.rs"
    replace(path, """pub use parts::{
    CraftingGrid, CraftingRecipeCodec, CraftingRecipeHandler, CraftingRecipeKind,
    CraftingRecipeValue, CraftingRemainders,
};
""", """pub use parts::{CraftingGrid, CraftingRecipeKind, CraftingRemainders};
pub mod __private {
    pub use crate::parts::{LoadedCraftingRecipe, RegisteredRecipeSerializer};
}
""")

    path = VOCAB + "parts/installation_tokens/crafting_recipes.rs"
    replace(path, """/// Resolve only the enabled owner of this namespaced kind.
pub fn crafting_recipe(name: &str) -> Option<std::sync::Arc<dyn crate::CraftingRecipeHandler>> {
    crate::parts::installed_crafting_recipe(name)
}
""", "use crate::__private::RegisteredRecipeSerializer;\n\n")
    replace(path, """pub fn recipe_serializer(
    name: &str,
) -> Option<(String, std::sync::Arc<dyn crate::CraftingRecipeHandler>)> {
""", """pub fn recipe_serializer(name: &str) -> Option<(String, RegisteredRecipeSerializer)> {
""")

    path = VOCAB + "parts/recipe_serializers.rs"
    replace(path, "//! Read-only serializers share registration and portable values with crafting serializers.",
            "//! Read-only serializers share registration and native values with crafting serializers.")
    replace(path, """    /// Mod-owned portable value.
    type Value: Send + Sync + 'static;
    /// Missing Wire support refuses registration by serializer name.
    fn codec(&self) -> Option<crate::CraftingRecipeCodec<Self::Value>> {
        None
    }
""", """    /// Mod-owned recipe data retained with the serializer for the loaded generation.
    type Value: Send + Sync + 'static;
""")
    replace(path, """    fn codec(&self) -> Option<crate::CraftingRecipeCodec<Self::Value>> {
        self.0.codec()
    }
""", "")
    replace(path, """    fn codec(&self) -> Option<crate::CraftingRecipeCodec<String>> {
        Some(crate::CraftingRecipeCodec::wire())
    }
    fn decode(&self, json: &str) -> Result<String, String> {
        let value: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        value
            .is_object()
            .then(|| json.to_owned())
            .ok_or_else(|| "recipe must be an object".into())
    }
""", """    fn decode(&self, json: &str) -> Result<String, String> {
        // The loader supplies a compact object with `type` removed. Keep the old
        // string limit: 1 MiB minus the four bytes of its former length prefix.
        const MAX_RAW_RECIPE_JSON_BYTES: usize = 1024 * 1024 - 4;
        if json.len() > MAX_RAW_RECIPE_JSON_BYTES {
            return Err(format!(
                "raw recipe JSON exceeds {MAX_RAW_RECIPE_JSON_BYTES} bytes"
            ));
        }
        Ok(json.to_owned())
    }
""")
    replace(path, """        self.register_read_only_recipe_serializer(name, recipe_type, Raw);
    }
}
""", r'''        self.register_read_only_recipe_serializer(name, recipe_type, Raw);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_recipe_accepts_the_previous_json_limit_but_not_one_byte_more() {
        let padding = "x".repeat(1_048_572 - r#"{"value":""}"#.len());
        let at_limit = format!(r#"{{"value":"{padding}"}}"#);
        assert_eq!(Raw.decode(&at_limit).unwrap(), at_limit);
        let over_limit = format!(r#"{{"value":"{padding}x"}}"#);
        assert!(Raw.decode(&over_limit).is_err());
    }
}
''')

    path = API + "parts/crafting_recipes.rs"
    replace(path, "        handler: Option<Arc<dyn crate::CraftingRecipeHandler>>,\n",
            "        handler: RegisteredRecipeSerializer,\n")
    replace(path, """        let Some(handler) = handler else {
            self.refused.push(format!(
                "{} recipe serializer {id} requires a Wire value codec",
                self.mod_id
            ));
            return;
        };
""", "")
    replace(path, "    struct Literal {\n", "    struct Count(u32);\n    struct Literal {\n")
    replace(path, """        type Value = u32;
        fn codec(&self) -> Option<crate::CraftingRecipeCodec<u32>> {
            Some(crate::CraftingRecipeCodec::wire())
        }
        fn decode(&self, json: &str) -> Result<u32, String> {
""", """        type Value = Count;
        fn decode(&self, json: &str) -> Result<Count, String> {
""")
    replace(path, "                .map(|n| n as u32)\n", "                .map(|n| Count(n as u32))\n")
    replace(path, r'''        fn write(&self, value: &u32) -> Result<String, String> {
            Ok(format!("{{\"count\":{value}}}"))
''', r'''        fn write(&self, value: &Count) -> Result<String, String> {
            Ok(format!("{{\"count\":{}}}", value.0))
''')
    replace(path, """            value: &u32,
            grid: crate::CraftingGrid,
        ) -> bool {
            *value == 3
""", """            value: &Count,
            grid: crate::CraftingGrid,
        ) -> bool {
            value.0 == 3
""")
    replace(path, """            value: &u32,
            _: crate::CraftingGrid,
        ) -> Option<crate::ItemStack> {
            crate::ItemStack::new(self.result, *value)
""", """            value: &Count,
            _: crate::CraftingGrid,
        ) -> Option<crate::ItemStack> {
            crate::ItemStack::new(self.result, value.0)
""")
    replace(path, """            _: &u32,
            _: crate::CraftingGrid,
        ) -> Option<Vec<Option<crate::ItemStack>>> {
            let mut cells = vec![None; 9];
            cells[4] = crate::ItemStack::new(self.result, 1);
""", """            value: &Count,
            _: crate::CraftingGrid,
        ) -> Option<Vec<Option<crate::ItemStack>>> {
            let mut cells = vec![None; 9];
            cells[4] = crate::ItemStack::new(self.result, value.0 / 2);
""")
    span(path, "        let handler =\n            mod_vocab::__private::parts_installation::crafting_recipe",
         "    struct Invalid;",
         "3176861a2c5f148203ec9d602084c2f6791b2bd4056dda3e6e5261c3dccd1cc1", r'''        let (kind, serializer) =
            mod_vocab::__private::parts_installation::recipe_serializer("test:diagonal").unwrap();
        assert_eq!(kind, "minecraft:crafting");
        let loaded = serializer.decode(r#"{"count":3}"#).unwrap();
        let key = mod_vocab::__private::item_key("test:result").unwrap();
        let mut cells = vec![None; 9];
        for i in [0, 4, 8] {
            cells[i] = crate::ItemStack::new(key, 1);
        }
        let grid = crate::CraftingGrid::new(3, 3, cells).unwrap();
        assert!(loaded.matches(&Context, grid.clone()));
        let out = loaded.assemble(&Context, grid.clone()).unwrap();
        assert_eq!(out.count(), 3);
        assert_eq!(out.item(), key);
        assert_eq!(loaded.write().unwrap(), r#"{"count":3}"#);
        let mut expected = vec![None; 9];
        expected[4] = crate::ItemStack::new(key, 1);
        assert_eq!(
            loaded.remaining_items(&Context, grid.clone()),
            Some(expected)
        );
        let mut wrong = grid;
        wrong.cells[4] = None;
        assert!(!loaded.matches(&Context, wrong));
        assert!(serializer.decode("{").is_err());
        assert!(serializer.decode(r#"{"count":2}"#).is_err());
    }
''')
    replace(path, '            r.register_crafting_recipe_serializer("missing_wire", NoWire);\n', "")
    replace(path, "    fn duplicate_and_non_wire_kinds_refuse_by_name() {",
            "    fn duplicate_kinds_refuse_by_name() {")
    replace(path, """        assert!(
            errors
                .iter()
                .any(|e| e.contains("test:missing_wire") && e.contains("Wire")),
            "{errors:?}"
        );
""", "")

    path = CLIENT + "crafting/mod_kinds.rs"
    replace(path, "use mod_api::{CraftingGrid, CraftingRecipeHandler, CraftingRecipeValue};",
            "use mod_api::{__private::LoadedCraftingRecipe, CraftingGrid};")
    replace(path, """    handler: Arc<dyn CraftingRecipeHandler>,
    value: CraftingRecipeValue,
""", """    loaded: LoadedCraftingRecipe,
""")
    replace(path, "    let (_, handler) = mod_api::__private::parts_installation::recipe_serializer(kind)?;",
            "    let (_, serializer) = mod_api::__private::parts_installation::recipe_serializer(kind)?;")
    replace(path, "        handler.decode(&Value::Object(object).to_string())",
            "        serializer.decode(&Value::Object(object).to_string())")
    replace(path, """    let decoded = decoded.and_then(|value| {
        if value.bytes.len() <= 1024 * 1024 {
            Ok(value)
        } else {
            Err("decoded value exceeds 1 MiB".into())
        }
    });
    match decoded {
        Ok(value) => Some(ModRecipe {
            serializer: kind.into(),
            handler,
            value,
        }),
""", """    match decoded {
        Ok(loaded) => Some(ModRecipe {
            serializer: kind.into(),
            loaded,
        }),
""")
    replace(path, """            self.handler
                .matches(&context, &self.value, input.clone())
                .then(|| {
                    self.handler
                        .assemble(&context, &self.value, input)
                        .and_then(from_api)
                })
""", """            self.loaded
                .matches(&context, input.clone())
                .then(|| self.loaded.assemble(&context, input).and_then(from_api))
""")
    replace(path, """        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.handler.write(&self.value)
        }))
        .unwrap_or_else(|_| Err("serializer panicked".into()))
""", """        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.loaded.write()))
            .unwrap_or_else(|_| Err("serializer panicked".into()))
""")
    replace(path, "            recipe.handler.remaining_items(&view, &recipe.value, input)",
            "            recipe.loaded.remaining_items(&view, input)")

    # The book source loader also used the removed lookup. Keep the same test
    # for enabled custom crafting serializers; do not hide other recipe types.
    replace(CLIENT + "recipe_book.rs",
            "            let special = mod_api::__private::parts_installation::crafting_recipe(kind).is_some()\n",
            """            let special = mod_api::__private::parts_installation::recipe_serializer(kind)
                .is_some_and(|(recipe_type, _)| recipe_type == "minecraft:crafting")
""")

    for relative, codec, value in (
        ("crafting/mod_kinds_tests.rs", "mod_api::CraftingRecipeCodec", "u32"),
        ("crafting/mod_kinds_tests.rs", "mod_api::CraftingRecipeCodec", "()"),
        ("game/modding/crafting_recipe_kind_tests.rs", "mod_api::CraftingRecipeCodec", "u32"),
        ("game/modding/rectangular_recipe_remainder_tests.rs", "CraftingRecipeCodec", "()"),
    ):
        replace(CLIENT + relative, f"""    fn codec(&self) -> Option<{codec}<{value}>> {{
        Some({codec}::wire())
    }}
""", "")
    replace(CLIENT + "game/modding/rectangular_recipe_remainder_tests.rs",
            "use mod_api::{CraftingGrid, CraftingRecipeCodec, CraftingRecipeKind, ItemKey, ItemStack};",
            "use mod_api::{CraftingGrid, CraftingRecipeKind, ItemKey, ItemStack};")

    migration.write()


if __name__ == "__main__":
    main()
