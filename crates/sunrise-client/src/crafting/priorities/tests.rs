use super::*;
use crate::{
    crafting::Recipes,
    enchant::Enchantments,
    items::{ItemStack, Items},
    stations::StationRecipes,
};
use serde_json::json;

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("sunrise-priorities-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("data/minecraft/recipe")).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn matching_shapeless_default_priority_and_replace_empty() {
    let root = Fixture::new("shapeless");
    let items = Items::for_tests(HashMap::new());
    let mut files = vec![];
    for (id, output) in [("a", "minecraft:stick"), ("b", "minecraft:diamond")] {
        files.push((
            format!("recipe/test:{id}"),
            vec![
                serde_json::to_vec(&json!({
                    "type": "minecraft:crafting_shapeless", "ingredients": ["minecraft:stone"],
                    "result": {"id": output}
                }))
                .unwrap(),
            ],
        ));
    }
    let grid = [Some(ItemStack::new("minecraft:stone", 1)), None, None, None];
    let context = crate::test_resources::server_loop_context();
    let load = |files: &[_]| {
        Recipes::load(
            &root.0,
            &items,
            &PackOverlay::new(files),
            &crate::test_resources::server_load(),
        )
        .unwrap()
    };
    let recipes = load(&files);
    let (id, output) = recipes.find(context, &grid, 2).unwrap();
    assert_eq!((id, output.item.as_ref()), ("test:a", "minecraft:stick"));
    files.push((
        "recipe_priorities".into(),
        vec![br#"{"entries":{"test:b":5}}"#.to_vec()],
    ));
    let recipes = load(&files);
    let (id, output) = recipes.find(context, &grid, 2).unwrap();
    assert_eq!((id, output.item.as_ref()), ("test:b", "minecraft:diamond"));
    files
        .last_mut()
        .unwrap()
        .1
        .push(br#"{"replace":true}"#.to_vec());
    let recipes = load(&files);
    let (id, output) = recipes.find(context, &grid, 2).unwrap();
    assert_eq!((id, output.item.as_ref()), ("test:a", "minecraft:stick"));
}

#[test]
fn layers_conditions_unknown_ids_and_integer_boundaries() {
    let root = Fixture::new("layers");
    fs::write(
        root.0.join("data/minecraft/recipe_priorities.json"),
        br#"{"entries":{"test:a":3,"test:absent":17,"test:min":-2147483648}}"#,
    )
    .unwrap();
    let files = vec![(
        "recipe_priorities".into(),
        vec![
            br#"{"entries":{"test:a":5,"test:b":-2}}"#.to_vec(),
            br#"{"replace":true,"neoforge:conditions":[{"type":"neoforge:never"}]}"#.to_vec(),
            br#"{"entries":{"test:b":2147483647}}"#.to_vec(),
        ],
    )];
    let priorities = Priorities::load(&root.0, &PackOverlay::new(&files)).unwrap();
    assert_eq!(priorities.get("test:a"), 5);
    assert_eq!(priorities.get("test:b"), i32::MAX);
    assert_eq!(priorities.get("test:min"), i32::MIN);
    assert_eq!(priorities.0.get("test:absent"), Some(&17));
    assert_eq!(priorities.get("test:unspecified"), 0);
}

#[test]
fn an_invalid_layer_is_refused_whole_naming_its_pack() {
    let root = Fixture::new("refused");
    let layer = |pack: &str, json: &str| {
        format!("{pack}: data/x/recipe_priorities.json\0{json}").into_bytes()
    };
    for (json, reason) in [
        // The root must be an object, whatever else it holds.
        ("[]", "the root must be an object"),
        ("5", "the root must be an object"),
        (r#"[false,{}]"#, "the root must be an object"),
        (r#"{"replace":"yes"}"#, "invalid type"),
        (r#"{"entries":[]}"#, "invalid type"),
        // Out of range, and with `replace`: nothing of the layer applies.
        (
            r#"{"replace":true,"entries":{"test:a":2147483648}}"#,
            "invalid value",
        ),
        ("{", "EOF"),
    ] {
        let files = vec![(
            "recipe_priorities".into(),
            vec![
                layer("good", r#"{"entries":{"test:a":5}}"#),
                layer("bad_pack", json),
            ],
        )];
        let error = Priorities::load(&root.0, &PackOverlay::new(&files))
            .err()
            .unwrap()
            .to_string();
        assert!(
            error.contains("bad_pack: data/x/recipe_priorities.json"),
            "{error}"
        );
        assert!(error.contains(reason), "{json}: {error}");
    }
    // A layer whose conditions fail is omitted before it is read as a value.
    let files = vec![(
        "recipe_priorities".into(),
        vec![layer(
            "off",
            r#"{"replace":"yes","neoforge:conditions":[{"type":"neoforge:never"}]}"#,
        )],
    )];
    assert!(Priorities::load(&root.0, &PackOverlay::new(&files)).is_ok());
}

#[test]
fn invalid_load_conditions_refuse_a_replacing_layer_without_clearing_priorities() {
    let conditions = LoadConditions::vanilla();
    let mut priorities = Priorities::default();
    let path = "data/neoforge/recipe_priorities.json";
    priorities
        .layer(
            "lower: data/neoforge/recipe_priorities.json",
            br#"{"entries":{"test:a":5}}"#,
            &conditions,
        )
        .unwrap();
    for cases in [
        json!(true),
        json!([false]),
        json!([{"type":"test:unknown"}]),
    ] {
        let bytes = serde_json::to_vec(&json!({
            "replace": true,
            "neoforge:conditions": cases,
            "entries": {"test:b": 12}
        }))
        .unwrap();
        let source = format!("higher: {path}");
        let error = priorities.layer(&source, &bytes, &conditions).unwrap_err();
        assert!(error.to_string().contains(&source), "{error}");
        assert_eq!(priorities.get("test:a"), 5);
        assert_eq!(priorities.get("test:b"), 0);
    }
}

#[test]
fn recipe_identifiers_are_canonical_and_a_colliding_layer_applies_nothing() {
    let conditions = LoadConditions::vanilla();
    let mut priorities = Priorities::default();
    let apply = |priorities: &mut Priorities, bytes: &[u8]| {
        priorities.layer(
            "test: data/neoforge/recipe_priorities.json",
            bytes,
            &conditions,
        )
    };
    apply(
        &mut priorities,
        br#"{"entries":{"a":5,"example:missing":9}}"#,
    )
    .unwrap();
    for json in [
        br#"{"replace":true,"entries":{"a":1,"minecraft:a":2}}"#.as_slice(),
        br#"{"replace":true,"entries":{":a":1,"a":2}}"#,
        br#"{"replace":true,"entries":{"a":1,"A":2}}"#,
    ] {
        assert!(apply(&mut priorities, json).is_err());
        assert_eq!(priorities.get("minecraft:a"), 5);
        assert_eq!(priorities.get("example:missing"), 9);
    }
    apply(&mut priorities, br#"{"entries":{":a":12}}"#).unwrap();
    assert_eq!(priorities.get("a"), 12);
    assert_eq!(priorities.get("example:missing"), 9);
}

struct Crusher;
#[mod_api::__subscriptions]
impl mod_api::Hooks for Crusher {
    fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
        r.register_recipe_type("test:crusher");
        r.register_raw_recipe_serializer("test:crusher", "test:crusher");
    }
}

#[test]
fn every_projection_uses_the_same_priority_order() {
    let mods = || [mod_api::ModEntry::new("test", Box::new(Crusher))];
    mod_platform::testing::with_mods(mods().into(), || {
        mod_platform::__private::part_tests::install_parts_fixture(&mods()).unwrap();
        projections_follow_one_order();
    });
}

fn projections_follow_one_order() {
    let root = Fixture::new("projections");
    let items = Items::for_tests(HashMap::new());
    let mut sources = Vec::new();
    let mut entries = serde_json::Map::new();
    for suffix in ["a", "b", "c"] {
        for kind in [
            "smelting",
            "blasting",
            "smoking",
            "campfire_cooking",
            "smithing_transform",
            "stonecutting",
            "crusher",
        ] {
            let id = format!("test:{kind}_{suffix}");
            let recipe = match kind {
                "smithing_transform" => {
                    json!({"type":"minecraft:smithing_transform", "base":"minecraft:stone", "result":{"id":"minecraft:diamond"}})
                }
                "stonecutting" => {
                    json!({"type":"minecraft:stonecutting", "ingredient":"minecraft:stone", "result":{"id":"minecraft:diamond"}})
                }
                "crusher" => json!({"type":"test:crusher", "output":suffix}),
                _ => {
                    json!({"type":format!("minecraft:{kind}"), "ingredient":"minecraft:stone", "result":{"id":"minecraft:diamond"}})
                }
            };
            entries.insert(id.clone(), json!(if suffix == "a" { -1 } else { 5 }));
            sources.push((id, recipe));
        }
    }
    let files = vec![(
        "recipe_priorities".into(),
        vec![serde_json::to_vec(&json!({"entries":entries})).unwrap()],
    )];
    let overlay = PackOverlay::new(&files);
    ResolvedSources::order_by_priority(&mut sources, &root.0, &overlay).unwrap();
    let resolved =
        ResolvedSources::resolve_collecting(sources, &crate::test_resources::server_load())
            .unwrap()
            .checked()
            .unwrap();
    let recipes = Recipes::from_resolved(&root.0, &items, &overlay, &resolved).unwrap();
    let stations = StationRecipes::from_sources(&root.0, &items, &resolved);
    let context = crate::test_resources::server_loop_context();
    let stone = ItemStack::new("minecraft:stone", 1);
    for kind in ["smelting", "blasting", "smoking", "campfire_cooking"] {
        assert_eq!(
            recipes
                .cook(context, &format!("minecraft:{kind}"), &stone)
                .unwrap()
                .id,
            format!("test:{kind}_b")
        );
    }
    assert_eq!(
        stations
            .smithing_recipe(context, &Enchantments::default(), None, Some(&stone), None)
            .unwrap()
            .0,
        "test:smithing_transform_b"
    );
    assert_eq!(
        stations
            .stonecutter(context, &stone)
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>(),
        [
            "test:stonecutting_b",
            "test:stonecutting_c",
            "test:stonecutting_a"
        ]
    );
    assert_eq!(
        recipes.by_type["test:crusher"]
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>(),
        ["test:crusher_b", "test:crusher_c", "test:crusher_a"]
    );
    // Smithing's serializers share one recipe type.
    for (kind, recipe_type) in [
        ("smelting", "minecraft:smelting"),
        ("blasting", "minecraft:blasting"),
        ("smoking", "minecraft:smoking"),
        ("campfire_cooking", "minecraft:campfire_cooking"),
        ("smithing_transform", "minecraft:smithing"),
        ("stonecutting", "minecraft:stonecutting"),
    ] {
        let ids: Vec<_> = recipes.by_type[recipe_type]
            .iter()
            .map(|r| r.id.clone())
            .collect();
        assert_eq!(
            ids,
            [
                format!("test:{kind}_b"),
                format!("test:{kind}_c"),
                format!("test:{kind}_a")
            ]
        );
    }
}
