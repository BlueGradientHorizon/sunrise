#!/usr/bin/env python3
"""Route station ingredient parsing and recipe fixtures through their owners."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "crates/sunrise-client/src"


def main():
    pending = {}

    def replace(path, old, new, count=1):
        source = pending.get(path)
        if source is None:
            source = (ROOT / path).read_text(encoding="utf-8")
        found = source.count(old)
        if found != count:
            raise SystemExit(f"{path}: expected {count} matches, found {found}: {old!r}")
        pending[path] = source.replace(old, new)

    crafting = "crafting.rs"
    replace(crafting, "/// The recipes one load reads,", """#[cfg(test)]
impl ResolvedRecipe {
    pub(crate) fn fixture(name: String, file: Arc<Value>) -> Self {
        Self {
            name,
            resolved: file.clone(),
            file,
        }
    }
}

/// The recipes one load reads,""")
    replace(crafting, "                        &recipe.resolved,\n                        items,",
            "                        &recipe.resolved,\n                        |value| Ingredient::parse(value, items),")
    replace(crafting, """        let recipe = |s: &str| {
            let file = Arc::new(serde_json::from_str::<Value>(s).unwrap());
            (file.clone(), file)
        };
        let named = |name: &str, s: &str| {
            let (file, resolved) = recipe(s);
            ResolvedRecipe {
                name: format!("mymod:{name}"),
                file,
                resolved,
            }
        };""", """        let named = |name: &str, s: &str| {
            let file = Arc::new(serde_json::from_str::<Value>(s).unwrap());
            ResolvedRecipe::fixture(format!("mymod:{name}"), file)
        };""")
    for spaces in (12, 16, 20):
        indent = " " * spaces
        name = 'format!("test:{kind}")' if spaces == 20 else "name.into()"
        replace(crafting, f"""{indent}ResolvedRecipe {{
{indent}    name: {name},
{indent}    resolved: file.clone(),
{indent}    file,
{indent}}}""", f"{indent}ResolvedRecipe::fixture({name}, file)")

    families = "crafting/ingredient/builtin_kinds_tests/families/"
    replace(families + "rest.rs", """        ResolvedRecipe {
            name: name.into(),
            file: file.clone(),
            resolved: file,
        }""", "        ResolvedRecipe::fixture(name.into(), file)")
    replace(families + "decoder_4209.rs", """        ResolvedRecipe {
            name: format!("fixture:{name}"),
            resolved: file.clone(),
            file,
        }""", '        ResolvedRecipe::fixture(format!("fixture:{name}"), file)')
    replace("lan/recipes.rs", """                sources.recipes.push(ResolvedRecipe {
                    name: name.clone(),
                    file: value.clone(),
                    resolved: value,
                });""", """                sources
                    .recipes
                    .push(ResolvedRecipe::fixture(name.clone(), value));""")

    stations = "stations.rs"
    replace(stations, "    pub(crate) fn parse(id: String, v: &Value, items: &Items) -> Option<Self> {",
            """    pub(crate) fn parse(
        id: String,
        v: &Value,
        ingredient: impl Fn(&Value) -> Option<Ingredient>,
    ) -> Option<Self> {""")
    replace(stations, "Ingredient::parse(value, items).map(Some)", "ingredient(value).map(Some)")
    for key, count in (("base", 2), ("template", 1), ("addition", 1), ("ingredient", 1)):
        replace(stations, f'Ingredient::parse(&v["{key}"], items)', f'ingredient(&v["{key}"])', count)
    replace(stations, ', &json, &items)', ', &json, |v| Ingredient::parse(v, &items))', 4)

    for path, source in pending.items():
        (ROOT / path).write_text(source, encoding="utf-8")


if __name__ == "__main__":
    main()
