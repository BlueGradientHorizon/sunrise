use super::*;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn png(path: &Path, w: u32, h: u32, color: [u8; 4]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut encoder = png::Encoder::new(fs::File::create(path).unwrap(), w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer
        .write_image_data(&color.repeat((w * h) as usize))
        .unwrap();
    writer.finish().unwrap();
}

fn fixture(name: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("sunrise-glint-5851-{name}-{}", std::process::id()));
    let write = |name: &str, data: &[u8]| {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    };
    write("items.json", b"{}");
    write(
        "assets/minecraft/atlases/items.json",
        br#"{"sources":[
        {"type":"directory","source":"item","prefix":"item/"},
        {"type":"single","resource":"test:source","sprite":"test:item/enchanted_book"},
        {"type":"unstitch","resource":"test:sheet","divisor_x":2,"divisor_y":1,
         "regions":[{"sprite":"minecraft:item/compass_00","x":1,"y":0,"width":1,"height":1}]}]}"#,
    );
    for (name, w, h) in [
        ("minecraft/item/enchanted_book", 16, 16),
        ("minecraft/item/book", 8, 16),
        ("minecraft/item/diamond_sword", 32, 16),
        ("minecraft/item/clock_00", 16, 64),
        ("test/source", 16, 16),
        ("test/sheet", 16, 16),
    ] {
        let (namespace, path) = name.split_once('/').unwrap();
        png(
            &root.join(format!("assets/{namespace}/textures/{path}.png")),
            w,
            h,
            [255; 4],
        );
    }
    write(
        "assets/minecraft/textures/item/clock_00.png.mcmeta",
        br#"{"animation":{"width":16,"height":16}}"#,
    );
    root
}

fn state(sprite: &str, foil: FoilType, display: DisplayContext) -> ItemRenderState {
    let material = sunrise_assets::model::Material {
        sprite: sprite.into(),
        force_translucent: false,
    };
    let baked = BakedItemModel {
        quads: vec![sunrise_assets::bake::ModelQuad {
            positions: [
                [-0.5, 0.5, 0.0],
                [-0.5, -0.5, 0.0],
                [0.5, -0.5, 0.0],
                [0.5, 0.5, 0.0],
            ],
            uv: [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]],
            facing: Direction::South,
            cull: None,
            material: material.clone(),
            tint_index: -1,
            shade_direction_override: None,
            light_emission: 0,
            light: [0; 2],
            color: None,
            ambient_occlusion: None,
            render_type: None,
            shade: true,
        }],
        uses_block_light: false,
        particle: material,
        transforms: Default::default(),
    };
    ItemRenderState {
        display,
        layers: vec![LayerRender {
            geometry: LayerGeometry::Quads(Arc::new(baked)),
            tints: LayerTints::Owned(Vec::new()),
            foil,
            matrix: Mat4::IDENTITY,
            particle: sprite.into(),
            uses_block_light: false,
            extents: Arc::new(vec![]),
            display,
        }],
        oversized_in_gui: false,
        animated: false,
        cit_glint: None,
        hdr_tier: 0,
        hdr_light: 0,
        held_tint: None,
    }
}

#[test]
fn held_item_tint_is_per_draw_on_cpu_and_gpu() {
    use crate::{
        gfx::entity_skin::{self, SkinFrame},
        mobs::skin::{Skin, SkinModels},
    };

    let root = fixture("held-item-tint");
    let data = crate::test_resources::item_data(
        &root,
        sunrise_world::native_load_context::DataLoadOrigin::ClientPreload,
    )
    .unwrap();
    let renderer = ItemRenderer::load(
        &sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
        &data,
    )
    .unwrap();
    fs::remove_dir_all(root).unwrap();
    let sprite = "item/book";
    let sprites = HashMap::from([(
        sprite.into(),
        Rect {
            u0: 0.0,
            v0: 0.0,
            u1: 1.0,
            v1: 1.0,
            w: 8,
            h: 16,
        },
    )]);
    let mut plain = state(sprite, FoilType::None, DisplayContext::ThirdPersonRightHand);
    let LayerGeometry::Quads(baked) = &mut plain.layers[0].geometry else {
        unreachable!()
    };
    Arc::get_mut(baked).unwrap().quads[0].tint_index = 0;
    plain.layers[0].tints = LayerTints::Owned(vec![0x7F_FF_60_11]);
    let mut tinted = plain.clone();
    // The high byte is ignored. With ambient-only lighting on the south face,
    // the blue byte goes 17 -> 13 -> 5; lighting before tinting would give 4.
    tinted.set_held_tint(0x23_80_40_C8);
    let lights = Some((Vec3::NEG_Z, Vec3::NEG_Z));
    let pose = Mat4::from_translation(Vec3::Y);
    let mut cpu = Vec::new();
    let mut frame = SkinFrame::default();
    let mut models = SkinModels::default();
    for draw in [&plain, &tinted, &plain] {
        draw.submit(
            &renderer,
            &sprites,
            &NoHost,
            pose,
            &mut WorldSink {
                out: &mut cpu,
                light: 0xF0F0,
                lights,
                glint: None,
                tier: 0,
            },
        );
        assert!(draw.submit_skinned(
            &renderer,
            &sprites,
            pose,
            (0xF0F0, lights, 0),
            &mut Skin::new(&mut models, &mut frame),
        ));
    }
    let gpu = entity_skin::expand(&frame.new_model, &frame);
    assert_eq!(
        bytemuck::cast_slice::<_, u8>(&gpu),
        bytemuck::cast_slice::<_, u8>(&cpu)
    );
    assert_eq!(
        cpu.iter().map(|v| v.color).collect::<Vec<_>>(),
        [[0x7F_06_26_66; 4], [0x7F_05_09_33; 4], [0x7F_06_26_66; 4]].concat()
    );
}

#[test]
fn gui_glint_matches_pinned_vanilla_atlas_input() {
    let root = fixture("gui");
    let data = crate::test_resources::item_data(
        &root,
        sunrise_world::native_load_context::DataLoadOrigin::ClientPreload,
    )
    .unwrap();
    let renderer = ItemRenderer::load(
        &sunrise_assets::resource_source::ResourceRoot::directory(root.to_path_buf()),
        &data,
    )
    .unwrap();
    renderer.prepare_glint_atlas(16384, (0, 2)).unwrap();
    // The real vanilla oracle also checks a rectangular texture, an animation frame,
    // an unstitch source and an equal-sized sprite in a second namespace.
    for (name, x, y, w, h) in [
        ("item/enchanted_book", 1, 33, 16, 16),
        ("item/book", 53, 1, 8, 16),
        ("item/diamond_sword", 1, 1, 32, 16),
        ("item/clock_00", 35, 1, 16, 16),
        ("item/compass_00", 55, 33, 8, 16),
        ("test:item/enchanted_book", 19, 33, 16, 16),
        ("missingno", 37, 33, 16, 16),
    ] {
        let r = renderer
            .glint_atlas
            .lock()
            .unwrap()
            .get_stitched(name)
            .unwrap();
        assert_eq!(
            [r.u0, r.v0, r.u1, r.v1],
            [x, y, x + w, y + h].map(|n| n as f32 / 64.0)
        );
    }
    let sprites = HashMap::from([
        (
            "item/enchanted_book".into(),
            Rect {
                u0: 0.2,
                v0: 0.3,
                u1: 0.4,
                v1: 0.5,
                w: 16,
                h: 16,
            },
        ),
        (
            "misc/enchanted_glint_item".into(),
            Rect {
                u0: 0.0,
                v0: 0.0,
                u1: 1.0,
                v1: 1.0,
                w: 128,
                h: 128,
            },
        ),
    ]);
    let render = |foil| {
        let mut batch = crate::hud::HudBatch::scratch([64, 64]);
        gui::draw(
            &state("minecraft:item/enchanted_book", foil, DisplayContext::Gui),
            &renderer,
            &sprites,
            &NoHost,
            &mut batch,
            0.0,
            0.0,
            0,
            0.0,
            0.75,
        );
        batch
    };
    let batch = render(FoilType::Standard);
    // Real 26.3 SpriteSourceList + SpriteResourceLoader + Stitcher + JOML, over this fixture:
    // atlas 64x64, book rectangle (1,33)..(17,49). HUD packing above deliberately differs.
    let expected = [
        [-0.5931978, 4.084038],
        [-0.94049406, 6.0536532],
        [1.0291214, 6.4009495],
        [1.3764176, 4.431334],
    ];
    let mut seen = [false; 4];
    for vertex in batch.vertices.iter().filter(|v| v.uv[0] >= 2.0) {
        let index = match vertex.pos {
            [0.0, 0.0] => 0,
            [0.0, 16.0] => 1,
            [16.0, 16.0] => 2,
            [16.0, 0.0] => 3,
            p => panic!("unexpected corner {p:?}"),
        };
        for (got, want) in vertex.glint_uv.into_iter().zip(expected[index]) {
            assert!((got - want).abs() < 1e-5, "glint {got} != vanilla {want}");
        }
        seen[index] = true;
    }
    assert!(seen.into_iter().all(|x| x));
    let plain = render(FoilType::None);
    let base: Vec<_> = batch
        .vertices
        .iter()
        .filter(|v| v.uv[0] < 2.0)
        .map(|v| (v.pos, v.uv, v.color))
        .collect();
    assert_eq!(
        base,
        plain
            .vertices
            .iter()
            .map(|v| (v.pos, v.uv, v.color))
            .collect::<Vec<_>>()
    );
    let book_rect = renderer
        .glint_atlas
        .lock()
        .unwrap()
        .get_stitched("item/enchanted_book")
        .unwrap();
    let book_atlas_uv = [
        [book_rect.u0, book_rect.v0],
        [book_rect.u0, book_rect.v1],
        [book_rect.u1, book_rect.v1],
        [book_rect.u1, book_rect.v0],
    ];
    for display in [
        DisplayContext::Ground,
        DisplayContext::FirstPersonRightHand,
        DisplayContext::Fixed,
    ] {
        let mut quads = Vec::new();
        state("minecraft:item/enchanted_book", FoilType::Standard, display).submit(
            &renderer,
            &sprites,
            &NoHost,
            Mat4::IDENTITY,
            &mut quads,
        );
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].gui_glint_uv, Some(book_atlas_uv), "{display:?}");
        assert_eq!(
            quads[0].foil_uv,
            [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]]
        );
    }
    // `FoilType::Special` projects decal UV3 while keeping sprite-local `foil_uv`.
    let mut special_quads = Vec::new();
    state(
        "minecraft:item/enchanted_book",
        FoilType::Special,
        DisplayContext::Gui,
    )
    .submit(
        &renderer,
        &sprites,
        &NoHost,
        Mat4::IDENTITY,
        &mut special_quads,
    );
    assert_eq!(special_quads.len(), 1);
    assert!(special_quads[0].gui_glint_uv.is_some());
    assert_ne!(
        special_quads[0].gui_glint_uv,
        Some(special_quads[0].foil_uv)
    );
    assert_eq!(
        special_quads[0].foil_uv,
        [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]]
    );
    // Nested stacks keep cuboid atlas glint across both outer and inner display contexts.
    for (outer, inner) in [
        (DisplayContext::Fixed, DisplayContext::Gui),
        (DisplayContext::Gui, DisplayContext::Fixed),
    ] {
        let book = "minecraft:item/enchanted_book";
        let mut state = state(book, FoilType::Standard, outer);
        state
            .layers
            .extend(self::state(book, FoilType::Standard, inner).layers);
        let mut quads = Vec::new();
        state.submit(&renderer, &sprites, &NoHost, Mat4::IDENTITY, &mut quads);
        assert_eq!(quads.len(), 2);
        assert_eq!(quads[0].gui_glint_uv, Some(book_atlas_uv), "{outer:?}");
        assert_eq!(quads[1].gui_glint_uv, Some(book_atlas_uv), "{inner:?}");
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn glint_atlas_boundaries() {
    let root = fixture("boundary");
    // Pinned vanilla over the same generated fixture; padding 1,2,4,8,16.
    for (i, (width, height, x, y)) in [
        (64, 64, 1, 33),
        (128, 64, 2, 34),
        (128, 64, 4, 36),
        (128, 64, 40, 40),
        (256, 128, 16, 80),
    ]
    .into_iter()
    .enumerate()
    {
        let atlas = glint_atlas::load_atlas(
            &sunrise_assets::resource_source::ResourceSource::directory(root.clone()),
            "minecraft:items",
            0,
            i as u32,
            16384,
        )
        .unwrap();
        let r = atlas.get_stitched("item/enchanted_book").unwrap();
        assert_eq!(
            [r.u0, r.v0, r.u1, r.v1],
            [
                x as f32 / width as f32,
                y as f32 / height as f32,
                (x + 16) as f32 / width as f32,
                (y + 16) as f32 / height as f32
            ]
        );
    }
    png(
        &root.join("assets/minecraft/textures/item/wide.png"),
        16384,
        1,
        [255; 4],
    );
    let atlas = glint_atlas::load_atlas(
        &sunrise_assets::resource_source::ResourceSource::directory(root.clone()),
        "minecraft:items",
        0,
        0,
        32768,
    )
    .unwrap();
    let book = atlas.get_stitched("item/enchanted_book").unwrap();
    assert_eq!(
        [book.u0, book.v0, book.u1, book.v1],
        [1.0 / 32768.0, 33.0 / 64.0, 17.0 / 32768.0, 49.0 / 64.0]
    );
    let atlas = glint_atlas::load_atlas(
        &sunrise_assets::resource_source::ResourceSource::directory(root.clone()),
        "minecraft:items",
        0,
        0,
        16384,
    )
    .unwrap();
    let book = atlas.get_stitched("item/enchanted_book").unwrap();
    assert_eq!(
        [book.u0, book.v0, book.u1, book.v1],
        [1.0 / 64.0, 33.0 / 128.0, 17.0 / 64.0, 49.0 / 128.0]
    );
    assert!(atlas.get_stitched("item/wide").is_none());
    assert_eq!(
        atlas.get_or_missing("item/wide").unwrap().u0,
        atlas.missing().unwrap().u0
    );
    assert!(
        glint_atlas::load_atlas(
            &sunrise_assets::resource_source::ResourceSource::directory(root.clone()),
            "minecraft:items",
            0,
            0,
            16,
        )
        .is_err()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_and_generated_sprites_match_vanilla() {
    let root = fixture("metadata");
    for name in ["invalid_size", "invalid_metadata", "not_animated"] {
        png(
            &root.join(format!("assets/minecraft/textures/item/{name}.png")),
            16,
            32,
            [255; 4],
        );
    }
    for (name, meta) in [
        ("invalid_size", r#"{"animation":{"width":9,"height":16}}"#),
        ("invalid_metadata", r#"{"animation":{"frames":"bad"}}"#),
        ("not_animated", r#"{"texture":{"blur":true}}"#),
    ] {
        fs::write(
            root.join(format!("assets/minecraft/textures/item/{name}.png.mcmeta")),
            meta,
        )
        .unwrap();
    }
    let path = root.join("assets/minecraft/atlases/items.json");
    let mut atlas: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    atlas["sources"].as_array_mut().unwrap().extend([
        serde_json::json!({"type":"single","resource":"test:absent","sprite":"test:missing_alias"}),
        serde_json::json!({"type":"paletted_permutations","textures":["test:trim"],"palette_key":"test:key","permutations":{"red":"test:red"}}),
    ]);
    fs::write(path, atlas.to_string()).unwrap();
    for (name, w, h, color) in [
        ("trim", 8, 16, [255; 4]),
        ("key", 1, 1, [255; 4]),
        ("red", 1, 1, [255, 0, 0, 255]),
    ] {
        png(
            &root.join(format!("assets/test/textures/{name}.png")),
            w,
            h,
            color,
        );
    }
    let atlas = glint_atlas::load_atlas(
        &sunrise_assets::resource_source::ResourceSource::directory(root.clone()),
        "minecraft:items",
        0,
        0,
        16384,
    )
    .unwrap();
    // Real SpriteResourceLoader rejects both invalid animations; TextureAtlas resolves missingno.
    for name in ["item/invalid_size", "item/invalid_metadata"] {
        assert!(atlas.get_stitched(name).is_none());
        let r = atlas.get_or_missing(name).unwrap();
        assert_eq!(
            [r.u0, r.v0, r.u1, r.v1],
            [103.0 / 128.0, 1.0 / 64.0, 119.0 / 128.0, 17.0 / 64.0]
        );
    }
    let r = atlas.get_stitched("item/not_animated").unwrap();
    assert_eq!(
        [r.u0, r.v0, r.u1, r.v1],
        [1.0 / 128.0, 1.0 / 64.0, 17.0 / 128.0, 33.0 / 64.0]
    );
    let r = atlas.get_stitched("test:trim_red").unwrap();
    assert_eq!(
        [r.u0, r.v0, r.u1, r.v1],
        [19.0 / 128.0, 37.0 / 64.0, 27.0 / 128.0, 53.0 / 64.0]
    );
    assert!(atlas.get_stitched("test:missing_alias").is_none());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn namespaces_sharing_a_prefix_are_all_listed_in_the_glint_atlas() {
    // `a0:` sorts before `a:` as an identifier, though `assets/a/` comes first as a path.
    let root = std::env::temp_dir().join(format!("sunrise-glint-prefix-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("assets/minecraft/atlases")).unwrap();
    fs::write(
        root.join("assets/minecraft/atlases/items.json"),
        r#"{"sources":[{"type":"single","resource":"a:item/one"},{"type":"single","resource":"a0:item/two"}]}"#,
    )
    .unwrap();
    png(
        &root.join("assets/a/textures/item/one.png"),
        16,
        16,
        [255; 4],
    );
    png(
        &root.join("assets/a0/textures/item/two.png"),
        16,
        16,
        [255; 4],
    );
    let atlas = glint_atlas::load_atlas(
        &sunrise_assets::resource_source::ResourceSource::directory(root.clone()),
        "minecraft:items",
        0,
        0,
        16384,
    )
    .unwrap();
    // Each is its own 16 x 16 cell; one the listing missed would be the missing sprite's.
    let corner = |name: &str| {
        let r = atlas.get_stitched(name).unwrap();
        [r.u0, r.v0]
    };
    assert_ne!(corner("a:item/one"), corner("a0:item/two"));
    assert_ne!(corner("a:item/one"), corner("missingno"));
    assert_ne!(corner("a0:item/two"), corner("missingno"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_texture_metadata_does_not_move_book_glint() {
    let root = fixture("texture-metadata");
    let image = root.join("assets/minecraft/textures/item/large.png");
    png(&image, 512, 256, [255; 4]);
    let metadata = image.with_extension("png.mcmeta");
    for invalid in [
        r#"{"texture":[]}"#,
        r#"{"texture":null}"#,
        r#"{"texture":{"mipmap_strategy":"issue5851_invalid"}}"#,
        r#"{"texture":{"blur":"true"}}"#,
        r#"{"texture":{"clamp":1}}"#,
        r#"{"texture":{"alpha_cutoff_bias":"0.5"}}"#,
    ] {
        fs::write(&metadata, invalid).unwrap();
        let atlas = glint_atlas::load_atlas(
            &sunrise_assets::resource_source::ResourceSource::directory(root.clone()),
            "minecraft:items",
            0,
            0,
            16384,
        )
        .unwrap();
        assert!(atlas.get_stitched("item/large").is_none(), "{invalid}");
        let rejected = atlas.get_or_missing("item/large").unwrap();
        let missing = atlas.missing().unwrap();
        assert_eq!(
            [rejected.u0, rejected.v0, rejected.u1, rejected.v1],
            [missing.u0, missing.v0, missing.u1, missing.v1],
            "{invalid}"
        );
        // Existing real-vanilla fixture: rejecting the extra image leaves its book unchanged.
        let book = atlas.get_stitched("item/enchanted_book").unwrap();
        assert_eq!(
            [book.u0, book.v0, book.u1, book.v1],
            [1.0 / 64.0, 33.0 / 64.0, 17.0 / 64.0, 49.0 / 64.0],
            "{invalid}"
        );
    }
    for strategy in ["auto", "mean", "cutout", "strict_cutout", "dark_cutout"] {
        fs::write(
            &metadata,
            serde_json::json!({"texture":{"blur":true,"clamp":false,
                "mipmap_strategy":strategy,"alpha_cutoff_bias":0.5}})
            .to_string(),
        )
        .unwrap();
        let atlas = glint_atlas::load_atlas(
            &sunrise_assets::resource_source::ResourceSource::directory(root.clone()),
            "minecraft:items",
            0,
            0,
            16384,
        )
        .unwrap();
        let large = atlas.get_stitched("item/large").unwrap();
        assert_eq!((large.w, large.h), (512, 256));
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn glint_atlas_reads_prepared_images_and_metadata_together() {
    let root = fixture("prepared-metadata");
    let image = root.join("assets/minecraft/textures/item/large.png");
    png(&image, 512, 256, [255; 4]);
    let metadata = image.with_extension("png.mcmeta");
    fs::write(&metadata, r#"{"texture":[]}"#).unwrap();
    fs::write(root.join("sunrise-resources.json"), "{}").unwrap();
    sunrise_core::prepared_files::write(&root).unwrap();
    // The renderer reads the prepared snapshot, even if loose files now differ.
    fs::write(&metadata, r#"{"texture":{"blur":true}}"#).unwrap();
    png(
        &root.join("assets/minecraft/textures/item/enchanted_book.png"),
        256,
        256,
        [0, 255, 0, 255],
    );
    let atlas = glint_atlas::load_atlas(
        &sunrise_assets::resource_source::ResourceSource::directory(root.clone()),
        "minecraft:items",
        0,
        0,
        16384,
    )
    .unwrap();
    assert!(atlas.get_stitched("item/large").is_none());
    assert_eq!(
        atlas.get_or_missing("item/large").unwrap().u0,
        atlas.missing().unwrap().u0
    );
    let book = atlas.get_stitched("item/enchanted_book").unwrap();
    assert_eq!(
        [book.u0, book.v0, book.u1, book.v1],
        [1.0 / 64.0, 33.0 / 64.0, 17.0 / 64.0, 49.0 / 64.0]
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cuboid_glint_prefers_stitched_blocks_and_blocks_missingno() {
    let root = fixture("material-baker-fallback");
    // Extra large sprite in `blocks` makes its `missingno` rectangle distinct from `items`.
    fs::write(
        root.join("assets/minecraft/atlases/items.json"),
        r#"{"sources":[
            {"type":"single","resource":"test:bad_item","sprite":"test:shared"},
            {"type":"single","resource":"minecraft:item/enchanted_book"}
        ]}"#,
    )
    .unwrap();
    fs::write(
        root.join("assets/minecraft/atlases/blocks.json"),
        r#"{"sources":[
            {"type":"single","resource":"test:good_block","sprite":"test:shared"},
            {"type":"single","resource":"test:big_block"}
        ]}"#,
    )
    .unwrap();
    // Corrupt PNG in `items` fails to decode; valid PNG in `blocks` stitches for the same ID.
    fs::create_dir_all(root.join("assets/test/textures")).unwrap();
    fs::write(root.join("assets/test/textures/bad_item.png"), b"not a png").unwrap();
    png(
        &root.join("assets/test/textures/good_block.png"),
        16,
        16,
        [255; 4],
    );
    png(
        &root.join("assets/test/textures/big_block.png"),
        128,
        128,
        [255; 4],
    );
    let data = crate::test_resources::item_data(
        &root,
        sunrise_world::native_load_context::DataLoadOrigin::ClientPreload,
    )
    .unwrap();
    let renderer = ItemRenderer::load(
        &sunrise_assets::resource_source::ResourceRoot::directory(root.to_path_buf()),
        &data,
    )
    .unwrap();
    renderer
        .prepare_glint_atlas_with_mips(16384, (0, 2), 0)
        .unwrap();

    let items_missing = renderer.glint_atlas.lock().unwrap().missing().unwrap();
    let blocks_missing = renderer
        .block_glint_atlas
        .lock()
        .unwrap()
        .missing()
        .unwrap();
    let blocks_shared = renderer
        .block_glint_atlas
        .lock()
        .unwrap()
        .get_stitched("test:shared")
        .unwrap();
    assert!(
        renderer
            .glint_atlas
            .lock()
            .unwrap()
            .get_stitched("test:shared")
            .is_none()
    );
    assert_eq!(
        renderer
            .glint_atlas
            .lock()
            .unwrap()
            .get_or_missing("test:shared")
            .unwrap(),
        items_missing
    );
    assert_ne!(
        [
            items_missing.u0,
            items_missing.v0,
            items_missing.u1,
            items_missing.v1
        ],
        [
            blocks_missing.u0,
            blocks_missing.v0,
            blocks_missing.u1,
            blocks_missing.v1
        ]
    );

    let dummy = Rect {
        u0: 0.0,
        v0: 0.0,
        u1: 1.0,
        v1: 1.0,
        w: 16,
        h: 16,
    };
    let sprites = HashMap::from([("test:shared".into(), dummy), ("missingno".into(), dummy)]);
    let rect_uv = |r: Rect| [[r.u0, r.v0], [r.u0, r.v1], [r.u1, r.v1], [r.u1, r.v0]];

    // Failed items source + stitched blocks source resolves to blocks' stitched rectangle.
    let mut shared_quads = Vec::new();
    state("test:shared", FoilType::Standard, DisplayContext::Gui).submit(
        &renderer,
        &sprites,
        &NoHost,
        Mat4::IDENTITY,
        &mut shared_quads,
    );
    assert_eq!(shared_quads.len(), 1);
    assert_eq!(shared_quads[0].gui_glint_uv, Some(rect_uv(blocks_shared)));

    // Explicit `minecraft:missingno` resolves to blocks' missingno, never items' missingno.
    let mut missing_quads = Vec::new();
    state(
        "minecraft:missingno",
        FoilType::Standard,
        DisplayContext::Gui,
    )
    .submit(
        &renderer,
        &sprites,
        &NoHost,
        Mat4::IDENTITY,
        &mut missing_quads,
    );
    assert_eq!(missing_quads.len(), 1);
    assert_eq!(missing_quads[0].gui_glint_uv, Some(rect_uv(blocks_missing)));
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "daily"]
fn prepared_item_atlas_matches_pinned_vanilla() {
    let root =
        PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").expect("prepared resources"));
    let atlas = glint_atlas::load_atlas(
        &sunrise_assets::resource_source::ResourceSource::directory(root.clone()),
        "minecraft:items",
        0,
        0,
        16384,
    )
    .unwrap();
    assert_eq!(atlas.len(), 896);
    let book = atlas.get_stitched("item/enchanted_book").unwrap();
    assert_eq!(
        [book.u0, book.v0, book.u1, book.v1],
        [437.0 / 1024.0, 163.0 / 512.0, 453.0 / 1024.0, 179.0 / 512.0]
    );
    let glint = Glint {
        millis: 0,
        speed: 0.0,
        alpha: 0.75,
        scale: 8.0,
    };
    let uv = glint.uv([book.u0, book.v0]);
    for (got, want) in uv.into_iter().zip([2.919935, 3.101028]) {
        assert!((got - want).abs() < 1e-5);
    }
}

#[test]
#[ignore = "requires an independently generated pinned vanilla atlas JSON and matching resources"]
fn external_item_atlas_oracle() {
    let root = PathBuf::from(std::env::var_os("SUNRISE_GLINT_ROOT").expect("resource root"));
    let data = crate::test_resources::item_data(
        &root,
        sunrise_world::native_load_context::DataLoadOrigin::ClientPreload,
    )
    .unwrap();
    let expected: Value = serde_json::from_str(
        &fs::read_to_string(
            std::env::var_os("SUNRISE_GLINT_EXPECTED").expect("vanilla atlas JSON"),
        )
        .unwrap(),
    )
    .unwrap();
    let bit = std::env::var("SUNRISE_GLINT_ANISO_BITS")
        .unwrap()
        .parse()
        .unwrap();
    let sprites = glint_atlas::load_atlas(
        &sunrise_assets::resource_source::ResourceSource::directory(root.clone()),
        "minecraft:items",
        0,
        glint_padding(if bit == 0 { 0 } else { 2 }, bit) as u32,
        32768,
    )
    .unwrap();
    for (name, value) in expected["sprites"].as_object().unwrap() {
        let actual = sprites.get_stitched(sprite_name(name)).unwrap();
        for (got, key) in [actual.u0, actual.v0, actual.u1, actual.v1]
            .into_iter()
            .zip(["u0", "v0", "u1", "v1"])
        {
            let want = value[key].as_f64().unwrap() as f32;
            assert!(
                (got - want).abs() < 1e-7,
                "{name} {key}: {got} != vanilla {want}"
            );
        }
    }
    if let Some(rejected) = expected["rejected"].as_array() {
        let missing = sprites.missing().unwrap();
        for name in rejected {
            let key = sprite_name(name.as_str().unwrap());
            assert!(sprites.get_stitched(key).is_none(), "{key}");
            let actual = sprites.get_or_missing(key).unwrap();
            assert_eq!(
                [actual.u0, actual.v0, actual.u1, actual.v1],
                [missing.u0, missing.v0, missing.u1, missing.v1]
            );
        }
        let renderer = ItemRenderer::load(
            &sunrise_assets::resource_source::ResourceRoot::directory(root.to_path_buf()),
            &data,
        )
        .unwrap();
        renderer
            .prepare_glint_atlas(32768, (if bit == 0 { 0 } else { 2 }, bit))
            .unwrap();
        let hud_sprites = HashMap::from([
            ("item/enchanted_book".into(), missing),
            ("misc/enchanted_glint_item".into(), missing),
        ]);
        let mut batch = crate::hud::HudBatch::scratch([64, 64]);
        gui::draw(
            &state(
                "minecraft:item/enchanted_book",
                FoilType::Standard,
                DisplayContext::Gui,
            ),
            &renderer,
            &hud_sprites,
            &NoHost,
            &mut batch,
            0.0,
            0.0,
            0,
            0.0,
            0.75,
        );
        let want = &expected["sprites"]["minecraft:item/enchanted_book"]["glint_uv_clock0"];
        let mut seen = [false; 4];
        for vertex in batch.vertices.iter().filter(|v| v.uv[0] >= 2.0) {
            let index = match vertex.pos {
                [0.0, 0.0] => 0,
                [0.0, 16.0] => 3,
                [16.0, 16.0] => 2,
                [16.0, 0.0] => 1,
                p => panic!("unexpected corner {p:?}"),
            };
            for (got, value) in vertex
                .glint_uv
                .into_iter()
                .zip(want[index].as_array().unwrap())
            {
                assert!((got - value.as_f64().unwrap() as f32).abs() < 1e-5);
            }
            seen[index] = true;
        }
        assert!(seen.into_iter().all(|v| v));
    }
}

/// A posed item has a depth of its own, as a GUI model does: its vertices carry a depth from the
/// turn (nearest 0, farthest 1) and one run takes the depth test within the item's own bounds.
#[test]
fn a_posed_item_has_a_depth_of_its_own() {
    let root = fixture("posed");
    let data = crate::test_resources::item_data(
        &root,
        sunrise_world::native_load_context::DataLoadOrigin::ClientPreload,
    )
    .unwrap();
    let renderer = ItemRenderer::load(
        &sunrise_assets::resource_source::ResourceRoot::directory(root.to_path_buf()),
        &data,
    )
    .unwrap();
    let sprite = Rect {
        u0: 0.2,
        v0: 0.3,
        u1: 0.4,
        v1: 0.5,
        w: 16,
        h: 16,
    };
    let sprites = HashMap::from([("item/enchanted_book".to_owned(), sprite)]);
    let mut batch = crate::hud::HudBatch::scratch([64, 64]);
    // A flat face about its centre (40.75, 30.5), 16 pixels a block, turned 60 degrees about y:
    // its corners land at x 40.75 +- 4 (16 * 0.5 * cos 60) and y 30.5 +- 8, one side nearer.
    let pose = Mat4::from_translation(Vec3::new(40.75, 30.5, 0.0))
        * Mat4::from_scale(Vec3::new(16.0, -16.0, 16.0))
        * Mat4::from_rotation_y(60f32.to_radians());
    assert!(gui::draw_posed(
        &state(
            "minecraft:item/enchanted_book",
            FoilType::None,
            DisplayContext::Gui
        ),
        &gui::GuiItemEnv {
            renderer: &renderer,
            sprites: &sprites,
            host: &NoHost,
        },
        &mut batch,
        pose,
        GlintFrame::default(),
    ));
    assert_eq!(batch.vertices.len(), 6);
    let depths: Vec<f32> = batch.vertices.iter().map(|v| v.depth).collect();
    assert_eq!(depths.iter().copied().fold(f32::INFINITY, f32::min), 0.0);
    assert_eq!(
        depths.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        1.0
    );
    // The scissor takes every pixel the bounds [36.75, 44.75] x [22.5, 38.5] touch, including
    // the column centred at x = 44.5: rounding the edges out gives a width of 9 and a height of 17.
    assert_eq!(batch.scale, 1.0);
    assert_eq!(
        batch.blends,
        [(0..6, crate::hud::Blend::Depth([36, 22, 9, 17]))]
    );
    fs::remove_dir_all(root).ok();
}

fn glint_witness_26_3() -> Value {
    let path = std::env::var("SUNRISE_ITEM_GLINT_WITNESS")
        .expect("SUNRISE_ITEM_GLINT_WITNESS must be set (run via tools/test_glint_parity.py)");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read item glint witness {path}: {e}"));
    serde_json::from_str(&text).expect("valid 26.3 item_glint witness")
}

fn prepared_resources_26_3() -> PathBuf {
    std::env::var_os("SUNRISE_TEST_RESOURCES")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.local/sunrise-client/26.3")
        })
}

fn full_renderer_26_3() -> ItemRenderer {
    let root = prepared_resources_26_3();
    let data = crate::test_resources::item_data(
        &root,
        sunrise_world::native_load_context::DataLoadOrigin::ClientPreload,
    )
    .expect("26.3 prepared item data");
    let renderer = ItemRenderer::load(&ResourceRoot::directory(root), &data).unwrap();
    renderer.prepare_glint_atlas(16384, (0, 2)).unwrap();
    renderer
}

fn dummy_hud_sprites(names: &[&str]) -> HashMap<String, Rect> {
    // Deliberately non-vanilla rects so tests fail if HUD UVs leak into glint UVs.
    let rect = Rect {
        u0: 0.1,
        v0: 0.2,
        u1: 0.3,
        v1: 0.4,
        w: 16,
        h: 16,
    };
    let mut map = HashMap::from([("misc/enchanted_glint_item".to_owned(), rect)]);
    for &name in names {
        map.insert(name.to_owned(), rect);
    }
    map
}

fn parse_quad_uv_bits(v: &Value) -> [[u32; 2]; 4] {
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 4);
    std::array::from_fn(|i| {
        let pair = arr[i].as_array().unwrap();
        [
            pair[0].as_u64().unwrap() as u32,
            pair[1].as_u64().unwrap() as u32,
        ]
    })
}

type PosBits = [[u32; 3]; 4];
type UvBits = [[u32; 2]; 4];
type GeomKey = (PosBits, UvBits);

fn parse_quad_pos_bits(v: &Value) -> PosBits {
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 4);
    std::array::from_fn(|i| {
        let p = arr[i].as_array().unwrap();
        [
            p[0].as_u64().unwrap() as u32,
            p[1].as_u64().unwrap() as u32,
            p[2].as_u64().unwrap() as u32,
        ]
    })
}

fn vertex_quad_glint_bits(verts: &[Vertex]) -> Vec<UvBits> {
    assert_eq!(verts.len() % 4, 0);
    verts
        .as_chunks::<4>()
        .0
        .iter()
        .map(|quad| std::array::from_fn(|i| quad[i].glint_uv.map(f32::to_bits)))
        .collect()
}

struct SubmittedQuad {
    pos: PosBits,
    uv0: UvBits,
    uv3: Option<UvBits>,
    glint_uv: UvBits,
}

struct PairedWorldSink<'a> {
    world: WorldSink<'a>,
    quads: Vec<ItemQuad>,
}

impl ItemSink for PairedWorldSink<'_> {
    fn quad(&mut self, q: ItemQuad) {
        self.quads.push(q);
        self.world.quad(q);
    }
}

fn submitted_quads(
    quads: &[ItemQuad],
    verts: &[Vertex],
    special_atlas_rect: Option<Rect>,
) -> Vec<SubmittedQuad> {
    assert_eq!(verts.len(), quads.len() * 4);
    quads
        .iter()
        .zip(verts.as_chunks::<4>().0)
        .map(|(q, vquad)| {
            let pos: PosBits = std::array::from_fn(|i| vquad[i].pos.map(f32::to_bits));
            for i in 0..4 {
                assert_eq!(q.pos[i].to_array().map(f32::to_bits), pos[i]);
            }
            let glint_uv: UvBits = std::array::from_fn(|i| vquad[i].glint_uv.map(f32::to_bits));
            let (uv0, uv3) = match special_atlas_rect {
                Some(r) => (
                    q.foil_uv.map(|[u, v]| {
                        [r.u0 + (r.u1 - r.u0) * u, r.v0 + (r.v1 - r.v0) * v].map(f32::to_bits)
                    }),
                    Some(q.gui_glint_uv.unwrap().map(|p| p.map(f32::to_bits))),
                ),
                None => (q.gui_glint_uv.unwrap().map(|p| p.map(f32::to_bits)), None),
            };
            SubmittedQuad {
                pos,
                uv0,
                uv3,
                glint_uv,
            }
        })
        .collect()
}

/// Compare extruded `item/generated` quads: front/back faces `[0..2]` match by index, while side
/// faces `[2..]` match 1-to-1 by `(pos_bits, uv0_bits)` (vanilla's `HashSet<SideFace>` order
/// varies per launch through `SideDirection`'s `Enum` identity hash), asserting every field exact.
fn assert_extruded_quads_match(
    got: &[SubmittedQuad],
    expected_quads: &[Value],
    glint_key: &str,
    label: &str,
) {
    assert_eq!(got.len(), expected_quads.len(), "{label} quad count");
    for i in 0..2 {
        let w = &expected_quads[i];
        assert_eq!(
            got[i].pos,
            parse_quad_pos_bits(&w["pos_bits"]),
            "{label} quad[{i}] pos"
        );
        assert_eq!(
            got[i].uv0,
            parse_quad_uv_bits(&w["uv0_bits"]),
            "{label} quad[{i}] uv0"
        );
        assert_eq!(
            got[i].uv3,
            w.get("uv3_bits").map(parse_quad_uv_bits),
            "{label} quad[{i}] uv3"
        );
        assert_eq!(
            got[i].glint_uv,
            parse_quad_uv_bits(&w[glint_key]),
            "{label} quad[{i}] {glint_key}"
        );
    }
    let mut witness_sides: HashMap<GeomKey, &Value> =
        HashMap::with_capacity(expected_quads.len() - 2);
    for w in &expected_quads[2..] {
        let key = (
            parse_quad_pos_bits(&w["pos_bits"]),
            parse_quad_uv_bits(&w["uv0_bits"]),
        );
        assert!(
            witness_sides.insert(key, w).is_none(),
            "{label} duplicate witness side-face key {key:#x?}"
        );
    }
    let mut seen: std::collections::HashSet<GeomKey> =
        std::collections::HashSet::with_capacity(got.len() - 2);
    for q in &got[2..] {
        let key = (q.pos, q.uv0);
        assert!(
            seen.insert(key),
            "{label} duplicate Sunrise side-face key {key:#x?}"
        );
        let w = witness_sides
            .remove(&key)
            .unwrap_or_else(|| panic!("{label} unmatched Sunrise side-face key {key:#x?}"));
        assert_eq!(
            q.pos,
            parse_quad_pos_bits(&w["pos_bits"]),
            "{label} side pos"
        );
        assert_eq!(
            q.uv0,
            parse_quad_uv_bits(&w["uv0_bits"]),
            "{label} side uv0"
        );
        assert_eq!(
            q.uv3,
            w.get("uv3_bits").map(parse_quad_uv_bits),
            "{label} side uv3"
        );
        assert_eq!(
            q.glint_uv,
            parse_quad_uv_bits(&w[glint_key]),
            "{label} side {glint_key}"
        );
    }
    assert!(
        witness_sides.is_empty(),
        "{label} {} unmatched witness side faces",
        witness_sides.len()
    );
}

/// Exact `DisplayRenderer.ItemDisplayRenderer` pose for genpvp EID 83245 (`CENTER` billboard,
/// camera at zero rotation, translation `(-0.945, 0.3885, -4.2)`, `left_rotation = (0, 1, 0, 0)`,
/// `scale = 0.525`, then `ItemDisplayRenderer`'s `rotate(Axis.YP, PI)`).
fn eid_83245_pose() -> Mat4 {
    let mut p = crate::camera::joml::Mat4::IDENTITY;
    p.rotate(crate::camera::joml::Quat::rotation_yxz(
        (-std::f64::consts::PI / 180.0) as f32 * -180.0,
        0.0,
        0.0,
    ));
    p.translate(-0.945, 0.3885, -4.2);
    p.rotate(crate::camera::joml::Quat {
        x: 0.0,
        y: 1.0,
        z: 0.0,
        w: 0.0,
    });
    p.scale(0.525, 0.525, 0.525);
    p.rotate_y(std::f32::consts::PI);
    p.to_glam()
}

/// 26.3 parity: `Glint::uv` matches `TextureTransform.setupGlintTexturing` bit-for-bit.
#[test]
#[ignore = "run by tools/test_glint_parity.py"]
fn glint_uv_matches_vanilla_26_3_texture_transform_bits() {
    let witness = glint_witness_26_3();
    let transforms = witness["transforms"].as_array().unwrap();
    assert!(!transforms.is_empty());
    for entry in transforms {
        let name = entry["name"].as_str().unwrap();
        let scale = f32::from_bits(entry["scale_bits"].as_u64().unwrap() as u32);
        let nanos = entry["nanos"].as_i64().unwrap();
        let millis = entry["millis"].as_i64().unwrap();
        assert_eq!(
            std::time::Duration::from_nanos(nanos as u64).as_millis() as i64,
            millis,
            "26.3 {name} nanos={nanos} whole-ms truncation"
        );
        let speed = f64::from_bits(
            u64::from_str_radix(entry["speed_f64_bits"].as_str().unwrap(), 16).unwrap(),
        );
        let expected_scroll = f32::from_bits(entry["scroll_ticks_bits"].as_u64().unwrap() as u32);
        assert_eq!(
            Glint::scroll_ticks(millis, speed).to_bits(),
            expected_scroll.to_bits(),
            "26.3 {name} millis={millis} speed={speed} scroll_ticks"
        );
        let glint = Glint {
            millis,
            speed,
            alpha: 0.75,
            scale,
        };
        for (idx, sample) in entry["samples"].as_array().unwrap().iter().enumerate() {
            let in_arr = sample["in_uv_bits"].as_array().unwrap();
            let in_uv = [
                f32::from_bits(in_arr[0].as_u64().unwrap() as u32),
                f32::from_bits(in_arr[1].as_u64().unwrap() as u32),
            ];
            let out_arr = sample["out_uv_bits"].as_array().unwrap();
            let want = [
                out_arr[0].as_u64().unwrap() as u32,
                out_arr[1].as_u64().unwrap() as u32,
            ];
            let got = glint.uv(in_uv).map(f32::to_bits);
            assert_eq!(
                got, want,
                "26.3 {name} millis={millis} speed={speed} sample[{idx}] in={in_uv:?}: got={got:#x?} want={want:#x?}"
            );
        }
    }
}

/// 26.3 parity: `minecraft:end_crystal` submitted into `WorldSink` (including genpvp EID 83245
/// and every `ItemDisplayContext`) uses `GLINT_TEXTURING` (8.0) over vanilla `items` atlas UVs.
#[test]
#[ignore = "run by tools/test_glint_parity.py"]
fn end_crystal_world_sink_uses_atlas_uv_and_item_glint_scale_26_3() {
    use sunrise_assets::item_model::NoEnv;

    let witness = glint_witness_26_3();
    let renderer = full_renderer_26_3();
    let sprites = dummy_hud_sprites(&["item/end_crystal"]);
    let stack = StackView::new("minecraft:end_crystal", 1);
    let eid_quads = witness["end_crystal_item_display"]["quads"]
        .as_array()
        .unwrap();
    let eid_pose = eid_83245_pose();

    let cases: [(&str, DisplayContext, Mat4); 11] = [
        ("eid_83245", DisplayContext::Gui, eid_pose),
        ("gui", DisplayContext::Gui, Mat4::IDENTITY),
        ("ground", DisplayContext::Ground, Mat4::IDENTITY),
        ("fixed", DisplayContext::Fixed, Mat4::IDENTITY),
        (
            "firstperson_righthand",
            DisplayContext::FirstPersonRightHand,
            Mat4::IDENTITY,
        ),
        (
            "firstperson_lefthand",
            DisplayContext::FirstPersonLeftHand,
            Mat4::IDENTITY,
        ),
        (
            "thirdperson_righthand",
            DisplayContext::ThirdPersonRightHand,
            Mat4::IDENTITY,
        ),
        (
            "thirdperson_lefthand",
            DisplayContext::ThirdPersonLeftHand,
            Mat4::IDENTITY,
        ),
        ("head", DisplayContext::Head, Mat4::IDENTITY),
        ("on_shelf", DisplayContext::OnShelf, Mat4::IDENTITY),
        ("none", DisplayContext::None, Mat4::IDENTITY),
    ];

    for (label, ctx, pose) in cases {
        let expected_quads = if label == "eid_83245" {
            eid_quads
        } else {
            assert_eq!(
                witness["contexts"][label]["matches_eid_83245_all_quads"].as_bool(),
                Some(true)
            );
            witness["contexts"][label]["quads"].as_array().unwrap()
        };
        let state = renderer.resolve(&stack, ctx, None, None, 83245, &NoEnv);
        for (millis, key) in [
            (0_i64, "glint_uv_t0_bits"),
            (123_456_i64, "glint_uv_t123456_bits"),
        ] {
            let mut out = Vec::new();
            let mut sink = PairedWorldSink {
                world: WorldSink {
                    out: &mut out,
                    light: 0xF0F0,
                    lights: None,
                    glint: Some(Glint {
                        millis,
                        speed: 0.5,
                        alpha: 0.75,
                        scale: 8.0,
                    }),
                    tier: 0,
                },
                quads: Vec::new(),
            };
            state.submit(&renderer, &sprites, &NoHost, pose, &mut sink);
            let PairedWorldSink { quads, .. } = sink;
            assert_eq!(
                out.len(),
                expected_quads.len() * 4,
                "{label} {key} vertex count"
            );
            assert!(
                out.iter().all(|v| (v.layer & 0xFFFF) == 7),
                "{label} {key} expected glint overlay 7"
            );
            let got = submitted_quads(&quads, &out, None);
            assert_extruded_quads_match(
                &got,
                expected_quads,
                key,
                &format!("26.3 end_crystal {label} {key}"),
            );
        }
    }
}

/// 26.3 parity: block-textured enchanted items (`minecraft:stone`) use the stitched
/// `minecraft:blocks` (mip 4) atlas UVs with `GLINT_TEXTURING` (8.0) in both world and GUI.
#[test]
#[ignore = "run by tools/test_glint_parity.py"]
fn block_item_glint_uses_blocks_atlas_uv_26_3() {
    use sunrise_assets::item_model::NoEnv;

    let witness = glint_witness_26_3();
    let renderer = full_renderer_26_3();
    let sprites = dummy_hud_sprites(&["block/stone"]);
    let mut stack = StackView::new("minecraft:stone", 1);
    stack.patch.insert(
        "minecraft:enchantment_glint_override".into(),
        Some(Value::Bool(true)),
    );

    for (ctx_name, ctx) in [
        ("gui", DisplayContext::Gui),
        ("ground", DisplayContext::Ground),
    ] {
        let expected_quads = witness["block_item"][ctx_name]["quads"].as_array().unwrap();
        let state = renderer.resolve(&stack, ctx, None, None, 0, &NoEnv);
        for (millis, key) in [
            (0_i64, "glint_uv_t0_bits"),
            (123_456_i64, "glint_uv_t123456_bits"),
        ] {
            let mut out = Vec::new();
            state.submit(
                &renderer,
                &sprites,
                &NoHost,
                Mat4::IDENTITY,
                &mut WorldSink {
                    out: &mut out,
                    light: 0xF0F0,
                    lights: None,
                    glint: Some(Glint {
                        millis,
                        speed: 0.5,
                        alpha: 0.75,
                        scale: 8.0,
                    }),
                    tier: 0,
                },
            );
            assert_eq!(
                out.len(),
                expected_quads.len() * 4,
                "stone {ctx_name} {key}"
            );
            let got = vertex_quad_glint_bits(&out);
            for (q, want_quad) in expected_quads.iter().enumerate() {
                let want = parse_quad_uv_bits(&want_quad[key]);
                assert_eq!(
                    got[q], want,
                    "26.3 stone {ctx_name} {key} quad[{q}]: got={:#x?} want={:#x?}",
                    got[q], want
                );
            }
        }
    }

    // GUI draw_posed (no slot clipping) must also emit exact `blocks` atlas glint corners.
    let state = renderer.resolve(&stack, DisplayContext::Gui, None, None, 0, &NoEnv);
    let mut batch = crate::hud::HudBatch::scratch([64, 64]);
    let pose = Mat4::from_translation(Vec3::new(8.0, 8.0, 0.0))
        * Mat4::from_scale(Vec3::new(16.0, -16.0, 16.0));
    assert!(gui::draw_posed(
        &state,
        &gui::GuiItemEnv {
            renderer: &renderer,
            sprites: &sprites,
            host: &NoHost,
        },
        &mut batch,
        pose,
        GlintFrame {
            millis: 123_456,
            speed: 0.5,
            alpha: 0.75,
            sprite: true,
        },
    ));
    let allowed_corners =
        parse_quad_uv_bits(&witness["block_item"]["gui"]["quads"][0]["glint_uv_t123456_bits"]);
    let gui_glints: Vec<[u32; 2]> = batch
        .vertices
        .iter()
        .filter(|v| v.uv[0] >= 2.0)
        .map(|v| v.glint_uv.map(f32::to_bits))
        .collect();
    assert!(
        !gui_glints.is_empty(),
        "stone gui_draw_posed emitted no glint vertices"
    );
    for uv in gui_glints {
        assert!(
            allowed_corners.contains(&uv),
            "26.3 stone gui glint vertex {uv:#x?} not in vanilla blocks-atlas corners {allowed_corners:#x?}"
        );
    }
}

/// 26.3 parity: `Foil::Special` (`minecraft:compass`) projects sheeted decal `UV3` via
/// `ItemFeatureRenderer.computeFoilDecalPose` and transforms it with `GLINT_TEXTURING` (8.0).
#[test]
#[ignore = "run by tools/test_glint_parity.py"]
fn compass_special_foil_uses_decal_uv_26_3() {
    use sunrise_assets::item_model::NoEnv;

    let witness = glint_witness_26_3();
    let renderer = full_renderer_26_3();
    let sprites = dummy_hud_sprites(&["item/compass_16", "item/compass_00"]);
    let mut stack = StackView::new("minecraft:compass", 1);
    stack.patch.insert(
        "minecraft:enchantment_glint_override".into(),
        Some(Value::Bool(true)),
    );

    let gui_slot_pose = Mat4::from_translation(Vec3::new(8.0, 8.0, 0.0))
        * Mat4::from_scale(Vec3::new(16.0, -16.0, 16.0));
    // Match `VanillaItemGlintOracle.java`'s JOML `PoseStack` (`Quaternionf().rotationY(0.7F)`).
    let mut gp = crate::camera::joml::Mat4::IDENTITY;
    gp.translate(1.5, 2.0, -3.5);
    gp.rotate(crate::camera::joml::Quat::rotation_y(0.7));
    gp.scale(0.5, 0.5, 0.5);
    let ground_posed = gp.to_glam();

    let cases: [(&str, DisplayContext, Mat4); 9] = [
        ("gui", DisplayContext::Gui, Mat4::IDENTITY),
        (
            "firstperson_righthand",
            DisplayContext::FirstPersonRightHand,
            Mat4::IDENTITY,
        ),
        (
            "firstperson_lefthand",
            DisplayContext::FirstPersonLeftHand,
            Mat4::IDENTITY,
        ),
        ("ground", DisplayContext::Ground, Mat4::IDENTITY),
        ("fixed", DisplayContext::Fixed, Mat4::IDENTITY),
        (
            "thirdperson_righthand",
            DisplayContext::ThirdPersonRightHand,
            Mat4::IDENTITY,
        ),
        ("head", DisplayContext::Head, Mat4::IDENTITY),
        ("gui_slot_pose", DisplayContext::Gui, gui_slot_pose),
        ("ground_posed", DisplayContext::Ground, ground_posed),
    ];

    let compass_rect = renderer
        .glint_atlas
        .lock()
        .unwrap()
        .get_stitched("item/compass_16")
        .unwrap();
    for (case_key, ctx, pose) in cases {
        let expected_quads = witness["special_foil"][case_key]["quads"]
            .as_array()
            .unwrap();
        let state = renderer.resolve(&stack, ctx, None, None, 0, &NoEnv);
        for (millis, key) in [
            (0_i64, "glint_uv_t0_bits"),
            (123_456_i64, "glint_uv_t123456_bits"),
        ] {
            let mut out = Vec::new();
            let mut sink = PairedWorldSink {
                world: WorldSink {
                    out: &mut out,
                    light: 0xF0F0,
                    lights: None,
                    glint: Some(Glint {
                        millis,
                        speed: 0.5,
                        alpha: 0.75,
                        scale: 8.0,
                    }),
                    tier: 0,
                },
                quads: Vec::new(),
            };
            state.submit(&renderer, &sprites, &NoHost, pose, &mut sink);
            let PairedWorldSink { quads, .. } = sink;
            assert_eq!(
                out.len(),
                expected_quads.len() * 4,
                "compass {case_key} {key} vertex count"
            );
            let got = submitted_quads(&quads, &out, Some(compass_rect));
            assert_extruded_quads_match(
                &got,
                expected_quads,
                key,
                &format!("26.3 compass {case_key} {key}"),
            );
        }
    }
}

/// 26.3 parity: `minecraft:trident` uses `GLINT_TEXTURING` (8.0) + `items` atlas in 2D contexts
/// (`Gui`/`Ground`) and `ENTITY_GLINT_TEXTURING` (0.5) + raw UV in 3D hand contexts;
/// `minecraft:shield` (plain & patterned) uses `ENTITY_GLINT_TEXTURING` (0.5) +
/// `minecraft:shield_patterns` atlas UV in both `WorldSink` and GUI.
#[test]
#[ignore = "run by tools/test_glint_parity.py"]
fn special_model_trident_and_shield_glint_uses_entity_scale_26_3() {
    use sunrise_assets::item_model::NoEnv;

    let witness = glint_witness_26_3();
    let renderer = full_renderer_26_3();
    let sprites = dummy_hud_sprites(&[
        "item/trident",
        "entity/trident/trident",
        "entity/shield/shield_base_nopattern",
        "entity/shield/shield_base",
        "entity/shield/base",
    ]);

    let enchanted = |item: &str| {
        let mut s = StackView::new(item, 1);
        s.patch.insert(
            "minecraft:enchantment_glint_override".into(),
            Some(Value::Bool(true)),
        );
        s
    };

    let trident = enchanted("minecraft:trident");
    for (ctx_name, ctx) in [
        ("gui", DisplayContext::Gui),
        ("ground", DisplayContext::Ground),
    ] {
        let expected_quads = witness["special_models"]["trident"][ctx_name]["items"]["quads"]
            .as_array()
            .unwrap();
        let state = renderer.resolve(&trident, ctx, None, None, 0, &NoEnv);
        let mut out = Vec::new();
        let mut sink = PairedWorldSink {
            world: WorldSink {
                out: &mut out,
                light: 0xF0F0,
                lights: None,
                glint: Some(Glint {
                    millis: 123_456,
                    speed: 0.5,
                    alpha: 0.75,
                    scale: 8.0,
                }),
                tier: 0,
            },
            quads: Vec::new(),
        };
        state.submit(&renderer, &sprites, &NoHost, Mat4::IDENTITY, &mut sink);
        let PairedWorldSink { quads, .. } = sink;
        assert_eq!(out.len(), expected_quads.len() * 4, "trident 2d {ctx_name}");
        let got = submitted_quads(&quads, &out, None);
        assert_extruded_quads_match(
            &got,
            expected_quads,
            "glint_uv_t123456_bits",
            &format!("26.3 trident 2d {ctx_name}"),
        );
    }

    // Special-model child order differs from vanilla HashMap order, so compare sorted quads.
    for (ctx_name, ctx) in [
        (
            "firstperson_righthand",
            DisplayContext::FirstPersonRightHand,
        ),
        (
            "thirdperson_righthand",
            DisplayContext::ThirdPersonRightHand,
        ),
    ] {
        let expected_quads = witness["special_models"]["trident"][ctx_name]["models"][0]["quads"]
            .as_array()
            .unwrap();
        let mut want: Vec<[[u32; 2]; 4]> = expected_quads
            .iter()
            .map(|q| parse_quad_uv_bits(&q["glint_uv_t123456_bits"]))
            .collect();
        want.sort();
        let state = renderer.resolve(&trident, ctx, None, None, 0, &NoEnv);
        let mut out = Vec::new();
        state.submit(
            &renderer,
            &sprites,
            &NoHost,
            Mat4::IDENTITY,
            &mut WorldSink {
                out: &mut out,
                light: 0xF0F0,
                lights: None,
                glint: Some(Glint {
                    millis: 123_456,
                    speed: 0.5,
                    alpha: 0.75,
                    scale: 8.0,
                }),
                tier: 0,
            },
        );
        let mut got = vertex_quad_glint_bits(&out);
        got.sort();
        assert_eq!(got, want, "26.3 trident 3d {ctx_name}");
    }

    let shield_plain = enchanted("minecraft:shield");
    for (ctx_name, ctx) in [
        ("gui", DisplayContext::Gui),
        ("ground", DisplayContext::Ground),
    ] {
        let expected_quads =
            witness["special_models"]["shield_plain"][ctx_name]["models"][0]["quads"]
                .as_array()
                .unwrap();
        let mut want: Vec<[[u32; 2]; 4]> = expected_quads
            .iter()
            .map(|q| parse_quad_uv_bits(&q["glint_uv_t123456_bits"]))
            .collect();
        want.sort();
        let state = renderer.resolve(&shield_plain, ctx, None, None, 0, &NoEnv);
        let mut out = Vec::new();
        state.submit(
            &renderer,
            &sprites,
            &NoHost,
            Mat4::IDENTITY,
            &mut WorldSink {
                out: &mut out,
                light: 0xF0F0,
                lights: None,
                glint: Some(Glint {
                    millis: 123_456,
                    speed: 0.5,
                    alpha: 0.75,
                    scale: 8.0,
                }),
                tier: 0,
            },
        );
        let mut got = vertex_quad_glint_bits(&out);
        got.sort();
        assert_eq!(got, want, "26.3 shield_plain world {ctx_name}");
    }

    // GUI shields use ENTITY_GLINT_TEXTURING (0.5) over shield_patterns, never item scale 8.0.
    let state = renderer.resolve(&shield_plain, DisplayContext::Gui, None, None, 0, &NoEnv);
    let mut batch = crate::hud::HudBatch::scratch([64, 64]);
    let pose = Mat4::from_translation(Vec3::new(8.0, 8.0, 0.0))
        * Mat4::from_scale(Vec3::new(16.0, -16.0, 16.0));
    assert!(gui::draw_posed(
        &state,
        &gui::GuiItemEnv {
            renderer: &renderer,
            sprites: &sprites,
            host: &NoHost,
        },
        &mut batch,
        pose,
        GlintFrame {
            millis: 123_456,
            speed: 0.5,
            alpha: 0.75,
            sprite: true,
        },
    ));
    let expected_front_corners = parse_quad_uv_bits(
        &witness["special_models"]["shield_plain"]["gui"]["models"][0]["quads"][3]["glint_uv_t123456_bits"],
    );
    let expected_shield_vertices: Vec<[f32; 2]> = witness["special_models"]["shield_plain"]["gui"]
        ["models"][0]["quads"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|q| parse_quad_uv_bits(&q["glint_uv_t123456_bits"]))
        .map(|pair| [f32::from_bits(pair[0]), f32::from_bits(pair[1])])
        .collect();
    let (mut min_u, mut max_u) = (f32::INFINITY, f32::NEG_INFINITY);
    let (mut min_v, mut max_v) = (f32::INFINITY, f32::NEG_INFINITY);
    for [u, v] in &expected_shield_vertices {
        min_u = min_u.min(*u);
        max_u = max_u.max(*u);
        min_v = min_v.min(*v);
        max_v = max_v.max(*v);
    }
    let gui_glint_bits: Vec<[u32; 2]> = batch
        .vertices
        .iter()
        .filter(|v| v.uv[0] >= 2.0)
        .map(|v| v.glint_uv.map(f32::to_bits))
        .collect();
    assert!(
        !gui_glint_bits.is_empty(),
        "shield_plain gui emitted no glint vertices"
    );
    for corner in expected_front_corners {
        assert!(
            gui_glint_bits.contains(&corner),
            "26.3 shield_plain gui front-plate corner {corner:#x?} missing from emitted glint vertices"
        );
    }
    for [ub, vb] in gui_glint_bits {
        let (u, v) = (f32::from_bits(ub), f32::from_bits(vb));
        assert!(
            (min_u..=max_u).contains(&u) && (min_v..=max_v).contains(&v),
            "26.3 shield_plain gui glint ({u}, {v}) outside vanilla ENTITY_GLINT_TEXTURING bounds [{min_u}..={max_u}] x [{min_v}..={max_v}]"
        );
    }

    let mut shield_red = enchanted("minecraft:shield");
    shield_red.patch.insert(
        "minecraft:base_color".into(),
        Some(Value::String("red".into())),
    );
    let patterned_glint_quads =
        witness["special_models"]["shield_red"]["ground"]["models"][2]["quads"]
            .as_array()
            .unwrap();
    let mut want_single: Vec<[[u32; 2]; 4]> = patterned_glint_quads
        .iter()
        .map(|q| parse_quad_uv_bits(&q["glint_uv_t123456_bits"]))
        .collect();
    // Both base and pattern layers carry foil in Sunrise and sample shield_base's glint UVs.
    let mut want_both = want_single.clone();
    want_both.append(&mut want_single);
    want_both.sort();
    let state = renderer.resolve(&shield_red, DisplayContext::Ground, None, None, 0, &NoEnv);
    let mut out = Vec::new();
    state.submit(
        &renderer,
        &sprites,
        &NoHost,
        Mat4::IDENTITY,
        &mut WorldSink {
            out: &mut out,
            light: 0xF0F0,
            lights: None,
            glint: Some(Glint {
                millis: 123_456,
                speed: 0.5,
                alpha: 0.75,
                scale: 8.0,
            }),
            tier: 0,
        },
    );
    let mut got = vertex_quad_glint_bits(&out);
    got.sort();
    assert_eq!(got, want_both, "26.3 shield_red patterned_shield_glint");
}

/// 26.3 parity: `prepare_glint_atlas_with_mips` matches vanilla `SpriteLoader.stitch` bit-for-bit
/// across `items` (none & aniso2), `blocks` (mip 4 & mip 0), and `shield_patterns`.
#[test]
#[ignore = "run by tools/test_glint_parity.py"]
fn glint_atlases_match_vanilla_26_3_mip_and_aniso() {
    let witness = glint_witness_26_3();
    let root = prepared_resources_26_3();
    let data = crate::test_resources::item_data(
        &root,
        sunrise_world::native_load_context::DataLoadOrigin::ClientPreload,
    )
    .expect("26.3 prepared item data");
    let renderer = ItemRenderer::load(&ResourceRoot::directory(root), &data).unwrap();

    let check = |atlas: &glint_atlas::GlintAtlas, key: &str| {
        let expected = &witness["atlases"][key];
        assert_eq!(
            atlas.len(),
            expected["sprite_count"].as_u64().unwrap() as usize,
            "{key} sprite_count"
        );
        for (id, sprite) in expected["sprites"].as_object().unwrap() {
            let r = atlas.get_stitched(sprite_name(id)).unwrap();
            let got = [r.u0, r.v0, r.u1, r.v1].map(f32::to_bits);
            let want = [
                sprite["u0_bits"].as_u64().unwrap() as u32,
                sprite["v0_bits"].as_u64().unwrap() as u32,
                sprite["u1_bits"].as_u64().unwrap() as u32,
                sprite["v1_bits"].as_u64().unwrap() as u32,
            ];
            assert_eq!(got, want, "26.3 atlas {key} sprite {id}");
        }
    };

    renderer
        .prepare_glint_atlas_with_mips(16384, (0, 2), 4)
        .unwrap();
    check(&renderer.glint_atlas.lock().unwrap(), "items_none");
    check(
        &renderer.block_glint_atlas.lock().unwrap(),
        "blocks_mip4_none",
    );
    check(
        &renderer.shield_glint_atlas.lock().unwrap(),
        "shield_patterns_none",
    );
    // `MaterialBaker.get` maps explicit `minecraft:missingno` to the blocks atlas's missing sprite.
    let resolved_missing = resolve_cuboid_glint(
        "missingno",
        &renderer.glint_atlas.lock().unwrap(),
        &renderer.block_glint_atlas.lock().unwrap(),
    )
    .unwrap();
    let want_missing: [u32; 4] = witness["material_baker"]["vanilla_missingno_bits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u32)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    assert_eq!(
        [
            resolved_missing.u0,
            resolved_missing.v0,
            resolved_missing.u1,
            resolved_missing.v1
        ]
        .map(f32::to_bits),
        want_missing,
        "26.3 MaterialBaker missingno"
    );
    assert_eq!(
        witness["material_baker"]["synth_items_has_shared"].as_bool(),
        Some(false)
    );
    assert_eq!(
        witness["material_baker"]["synth_shared_atlas"].as_str(),
        Some("minecraft:blocks")
    );
    assert_eq!(
        witness["material_baker"]["synth_missingno_atlas"].as_str(),
        Some("minecraft:blocks")
    );

    renderer
        .prepare_glint_atlas_with_mips(16384, (0, 2), 0)
        .unwrap();
    check(
        &renderer.block_glint_atlas.lock().unwrap(),
        "blocks_mip0_none",
    );

    renderer
        .prepare_glint_atlas_with_mips(16384, (2, 2), 4)
        .unwrap();
    check(&renderer.glint_atlas.lock().unwrap(), "items_aniso2");
}
