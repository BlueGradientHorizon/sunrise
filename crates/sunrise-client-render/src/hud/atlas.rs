use super::*;

pub struct HudAtlas {
    pub(super) pictures: std::sync::Mutex<pictures::Cache>,
    /// Identity of this packed atlas, used to invalidate cached model UV data
    /// even when a resource reload places sprites at the same coordinates.
    pub revision: u64,
    /// RGBA8, `width` by `height`, as built: empty once `pack_pixels` has
    /// compressed them (read them through `pixels()`).
    pub pixels: Vec<u8>,
    /// `pixels` compressed (lz4) on a thread of its own once the renderer
    /// has uploaded them: only rebuilding a renderer reads them again.
    pub(super) packed: Option<Arc<OnceLock<Vec<u8>>>>,
    /// The width each Standard Galactic letter uses in `font/ascii_sga`'s
    /// cell, by ASCII code (the enchanting table's text), read when built.
    pub galactic_widths: [u16; 128],
    pub width: u32,
    pub height: u32,
    pub sprites: HashMap<String, Rect>,
    /// The GUI atlas's own sprites by GUI id, each with an entry of its own: its rect, or none for
    /// a sprite its list removed or failed ([`HudAtlas::gui_lookup`]).
    pub gui: HashMap<String, GuiSprite>,
    /// The packed names no GUI lookup of an id the GUI list lacks answers with: the model atlases'
    /// sprites and the walked files a GUI entry stands on.
    pub non_gui: HashSet<String>,
    /// A registered atlas's sprites by their GUI names (`mymod:gui/atlases/icons/mymod/heart`),
    /// each packed under a key of its own so that no model or GUI sprite can take its place.
    pub registered: HashMap<String, Rect>,
    /// The live (dynamic) textures by GUI id, a lane of their own: the GUI lookup reads it first,
    /// and no reload of another lane writes it ([`dynamic_textures::Names`]).
    pub live: HashMap<String, Rect>,
    /// Resolved player-profile names and UUIDs for inline head components.
    pub object_skins: HashMap<String, String>,
    pub object_skins_revision: u64,
    /// Sprites with a 0 < alpha < 255 texel (`SpriteContents.transparency().hasTranslucent`),
    /// which puts their particles on vanilla's translucent layer.
    pub translucent: HashSet<String>,
    /// Sprites whose `.png.mcmeta` sets `blur` or `clamp`; the others
    /// sample as vanilla's defaults (nearest, repeat).
    pub flags: HashMap<String, TextureFlags>,
    /// Every animated block or item sprite the atlas packs: its atlas rect
    /// and the mcmeta-driven frame list, re-uploaded into the rect as the
    /// frames turn (vanilla's stitched atlas animates all of these).
    pub sprite_animations: Vec<(Rect, crate::sprites::SpriteAnimation)>,
    /// The default font (bitmap sheets in this atlas, unifont rasterized on demand).
    pub font: crate::font::Font,
    /// `misc/enchanted_glint_item` as its own image: vanilla's `GlintSampler`
    /// repeats it per pixel, which one sprite inside this atlas cannot.
    pub glint: (Vec<u8>, u32, u32),
    /// `misc/enchanted_glint_armor`, the armour layers' `GlintSampler`.
    pub glint_armor: (Vec<u8>, u32, u32),
    pub text_shaders: crate::gfx::core_text::TextShaders,
    /// A resource pack's own `core/rendertype_lines` shader for the block outline, translated;
    /// none draws the outline natively.
    pub lines_shader: Option<std::sync::Arc<crate::core_shader::CoreProgram>>,
    /// Its Polytone expression uniforms (`shader_modifiers`), resolved when it loaded.
    pub lines_expressions: Vec<crate::polytone_post::ExpressionUniform>,
    /// The packs' `core/sky` and `core/stars`, when a pack replaces them.
    pub sky_shader: Option<std::sync::Arc<crate::core_shader::CoreProgram>>,
    pub stars_shader: Option<std::sync::Arc<crate::core_shader::CoreProgram>>,
    /// Their Polytone expression uniforms, resolved when they loaded.
    pub sky_expressions: Vec<crate::polytone_post::ExpressionUniform>,
    pub stars_expressions: Vec<crate::polytone_post::ExpressionUniform>,
    /// The packs' `core/lightmap` (with `core/screenquad`), when a pack replaces either stage.
    pub lightmap_shader: Option<std::sync::Arc<crate::core_shader::CoreProgram>>,
    /// Its Polytone expression uniforms (`shader_modifiers`), resolved when it loaded: the
    /// blocks its stages read, with their programs.
    pub lightmap_expressions: Vec<crate::polytone_post::ExpressionUniform>,
    /// The packs' `core/clouds`, when a pack replaces either stage.
    pub clouds_shader: Option<std::sync::Arc<crate::core_shader::CoreProgram>>,
    /// The packs' Polytone screen sprite modifiers.
    pub sprite_modifiers: std::sync::Arc<sprite_modifiers::SpriteModifiers>,
    /// The packs' OptiFine random entity textures, whose variants this atlas packs.
    pub random_textures: sunrise_optifine::random_textures::RandomTextures,
    /// Their emissive overlays (`_e`), packed here too.
    pub emissive: std::sync::Arc<sunrise_optifine::emissive::Emissive>,
    /// ETF's blinking frames (`_blink`), packed here too.
    pub blinks: sunrise_optifine::blink::Blinks,
    /// Of the shared atlas's textures, their LabPBR normal and specular maps as paged maps
    /// (`paged_maps.rs`), read while loading for shader packs (`sprites::labpbr_maps`).
    pub material_maps: std::sync::Arc<[Option<crate::paged_maps::PagedMap>; 2]>,
    /// The white texel flat fills sample (also `hud_kit::shapes`).
    pub(crate) white: Rect,
    /// Where the slots of client mods' dynamic textures lie, when the atlas packed them
    /// ([`HudAtlas::build_over_with_dynamic_textures`]); their names are not among `sprites`.
    pub dynamic_texture_pool: Option<dynamic_textures::Pool>,
}
/// Atlas rows reserved for glyphs rasterized at run time.
pub const GLYPH_ROWS: u32 = 512;
/// The largest atlas side: the 2D texture limit of the GPUs Sunrise runs on
/// (Apple silicon, RADV and Intel allow 16384; NVIDIA 32768).
pub const MAX_ATLAS_SIZE: u32 = 16384;
/// 128x128 atlas slots for map textures. A lobby paints its signs as walls of
/// framed maps, so the pool holds a wall; too few slots re-upload every map
/// every frame. 26.3's `MapTextureManager` keeps a texture per map instead.
pub const MAP_SLOTS: usize = 256;
/// The atlas sprite of map slot `slot` (`sunrise/map{slot}`), named once:
/// every map shown asks for its slot's each frame.
pub fn map_slot_sprite(slot: usize) -> Option<&'static str> {
    static NAMES: std::sync::LazyLock<Vec<String>> =
        std::sync::LazyLock::new(|| (0..MAP_SLOTS).map(|i| format!("sunrise/map{i}")).collect());
    NAMES.get(slot).map(String::as_str)
}
/// Atlas slots for downloaded player skins (`skins/<sha1>`), 64x64 each.
/// Vanilla registers a texture per skin. Here the slots are shared
/// (`skins::SkinSlots`): skins drawn in the last frame keep theirs, and a
/// skin past them shows the default skin until a slot frees up.
pub const SKIN_SLOTS: usize = 128;
/// Reused by the visible rows of the classic world selector.
pub const WORLD_ICON_SLOTS: usize = 32;
/// Atlas slots for the pack icons on the pack screen, one per icon drawn
/// (`PackSelectionScreen.loadPackIcon` registers a texture per pack): two lists
/// of 36-pixel rows at GUI scale 2 on a 4K screen still fit.
pub const PACK_ICON_SLOTS: usize = 64;
/// A pack icon slot's side: vanilla's own icons are 128x128.
pub const PACK_ICON_SIZE: u32 = 128;
/// Atlas slots for armour trim textures made at run time (`trims`), each the
/// size of the largest trim texture.
pub const TRIM_SLOTS: usize = 32;

/// The textures the enabled packs' Custom Item Textures rules draw, beyond those already packed:
/// item rules' textures and their JSON models', armour and elytra rules' textures, under their
/// `cit_assets` ids, with their flags and whether they animate. An animated strip keeps its
/// first frame; a still texture is packed whole. None without `optifine/cit` rules.
pub(super) fn cit_sprites(
    root: &ResourceSource,
    packed: &[PackedImage],
) -> Vec<(PackedImage, Option<TextureFlags>, bool)> {
    let have: HashSet<&str> = packed.iter().map(|i| i.0.as_str()).collect();
    let rules = crate::cit::load_rules(root);
    let mut ids = crate::cit_assets::item_sprites(&rules);
    if !rules.is_empty() {
        let models = crate::cit_assets::load_models(root, &rules);
        ids.extend(crate::cit_assets::model_sprites(&models));
        ids.extend(crate::cit_armor::armor_sprites(&rules));
    }
    let ids: Vec<String> = ids
        .into_iter()
        // Vanilla's sprites are packed without their namespace, as `sprite_name` looks them up;
        // Sunrise's own (`sunrise/white`, the run-time slots) are added after, and stay its own.
        .filter(|id| {
            let name = id.strip_prefix("minecraft:").unwrap_or(id);
            !have.contains(name) && !name.starts_with("sunrise/")
        })
        .collect();
    // No more than the atlas could ever hold, by their headers, so a pack naming thousands of
    // big textures is cut short before they are decoded, not after. Each is charged what it
    // can be packed as (a strip's frame, or a still image whole), 1024x512 at most since
    // `cit_sprite` refuses more, with a gutter;
    // the glyph rows and the shelves' waste (a tenth) are kept aside.
    let side = u64::from(MAX_ATLAS_SIZE);
    let used: u64 = packed
        .iter()
        .map(|i| u64::from(i.1 + 1) * u64::from(i.2 + 1))
        .sum();
    let mut left = (side * side * 9 / 10).saturating_sub(side * u64::from(GLYPH_ROWS) + used);
    let before = ids.len();
    let ids: Vec<String> = ids
        .into_iter()
        .filter(|id| {
            let path = crate::atlas_sources::texture_resource(id, "textures");
            let Some((w, h)) = png_size(root, &path) else {
                return true;
            };
            let (w, h) = (u64::from(w), u64::from(h));
            if w * h > 1024 * 512 * 64 {
                // Refused by `cit_sprite` without decoding: costs nothing.
                return true;
            }
            // A strip's frame is at most its width squared; a still image is all of it.
            let frame = (w * h).min(w * w).min(1024 * 512) + w + h + 1;
            match left.checked_sub(frame) {
                Some(rest) => {
                    left = rest;
                    true
                }
                None => false,
            }
        })
        .collect();
    if ids.len() < before {
        log::warn!(
            "left out {} custom item textures: more than the atlas holds",
            before - ids.len()
        );
    }
    // Decoded on the threads `pack` decodes the rest on: a big pack names thousands.
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = ids.len().div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        let workers: Vec<_> = ids
            .chunks(chunk)
            .map(|ids| scope.spawn(|| ids.iter().filter_map(|id| cit_sprite(root, id)).collect()))
            .collect();
        workers
            .into_iter()
            .flat_map(|w| {
                w.join().unwrap_or_else(|_| {
                    log::warn!("a custom item texture decoder panicked; its textures are left out");
                    Vec::new()
                })
            })
            .collect()
    })
}
/// One of `cit_sprites`, or `None` (with a warning) when it does not load or is too big.
fn cit_sprite(
    root: &ResourceSource,
    id: &str,
) -> Option<(PackedImage, Option<TextureFlags>, bool)> {
    let name = id.strip_prefix("minecraft:").unwrap_or(id);
    let path = crate::atlas_sources::texture_resource(id, "textures");
    // The limit `pack` keeps every other texture to, for a frame; and from the header first,
    // so an image bigger than 64 such frames is never decoded.
    let fits = |w: u32, h: u32| w <= 1024 && h <= 512;
    if let Some((w, h)) = png_size(root, &path)
        && u64::from(w) * u64::from(h) > 1024 * 512 * 64
    {
        log::warn!("skipped custom item texture {}: {w}x{h}", path);
        return None;
    }
    let image = match decode_resource(root, &path) {
        Ok(image) => image,
        Err(e) => {
            log::warn!("skipped custom item texture {}: {e}", path);
            return None;
        }
    };
    let mcmeta = root.read_to_string(&format!("{path}.mcmeta")).ok();
    let animation = animation_of(mcmeta.as_deref());
    // Only an animation is cut to its first frame: a still CIT texture is drawn whole, and
    // armour layers are twice as wide as they are tall.
    let (w, h, pixels) = match &animation {
        Some(animation) => match first_frame(image, Some(animation)) {
            Ok(image) => image,
            Err(e) => {
                log::warn!("skipped custom item texture {}: {e}", path);
                return None;
            }
        },
        None => image,
    };
    if !fits(w, h) {
        log::warn!("skipped custom item texture {}: {w}x{h}", path);
        return None;
    }
    let flags = mcmeta.as_deref().and_then(TextureFlags::from_mcmeta);
    Some(((name.to_owned(), w, h, pixels), flags, animation.is_some()))
}
/// How the GUI atlas publishes one of its ids.
pub(super) enum GuiPublished {
    /// The sprite is packed under this name, with the nine-slice scaling its file gave it.
    Packed(String, Option<crate::ui::NineSlice>),
    /// The list removed it or its origin didn't finish.
    Missing,
}
/// The GUI atlas's answer for one id ([`HudAtlas::gui_lookup`]).
#[derive(Clone, Copy, Debug)]
pub enum GuiSprite {
    /// The sprite the id names, with the nine-slice scaling of the file it is (none for a
    /// generated sprite, a live texture or any sprite without that metadata: it stretches).
    Present {
        rect: Rect,
        scaling: Option<crate::ui::NineSlice>,
    },
    /// The id is listed, and its sprite was removed or its origin didn't finish.
    Missing,
}
/// The nine-slice scaling a sprite file's `.png.mcmeta` gives (none: it stretches).
fn nine_slice_of(root: &ResourceSource, file: &str) -> Option<crate::ui::NineSlice> {
    let text = root.read_to_string(&format!("{file}.mcmeta")).ok()?;
    crate::packs::first_json(text.as_bytes())
        .ok()
        .as_ref()
        .and_then(crate::ui::parse_nine_slice)
}
/// What [`atlas_sprites`] found.
#[derive(Default)]
pub(super) struct AtlasSprites {
    /// The sprites to pack, by packed name.
    pub(super) images: Vec<PackedImage>,
    /// The packed names that animate.
    pub(super) animated: Vec<String>,
    /// The packed names to drop.
    pub(super) dropped: HashSet<String>,
    /// The render flags of the sprites added.
    pub(super) flags: HashMap<String, TextureFlags>,
    /// The resource file each GUI sprite added from a file takes its animation from.
    pub(super) gui_files: HashMap<String, String>,
    /// The GUI atlas's own answer for the names a model or a removal makes ambiguous: the packed
    /// name its pixels are under, or none for a sprite its list removed or failed.
    pub(super) gui: Vec<(String, GuiPublished)>,
    /// The packed names no GUI lookup of an unlisted id answers with: the model atlases' sprites
    /// and the walked files a GUI entry stands on.
    pub(super) non_gui: HashSet<String>,
}
/// What the atlas definitions (`atlases/*.json`) change in the texture files already packed:
/// the sprites to add (generated ones such as the armour trim palettes and `unstitch` regions,
/// and files listed under another id), the animated ones' names, and the packed names to drop.
///
/// A model's material is one finished sprite, the one the item masks resolve: the items atlas's
/// first, then the blocks atlas's. The walked file of its name stands only when that is the
/// winning origin; a rename, a cut or a palette replaces it, and a walked block or item texture
/// no atlas keeps is dropped, as the model baker would find it missing.
///
/// The GUI atlas's final list decides its sprites the same way, under the names the GUI is looked
/// up by: a sprite `minecraft:container/slot` is packed as `gui/sprites/container/slot` where the
/// walk packed it so (or by its plain id where that is walked), a mod's `mymod:panel` under its
/// plain id, and a walked `gui/sprites/*` file no entry keeps is dropped. Where a model's sprite
/// has the same packed name the GUI's sprite is packed apart (`GUI_KEY`) and found through
/// `HudAtlas::gui`, which also answers for a GUI sprite its list removed or failed.
pub(super) fn atlas_sprites(root: &ResourceSource, packed: &[PackedImage]) -> AtlasSprites {
    use crate::atlas_sources::{self, Materials, SpriteOrigin};
    let have: HashSet<&str> = packed.iter().map(|i| i.0.as_str()).collect();
    let materials = Materials::new(root);
    let listing = materials.listing();
    let ids: Vec<&str> = materials.ids().into_iter().collect();
    // Each id's winner, all at once: `None` where no origin finishes, `Some(None)` where the
    // file already walked stands.
    let winners: HashMap<&str, Option<Option<atlas_sources::Material>>> = ids
        .iter()
        .copied()
        .zip(crate::par::par_map(&ids, |id| {
            let name = id.strip_prefix("minecraft:").unwrap_or(id);
            materials.resolve(id, |origin| match origin {
                // The walked file stands if its frames tile it, as a finished sprite's do.
                SpriteOrigin::File(texture)
                    if texture == id && have.contains(name) && walked_frames_fit(root, texture) =>
                {
                    Some(None)
                }
                // A file under a walked folder is packed under its own name or was too large
                // for the atlas: either way it is not added again.
                SpriteOrigin::File(texture)
                    if texture.strip_prefix("minecraft:") == Some(name)
                        && walked_minecraft_texture(name) =>
                {
                    None
                }
                origin => atlas_sources::finish_origin(root, origin).map(Some),
            })
        }))
        .collect();
    let model_names: HashSet<&str> = winners
        .keys()
        .map(|id| id.strip_prefix("minecraft:").unwrap_or(id))
        .collect();
    let mut dropped = HashSet::new();
    let mut out = Vec::new();
    let mut animated = Vec::new();
    let mut flags = HashMap::new();
    let mut gui_files = HashMap::new();
    let mut gui_table: Vec<(String, GuiPublished)> = Vec::new();
    let mut non_gui: HashSet<String> = model_names.iter().map(|name| (*name).to_owned()).collect();
    let gui = listing.sprites("minecraft:gui");
    let mut finished: Vec<(String, atlas_sources::Material, bool)> = Vec::new();
    if !materials.is_bare() {
        for (name, ..) in packed {
            let path = name.split_once(':').map_or(name.as_str(), |(_, p)| p);
            let winner = winners
                .get(atlas_sources::qualified(name).as_str())
                .and_then(|w| w.as_ref());
            let stands = matches!(winner, Some(None));
            let replaced = matches!(winner, Some(Some(_)));
            if replaced || (!stands && (path.starts_with("block/") || path.starts_with("item/"))) {
                dropped.insert(name.clone());
            }
        }
        finished.extend(winners.into_iter().filter_map(|(id, winner)| {
            Some((
                id.strip_prefix("minecraft:").unwrap_or(id).to_owned(),
                winner??,
                true,
            ))
        }));
    }
    // The GUI atlas owns every id its final list names, and every id a filter or `Remove` took
    // out of it, by that id: its sprite is the walked file the entry itself is, else packed under
    // a name of its own (`#gui:<id>`: `#` is in no identifier, so no model's or mod's name can be
    // it). A walked file is dropped only for the id whose own key it is, and not while another
    // entry stands on it; a file of the same name another owner packed (a registered atlas's
    // projection, a live texture's) is no GUI id's to drop.
    if listing.defines("minecraft:gui") {
        let mut claimed: HashSet<String> = HashSet::new();
        let mut standing: HashSet<String> = HashSet::new();
        let mut dropping: Vec<String> = Vec::new();
        let removed = listing.removed("minecraft:gui").map(|id| (id, None));
        for (id, origin) in gui
            .iter()
            .map(|(id, origin)| (id, Some(origin)))
            .chain(removed)
        {
            let plain = id.strip_prefix("minecraft:").unwrap_or(id);
            let key = gui_sprite::atlas_key_of_gui_sprite(id);
            let own = (have.contains(key.as_str()) && !model_names.contains(key.as_str()))
                .then(|| key.clone());
            claimed.extend(own.clone());
            let stands = match origin {
                Some(SpriteOrigin::File(texture)) => {
                    let name = texture.strip_prefix("minecraft:").unwrap_or(texture);
                    ([key.as_str(), plain].contains(&name)
                        && have.contains(name)
                        && !model_names.contains(name)
                        && walked_frames_fit(root, texture))
                    .then(|| name.to_owned())
                }
                _ => None,
            };
            if let Some(name) = stands {
                claimed.insert(name.clone());
                standing.insert(name.clone());
                let scaling =
                    nine_slice_of(root, &atlas_sources::texture_resource(&name, "textures"));
                gui_table.push((plain.to_owned(), GuiPublished::Packed(name, scaling)));
                continue;
            }
            dropping.extend(own);
            let material = origin.and_then(|origin| atlas_sources::finish_origin(root, origin));
            let Some(material) = material else {
                gui_table.push((plain.to_owned(), GuiPublished::Missing));
                continue;
            };
            let packed_as = format!("{GUI_KEY}{id}");
            if let Some(file) = &material.resource {
                gui_files.insert(packed_as.clone(), file.clone());
            }
            let scaling = material
                .resource
                .as_deref()
                .and_then(|file| nine_slice_of(root, file));
            gui_table.push((
                plain.to_owned(),
                GuiPublished::Packed(packed_as.clone(), scaling),
            ));
            finished.push((packed_as, material, false));
        }
        dropped.extend(dropping.into_iter().filter(|name| !standing.contains(name)));
        non_gui.extend(standing);
        // A walked GUI sprite the list names nowhere is no sprite of the GUI atlas.
        for (name, ..) in packed {
            let path = name.split_once(':').map_or(name.as_str(), |(_, p)| p);
            if path.starts_with("gui/sprites/")
                && !claimed.contains(name.as_str())
                && !model_names.contains(name.as_str())
            {
                dropped.insert(name.clone());
            }
        }
    }
    {
        for (name, material, model) in finished {
            let path = name.split_once(':').map_or(name.as_str(), |(_, p)| p);
            let image = match &material.resource {
                Some(file) => {
                    // Animated strips keep their first frame and animate, like the packed
                    // files, under any folder: vanilla animates every sprite of these atlases
                    // (a server pack's `wynn/...` as well as `item/...`).
                    let mcmeta = root.read_to_string(&format!("{file}.mcmeta")).ok();
                    let animation = material.animation.as_ref();
                    let image = if animation.is_some()
                        || (model && (path.starts_with("block/") || path.starts_with("item/")))
                    {
                        match first_frame(material.image, animation) {
                            Ok(image) => image,
                            Err(error) => {
                                log::warn!("skipped GUI texture {file}: {error}");
                                continue;
                            }
                        }
                    } else {
                        material.image
                    };
                    if animation.is_some() {
                        animated.push(name.clone());
                    }
                    // The winner's own render flags (blur, clamp), as the walk reads them.
                    if let Some(texture_flags) =
                        mcmeta.as_deref().and_then(TextureFlags::from_mcmeta)
                    {
                        flags.insert(name.clone(), texture_flags);
                    }
                    image
                }
                None => material.image,
            };
            out.push((name, image.0, image.1, image.2));
        }
        out.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    }
    // The other atlases' generated sprites (GUI sprites, particles...), by the first origin
    // that loads, in the atlases' order; they load at once (a content pack's atlases list
    // tens of thousands of files).
    let mut names: Vec<(String, Vec<SpriteOrigin>)> = Vec::new();
    let mut slots = HashMap::new();
    for (atlas, sprites) in listing.atlases() {
        if matches!(
            atlas.as_str(),
            "minecraft:blocks" | "minecraft:items" | "minecraft:gui"
        ) {
            continue;
        }
        for (id, origin) in sprites {
            let name = id.strip_prefix("minecraft:").unwrap_or(id).to_owned();
            // A model's sprite is the model atlases' winner above and a GUI sprite the GUI
            // atlas's: not another atlas's output.
            if matches!(origin, SpriteOrigin::File(_))
                || have.contains(name.as_str())
                || model_names.contains(name.as_str())
                || gui.contains_key(id)
            {
                continue;
            }
            let slot = *slots.entry(name.clone()).or_insert_with(|| {
                names.push((name, Vec::new()));
                names.len() - 1
            });
            names[slot].1.push(origin.clone());
        }
    }
    let load = |texture: &str| {
        decode_resource(root, &atlas_sources::texture_resource(texture, "textures")).ok()
    };
    out.extend(
        crate::par::par_map(&names, |(name, origins)| {
            origins
                .iter()
                .find_map(|origin| atlas_sources::generate(origin, &load))
                .map(|(w, h, pixels)| (name.clone(), w, h, pixels))
        })
        .into_iter()
        .flatten(),
    );
    AtlasSprites {
        images: out,
        animated,
        dropped,
        flags,
        gui_files,
        gui: gui_table,
        non_gui,
    }
}
/// The prefix of the name a GUI atlas sprite is packed under when a model's sprite has its name.
pub(super) const GUI_KEY: &str = "#gui:";
/// The prefix of the name a registered atlas's sprite is packed under.
pub(super) const REGISTERED_KEY: &str = "#registered:";

/// `MissingTextureAtlasSprite`: what a sprite the atlas lacks draws as.
pub const MISSING_SPRITE: &str = "sunrise/missing";

/// The used width of each Standard Galactic letter (A-Z, a-z) in the
/// `font/ascii_sga` sheet: one past its last column with a texel that isn't
/// transparent.
fn galactic_widths(
    sprites: &HashMap<String, Rect>,
    pixels: &[u8],
    width: u32,
    height: u32,
) -> [u16; 128] {
    let mut widths = [0u16; 128];
    let Some(sheet) = sprites.get("font/ascii_sga") else {
        return widths;
    };
    let cell = sheet.w / 16;
    let (px, py) = (
        (sheet.u0 * width as f32) as u32,
        (sheet.v0 * height as f32) as u32,
    );
    for code in (0x41..=0x5Au32).chain(0x61..=0x7A) {
        let (cx, cy) = (px + code % 16 * cell, py + code / 16 * cell);
        let alpha = |x: u32, y: u32| {
            pixels
                .get(((y * width + x) * 4 + 3) as usize)
                .copied()
                .unwrap_or(0)
        };
        widths[code as usize] = (0..cell)
            .rev()
            .find(|&dx| (0..cell).any(|dy| alpha(cx + dx, cy + dy) != 0))
            .map_or(0, |dx| (dx + 1).min(u32::from(u16::MAX)) as u16);
    }
    widths
}
/// `TextureAtlas.dumpContents` (`F3 + S`, vanilla's "dynamic textures"): the atlas
/// written as `name.png` beside `name.txt`, every sprite in name order with its
/// rect in texels (`dumpSpriteNames`). Vanilla names each file after its atlas
/// with `/` and `:` as `_` (`Identifier.toDebugFileName`); Sunrise packs the GUI,
/// font, particle, map and skin sprites into one atlas, so one file stands in for
/// the atlases vanilla dumps apart.
pub fn dump_atlas(
    dir: &Path,
    name: &str,
    width: u32,
    height: u32,
    pixels: &[u8],
    sprites: &HashMap<String, Rect>,
) -> std::io::Result<PathBuf> {
    // A build whose pixels were freed (`fonts`) has the size and no texels:
    // write nothing rather than a file every reader would reject.
    if pixels.len() != (width as usize) * (height as usize) * 4 {
        return Err(std::io::Error::other(format!(
            "wrong data size, expected {} got {}",
            (width as usize) * (height as usize) * 4,
            pixels.len()
        )));
    }
    fs::create_dir_all(dir)?;
    let png_path = dir.join(format!("{name}.png"));
    let mut encoder = png::Encoder::new(fs::File::create(&png_path)?, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(std::io::Error::other)?;
    writer
        .write_image_data(pixels)
        .map_err(std::io::Error::other)?;
    drop(writer);
    let mut names = sprites.iter().collect::<Vec<_>>();
    names.sort_by(|a, b| a.0.cmp(b.0));
    let mut list = String::new();
    for (sprite, rect) in names {
        let (w, h) = rect.texels((width, height));
        let x = (rect.u0 * width as f32).round() as u32;
        let y = (rect.v0 * height as f32).round() as u32;
        list.push_str(&format!("{sprite}\tx={x}\ty={y}\tw={w}\th={h}\n"));
    }
    fs::write(dir.join(format!("{name}.txt")), list)?;
    Ok(png_path)
}

impl HudAtlas {
    /// `TextureAtlas.dumpContents` for this atlas: `dir` is `screenshots/debug`,
    /// the folder `TextureUtil.getDebugTexturePath` names.
    pub fn dump_contents(&self, dir: &Path, name: &str) -> std::io::Result<PathBuf> {
        // `pixels()`: the copy `pack_pixels` compressed once the GPU had the
        // atlas, decompressed; vanilla reads the texture back from the GPU.
        let pixels = self.pixels();
        dump_atlas(dir, name, self.width, self.height, &pixels, &self.sprites)
    }

    fn next_revision() -> u64 {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
    /// The atlas's pixels: as built, or uncompressed again after
    /// `pack_pixels` (a renderer being rebuilt).
    pub fn pixels(&self) -> std::borrow::Cow<'_, [u8]> {
        let Some(packed) = self.packed.as_ref().filter(|_| self.pixels.is_empty()) else {
            return std::borrow::Cow::Borrowed(&self.pixels);
        };
        let size = (self.width * self.height * 4) as usize;
        std::borrow::Cow::Owned(
            lz4_flex::decompress_size_prepended(packed.wait())
                .ok()
                .filter(|p| p.len() == size)
                .unwrap_or_else(|| vec![0; size]),
        )
    }
    /// Keep the pixels compressed from now on, once a renderer has them: the
    /// window's GPU holds the atlas, and a copy as built only serves a
    /// renderer made again (another window, a lost device, another graphics
    /// API). They are compressed on a thread of their own; `pixels()` waits
    /// for it.
    pub fn pack_pixels(&mut self) {
        if self.pixels.is_empty() || self.packed.is_some() {
            return;
        }
        let raw = Arc::new(std::mem::take(&mut self.pixels));
        let slot = Arc::new(OnceLock::new());
        let (job_raw, job_slot) = (raw.clone(), slot.clone());
        let spawned = std::thread::Builder::new()
            .name("Atlas packing".into())
            .spawn(move || {
                let _ = job_slot.set(lz4_flex::compress_prepend_size(&job_raw));
            });
        if spawned.is_err() {
            let _ = slot.set(lz4_flex::compress_prepend_size(&raw));
        }
        self.packed = Some(slot);
    }
    /// The sprite of the GUI id `id` (`minecraft:` dropped) that is there, with its scaling: a live
    /// texture's; else the GUI list's own (a removed or failed entry ends the search); else, where
    /// the list says nothing of the id, a registered atlas's sprite of the name, and last a packed
    /// sprite no model or GUI entry owns.
    pub(super) fn gui_present(&self, id: &str) -> Option<(&Rect, Option<crate::ui::NineSlice>)> {
        if let Some(rect) = self.live.get(id) {
            return Some((rect, None));
        }
        match self.gui.get(id) {
            Some(GuiSprite::Present { rect, scaling }) => Some((rect, *scaling)),
            Some(GuiSprite::Missing) => None,
            None => self
                .registered
                .get(id)
                .or_else(|| self.sprites.get(id).filter(|_| !self.non_gui.contains(id)))
                .map(|rect| (rect, None)),
        }
    }
    /// A GUI atlas sprite by its id (`minecraft:container/slot`, `container/slot`, a mod's
    /// `mymod:panel`; the two spellings of a vanilla id are one key): [`GuiSprite::Present`] for a
    /// live texture, the GUI list's sprite, or for an unlisted id a registered atlas's sprite or a
    /// packed sprite no model or GUI entry owns, in that order; [`GuiSprite::Missing`] for an id the list removed or failed; none for an id the GUI
    /// atlas does not own. The sprite carries the scaling of the entry itself.
    pub fn gui_lookup(&self, id: &str) -> Option<GuiSprite> {
        let id = id.strip_prefix("minecraft:").unwrap_or(id);
        match self.gui_present(id) {
            Some((rect, scaling)) => Some(GuiSprite::Present {
                rect: *rect,
                scaling,
            }),
            None => self.gui.get(id).copied(),
        }
    }
    /// The rect a GUI draw uses for GUI sprite `id`: [`HudAtlas::gui_lookup`]'s, the missing
    /// sprite for an id the list removed or failed, none for an id the atlas does not own.
    pub fn gui_rect(&self, id: &str) -> Option<&Rect> {
        let id = id.strip_prefix("minecraft:").unwrap_or(id);
        match self.gui_present(id) {
            Some((rect, _)) => Some(rect),
            None => self
                .gui
                .contains_key(id)
                .then(|| self.sprites.get(MISSING_SPRITE))
                .flatten(),
        }
    }
    /// [`HudAtlas::gui_rect`] for a draw: an id the atlas lacks draws the missing texture, as vanilla's atlas does.
    pub fn gui_draw_rect(&self, id: &str) -> Option<&Rect> {
        if let Some(rect) = self.gui_rect(id) {
            return Some(rect);
        }
        // An atlas built with no resources has no GUI list, so nothing to miss against.
        if self.gui.is_empty() {
            return None;
        }
        missing_gui_sprite(id);
        self.sprites.get(MISSING_SPRITE)
    }
    /// The rect of a texture by its exact name: the packed sprite, else a registered atlas's
    /// sprite. The raw lookup of the camera draw; a GUI id goes through [`HudAtlas::gui_lookup`].
    pub fn texture_rect(&self, name: &str) -> Option<&Rect> {
        self.sprites.get(name).or_else(|| self.registered.get(name))
    }
    /// The rect of a `Blit` texture by its name: a live texture's first, else
    /// [`HudAtlas::texture_rect`]. One lookup for the refusal and the draw.
    pub fn blit_rect(&self, texture: &str) -> Option<&Rect> {
        self.live
            .get(texture.strip_prefix("minecraft:").unwrap_or(texture))
            .or_else(|| self.texture_rect(texture))
    }
    /// The pool and the tables a client mod's dynamic texture is named in.
    pub fn dynamic_names(
        &mut self,
    ) -> (Option<&dynamic_textures::Pool>, dynamic_textures::Names<'_>) {
        (
            self.dynamic_texture_pool.as_ref(),
            dynamic_textures::Names {
                sprites: &mut self.sprites,
                live: &mut self.live,
                gui: &mut self.gui,
                registered: &self.registered,
            },
        )
    }
    /// [`HudAtlas::gui_rect`], or the missing sprite where the atlas has no such GUI sprite (an
    /// effect without its icon).
    pub fn gui_sprite_or_missing(&self, id: &str) -> Option<Rect> {
        self.gui_rect(id)
            .or_else(|| self.sprites.get(MISSING_SPRITE))
            .copied()
    }
    /// A sprite, or the missing one when the atlas has no such sprite (an
    /// effect without its icon), as a stitched atlas answers.
    pub fn sprite_or_missing(&self, name: &str) -> Option<Rect> {
        self.sprites
            .get(name)
            .or_else(|| self.sprites.get(MISSING_SPRITE))
            .copied()
    }
    pub fn clear_object_skins(&mut self) {
        self.object_skins.clear();
        self.object_skins_revision = Self::next_revision();
    }
    /// Keep a ready profile skin when another source only has its default.
    pub fn set_object_skin(&mut self, key: String, skin: String) {
        use std::collections::hash_map::Entry;
        if self.object_skins.len() >= 1024 && !self.object_skins.contains_key(&key) {
            self.clear_object_skins();
        }
        match self.object_skins.entry(key) {
            Entry::Occupied(entry)
                if skin.starts_with("entity/player/") && self.sprites.contains_key(entry.get()) => {
            }
            Entry::Occupied(mut entry) => {
                if entry.get() != &skin {
                    entry.insert(skin);
                    self.object_skins_revision = Self::next_revision();
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(skin);
                self.object_skins_revision = Self::next_revision();
            }
        }
    }
    /// A font glyph's atlas cell (8x8 GUI pixels) and advance.
    pub fn glyph(&self, ch: char) -> Option<(Rect, f32)> {
        let g = self.font.glyph(ch);
        g.rect.map(|rect| (rect, g.advance))
    }
    /// A font glyph in the default font: its atlas cell (none for blank
    /// glyphs such as the space), its box and advance.
    pub fn glyph_cell(&self, ch: char) -> crate::font::Glyph {
        self.font.glyph(ch)
    }
    pub fn build(root: &ResourceSource) -> Result<Self> {
        Self::build_over(root, root)
    }
    /// The atlas of a resource root that layers packs over `defaults` (the
    /// game's own resources), whose textures give the sprites' GUI sizes.
    pub fn build_over(root: &ResourceSource, defaults: &ResourceSource) -> Result<Self> {
        Self::build_over_with_dynamic_textures(root, defaults, defaults, false)
    }
    /// The atlas above, with the pool of slots for client mods' dynamic textures
    /// (`dynamic_textures::DYNAMIC_TEXTURE_SLOTS` of each class, 10.5 MiB of texels) only when
    /// `pool` says a client mod is loaded: a game without mods packs no slot
    /// and its atlas does not grow. `game` is the game's own resources, without the packs: a
    /// pack's core shader (text, block outline, sky, stars, lightmap) is one that differs from
    /// the game's, so a `root` given as its own `game` loads none.
    pub fn build_over_with_dynamic_textures(
        root: &ResourceSource,
        defaults: &ResourceSource,
        game: &ResourceSource,
        pool: bool,
    ) -> Result<Self> {
        let mut atlas = Self::pack(root, defaults, WALKED_TEXTURE_FOLDERS, true, pool)?;
        atlas.dynamic_texture_pool =
            dynamic_textures::Pool::take(&mut atlas.sprites, atlas.width, atlas.height);
        pack_text::load_shaders(&mut atlas, root, game);
        atlas.sprite_modifiers = std::sync::Arc::new(sprite_modifiers::SpriteModifiers::read(
            root,
            atlas.gui.keys(),
        ));
        Ok(atlas)
    }
    /// The packs' core shaders at `root` over the game's own resources `game`, as
    /// [`Self::build_over_with_dynamic_textures`] loads them, for tests of the pipelines.
    #[cfg(test)]
    pub(crate) fn load_pack_shaders(&mut self, root: &ResourceSource, game: &ResourceSource) {
        pack_text::load_shaders(self, root, game);
    }
    /// Empty slots written at run time: map textures (game/blocks/maps/mod.rs),
    /// client mods' dynamic textures (only with `pool`), downloaded skins, world
    /// and pack icons and the on-screen keyboard's text. Their pixels are already zero in
    /// the destination atlas, so keep the source buffers empty and avoid duplicating the
    /// atlas-sized allocation during packing.
    pub(super) fn slots(images: &mut Vec<PackedImage>, trim: (u32, u32), pool: bool) {
        for i in 0..MAP_SLOTS {
            let name = map_slot_sprite(i).unwrap_or_default();
            images.push((name.into(), 128, 128, Vec::new()));
        }
        // The pool of client mods' dynamic textures, only when a client mod is loaded.
        if pool {
            let classes = mod_api::DYNAMIC_TEXTURE_CLASSES;
            for (class, slots) in dynamic_textures::DYNAMIC_TEXTURE_SLOTS.iter().enumerate() {
                for slot in 0..*slots {
                    let (name, side) = (dynamic_textures::slot_sprite(class, slot), classes[class]);
                    images.push((name, side, side, Vec::new()));
                }
            }
        }
        // Downloaded skins (game/interface/scoreboard/mod.rs).
        for i in 0..SKIN_SLOTS {
            images.push((format!("sunrise/skin{i}"), 64, 64, Vec::new()));
        }
        for i in 0..TRIM_SLOTS {
            images.push((format!("sunrise/trim{i}"), trim.0, trim.1, Vec::new()));
        }
        for i in 0..WORLD_ICON_SLOTS {
            images.push((format!("sunrise/world-icon{i}"), 64, 64, Vec::new()));
        }
        for i in 0..PACK_ICON_SLOTS {
            images.push((
                format!("sunrise/pack-icon{i}"),
                PACK_ICON_SIZE,
                PACK_ICON_SIZE,
                Vec::new(),
            ));
        }
        // The on-screen keyboard's text and corner masks (ui_theme), rasterized at run time.
        images.push(("sunrise/ui-text".into(), 2048, 1024, Vec::new()));
    }
    /// Only the font's metrics, for a game that draws nothing (the
    /// integrated or dedicated server): text measures as with the full atlas,
    /// without packing textures or rasterizing glyphs.
    pub fn fonts(root: &ResourceSource) -> Result<Self> {
        let white = Rect {
            u0: 0.0,
            v0: 0.0,
            u1: 1.0,
            v1: 1.0,
            w: 1,
            h: 1,
        };
        Ok(Self {
            pictures: Default::default(),
            revision: Self::next_revision(),
            pixels: Vec::new(),
            packed: None,
            material_maps: Default::default(),
            galactic_widths: [0; 128],
            width: 1,
            height: 1,
            sprites: HashMap::new(),
            gui: HashMap::new(),
            non_gui: HashSet::new(),
            registered: HashMap::new(),
            live: HashMap::new(),
            object_skins: HashMap::new(),
            object_skins_revision: Self::next_revision(),
            translucent: HashSet::new(),
            flags: HashMap::new(),
            sprite_animations: Vec::new(),
            font: crate::font::Font::metrics(root),
            glint: (Vec::new(), 1, 1),
            glint_armor: (Vec::new(), 1, 1),
            text_shaders: Default::default(),
            lines_shader: None,
            lines_expressions: Vec::new(),
            sky_shader: None,
            stars_shader: None,
            sky_expressions: Vec::new(),
            stars_expressions: Vec::new(),
            lightmap_shader: None,
            lightmap_expressions: Vec::new(),
            clouds_shader: None,
            sprite_modifiers: Default::default(),
            random_textures: Default::default(),
            emissive: Arc::default(),
            blinks: Default::default(),
            white,
            dynamic_texture_pool: None,
        })
    }
    /// The boot screen's atlas, before any pack loads: the theme's run-time
    /// glyphs (`sunrise/ui-text`), a white sprite and the glyph rows over
    /// transparent black. Nothing from a pack: the boot screen draws with
    /// the theme's bundled faces, so the font here has no sheets to read.
    pub fn boot() -> Self {
        let (width, height) = (1024, 1024);
        let text_rows = 512;
        let glyph_top = text_rows + 4;
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        // The white sprite the fills and the font's underline pass sample.
        for row in 0..4 {
            let at = ((text_rows + row) * width * 4) as usize;
            pixels[at..at + 16].fill(255);
        }
        let rect = |u0: u32, v0: u32, u1: u32, v1: u32, w: u32, h: u32| Rect {
            u0: u0 as f32 / width as f32,
            v0: v0 as f32 / height as f32,
            u1: u1 as f32 / width as f32,
            v1: v1 as f32 / height as f32,
            w,
            h,
        };
        let mut sprites = HashMap::new();
        sprites.insert(
            "sunrise/white".into(),
            rect(0, text_rows, 4, glyph_top, 4, 4),
        );
        sprites.insert(
            "sunrise/ui-text".into(),
            rect(0, 0, width, text_rows, width, text_rows),
        );
        let white = sprites["sunrise/white"];
        Self {
            pictures: Default::default(),
            revision: Self::next_revision(),
            font: crate::font::Font::load(
                &crate::resource_source::ResourceSource::default(),
                &sprites,
                &pixels,
                (width, height),
                (glyph_top, height - glyph_top),
                white,
            ),
            material_maps: Default::default(),
            galactic_widths: galactic_widths(&sprites, &pixels, width, height),
            pixels,
            packed: None,
            width,
            height,
            sprites,
            translucent: HashSet::new(),
            flags: HashMap::new(),
            gui: HashMap::new(),
            non_gui: HashSet::new(),
            registered: HashMap::new(),
            live: HashMap::new(),
            object_skins: HashMap::new(),
            object_skins_revision: Self::next_revision(),
            sprite_animations: Vec::new(),
            glint: (vec![0; 4], 1, 1),
            glint_armor: (vec![0; 4], 1, 1),
            white,
            text_shaders: Default::default(),
            lines_shader: None,
            lines_expressions: Vec::new(),
            sky_shader: None,
            stars_shader: None,
            sky_expressions: Vec::new(),
            stars_expressions: Vec::new(),
            lightmap_shader: None,
            lightmap_expressions: Vec::new(),
            clouds_shader: None,
            sprite_modifiers: Default::default(),
            random_textures: Default::default(),
            emissive: Default::default(),
            blinks: Default::default(),
            dynamic_texture_pool: None,
        }
    }
    /// Shelf placements of `images` (tallest first) in the narrowest atlas
    /// width from 4096 up that keeps the atlas, glyph rows included, within
    /// `max` texels a side; and the first row free for glyphs. High-resolution
    /// packs fill a 4096-wide atlas past what GPUs allow (Faithful 64x needs
    /// 32768 rows); vanilla's stitcher fails the reload when its atlases don't
    /// fit, and the packs are then turned off (`Minecraft.clearResourcePacksOnError`).
    /// A width where the plain shelves (each image after the last) leave too
    /// much unused is tried again with each image in the first shelf with
    /// room: a pack of thousands of mixed sizes (server packs with custom
    /// items) fits where the shelves' ragged ends would not.
    pub(super) fn layout(images: &[PackedImage], max: u32) -> Result<(u32, Vec<(u32, u32)>, u32)> {
        let mut width = 4096.min(max);
        loop {
            let wide_enough = images.iter().all(|i| i.1 < width);
            if wide_enough {
                for first_fit in [false, true] {
                    let (placements, glyph_region) = Self::shelves(images, width, first_fit);
                    if glyph_region + GLYPH_ROWS <= max {
                        return Ok((width, placements, glyph_region));
                    }
                }
            }
            if width >= max {
                let texels: u64 = images.iter().map(|i| u64::from(i.1) * u64::from(i.2)).sum();
                return Err(format!(
                    "the textures ({} images, {texels} texels) do not fit a {max}x{max} atlas",
                    images.len()
                )
                .into());
            }
            width = (width * 2).min(max);
        }
    }
    /// `layout`'s shelves at one width: the placements and the first row below
    /// them. `first_fit` puts an image in the first shelf that is tall enough
    /// and has room, instead of only the last one.
    fn shelves(images: &[PackedImage], width: u32, first_fit: bool) -> (Vec<(u32, u32)>, u32) {
        // Each shelf: its top row, its height (its first image's) and the next free column.
        let mut shelves: Vec<(u32, u32, u32)> = Vec::new();
        let mut placements = Vec::with_capacity(images.len());
        for (_, w, h, _) in images {
            let open = if first_fit {
                shelves.iter().position(|s| s.1 >= *h && s.2 + w < width)
            } else {
                shelves
                    .len()
                    .checked_sub(1)
                    .filter(|&i| shelves[i].1 >= *h && shelves[i].2 + w < width)
            };
            let i = open.unwrap_or_else(|| {
                let top = shelves.last().map_or(0, |s| s.0 + s.1 + 1);
                shelves.push((top, *h, 0));
                shelves.len() - 1
            });
            placements.push((shelves[i].2, shelves[i].0));
            shelves[i].2 += w + 1;
        }
        let glyph_region = shelves.last().map_or(1, |s| s.0 + s.1 + 1);
        (placements, glyph_region)
    }
    /// Pack the textures under these folders of `textures/`, and for the
    /// `full` atlas the generated sprites and the run-time slots too.
    fn pack(
        root: &ResourceSource,
        defaults: &ResourceSource,
        folders: &[&str],
        full: bool,
        pool: bool,
    ) -> Result<Self> {
        let textures = "assets/minecraft/textures";
        let mut files = Vec::new();
        for folder in folders {
            walk(root, &format!("{textures}/{folder}"), &mut files);
        }
        if folders.contains(&"font") {
            let mut seen: HashSet<String> = files.iter().cloned().collect();
            files.extend(
                bitmap_font_textures(root)
                    .into_iter()
                    .filter(|p| seen.insert(p.clone())),
            );
        }
        // Native and content resources may use any namespace in the resolved pack.
        for namespace in root
            .namespaces("assets")
            .into_iter()
            .filter(|namespace| namespace != "minecraft")
        {
            for folder in ["entity", "mob_effect", "particle", "gui"] {
                if ((folder == "entity" || folder == "mob_effect") && folders.contains(&folder))
                    || (folder == "particle" && folders.contains(&"particle"))
                    || (folder == "gui" && full)
                {
                    walk(
                        root,
                        &format!("assets/{namespace}/textures/{folder}"),
                        &mut files,
                    );
                }
            }
        }
        files.retain(|p| !menu_pictures::menu_picture_only(p));
        // OptiFine random entity variants, under the names their scan gives them (those
        // outside `textures/` are `<ns>:optifine/random/...`).
        let random_textures = if folders.contains(&"entity") {
            sunrise_optifine::random_textures::RandomTextures::scan(root)
        } else {
            Default::default()
        };
        let blinks = if folders.contains(&"entity") {
            sunrise_optifine::blink::Blinks::scan(root, &random_textures)
        } else {
            Default::default()
        };
        let emissive = std::sync::Arc::new(if folders.contains(&"entity") {
            let mut e = sunrise_optifine::emissive::Emissive::scan(root, &random_textures);
            e.pair_beside(root, blinks.files());
            e
        } else {
            Default::default()
        });
        let mut named: HashMap<String, String> = HashMap::new();
        let extra = random_textures
            .files()
            .chain(emissive.files())
            .chain(blinks.files());
        for (file, name) in extra {
            named.insert(file.to_owned(), name.to_owned());
        }
        if !named.is_empty() {
            // A pack's variants can't grow the shared atlas past what a GPU takes: every variant
            // (those beside their texture too) counts, in name order, and those past the budget
            // stay out (their mobs wear the texture itself), not the whole pack.
            // Overlays and blink frames only after every variant.
            let overlays: HashSet<String> = emissive
                .files()
                .chain(blinks.files())
                .map(|(f, _)| f.to_owned())
                .collect();
            let kept = budgeted(root, named.keys(), &overlays, RANDOM_TEXELS);
            if kept.len() < named.len() {
                log::warn!(
                    "{} random entity textures left out of the atlas: past its budget",
                    named.len() - kept.len()
                );
            }
            // A dropped variant beside its texture takes its walked overlay and blink frames
            // (and theirs) with it: nothing would draw them.
            let png = format!("{}.png", emissive.suffix);
            let png = png.as_str();
            let orphans: HashSet<String> = named
                .keys()
                .filter(|p| !kept.contains(*p))
                .filter_map(|p| p.strip_suffix(".png").map(str::to_owned))
                .flat_map(|stem| {
                    ["", "_blink", "_blink2"]
                        .into_iter()
                        .flat_map(move |frame| {
                            [format!("{stem}{frame}.png"), format!("{stem}{frame}{png}")]
                        })
                })
                .filter(|p| !named.contains_key(p))
                .collect();
            files.retain(|p| (!named.contains_key(p) || kept.contains(p)) && !orphans.contains(p));
            // Those beside their texture are walked already.
            let walked: HashSet<&String> = files.iter().collect();
            let mut extra: Vec<String> = kept
                .iter()
                .filter(|p| !walked.contains(p))
                .cloned()
                .collect();
            extra.sort();
            files.extend(extra);
        }
        // A texture's LabPBR maps, as decoded (width, height, RGBA8), for shader packs.
        type Maps = [Option<(u32, u32, Vec<u8>)>; 2];
        let read_maps = full && crate::sprites::labpbr_maps();
        type Decoded = (
            PackedImage,
            Option<TextureFlags>,
            Option<(u32, u32)>,
            bool,
            Option<Box<Maps>>,
        );
        let decode_one = |path: &String| -> Option<Decoded> {
            let name = if let Some(name) = named.get(path) {
                name.clone()
            } else {
                let name = path.strip_suffix(".png")?;
                if let Some(relative) = name.strip_prefix("assets/minecraft/textures/") {
                    relative.to_owned()
                } else {
                    let (namespace, relative) =
                        name.strip_prefix("assets/")?.split_once("/textures/")?;
                    format!("{namespace}:{relative}")
                }
            };
            // LabPBR's maps beside a texture are the world's (sprites.rs), not images to draw.
            if let Some(base) = name.strip_suffix("_n").or_else(|| name.strip_suffix("_s"))
                && root.contains(&crate::atlas_sources::texture_resource(base, "textures"))
            {
                return None;
            }
            let image = decode_resource(root, path)
                .map_err(|e| log::warn!("skipped GUI texture {}: {e}", path))
                .ok()?;
            let mcmeta = root.read_to_string(&format!("{path}.mcmeta")).ok();
            let animation = animation_of(mcmeta.as_deref());
            // An animated strip keeps its first frame.
            let frames =
                animation.is_some() || name.starts_with("block/") || name.starts_with("item/");
            let (w, h, pixels) = if frames {
                first_frame(image, animation.as_ref())
                    .map_err(|e| log::warn!("skipped GUI texture {}: {e}", path))
                    .ok()?
            } else {
                image
            };
            // The default texture's size, when a pack's differs from it.
            let gui_size = png_size(
                defaults,
                &crate::atlas_sources::texture_resource(&name, "textures"),
            )
            .map(|(dw, dh)| {
                let side = dw.min(dh);
                if frames { (side, side) } else { (dw, dh) }
            })
            .filter(|&size| size != (w, h));
            let flags = mcmeta.as_deref().and_then(TextureFlags::from_mcmeta);
            let animated = frames && animation.is_some();
            // Vanilla particle atlases accept sprites of any size.
            let particle = name.starts_with("particle/") || name.contains(":particle/");
            let (gw, gh) = if particle {
                (0, 0)
            } else {
                gui_size.unwrap_or((w, h))
            };
            let maps = read_maps
                .then(|| {
                    let stem = path.strip_suffix(".png")?;
                    let maps: Maps = ["_n", "_s"]
                        .map(|suffix| decode_resource(root, &format!("{stem}{suffix}.png")).ok());
                    maps.iter().any(Option::is_some).then(|| Box::new(maps))
                })
                .flatten();
            (gw <= 1024
                && (gh <= 512
                    || matches!(
                        name.as_str(),
                        "gui/title/background/panorama_0"
                            | "gui/title/background/panorama_2"
                            | "gui/title/background/panorama_3"
                            | "gui/presets/isles"
                    )
                    || name.starts_with("font/")))
            .then_some(((name, w, h, pixels), flags, gui_size, animated, maps))
        };
        let threads = std::thread::available_parallelism().map_or(2, |n| n.get());
        let chunk = files.len().div_ceil(threads).max(1);
        let decoded: Vec<_> = std::thread::scope(|scope| {
            let handles: Vec<_> = files
                .chunks(chunk)
                .map(|part| {
                    scope.spawn(move || part.iter().filter_map(decode_one).collect::<Vec<_>>())
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap_or_default())
                .collect()
        });
        let mut flags = HashMap::new();
        let mut gui_sizes = HashMap::new();
        let mut animated: Vec<String> = Vec::new();
        let mut images: Vec<PackedImage> = Vec::with_capacity(decoded.len());
        let mut maps: HashMap<String, Box<Maps>> = HashMap::new();
        for (image, image_flags, gui_size, is_animated, image_maps) in decoded {
            if let Some(m) = image_maps {
                maps.insert(image.0.clone(), m);
            }
            if let Some(f) = image_flags {
                flags.insert(image.0.clone(), f);
            }
            if let Some(size) = gui_size {
                gui_sizes.insert(image.0.clone(), size);
            }
            if is_animated {
                animated.push(image.0.clone());
            }
            images.push(image);
        }
        // A registered atlas's sprites (`ns:gui/atlases/...`) are packed under keys of their own,
        // so no model's or GUI entry's sprite of the same name can displace or be taken for one.
        for image in &mut images {
            let path = image.0.split_once(':').map_or(image.0.as_str(), |(_, p)| p);
            if !path.starts_with("gui/atlases/") {
                continue;
            }
            let key = format!("{REGISTERED_KEY}{}", image.0);
            let old = std::mem::replace(&mut image.0, key.clone());
            if let Some(value) = flags.remove(&old) {
                flags.insert(key.clone(), value);
            }
            if let Some(value) = gui_sizes.remove(&old) {
                gui_sizes.insert(key.clone(), value);
            }
            if let Some(value) = maps.remove(&old) {
                maps.insert(key.clone(), value);
            }
            for name in animated.iter_mut().filter(|name| **name == old) {
                name.clone_from(&key);
            }
        }
        let mut cit_names = HashSet::new();
        let mut gui_files = HashMap::new();
        let mut gui_names = Vec::new();
        let mut non_gui = HashSet::new();
        if full {
            let found = atlas_sprites(root, &images);
            let dropped = found.dropped;
            images.retain(|image| !dropped.contains(&image.0));
            flags.retain(|name, _| !dropped.contains(name));
            gui_sizes.retain(|name, _| !dropped.contains(name));
            maps.retain(|name, _| !dropped.contains(name));
            animated.retain(|name| !dropped.contains(name));
            images.extend(found.images);
            animated.extend(found.animated);
            flags.extend(found.flags);
            gui_files = found.gui_files;
            gui_names = found.gui;
            non_gui = found.non_gui;
            for (image, image_flags, is_animated) in cit_sprites(root, &images) {
                cit_names.insert(image.0.clone());
                if let Some(f) = image_flags {
                    flags.insert(image.0.clone(), f);
                }
                if is_animated {
                    animated.push(image.0.clone());
                }
                images.push(image);
            }
        }
        images.push(("sunrise/white".into(), 4, 4, vec![255; 64]));
        // The controller's art, unless a pack brought its own (walked above, as a mod's GUI
        // textures are).
        for image in crate::pad_glyph::images().chain(crate::mark_sprite::image()) {
            if !images.iter().any(|i| i.0 == image.0) {
                images.push(image);
            }
        }
        if full {
            // Official Recipes artwork from Just Enough Items, under MIT.
            for (name, png) in crate::jei_sprites::SPRITES {
                let (width, height, rgba) = decode_bytes(png)?;
                images.push((format!("sunrise/jei/{name}"), width, height, rgba));
            }
            Self::slots(&mut images, crate::trims::slot_size(root), pool);
            // MissingTextureAtlasSprite.generateMissingImage: 16x16, magenta
            // and black quarters.
            let missing = (0..256)
                .flat_map(|i| match (i / 16 < 8) ^ (i % 16 < 8) {
                    true => [248, 0, 248, 255],
                    false => [0, 0, 0, 255],
                })
                .collect();
            images.push((MISSING_SPRITE.into(), 16, 16, missing));
        }
        // Blurred sprites are sampled linearly in the atlas (screen.wgsl): a
        // border wrapped from their own edges keeps the filter off their
        // neighbours, as repeat addressing of their own texture would.
        let mut bordered = std::collections::HashSet::new();
        for image in &mut images {
            if flags.get(&image.0).is_some_and(|f: &TextureFlags| f.blur) {
                *image = wrap_border(image);
                bordered.insert(image.0.clone());
            }
        }
        if full {
            let (width, height, rgba) =
                decode_bytes(include_bytes!("../jei_recipes/config_button.png"))?;
            images.push(("sunrise/recipes/settings".to_owned(), width, height, rgba));
        }
        images.sort_by(|a, b| b.2.cmp(&a.2).then(b.1.cmp(&a.1)));
        let (width, placements, glyph_region) =
            Self::layout_leaving_out(&mut images, &mut animated, &cit_names, MAX_ATLAS_SIZE)
                .or_else(|error| -> Result<_> {
                    let models = reducible_model_sprites(root, &images, &bordered);
                    let layout = Self::layout_reducing_models(
                        &mut images,
                        &mut gui_sizes,
                        &models,
                        MAX_ATLAS_SIZE,
                    )?;
                    log::warn!(
                        "reduced HUD model texture resolution to keep the resource pack: {error}"
                    );
                    Ok(layout)
                })?;
        // A multiple of 1024 rather than a power of two: the default resources
        // need 4096x5120 (8192 rows before), Faithful 32x 4096x12288 (16384).
        let height = (glyph_region + GLYPH_ROWS).next_multiple_of(1024);
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let mut sprites = HashMap::new();
        let mut translucent = HashSet::new();
        for ((name, w, h, data), (px, py)) in images.iter().zip(placements) {
            if data
                .as_chunks::<4>()
                .0
                .iter()
                .any(|p| p[3] != 0 && p[3] != 255)
            {
                translucent.insert(name.clone());
            }
            if !data.is_empty() {
                for row in 0..*h {
                    let src = &data[(row * w * 4) as usize..((row + 1) * w * 4) as usize];
                    let dst = (((py + row) * width + px) * 4) as usize;
                    pixels[dst..dst + (w * 4) as usize].copy_from_slice(src);
                }
            }
            let b = u32::from(bordered.contains(name));
            let (px, py, w, h) = (px + b, py + b, w - 2 * b, h - 2 * b);
            let (gw, gh) = gui_sizes.get(name).copied().unwrap_or((w, h));
            sprites.insert(
                name.clone(),
                Rect {
                    u0: px as f32 / width as f32,
                    v0: py as f32 / height as f32,
                    u1: (px + w) as f32 / width as f32,
                    v1: (py + h) as f32 / height as f32,
                    w: gw,
                    h: gh,
                },
            );
        }
        // Every animated sprite animates: the packed strip is the raw first
        // frame, while the mcmeta's frame list may start anywhere, so each
        // animation's frame(0) overwrites its atlas rect before drawing.
        let mut loaded = crate::sprites::Sprites::new(root.clone());
        let mut sprite_animations = Vec::new();
        animated.sort();
        for name in animated {
            let Some(rect) = sprites.get(&name).copied() else {
                continue;
            };
            // A registered atlas's sprite is named by its public name under its key.
            let registered = name.strip_prefix(REGISTERED_KEY);
            let public = registered.unwrap_or(&name);
            // Another namespace's sprite is named with it (`lobby:item/...`).
            let id = if public.contains(':') {
                public.to_owned()
            } else {
                format!("minecraft:{public}")
            };
            // A GUI sprite animates the file it was made from, an alias's source included; a
            // registered atlas's sprite its staged file, never a model's origin of the name; a
            // block or item sprite is as its atlas finishes it; the other atlases' are files.
            let Ok(layer) = (match (gui_files.get(&name), registered) {
                (Some(file), _) => loaded.insert_file(file),
                (None, Some(_)) => {
                    loaded.insert_file(&crate::atlas_sources::texture_resource(&id, "textures"))
                }
                (None, None) => loaded.try_insert(&id).or_else(|_| {
                    loaded.insert_file(&crate::atlas_sources::texture_resource(&id, "textures"))
                }),
            }) else {
                continue;
            };
            let Some(animation) = loaded.animations.iter().find(|a| a.layer == layer).cloned()
            else {
                continue;
            };
            let (tw, th) = rect.texels((width, height));
            let image = animation.resized_pixels(animation.frame(0), tw, th);
            let x = (rect.u0 * width as f32).round() as usize;
            let y = (rect.v0 * height as f32).round() as usize;
            let row_bytes = tw as usize * 4;
            for row in 0..th as usize {
                let dst = ((y + row) * width as usize + x) * 4;
                let src = row * row_bytes;
                pixels[dst..dst + row_bytes].copy_from_slice(&image[src..src + row_bytes]);
            }
            sprite_animations.push((rect, animation));
        }
        // Each mapped texture's maps at its size in the atlas, paged.
        let material_maps = if maps.is_empty() {
            Default::default()
        } else {
            let mapped: Vec<crate::paged_maps::SpriteMaps> = maps
                .iter()
                .filter_map(|(name, m)| {
                    let rect = sprites.get(name)?;
                    let (tw, th) = rect.texels((width, height));
                    let x = (rect.u0 * width as f32).round() as u32;
                    let y = (rect.v0 * height as f32).round() as u32;
                    Some(crate::paged_maps::SpriteMaps {
                        rect: [x, y, tw, th],
                        maps: m.each_ref().map(|map| {
                            let (mw, mh, pixels) = map.as_ref()?;
                            crate::paged_maps::fit(pixels, (*mw, *mh), (tw, th))
                        }),
                    })
                })
                .collect();
            crate::paged_maps::build((width, height), &mapped, MAX_ATLAS_SIZE)
        };
        let white = sprites["sunrise/white"];
        let glint = Self::glint_texture(root, defaults, "enchanted_glint_item");
        let glint_armor = Self::glint_texture(root, defaults, "enchanted_glint_armor");
        let font = crate::font::Font::load_over(
            root,
            defaults,
            &sprites,
            &pixels,
            (width, height),
            (glyph_region, height - glyph_region),
            white,
        );
        // The GUI atlas's sprites packed apart because a model has the name are found here only.
        let gui: HashMap<String, GuiSprite> = gui_names
            .into_iter()
            .map(|(id, published)| {
                let sprite = match published {
                    GuiPublished::Packed(key, scaling) => {
                        sprites
                            .get(&key)
                            .map_or(GuiSprite::Missing, |rect| GuiSprite::Present {
                                rect: *rect,
                                scaling,
                            })
                    }
                    GuiPublished::Missing => GuiSprite::Missing,
                };
                (id, sprite)
            })
            .collect();
        let registered: HashMap<String, Rect> = sprites
            .iter()
            .filter_map(|(key, rect)| Some((key.strip_prefix(REGISTERED_KEY)?.to_owned(), *rect)))
            .collect();
        sprites.retain(|name, _| !name.starts_with(GUI_KEY) && !name.starts_with(REGISTERED_KEY));
        // A registered sprite no packed sprite shares its name with keeps its sampling flags under
        // the public name, where a raw texture lookup finds both.
        for name in registered
            .keys()
            .filter(|name| !sprites.contains_key(*name))
        {
            if let Some(value) = flags.remove(&format!("{REGISTERED_KEY}{name}")) {
                flags.insert(name.clone(), value);
            }
        }
        Ok(Self {
            pictures: std::sync::Mutex::new(pictures::Cache::new(root, defaults)),
            revision: Self::next_revision(),
            material_maps: std::sync::Arc::new(material_maps),
            galactic_widths: galactic_widths(&sprites, &pixels, width, height),
            pixels,
            packed: None,
            width,
            height,
            sprites,
            gui,
            non_gui,
            registered,
            live: HashMap::new(),
            object_skins: HashMap::new(),
            object_skins_revision: Self::next_revision(),
            translucent,
            flags,
            sprite_animations,
            glint,
            glint_armor,
            text_shaders: Default::default(),
            lines_shader: None,
            lines_expressions: Vec::new(),
            sky_shader: None,
            stars_shader: None,
            sky_expressions: Vec::new(),
            stars_expressions: Vec::new(),
            lightmap_shader: None,
            lightmap_expressions: Vec::new(),
            clouds_shader: None,
            sprite_modifiers: Default::default(),
            random_textures,
            emissive,
            blinks,
            font,
            white,
            dynamic_texture_pool: None,
        })
    }
    /// `misc/<name>.png` on its own, or a transparent texel if the resource
    /// pack lacks it.
    fn glint_texture(
        root: &ResourceSource,
        defaults: &ResourceSource,
        name: &str,
    ) -> (Vec<u8>, u32, u32) {
        let relative = format!("assets/minecraft/textures/misc/{name}.png");
        let source = if root.contains(&relative) {
            root
        } else {
            defaults
        };
        decode_resource(source, &relative)
            .map_or((vec![0; 4], 1, 1), |(w, h, pixels)| (pixels, w, h))
    }
}
