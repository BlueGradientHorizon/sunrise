//! A special model of a type a mod registered draws the resource models its renderer answers
//! (`ItemEnv::special_model_parts`), with the `base` model's display transforms, particle and GUI
//! light, in every display context and after a reload. Original assets: one cube A, two cubes B.
use super::*;
use mod_api::{
    __private::parts_installation::item_key, ClientHost, ItemDisplayContext, ItemStack,
    SpecialModelContext, SpecialModelFactory, SpecialModelPart, SpecialModelPolicy,
    SpecialModelQuad, SpecialModelRenderer, SpecialPartContent, testing::FakeHost,
};
use std::fs;
use sunrise_assets::{
    item_model::{PreparedParts, RegisteredSpecialModel, SpecialContent, SpecialPart},
    math::Mat4 as AssetMatrix,
    resource_source::{Resource, ResourceRoot},
};

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Pan {
    handle: u8,
}

impl SpecialModelRenderer for Pan {
    fn extract(
        &self,
        _: &dyn ClientHost,
        input_stack: &ItemStack,
        _: &SpecialModelContext,
    ) -> Result<Vec<SpecialModelPart>, String> {
        let stack_part = |key, count| {
            SpecialModelPart::stack(
                ItemStack::new(item_key(key), count).unwrap(),
                ItemDisplayContext::Gui,
            )
        };
        let part = |model| {
            SpecialModelPart::new(model)
                .translated([f32::from(self.handle) / 8.0, 0.0, 0.0])
                .with_tints(vec![0xFFCC_2211])
        };
        let a = || part("spt:item/a");
        let b = || part("spt:item/b");
        let parts = match input_stack.count() {
            1 => vec![a()],
            3 => vec![a(), part("spt:item/nope")],
            4 => return Err("the test renderer refuses this stack".into()),
            5 => vec![
                a().with_transform([2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0]),
            ],
            6 | 7 => {
                vec![a().with_tints(vec![[WHITE, GREY][input_stack.count() as usize - 6] as u32])]
            }
            // The plain cube item nested beside B; then the pan itself, a cycle.
            8 => vec![stack_part(1, 1).translated([0.25, 0.0, 0.0]), b()],
            9 => vec![stack_part(0, 9).translated([0.25, 0.0, 0.0])],
            // A to B to A with a valid sibling model: the cycle refuses every answer above it.
            10 => vec![b(), stack_part(0, 11)],
            11 => vec![a(), stack_part(0, 10)],
            // 64 cubes, 65, and the same below a first level: draws expand 1 + 64 and 1 + 63.
            20 | 21 => (0..44 + input_stack.count())
                .map(|_| stack_part(1, 1))
                .collect(),
            22 => vec![b(), stack_part(0, 20)],
            23 => vec![b(), stack_part(0, 24)],
            24 => (0..63).map(|_| stack_part(1, 1)).collect(),
            _ => vec![b()],
        };
        Ok(parts)
    }
}

/// Bridge the fixture's two synthetic items to the recipe consumer, without booting a game.
struct PanEnv<'r>(&'r ItemRenderer, FakeHost);
impl ItemEnv for PanEnv<'_> {
    fn special_model_parts(
        &self,
        base: &str,
        model: &RegisteredSpecialModel,
        input_stack: &StackView,
        _: &Context,
    ) -> Option<PreparedParts> {
        let model = model.0.downcast_ref::<BakedSpecialModel>().unwrap();
        assert_eq!((model.kind.as_ref(), base), ("spt:pan", "spt:item/base"));
        let api_stack = ItemStack::new(item_key(0), input_stack.count as u32).unwrap();
        let parts: Vec<_> = model
            .renderer
            .extract(&self.1, &api_stack, &SpecialModelContext::STACK)
            .ok()?
            .into_iter()
            .map(|part| {
                let [a, b, c, d, e, f, g, h, i, j, k, l] = part.transform;
                let content = match part.content {
                    SpecialPartContent::Model(model) => SpecialContent::Model(model),
                    SpecialPartContent::Stack {
                        stack: inner,
                        display: ItemDisplayContext::Gui,
                    } => SpecialContent::Stack {
                        stack: stack(
                            if inner.item() == item_key(0) {
                                "spt:pan"
                            } else {
                                "spt:cube"
                            },
                            inner.count() as i32,
                        ),
                        display: DisplayContext::Gui,
                    },
                    _ => unreachable!("the pan fixture only gives known recipes"),
                };
                SpecialPart {
                    content,
                    pose: AssetMatrix::from_row_major([
                        a, b, c, d, e, f, g, h, i, j, k, l, 0.0, 0.0, 0.0, 1.0,
                    ]),
                    tints: part.tints.into_iter().map(|argb| argb as i32).collect(),
                }
            })
            .collect();
        Some(self.0.prepare_parts(base, &parts))
    }
}

fn pan_factories(on_bake: impl Fn(u8) + Send + Sync + 'static) -> SpecialModelFactories {
    BTreeMap::from([(
        "spt:pan".into(),
        (
            SpecialModelPolicy::Stack,
            SpecialModelFactory::new(
                |config| config.decode::<Pan>(),
                move |pan, _| {
                    on_bake(pan.handle);
                    Ok(pan)
                },
            ),
        ),
    )])
}

/// White and an encoded 0.5 grey, packed vanilla-space ARGB.
const WHITE: i32 = 0xFFFF_FFFF_u32 as i32;
const GREY: i32 = 0xFF80_8080_u32 as i32;

/// `n` separated 4-pixel cubes, every face tinted by index 0.
fn cubes(n: usize) -> String {
    let faces = ["north", "east", "south", "west", "up", "down"]
        .map(|f| format!(r##""{f}":{{"uv":[0,0,4,4],"texture":"#all","tintindex":0}}"##))
        .join(",");
    let elements = (0..n)
        .map(|i| {
            let x = i * 5;
            format!(
                r#"{{"from":[{x},0,0],"to":[{},4,4],"faces":{{{faces}}}}}"#,
                x + 4
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(r#"{{"textures":{{"all":"spt:item/grey"}},"elements":[{elements}]}}"#)
}

fn write(root: &std::path::Path, name: &str, data: &str) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, data).unwrap();
}

/// The packs in `root`, with B of `b_cubes` cubes.
fn renderer(
    root: &std::path::Path,
    b_cubes: usize,
    handle: u8,
    factories: &SpecialModelFactories,
) -> ItemRenderer {
    write(root, "items.json", "{}");
    write(
        root,
        "assets/minecraft/atlases/items.json",
        r#"{"sources":[{"type":"single","resource":"spt:item/grey"},{"type":"single","resource":"spt:item/flour"}]}"#,
    );
    super::glint_tests::png(
        &root.join("assets/spt/textures/item/grey.png"),
        16,
        16,
        [128, 128, 128, 255],
    );
    super::glint_tests::png(
        &root.join("assets/spt/textures/item/flour.png"),
        16,
        16,
        [250, 250, 250, 255],
    );
    write(
        root,
        "assets/spt/items/pan.json",
        &format!(
            r#"{{"model":{{"type":"minecraft:special","base":"spt:item/base","model":{{"type":"spt:pan","handle":{handle}}}}}}}"#
        ),
    );
    write(
        root,
        "assets/spt/items/cube.json",
        r#"{"model":{"type":"minecraft:model","model":"spt:item/a"}}"#,
    );
    // Cube A tinted white or grey by vanilla's constant tint source.
    for (name, tint) in [("white_cube", WHITE), ("grey_cube", GREY)] {
        write(
            root,
            &format!("assets/spt/items/{name}.json"),
            &format!(
                r#"{{"model":{{"type":"minecraft:model","model":"spt:item/a","tints":[{{"type":"minecraft:constant","value":{tint}}}]}}}}"#
            ),
        );
    }
    write(
        root,
        "assets/spt/items/stranger.json",
        r#"{"model":{"type":"minecraft:special","base":"spt:item/base","model":{"type":"spt:unregistered"}}}"#,
    );
    // The base has no geometry of its own: its display transforms differ by context, its
    // particle is flour and its GUI light is flat.
    write(
        root,
        "assets/spt/models/item/base.json",
        r#"{"gui_light":"front","textures":{"particle":"spt:item/flour"},"display":{
            "ground":{"scale":[0.5,0.5,0.5]},
            "fixed":{"rotation":[0,180,0]},
            "head":{"translation":[0,13,7]},
            "thirdperson_righthand":{"rotation":[75,45,0],"translation":[0,2.5,0]},
            "gui":{"rotation":[30,225,0],"scale":[0.625,0.625,0.625]}}}"#,
    );
    write(root, "assets/spt/models/item/a.json", &cubes(1));
    write(root, "assets/spt/models/item/b.json", &cubes(b_cubes));
    let data = crate::test_resources::item_data(
        root,
        sunrise_world::native_load_context::DataLoadOrigin::ClientPreload,
    )
    .unwrap();
    let root = ResourceRoot::directory(root.to_path_buf());
    ItemRenderer::load_tables(
        &root,
        crate::model_library::ParsedModels::load(root.source()).into_item_library(),
        true,
        &data,
        None,
        Some(factories),
    )
    .unwrap()
}

fn stack(item: &str, count: i32) -> StackView {
    let mut stack = StackView::new(item, count);
    stack
        .patch
        .insert("minecraft:item_model".into(), Some(serde_json::json!(item)));
    stack
}

fn sprites() -> HashMap<String, Rect> {
    let rect = Rect {
        u0: 0.0,
        v0: 0.0,
        u1: 1.0,
        v1: 1.0,
        w: 16,
        h: 16,
    };
    HashMap::from([
        ("spt:item/grey".to_owned(), rect),
        ("spt:item/flour".to_owned(), rect),
    ])
}

fn quads(renderer: &ItemRenderer, state: &ItemRenderState) -> Vec<ItemQuad> {
    let mut quads = Vec::new();
    state.submit(renderer, &sprites(), &NoHost, Mat4::IDENTITY, &mut quads);
    quads
}

#[test]
fn an_invalid_winning_registered_model_does_not_load_a_lower_pack() {
    let item_path = "assets/spt/items/pan.json".to_owned();
    let lower = Resource::bytes(
        br#"{"model":{"type":"special","base":"spt:item/base","model":{"type":"spt:pan","handle":2}}}"#.to_vec(),
    );
    let winner = Resource::bytes(
        br#"{"model":{"type":"special","base":"spt:item/base","model":{"type":"spt:pan","handle":null}}}"#.to_vec(),
    );
    let files = BTreeMap::from([
        (item_path.clone(), winner.clone()),
        ("assets/spt/items/other.json".to_owned(), lower.clone()),
        ("assets/spt/items/decode-panic.json".to_owned(), Resource::bytes(
            br#"{"model":{"type":"special","base":"spt:item/base","model":{"type":"spt:decode-panic"}}}"#.to_vec(),
        )),
        ("assets/spt/items/bake-panic.json".to_owned(), Resource::bytes(
            br#"{"model":{"type":"special","base":"spt:item/base","model":{"type":"spt:bake-panic"}}}"#.to_vec(),
        )),
    ]);
    let pack = ResourceSource::indexed_with_layers(
        files,
        BTreeMap::from([(item_path, vec![lower, winner])]),
    );
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path();
    write(directory, "items.json", "{}");
    let item_data = crate::test_resources::item_data(
        directory,
        sunrise_world::native_load_context::DataLoadOrigin::ClientPreload,
    )
    .unwrap();
    let root = ResourceRoot::new(directory.to_owned(), directory.to_owned(), pack);
    let mut factories = pan_factories(|_| {});
    factories.extend([
        (
            "spt:decode-panic".into(),
            (
                SpecialModelPolicy::Stack,
                SpecialModelFactory::new(
                    |_| -> Result<Pan, String> { panic!("decoder panicked") },
                    |pan, _| Ok(pan),
                ),
            ),
        ),
        (
            "spt:bake-panic".into(),
            (
                SpecialModelPolicy::Stack,
                SpecialModelFactory::new(
                    |_| Ok(()),
                    |(), _| -> Result<Pan, String> { panic!("baker panicked") },
                ),
            ),
        ),
    ]);
    let renderer = ItemRenderer::load_tables(
        &root,
        crate::model_library::ParsedModels::load(root.source()).into_item_library(),
        true,
        &item_data,
        None,
        Some(&factories),
    )
    .unwrap();
    let definitions = &renderer.models.definitions;
    assert_eq!(
        definitions.keys().map(String::as_str).collect::<Vec<_>>(),
        ["spt:other"]
    );
    assert!(matches!(
        &definitions["spt:other"].model,
        sunrise_assets::item_model::ItemModelNode::RegisteredSpecial { .. },
    ));
}

#[test]
fn a_generation_bakes_typed_config_once_and_draws_use_the_baked_instance() {
    let temp = tempfile::tempdir().unwrap();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let factories = pan_factories({
        let calls = Arc::clone(&calls);
        move |handle| calls.lock().unwrap().push(handle)
    });
    let draw = |renderer: &ItemRenderer| {
        let env = PanEnv(renderer, FakeHost::new());
        let state = renderer.resolve(
            &stack("spt:pan", 1),
            DisplayContext::None,
            None,
            None,
            0,
            &env,
        );
        assert_eq!(quads(renderer, &state).len(), 6);
        let (lo, hi) = state.bounding_box();
        (lo.x, hi.x)
    };
    let first = renderer(temp.path(), 2, 2, &factories);
    assert_eq!(*calls.lock().unwrap(), [2]);
    assert_eq!(draw(&first), (-0.25, 0.0));
    assert_eq!(draw(&first), (-0.25, 0.0));
    assert_eq!(*calls.lock().unwrap(), [2]);

    let second = renderer(temp.path(), 2, 4, &factories);
    assert_eq!(*calls.lock().unwrap(), [2, 4]);
    assert_eq!(draw(&second), (0.0, 0.25));
    assert_eq!(draw(&first), (-0.25, 0.0));
    assert_eq!(draw(&second), (0.0, 0.25));
    assert_eq!(*calls.lock().unwrap(), [2, 4]);
}

#[test]
fn special_geometry_leases_resolve_pack_sprites_and_reject_invalid_quads() {
    let temp = tempfile::tempdir().unwrap();
    let leases = Arc::new(Mutex::new(None));
    let bake = {
        let leases = Arc::clone(&leases);
        move |pan, context: &dyn mod_api::SpecialModelBakeContext| {
            let positions = [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ];
            let mut quad = SpecialModelQuad::new(
                positions,
                [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                "spt:item/grey",
            );
            quad.tint_index = Some(2);
            quad.light_emission = 12;
            quad.force_translucent = true;
            let resource = context.bake_model("spt:item/b")?;
            let custom = context.bake_geometry(&[quad.clone()])?;
            let invalid: [(&str, fn(&mut SpecialModelQuad)); 6] = [
                ("non-finite", |q| q.positions[0][0] = f32::NAN),
                ("facing", |q| q.positions = [[0.0; 3]; 4]),
                ("UV", |q| q.uv[0][0] = -1.0),
                ("light", |q| q.light_emission = 16),
                ("tint", |q| q.tint_index = Some(u32::MAX)),
                ("missing", |q| q.sprite = "spt:item/not-present".into()),
            ];
            for (message, invalidate) in invalid {
                let mut invalid_quad = quad.clone();
                invalidate(&mut invalid_quad);
                match context.bake_geometry(&[invalid_quad]) {
                    Err(error) if error.contains(message) => {}
                    _ => return Err(format!("did not reject invalid {message}")),
                }
            }
            *leases.lock().unwrap() = Some((resource, custom));
            Ok(pan)
        }
    };
    let factories = BTreeMap::from([(
        "spt:pan".into(),
        (
            SpecialModelPolicy::Stack,
            SpecialModelFactory::new(|config| config.decode::<Pan>(), bake),
        ),
    )]);
    let renderer = renderer(temp.path(), 2, 2, &factories);
    assert!(renderer.models.definitions.contains_key("spt:pan"));
    let (resource, custom) = leases.lock().unwrap().take().unwrap();
    let resource = resource.downcast_ref::<BakedSpecialGeometry>().unwrap();
    assert_eq!(resource.model.quads.len(), 12);
    let custom = custom.downcast_ref::<BakedSpecialGeometry>().unwrap();
    assert_eq!(
        custom.extents.as_slice(),
        [Vec3::ZERO, Vec3::X, Vec3::new(1.0, 1.0, 0.0), Vec3::Y]
    );
    let [quad] = custom.model.quads.as_slice() else {
        panic!("expected one custom quad")
    };
    assert_eq!(quad.facing, Direction::South);
    assert_eq!(quad.material.sprite, "spt:item/grey");
    assert!(quad.material.force_translucent);
    assert_eq!(quad.tint_index, 2);
    assert_eq!(quad.light_emission, 12);
}

#[test]
#[ignore = "daily"]
fn a_registered_special_model_draws_its_renderers_resource_models() {
    let root = std::env::temp_dir().join(format!("sunrise-special-parts-{}", std::process::id()));
    write(
        &root,
        "assets/spt/items/bundle.json",
        r#"{"model":{"type":"bundle/selected_item"}}"#,
    );
    let factories = pan_factories(|_| {});
    let renderer = renderer(&root, 2, 2, &factories);
    let env = PanEnv(&renderer, FakeHost::new());
    let resolve = |item: &str, count, display| {
        renderer.resolve(&stack(item, count), display, None, None, 0, &env)
    };

    // A or B, in the inventory and in hand, tinted and with the base's particle and GUI light.
    for display in [DisplayContext::Gui, DisplayContext::FirstPersonRightHand] {
        let one = resolve("spt:pan", 1, display);
        let two = resolve("spt:pan", 2, display);
        assert_eq!(quads(&renderer, &one).len(), 6, "{display:?}");
        assert_eq!(quads(&renderer, &two).len(), 12, "{display:?}");
        assert!(
            quads(&renderer, &two)
                .iter()
                .all(|q| q.color == 0xFFCC_2211)
        );
        assert_eq!(two.layers[0].particle.as_ref(), "spt:item/flour");
        assert!(!two.uses_block_light());
        assert!(
            two.animated,
            "a mod's drawing is never kept as a static GUI item"
        );
    }

    // B on the ground, in a frame, on a head and in another player's hand: the base's
    // transform for each context, so each draws B somewhere else.
    let contexts = [
        DisplayContext::Ground,
        DisplayContext::Fixed,
        DisplayContext::Head,
        DisplayContext::ThirdPersonRightHand,
    ];
    let matrices: Vec<Mat4> = contexts
        .iter()
        .map(|&display| {
            let state = resolve("spt:pan", 2, display);
            assert_eq!(quads(&renderer, &state).len(), 12, "{display:?}");
            state.layers[0].matrix
        })
        .collect();
    for (i, a) in matrices.iter().enumerate() {
        for b in &matrices[i + 1..] {
            assert_ne!(a, b);
        }
    }
    // A part's pose is about the model's corner, as its elements are: A's cube spans x 0 to 1/4
    // there, moved a quarter block or doubled; the drawn item then sits half a block back.
    let x = |count| {
        let (lo, hi) = resolve("spt:pan", count, DisplayContext::None).bounding_box();
        (lo.x, hi.x)
    };
    assert_eq!(x(1), (-0.25, 0.0));
    assert_eq!(x(5), (-0.5, 0.0));

    // A part naming a model no pack has, or a refused draw, draws the missing model; a plain
    // cube item still draws its 6 quads.
    let missing = renderer.bake(MISSING_MODEL);
    let is_missing = |state: &ItemRenderState| {
        matches!(&state.layers[..], [layer]
            if matches!(&layer.geometry, LayerGeometry::Quads(baked) if Arc::ptr_eq(baked, &missing)))
    };
    assert!(is_missing(&resolve("spt:pan", 3, DisplayContext::Gui)));
    assert!(is_missing(&resolve("spt:pan", 4, DisplayContext::Gui)));
    // A stack part draws the inner item through the resolver, under its own display context and
    // tints, not the part's: the cube's 6 white quads beside B's 12 tinted ones. A stack part
    // that names the stack being drawn is a cycle, refused whole.
    let nested = resolve("spt:pan", 8, DisplayContext::Fixed);
    let colours: Vec<u32> = quads(&renderer, &nested).iter().map(|q| q.color).collect();
    assert_eq!(colours.iter().filter(|&&c| c == 0xFFFF_FFFF).count(), 6);
    assert_eq!(colours.iter().filter(|&&c| c == 0xFFCC_2211).count(), 12);
    assert!(nested.animated);
    assert!(is_missing(&resolve("spt:pan", 9, DisplayContext::Gui)));
    // A refusal below a first level refuses the answers above it whole, their valid sibling
    // models too; the expansion budget is 64 for the whole draw, below the first level as well.
    assert!(is_missing(&resolve("spt:pan", 10, DisplayContext::Gui)));
    assert!(is_missing(&resolve("spt:pan", 11, DisplayContext::Gui)));
    let quads_of = |count| quads(&renderer, &resolve("spt:pan", count, DisplayContext::Gui)).len();
    assert_eq!(quads_of(20), 64 * 6);
    assert!(is_missing(&resolve("spt:pan", 21, DisplayContext::Gui)));
    assert!(is_missing(&resolve("spt:pan", 22, DisplayContext::Gui)));
    assert_eq!(quads_of(23), 12 + 63 * 6);
    assert!(!is_missing(&resolve("spt:pan", 1, DisplayContext::Gui)));
    assert_eq!(
        quads(&renderer, &resolve("spt:cube", 1, DisplayContext::Gui)).len(),
        6
    );

    // A selected stack of two pans draws B with its own foil; the single bundle would draw A.
    let mut bundle = stack("spt:bundle", 1);
    bundle.bundle_selected = 0;
    bundle.patch.insert(
        "minecraft:bundle_contents".into(),
        Some(serde_json::json!([{
            "id": "spt:pan", "count": 2, "components": {
                "minecraft:item_model": "spt:pan",
                "minecraft:enchantment_glint_override": true
            }
        }])),
    );
    let picked = renderer.resolve(&bundle, DisplayContext::Gui, None, None, 0, &env);
    let quads_picked = quads(&renderer, &picked);
    assert_eq!(quads_picked.len(), 12);
    assert!(quads_picked.iter().all(|quad| quad.foil == Foil::Standard));

    // A special type no mod registered isn't loaded: its item draws the missing model.
    assert!(is_missing(&resolve("spt:stranger", 1, DisplayContext::Gui)));

    // A reload that makes B three cubes draws 18 quads for the same stack.
    let reloaded = super::special_parts_tests::renderer(&root, 3, 2, &factories);
    let state = reloaded.resolve(
        &stack("spt:pan", 2),
        DisplayContext::Gui,
        None,
        None,
        0,
        &PanEnv(&reloaded, FakeHost::new()),
    );
    assert_eq!(quads(&reloaded, &state).len(), 18);
    fs::remove_dir_all(root).unwrap();
}

/// Row 5 of the matrix (`docs/design/mod-api/resource-renderers.md`), an old state across a
/// reload. A part's quads are CPU vertices submitted every frame, not prepared GPU models, so the
/// state's own `Arc<BakedItemModel>` is the only lease and there is no retirement step: a state
/// resolved before a reload still draws its 12 quads after it, the old baked model is released
/// exactly once, when the last state holding it drops, and a new resolve draws the reload's 18.
#[test]
#[ignore = "daily"]
fn an_old_state_draws_across_a_reload_and_releases_its_models_once() {
    let root = std::env::temp_dir().join(format!(
        "sunrise-special-parts-lease-{}",
        std::process::id()
    ));
    let factories = pan_factories(|_| {});
    let before = renderer(&root, 2, 2, &factories);
    let resolve = |renderer: &ItemRenderer| {
        renderer.resolve(
            &stack("spt:pan", 2),
            DisplayContext::Gui,
            None,
            None,
            0,
            &PanEnv(renderer, FakeHost::new()),
        )
    };
    let (old, also_old) = (resolve(&before), resolve(&before));
    let LayerGeometry::Quads(b) = &old.layers[0].geometry else {
        panic!("not quads: {:?}", old.layers[0].geometry);
    };
    let b = Arc::downgrade(b);
    let after = renderer(&root, 3, 2, &factories);
    drop(before);
    assert_eq!(quads(&after, &old).len(), 12);
    assert_eq!(quads(&after, &resolve(&after)).len(), 18);
    assert_eq!(b.strong_count(), 2, "the two old states hold the old B");
    drop(old);
    assert_eq!(b.strong_count(), 1);
    assert_eq!(quads(&after, &also_old).len(), 12);
    drop(also_old);
    assert_eq!(b.strong_count(), 0, "the old B outlived its last state");
    fs::remove_dir_all(root).unwrap();
}

/// Row 11 of the matrix, a part's tint. It is packed vanilla-space ARGB, as an item model's
/// tints are: white and an encoded 0.5 reach the vertices as the colour vanilla's constant tint
/// source gives the same cube, not decoded, so the frame decodes them once (vanilla's power 2.2:
/// shader values 1 and about 0.21763764, pinned by `gfx::linear_blend_tests`). A value above 1 has
/// no input.
#[test]
#[ignore = "daily"]
fn a_parts_tint_reaches_the_vertices_as_a_vanilla_tint_does() {
    let root =
        std::env::temp_dir().join(format!("sunrise-special-parts-tint-{}", std::process::id()));
    let factories = pan_factories(|_| {});
    let renderer = renderer(&root, 2, 2, &factories);
    let env = PanEnv(&renderer, FakeHost::new());
    let colours = |item: &str, count| {
        let state = renderer.resolve(
            &stack(item, count),
            DisplayContext::Gui,
            None,
            None,
            0,
            &env,
        );
        quads(&renderer, &state)
            .iter()
            .map(|q| q.color)
            .collect::<Vec<_>>()
    };
    for (count, vanilla, argb) in [(6, "spt:white_cube", WHITE), (7, "spt:grey_cube", GREY)] {
        let part = colours("spt:pan", count);
        assert_eq!(part, colours(vanilla, 1), "{vanilla}");
        assert_eq!(part, vec![argb as u32; 6], "{vanilla}");
    }
    fs::remove_dir_all(root).unwrap();
}
