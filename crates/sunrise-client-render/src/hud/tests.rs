/// A scratch tree with a 2 x 1 sheet (an opaque red texel, then a transparent green one) and
/// the two atlas definitions.
fn model_atlas_root(tag: &str, items: &str, blocks: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("sunrise-hud-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let textures = root.join("assets/demo/textures/item");
    let atlases = root.join("assets/minecraft/atlases");
    std::fs::create_dir_all(&textures).unwrap();
    std::fs::create_dir_all(&atlases).unwrap();
    let mut encoder = png::Encoder::new(
        std::fs::File::create(textures.join("sheet.png")).unwrap(),
        2,
        1,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&[255, 0, 0, 255, 0, 255, 0, 0])
        .unwrap();
    std::fs::write(atlases.join("items.json"), items).unwrap();
    std::fs::write(atlases.join("blocks.json"), blocks).unwrap();
    root
}
fn cut(sprite: &str, x: u8) -> String {
    format!(
        r#"{{"sources":[{{"type":"minecraft:unstitch","resource":"demo:item/sheet","divisor_x":2,
        "regions":[{{"sprite":"{sprite}","x":{x},"y":0,"width":1,"height":1}}]}}]}}"#
    )
}
/// A model's material is the items atlas's cut when it has one: it replaces both the blocks
/// atlas's cut and the walked file of that name, which is dropped from the pack.
#[test]
fn the_items_atlas_cut_replaces_a_packed_file_and_the_blocks_cut() {
    let root = model_atlas_root(
        "cut-wins",
        &cut("demo:item/cut", 1),
        &cut("demo:item/cut", 0),
    );
    let packed = vec![("demo:item/cut".to_owned(), 1, 1, vec![9, 9, 9, 9])];
    let AtlasSprites {
        images, dropped, ..
    } = atlas_sprites(&ResourceSource::directory(root.clone()), &packed);
    assert_eq!(
        images,
        [("demo:item/cut".to_owned(), 1, 1, vec![0, 255, 0, 0])]
    );
    assert_eq!(dropped, HashSet::from(["demo:item/cut".to_owned()]));
    std::fs::remove_dir_all(&root).unwrap();
}
/// A walked file whose animation frames can't tile it does not stand: as the item mask does,
/// the pack takes the blocks atlas's sprite of the id instead.
#[test]
fn a_walked_file_with_unfitting_frames_gives_way_to_the_blocks_cut() {
    let items = r#"{"sources":[{"type":"minecraft:single","resource":"minecraft:item/target"}]}"#;
    let root = model_atlas_root("unfitting", items, &cut("minecraft:item/target", 1));
    let textures = root.join("assets/minecraft/textures/item");
    std::fs::create_dir_all(&textures).unwrap();
    // A file the HUD walk really packs: 1 x 1, declaring frames 2 wide.
    write_strip(
        &textures.join("target.png"),
        1,
        1,
        &[[9, 9, 9, 9]],
        Some(r#"{"animation":{"width":2}}"#),
    );
    let atlas = HudAtlas::build(&ResourceSource::directory(root.clone())).unwrap();
    let rect = atlas.sprites["item/target"];
    assert_eq!(rect.texels((atlas.width, atlas.height)), (1, 1));
    assert_eq!(texel_at(&atlas, rect, 0, 0), [0, 255, 0, 0]);
    std::fs::remove_dir_all(&root).unwrap();
}
/// The winning renamed file's own render flags come with its pixels, and another atlas's
/// output of a model id does not replace the model atlases' winner.
#[test]
fn a_winner_carries_its_flags_and_no_other_atlas_replaces_it() {
    let items = r#"{"sources":[
        {"type":"minecraft:single","resource":"demo:item/sheet","sprite":"demo:item/blurred"}]}"#;
    let root = model_atlas_root("flags", items, &cut("demo:item/cut", 0));
    std::fs::write(
        root.join("assets/demo/textures/item/sheet.png.mcmeta"),
        r#"{"texture":{"blur":true,"clamp":true},"animation":{"width":1,"height":1}}
// High Contrast pack signature"#,
    )
    .unwrap();
    // The GUI atlas cuts the other quadrant under the id the blocks atlas cuts.
    std::fs::write(
        root.join("assets/minecraft/atlases/gui.json"),
        cut("demo:item/cut", 1),
    )
    .unwrap();
    let atlas = HudAtlas::build(&ResourceSource::directory(root.clone())).unwrap();
    assert_eq!(
        texel_at(&atlas, atlas.sprites["demo:item/cut"], 0, 0),
        [255, 0, 0, 255],
        "the blocks atlas's quadrant, not the GUI atlas's"
    );
    let rect = atlas.sprites["demo:item/blurred"];
    assert_eq!(rect.texels((atlas.width, atlas.height)), (1, 1));
    assert_eq!(texel_at(&atlas, rect, 0, 0), [255, 0, 0, 255]);
    assert_eq!(
        atlas.flags["demo:item/blurred"],
        TextureFlags {
            blur: true,
            clamp: true,
        }
    );
    let (_, animation) = atlas
        .sprite_animations
        .iter()
        .find(|(r, _)| r.u0 == rect.u0 && r.v0 == rect.v0)
        .expect("the renamed sprite animates");
    assert_eq!(animation.frame_count(), 2);
    std::fs::remove_dir_all(&root).unwrap();
}
/// The items atlas keeps the walked file of its name; the blocks atlas's lower cut of that
/// name adds nothing and drops nothing. A walked item texture neither atlas lists is dropped.
#[test]
fn a_walked_file_the_items_atlas_keeps_stands_and_an_unlisted_one_goes() {
    let items = r#"{"sources":[{"type":"minecraft:single","resource":"demo:item/kept"}]}"#;
    let root = model_atlas_root("walked", items, &cut("demo:item/kept", 0));
    let textures = root.join("assets/demo/textures/item");
    std::fs::copy(textures.join("sheet.png"), textures.join("kept.png")).unwrap();
    let packed = vec![
        ("demo:item/kept".to_owned(), 1, 1, vec![9, 9, 9, 9]),
        ("item/unlisted".to_owned(), 1, 1, vec![8, 8, 8, 8]),
        ("gui/not_a_model".to_owned(), 1, 1, vec![7, 7, 7, 7]),
    ];
    let AtlasSprites {
        images, dropped, ..
    } = atlas_sprites(&ResourceSource::directory(root.clone()), &packed);
    assert_eq!(images, []);
    assert_eq!(dropped, HashSet::from(["item/unlisted".to_owned()]));
    std::fs::remove_dir_all(&root).unwrap();
}
use super::*;

#[test]
fn scratch_batch_ignores_view_settings() {
    let batch = HudBatch::scratch([960, 720]);
    let expected = sunrise_client_input::window::GuiSize::new([960, 720], 0, false);
    assert_eq!(batch.scale, expected.scale as f32);
    assert_eq!([batch.width, batch.height], expected.size.map(|v| v as f32));
}
#[test]
fn animated_texture_metadata_ignores_trailing_pack_signature() {
    let meta = r#"{"animation":{"frametime":2,"width":2,"height":2}}
// High Contrast pack signature"#;
    let animation = animation_of(Some(meta)).expect("the first JSON value is metadata");
    assert_eq!(animation.frame_size(8, 4), [2, 2]);
    let pixels = (0..8 * 4).flat_map(|i| [i as u8, 0, 0, 255]).collect();
    let (_, height, first) = first_frame((8, 4, pixels), Some(&animation)).unwrap();
    assert_eq!(height, 2);
    assert_eq!(first.len(), 2 * 2 * 4);
}

#[test]
fn object_skin_revision_tracks_changes_and_capacity_clear() {
    let mut atlas = HudAtlas::boot();
    atlas.set_object_skin("name:player".into(), "skins/server".into());
    let revision = atlas.object_skins_revision;
    atlas.set_object_skin("name:player".into(), "skins/server".into());
    assert_eq!(atlas.object_skins_revision, revision);
    atlas.set_object_skin("name:player".into(), "skins/inline".into());
    assert_ne!(atlas.object_skins_revision, revision);
    for i in 1..1024 {
        atlas.set_object_skin(format!("name:{i}"), "skins/server".into());
    }
    let revision = atlas.object_skins_revision;
    atlas.set_object_skin("name:overflow".into(), "skins/server".into());
    assert_ne!(atlas.object_skins_revision, revision);
    assert_eq!(atlas.object_skins.len(), 1);
    let revision = atlas.object_skins_revision;
    atlas.clear_object_skins();
    assert_ne!(atlas.object_skins_revision, revision);
    assert!(atlas.object_skins.is_empty());
}

/// The pool of dynamic texture slots is paid only by a game with a client mod loaded: with
/// none, the atlas packs no slot and does not grow. The slots' names leave the sprites, so
/// no mod draws a slot another mod's image holds.
#[test]
fn the_dynamic_texture_pool_exists_only_when_a_mod_is_loaded() {
    let root = sunrise_assets::resource_source::ResourceSource::directory(
        Path::new("/nonexistent/sunrise-test-assets").to_path_buf(),
    );
    let plain = HudAtlas::build(&root).unwrap();
    let without = HudAtlas::build_over_with_dynamic_textures(&root, &root, &root, false).unwrap();
    assert!(plain.dynamic_texture_pool.is_none() && without.dynamic_texture_pool.is_none());
    assert_eq!((without.width, without.height), (plain.width, plain.height));
    let with = HudAtlas::build_over_with_dynamic_textures(&root, &root, &root, true).unwrap();
    let pool = with.dynamic_texture_pool.as_ref().expect("the pool");
    assert_eq!(pool.slots(), dynamic_textures::DYNAMIC_TEXTURE_SLOTS);
    assert!(
        !with
            .sprites
            .keys()
            .any(|name| name.starts_with("sunrise/dynamic-texture/"))
    );
    // Every slot lies whole in the atlas, apart from every other slot.
    let mut slots = Vec::new();
    for (class, side) in mod_api::DYNAMIC_TEXTURE_CLASSES.into_iter().enumerate() {
        for slot in 0..pool.slots()[class] {
            let [x, y] = pool.origin(class, slot);
            assert!(x + side <= with.width && y + side <= with.height);
            slots.push([x, y, x + side, y + side]);
        }
    }
    for (i, a) in slots.iter().enumerate() {
        for b in &slots[i + 1..] {
            assert!(a[2] <= b[0] || b[2] <= a[0] || a[3] <= b[1] || b[3] <= a[1]);
        }
    }
}

/// `assets/minecraft/atlases/gui.json` as the game's resources have it.
fn write_vanilla_gui_definition(root: &std::path::Path) {
    let atlases = root.join("assets/minecraft/atlases");
    fs::create_dir_all(&atlases).unwrap();
    fs::write(
        atlases.join("gui.json"),
        r#"{"sources":[
            {"type":"minecraft:directory","prefix":"","source":"gui/sprites"},
            {"type":"minecraft:directory","prefix":"mob_effect/","source":"mob_effect"}]}"#,
    )
    .unwrap();
}
fn write_pixel(path: &std::path::Path, rgba: [u8; 4]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), 1, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&rgba)
        .unwrap();
}
/// The GUI atlas's final list decides its sprites, under the names the GUI is looked up by
/// (vanilla's definition: the folder `gui/sprites` with no prefix): a mod's pixels replace a
/// walked sprite, an alias takes its texture's pixels and flags under the mod's plain id, a
/// removed or filtered sprite and a walked one no entry lists are dropped, a kept one
/// stands, and every reader of the load shares one answer from the mods' source.
#[test]
fn the_gui_atlas_takes_replacement_alias_and_removal_from_its_final_list() {
    let root = std::env::temp_dir().join(format!("sunrise-hud-gui-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let sprites = root.join("assets/minecraft/textures/gui/sprites");
    for (name, rgba) in [
        ("replaced", [1, 1, 1, 255]),
        ("removed", [2, 2, 2, 255]),
        ("filtered", [3, 3, 3, 255]),
        ("kept", [4, 4, 4, 255]),
        ("source", [5, 5, 5, 255]),
    ] {
        write_pixel(&sprites.join(format!("{name}.png")), rgba);
    }
    std::fs::write(
        sprites.join("source.png.mcmeta"),
        r#"{"texture":{"blur":true}}"#,
    )
    .unwrap();
    std::fs::create_dir_all(root.join("assets/minecraft/atlases")).unwrap();
    for model_atlas in ["blocks", "items"] {
        std::fs::write(
            root.join(format!("assets/minecraft/atlases/{model_atlas}.json")),
            r#"{"sources":[]}"#,
        )
        .unwrap();
    }
    std::fs::write(
        root.join("assets/minecraft/atlases/gui.json"),
        r#"{"sources":[
            {"type":"directory","source":"gui/sprites","prefix":""},
            {"type":"srcgui:own"},
            {"type":"filter","pattern":{"path":"filtered"}}]}"#,
    )
    .unwrap();
    let mods = crate::atlas_sources::test_mods::Mods::new(&["srcgui:own"], |_, _, _, _| {
        vec![
            mod_api::SpriteSourceOutput::Pixels {
                sprite: mod_api::Identifier::parse("minecraft:replaced").unwrap(),
                width: 1,
                height: 1,
                rgba: vec![9, 9, 9, 255],
            },
            mod_api::SpriteSourceOutput::Remove {
                sprite: mod_api::Identifier::parse("minecraft:removed").unwrap(),
            },
            mod_api::SpriteSourceOutput::Texture {
                sprite: mod_api::Identifier::parse("srcgui:alias").unwrap(),
                texture: mod_api::Identifier::parse("minecraft:gui/sprites/source").unwrap(),
            },
        ]
    });
    let source = ResourceSource::directory(root.clone());
    crate::atlas_sources::shared(&source, &mods);
    let packed: Vec<PackedImage> = ["replaced", "removed", "filtered", "kept", "stray"]
        .iter()
        .map(|name| (format!("gui/sprites/{name}"), 1, 1, vec![0, 0, 0, 255]))
        .collect();
    let AtlasSprites {
        images,
        dropped,
        flags,
        ..
    } = atlas_sprites(&source, &packed);
    let pixels = |name: &str| {
        images
            .iter()
            .find(|image| image.0 == name)
            .map(|image| image.3.clone())
    };
    assert_eq!(pixels("#gui:minecraft:replaced"), Some(vec![9, 9, 9, 255]));
    assert_eq!(pixels("#gui:srcgui:alias"), Some(vec![5, 5, 5, 255]));
    assert!(
        flags["#gui:srcgui:alias"].blur,
        "the aliased file's own flags"
    );
    assert_eq!(
        pixels("gui/sprites/kept"),
        None,
        "a walked file stands as packed"
    );
    assert_eq!(
        dropped,
        HashSet::from(
            ["replaced", "removed", "filtered", "stray"].map(|name| format!("gui/sprites/{name}"))
        )
    );
    // The HUD's other readers of the same load: no further question to the mods.
    crate::atlas_sources::Materials::new(&source);
    crate::atlas_sources::listing(&source);
    assert_eq!(
        *mods.asked.lock().unwrap(),
        [("minecraft:gui".to_owned(), "srcgui:own".to_owned())]
    );
    std::fs::remove_dir_all(&root).unwrap();
}
/// A GUI alias of an animated strip animates the strip it names under its own name, even
/// where a model has the alias's id.
#[test]
fn a_gui_alias_keeps_its_source_file() {
    let root = model_atlas_root(
        "gui-alias-animated",
        &cut("demo:item/model", 0),
        &cut("demo:item/other", 1),
    );
    std::fs::write(
        root.join("assets/demo/textures/item/sheet.png.mcmeta"),
        r#"{"animation":{"width":1}}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("assets/minecraft/atlases/gui.json"),
        r#"{"sources":[
            {"type":"minecraft:single","resource":"demo:item/sheet","sprite":"demo:alias"},
            {"type":"minecraft:single","resource":"demo:item/sheet","sprite":"demo:item/model"}]}"#,
    )
    .unwrap();
    let found = atlas_sprites(&ResourceSource::directory(root.clone()), &[]);
    for name in ["#gui:demo:alias", "#gui:demo:item/model"] {
        assert!(
            found.animated.iter().any(|animated| animated == name),
            "{name}"
        );
        assert_eq!(
            found.gui_files.get(name).map(String::as_str),
            Some("assets/demo/textures/item/sheet.png"),
            "the entry's own file, not a model's pixels"
        );
    }
    std::fs::remove_dir_all(&root).unwrap();
}
/// Where a model's sprite has the id of a GUI entry, the GUI lookup is the GUI atlas's own
/// answer and the model lookup is the model's: a GUI replacement is found as the GUI's pixels,
/// a GUI sprite its list removed or failed is the missing sprite, never the model's. Names the
/// GUI list says nothing of (a registered atlas's sprite, a live dynamic texture) resolve
/// through the plain table, where no model has the name.
#[test]
fn a_gui_entry_never_takes_a_models_sprite_of_its_id() {
    let items = r#"{"sources":[{"type":"minecraft:unstitch","resource":"demo:item/sheet","divisor_x":2,
        "regions":[
          {"sprite":"demo:item/replaced","x":0,"y":0,"width":1,"height":1},
          {"sprite":"demo:item/removed","x":0,"y":0,"width":1,"height":1},
          {"sprite":"demo:item/failed","x":0,"y":0,"width":1,"height":1}]}]}"#;
    let root = model_atlas_root("gui-collision", items, r#"{"sources":[]}"#);
    let textures = root.join("assets/demo/textures/item");
    // A strip whose animation frames don't tile it: no sprite.
    std::fs::copy(textures.join("sheet.png"), textures.join("bad.png")).unwrap();
    std::fs::write(
        textures.join("bad.png.mcmeta"),
        r#"{"animation":{"width":3}}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("assets/minecraft/atlases/gui.json"),
        r#"{"sources":[
            {"type":"minecraft:single","resource":"demo:item/sheet","sprite":"demo:item/replaced"},
            {"type":"minecraft:single","resource":"demo:item/sheet","sprite":"demo:item/removed"},
            {"type":"minecraft:single","resource":"demo:item/bad","sprite":"demo:item/failed"},
            {"type":"minecraft:filter","pattern":{"path":"item/removed"}}]}"#,
    )
    .unwrap();
    let mut atlas = HudAtlas::build(&ResourceSource::directory(root.clone())).unwrap();
    let size = |rect: Option<&Rect>| rect.map(|rect| rect.texels((atlas.width, atlas.height)));
    let missing = size(atlas.sprites.get(MISSING_SPRITE));
    // The GUI's own 2 x 1 strip, and the model's 1 x 1 cut under the same id.
    assert_eq!(size(atlas.gui_rect("demo:item/replaced")), Some((2, 1)));
    assert_eq!(size(atlas.sprites.get("demo:item/replaced")), Some((1, 1)));
    for name in ["demo:item/removed", "demo:item/failed"] {
        assert_eq!(
            size(atlas.gui_rect(name)),
            missing,
            "{name}: the missing sprite"
        );
        assert_eq!(
            size(atlas.sprites.get(name)),
            Some((1, 1)),
            "{name}: the model's sprite is untouched"
        );
    }
    assert!(
        atlas.sprites.keys().all(|name| !name.starts_with(GUI_KEY)),
        "the internal names stay internal"
    );
    // Not in the GUI list: found as packed, a dynamic texture or a registered atlas's sprite.
    let live = *atlas.sprites.get("demo:item/replaced").unwrap();
    atlas.sprites.insert("demo:live".into(), live);
    assert_eq!(size(atlas.gui_rect("demo:live")), Some((1, 1)));
    assert!(atlas.gui_rect("demo:unknown").is_none());
    std::fs::remove_dir_all(&root).unwrap();
}
/// Two legal GUI ids that name one walked file each keep their own sprite: `panel` stands on
/// the walked `gui/sprites/panel`, and `gui/sprites/panel` (the id whose plain name that is)
/// has another origin, so replacing one never touches the other. An id only a model has
/// is no GUI sprite, while its model lookup finds it.
#[test]
fn two_gui_ids_of_one_walked_name_and_a_model_only_id_keep_their_own_results() {
    let items = r#"{"sources":[{"type":"minecraft:unstitch","resource":"demo:item/sheet","divisor_x":2,
        "regions":[{"sprite":"demo:item/model_only","x":0,"y":0,"width":1,"height":1}]}]}"#;
    let root = model_atlas_root("gui-two-ids", items, r#"{"sources":[]}"#);
    write_pixel(
        &root.join("assets/minecraft/textures/gui/sprites/panel.png"),
        [200, 0, 0, 255],
    );
    std::fs::write(
        root.join("assets/minecraft/atlases/gui.json"),
        r#"{"sources":[
            {"type":"minecraft:directory","prefix":"","source":"gui/sprites"},
            {"type":"minecraft:single","resource":"demo:item/sheet","sprite":"minecraft:gui/sprites/panel"}]}"#,
    )
    .unwrap();
    let atlas = HudAtlas::build(&ResourceSource::directory(root.clone())).unwrap();
    let panel = *atlas.gui_rect("panel").unwrap();
    let other = *atlas.gui_rect("gui/sprites/panel").unwrap();
    assert_eq!(
        texel_at(&atlas, panel, 0, 0),
        [200, 0, 0, 255],
        "the walked file"
    );
    assert_eq!(
        other.texels((atlas.width, atlas.height)),
        (2, 1),
        "the sheet"
    );
    assert_eq!(texel_at(&atlas, other, 0, 0), [255, 0, 0, 255]);
    assert_ne!((panel.u0, panel.v0), (other.u0, other.v0), "two sprites");
    assert!(atlas.gui_lookup("demo:item/model_only").is_none());
    // A filter of the second id leaves it a removal, never the first id's surviving file.
    std::fs::write(
        root.join("assets/minecraft/atlases/gui.json"),
        r#"{"sources":[
            {"type":"minecraft:directory","prefix":"","source":"gui/sprites"},
            {"type":"minecraft:single","resource":"demo:item/sheet","sprite":"minecraft:gui/sprites/panel"},
            {"type":"minecraft:filter","pattern":{"path":"gui/sprites/panel"}}]}"#,
    )
    .unwrap();
    let atlas = HudAtlas::build(&ResourceSource::directory(root.clone())).unwrap();
    assert_eq!(
        texel_at(&atlas, *atlas.gui_rect("panel").unwrap(), 0, 0),
        [200, 0, 0, 255]
    );
    for id in ["gui/sprites/panel", "minecraft:gui/sprites/panel"] {
        assert!(
            matches!(atlas.gui_lookup(id), Some(GuiSprite::Missing)),
            "{id}"
        );
        assert!(
            atlas.find_inline_sprite("minecraft:gui", id).is_none(),
            "{id}"
        );
        assert_eq!(
            atlas.inline_sprite("minecraft:gui", id),
            Some(atlas.sprites[MISSING_SPRITE]),
            "{id}"
        );
    }
    assert!(atlas.gui_rect("demo:item/model_only").is_none());
    assert!(atlas.sprites.contains_key("demo:item/model_only"));
    std::fs::remove_dir_all(&root).unwrap();
}
/// A file another owner packed under a name that is also a GUI id (a registered atlas's
/// projection is `mymod:gui/atlases/...`) is not erased by a GUI filter of that id: it stays
/// in the packed table, while the GUI lane's answer for the id is its removal.
#[test]
fn a_gui_filter_never_erases_a_registered_atlas_sprite_of_its_name() {
    let root = model_atlas_root("gui-registered", r#"{"sources":[]}"#, r#"{"sources":[]}"#);
    write_pixel(
        &root.join("assets/demo/textures/gui/atlases/icons/demo/heart.png"),
        [0, 200, 0, 255],
    );
    std::fs::write(
        root.join("assets/minecraft/atlases/gui.json"),
        r#"{"sources":[
            {"type":"minecraft:single","resource":"demo:item/sheet","sprite":"demo:gui/atlases/icons/demo/heart"},
            {"type":"minecraft:filter","pattern":{"namespace":"demo","path":"gui/atlases/icons/demo/heart"}}]}"#,
    )
    .unwrap();
    let atlas = HudAtlas::build(&ResourceSource::directory(root.clone())).unwrap();
    let name = "demo:gui/atlases/icons/demo/heart";
    let packed = *atlas
        .registered
        .get(name)
        .expect("the projection stays packed");
    assert_eq!(texel_at(&atlas, packed, 0, 0), [0, 200, 0, 255]);
    // The GUI list removed the id: the GUI lane says so, whatever else is packed under the name.
    assert!(matches!(atlas.gui_lookup(name), Some(GuiSprite::Missing)));
    std::fs::remove_dir_all(&root).unwrap();
}
/// A registered atlas's sprite and an items atlas entry of the same name (`mymod:gui/atlases/..`)
/// each keep their own packed sprite: the GUI lookup finds the registered sprite, and the
/// model lookup its own.
#[test]
fn a_registered_atlas_sprite_and_a_model_of_one_name_both_survive() {
    let name = "demo:gui/atlases/icons/demo/heart";
    let items = format!(
        r#"{{"sources":[{{"type":"minecraft:single","resource":"demo:item/sheet","sprite":"{name}"}}]}}"#
    );
    let root = model_atlas_root("gui-registered-model", &items, r#"{"sources":[]}"#);
    write_vanilla_gui_definition(&root);
    // The staged file of the registered atlas's sprite, as its projection lays it down: two
    // frames (green, blue), played from the second.
    let staged = root.join("assets/demo/textures/gui/atlases/icons/demo/heart.png");
    std::fs::create_dir_all(staged.parent().unwrap()).unwrap();
    let mut encoder = png::Encoder::new(std::fs::File::create(&staged).unwrap(), 1, 2);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&[0, 200, 0, 255, 0, 0, 200, 255])
        .unwrap();
    std::fs::write(
        staged.with_extension("png.mcmeta"),
        r#"{"animation":{"width":1,"height":1,"frames":[1,0]}}"#,
    )
    .unwrap();
    let raw_name = "gui/atlases/icons/demo/raw";
    let raw_file = root.join(format!("assets/minecraft/textures/{raw_name}.png"));
    write_pixel(&raw_file, [200, 100, 0, 255]);
    std::fs::write(
        raw_file.with_extension("png.mcmeta"),
        r#"{"texture":{"blur":true,"clamp":true}}"#,
    )
    .unwrap();
    let atlas = HudAtlas::build(&ResourceSource::directory(root.clone())).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
    let registered = *atlas.gui_rect(name).expect("the registered sprite");
    assert_eq!(
        texel_at(&atlas, registered, 0, 0),
        [0, 0, 200, 255],
        "the first frame of the staged file's animation"
    );
    let (_, animation) = atlas
        .sprite_animations
        .iter()
        .find(|(rect, _)| rect.u0 == registered.u0 && rect.v0 == registered.v0)
        .expect("the registered sprite animates");
    assert_eq!(
        animation.resized_pixels(0, 1, 1),
        [0, 200, 0, 255],
        "a later frame"
    );
    assert_eq!(atlas.texture_rect(name), atlas.sprites.get(name));
    let model = *atlas.sprites.get(name).expect("the model's sprite");
    assert_eq!(texel_at(&atlas, model, 0, 0), [255, 0, 0, 255]);
    let raw = atlas.texture_rect(raw_name).expect("the exact raw name");
    assert_eq!(texel_at(&atlas, *raw, 0, 0), [200, 100, 0, 255]);
    assert_eq!(
        atlas.flags[raw_name],
        TextureFlags {
            blur: true,
            clamp: true,
        }
    );
    let prefixed = format!("minecraft:{raw_name}");
    assert!(atlas.texture_rect(&prefixed).is_none());
    assert_eq!(atlas.gui_rect(&prefixed), Some(raw));
    assert!(
        atlas
            .sprites
            .keys()
            .all(|key| !key.starts_with(REGISTERED_KEY)),
        "the internal keys stay internal"
    );
}
/// A resource tree with only a GUI definition publishes its GUI sprites.
#[test]
fn a_tree_with_only_a_gui_definition_publishes_its_sprites() {
    let root = model_atlas_root("gui-only", "{}", "{}");
    std::fs::remove_file(root.join("assets/minecraft/atlases/blocks.json")).unwrap();
    std::fs::remove_file(root.join("assets/minecraft/atlases/items.json")).unwrap();
    std::fs::write(
        root.join("assets/minecraft/atlases/gui.json"),
        r#"{"sources":[
            {"type":"minecraft:single","resource":"demo:item/sheet","sprite":"demo:alias"},
            {"type":"minecraft:unstitch","resource":"demo:item/sheet","divisor_x":2,
             "regions":[{"sprite":"demo:cut","x":1,"y":0,"width":1,"height":1}]}]}"#,
    )
    .unwrap();
    let atlas = HudAtlas::build(&ResourceSource::directory(root.clone())).unwrap();
    let size = |name: &str| {
        atlas
            .gui_rect(name)
            .map(|rect| rect.texels((atlas.width, atlas.height)))
    };
    assert_eq!(size("demo:alias"), Some((2, 1)));
    assert_eq!(size("demo:cut"), Some((1, 1)));
    std::fs::remove_dir_all(&root).unwrap();
}
/// A GUI override of a walked vanilla sprite is what each kind of GUI consumer gets by the
/// sprite's id: a built-in draw, an inline atlas glyph, a size measurement and a mod's map
/// decoration sprite (all the one lookup), and the walked file's packed name is gone.
#[test]
fn a_gui_override_reaches_every_kind_of_consumer() {
    let root = model_atlas_root("gui-consumers", r#"{"sources":[]}"#, r#"{"sources":[]}"#);
    write_vanilla_gui_definition(&root);
    // The walked sprite: 1 x 1. The override: the 2 x 1 sheet, under the same id.
    write_pixel(
        &root.join("assets/minecraft/textures/gui/sprites/container/slot.png"),
        [9, 9, 9, 255],
    );
    std::fs::write(
        root.join("assets/minecraft/atlases/gui.json"),
        r#"{"sources":[
            {"type":"minecraft:directory","prefix":"","source":"gui/sprites"},
            {"type":"minecraft:single","resource":"demo:item/sheet","sprite":"minecraft:container/slot"},
            {"type":"minecraft:single","resource":"demo:item/sheet","sprite":"demo:marker"}]}"#,
    )
    .unwrap();
    let atlas = HudAtlas::build(&ResourceSource::directory(root.clone())).unwrap();
    let (w, h) = (atlas.width, atlas.height);
    let own = *atlas.gui_rect("container/slot").unwrap();
    assert_eq!(own.texels((w, h)), (2, 1), "the override's pixels");
    assert!(
        !atlas.sprites.contains_key("gui/sprites/container/slot"),
        "the walked file it replaced is no sprite"
    );
    // A built-in draw by id.
    let mut batch = HudBatch::scratch([320, 240]);
    batch.sprite(&atlas, "container/slot", 10.0, 20.0);
    assert_eq!(batch.vertices[1].pos, [12.0, 20.0], "drawn 2 wide");
    assert_eq!(batch.vertices[2].pos, [12.0, 21.0], "and 1 high");
    assert_eq!(batch.vertices[0].uv, [own.u0, own.v0]);
    // An inline atlas glyph of the GUI atlas, by the qualified id.
    let glyph = atlas
        .inline_sprite("minecraft:gui", "minecraft:container/slot")
        .unwrap();
    assert_eq!((glyph.u0, glyph.v0), (own.u0, own.v0));
    // A size measurement and a mod's decoration sprite: the same lookup, by id.
    let measured = atlas.gui_rect("minecraft:container/slot").unwrap();
    assert_eq!((measured.w, measured.h), (2, 1));
    assert_eq!(
        atlas.gui_rect("demo:marker").unwrap().texels((w, h)),
        (2, 1)
    );
    std::fs::remove_dir_all(&root).unwrap();
}
/// A GUI alias of an animated strip shows the strip's first frame in the atlas and animates
/// through the strip's later ones, under its own id, with the pixels of each.
#[test]
fn a_gui_alias_animates_through_its_sources_frames() {
    let root = model_atlas_root("gui-alias-frames", r#"{"sources":[]}"#, r#"{"sources":[]}"#);
    write_vanilla_gui_definition(&root);
    std::fs::write(
        root.join("assets/demo/textures/item/sheet.png.mcmeta"),
        r#"{"animation":{"width":1,"frametime":1}}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("assets/minecraft/atlases/gui.json"),
        r#"{"sources":[{"type":"minecraft:single","resource":"demo:item/sheet","sprite":"demo:alias"}]}"#,
    )
    .unwrap();
    let atlas = HudAtlas::build(&ResourceSource::directory(root.clone())).unwrap();
    let rect = *atlas.gui_rect("demo:alias").unwrap();
    assert_eq!(
        texel_at(&atlas, rect, 0, 0),
        [255, 0, 0, 255],
        "the first frame"
    );
    let (_, animation) = atlas
        .sprite_animations
        .iter()
        .find(|(r, _)| r.u0 == rect.u0 && r.v0 == rect.v0)
        .expect("the alias animates");
    assert_eq!(animation.resized_pixels(0, 1, 1), [255, 0, 0, 255]);
    assert_eq!(
        animation.resized_pixels(1, 1, 1),
        [0, 255, 0, 0],
        "the strip's second frame"
    );
    std::fs::remove_dir_all(&root).unwrap();
}
#[test]
fn dumping_the_atlas_writes_its_pixels_and_sprite_rects() {
    let root = std::env::temp_dir().join("sunrise-atlas-dump-test");
    let _ = fs::remove_dir_all(&root);
    let pixels: Vec<u8> = (0..4u8).flat_map(|i| [i, 0, 0, 255]).collect();
    let rect = |u0: f32, v0: f32| Rect {
        u0,
        v0,
        u1: u0 + 0.5,
        v1: v0 + 0.5,
        w: 8,
        h: 8,
    };
    let sprites = HashMap::from([
        ("particle/smoke".to_string(), rect(0.5, 0.0)),
        ("gui/sprites/bars/experience".to_string(), rect(0.0, 0.5)),
    ]);
    let path = dump_atlas(
        &root,
        "minecraft_textures_atlas_gui",
        2,
        2,
        &pixels,
        &sprites,
    )
    .expect("the dump writes");
    assert_eq!(path, root.join("minecraft_textures_atlas_gui.png"));
    let written = fs::read(&path).expect("the PNG is there");
    let (width, height, rgba) = decode_bytes(&written).expect("the PNG decodes");
    assert_eq!((width, height), (2, 2));
    assert_eq!(rgba, pixels);
    // Sorted by sprite name, rects in texels, as `dumpSpriteNames` writes them.
    let mut expected = String::from("gui/sprites/bars/experience\tx=0\ty=1\tw=1\th=1\n");
    expected.push_str("particle/smoke\tx=1\ty=0\tw=1\th=1\n");
    assert_eq!(
        fs::read_to_string(root.join("minecraft_textures_atlas_gui.txt")).unwrap(),
        expected
    );
    // A build with no texels in hand writes no file at all.
    let empty = dump_atlas(&root, "empty", 2, 2, &[], &sprites);
    assert!(empty.is_err());
    assert!(!root.join("empty.png").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn image_formats_are_told_apart_by_their_leading_bytes() {
    let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0];
    assert!(matches!(ImageFormat::sniff(&png), Some(ImageFormat::Png)));
    // RIFF with a WEBP tag at byte 8; RIFF alone is not enough (WAV is
    // RIFF too).
    let mut webp = b"RIFF    WEBPVP8 ".to_vec();
    assert!(matches!(ImageFormat::sniff(&webp), Some(ImageFormat::WebP)));
    webp[8..12].copy_from_slice(b"WAVE");
    assert!(ImageFormat::sniff(&webp).is_none());
    assert!(matches!(
        ImageFormat::sniff(&[0xFF, 0xD8, 0xFF, 0xE0]),
        Some(ImageFormat::Jpeg)
    ));
    // Too short to be any of them, and plainly something else.
    assert!(ImageFormat::sniff(b"RIFF").is_none());
    assert!(ImageFormat::sniff(b"").is_none());
    assert!(ImageFormat::sniff(b"<html>").is_none());
}

#[test]
fn a_fetched_image_may_not_claim_more_than_4096_squared() {
    assert!(check_fetched_size(4096, 4096).is_ok());
    assert!(check_fetched_size(4097, 4096).is_err());
    assert!(check_fetched_size(16_383, 16_383).is_err());
}

/// A fetched PNG is refused by the size its own header claims, before the
/// reader makes the pixel buffer: a few hundred bytes of payload would
/// otherwise ask for 64 MiB or much more.
#[test]
fn a_fetched_png_is_refused_by_the_size_its_header_claims() {
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, 2, 2);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[7; 16]).unwrap();
        writer.finish().unwrap();
    }
    assert!(matches!(
        ImageFormat::sniff(&encoded),
        Some(ImageFormat::Png)
    ));
    assert_eq!(
        decode_image_bytes(&encoded).expect("a small PNG decodes").0,
        2
    );

    // The same image, its IHDR claiming one pixel row past the limit: the
    // CRC is ignored on purpose, so this stands in for a hostile header.
    let ihdr = encoded.windows(4).position(|w| w == b"IHDR").expect("IHDR");
    encoded[ihdr + 4..ihdr + 8].copy_from_slice(&4097u32.to_be_bytes());
    encoded[ihdr + 8..ihdr + 12].copy_from_slice(&4096u32.to_be_bytes());
    let refused = decode_image_bytes(&encoded).expect_err("the size is refused");
    assert!(
        refused.to_string().contains("too large to decode"),
        "{refused}"
    );
}

/// A pack PNG is refused by the side its header claims when no atlas would
/// take it: the reader sizes the whole pixel buffer from that header, and
/// its own limit covers one row.
#[test]
fn a_pack_png_claiming_a_side_no_atlas_takes_is_refused() {
    let png = |width: u32| {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, width, 1);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&vec![0; width as usize]).unwrap();
        writer.finish().unwrap();
        bytes
    };
    assert_eq!(decode_bytes(&png(32_768)).unwrap().0, 32_768);
    let refused = decode_bytes(&png(40_000)).map(|(width, height, _)| (width, height));
    assert!(
        refused.as_ref().is_err_and(|e| e.is::<OverSizeLimit>()),
        "{refused:?}"
    );
}

/// A header within the side limit can still claim gigabytes: 32768 x 32768
/// is refused by its pixel count before the reader sizes a buffer from it.
#[test]
fn a_pack_png_claiming_more_pixels_than_an_atlas_is_refused() {
    let mut header = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut header, 32_768, 32_768);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        // No pixels, only an empty IDAT: `read_info` reads up to the image data.
        writer.write_chunk(png::chunk::IDAT, &[]).unwrap();
    }
    let refused = decode_bytes(&header).map(|(width, height, _)| (width, height));
    assert!(
        refused.as_ref().is_err_and(|e| e.is::<OverSizeLimit>()),
        "{refused:?}"
    );
}

/// The decoder's buffer and the RGBA copy are each held to the byte budget before they are
/// made: a 64x64 grey PNG decodes to 4096 bytes and converts to 16384.
#[test]
fn a_pack_png_past_the_byte_budget_is_refused_before_its_buffers() {
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, 64, 64);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[7; 64 * 64]).unwrap();
    }
    let over = |budget| decode_bytes_within(&png, budget).is_err_and(|e| e.is::<OverSizeLimit>());
    assert_eq!(
        decode_bytes_within(&png, 64 * 64 * 4).unwrap().2.len(),
        64 * 64 * 4
    );
    assert!(over(64 * 64 * 4 - 1), "the RGBA copy");
    assert!(over(64 * 64 - 1), "the decoder's buffer");
}

/// A narrow frame widens to a square: its bytes are held to the budget before it is made, and
/// a size no address holds is refused rather than wrapped.
#[test]
fn a_strip_frame_made_square_is_held_to_the_budget() {
    let pixels = vec![9; 64 * 2 * 4];
    let over = |w, budget| {
        strip_frame(&pixels, 64, 0, 0, w, 1, w, budget).is_err_and(|e| e.is::<OverSizeLimit>())
    };
    assert_eq!(
        strip_frame(&pixels, 64, 0, 0, 64, 1, 64, 64 * 64 * 4)
            .unwrap()
            .len(),
        64 * 64 * 4
    );
    assert!(over(64, 64 * 64 * 4 - 1));
    assert!(over(u32::MAX, usize::MAX));
}

/// Only the network path reads WebP and JPEG: a resource pack's texture
/// stays PNG-only, as vanilla's `NativeImage.read` is.
#[test]
fn decode_bytes_stays_png_only() {
    let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10];
    assert!(decode_bytes(&jpeg).is_err());
    let webp = b"RIFF\x00\x00\x00\x00WEBPVP8 ";
    assert!(decode_bytes(webp).is_err());
}

#[test]
fn decode_bytes_ignores_crc_checksum_error() {
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, 2, 2);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&[
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
            ])
            .unwrap();
        writer.finish().unwrap();
    }
    let idat_pos = encoded
        .windows(4)
        .position(|w| w == b"IDAT")
        .expect("IDAT chunk");
    let len = u32::from_be_bytes([
        encoded[idat_pos - 4],
        encoded[idat_pos - 3],
        encoded[idat_pos - 2],
        encoded[idat_pos - 1],
    ]) as usize;
    let crc_pos = idat_pos + 4 + len;
    encoded[crc_pos] ^= 0xFF;

    let (w, h, rgba) = decode_bytes(&encoded).expect("should decode despite bad CRC");
    assert_eq!(w, 2);
    assert_eq!(h, 2);
    assert_eq!(rgba.len(), 16);
}

#[test]
fn decode_bytes_ignores_a_bad_ihdr_checksum() {
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, 2, 2);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[7; 16]).unwrap();
        writer.finish().unwrap();
    }
    // The IHDR chunk's CRC is the 4 bytes after its 13 data bytes.
    let ihdr = encoded.windows(4).position(|w| w == b"IHDR").unwrap();
    encoded[ihdr + 4 + 13] ^= 0xFF;
    let (w, h, rgba) = decode_bytes(&encoded).expect("a bad IHDR CRC decodes, as stb_image's");
    assert_eq!((w, h, rgba.len()), (2, 2, 16));
}

#[test]
fn clip_since_keeps_the_visible_part_of_a_straddling_triangle() {
    let mut batch = HudBatch::scratch([320, 240]);
    let start = batch.vertices.len();
    // A right triangle (a quad `a, b, c, c`) whose centre lies outside the rect.
    batch.quad(
        [[0.0, 0.0], [16.0, 0.0], [0.0, 16.0], [0.0, 16.0]],
        [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [0.0, 1.0]],
        u32::MAX,
    );
    batch.clip_since(start, 0.0, 0.0, 16.0, 4.0);
    assert!(!batch.vertices.is_empty());
    assert!(batch.vertices.iter().all(|v| v.pos[1] <= 4.0));
    let area: f32 = batch
        .vertices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| {
            ((t[1].pos[0] - t[0].pos[0]) * (t[2].pos[1] - t[0].pos[1])
                - (t[2].pos[0] - t[0].pos[0]) * (t[1].pos[1] - t[0].pos[1]))
                .abs()
                / 2.0
        })
        .sum();
    // Rows 0..4 of the triangle: 16 * 16 / 2 - 12 * 12 / 2.
    assert!((area - 56.0).abs() < 1e-3, "{area}");
}

#[test]
fn clip_since_moves_blend_ranges_with_their_vertices() {
    let mut batch = HudBatch::scratch([320, 240]);
    let start = batch.vertices.len();
    let tri = |batch: &mut HudBatch, y: f32| {
        batch.quad(
            [[0.0, y], [8.0, y], [0.0, y + 8.0], [0.0, y + 8.0]],
            [[0.0; 2]; 4],
            u32::MAX,
        );
    };
    tri(&mut batch, 100.0); // outside, dropped
    let foil = batch.vertices.len();
    tri(&mut batch, 0.0); // straddles the bottom edge
    batch.blend_since(foil, Blend::Additive);
    batch.clip_since(start, 0.0, 0.0, 16.0, 4.0);
    assert_eq!(
        batch.blends,
        [(0..batch.vertices.len() as u32, Blend::Additive)]
    );
}

#[test]
fn clip_since_keeps_the_depth_of_an_item_or_scene_face() {
    let mut batch = HudBatch::scratch([320, 240]);
    let start = batch.vertices.len();
    // Two axis-aligned model faces at different depths, as an item model or a
    // `draw_scene` picture leaves them after `depth_since`.
    let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    batch.quad_3d(
        [
            [0.0, 0.0, 1.0],
            [8.0, 0.0, 1.0],
            [8.0, 8.0, 1.0],
            [0.0, 8.0, 1.0],
        ],
        uv,
        u32::MAX,
    );
    batch.quad_3d(
        [
            [2.0, 2.0, 3.0],
            [6.0, 2.0, 3.0],
            [6.0, 6.0, 5.0],
            [2.0, 6.0, 5.0],
        ],
        uv,
        u32::MAX,
    );
    batch.depth_since(start, [0.0, 0.0, 320.0, 240.0]);
    let before = batch.vertices.clone();
    assert!(before.iter().any(|v| v.depth > 0.0));
    // A clip containing both faces leaves every attribute as it was.
    batch.clip_since(start, 0.0, 0.0, 16.0, 16.0);
    assert_eq!(batch.vertices.len(), before.len());
    for (v, w) in batch.vertices.iter().zip(&before) {
        assert_eq!((v.pos, v.color, v.text), (w.pos, w.color, w.text));
        let near = |a: f32, b: f32| (a - b).abs() < 1e-6;
        assert!(near(v.depth, w.depth), "depth {} vs {}", v.depth, w.depth);
        assert!(near(v.uv[0], w.uv[0]) && near(v.uv[1], w.uv[1]));
    }
    // A clip cutting the sloped face keeps its depth across the cut.
    batch.clip_since(start, 0.0, 0.0, 16.0, 4.0);
    let sloped: Vec<_> = batch.vertices[6..]
        .iter()
        .map(|v| (v.pos[1], v.depth))
        .collect();
    let at = |y: f32| before[6..].iter().find(|v| v.pos[1] == y).unwrap().depth;
    for (y, depth) in sloped {
        let want = at(2.0) + (at(6.0) - at(2.0)) * (y - 2.0) / 4.0;
        assert!((depth - want).abs() < 1e-6, "y {y}: {depth} vs {want}");
    }
}

/// Write a PNG `width x height` whose `frames` stacked rows are solid
/// colours, then its mcmeta beside it.
fn write_strip(path: &Path, width: u32, height: u32, frames: &[[u8; 4]], mcmeta: Option<&str>) {
    let file = std::io::BufWriter::new(fs::File::create(path).unwrap());
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for frame in frames {
        let rows = height / frames.len() as u32;
        pixels.extend((0..width * rows).flat_map(|_| *frame));
    }
    writer.write_image_data(&pixels).unwrap();
    writer.finish().unwrap();
    if let Some(mcmeta) = mcmeta {
        let meta = path.with_extension("png.mcmeta");
        fs::write(meta, mcmeta).unwrap();
    }
}
/// A resource root holding one animated block texture: a strip of two
/// 16x16 frames, red then blue, whose mcmeta starts on the blue one.
/// Each call gets its own folder: tests run side by side, and one adds
/// a strip of its own that another would otherwise find.
fn animated_root() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("sunrise-atlas-anim-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let block = root.join("assets/minecraft/textures/block");
    fs::create_dir_all(&block).unwrap();
    write_strip(
        &block.join("test_anim.png"),
        16,
        32,
        &[[255, 0, 0, 255], [0, 0, 255, 255]],
        Some(r#"{"animation": {"frames": [1, 0]}}"#),
    );
    root
}
fn texel_at(atlas: &HudAtlas, rect: Rect, x: u32, y: u32) -> [u8; 4] {
    let px = (rect.u0 * atlas.width as f32).round() as u32 + x;
    let py = (rect.v0 * atlas.height as f32).round() as u32 + y;
    let at = ((py * atlas.width + px) * 4) as usize;
    atlas.pixels[at..at + 4].try_into().unwrap()
}
#[test]
fn a_jar_builds_the_gui_atlas_fonts_cit_trims_and_shaders_without_unpacking() {
    use std::io::Write;
    let root = std::env::temp_dir().join(format!("sunrise-gui-archive-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let jar = root.join("gui.jar");
    let mut writer = zip::ZipWriter::new(fs::File::create(&jar).unwrap());
    let mut add = |name: &str, bytes: &[u8]| {
        writer
            .start_file(
                name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
        writer.write_all(bytes).unwrap();
    };
    for (name, bytes) in [
        ("sunrise-mod-content.json", br#"["demo"]"#.as_slice()),
        ("assets/minecraft/font/default.json", br#"{"providers":[{"type":"space","advances":{"A":7}}]}"#),
        ("assets/minecraft/atlases/items.json", br#"{"sources":[{"type":"single","resource":"demo:item/custom","sprite":"demo:renamed"}]}"#),
        ("assets/minecraft/optifine/cit/tool.properties", b"items=stick\ntexture=./custom\n"),
        ("data/demo/trim_pattern/nested/test.json", br#"{"asset_id":"demo:custom"}"#),
        ("data/demo/trim_material/blue.json", br#"{"palette_id":"demo:trim/blue"}"#),
        ("assets/demo/textures/trims/entity/humanoid/custom.png.mcmeta", br#"{"palette":{"base_palette":"demo:trim/base"}}"#),
        ("assets/minecraft/shaders/core/text.vsh", b"#version 330\n#moj_import <demo:gui.glsl>\nin vec3 Position;\nin vec4 Color;\nin vec2 UV0;\nout vec4 color;\nvoid main(){color=Color;gl_Position=vec4(Position*GUI_SCALE,1.0);}\n"),
        ("assets/minecraft/shaders/core/text.fsh", b"#version 330\nin vec4 color;\nout vec4 fragColor;\nvoid main(){fragColor=color;}\n"),
        ("assets/demo/shaders/include/gui.glsl", b"const float GUI_SCALE=1.0;\n"),
    ] {
        add(name, bytes);
    }
    for (name, w, h, color) in [
        (
            "assets/demo/textures/item/custom.png",
            4,
            4,
            [10, 20, 30, 255],
        ),
        (
            "assets/demo/textures/gui/widget.png",
            4,
            4,
            [20, 30, 40, 255],
        ),
        (
            "assets/demo/textures/entity/machine/cog.png",
            4,
            4,
            [30, 40, 50, 255],
        ),
        (
            "assets/minecraft/optifine/cit/custom.png",
            4,
            4,
            [40, 50, 60, 255],
        ),
        (
            "assets/demo/textures/trims/entity/humanoid/custom.png",
            128,
            64,
            [1, 2, 3, 255],
        ),
        (
            "assets/demo/textures/palettes/trim/base.png",
            1,
            1,
            [1, 2, 3, 255],
        ),
        (
            "assets/demo/textures/palettes/trim/blue.png",
            1,
            1,
            [9, 8, 7, 255],
        ),
    ] {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, w, h);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&color.repeat((w * h) as usize))
                .unwrap();
        }
        add(name, &bytes);
    }
    writer.finish().unwrap();
    let source = ResourceSource::archive(&jar).unwrap();
    let atlas = HudAtlas::build_over(&source, &ResourceSource::default()).unwrap();
    for (name, color) in [
        ("demo:renamed", [10, 20, 30, 255]),
        ("demo:gui/widget", [20, 30, 40, 255]),
        ("demo:entity/machine/cog", [30, 40, 50, 255]),
        (
            "sunrise_cit:minecraft/optifine/cit/custom",
            [40, 50, 60, 255],
        ),
    ] {
        assert_eq!(texel_at(&atlas, atlas.sprites[name], 0, 0), color, "{name}");
    }
    assert_eq!(atlas.font.glyph('A').advance, 7.0);
    assert_eq!(
        (
            atlas.sprites["sunrise/trim0"].w,
            atlas.sprites["sunrise/trim0"].h
        ),
        (128, 64)
    );
    assert!(
        atlas.text_shaders.gui.is_some(),
        "the archive shader and its include translate"
    );
    let registry = crate::trims::TrimRegistry::load(&source);
    let trim = crate::trims::TrimTexture::resolve(
        &registry,
        &[],
        "demo:blue",
        "demo:nested/test",
        "humanoid",
    )
    .unwrap();
    let (w, h, pixels) = trim.generate(&source).unwrap();
    assert_eq!((w, h), (128, 64));
    assert!(
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| *pixel == [9, 8, 7, 255])
    );
    assert!(!root.join("assets").exists());
    assert!(!root.join("data").exists());
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    drop((atlas, source));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn content_mod_entity_textures_pack_under_their_namespace() {
    let root = std::env::temp_dir().join(format!("sunrise-atlas-equip-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let equipment = root.join("assets/gems/textures/entity/equipment/humanoid");
    fs::create_dir_all(&equipment).unwrap();
    write_strip(
        &equipment.join("ruby.png"),
        64,
        32,
        &[[200, 0, 0, 255]],
        None,
    );
    let other = root.join("assets/gems/textures/item");
    fs::create_dir_all(&other).unwrap();
    write_strip(&other.join("ruby.png"), 16, 16, &[[0, 200, 0, 255]], None);
    fs::write(
        root.join(sunrise_items::content_namespaces::MARKER),
        br#"["gems"]"#,
    )
    .unwrap();
    // A block entity's texture, for a native mod's renderer.
    let machine = root.join("assets/gems/textures/entity/machine");
    fs::create_dir_all(&machine).unwrap();
    write_strip(&machine.join("cog.png"), 32, 32, &[[0, 0, 200, 255]], None);
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let rect = atlas.sprites["gems:entity/equipment/humanoid/ruby"];
    assert_eq!(texel_at(&atlas, rect, 0, 0), [200, 0, 0, 255]);
    let rect = atlas.sprites["gems:entity/machine/cog"];
    assert_eq!(rect.texels((atlas.width, atlas.height)), (32, 32));
    assert_eq!(texel_at(&atlas, rect, 0, 0), [0, 0, 200, 255]);
    // Only entity textures: the mods' others reach the atlas through atlas definitions.
    assert!(!atlas.sprites.contains_key("gems:item/ruby"));
    fs::remove_dir_all(&root).unwrap();
}
/// A pack's particle sprite of any size packs whole, as vanilla's particle atlas stitches
/// it (a light shaft drawn from a 1024-wide texture); the cap on GUI textures stays.
#[test]
fn large_particle_sprites_pack_and_large_gui_textures_dont() {
    let root =
        std::env::temp_dir().join(format!("sunrise-atlas-big-particle-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let rays = root.join("assets/mypack/textures/particle/light_rays");
    fs::create_dir_all(&rays).unwrap();
    write_strip(
        &rays.join("02.png"),
        1024,
        1024,
        &[[250, 240, 200, 255]],
        None,
    );
    let gui = root.join("assets/minecraft/textures/gui");
    fs::create_dir_all(&gui).unwrap();
    write_strip(&gui.join("huge.png"), 2048, 64, &[[1, 2, 3, 255]], None);
    let atlas = HudAtlas::build(&ResourceSource::directory(root.clone())).unwrap();
    let rect = atlas.sprites["mypack:particle/light_rays/02"];
    assert_eq!(rect.texels((atlas.width, atlas.height)), (1024, 1024));
    assert_eq!(texel_at(&atlas, rect, 512, 512), [250, 240, 200, 255]);
    assert!(!atlas.sprites.contains_key("gui/huge"));
    fs::remove_dir_all(&root).unwrap();
}
/// OptiFine random entity variants outside `textures/` pack under the names
/// `RandomTextures` gives them; one beside its texture packs once, under its own.
#[test]
fn random_entity_variants_pack_under_their_scan_names() {
    let root = std::env::temp_dir().join(format!("sunrise-atlas-random-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let random = root.join("assets/minecraft/optifine/random/entity/pig");
    fs::create_dir_all(&random).unwrap();
    write_strip(&random.join("pig2.png"), 64, 64, &[[0, 200, 0, 255]], None);
    let beside = root.join("assets/minecraft/textures/entity/cat");
    fs::create_dir_all(&beside).unwrap();
    fs::write(beside.join("cat.properties"), "textures.1=2").unwrap();
    write_strip(&beside.join("cat.png"), 64, 32, &[[1, 1, 1, 255]], None);
    write_strip(&beside.join("cat2.png"), 64, 32, &[[0, 0, 200, 255]], None);
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let rect = atlas.sprites["minecraft:optifine/random/entity/pig/pig2"];
    assert_eq!(texel_at(&atlas, rect, 0, 0), [0, 200, 0, 255]);
    let rect = atlas.sprites["entity/cat/cat2"];
    assert_eq!(texel_at(&atlas, rect, 0, 0), [0, 0, 200, 255]);
    assert!(atlas.random_textures.get("entity/pig/pig").is_some());
    fs::remove_dir_all(&root).unwrap();
}
/// A random variant's emissive overlay packs beside the variant, under its name.
#[test]
fn variant_overlays_pack_under_their_names() {
    let root = std::env::temp_dir().join(format!("sunrise-atlas-overlay-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let random = root.join("assets/minecraft/optifine/random/entity/zombie");
    fs::create_dir_all(&random).unwrap();
    write_strip(
        &random.join("zombie3.png"),
        64,
        64,
        &[[0, 200, 0, 255]],
        None,
    );
    write_strip(
        &random.join("zombie3_e.png"),
        64,
        64,
        &[[200, 0, 0, 255]],
        None,
    );
    // A variant's blink frame, with its own overlay.
    write_strip(
        &random.join("zombie3_blink.png"),
        64,
        64,
        &[[0, 0, 200, 255]],
        None,
    );
    write_strip(
        &random.join("zombie3_blink_e.png"),
        64,
        64,
        &[[9, 9, 9, 255]],
        None,
    );
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let rect = atlas.sprites["minecraft:optifine/random/entity/zombie/zombie3_blink_e"];
    assert_eq!(texel_at(&atlas, rect, 0, 0), [9, 9, 9, 255]);
    let rect = atlas.sprites["minecraft:optifine/random/entity/zombie/zombie3_e"];
    assert_eq!(texel_at(&atlas, rect, 0, 0), [200, 0, 0, 255]);
    assert_eq!(
        atlas
            .emissive
            .overlay("minecraft:optifine/random/entity/zombie/zombie3"),
        Some("minecraft:optifine/random/entity/zombie/zombie3_e")
    );
    fs::remove_dir_all(&root).unwrap();
}
/// Variants past the budget, in name order, stay out of the atlas.
#[test]
fn random_variants_stop_at_their_budget() {
    let dir = std::env::temp_dir().join(format!("sunrise-atlas-budget-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let source = ResourceSource::directory(dir.clone());
    let files: Vec<String> = ["a2.png", "a3.png", "a4.png"]
        .iter()
        .map(|n| (*n).to_owned())
        .collect();
    for f in &files {
        write_strip(&dir.join(f), 16, 16, &[[1, 2, 3, 255]], None);
    }
    let refs: Vec<&String> = files.iter().collect();
    // 256 texels each: 600 holds two.
    let kept = within_budget(&source, &refs, 600);
    assert_eq!(kept, files[..2].iter().cloned().collect());
    assert_eq!(within_budget(&source, &refs, 768).len(), 3);
    // a2's overlay sorts before a3 by name, but every variant comes first.
    let overlay = "a2_e.png".to_owned();
    write_strip(&dir.join(&overlay), 16, 16, &[[1, 2, 3, 255]], None);
    let overlays: HashSet<String> = [overlay.clone()].into();
    let all = [&files[0], &overlay, &files[1]];
    let kept = budgeted(&source, all.into_iter(), &overlays, 600);
    assert_eq!(kept, [files[0].clone(), files[1].clone()].into());
    fs::remove_dir_all(&dir).unwrap();
}
#[test]
fn content_mod_effect_icons_pack_under_their_namespace() {
    let root = std::env::temp_dir().join(format!("sunrise-atlas-effect-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let icons = root.join("assets/gems/textures/mob_effect");
    fs::create_dir_all(&icons).unwrap();
    write_strip(&icons.join("glow.png"), 18, 18, &[[0, 0, 200, 255]], None);
    fs::write(
        root.join(sunrise_items::content_namespaces::MARKER),
        br#"["gems"]"#,
    )
    .unwrap();
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let rect = atlas.sprites[sunrise_client_core::potions::sprite("gems:glow").as_str()];
    assert_eq!(texel_at(&atlas, rect, 0, 0), [0, 0, 200, 255]);
    // The HUD and the inventory draw it from the effect's id, as vanilla's.
    let effect = crate::effects::EffectInstance::new("gems:glow", 600, 0);
    let icon = crate::hud_overlays::EffectIcon::from(&effect);
    let draws = crate::hud_overlays::effects(320, false, &[icon]);
    let mut batch = HudBatch::scratch([320, 240]);
    crate::hud_layout::paint(&mut batch, &atlas, &draws);
    assert!(batch.vertices.iter().any(|v| v.uv == [rect.u0, rect.v0]));
    let panels = {
        let space = crate::inventory_effects::EffectsSpace {
            screen_width: 320,
            origin: [72, 37],
            image_width: 176,
        };
        let vanilla = crate::inventory_effects::EffectsLayout::vanilla(space.available());
        crate::inventory_effects::panels(
            &space,
            &vanilla,
            std::slice::from_ref(&effect),
            20.0,
            &|_| None,
            &|_| 60,
        )
    };
    batch.vertices.clear();
    crate::inventory_effects::paint(&mut batch, &atlas, &panels, [0, 0]);
    assert!(batch.vertices.iter().any(|v| v.uv == [rect.u0, rect.v0]));
    // An effect without an icon draws the missing sprite.
    let missing = atlas.sprites[MISSING_SPRITE];
    assert_eq!(texel_at(&atlas, missing, 0, 0), [0, 0, 0, 255]);
    assert_eq!(texel_at(&atlas, missing, 8, 0), [248, 0, 248, 255]);
    let effect = crate::effects::EffectInstance::new("gems:none", 600, 0);
    let draws = crate::hud_overlays::effects(320, false, &[(&effect).into()]);
    batch.vertices.clear();
    crate::hud_layout::paint(&mut batch, &atlas, &draws);
    assert!(
        batch
            .vertices
            .iter()
            .any(|v| v.uv == [missing.u0, missing.v0])
    );
    fs::remove_dir_all(&root).unwrap();
}
#[test]
fn animated_block_sprites_register_and_pack_their_first_mcmeta_frame() {
    let root = animated_root();
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let (rect, animation) = atlas
        .sprite_animations
        .iter()
        .find(|(_, animation)| animation.frame_count() == 2)
        .expect("the animated block sprite");
    // The mcmeta's list starts on the strip's second (blue) frame.
    assert_eq!(animation.frame(0), 1);
    assert_eq!(rect.texels((atlas.width, atlas.height)), (16, 16));
    assert_eq!(texel_at(&atlas, *rect, 0, 0), [0, 0, 255, 255]);
    assert_eq!(texel_at(&atlas, *rect, 15, 15), [0, 0, 255, 255]);
}
#[test]
fn packed_pixels_come_back_the_same() {
    // Compression needs pixel bytes, not a resource reload's map/skin slots,
    // font rasterization and full-size atlas. Cover both literal bytes and
    // backreferences: transparent runs, solid rows, repeated patterned rows
    // and varied RGBA rows, with a buffer just beyond the 64 KiB window.
    let mut atlas = HudAtlas::fonts(&ResourceSource::default()).unwrap();
    atlas.width = 128;
    atlas.height = 160;
    atlas.pixels = (0..atlas.width * atlas.height)
        .flat_map(|i| {
            let x = (i % atlas.width) as u8;
            match i / atlas.width {
                0..32 => [0; 4],
                32..64 => [40, 80, 120, 255],
                64..96 => [x, x.wrapping_mul(3), 255 - x, 128],
                _ => [i as u8, (i / 128) as u8, 255 - i as u8, i as u8],
            }
        })
        .collect();
    let built = atlas.pixels.clone();
    atlas.pack_pixels();
    assert!(atlas.pixels.is_empty());
    assert!(atlas.packed.as_ref().unwrap().wait().len() < built.len());
    assert!(atlas.pixels() == built.as_slice());
    // Packing again keeps the same image.
    atlas.pack_pixels();
    assert!(atlas.pixels() == built.as_slice());
}
#[test]
fn a_strip_frames_size_comes_from_its_mcmeta() {
    let root = animated_root();
    let block = root.join("assets/minecraft/textures/block");
    // A row of two frames with the animation's own frame size.
    write_strip(
        &block.join("test_row.png"),
        32,
        16,
        &[[0, 255, 0, 255], [255, 255, 0, 255]],
        Some(r#"{"animation": {"width": 16, "height": 16}}"#),
    );
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let rect = atlas.sprites["block/test_row"];
    // The first frame is the left 16x16, not the whole row.
    assert_eq!(rect.texels((atlas.width, atlas.height)), (16, 16));
    assert_eq!(texel_at(&atlas, rect, 0, 0), [0, 255, 0, 255]);
}
#[test]
fn a_pack_directory_outside_the_walked_folders_still_packs() {
    let root = std::env::temp_dir().join(format!("sunrise-atlas-custom-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let atlases = root.join("assets/minecraft/atlases");
    let custom = root.join("assets/minecraft/textures/custom");
    fs::create_dir_all(&atlases).unwrap();
    fs::create_dir_all(&custom).unwrap();
    fs::write(
        atlases.join("blocks.json"),
        r#"{"sources":[{"type":"directory","source":"custom","prefix":"custom/"}]}"#,
    )
    .unwrap();
    write_strip(
        &custom.join("test.png"),
        16,
        16,
        &[[40, 180, 80, 255]],
        None,
    );
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let rect = atlas.sprites["custom/test"];
    assert_eq!(rect.texels((atlas.width, atlas.height)), (16, 16));
    assert_eq!(texel_at(&atlas, rect, 0, 0), [40, 180, 80, 255]);
    let _ = fs::remove_dir_all(&root);
}
#[test]
fn another_namespaces_animated_item_texture_packs_its_first_frame_and_animates() {
    let root = std::env::temp_dir().join(format!("sunrise-atlas-ns-anim-{}", std::process::id()));
    let atlases = root.join("assets/minecraft/atlases");
    let item = root.join("assets/lobby/textures/item/npc");
    fs::create_dir_all(&atlases).unwrap();
    fs::create_dir_all(&item).unwrap();
    fs::write(
        atlases.join("items.json"),
        r#"{"sources": [{"type": "directory", "source": "item", "prefix": "item/"}]}"#,
    )
    .unwrap();
    // A server pack's NPC skin: two 16x16 frames, green then red.
    write_strip(
        &item.join("skin.png"),
        16,
        32,
        &[[0, 255, 0, 255], [255, 0, 0, 255]],
        Some(r#"{"animation": {"frametime": 3}}"#),
    );
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let rect = atlas.sprites["lobby:item/npc/skin"];
    // One frame, not the whole strip, which a model's UVs would span.
    assert_eq!(rect.texels((atlas.width, atlas.height)), (16, 16));
    assert_eq!(texel_at(&atlas, rect, 0, 15), [0, 255, 0, 255]);
    let (_, animation) = atlas
        .sprite_animations
        .iter()
        .find(|(r, _)| r.u0 == rect.u0 && r.v0 == rect.v0)
        .expect("the namespaced sprite animates");
    assert_eq!(animation.frame_count(), 2);
}
#[test]
fn an_animated_texture_outside_item_folders_packs_its_first_frame_and_animates() {
    let root = std::env::temp_dir().join(format!("sunrise-atlas-dir-anim-{}", std::process::id()));
    let atlases = root.join("assets/minecraft/atlases");
    let custom = root.join("assets/minecraft/textures/custom");
    fs::create_dir_all(&atlases).unwrap();
    fs::create_dir_all(&custom).unwrap();
    fs::write(
        atlases.join("items.json"),
        r#"{"sources": [{"type": "directory", "source": "custom", "prefix": "custom/"}]}"#,
    )
    .unwrap();
    // A server pack's item under its own folder: two 16x16 frames, blue then yellow, and
    // a still 16x32 texture beside it.
    write_strip(
        &custom.join("gem.png"),
        16,
        32,
        &[[0, 0, 255, 255], [255, 255, 0, 255]],
        Some(r#"{"animation": {"frametime": 20}}"#),
    );
    write_strip(&custom.join("banner.png"), 16, 32, &[[9, 9, 9, 255]], None);
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let _ = fs::remove_dir_all(&root);
    let rect = atlas.sprites["custom/gem"];
    // One frame, not the whole strip squeezed into a slot.
    assert_eq!(rect.texels((atlas.width, atlas.height)), (16, 16));
    assert_eq!(texel_at(&atlas, rect, 0, 15), [0, 0, 255, 255]);
    let (_, animation) = atlas
        .sprite_animations
        .iter()
        .find(|(r, _)| r.u0 == rect.u0 && r.v0 == rect.v0)
        .expect("the sprite animates");
    assert_eq!(animation.frame_count(), 2);
    // A still texture there stays whole, as vanilla stitches it.
    let banner = atlas.sprites["custom/banner"];
    assert_eq!(banner.texels((atlas.width, atlas.height)), (16, 32));
}
#[test]
fn an_inline_atlas_sprite_resolves_in_its_named_atlas() {
    let root = std::env::temp_dir().join(format!("sunrise-atlas-inline-{}", std::process::id()));
    let gui = root.join("assets/minecraft/textures/gui/sprites/boss_bar");
    fs::create_dir_all(&gui).unwrap();
    write_vanilla_gui_definition(&root);
    write_strip(
        &gui.join("pink_background.png"),
        16,
        16,
        &[[1, 2, 3, 255]],
        None,
    );
    let atlas = HudAtlas::build(&ResourceSource::directory(root.clone())).unwrap();
    let _ = fs::remove_dir_all(&root);
    let packed = atlas.sprites["gui/sprites/boss_bar/pink_background"];
    let missing = atlas.sprites[MISSING_SPRITE];
    let found = atlas
        .inline_sprite("minecraft:gui", "minecraft:boss_bar/pink_background")
        .unwrap();
    assert_eq!((found.u0, found.v0), (packed.u0, packed.v0));
    let none = atlas
        .inline_sprite("minecraft:gui", "minecraft:boss_bar/nope")
        .unwrap();
    assert_eq!((none.u0, none.v0), (missing.u0, missing.v0));
}
#[test]
fn a_root_single_sprite_strip_taller_than_the_atlas_packs_one_frame() {
    let root = animated_root();
    let atlases = root.join("assets/minecraft/atlases");
    fs::create_dir_all(&atlases).unwrap();
    fs::write(
        atlases.join("items.json"),
        r#"{"sources":[{"type":"single","resource":"nexo:abc123"},
            {"type":"single","resource":"nexo:still"}]}"#,
    )
    .unwrap();
    // Plugin-generated names need no item/ prefix. Keep the production
    // strip's height, but use narrow frames to keep this fixture cheap.
    let textures = root.join("assets/nexo/textures");
    fs::create_dir_all(&textures).unwrap();
    write_strip(
        &textures.join("abc123.png"),
        16,
        32768,
        &[[255, 0, 0, 255], [0, 0, 255, 255]],
        Some(r#"{"animation":{"frames":[1024,0],"frametime":3}}"#),
    );
    write_strip(&textures.join("still.png"), 16, 32, &[[7, 8, 9, 255]], None);
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.clone(),
    ))
    .unwrap();
    let _ = fs::remove_dir_all(&root);
    let rect = atlas.sprites["nexo:abc123"];
    assert_eq!(rect.texels((atlas.width, atlas.height)), (16, 16));
    assert_eq!(texel_at(&atlas, rect, 0, 15), [0, 0, 255, 255]);
    let (_, animation) = atlas
        .sprite_animations
        .iter()
        .find(|(r, _)| r.u0 == rect.u0 && r.v0 == rect.v0)
        .expect("the root-level single sprite animates");
    assert_eq!(animation.frame(0), 1024);
    assert_eq!(animation.frame(3), 0);
    assert_eq!(animation.frame_count(), 2048);
    assert_eq!(
        atlas.sprites["nexo:still"].texels((atlas.width, atlas.height)),
        (16, 32)
    );
}
#[test]
fn an_animated_gui_sprite_packs_its_first_frame_and_animates() {
    let root = std::env::temp_dir().join(format!("sunrise-atlas-gui-anim-{}", std::process::id()));
    let sprites = root.join("assets/minecraft/textures/gui/sprites/icon");
    fs::create_dir_all(&sprites).unwrap();
    // A GUI sprite like icon/music_notes: 4 frames stacked vertically (16x64),
    // with animation metadata, and a still 16x32 texture beside it.
    write_strip(
        &sprites.join("music_notes.png"),
        16,
        64,
        &[
            [10, 20, 30, 255],
            [40, 50, 60, 255],
            [70, 80, 90, 255],
            [100, 110, 120, 255],
        ],
        Some(r#"{"animation": {"frametime": 2}}"#),
    );
    write_strip(
        &sprites.join("static_badge.png"),
        16,
        32,
        &[[9, 9, 9, 255]],
        None,
    );
    let atlas = HudAtlas::build(&crate::resource_source::ResourceSource::directory(
        root.clone(),
    ))
    .unwrap();
    let _ = fs::remove_dir_all(&root);
    let rect = atlas.sprites["gui/sprites/icon/music_notes"];
    // One frame, not the whole 16x64 strip squeezed into a slot.
    assert_eq!(rect.texels((atlas.width, atlas.height)), (16, 16));
    assert_eq!(texel_at(&atlas, rect, 0, 15), [10, 20, 30, 255]);
    let (_, animation) = atlas
        .sprite_animations
        .iter()
        .find(|(r, _)| r.u0 == rect.u0 && r.v0 == rect.v0)
        .expect("the gui sprite animates");
    assert_eq!(animation.frame_count(), 4);
    // A still texture there stays whole.
    let static_badge = atlas.sprites["gui/sprites/icon/static_badge"];
    assert_eq!(static_badge.texels((atlas.width, atlas.height)), (16, 32));
}
#[test]
fn arbitrary_namespace_entity_textures_are_packed_without_content_marker() {
    let root = std::env::temp_dir().join(format!(
        "sunrise-atlas-entity-namespace-{}",
        std::process::id()
    ));
    let file = root.join("assets/example/textures/entity/villager/profession/nested/engineer.png");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    write_strip(&file, 16, 16, &[[12, 34, 56, 255]], None);
    let atlas = HudAtlas::build(&ResourceSource::directory(root.clone())).unwrap();
    let rect = atlas.sprites["example:entity/villager/profession/nested/engineer"];
    assert_eq!(texel_at(&atlas, rect, 0, 0), [12, 34, 56, 255]);
    fs::remove_dir_all(root).unwrap();
}

/// A resource root with one plain item texture and these CIT files (relative path, contents).
fn cit_root(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("sunrise-atlas-cit-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let item = root.join("assets/minecraft/textures/item");
    fs::create_dir_all(&item).unwrap();
    write_strip(&item.join("stick.png"), 16, 16, &[[90, 60, 30, 255]], None);
    let cit = root.join("assets/minecraft/optifine/cit");
    fs::create_dir_all(&cit).unwrap();
    for (rel, text) in files {
        fs::write(cit.join(rel), text).unwrap();
    }
    root
}
#[test]
fn a_cit_item_rules_texture_is_packed_under_its_id() {
    let root = cit_root("plain", &[("a.properties", "items=stick\ntexture=a")]);
    let cit = root.join("assets/minecraft/optifine/cit");
    write_strip(&cit.join("a.png"), 2, 2, &[[10, 200, 30, 255]], None);
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let _ = fs::remove_dir_all(&root);
    let rect = atlas.sprites["sunrise_cit:minecraft/optifine/cit/a"];
    assert_eq!(rect.texels((atlas.width, atlas.height)), (2, 2));
    assert_eq!(texel_at(&atlas, rect, 1, 1), [10, 200, 30, 255]);
}
#[test]
fn a_cit_models_relative_texture_is_packed() {
    let root = cit_root(
        "model",
        &[
            ("a.properties", "items=stick\nmodel=sword"),
            (
                "sword.json",
                r#"{"parent":"item/generated","textures":{"layer0":"./blade"}}"#,
            ),
        ],
    );
    let cit = root.join("assets/minecraft/optifine/cit");
    write_strip(&cit.join("blade.png"), 2, 2, &[[40, 50, 60, 255]], None);
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let _ = fs::remove_dir_all(&root);
    let rect = atlas.sprites["sunrise_cit:minecraft/optifine/cit/blade"];
    assert_eq!(texel_at(&atlas, rect, 1, 1), [40, 50, 60, 255]);
}
#[test]
fn a_still_cit_texture_is_packed_whole() {
    // Not cut to a square as an unmarked strip would be: armour layers are 2:1.
    let root = cit_root("still", &[("a.properties", "items=stick\ntexture=a")]);
    let cit = root.join("assets/minecraft/optifine/cit");
    write_strip(&cit.join("a.png"), 4, 2, &[[7, 8, 9, 255]], None);
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let _ = fs::remove_dir_all(&root);
    let rect = atlas.sprites["sunrise_cit:minecraft/optifine/cit/a"];
    assert_eq!(rect.texels((atlas.width, atlas.height)), (4, 2));
}
#[test]
fn an_animated_cit_texture_packs_one_frame_and_animates() {
    let root = cit_root("anim", &[("a.properties", "items=stick\ntexture=a")]);
    let cit = root.join("assets/minecraft/optifine/cit");
    write_strip(
        &cit.join("a.png"),
        16,
        48,
        &[[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]],
        Some(r#"{"animation": {"frametime": 2}}"#),
    );
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let _ = fs::remove_dir_all(&root);
    let rect = atlas.sprites["sunrise_cit:minecraft/optifine/cit/a"];
    assert_eq!(rect.texels((atlas.width, atlas.height)), (16, 16));
    assert_eq!(texel_at(&atlas, rect, 0, 0), [255, 0, 0, 255]);
    let animations: Vec<_> = atlas
        .sprite_animations
        .iter()
        .filter(|(r, _)| r.u0 == rect.u0 && r.v0 == rect.v0)
        .collect();
    assert_eq!(animations.len(), 1);
    assert_eq!(animations[0].1.frame_count(), 3);
}
#[test]
fn an_armor_rules_texture_is_packed_too() {
    let root = cit_root(
        "armor",
        &[(
            "a.properties",
            "type=armor\nitems=iron_chestplate\ntexture.iron_layer_1=a",
        )],
    );
    let cit = root.join("assets/minecraft/optifine/cit");
    write_strip(&cit.join("a.png"), 64, 32, &[[1, 2, 3, 255]], None);
    let rules = crate::cit::load_rules(&ResourceSource::directory(root.clone()));
    assert_eq!(rules.len(), 1);
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let _ = fs::remove_dir_all(&root);
    // Worn armour draws from the same atlas, under the name the layer looks up.
    let rect = atlas.sprites["sunrise_cit:minecraft/optifine/cit/a"];
    assert_eq!(rect.texels((atlas.width, atlas.height)), (64, 32));
    let layer = crate::cit_armor::layer_sprite(&rules[0], "minecraft:iron", false, false);
    assert!(atlas.sprites.contains_key(&layer.unwrap()));
}
#[test]
fn named_cit_textures_pack_once_and_bad_ones_are_skipped() {
    let root = cit_root(
        "mixed",
        &[
            // Vanilla's own texture: already packed, under its usual name.
            (
                "v.properties",
                "items=stick
texture=item/stick",
            ),
            (
                "sub.properties",
                "items=bow
texture.bow_pulling_0=b",
            ),
            (
                "broken.properties",
                "items=apple
texture=broken",
            ),
            (
                "wide.properties",
                "items=egg
texture=wide",
            ),
        ],
    );
    let cit = root.join("assets/minecraft/optifine/cit");
    write_strip(&cit.join("b.png"), 2, 2, &[[5, 6, 7, 255]], None);
    fs::write(cit.join("broken.png"), b"not a png").unwrap();
    // A valid still image past both the 1024-wide and 512-high limits;
    // one pixel beyond each boundary is enough to exercise refusal.
    write_strip(&cit.join("wide.png"), 1025, 513, &[[1, 1, 1, 255]], None);
    // Vanilla's stick is already packed when the CIT pass runs.
    let stick = ("item/stick".to_owned(), 16, 16, vec![0; 16 * 16 * 4]);
    let added = cit_sprites(&ResourceSource::directory(root.clone()), &[stick]);
    let names: Vec<_> = added.iter().map(|(image, _, _)| image.0.as_str()).collect();
    assert_eq!(names, ["sunrise_cit:minecraft/optifine/cit/b"]);
    // And the atlas still builds around the broken and the too-wide texture.
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let _ = fs::remove_dir_all(&root);
    assert!(
        atlas
            .sprites
            .contains_key("sunrise_cit:minecraft/optifine/cit/b")
    );
    assert!(atlas.sprites.contains_key("item/stick"));
}
#[test]
fn one_oversized_texture_leaves_the_others_in() {
    // `a` sorts first and claims more than any atlas; `b` is small and must still pack.
    let root = cit_root(
        "oversized",
        &[
            ("a.properties", "items=stick\ntexture=a"),
            ("b.properties", "items=apple\ntexture=b"),
        ],
    );
    let cit = root.join("assets/minecraft/optifine/cit");
    // Only the header is read for `a`: a PNG signature and an IHDR of 20000x20000.
    let mut header = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    header.extend(20000u32.to_be_bytes());
    header.extend(20000u32.to_be_bytes());
    fs::write(cit.join("a.png"), header).unwrap();
    write_strip(&cit.join("b.png"), 2, 2, &[[4, 5, 6, 255]], None);
    let added = cit_sprites(&ResourceSource::directory(root.clone()), &[]);
    let _ = fs::remove_dir_all(&root);
    let names: Vec<_> = added.iter().map(|(i, _, _)| i.0.as_str()).collect();
    assert_eq!(names, ["sunrise_cit:minecraft/optifine/cit/b"]);
}
#[test]
fn a_wide_grid_animation_packs_its_frame() {
    // 1104 wide is past the limit for a still texture, not for a row of 16x16 frames.
    let root = cit_root("grid", &[("a.properties", "items=stick\ntexture=a")]);
    let cit = root.join("assets/minecraft/optifine/cit");
    write_strip(&cit.join("a.png"), 1104, 16, &[[9, 9, 9, 255]], None);
    let mcmeta = r#"{"animation":{"width":16,"height":16}}"#;
    fs::write(cit.join("a.png.mcmeta"), mcmeta).unwrap();
    let added = cit_sprites(&ResourceSource::directory(root.clone()), &[]);
    let _ = fs::remove_dir_all(&root);
    let sizes: Vec<_> = added.iter().map(|(i, _, _)| (i.1, i.2)).collect();
    assert_eq!(sizes, [(16, 16)]);
}
#[test]
fn too_many_custom_item_textures_leave_them_out() {
    // A 1024 atlas holds the stick and the glyph rows, not an image 1100 wide.
    let image = |name: &str, side: u32| {
        let pixels = vec![0; (side * side * 4) as usize];
        (name.to_owned(), side, side, pixels)
    };
    let mut images = vec![image("item/stick", 16), image("sunrise_cit:big", 1100)];
    let mut animated = vec!["sunrise_cit:big".to_owned()];
    let extras = HashSet::from(["sunrise_cit:big".to_owned()]);
    assert!(HudAtlas::layout(&images, 1024).is_err());
    HudAtlas::layout_leaving_out(&mut images, &mut animated, &extras, 1024).unwrap();
    let names: Vec<_> = images.iter().map(|i| i.0.as_str()).collect();
    assert_eq!(names, ["item/stick"]);
    assert!(animated.is_empty());
    // Without extras to leave out, it fails as before.
    let mut alone = vec![image("item/big", 1100)];
    let none = HashSet::new();
    assert!(HudAtlas::layout_leaving_out(&mut alone, &mut animated, &none, 1024).is_err());
}
#[test]
fn a_root_without_cit_rules_packs_the_same_sprites() {
    let root = cit_root("none", &[("a.properties", "items=stick\ntexture=a")]);
    let cit = root.join("assets/minecraft/optifine/cit");
    write_strip(&cit.join("a.png"), 2, 2, &[[10, 200, 30, 255]], None);
    let with: HashSet<String> = HudAtlas::build(
        &sunrise_assets::resource_source::ResourceSource::directory(root.to_path_buf()),
    )
    .unwrap()
    .sprites
    .into_keys()
    .collect();
    fs::remove_dir_all(root.join("assets/minecraft/optifine")).unwrap();
    let without: HashSet<String> = HudAtlas::build(
        &sunrise_assets::resource_source::ResourceSource::directory(root.to_path_buf()),
    )
    .unwrap()
    .sprites
    .into_keys()
    .collect();
    let _ = fs::remove_dir_all(&root);
    assert!(without.is_subset(&with));
    let added: Vec<_> = with.difference(&without).collect();
    assert_eq!(added, ["sunrise_cit:minecraft/optifine/cit/a"]);
    assert!(without.contains("item/stick"));
}
/// `count` images of `w x h`; layout reads only their sizes.
fn sized(count: usize, w: u32, h: u32) -> Vec<PackedImage> {
    (0..count)
        .map(|i| (format!("i{i}"), w, h, Vec::new()))
        .collect()
}
fn assert_inside(images: &[PackedImage], width: u32, placements: &[(u32, u32)], free: u32) {
    for ((_, w, h, _), (x, y)) in images.iter().zip(placements) {
        assert!(
            x + w <= width && y + h < free,
            "{x},{y} {w}x{h} outside {width}x{free}"
        );
    }
}
fn write_png(root: &Path, name: &str, w: u32, h: u32) {
    let path = root.join(format!("assets/minecraft/textures/{name}.png"));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut encoder = png::Encoder::new(fs::File::create(path).unwrap(), w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let pixels = vec![255; (w * h * 4) as usize];
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&pixels)
        .unwrap();
}
#[test]
fn a_pack_texture_keeps_the_default_textures_gui_size() {
    let temp = std::env::temp_dir().join(format!("sunrise-gui-size-{}", std::process::id()));
    let (defaults, pack) = (temp.join("defaults"), temp.join("pack"));
    write_png(&defaults, "gui/container/inventory", 256, 256);
    write_png(&defaults, "gui/sprites/widget/button", 200, 20);
    write_png(&defaults, "block/stone", 16, 16);
    write_png(&defaults, "item/compass", 16, 512);
    // Faithful 64x's inventory (past the 512 rows a texture may have),
    // 32x's button and stone, an animated strip, and a texture of its own.
    write_png(&pack, "gui/container/inventory", 1024, 1024);
    write_png(&pack, "gui/sprites/widget/button", 400, 40);
    write_png(&pack, "block/stone", 32, 32);
    write_png(&pack, "item/compass", 32, 1024);
    write_png(&pack, "gui/sprites/pack_only", 30, 10);
    let atlas = HudAtlas::build_over(
        &sunrise_assets::resource_source::ResourceSource::directory(pack.to_path_buf()),
        &sunrise_assets::resource_source::ResourceSource::directory(defaults.to_path_buf()),
    )
    .unwrap();
    let _ = fs::remove_dir_all(&temp);
    let size = (atlas.width, atlas.height);
    for (name, gui, texels) in [
        ("gui/container/inventory", (256, 256), (1024, 1024)),
        ("gui/sprites/widget/button", (200, 20), (400, 40)),
        ("block/stone", (16, 16), (32, 32)),
        ("item/compass", (16, 16), (32, 32)),
        ("gui/sprites/pack_only", (30, 10), (30, 10)),
    ] {
        let r = atlas.sprites[name];
        assert_eq!(((r.w, r.h), r.texels(size)), (gui, texels), "{name}");
    }
}
/// A bitmap font's `file` may be any texture, not only one under
/// `textures/font`; the atlas packs it so the font can read its glyphs
/// from there instead of copying them into the run-time region.
#[test]
fn a_bitmap_font_sheet_outside_font_is_packed() {
    let root = std::env::temp_dir().join(format!("sunrise-font-pack-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write_png(&root, "hud/sheet", 16, 8);
    let def = root.join("assets/minecraft/font/default.json");
    fs::create_dir_all(def.parent().unwrap()).unwrap();
    fs::write(
        &def,
        r#"{"providers":[{"type":"bitmap","file":"minecraft:hud/sheet.png","ascent":7,"chars":["ab"]}]}"#,
    )
    .unwrap();
    let atlas = HudAtlas::build(&ResourceSource::directory(root.clone())).unwrap();
    let _ = fs::remove_dir_all(&root);
    let rect = atlas
        .sprites
        .get("hud/sheet")
        .expect("the font sheet is packed");
    assert_eq!(rect.texels((atlas.width, atlas.height)), (16, 8));
}

/// A content mod's GUI textures join the atlas under its namespace;
/// its other textures and vanilla's names stay as they were.
#[test]
fn a_content_mods_gui_textures_are_sprites_under_its_namespace() {
    let root = std::env::temp_dir().join(format!("sunrise-mod-gui-{}", std::process::id()));
    write_png(&root, "gui/container/furnace", 256, 256);
    let png = |rel: &str, w: u32, h: u32| {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut encoder = png::Encoder::new(fs::File::create(path).unwrap(), w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let pixels = vec![255; (w * h * 4) as usize];
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
    };
    png("assets/mymod/textures/gui/container/machine.png", 176, 166);
    png("assets/mymod/textures/block/machine.png", 16, 16);
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let _ = fs::remove_dir_all(&root);
    assert_eq!(atlas.sprites["mymod:gui/container/machine"].w, 176);
    assert!(atlas.sprites.contains_key("gui/container/furnace"));
    assert!(!atlas.sprites.contains_key("mymod:block/machine"));
}
#[test]
fn the_default_textures_keep_a_4096_wide_atlas() {
    let images = sized(400, 64, 64);
    let (width, placements, free) = HudAtlas::layout(&images, MAX_ATLAS_SIZE).unwrap();
    assert_eq!(width, 4096);
    assert_inside(&images, width, &placements, free);
}
#[test]
fn a_high_resolution_pack_widens_the_atlas_to_fit_the_gpu() {
    // 34 rows of three 1024x512 GUI textures: 17442 rows at 4096 wide,
    // past the 16384 GPUs allow (Faithful 64x needed 32768).
    let images = sized(100, 1024, 512);
    let (width, placements, free) = HudAtlas::layout(&images, MAX_ATLAS_SIZE).unwrap();
    assert_eq!(width, 8192);
    assert!(free + GLYPH_ROWS <= MAX_ATLAS_SIZE);
    assert_inside(&images, width, &placements, free);
}
#[test]
fn mixed_sizes_fill_the_ends_of_the_shelves() {
    // 15 images 9000 wide leave 7383 of a 16384 shelf unused each; the
    // 4000-wide ones (same height) come after them, and on shelves of their
    // own (4 to a shelf) the atlas is 19 shelves tall: past 16384 rows.
    // Each in a gap, it is 15.
    let mut images = sized(15, 9000, 1000);
    images.extend(sized(15, 4000, 1000));
    let (width, placements, free) = HudAtlas::layout(&images, 16384).unwrap();
    assert_eq!(width, 16384);
    assert!(free + GLYPH_ROWS <= 16384);
    assert_inside(&images, width, &placements, free);
}
#[test]
fn model_resolution_fallback_protects_font_sources_and_gui_aliases() {
    let root = animated_root();
    write_png(&root, "item/font_sheet", 32, 32);
    write_png(&root, "gui/panel", 32, 32);
    write_png(&root, "hash", 32, 32);
    let font = root.join("assets/minecraft/font/default.json");
    fs::create_dir_all(font.parent().unwrap()).unwrap();
    fs::write(
        font,
        r#"{"providers":[{"type":"bitmap","file":"minecraft:item/font_sheet.png","ascent":7,"chars":["a"]}]}"#,
    ).unwrap();
    let atlases = root.join("assets/minecraft/atlases");
    fs::create_dir_all(&atlases).unwrap();
    fs::write(
        atlases.join("items.json"),
        r#"{"sources":[
            {"type":"single","resource":"item/font_sheet","sprite":"font_alias"},
            {"type":"single","resource":"gui/panel","sprite":"gui_alias"},
            {"type":"single","resource":"hash"}
        ]}"#,
    )
    .unwrap();
    let images = vec![
        ("item/font_sheet".to_owned(), 32, 32, Vec::new()),
        ("item/blurred".to_owned(), 32, 32, Vec::new()),
        ("block/stone".to_owned(), 32, 32, Vec::new()),
    ];
    let models = reducible_model_sprites(
        &ResourceSource::directory(root.clone()),
        &images,
        &HashSet::from(["item/blurred".to_owned()]),
    );
    let _ = fs::remove_dir_all(root);
    assert_eq!(
        models,
        HashSet::from(["hash".to_owned(), "block/stone".to_owned()])
    );
}
#[test]
fn overflowing_model_textures_shrink_without_changing_gui_sizes_or_slots() {
    let mut images = vec![
        (
            "nexo:hash".into(),
            1024,
            1024,
            [10, 20, 30, 255].repeat(1024 * 1024),
        ),
        (
            "gui/panel".into(),
            32,
            32,
            [40, 50, 60, 255].repeat(32 * 32),
        ),
        ("sunrise/map".into(), 128, 128, Vec::new()),
    ];
    let models = HashSet::from(["nexo:hash".to_owned(), "sunrise/map".to_owned()]);
    let mut sizes = HashMap::new();
    assert!(HudAtlas::layout(&images, 1024).is_err());
    let (width, placements, free) =
        HudAtlas::layout_reducing_models(&mut images, &mut sizes, &models, 1024).unwrap();
    assert_inside(&images, width, &placements, free);
    assert!(free + GLYPH_ROWS <= 1024);
    let model = images.iter().find(|i| i.0 == "nexo:hash").unwrap();
    assert_eq!((model.1, model.2), (256, 256));
    assert_eq!(model.3, [10, 20, 30, 255].repeat(256 * 256));
    assert_eq!(sizes["nexo:hash"], (1024, 1024));
    let gui = images.iter().find(|i| i.0 == "gui/panel").unwrap();
    assert_eq!((gui.1, gui.2), (32, 32));
    assert_eq!(gui.3, [40, 50, 60, 255].repeat(32 * 32));
    let slot = images.iter().find(|i| i.0 == "sunrise/map").unwrap();
    assert_eq!((slot.1, slot.2), (128, 128));
    assert!(slot.3.is_empty());
}
#[test]
fn fitting_models_keep_their_pixels_and_existing_logical_sizes() {
    let mut images = vec![("item/gem".into(), 32, 32, [1, 2, 3, 255].repeat(32 * 32))];
    let original = images.clone();
    let mut sizes = HashMap::from([("item/gem".to_owned(), (16, 16))]);
    let models = HashSet::from(["item/gem".to_owned()]);
    HudAtlas::layout_reducing_models(&mut images, &mut sizes, &models, 1024).unwrap();
    assert_eq!(images, original);
    assert_eq!(sizes["item/gem"], (16, 16));
    // Protected oversized images remain an error; no unbounded retry or
    // silent removal of GUI/font resources when shrinking cannot help.
    images.push(("font/sheet".into(), 1024, 1024, Vec::new()));
    assert!(HudAtlas::layout_reducing_models(&mut images, &mut sizes, &models, 1024).is_err());
    assert_eq!((images[0].1, images[0].2), (1024, 1024));
    assert_eq!(sizes["item/gem"], (16, 16));
}
#[test]
fn textures_that_cannot_fit_are_an_error_not_a_gpu_panic() {
    let error = HudAtlas::layout(&sized(300, 1024, 1024), MAX_ATLAS_SIZE).unwrap_err();
    assert!(error.to_string().contains("do not fit a 16384x16384 atlas"));
    // One image wider than the largest atlas.
    assert!(HudAtlas::layout(&sized(1, 2048, 16), 2048).is_err());
}

#[test]
fn a_wrapped_border_repeats_the_opposite_edges() {
    // 2x2 texels a b / c d, one byte each in red.
    let image = (
        "x".to_owned(),
        2,
        2,
        [1u8, 2, 3, 4]
            .iter()
            .flat_map(|&v| [v, 0, 0, 255])
            .collect(),
    );
    let (_, w, h, data) = wrap_border(&image);
    assert_eq!((w, h), (4, 4));
    let red: Vec<u8> = data.chunks(4).map(|t| t[0]).collect();
    #[rustfmt::skip]
    assert_eq!(red, [
        4, 3, 4, 3,
        2, 1, 2, 1,
        4, 3, 4, 3,
        2, 1, 2, 1,
    ]);
}

/// A synthetic default font: two 8x8 glyphs, 3 and 5 pixels wide.
fn two_glyph_font(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("sunrise-fonts-{name}-{}", std::process::id()));
    let font = root.join("assets/minecraft/font");
    let textures = root.join("assets/minecraft/textures/font");
    fs::create_dir_all(&font).unwrap();
    fs::create_dir_all(&textures).unwrap();
    fs::write(
        font.join("default.json"),
        r#"{"providers":[{"type":"bitmap","file":"minecraft:font/test.png","ascent":7,"chars":["ab"]}]}"#,
    )
    .unwrap();
    let mut pixels = vec![0u8; 16 * 8 * 4];
    for y in 0..8 {
        for x in (0..3).chain(8..13) {
            pixels[(y * 16 + x) * 4 + 3] = 255;
        }
    }
    let mut encoder =
        png::Encoder::new(fs::File::create(textures.join("test.png")).unwrap(), 16, 8);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&pixels)
        .unwrap();
    root
}
/// A pack layer that adds a font provider over the game's own resources:
/// its `font/default.json` names `minecraft:font/test.png`, which only the
/// game's root holds (`HudAtlas::build_over`'s `defaults`).
fn pack_font_over_defaults(name: &str) -> (PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!(
        "sunrise-font-fallback-{name}-{}",
        std::process::id()
    ));
    let pack = base.join("pack");
    let vanilla = base.join("vanilla");
    fs::create_dir_all(pack.join("assets/minecraft/font")).unwrap();
    fs::create_dir_all(vanilla.join("assets/minecraft/textures/font")).unwrap();
    fs::write(
        pack.join("assets/minecraft/font/default.json"),
        r#"{"providers":[{"type":"bitmap","file":"minecraft:font/test.png","ascent":7,"chars":["ab"]}]}"#,
    )
    .unwrap();
    let mut pixels = vec![0u8; 16 * 8 * 4];
    for y in 0..8 {
        for x in (0..3).chain(8..13) {
            pixels[(y * 16 + x) * 4 + 3] = 255;
        }
    }
    let mut encoder = png::Encoder::new(
        fs::File::create(vanilla.join("assets/minecraft/textures/font/test.png")).unwrap(),
        16,
        8,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&pixels)
        .unwrap();
    (pack, vanilla)
}
#[test]
fn a_pack_overlay_reads_font_sheets_from_the_game_below_it() {
    let (pack, vanilla) = pack_font_over_defaults("sheets");
    let atlas = HudAtlas::build_over(
        &ResourceSource::directory(pack.clone()),
        &ResourceSource::directory(vanilla.clone()),
    )
    .unwrap();
    let glyph = atlas.font.glyph('a');
    let _ = fs::remove_dir_all(pack.parent().unwrap());
    assert!(glyph.rect.is_some(), "text draws, not the missing glyph");
    assert!(glyph.advance > 0.0);
}
#[test]
fn the_fonts_only_atlas_measures_text_like_the_full_one() {
    let root = two_glyph_font("atlas");
    let full = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let fonts = HudAtlas::fonts(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let _ = fs::remove_dir_all(&root);
    assert!(fonts.pixels.is_empty());
    let width = full.font.width("abba");
    assert!(width > 0.0);
    assert_eq!(fonts.font.width("abba"), width);
}
#[test]
fn outlined_world_text_keeps_font_metrics_and_overrides_outline_color() {
    let root = two_glyph_font("world-outline");
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let _ = fs::remove_dir_all(&root);
    let mut batch = HudBatch::scratch([1, 1]);
    let text = crate::text::Text::colored("ab", 0xff0000);
    let outline = rgba(1, 2, 3, 128);
    let (advance, front) =
        batch.text_styled_outline_world(&atlas, &text, 10.0, 20.0, u32::MAX, outline);
    assert_eq!((advance, front, batch.vertices.len()), (10.0, 96, 108));
    assert!(batch.vertices[..front].iter().all(|v| v.color == outline));
    assert!(
        batch.vertices[front..]
            .iter()
            .all(|v| v.color == rgba(255, 0, 0, 255))
    );
    // Eight independently offset silhouettes, retaining both glyphs and
    // the bitmap font's advance (a=4, b=6); front text has no shadow.
    for (i, [dx, dy]) in [
        [-1.0, -1.0],
        [-1.0, 0.0],
        [-1.0, 1.0],
        [0.0, -1.0],
        [0.0, 1.0],
        [1.0, -1.0],
        [1.0, 0.0],
        [1.0, 1.0],
    ]
    .into_iter()
    .enumerate()
    {
        let at = batch.vertices[i * 12].pos;
        let main = batch.vertices[front].pos;
        assert_eq!(at, [main[0] + dx, main[1] + dy]);
        assert_eq!(batch.vertices[i * 12 + 6].pos[0] - at[0], 4.0);
    }
}
#[test]
fn obfuscated_glyphs_keep_the_width() {
    let root = two_glyph_font("obfuscated");
    let fonts = HudAtlas::fonts(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let _ = fs::remove_dir_all(&root);
    let (a, b) = (fonts.font.glyph('a').advance, fonts.font.glyph('b').advance);
    assert_ne!(a, b);
    for _ in 0..20 {
        assert_eq!(fonts.font.random_glyph(&Default::default(), a).advance, a);
        assert_eq!(fonts.font.random_glyph(&Default::default(), b).advance, b);
    }
    // No glyph that wide: FontSet's missing glyph.
    let missing = fonts.font.random_glyph(&Default::default(), 30.0);
    assert!(missing.rect.is_none() && missing.right == 5.0);
}

#[test]
fn component_shadow_color_overrides_text_call_shadow() {
    let root = two_glyph_font("component-shadow");
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let mut batch = HudBatch::scratch([320, 240]);
    let text = crate::text::Text::styled(
        "a",
        crate::text::Style {
            shadow_color: Some(0x8040_2010),
            ..Default::default()
        },
    );
    batch.text_styled(&atlas, &text, 10.0, 10.0, u32::MAX, false);
    assert_eq!(batch.vertices.len(), 12);
    assert_eq!(batch.vertices[0].color, rgba(0x40, 0x20, 0x10, 0x80));
    batch.vertices.clear();
    batch.text_styled_world(&atlas, &text, 10.0, 10.0, u32::MAX, false);
    assert_eq!(batch.vertices.len(), 12);
    assert_eq!(batch.vertices[0].depth, 0.0);
    assert_eq!(batch.vertices[6].depth, 0.03);
    batch.vertices.clear();
    let no_shadow = crate::text::Text::styled(
        "a",
        crate::text::Style {
            shadow_color: Some(0),
            ..Default::default()
        },
    );
    batch.text_styled(&atlas, &no_shadow, 10.0, 10.0, u32::MAX, true);
    assert_eq!(batch.vertices.len(), 6);
    batch.vertices.clear();
    let gradient = crate::component::to_text(
        &serde_json::json!({
            "text": "",
            "extra": [
                {"text": "a", "color": "#ff0000"},
                {"text": "b", "color": "#0000ff"}
            ]
        }),
        &mut |_| crate::text::Text::empty(),
    );
    batch.text_styled(&atlas, &gradient, 10.0, 10.0, u32::MAX, false);
    assert_eq!(batch.vertices[0].color, rgba(255, 0, 0, 255));
    assert_eq!(batch.vertices[6].color, rgba(0, 0, 255, 255));
}

#[test]
fn player_object_draws_an_inline_head() {
    let root = two_glyph_font("inline-head");
    write_png(&root, "entity/player/slim/alex", 64, 64);
    write_png(&root, "entity/player/wide/steve", 64, 64);
    let mut atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let text = crate::component::to_text(
        &serde_json::json!({"object":"player","player":{"id":"00000000-0000-0000-0000-000000000000"}}),
        &mut |_| crate::text::Text::empty(),
    );
    let mut batch = HudBatch::scratch([320, 240]);
    assert_eq!(
        batch.text_styled(&atlas, &text, 10.0, 10.0, u32::MAX, false),
        8.0
    );
    assert_eq!(batch.vertices.len(), 12); // face and hat, six vertices each
    assert_eq!(batch.inline_profiles.len(), 1);
    atlas.set_object_skin("name:elyapluggin".into(), "entity/player/wide/steve".into());
    let named = crate::component::to_text(
        &serde_json::json!({"object":"player","player":{"name":"elyapluggin"}}),
        &mut |_| crate::text::Text::empty(),
    );
    batch.vertices.clear();
    batch.text_styled(&atlas, &named, 10.0, 10.0, u32::MAX, false);
    let skin = atlas.sprites["entity/player/wide/steve"];
    assert_eq!(batch.vertices[0].uv[0], skin.u0 + (skin.u1 - skin.u0) / 8.0);
    batch.vertices.clear();
    batch.text_styled(&atlas, &named, 10.0, 10.0, u32::MAX, true);
    assert_eq!(batch.vertices.len(), 24); // shadow and front, each with face and hat
    assert_eq!(batch.vertices[0].pos, [11.0, 10.0]);
    assert_eq!(batch.vertices[0].color, rgba(63, 63, 63, 255));
    assert_eq!(batch.vertices[12].pos, [10.0, 9.0]);
    assert_eq!(batch.vertices[12].color, u32::MAX);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn atlas_object_draws_an_item_sprite_inline() {
    let root = two_glyph_font("inline-item");
    write_png(&root, "item/porkchop", 16, 16);
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let text = crate::component::to_text(
        &serde_json::json!({
            "text": "a",
            "extra": [{"object": "atlas", "sprite": "minecraft:item/porkchop"}]
        }),
        &mut |_| crate::text::Text::empty(),
    );
    let mut batch = HudBatch::scratch([320, 240]);
    let width = batch.text_styled(&atlas, &text, 10.0, 10.0, u32::MAX, false);
    assert_eq!(width, atlas.font.glyph('a').advance + 8.0);
    assert_eq!(batch.vertices.len(), 12); // glyph and one sprite
    let sprite = atlas.sprites["item/porkchop"];
    assert_eq!(batch.vertices[6].uv, [sprite.u0, sprite.v0]);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn name_tags_draw_bold_text_and_tall_glyphs_like_any_text() {
    // `ascii.png`'s 8x8 'a' (4 pixels wide) and `accented.png`'s 9x12
    // 'й' (height 12, ascent 10: the breve above the line), 7 wide.
    let root = std::env::temp_dir().join(format!("sunrise-name-tag-{}", std::process::id()));
    let textures = root.join("assets/minecraft/textures/font");
    fs::create_dir_all(root.join("assets/minecraft/font")).unwrap();
    fs::create_dir_all(&textures).unwrap();
    fs::write(
        root.join("assets/minecraft/font/default.json"),
        r#"{"providers":[
            {"type":"bitmap","file":"minecraft:font/a.png","ascent":7,"chars":["a"]},
            {"type":"bitmap","file":"minecraft:font/tall.png","height":12,"ascent":10,"chars":["й"]}]}"#,
    )
    .unwrap();
    for (name, (w, h), used) in [("a", (8, 8), 4), ("tall", (9, 12), 7)] {
        let pixels: Vec<u8> = (0..w * h)
            .flat_map(|i| [255, 255, 255, if i % w < used { 255 } else { 0 }])
            .collect();
        let file = fs::File::create(textures.join(format!("{name}.png"))).unwrap();
        let mut encoder = png::Encoder::new(file, w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&pixels).unwrap();
    }
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let _ = fs::remove_dir_all(&root);
    let text = crate::text::Text::literal("aй").bold();
    let mut batch = HudBatch::scratch([1, 1]);
    let behind = batch.name_tag(&atlas, &text, 0.0, u32::MAX, 0x3F00_0000);
    // Each quad's box: x0, y0, x1, y1.
    let boxes: Vec<[f32; 4]> = batch
        .vertices
        .as_chunks::<6>()
        .0
        .iter()
        .map(|q| [q[0].pos[0], q[0].pos[1], q[2].pos[0], q[2].pos[1]])
        .collect();
    // Advances 5 and 8, plus 1 each for bold: Font.width 15, centred.
    let x = -7.5;
    // Each glyph twice (bold), the copy a pixel right; 'й' in its 9x12
    // box from 3 pixels above the line, each bold quad 0.1 larger.
    let near = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-4);
    let expected = [
        [x - 0.1, -0.1, x + 8.1, 8.1],
        [x + 0.9, -0.1, x + 9.1, 8.1],
        [x + 5.9, -3.1, x + 15.1, 9.1],
        [x + 6.9, -3.1, x + 16.1, 9.1],
    ];
    for (got, want) in boxes[1..].iter().zip(expected) {
        assert!(near(*got, want), "{got:?} != {want:?}");
    }
    assert_eq!(boxes.len(), 1 + expected.len());
    // The background first: a pixel round the advances, 10 pixels tall.
    assert_eq!(behind, 6);
    assert_eq!(boxes[0], [x - 1.0, -1.0, x + 15.0, 9.0]);
}

#[test]
fn effect_icons_are_painted_from_the_mob_effect_directory() {
    use crate::{effects::EffectInstance, hud_layout, hud_overlays, inventory_effects};
    // A synthetic resource tree exercises loading and painting together.
    // No Minecraft installation or GPU is needed for this regression test.
    let root = std::env::temp_dir().join(format!("sunrise-effect-icons-{}", std::process::id()));
    write_vanilla_gui_definition(&root);
    for (name, size) in [
        ("mob_effect/levitation", 18),
        ("gui/sprites/hud/effect_background", 24),
        ("gui/sprites/container/inventory/effect_background", 32),
    ] {
        let path = root.join(format!("assets/minecraft/textures/{name}.png"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut encoder = png::Encoder::new(fs::File::create(path).unwrap(), size, size);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&vec![255; (size * size * 4) as usize])
            .unwrap();
    }
    let atlas = HudAtlas::build(&sunrise_assets::resource_source::ResourceSource::directory(
        root.to_path_buf(),
    ))
    .unwrap();
    let effect = EffectInstance::new("levitation", -1, 0);
    let icon = hud_overlays::EffectIcon::from(&effect);
    let draws = hud_overlays::effects(320, false, &[icon]);
    let mut batch = HudBatch::scratch([320, 240]);
    hud_layout::paint(&mut batch, &atlas, &draws);
    // Both the background and the 18x18 effect icon must reach the batch.
    assert_eq!(batch.vertices.len(), 12);
    let rect = atlas.sprites["mob_effect/levitation"];
    assert_eq!(batch.vertices[6].pos, [298.0, 30.0]);
    assert_eq!(batch.vertices[6].uv, [rect.u0, rect.v0]);
    assert_eq!(batch.vertices[8].uv, [rect.u1, rect.v1]);

    let panels = {
        let space = crate::inventory_effects::EffectsSpace {
            screen_width: 320,
            origin: [72, 37],
            image_width: 176,
        };
        let vanilla = crate::inventory_effects::EffectsLayout::vanilla(space.available());
        crate::inventory_effects::panels(
            &space,
            &vanilla,
            std::slice::from_ref(&effect),
            20.0,
            &|_| None,
            &|_| 60,
        )
    };
    batch.vertices.clear();
    let tooltip = inventory_effects::paint(&mut batch, &atlas, &panels, [257, 44]);
    let lines = inventory_effects::tooltip(tooltip.unwrap());
    let plain: Vec<_> = lines.iter().map(|(t, c)| (t.plain(), *c)).collect();
    assert_eq!(
        plain,
        [
            ("levitation".to_owned(), u32::MAX),
            ("∞".to_owned(), u32::MAX)
        ]
    );
    assert_eq!(batch.vertices.len(), 24);
    assert_eq!(batch.vertices[18].pos, [257.0, 44.0]);
    assert_eq!(batch.vertices[18].uv, [rect.u0, rect.v0]);
    assert_eq!(batch.vertices[20].uv, [rect.u1, rect.v1]);

    // Wide panels retain the four-pixel border instead of stretching it.
    let panels = {
        let space = crate::inventory_effects::EffectsSpace {
            screen_width: 480,
            origin: [72, 37],
            image_width: 176,
        };
        let vanilla = crate::inventory_effects::EffectsLayout::vanilla(space.available());
        crate::inventory_effects::panels(
            &space,
            &vanilla,
            std::slice::from_ref(&effect),
            20.0,
            &|_| None,
            &|_| 60,
        )
    };
    batch.vertices.clear();
    assert!(inventory_effects::paint(&mut batch, &atlas, &panels, [0, 0]).is_none());
    assert!(batch.vertices.iter().any(|v| v.color == 0xff808080));
    assert_eq!(batch.vertices[0].pos, [250.0, 37.0]);
    assert_eq!(batch.vertices[1].pos, [254.0, 37.0]);
    let background = atlas.sprites["gui/sprites/container/inventory/effect_background"];
    assert!(
        (batch.vertices[1].uv[0] - background.u0 - (background.u1 - background.u0) / 8.0).abs()
            < 1e-6
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn gui_models_keep_separate_depth_runs_between_overlay_draws() {
    let mut batch = HudBatch::scratch([640, 480]);
    batch.scale = 2.0;
    let p = [
        [0.0, 0.0, 2.0],
        [0.0, 1.0, 2.0],
        [1.0, 1.0, 2.0],
        [1.0, 0.0, 2.0],
    ];
    let uv = [[0.0; 2]; 4];
    batch.quad([[0.0; 2]; 4], uv, u32::MAX);
    for _ in 0..2 {
        let start = batch.vertices.len();
        batch.quad_3d(p, uv, u32::MAX);
        batch.depth_since(start, [1.0, 2.0, 4.0, 6.0]);
    }
    batch.quad([[0.0; 2]; 4], uv, u32::MAX);
    assert_eq!(
        batch.runs(),
        vec![
            (0..6, Blend::Alpha),
            (6..12, Blend::Depth([2, 4, 6, 8])),
            (12..18, Blend::Depth([2, 4, 6, 8])),
            (18..24, Blend::Alpha),
        ]
    );
    assert!(
        batch
            .vertices
            .iter()
            .all(|v| v.depth.is_finite() && (0.0..=1.0).contains(&v.depth))
    );
}

/// At GUI scale 2, geometry ending at GUI x = 5.375 reaches device x = 10.75 and takes
/// the pixel centred at 10.5. An explicit clip ending at GUI x = 5.125 reaches device
/// x = 10.25 and gains no column.
#[test]
fn a_depth_scissor_over_geometry_rounds_outward_and_an_explicit_clip_does_not() {
    let quad = |batch: &mut HudBatch| {
        let start = batch.vertices.len();
        batch.quad_3d(
            [
                [0.625, 1.0, 0.0],
                [0.625, 3.25, 0.0],
                [5.375, 3.25, 1.0],
                [5.375, 1.0, 1.0],
            ],
            [[0.0; 2]; 4],
            u32::MAX,
        );
        start
    };
    let mut batch = HudBatch::scratch([640, 480]);
    batch.scale = 2.0;
    let start = quad(&mut batch);
    batch.depth_over(start, [0.625, 1.0], [5.375, 3.25]);
    assert_eq!(batch.runs(), vec![(0..6, Blend::Depth([1, 2, 10, 5]))]);
    let mut batch = HudBatch::scratch([640, 480]);
    batch.scale = 2.0;
    let start = quad(&mut batch);
    batch.depth_since(start, [0.625, 1.0, 5.125, 3.25]);
    assert_eq!(batch.runs(), vec![(0..6, Blend::Depth([1, 2, 9, 4]))]);
}

#[test]
fn a_gui_models_glint_follows_it_in_its_depth_pass() {
    let mut batch = HudBatch::scratch([640, 480]);
    batch.scale = 2.0;
    let face = |z: f32| [[0.0, 0.0, z], [0.0, 1.0, z], [1.0, 1.0, z], [1.0, 0.0, z]];
    let uv = [[0.25, 0.5]; 4];
    let start = batch.vertices.len();
    batch.quad_3d(face(1.0), uv, u32::MAX);
    batch.quad_3d(face(3.0), uv, u32::MAX);
    let glint = batch.vertices.len();
    batch.quad_3d_glint(face(3.0), uv, [[7.0, 8.0]; 4], true);
    batch.quad_3d_glint(face(1.0), uv, [[1.0, 2.0]; 4], false);
    batch.depth_glint_since(start, glint, [1.0, 2.0, 4.0, 6.0]);
    assert_eq!(
        batch.runs(),
        vec![
            (0..12, Blend::Depth([2, 4, 6, 8])),
            (12..24, Blend::DepthGlint([2, 4, 6, 8])),
        ]
    );
    // The glint shares its face's depth, normalised with the model's.
    assert_eq!(batch.vertices[12].depth, batch.vertices[6].depth);
    assert_eq!(batch.vertices[18].depth, batch.vertices[0].depth);
    // The shader picks the armour glint by u >= 4, the item's by u >= 2,
    // and masks by the atlas UV under the marker.
    assert_eq!(batch.vertices[12].uv, [4.25, 0.5]);
    assert_eq!(batch.vertices[12].glint_uv, [7.0, 8.0]);
    assert_eq!(batch.vertices[18].uv, [2.25, 0.5]);
    // A model with no glint keeps a single depth run.
    let start = batch.vertices.len();
    batch.quad_3d(face(1.0), uv, u32::MAX);
    batch.depth_glint_since(start, batch.vertices.len(), [1.0, 2.0, 4.0, 6.0]);
    assert_eq!(
        batch.runs().last(),
        Some(&(24..30, Blend::Depth([2, 4, 6, 8])))
    );
}

#[test]
fn hud_shader_validates_both_fragment_entries() {
    let module = naga::front::wgsl::parse_str(include_str!("../hud.wgsl")).unwrap();
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap();
}

#[test]
fn inverted_ranges_split_the_draws_in_order() {
    let mut batch = HudBatch::scratch([854, 480]);
    let quad = |b: &mut HudBatch| b.quad([[0.0; 2]; 4], [[0.0; 2]; 4], 0);
    quad(&mut batch);
    let start = batch.vertices.len();
    quad(&mut batch);
    batch.invert_since(start);
    let start = batch.vertices.len();
    quad(&mut batch);
    batch.invert_since(start);
    batch.invert_since(batch.vertices.len());
    let start = batch.vertices.len();
    quad(&mut batch);
    batch.blend_since(start, Blend::Additive);
    quad(&mut batch);
    assert_eq!(
        batch.blends,
        vec![(6..18, Blend::Invert), (18..24, Blend::Additive)]
    );
    assert_eq!(
        batch.runs(),
        vec![
            (0..6, Blend::Alpha),
            (6..18, Blend::Invert),
            (18..24, Blend::Additive),
            (24..30, Blend::Alpha)
        ]
    );
}

#[test]
fn blur_boundary_splits_blend_runs_without_losing_or_repeating_vertices() {
    let mut batch = HudBatch::scratch([854, 480]);
    for _ in 0..5 {
        batch.quad([[0.0; 2]; 4], [[0.0; 2]; 4], 0);
    }
    batch.blends = vec![
        (6..18, Blend::Invert),
        (18..24, Blend::Depth([0, 0, 10, 10])),
    ];
    for split in [0, 6, 12, 18, 24, 30] {
        let before = batch.runs_in(0..split);
        let after = batch.runs_in(split..30);
        let actual: Vec<_> = before
            .into_iter()
            .chain(after)
            .flat_map(|(range, blend)| range.map(move |v| (v, blend)))
            .collect();
        let expected: Vec<_> = batch
            .runs()
            .into_iter()
            .flat_map(|(range, blend)| range.map(move |v| (v, blend)))
            .collect();
        assert_eq!(actual, expected, "blur at {split}");
    }
}
#[test]
fn container_origins_use_java_integer_division() {
    // Odd scaled dimensions, odd image sizes and an oversized panel.
    for (width, height) in [(640, 360), (641, 361), (456, 256), (534, 300), (100, 100)] {
        let batch = HudBatch {
            pictures: Vec::new(),
            vertices: Vec::new(),
            scale: 3.0,
            width: width as f32,
            height: height as f32,
            patches: Vec::new(),
            image_patches: Vec::new(),
            blends: Vec::new(),
            end_portal: EndPortalFill::default(),
            glint_alpha: 0.75,
            glint_colour: 0,
            game_time: 0.0,
            blur_at: 0,
            inline_profiles: Vec::new(),
        };
        for (image_width, image_height) in [(195, 136), (176, 166), (176, 222)] {
            assert_eq!(
                batch.centered_origin(image_width as f32, image_height as f32),
                (
                    ((width - image_width) / 2) as f32,
                    ((height - image_height) / 2) as f32
                )
            );
        }
    }
}
