use super::*;
use crate::{
    bake::{BlockModelTransform, ModelQuad, bake_block_model},
    model::{CuboidModel, Loader, Material, TextureSlot},
    resolve::{ModelLibrary, ResolvedLoader},
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// Answers a fixed list of quads for one loader id and counts the times that id is asked.
struct Fixed {
    id: &'static str,
    quads: Vec<LoaderQuad>,
    calls: AtomicUsize,
}
impl Fixed {
    fn new(id: &'static str, quads: Vec<LoaderQuad>) -> Self {
        Self {
            id,
            quads,
            calls: AtomicUsize::new(0),
        }
    }
}
impl LoaderGeometry for Fixed {
    fn quads(
        &self,
        loader: &str,
        _: &str,
        _: &Value,
        _: &BTreeMap<String, String>,
    ) -> Option<Vec<LoaderQuad>> {
        if loader != self.id {
            return None;
        }
        self.calls.fetch_add(1, Ordering::Relaxed);
        Some(self.quads.clone())
    }
}

/// The north face of a 4-pixel post in the middle of the block.
fn post(texture: &str) -> LoaderQuad {
    LoaderQuad {
        positions: [
            [0.625, 0.0, 0.375],
            [0.375, 0.0, 0.375],
            [0.375, 1.0, 0.375],
            [0.625, 1.0, 0.375],
        ],
        uvs: [[1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0]],
        texture: texture.into(),
        cull: Some(Direction::North),
        tint_index: 2,
        shade: false,
    }
}

fn library(models: &[(&str, &str)], loaders: Arc<dyn LoaderGeometry>) -> ModelLibrary {
    let models = models
        .iter()
        .map(|(name, json)| ((*name).to_owned(), CuboidModel::from_json(json).unwrap()))
        .collect();
    ModelLibrary::new(models).unwrap().with_loaders(loaders)
}

fn bake(library: &ModelLibrary, name: &str, turns: [u8; 3]) -> Vec<ModelQuad> {
    bake_block_model(
        &library.resolve(name).unwrap(),
        BlockModelTransform::new(turns, false),
    )
    .unwrap()
}

const BASE: &str = r#"{"textures":{"side":"block/post"}}"#;

#[test]
fn custom_loader_decodes_statelessly_and_canonicalizes_default_namespace() {
    let kept = CuboidModel::from_json(r#"{"loader":"asmtest:kept","width":3}"#).unwrap();
    let Some(Loader::Custom { kind, config }) = kept.loader else {
        panic!("a custom loader decodes as Loader::Custom");
    };
    assert_eq!(kind, "asmtest:kept");
    assert_eq!(config["width"], 3);
    assert_eq!(config["loader"], "asmtest:kept");

    let bare = CuboidModel::from_json(r#"{"loader":"asmtest_vanilla_ns","flag":true}"#).unwrap();
    assert!(matches!(
        bare.loader,
        Some(Loader::Custom { ref kind, .. }) if kind == "minecraft:asmtest_vanilla_ns"
    ));

    let leading_colon = CuboidModel::from_json(r#"{"loader":":leading"}"#).unwrap();
    assert!(matches!(
        leading_colon.loader,
        Some(Loader::Custom { ref kind, .. }) if kind == "minecraft:leading"
    ));

    let empty_path = CuboidModel::from_json(r#"{"loader":"asmtest:"}"#).unwrap();
    assert!(matches!(
        empty_path.loader,
        Some(Loader::Custom { ref kind, .. }) if kind == "asmtest:"
    ));
}

#[test]
fn loader_quads_turn_with_the_variant_and_take_the_models_textures() {
    let loaders = Arc::new(Fixed::new(
        "asmtest:post",
        vec![post("side"), post("absent")],
    ));
    let library = library(
        &[
            ("t:base", BASE),
            ("t:post", r#"{"parent":"t:base","loader":"asmtest:post"}"#),
        ],
        loaders.clone(),
    );
    let quads = bake(&library, "t:post", [0, 1, 0]);
    // The quad whose texture the model lacks is skipped; the other is baked.
    assert_eq!(quads.len(), 1);
    let quad = &quads[0];
    // A y quarter turn takes (x, z) to (1 - z, x): the north face faces east.
    assert_eq!(
        quad.positions,
        [
            [0.625, 0.0, 0.625],
            [0.625, 0.0, 0.375],
            [0.625, 1.0, 0.375],
            [0.625, 1.0, 0.625],
        ]
    );
    assert_eq!(quad.cull, Some(Direction::East));
    assert_eq!(quad.material.sprite, "minecraft:block/post");
    assert_eq!(quad.uv, post("side").uvs);
    assert_eq!((quad.tint_index, quad.shade), (2, false));
    // Each model is asked once for the same resolved textures, however often it is resolved.
    bake(&library, "t:post", [0, 0, 0]);
    assert_eq!(loaders.calls.load(Ordering::Relaxed), 1);
}

#[test]
fn unknown_loader_yields_none_while_registered_empty_loader_yields_registered_empty() {
    let loaders = Arc::new(Fixed::new("asmtest:empty", Vec::new()));
    let library = library(
        &[
            (
                "t:unknown",
                r##"{"loader":"asmtest:unregistered","textures":{"side":"block/post"},
                    "elements":[{"from":[0,0,0],"to":[16,16,16],"faces":{"up":{"texture":"#side"}}}]}"##,
            ),
            (
                "t:empty",
                r#"{"loader":"asmtest:empty","textures":{"side":"block/post"}}"#,
            ),
        ],
        loaders.clone(),
    );
    let resolved_unknown = library.resolve("t:unknown").unwrap();
    assert!(resolved_unknown.loader().is_none());
    let quads = bake(&library, "t:unknown", [0, 0, 0]);
    assert_eq!(quads.len(), 1, "the element's face only");
    assert_eq!(loaders.calls.load(Ordering::Relaxed), 0);

    let resolved_empty = library.resolve("t:empty").unwrap();
    assert!(matches!(
        resolved_empty.loader(),
        Some(ResolvedLoader::Registered(q)) if q.is_empty()
    ));
    assert_eq!(loaders.calls.load(Ordering::Relaxed), 1);
}

#[test]
fn inherited_loader_geometry_is_cached_per_model_identity() {
    struct Named(std::sync::Mutex<Vec<(String, BTreeMap<String, String>)>>);
    impl LoaderGeometry for Named {
        fn quads(
            &self,
            loader: &str,
            model: &str,
            _: &Value,
            textures: &BTreeMap<String, String>,
        ) -> Option<Vec<LoaderQuad>> {
            if loader != "asmtest:named" {
                return None;
            }
            self.0
                .lock()
                .unwrap()
                .push((model.to_owned(), textures.clone()));
            let mut quad = post("side");
            quad.tint_index = if model == "t:parent" { 1 } else { 7 };
            Some(vec![quad])
        }
    }
    let callbacks = Arc::new(Named(std::sync::Mutex::new(Vec::new())));
    let library = library(
        &[
            (
                "t:parent",
                r#"{"loader":"asmtest:named","textures":{"side":"block/post"}}"#,
            ),
            ("t:child", r#"{"parent":"t:parent"}"#),
        ],
        callbacks.clone(),
    );
    for _ in 0..2 {
        assert_eq!(bake(&library, "t:parent", [0; 3])[0].tint_index, 1);
        assert_eq!(bake(&library, "t:child", [0; 3])[0].tint_index, 7);
    }
    let expected_textures =
        BTreeMap::from([("side".to_owned(), "minecraft:block/post".to_owned())]);
    assert_eq!(
        *callbacks.0.lock().unwrap(),
        [
            ("t:parent".to_owned(), expected_textures.clone()),
            ("t:child".to_owned(), expected_textures),
        ]
    );
}

#[test]
fn resolve_with_textures_caches_loader_quads_per_effective_texture_map() {
    struct TextureTint(std::sync::Mutex<Vec<String>>);
    impl LoaderGeometry for TextureTint {
        fn quads(
            &self,
            loader: &str,
            _: &str,
            _: &Value,
            textures: &BTreeMap<String, String>,
        ) -> Option<Vec<LoaderQuad>> {
            if loader != "asmtest:tex_tint" {
                return None;
            }
            let sprite = textures.get("side").cloned().unwrap_or_default();
            self.0.lock().unwrap().push(sprite.clone());
            let mut quad = post("side");
            quad.tint_index = if sprite == "minecraft:block/gold_block" {
                10
            } else {
                20
            };
            Some(vec![quad])
        }
    }

    let make_override = |sprite: &str| {
        BTreeMap::from([(
            "side".to_owned(),
            TextureSlot::Material(Material {
                sprite: sprite.to_owned(),
                force_translucent: false,
            }),
        )])
    };
    let gold = make_override("minecraft:block/gold_block");
    let iron = make_override("minecraft:block/iron_block");

    // Test both resolution orders to prove neither leaks cached quads into the other.
    for (first, first_tint, second, second_tint) in [(&gold, 10, &iron, 20), (&iron, 20, &gold, 10)]
    {
        let callbacks = Arc::new(TextureTint(std::sync::Mutex::new(Vec::new())));
        let lib = library(
            &[(
                "t:tex",
                r#"{"loader":"asmtest:tex_tint","textures":{"side":"block/post"}}"#,
            )],
            callbacks.clone(),
        );
        let q1 = bake_block_model(
            &lib.resolve_with_textures("t:tex", first).unwrap(),
            BlockModelTransform::new([0; 3], false),
        )
        .unwrap();
        let q2 = bake_block_model(
            &lib.resolve_with_textures("t:tex", second).unwrap(),
            BlockModelTransform::new([0; 3], false),
        )
        .unwrap();
        let q1_again = bake_block_model(
            &lib.resolve_with_textures("t:tex", first).unwrap(),
            BlockModelTransform::new([0; 3], false),
        )
        .unwrap();
        assert_eq!(q1[0].tint_index, first_tint);
        assert_eq!(q2[0].tint_index, second_tint);
        assert_eq!(q1_again[0].tint_index, first_tint);
        assert_eq!(callbacks.0.lock().unwrap().len(), 2);
    }
}
