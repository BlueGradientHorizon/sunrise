//! 2D overlay: one atlas holding GUI sprites, the bitmap font, item and block
//! textures, and a batch of textured quads in GUI pixels (vanilla's automatic
//! GUI scale is applied when drawing).
use crate::{Result, resource_source::ResourceSource};
use smallvec::SmallVec;
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};
use sunrise_assets::animation::AnimationMetadata;
use wgpu::util::DeviceExt;

use model_reducing::reducible_model_sprites;

mod animate;
pub mod cit_glint;
mod clip_corner;
pub mod dynamic_textures;
mod gui_sprite;
pub use gui_sprite::atlas_key_of_gui_sprite;
mod menu_pictures;
mod model_reducing;
pub(crate) mod pack_text;
pub mod pictures;
pub(crate) mod sprite_modifiers;

/// GUI sprite ids a draw asked for that the atlas lacks, each warned about once.
static MISSING_GUI_SPRITES: std::sync::Mutex<BTreeSet<String>> =
    std::sync::Mutex::new(BTreeSet::new());

#[cfg(test)]
thread_local! {
    static GUI_SPRITE_MISSES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The GUI sprite ids this thread's draws missed since the last call, for tests that a screen names only real sprites.
#[cfg(test)]
pub(crate) fn take_gui_sprite_misses() -> Vec<String> {
    GUI_SPRITE_MISSES.with(|misses| std::mem::take(&mut *misses.borrow_mut()))
}

/// A draw named a GUI sprite the atlas lacks (a wrong name, a pack's filter, a mod's id): one warning per id.
fn missing_gui_sprite(id: &str) {
    #[cfg(test)]
    GUI_SPRITE_MISSES.with(|misses| misses.borrow_mut().push(id.to_owned()));
    let mut warned = MISSING_GUI_SPRITES
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if warned.insert(id.to_owned()) {
        log::warn!("no GUI sprite {id:?} in the atlas; drawing the missing texture");
    }
}

/// A sprite's atlas region, and its size in GUI pixels: the size of the
/// default resources' texture of that name, whatever the resolution of the
/// pack that replaced it (vanilla's GUI code draws in those units: `blit`
/// divides by the texture size it is given, sprites by their `gui.scaling`
/// size). Textures the default resources lack keep their own size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub w: u32,
    pub h: u32,
}
impl Rect {
    /// The region's size in texels of an atlas this large.
    pub fn texels(&self, (width, height): (u32, u32)) -> (u32, u32) {
        (
            ((self.u1 - self.u0) * width as f32).round() as u32,
            ((self.v1 - self.v0) * height as f32).round() as u32,
        )
    }
}
/// `TextureMetadataSection`'s `blur` and `clamp`: how vanilla samples a
/// standalone texture (`ReloadableTexture.setSampler`: linear rather than
/// nearest filtering, clamp-to-edge rather than repeat addressing).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextureFlags {
    pub blur: bool,
    pub clamp: bool,
}
impl TextureFlags {
    /// The `texture` section of a `.png.mcmeta` file, if it sets either flag.
    pub fn from_mcmeta(json: &str) -> Option<Self> {
        let meta: serde_json::Value = crate::packs::first_json(json.as_bytes()).ok()?;
        let section = meta.get("texture")?;
        let flag = |key| section.get(key).and_then(serde_json::Value::as_bool) == Some(true);
        let flags = Self {
            blur: flag("blur"),
            clamp: flag("clamp"),
        };
        (flags != Self::default()).then_some(flags)
    }
}
pub struct HudAtlas {
    pictures: std::sync::Mutex<pictures::Cache>,
    /// Identity of this packed atlas, used to invalidate cached model UV data
    /// even when a resource reload places sprites at the same coordinates.
    pub revision: u64,
    /// RGBA8, `width` by `height`, as built: empty once `pack_pixels` has
    /// compressed them (read them through `pixels()`).
    pub pixels: Vec<u8>,
    /// `pixels` compressed (lz4) on a thread of its own once the renderer
    /// has uploaded them: only rebuilding a renderer reads them again.
    packed: Option<Arc<OnceLock<Vec<u8>>>>,
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
pub fn decode(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    decode_bytes(&sunrise_core::prepared_files::read(path)?)
}
/// The image formats [`decode_image_bytes`] reads, told apart by their own
/// leading bytes rather than by a file name: what a server sends has no name,
/// and a name would not be trustworthy anyway.
///
/// The game's own resources are PNG throughout, and so are a resource pack's
/// (vanilla's `NativeImage.read` takes PNG only), so [`decode_bytes`] stays
/// PNG-only. WebP and JPEG are for images fetched from the network (Modrinth
/// project icons in the resource pack browser), which are commonly either.
enum ImageFormat {
    Png,
    /// `RIFF....WEBP`.
    WebP,
    /// Starts with the SOI marker `FF D8`.
    Jpeg,
}

impl ImageFormat {
    /// The format `data` begins with, or `None` for anything else.
    fn sniff(data: &[u8]) -> Option<Self> {
        const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        if data.starts_with(&PNG_MAGIC) {
            return Some(Self::Png);
        }
        if data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
            return Some(Self::WebP);
        }
        if data.starts_with(&[0xFF, 0xD8]) {
            return Some(Self::Jpeg);
        }
        None
    }
}

/// The most pixels a fetched image may claim: its header alone sizes the
/// buffer, and a hostile one could ask for gigabytes. 4096 x 4096 RGBA is 64 MiB.
const MAX_FETCHED_PIXELS: u64 = 4096 * 4096;

fn check_fetched_size(width: u64, height: u64) -> Result<()> {
    if width * height > MAX_FETCHED_PIXELS {
        return Err(format!("a {width}x{height} image is too large to decode").into());
    }
    Ok(())
}

/// A pack image may claim no side an atlas would refuse, nor more pixels than
/// the largest atlas holds: the PNG reader alone would trust the header and
/// allocate it, as its own limit covers one row.
pub(crate) fn check_pack_size(width: u32, height: u32) -> Result<()> {
    use crate::atlas_sources::MAX_SPRITE_SIDE;
    if i64::from(width.max(height)) > MAX_SPRITE_SIDE {
        return Err(OverSizeLimit(format!(
            "a {width}x{height} image exceeds the {MAX_SPRITE_SIDE}-pixel sprite side"
        ))
        .into());
    }
    if u64::from(width) * u64::from(height) > u64::from(MAX_ATLAS_SIZE).pow(2) {
        return Err(OverSizeLimit(format!(
            "a {width}x{height} image has more pixels than a {MAX_ATLAS_SIZE}x{MAX_ATLAS_SIZE} atlas"
        ))
        .into());
    }
    Ok(())
}

/// The most bytes any one buffer of a pack image may take (the decoder's, its RGBA copy, its
/// frames made square): 8192x8192 RGBA, 256 MiB. That is far past the panorama faces and
/// animated strips packs ship, where the 16384x16384 pixels [`check_pack_size`] lets through
/// would hold 1 GiB per buffer, two buffers per image and several images loading in parallel.
pub(crate) const MAX_IMAGE_BYTES: usize = 256 << 20;

/// A pack image refused for its size: past a side, pixel or byte limit, or a buffer the
/// allocator would not give.
#[derive(Debug)]
pub struct OverSizeLimit(String);

impl std::fmt::Display for OverSizeLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for OverSizeLimit {}

/// The bytes of a buffer the product of `factors` long, refused (an overflow too) past
/// `budget`.
pub(crate) fn image_bytes(factors: &[usize], budget: usize) -> Result<usize> {
    factors
        .iter()
        .try_fold(1usize, |bytes, &factor| bytes.checked_mul(factor))
        .filter(|&bytes| bytes <= budget)
        .ok_or_else(|| {
            OverSizeLimit(format!(
                "an image buffer of {factors:?} is over the {budget}-byte size limit"
            ))
            .into()
        })
}

/// An empty buffer with room for the product of `factors` values of `T`, refused past `budget`
/// bytes or when the allocator refuses it: a load error rather than an abort.
pub(crate) fn image_buffer<T>(factors: &[usize], budget: usize) -> Result<Vec<T>> {
    let bytes = image_bytes(&[factors, &[size_of::<T>()]].concat(), budget)?;
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(bytes / size_of::<T>())
        .map_err(|e| OverSizeLimit(format!("an image buffer of {bytes} bytes: {e}")))?;
    Ok(buffer)
}

/// A WebP image as RGBA8. Still frames and animations both decode; an
/// animation gives its first frame, as an animated PNG would.
fn decode_webp(data: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let mut decoder =
        image_webp::WebPDecoder::new(std::io::Cursor::new(data)).map_err(|e| e.to_string())?;
    let (width, height) = decoder.dimensions();
    check_fetched_size(u64::from(width), u64::from(height))?;
    // Four bytes a pixel whether or not the image carries alpha: the caller
    // always wants RGBA.
    let mut rgba = vec![0u8; (width as usize) * (height as usize) * 4];
    if decoder.has_alpha() {
        decoder.read_image(&mut rgba).map_err(|e| e.to_string())?;
    } else {
        let mut rgb = vec![0u8; (width as usize) * (height as usize) * 3];
        decoder.read_image(&mut rgb).map_err(|e| e.to_string())?;
        for (out, pixel) in rgba
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(rgb.as_chunks::<3>().0)
        {
            out.copy_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
        }
    }
    Ok((width, height, rgba))
}

/// A JPEG image as RGBA8. JPEG has no alpha, so every pixel is opaque.
fn decode_jpeg(data: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let mut decoder = zune_jpeg::JpegDecoder::new(data);
    // The size comes from the headers, before any pixel buffer is made.
    decoder.decode_headers().map_err(|e| e.to_string())?;
    let info = decoder.info().ok_or("the JPEG states no size")?;
    let (width, height) = (u32::from(info.width), u32::from(info.height));
    check_fetched_size(u64::from(width), u64::from(height))?;
    let pixels = decoder.decode().map_err(|e| e.to_string())?;
    let count = (width as usize) * (height as usize);
    // The decoder gives RGB for a colour image and one byte a pixel for a
    // greyscale one; both become RGBA here.
    let channels = pixels.len().checked_div(count).unwrap_or(0);
    let mut rgba = Vec::with_capacity(count * 4);
    match channels {
        3 => {
            for p in pixels.as_chunks::<3>().0 {
                rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
            }
        }
        1 => {
            for p in &pixels {
                rgba.extend_from_slice(&[*p, *p, *p, 255]);
            }
        }
        _ => return Err("the JPEG decoded to an unexpected channel count".into()),
    }
    Ok((width, height, rgba))
}

/// An image fetched from the network (PNG, WebP or JPEG) as RGBA pixels. WebP
/// and JPEG go to their own decoders; anything else goes to the PNG reader, so
/// its error is the one reported.
pub fn decode_image_bytes(data: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    match ImageFormat::sniff(data) {
        Some(ImageFormat::WebP) => decode_webp(data),
        Some(ImageFormat::Jpeg) => decode_jpeg(data),
        // A PNG's IHDR states the size before the decoder sizes its pixel
        // buffer, so it is checked there like the WebP and JPEG headers: the
        // PNG reader alone would trust the header and allocate it.
        Some(ImageFormat::Png) | None => {
            if let Some((width, height)) = png_bytes_size(data) {
                check_fetched_size(u64::from(width), u64::from(height))?;
            }
            decode_bytes(data)
        }
    }
}

/// A PNG in memory as RGBA pixels.
pub fn decode_bytes(data: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    decode_bytes_within(data, MAX_IMAGE_BYTES)
}

/// [`decode_bytes`], each of its buffers held to `budget` bytes.
fn decode_bytes_within(data: &[u8], budget: usize) -> Result<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(data));
    decoder.ignore_checksums(true);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    check_pack_size(reader.info().width, reader.info().height)?;
    let size = reader.output_buffer_size().unwrap_or(usize::MAX);
    let mut bytes = image_buffer(&[size], budget)?;
    bytes.resize(size, 0);
    let info = reader.next_frame(&mut bytes)?;
    let channels = info.color_type.samples();
    let mut rgba = image_buffer(&[info.width as usize, info.height as usize, 4], budget)?;
    for p in bytes[..info.buffer_size()].chunks_exact(channels) {
        rgba.extend(match channels {
            4 => [p[0], p[1], p[2], p[3]],
            3 => [p[0], p[1], p[2], 255],
            2 => [p[0], p[0], p[0], p[1]],
            _ => [p[0], p[0], p[0], 255],
        });
    }
    Ok((info.width, info.height, rgba))
}
/// The most texels of OptiFine random entity variants the entity atlas packs: 2048 rows of
/// its 4096, which keeps the default atlas (4096x5120) under the 8192 rows many GPUs take.
const RANDOM_TEXELS: u64 = 4096 * 2048;
/// The variant images (and `overlays`) the atlas packs: every variant before any overlay, by
/// name, while their sizes add up to at most `budget` texels. A full budget drops glows before
/// looks.
fn budgeted<'a>(
    source: &ResourceSource,
    files: impl Iterator<Item = &'a String>,
    overlays: &HashSet<String>,
    budget: u64,
) -> HashSet<String> {
    let mut files: Vec<&String> = files.collect();
    files.sort_by_key(|p| (overlays.contains(*p), *p));
    within_budget(source, &files, budget)
}
/// The `files` (in order) whose sizes add up to at most `budget` texels.
fn within_budget(source: &ResourceSource, files: &[&String], budget: u64) -> HashSet<String> {
    let mut texels = 0u64;
    files
        .iter()
        .filter(|p| {
            let (w, h) = png_size(source, p).unwrap_or((0, 0));
            texels += u64::from(w) * u64::from(h);
            texels <= budget
        })
        .map(|p| (*p).clone())
        .collect()
}
/// A PNG's width and height from its first 24 bytes (`IHDR`), before any
/// pixel buffer is made.
pub fn png_bytes_size(data: &[u8]) -> Option<(u32, u32)> {
    let header = data.get(..24)?;
    if header[..8] != *b"\x89PNG\r\n\x1a\n" || header[12..16] != *b"IHDR" {
        return None;
    }
    let field = |at: usize| u32::from_be_bytes(header[at..at + 4].try_into().unwrap_or_default());
    Some((field(16), field(20)))
}
/// A PNG's width and height, from its header alone.
fn png_size(source: &ResourceSource, path: &str) -> Option<(u32, u32)> {
    png_bytes_size(&source.read_head(path, 24).ok()?)
}
fn walk(source: &ResourceSource, directory: &str, out: &mut Vec<String>) {
    out.extend(
        source
            .list(&format!("{directory}/"))
            .into_iter()
            .filter(|name| name.ends_with(".png")),
    );
}
pub(crate) fn decode_resource(source: &ResourceSource, name: &str) -> Result<(u32, u32, Vec<u8>)> {
    decode_bytes(&source.read(name)?)
}
/// A packed atlas image: sprite name, size and RGBA pixels.
type PackedImage = (String, u32, u32, Vec<u8>);
/// A frame at `x, y`, sampled as if squared to its longer side and then resized
/// to `output_side`, without making the intermediate square. The output is held
/// to `budget` bytes before it is made.
pub(crate) fn strip_frame(
    pixels: &[u8],
    stride: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    output_side: u32,
    budget: usize,
) -> Result<Vec<u8>> {
    let side = w.max(h);
    let mut out = image_buffer(&[output_side as usize, output_side as usize, 4], budget)?;
    // Keep the two nearest-neighbour roundings when shrinking a rectangular frame.
    let index = |i| {
        if output_side == side {
            i
        } else {
            i * side / output_side
        }
    };
    for row in 0..output_side {
        let source_row = y + index(row) * h / side;
        for column in 0..output_side {
            let at = ((source_row * stride + x + index(column) * w / side) * 4) as usize;
            out.extend_from_slice(&pixels[at..at + 4]);
        }
    }
    Ok(out)
}
/// The `animation` section of a texture's `.mcmeta`.
pub(crate) fn animation_of(mcmeta: Option<&str>) -> Option<AnimationMetadata> {
    let meta: serde_json::Value = crate::packs::first_json(mcmeta?.as_bytes()).ok()?;
    AnimationMetadata::from_json(&meta.get("animation")?.to_string()).ok()
}
/// An animated strip's first frame: the region the mcmeta's frame size picks
/// (`AnimationMetadataSection.frameSize`), which is the top-left square unless
/// it names other dimensions or the frames run along a row.
fn first_frame(
    (w, h, pixels): (u32, u32, Vec<u8>),
    animation: Option<&AnimationMetadata>,
) -> Result<(u32, u32, Vec<u8>)> {
    let [fw, fh] = animation.map_or([w.min(h) as i32; 2], |meta| {
        meta.frame_size(w as i32, h as i32)
    });
    match (u32::try_from(fw), u32::try_from(fh)) {
        (Ok(fw), Ok(fh))
            if fw > 0 && fh > 0 && w % fw == 0 && h % fh == 0 && (fw, fh) != (w, h) =>
        {
            let side = fw.max(fh);
            let frame = strip_frame(&pixels, w, 0, 0, fw, fh, side, MAX_IMAGE_BYTES)?;
            Ok((side, side, frame))
        }
        _ => Ok((w, h, pixels)),
    }
}
/// `image` inside a one-texel border copied from its opposite edges, the
/// texels repeat addressing reads past each edge.
fn wrap_border((name, w, h, data): &PackedImage) -> PackedImage {
    let (bw, bh) = (w + 2, h + 2);
    let mut out = Vec::with_capacity((bw * bh * 4) as usize);
    for y in 0..bh {
        let row = (y + h - 1) % h;
        for x in 0..bw {
            let at = ((row * w + (x + w - 1) % w) * 4) as usize;
            out.extend_from_slice(&data[at..at + 4]);
        }
    }
    (name.clone(), bw, bh, out)
}
/// The textures the enabled packs' Custom Item Textures rules draw, beyond those already packed:
/// item rules' textures and their JSON models', armour and elytra rules' textures, under their
/// `cit_assets` ids, with their flags and whether they animate. An animated strip keeps its
/// first frame; a still texture is packed whole. None without `optifine/cit` rules.
fn cit_sprites(
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
/// The texture of every `bitmap` font provider. A provider's `file` may name
/// any texture, so a pack's glyph sheet need not live under `textures/font`.
fn bitmap_font_textures(root: &ResourceSource) -> Vec<String> {
    let defs = root
        .namespaces("assets")
        .into_iter()
        .flat_map(|ns| root.list(&format!("assets/{ns}/font/")))
        .filter(|path| path.ends_with(".json"));
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for def in defs {
        let Ok(bytes) = root.read(&def) else { continue };
        let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
        let Ok(json) = serde_json::from_slice::<serde_json::Value>(bytes) else {
            continue;
        };
        for p in json["providers"].as_array().into_iter().flatten() {
            if p["type"] != "bitmap" {
                continue;
            }
            let Some(file) = p["file"].as_str() else {
                continue;
            };
            let (ns, path) = file.split_once(':').unwrap_or(("minecraft", file));
            let path = format!("assets/{ns}/textures/{path}");
            if root.contains(&path) && seen.insert(path.clone()) {
                out.push(path);
            }
        }
    }
    out
}

/// Folders of `assets/minecraft/textures/` that [`HudAtlas::pack`] walks for
/// the full atlas. A pack may add others through `atlases/blocks.json`.
const WALKED_TEXTURE_FOLDERS: &[&str] = &[
    "gui",
    "item",
    "block",
    "font",
    "environment",
    "misc",
    "painting",
    "entity",
    "particle",
    "mob_effect",
    "map",
];

fn walked_minecraft_texture(name: &str) -> bool {
    WALKED_TEXTURE_FOLDERS
        .iter()
        .any(|folder| name.starts_with(folder) && name.as_bytes().get(folder.len()) == Some(&b'/'))
}

/// Whether the file a walk packed under its own name tiles into its animation's frames.
fn walked_frames_fit(root: &ResourceSource, texture: &str) -> bool {
    let file = crate::atlas_sources::texture_resource(texture, "textures");
    let mcmeta = root.read_to_string(&format!("{file}.mcmeta")).ok();
    png_size(root, &file).is_some_and(|size| {
        crate::atlas_sources::frames_fit(size, animation_of(mcmeta.as_deref()).as_ref())
    })
}
/// How the GUI atlas publishes one of its ids.
enum GuiPublished {
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
struct AtlasSprites {
    /// The sprites to pack, by packed name.
    images: Vec<PackedImage>,
    /// The packed names that animate.
    animated: Vec<String>,
    /// The packed names to drop.
    dropped: HashSet<String>,
    /// The render flags of the sprites added.
    flags: HashMap<String, TextureFlags>,
    /// The resource file each GUI sprite added from a file takes its animation from.
    gui_files: HashMap<String, String>,
    /// The GUI atlas's own answer for the names a model or a removal makes ambiguous: the packed
    /// name its pixels are under, or none for a sprite its list removed or failed.
    gui: Vec<(String, GuiPublished)>,
    /// The packed names no GUI lookup of an unlisted id answers with: the model atlases' sprites
    /// and the walked files a GUI entry stands on.
    non_gui: HashSet<String>,
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
fn atlas_sprites(root: &ResourceSource, packed: &[PackedImage]) -> AtlasSprites {
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
const GUI_KEY: &str = "#gui:";
/// The prefix of the name a registered atlas's sprite is packed under.
const REGISTERED_KEY: &str = "#registered:";

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
    fn gui_present(&self, id: &str) -> Option<(&Rect, Option<crate::ui::NineSlice>)> {
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
    fn slots(images: &mut Vec<PackedImage>, trim: (u32, u32), pool: bool) {
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
    fn layout(images: &[PackedImage], max: u32) -> Result<(u32, Vec<(u32, u32)>, u32)> {
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
                decode_bytes(include_bytes!("jei_recipes/config_button.png"))?;
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
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HudVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: u32,
    pub depth: f32,
    /// The `GLINT_TEXTURING` UV into the repeating glint texture, unused
    /// unless `uv.x >= 2`.
    pub glint_uv: [f32; 2],
    /// 1 for the GUI's text (`RenderPipelines.GUI_TEXT`), which a resource
    /// pack's text shader draws when it has one.
    pub text: u32,
}
/// Sutherland-Hodgman against one half plane, interpolating every attribute.
fn clip_polygon(poly: &[HudVertex], axis: usize, limit: f32, keep_below: bool) -> Vec<HudVertex> {
    let inside = |v: &HudVertex| {
        if keep_below {
            v.pos[axis] < limit
        } else {
            v.pos[axis] >= limit
        }
    };
    let lerp = |a: &HudVertex, b: &HudVertex, t: f32| {
        let mix = |p: f32, q: f32| p + (q - p) * t;
        let mut v = *a;
        v.pos = [mix(a.pos[0], b.pos[0]), mix(a.pos[1], b.pos[1])];
        v.uv = [mix(a.uv[0], b.uv[0]), mix(a.uv[1], b.uv[1])];
        v.depth = mix(a.depth, b.depth);
        // A shaded shape (a gradient, `hud_kit::shapes`) keeps its colours across the cut.
        v.color = crate::hud_kit::shapes::mix_colour(a.color, b.color, t);
        v.glint_uv = [
            mix(a.glint_uv[0], b.glint_uv[0]),
            mix(a.glint_uv[1], b.glint_uv[1]),
        ];
        v.pos[axis] = limit;
        v
    };
    let mut out = Vec::with_capacity(poly.len() + 2);
    for i in 0..poly.len() {
        let (a, b) = (&poly[i], &poly[(i + 1) % poly.len()]);
        let (ia, ib) = (inside(a), inside(b));
        if ia {
            out.push(*a);
        }
        if ia != ib {
            let t = (limit - a.pos[axis]) / (b.pos[axis] - a.pos[axis]);
            out.push(lerp(a, b, t));
        }
    }
    out
}
/// Scale inputs for one view, copied from its owner's settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuiScale {
    pub requested: u32,
    pub unicode: bool,
}

/// One frame of overlay geometry in GUI pixels.
pub struct HudBatch {
    pub pictures: Vec<Arc<pictures::Picture>>,
    pub vertices: Vec<HudVertex>,
    pub scale: f32,
    pub width: f32,
    pub height: f32,
    /// Glyph pixels to upload into the atlas before drawing.
    pub patches: Vec<crate::font::Patch>,
    /// Client mods' dynamic textures to upload into their atlas slots before drawing, sharing
    /// the pixels the game keeps of them.
    pub image_patches: Vec<dynamic_textures::ImagePatch>,
    /// Vertex ranges with another blend or depth/scissor state, in order.
    pub blends: Vec<(std::ops::Range<u32>, Blend)>,
    /// The textures and time of the `Blend::EndPortal` runs.
    pub end_portal: EndPortalFill,
    /// `options.glintStrength` for the glint pass (`GlintAlpha`).
    pub glint_alpha: f32,
    /// PvP & HUD's glint colour (`pvp::Tweaks::glint_texel`), 0 for vanilla's.
    pub glint_colour: u32,
    /// A resource pack text shader's `GameTime`: the day's fraction
    /// (`gameTime % 24000` and the partial tick, over 24000).
    pub game_time: f32,
    /// Screen's first vertex: the preceding HUD is part of its blurred background.
    pub blur_at: u32,
    /// Player object profiles encountered while painting text this frame.
    pub inline_profiles: Vec<std::sync::Arc<serde_json::Value>>,
}
/// How a run of HUD vertices blends and tests depth.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Blend {
    Picture(u64),
    /// `BlendFunction.TRANSLUCENT`.
    Alpha,
    /// `BlendFunction.INVERT` (`RenderPipelines.CROSSHAIR`, `GUI_INVERT`).
    Invert,
    /// `BlendFunction.ADDITIVE` (`RenderPipelines.GUI_TEXT_HIGHLIGHT`).
    Additive,
    /// Alpha blending with an isolated depth buffer and a pixel scissor.
    Depth([u32; 4]),
    /// The glint over the `Depth` run before it with the same scissor, in its
    /// pass: `RenderPipelines.GLINT`'s additive blend, depth tested against the
    /// model and never written.
    DepthGlint([u32; 4]),
    /// `RenderPipelines.VIGNETTE`: the destination darkened by the texture
    /// (`ZERO, ONE_MINUS_SRC_COLOR`).
    Vignette,
    /// `RenderPipelines.END_PORTAL` as a GUI fill: the End portal's layers
    /// projected from the screen (`HudBatch::end_portal` sets the textures).
    EndPortal,
    /// An OptiFine CIT glint layer (`cit_glint`), blended as entry `n` of
    /// `GlintTextures::blends` says.
    CitGlint(u8),
}
/// What a `Blend::EndPortal` fill samples: the atlas rects (u0, v0, width,
/// height) of `end_sky.png` and `end_portal.png`, and the `GameTime` uniform.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EndPortalFill {
    pub sprites: [[f32; 4]; 2],
    pub game_time: f32,
}
pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    (a as u32) << 24 | (b as u32) << 16 | (g as u32) << 8 | r as u32
}
impl HudBatch {
    /// Window.calculateScale: the largest scale keeping >= 320x240 GUI pixels,
    /// capped by the GUI Scale option (0 = auto).
    pub fn new(window: [u32; 2], inputs: GuiScale) -> Self {
        let gui =
            sunrise_client_input::window::GuiSize::new(window, inputs.requested, inputs.unicode);
        Self::with_gui_size(gui)
    }
    /// An offscreen or test batch, independent of the active view's settings.
    pub fn scratch(window: [u32; 2]) -> Self {
        Self::with_gui_size(sunrise_client_input::window::GuiSize::new(window, 0, false))
    }
    fn with_gui_size(gui: sunrise_client_input::window::GuiSize) -> Self {
        Self {
            pictures: Vec::new(),
            vertices: Vec::new(),
            scale: gui.scale as f32,
            width: gui.size[0] as f32,
            height: gui.size[1] as f32,
            patches: Vec::new(),
            image_patches: Vec::new(),
            blends: Vec::new(),
            end_portal: EndPortalFill::default(),
            glint_alpha: 0.75,
            glint_colour: 0,
            game_time: 0.0,
            blur_at: 0,
            inline_profiles: Vec::new(),
        }
    }
    /// Draw the vertices added since `start` with the inverting blend.
    pub fn invert_since(&mut self, start: usize) {
        self.blend_since(start, Blend::Invert);
    }
    /// Draw the vertices added since `start` with `blend`.
    pub fn blend_since(&mut self, start: usize, blend: Blend) {
        let range = start as u32..self.vertices.len() as u32;
        if range.is_empty() || blend == Blend::Alpha {
            return;
        }
        match self.blends.last_mut() {
            Some((last, b)) if last.end == range.start && *b == blend => last.end = range.end,
            _ => self.blends.push((range, blend)),
        }
    }
    /// `GuiGraphicsExtractor.textHighlight`: with `invert_text` the area's
    /// colours are inverted first (white text on a dark field turns blue on
    /// white), then blue is added.
    pub fn text_highlight(
        &mut self,
        atlas: &HudAtlas,
        (x, y, w, h): (f32, f32, f32, f32),
        invert_text: bool,
    ) {
        if invert_text {
            let start = self.vertices.len();
            self.fill(atlas, x, y, w, h, rgba(255, 255, 255, 255));
            self.invert_since(start);
        }
        let start = self.vertices.len();
        self.fill(atlas, x, y, w, h, rgba(0, 0, 255, 255));
        self.blend_since(start, Blend::Additive);
    }
    /// The vertex ranges in order, each with its blend.
    pub fn runs(&self) -> Vec<(std::ops::Range<u32>, Blend)> {
        let mut out = Vec::new();
        let mut at = 0;
        for (r, blend) in &self.blends {
            if r.start > at {
                out.push((at..r.start, Blend::Alpha));
            }
            out.push((r.clone(), *blend));
            at = r.end;
        }
        let end = self.vertices.len() as u32;
        if end > at {
            out.push((at..end, Blend::Alpha));
        }
        out
    }
    fn runs_in(
        &self,
        vertices: std::ops::Range<u32>,
    ) -> SmallVec<[(std::ops::Range<u32>, Blend); 16]> {
        let mut runs = SmallVec::new();
        let mut at = vertices.start;
        for (range, blend) in &self.blends {
            if range.start > at {
                let start = at.max(vertices.start);
                let end = range.start.min(vertices.end);
                if start < end {
                    runs.push((start..end, Blend::Alpha));
                }
            }
            let start = range.start.max(vertices.start);
            let end = range.end.min(vertices.end);
            if start < end {
                runs.push((start..end, *blend));
            }
            at = at.max(range.end);
        }
        let end = (self.vertices.len() as u32).min(vertices.end);
        if at < end {
            runs.push((at.max(vertices.start)..end, Blend::Alpha));
        }
        runs
    }
    /// `AbstractContainerScreen.init`: center in integer GUI coordinates.
    pub fn centered_origin(&self, width: f32, height: f32) -> (f32, f32) {
        (
            ((self.width - width) / 2.0).trunc(),
            ((self.height - height) / 2.0).trunc(),
        )
    }
    pub fn quad(&mut self, p: [[f32; 2]; 4], uv: [[f32; 2]; 4], color: u32) {
        for i in [0, 1, 2, 0, 2, 3] {
            self.vertices.push(HudVertex {
                pos: p[i],
                uv: uv[i],
                color,
                depth: 0.0,
                glint_uv: [0.0, 0.0],
                text: 0,
            });
        }
    }
    /// A quad whose texture is the `GLINT_TEXTURING` sample at `glint_uv`;
    /// `uv` is the item's atlas UV, which the shader masks by.
    pub fn quad_glint(
        &mut self,
        p: [[f32; 2]; 4],
        uv: [[f32; 2]; 4],
        glint_uv: [[f32; 2]; 4],
        color: u32,
    ) {
        for i in [0, 1, 2, 0, 2, 3] {
            self.vertices.push(HudVertex {
                pos: p[i],
                uv: uv[i],
                color,
                depth: 0.0,
                glint_uv: glint_uv[i],
                text: 0,
            });
        }
    }
    pub fn quad_3d(&mut self, p: [[f32; 3]; 4], uv: [[f32; 2]; 4], color: u32) {
        for i in [0, 1, 2, 0, 2, 3] {
            self.vertices.push(HudVertex {
                pos: [p[i][0], p[i][1]],
                uv: uv[i],
                color,
                depth: p[i][2],
                glint_uv: [0.0, 0.0],
                text: 0,
            });
        }
    }
    /// `RenderTypes.armorCutoutNoCullGlint` (and the items' glint types) over a
    /// [`Self::quad_3d`] face: `uv` is the face's atlas UV,
    /// which the shader masks by, and `glint_uv` the transformed glint UV.
    /// `armor` samples `enchanted_glint_armor` (`ARMOR_ENTITY_GLINT`), otherwise
    /// `enchanted_glint_item`. Drawn by [`Self::depth_glint_since`].
    pub fn quad_3d_glint(
        &mut self,
        p: [[f32; 3]; 4],
        uv: [[f32; 2]; 4],
        glint_uv: [[f32; 2]; 4],
        armor: bool,
    ) {
        let marker = if armor { 4.0 } else { 2.0 };
        for i in [0, 1, 2, 0, 2, 3] {
            self.vertices.push(HudVertex {
                pos: [p[i][0], p[i][1]],
                uv: [uv[i][0] + marker, uv[i][1]],
                color: u32::MAX,
                depth: p[i][2],
                glint_uv: glint_uv[i],
                text: 0,
            });
        }
    }
    /// Keep per-vertex depth for an orthographic GUI model, with +z toward the viewer. `clip` is
    /// an explicit scissor in GUI pixels, cut to whole device pixels as it always was.
    pub fn depth_since(&mut self, start: usize, clip: [f32; 4]) {
        self.depth_glint_since(start, self.vertices.len(), clip);
    }
    /// `RenderPipelines.GLINT` after a GUI model: [`Self::depth_since`] for a model whose glint faces
    /// ([`Self::quad_3d_glint`]) follow its own from `glint`: both share the
    /// depth buffer, and the glint is drawn over the model in the same pass.
    pub fn depth_glint_since(&mut self, start: usize, glint: usize, clip: [f32; 4]) {
        let [x0, y0, x1, y1] = clip.map(|v| (v * self.scale).max(0.0) as u32);
        self.depth_pass_since(
            start,
            glint,
            [x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0)],
        );
    }
    /// [`Self::depth_glint_since`] for the screen bounds of the geometry itself (`lo` to `hi`, in
    /// GUI pixels): the scissor takes every device pixel the bounds touch, so the min edges round
    /// down and the max edges up.
    pub fn depth_glint_over(&mut self, start: usize, glint: usize, lo: [f32; 2], hi: [f32; 2]) {
        let [x0, y0] = lo.map(|v| (v * self.scale).max(0.0).floor() as u32);
        let [x1, y1] = hi.map(|v| (v * self.scale).max(0.0).ceil() as u32);
        self.depth_pass_since(
            start,
            glint,
            [x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0)],
        );
    }
    /// [`Self::depth_since`] for the screen bounds of the geometry itself: see
    /// [`Self::depth_glint_over`].
    pub fn depth_over(&mut self, start: usize, lo: [f32; 2], hi: [f32; 2]) {
        self.depth_glint_over(start, self.vertices.len(), lo, hi);
    }
    /// The depth runs of `start..` with the scissor `clip` in device pixels (`[x, y, w, h]`).
    fn depth_pass_since(&mut self, start: usize, glint: usize, clip: [u32; 4]) {
        let glint = glint.clamp(start, self.vertices.len());
        let vertices = &mut self.vertices[start..];
        if vertices.is_empty() {
            return;
        }
        let min = vertices
            .iter()
            .map(|v| v.depth)
            .fold(f32::INFINITY, f32::min);
        let max = vertices
            .iter()
            .map(|v| v.depth)
            .fold(f32::NEG_INFINITY, f32::max);
        for v in vertices {
            v.depth = (max - v.depth) / (max - min).max(f32::EPSILON);
        }
        // Separate previews clear depth independently, even if their scissors match.
        if glint > start {
            self.blends
                .push((start as u32..glint as u32, Blend::Depth(clip)));
        }
        let end = self.vertices.len();
        if end > glint {
            self.blends
                .push((glint as u32..end as u32, Blend::DepthGlint(clip)));
        }
    }
    pub fn rect_uv(&mut self, x: f32, y: f32, w: f32, h: f32, r: Rect, color: u32) {
        self.quad(
            [[x, y], [x + w, y], [x + w, y + h], [x, y + h]],
            [[r.u0, r.v0], [r.u1, r.v0], [r.u1, r.v1], [r.u0, r.v1]],
            color,
        );
    }
    pub fn sprite(&mut self, atlas: &HudAtlas, name: &str, x: f32, y: f32) {
        if let Some(r) = atlas.gui_draw_rect(name).copied() {
            self.rect_uv(x, y, r.w as f32, r.h as f32, r, u32::MAX);
        }
    }
    pub fn sprite_scaled(
        &mut self,
        atlas: &HudAtlas,
        name: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: u32,
    ) {
        if let Some(r) = atlas.gui_draw_rect(name).copied() {
            self.rect_uv(x, y, w, h, r, color);
        }
    }
    /// Draw part of a sprite, `fraction` of its width from the left.
    pub fn sprite_part(&mut self, atlas: &HudAtlas, name: &str, x: f32, y: f32, fraction: f32) {
        if let Some(mut r) = atlas.gui_draw_rect(name).copied() {
            let w = r.w as f32 * fraction;
            r.u1 = r.u0 + (r.u1 - r.u0) * fraction;
            self.rect_uv(x, y, w, r.h as f32, r, u32::MAX);
        }
    }
    /// Clip the quads drawn since `start` to `[x0, x1) x [y0, y1)` (the
    /// GUI scissor): axis-aligned quads are cut with their UVs, others (item
    /// triangles) are clipped as polygons.
    /// Scale the vertices pushed since `start` about `(cx, cy)` (a GUI pose
    /// matrix of translate, scale, translate back).
    pub fn scale_since(&mut self, start: usize, cx: f32, cy: f32, sx: f32, sy: f32) {
        for v in &mut self.vertices[start..] {
            v.pos[0] = cx + (v.pos[0] - cx) * sx;
            v.pos[1] = cy + (v.pos[1] - cy) * sy;
        }
    }
    pub fn clip_since(&mut self, start: usize, x0: f32, y0: f32, x1: f32, y1: f32) {
        let quads: Vec<[HudVertex; 6]> = self.vertices[start..].as_chunks::<6>().0.to_vec();
        self.vertices.truncate(start);
        // Where each source quad now begins, to move the blend ranges (foil
        // items are an additive run) along with their vertices.
        let mut moved = Vec::with_capacity(quads.len() + 1);
        for q in quads {
            moved.push(self.vertices.len() as u32);
            let corners = [q[0], q[1], q[2], q[5]];
            let (mut x_lo, mut x_hi) = (f32::INFINITY, f32::NEG_INFINITY);
            let (mut y_lo, mut y_hi) = (f32::INFINITY, f32::NEG_INFINITY);
            for v in &corners {
                x_lo = x_lo.min(v.pos[0]);
                x_hi = x_hi.max(v.pos[0]);
                y_lo = y_lo.min(v.pos[1]);
                y_hi = y_hi.max(v.pos[1]);
            }
            // An axis-aligned rectangle, its corners in any order.
            let find = |x: f32, y: f32| corners.iter().find(|v| v.pos == [x, y]).copied();
            let rect = (
                find(x_lo, y_lo),
                find(x_hi, y_lo),
                find(x_hi, y_hi),
                find(x_lo, y_hi),
            );
            let (Some(tl), Some(tr), Some(br), Some(bl)) = rect else {
                // Item geometry arrives as triangles: clip each against the
                // rectangle and re-emit the fan, so a triangle straddling the
                // edge keeps its visible part instead of vanishing whole.
                for tri in [[q[0], q[1], q[2]], [q[3], q[4], q[5]]] {
                    let area = (tri[1].pos[0] - tri[0].pos[0]) * (tri[2].pos[1] - tri[0].pos[1])
                        - (tri[2].pos[0] - tri[0].pos[0]) * (tri[1].pos[1] - tri[0].pos[1]);
                    if area == 0.0 {
                        continue;
                    }
                    let mut poly = tri.to_vec();
                    for (axis, limit, keep_below) in
                        [(0, x0, false), (0, x1, true), (1, y0, false), (1, y1, true)]
                    {
                        poly = clip_polygon(&poly, axis, limit, keep_below);
                    }
                    for i in 1..poly.len().saturating_sub(1) {
                        // A triangle as the quad `a, b, c, c`.
                        let (a, b, c) = (poly[0], poly[i], poly[i + 1]);
                        self.vertices.extend_from_slice(&[a, b, c, a, c, c]);
                    }
                }
                continue;
            };
            let (cx0, cy0) = (x_lo.max(x0), y_lo.max(y0));
            let (cx1, cy1) = (x_hi.min(x1), y_hi.min(y1));
            if cx0 >= cx1 || cy0 >= cy1 {
                continue;
            }
            let t = |p: f32, lo: f32, hi: f32| if hi == lo { 0.0 } else { (p - lo) / (hi - lo) };
            let corner = |x, y| {
                clip_corner::clip_corner(
                    [tl, tr, br, bl],
                    [x, y],
                    [t(x, x_lo, x_hi), t(y, y_lo, y_hi)],
                )
            };
            let c = [
                corner(cx0, cy0),
                corner(cx1, cy0),
                corner(cx1, cy1),
                corner(cx0, cy1),
            ];
            self.vertices.extend([0, 1, 2, 0, 2, 3].map(|i| c[i]));
        }
        moved.push(self.vertices.len() as u32);
        let at = |v: u32| moved[((v as usize).saturating_sub(start) / 6).min(moved.len() - 1)];
        for (range, _) in &mut self.blends {
            if range.start as usize >= start {
                *range = at(range.start)..at(range.end);
            }
        }
        self.blends.retain(|(range, _)| !range.is_empty());
    }
    /// `blit` of the top-left `w x h` texels of a `tw x th` texture.
    pub fn sprite_region(
        &mut self,
        atlas: &HudAtlas,
        name: &str,
        [x, y, w, h]: [f32; 4],
        tw: f32,
        th: f32,
    ) {
        if let Some(mut r) = atlas.sprites.get(name).copied() {
            r.u1 = r.u0 + (r.u1 - r.u0) * w / tw;
            r.v1 = r.v0 + (r.v1 - r.v0) * h / th;
            self.rect_uv(x, y, w, h, r, u32::MAX);
        }
    }
    pub fn fill(&mut self, atlas: &HudAtlas, x: f32, y: f32, w: f32, h: f32, color: u32) {
        let mut r = atlas.white;
        let (cu, cv) = ((r.u0 + r.u1) / 2.0, (r.v0 + r.v1) / 2.0);
        r.u0 = cu;
        r.u1 = cu;
        r.v0 = cv;
        r.v1 = cv;
        self.rect_uv(x, y, w, h, r, color);
    }
    /// Any four-cornered shape in one colour, corners in order round it.
    pub fn fill_quad(&mut self, atlas: &HudAtlas, corners: [[f32; 2]; 4], color: u32) {
        let w = atlas.white;
        let uv = [(w.u0 + w.u1) / 2.0, (w.v0 + w.v1) / 2.0];
        self.quad(corners, [uv; 4], color);
    }
    /// GuiGraphics.fillGradient: `top` to `bottom`, down the rectangle.
    pub fn fill_gradient(
        &mut self,
        atlas: &HudAtlas,
        (x, y, w, h): (f32, f32, f32, f32),
        top: u32,
        bottom: u32,
    ) {
        let r = atlas.white;
        let uv = [(r.u0 + r.u1) / 2.0, (r.v0 + r.v1) / 2.0];
        let p = [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
        let c = [top, top, bottom, bottom];
        for i in [0, 1, 2, 0, 2, 3] {
            self.vertices.push(HudVertex {
                pos: p[i],
                uv,
                color: c[i],
                depth: 0.0,
                glint_uv: [0.0, 0.0],
                text: 0,
            });
        }
    }
    pub fn text_width(atlas: &HudAtlas, text: &str) -> f32 {
        atlas.font.width(text)
    }
    /// Draw plain text (legacy `§` codes are honoured); returns the advance.
    pub fn text(
        &mut self,
        atlas: &HudAtlas,
        text: &str,
        x: f32,
        y: f32,
        color: u32,
        shadow: bool,
    ) -> f32 {
        let t = if text.contains('§') {
            crate::text::Text::legacy(text)
        } else {
            crate::text::Text::literal(text)
        };
        self.text_styled(atlas, &t, x, y, color, shadow)
    }
    /// Font.drawInternal: styled glyphs, shadows first, with bold, italic,
    /// underline and strikethrough. `color` is the default colour and alpha.
    pub fn text_styled(
        &mut self,
        atlas: &HudAtlas,
        text: &crate::text::Text,
        x: f32,
        y: f32,
        color: u32,
        shadow: bool,
    ) -> f32 {
        self.styled_text(atlas, text, x, y, color, shadow, false, None)
    }
    /// `text_styled` for text drawn in the world (`TextRenderable.render`
    /// with `flat` false): each quad's `depth` is its z in text units, the
    /// glyphs `Font.SHADOW_DEPTH` in front of their shadow, a bold copy 0.001
    /// in front of its glyph and the lines 0.01 (`BakedSheetGlyph`), so they
    /// do not fight over the same pixels.
    pub fn text_styled_world(
        &mut self,
        atlas: &HudAtlas,
        text: &crate::text::Text,
        x: f32,
        y: f32,
        color: u32,
        shadow: bool,
    ) -> f32 {
        self.styled_text(atlas, text, x, y, color, shadow, true, None)
    }
    /// `Font.drawInBatch8xOutline`: eight glyph offsets in the outline color,
    /// then the original text. Returns (advance, first front-text vertex), so
    /// world rendering can use POLYGON_OFFSET for the front and NORMAL for
    /// its outline. Offsets use each glyph's own shadow offset (unihex: 0.5).
    pub fn text_styled_outline_world(
        &mut self,
        atlas: &HudAtlas,
        text: &crate::text::Text,
        x: f32,
        y: f32,
        color: u32,
        outline: u32,
    ) -> (f32, usize) {
        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx != 0 || dy != 0 {
                    self.styled_text(
                        atlas,
                        text,
                        x,
                        y,
                        color,
                        false,
                        true,
                        Some((outline, [dx as f32, dy as f32])),
                    );
                }
            }
        }
        let front = self.vertices.len();
        (
            self.styled_text(atlas, text, x, y, color, false, true, None),
            front,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn styled_text(
        &mut self,
        atlas: &HudAtlas,
        text: &crate::text::Text,
        x: f32,
        y: f32,
        color: u32,
        shadow: bool,
        world: bool,
        outline: Option<(u32, [f32; 2])>,
    ) -> f32 {
        // StringDecomposer.iterateFormatted inside Font.drawInternal:
        // `§` codes inside a span restyle what follows and are not drawn.
        let text = text.resolved();
        // Language.getVisualOrder: shaped and reordered for right-to-left text.
        let text = crate::bidi::visual(&text);
        let font = &atlas.font;
        let alpha = (color >> 24) as u8;
        let mut glyphs = Vec::new();
        let mut cursor = x;
        let mut previous = None;
        for (i, (ch, style)) in text.chars().enumerate() {
            let object = (ch == '\u{fffc}')
                .then_some(style.inline_object.as_ref())
                .flatten();
            if let Some(crate::text::InlineObject::Player { profile, .. }) = object
                && self.inline_profiles.len() < 256
                && !self.inline_profiles.contains(profile)
            {
                self.inline_profiles.push(profile.clone());
            }
            // Font.PreparedTextBuilder.accept: lines start 1 left of a
            // sequence's first character. Language.getVisualOrder makes a
            // sequence of each run of one style, which world text has.
            let first = i == 0 || world && previous != Some(style);
            previous = Some(style);
            // The style's font (`Style.getFont`), the default one unless set.
            let mut g = if object.is_some() {
                crate::font::Glyph {
                    rect: None,
                    left: 0.0,
                    top: 0.0,
                    right: 0.0,
                    bottom: crate::font::INLINE_OBJECT_SIZE,
                    advance: crate::font::INLINE_OBJECT_SIZE,
                    offset: 1.0,
                }
            } else {
                font.glyph_styled(ch, style)
            };
            // Font.StringRenderOutput.accept: obfuscated characters draw a
            // random glyph of the same width each frame.
            if object.is_none() && style.obfuscated == Some(true) && ch != ' ' {
                g = font.random_glyph(style, g.advance);
            }
            let c = style.color.map_or(color, |rgb| {
                rgba((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, alpha)
            });
            let bold = object.is_none() && style.is_bold();
            let advance = g.advance + if bold { g.offset } else { 0.0 };
            glyphs.push((
                cursor,
                g,
                c,
                style.shadow_color,
                bold,
                style.is_italic(),
                style.underlined == Some(true),
                style.strikethrough == Some(true),
                first,
                advance,
                object.cloned(),
            ));
            cursor += advance;
        }
        self.patches.extend(font.take_patches());
        let white = font.white();
        let has_shadow = outline.is_none()
            && (shadow
                || glyphs
                    .iter()
                    .any(|glyph| glyph.3.is_some_and(|color| color != 0)));
        let passes: &[bool] = if has_shadow { &[true, false] } else { &[false] };
        // ARGB.scaleRGB(color, 0.25): the default shadow colour.
        let shade = |c: u32| {
            let b = c.to_le_bytes();
            rgba(
                (f32::from(b[0]) * 0.25) as u8,
                (f32::from(b[1]) * 0.25) as u8,
                (f32::from(b[2]) * 0.25) as u8,
                b[3],
            )
        };
        for &is_shadow in passes {
            for (
                gx,
                g,
                c,
                custom_shadow,
                bold,
                italic,
                underline,
                strike,
                first,
                advance,
                object,
            ) in &glyphs
            {
                let shadow_color = if let Some(argb) = custom_shadow {
                    let text_alpha = (c >> 24) as u8 as u16;
                    let shadow_alpha = (argb >> 24) as u8 as u16;
                    let alpha = (shadow_alpha * text_alpha / 255) as u8;
                    rgba((argb >> 16) as u8, (argb >> 8) as u8, *argb as u8, alpha)
                } else {
                    shade(*c)
                };
                let has_shadow = outline.is_none()
                    && (custom_shadow.is_some_and(|argb| argb != 0)
                        || custom_shadow.is_none() && shadow)
                    && !(world && shadow_color == 0);
                if is_shadow && !has_shadow {
                    continue;
                }
                let (dx, col) = if is_shadow {
                    (g.offset, shadow_color)
                } else {
                    (0.0, *c)
                };
                let (px, py, col) = if let Some((color, [dx, dy])) = outline {
                    (*gx + dx * g.offset, y + dy * g.offset, color)
                } else {
                    (*gx + dx, y + dx, col)
                };
                if let Some(object) = object {
                    // `PlainTextRenderable.render`: sprite glyphs use the same
                    // shadow colour and offset as text, with the front at SHADOW_DEPTH.
                    let start = self.vertices.len();
                    self.draw_inline_object(atlas, object, px, py - 1.0, col);
                    if world && has_shadow && !is_shadow {
                        for v in &mut self.vertices[start..] {
                            v.depth = 0.03;
                        }
                    }
                    continue;
                }
                let shear = |v: f32| if *italic { 1.0 - 0.25 * v } else { 0.0 };
                let thick = if *bold { 0.1 } else { 0.0 };
                let draw = |batch: &mut Self, ox: f32| match g.rect {
                    Some(r) => {
                        let (x0, x1, y0, y1) = (
                            px + ox + g.left,
                            px + ox + g.right,
                            py + g.top,
                            py + g.bottom,
                        );
                        batch.quad(
                            [
                                [x0 + shear(g.top) - thick, y0 - thick],
                                [x1 + shear(g.top) + thick, y0 - thick],
                                [x1 + shear(g.bottom) + thick, y1 + thick],
                                [x0 + shear(g.bottom) - thick, y1 + thick],
                            ],
                            [[r.u0, r.v0], [r.u1, r.v0], [r.u1, r.v1], [r.u0, r.v1]],
                            col,
                        );
                    }
                    None if g.right > 0.0 => {
                        // SpecialGlyphs.MISSING: a 5x8 outline.
                        let (bx, by) = (px + ox, py);
                        for (fx, fy, fw, fh) in [
                            (0.0, 0.0, 5.0, 1.0),
                            (0.0, 7.0, 5.0, 1.0),
                            (0.0, 1.0, 1.0, 6.0),
                            (4.0, 1.0, 1.0, 6.0),
                        ] {
                            batch.rect_uv(bx + fx, by + fy, fw, fh, white, col);
                        }
                    }
                    None => {}
                };
                let base = if has_shadow && !is_shadow { 0.03 } else { 0.0 };
                let lift = |batch: &mut Self, start: usize, z: f32| {
                    if world {
                        for v in &mut batch.vertices[start..] {
                            v.depth = z;
                        }
                    }
                };
                let glyph = self.vertices.len();
                let start = self.vertices.len();
                draw(self, 0.0);
                lift(self, start, base);
                if *bold {
                    let start = self.vertices.len();
                    draw(self, g.offset);
                    lift(self, start, base + 0.001);
                }
                let x0 = if *first { px - 1.0 } else { px };
                let start = self.vertices.len();
                if *strike {
                    self.rect_uv(x0, py + 3.5, px + *advance - x0, 1.0, white, col);
                }
                if *underline {
                    self.rect_uv(x0, py + 8.0, px + *advance - x0, 1.0, white, col);
                }
                lift(self, start, base + 0.01);
                if !world {
                    for v in &mut self.vertices[glyph..] {
                        v.text = 1;
                    }
                }
            }
        }
        cursor - x
    }

    fn draw_inline_object(
        &mut self,
        atlas: &HudAtlas,
        object: &crate::text::InlineObject,
        x: f32,
        y: f32,
        tint: u32,
    ) {
        use crate::text::InlineObject;
        match object {
            InlineObject::Player { profile, hat, .. } => {
                let id = crate::skins::parse_profile(profile).id;
                let name = inline_profile_name(profile);
                let by_id = atlas.object_skins.get(&format!("id:{id:032x}"));
                let by_name = name.and_then(|name| {
                    atlas
                        .object_skins
                        .get(&format!("name:{}", name.to_ascii_lowercase()))
                });
                let resolved = if profile.get("id").is_some() {
                    by_id.or(by_name)
                } else {
                    by_name.or(by_id)
                };
                let default = crate::tab_list::default_skin(id);
                let skin = resolved
                    .filter(|path| atlas.sprites.contains_key(*path))
                    .map_or(default, String::as_str);
                let Some(rect) = atlas.sprites.get(skin).copied() else {
                    return;
                };
                for u in [8.0, 40.0].into_iter().take(if *hat { 2 } else { 1 }) {
                    let du = (rect.u1 - rect.u0) / 64.0;
                    let dv = (rect.v1 - rect.v0) / 64.0;
                    self.rect_uv(
                        x,
                        y,
                        8.0,
                        8.0,
                        Rect {
                            u0: rect.u0 + du * u,
                            v0: rect.v0 + dv * 8.0,
                            u1: rect.u0 + du * (u + 8.0),
                            v1: rect.v0 + dv * 16.0,
                            w: 8,
                            h: 8,
                        },
                        tint,
                    );
                }
            }
            InlineObject::Atlas { .. } => {
                if let Some(rect) = atlas.inline_object_sprite(object) {
                    let size = crate::font::INLINE_OBJECT_SIZE;
                    self.rect_uv(x, y, size, size, rect, tint);
                }
            }
        }
    }

    /// SubmitNodeCollection.submitNameTag's text in its own space (x right,
    /// y down, in GUI pixels): centred at `-Font.width / 2`, drawn as
    /// Font.prepareText draws any text (styles, fonts and glyph boxes; no
    /// shadow), over `background` (RGBA) unless its alpha is 0. Returns how
    /// many vertices at the start of the batch are the background.
    pub fn name_tag(
        &mut self,
        atlas: &HudAtlas,
        text: &crate::text::Text,
        y: f32,
        color: u32,
        background: u32,
    ) -> usize {
        let x = -atlas.font.width_text(text) / 2.0;
        let start = self.vertices.len();
        let advance = self.text_styled(atlas, text, x, y, color, false);
        if background >> 24 == 0 {
            return 0;
        }
        // PreparedTextBuilder.markBackground: one pixel round the advances,
        // from a pixel above the line to 9 below its top.
        let glyphs = self.vertices.split_off(start);
        self.fill(atlas, x - 1.0, y - 1.0, advance + 1.0, 10.0, background);
        let behind = self.vertices.len() - start;
        self.vertices.extend(glyphs);
        behind
    }
}

fn inline_profile_name(profile: &serde_json::Value) -> Option<&str> {
    profile
        .as_str()
        .or_else(|| profile.get("name").and_then(serde_json::Value::as_str))
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct HudUniform {
    size: [f32; 4],
    /// `EndPortalFill`: the two sprites, then the game time in x.
    end_portal: [[f32; 4]; 3],
}
impl HudUniform {
    fn new(size: [f32; 4], fill: EndPortalFill, glint_colour: u32) -> Self {
        let [r, g, b, _] = glint_colour.to_le_bytes().map(|c| f32::from(c) / 255.0);
        let tint = if glint_colour == 0 {
            [-1.0, 0.0, 0.0]
        } else {
            [r, g, b]
        };
        Self {
            size,
            end_portal: [
                fill.sprites[0],
                fill.sprites[1],
                [fill.game_time, tint[0], tint[1], tint[2]],
            ],
        }
    }
}
use crate::gfx::hud_coverage::{self, Pipeline};

pub struct HudRenderer {
    pictures: HashMap<u64, (Arc<pictures::Picture>, wgpu::BindGroup)>,
    pub sprite_animations: Vec<(Rect, crate::sprites::SpriteAnimation, (usize, usize, u32))>,
    /// The shared atlas texture, for run-time glyph uploads.
    pub atlas_texture: Option<wgpu::Texture>,
    pipeline: Pipeline,
    /// `RenderPipelines.CROSSHAIR`.
    invert_pipeline: Pipeline,
    additive_pipeline: Pipeline,
    depth_pipeline: Pipeline,
    /// `RenderPipelines.GLINT` over a GUI model (`Blend::DepthGlint`).
    depth_glint_pipeline: Pipeline,
    vignette_pipeline: Pipeline,
    end_portal_pipeline: Pipeline,
    /// The layout of `group`, which the CIT glint pipelines share.
    layout: wgpu::BindGroupLayout,
    group: wgpu::BindGroup,
    /// The pack's CIT enchantment glints, when it has any.
    cit_glint: Option<cit_glint::CitGlintPass>,
    uniform: wgpu::Buffer,
    buffer: wgpu::Buffer,
    capacity: u64,
    format: wgpu::TextureFormat,
    coverage: bool,
    atlas_view: wgpu::TextureView,
    /// The resource pack's text shader, when it has one that works.
    pack_text: Option<pack_text::PackText>,
    /// Target pixels per window pixel: above 1 for a screenshot drawn larger than the
    /// window (`Renderer::screenshot_quality`).
    pub zoom: f32,
}
impl HudRenderer {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        view: &wgpu::TextureView,
        glint_view: &wgpu::TextureView,
        glint_armor_view: &wgpu::TextureView,
        glint_sampler: &wgpu::Sampler,
        coverage: bool,
    ) -> Result<Self> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sunrise HUD"),
            source: wgpu::ShaderSource::Wgsl(include_str!("hud.wgsl").into()),
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("HUD"),
            ..Default::default()
        });
        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("HUD linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("HUD size"),
            contents: bytemuck::bytes_of(&HudUniform::new([1.0; 4], EndPortalFill::default(), 0)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HUD bindings"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HUD resources"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(glint_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(glint_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&linear_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(glint_armor_view),
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HUD layout"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let attributes =
            wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Unorm8x4,3=>Float32,4=>Float32x2];
        // `depth`: the model's depth test, and whether the run writes depth.
        let create = |label, blend, fragment_entry, depth: Option<bool>| {
            let desc = wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<HudVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &attributes,
                    })],
                },
                primitive: Default::default(),
                depth_stencil: depth.map(|write| wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(write),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment_entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            };
            let source = wgpu::ShaderSource::Wgsl(include_str!("hud.wgsl").into());
            Pipeline::new(device, &desc, source, coverage).expect("the HUD's coverage")
        };
        let pipeline = create("HUD", wgpu::BlendState::ALPHA_BLENDING, "fs_main", None);
        let depth_pipeline = create(
            "HUD model",
            wgpu::BlendState::ALPHA_BLENDING,
            "fs_main",
            Some(true),
        );
        // BlendFunction.INVERT: ONE_MINUS_DST_COLOR, ONE_MINUS_SRC_COLOR,
        // ONE, ZERO.
        let invert_pipeline = create(
            "HUD crosshair",
            wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::OneMinusDst,
                    dst_factor: wgpu::BlendFactor::OneMinusSrc,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
            },
            "fs_crosshair",
            None,
        );
        // BlendFunction.ADDITIVE: ONE, ONE.
        let add = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let additive_pipeline = create(
            "HUD text highlight",
            wgpu::BlendState {
                color: add,
                alpha: add,
            },
            "fs_main",
            None,
        );
        // RenderPipelines.GLINT tests depth EQUAL and writes none; the faces are
        // the model's own, so LEQUAL passes the same pixels.
        let depth_glint_pipeline = create(
            "HUD model glint",
            wgpu::BlendState {
                color: add,
                alpha: add,
            },
            "fs_main",
            Some(false),
        );
        // RenderPipelines.VIGNETTE: ZERO, ONE_MINUS_SRC_COLOR, ZERO, ONE.
        let vignette_pipeline = create(
            "HUD vignette",
            wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Zero,
                    dst_factor: wgpu::BlendFactor::OneMinusSrc,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Zero,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            },
            "fs_vignette",
            None,
        );
        // RenderPipelines.END_PORTAL: opaque.
        let end_portal_pipeline = create(
            "HUD end portal",
            wgpu::BlendState::REPLACE,
            "fs_end_portal",
            None,
        );
        let capacity = 1 << 16;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("HUD vertices"),
            size: capacity * std::mem::size_of::<HudVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Self {
            pictures: HashMap::new(),
            atlas_texture: None,
            sprite_animations: Vec::new(),
            pipeline,
            invert_pipeline,
            additive_pipeline,
            depth_pipeline,
            depth_glint_pipeline,
            vignette_pipeline,
            end_portal_pipeline,
            layout,
            group,
            cit_glint: None,
            uniform,
            buffer,
            capacity,
            format,
            coverage,
            atlas_view: view.clone(),
            pack_text: None,
            zoom: 1.0,
        })
    }
    /// Draw the CIT glint layers of `textures` (the pack's enchantment rules); `None` or no
    /// textures draws none.
    pub fn set_cit_glints(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        textures: Option<&cit_glint::GlintTextures>,
    ) {
        self.cit_glint = textures.and_then(|t| {
            cit_glint::CitGlintPass::new(device, queue, self.format, &self.layout, t, self.coverage)
        });
    }
    /// Draw the GUI's text with `shader` (`HudAtlas::text_shaders`' `gui`, with the Polytone
    /// expression uniforms its stages read, `shader_modifiers`), or natively.
    pub fn set_text_shader(
        &mut self,
        device: &wgpu::Device,
        shader: Option<(
            &crate::core_shader::CoreProgram,
            &[crate::polytone_post::ExpressionUniform],
        )>,
    ) {
        self.pack_text = shader.and_then(|shader| {
            pack_text::PackText::new(device, self.format, &self.atlas_view, shader, self.coverage)
        });
    }
    /// The GUI text's Polytone blocks, when a pack's text shader draws it.
    pub(crate) fn text_expression_blocks(
        &mut self,
    ) -> Option<&mut crate::gfx::expression_blocks::ExpressionBlocks> {
        self.pack_text.as_mut().map(|text| &mut text.extra)
    }
    /// Stage the batch's `patches` and `image_patches` (client mods' dynamic textures) into
    /// the shared atlas texture.
    pub fn upload_patches(
        &self,
        uploads: &mut crate::gfx::upload::Uploads,
        encoder: &mut wgpu::CommandEncoder,
        batch: &HudBatch,
    ) {
        let Some(texture) = &self.atlas_texture else {
            return;
        };
        let glyphs = batch
            .patches
            .iter()
            .map(|p| ([p.x, p.y], [p.width, p.height], &p.rgba[..]));
        let images = batch
            .image_patches
            .iter()
            .map(|p| (p.origin, p.size, &p.rgba[..]));
        for ([x, y], [width, height], rgba) in glyphs.chain(images) {
            if x.saturating_add(width) <= texture.width()
                && y.saturating_add(height) <= texture.height()
            {
                uploads.texture(
                    encoder,
                    wgpu::TexelCopyTextureInfo {
                        texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x, y, z: 0 },
                        aspect: wgpu::TextureAspect::All,
                    },
                    width,
                    height,
                    rgba,
                );
            }
        }
    }
    /// Stage the batch's atlas patches, vertices and uniform: before the
    /// frame's first pass, so the copies share one command buffer.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        uploads: &mut crate::gfx::upload::Uploads,
        encoder: &mut wgpu::CommandEncoder,
        batch: &HudBatch,
        size: [u32; 2],
    ) {
        self.prepare_pictures(device, uploads, encoder, batch);
        self.upload_patches(uploads, encoder, batch);
        if batch.vertices.is_empty() || size.contains(&0) {
            return;
        }
        let count = batch.vertices.len() as u64;
        if count > self.capacity {
            self.capacity = count.next_power_of_two();
            self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("HUD vertices"),
                size: self.capacity * std::mem::size_of::<HudVertex>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        uploads.buffer(
            encoder,
            &self.buffer,
            0,
            bytemuck::cast_slice(&batch.vertices),
        );
        uploads.buffer(
            encoder,
            &self.uniform,
            0,
            bytemuck::bytes_of(&HudUniform::new(
                [
                    size[0] as f32,
                    size[1] as f32,
                    batch.scale * self.zoom,
                    batch.glint_alpha,
                ],
                batch.end_portal,
                batch.glint_colour,
            )),
        );
        if let Some(pack_text) = &self.pack_text {
            pack_text.prepare(uploads, encoder, batch, size, batch.scale * self.zoom);
        }
    }
    /// Draw the batch `prepare` staged.
    pub fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        batch: &HudBatch,
        size: [u32; 2],
    ) {
        self.draw_range(
            encoder,
            target,
            None,
            depth,
            batch,
            size,
            0..batch.vertices.len() as u32,
            None,
        );
    }
    /// Draw part of the prepared batch, preserving its blend/depth runs.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_range(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        coverage: Option<&wgpu::TextureView>,
        depth: &wgpu::TextureView,
        batch: &HudBatch,
        size: [u32; 2],
        vertices: std::ops::Range<u32>,
        mut timing: Option<&mut crate::gfx::pack_timing::PassTiming>,
    ) {
        if vertices.is_empty() || size.contains(&0) {
            return;
        }
        let mut attachments = [None, None];
        attachments[0] = Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        });
        let attachment_count;
        if let Some(view) = coverage {
            attachments[1] = Some(hud_coverage::attachment(view));
            attachment_count = 2;
        } else {
            attachment_count = 1;
        }
        let runs = batch.runs_in(vertices);
        let mut at = 0;
        while at < runs.len() {
            let clip = match runs[at].1 {
                Blend::Depth(clip) | Blend::DepthGlint(clip) => Some(clip),
                _ => None,
            };
            let end = if let Some(clip) = clip {
                // The model's glint shares its pass and depth buffer.
                at + 1
                    + runs[at + 1..]
                        .iter()
                        .take_while(|(_, b)| *b == Blend::DepthGlint(clip))
                        .count()
            } else {
                runs[at..]
                    .iter()
                    .position(|(_, b)| matches!(b, Blend::Depth(_) | Blend::DepthGlint(_)))
                    .map_or(runs.len(), |n| at + n)
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HUD"),
                timestamp_writes: timing.as_deref_mut().and_then(|t| t.writes("HUD")),
                color_attachments: &attachments[..attachment_count],
                depth_stencil_attachment: clip.map(|_| wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_bind_group(0, &self.group, &[]);
            pass.set_vertex_buffer(0, self.buffer.slice(..));
            if let Some(clip) = clip {
                let [x, y, w, h] = clip.map(|v| (v as f32 * self.zoom) as u32);
                let (x, y) = (x.min(size[0]), y.min(size[1]));
                let (w, h) = (w.min(size[0] - x), h.min(size[1] - y));
                if w == 0 || h == 0 {
                    at = end;
                    continue;
                }
                pass.set_scissor_rect(x, y, w, h);
            }
            for (range, blend) in &runs[at..end] {
                let pipeline = match blend {
                    Blend::Picture(id) => {
                        self.draw_picture(&mut pass, *id, range.clone(), coverage.is_some());
                        continue;
                    }
                    Blend::Alpha => &self.pipeline,
                    Blend::Invert => &self.invert_pipeline,
                    Blend::Additive => &self.additive_pipeline,
                    Blend::Depth(_) => &self.depth_pipeline,
                    Blend::DepthGlint(_) => &self.depth_glint_pipeline,
                    Blend::Vignette => &self.vignette_pipeline,
                    Blend::EndPortal => &self.end_portal_pipeline,
                    Blend::CitGlint(blend) => {
                        if let Some(cit) = &self.cit_glint {
                            cit.draw(&mut pass, *blend, range.clone(), coverage.is_some());
                        }
                        continue;
                    }
                };
                let pipeline = pipeline.get(coverage.is_some());
                match &self.pack_text {
                    Some(pack_text) if *blend == Blend::Alpha => {
                        for (part, text) in pack_text::text_runs(&batch.vertices, range.clone()) {
                            if text {
                                pack_text.draw(&mut pass, part, coverage.is_some());
                                pass.set_bind_group(0, &self.group, &[]);
                            } else {
                                pass.set_pipeline(pipeline);
                                pass.draw(part, 0..1);
                            }
                        }
                    }
                    _ => {
                        pass.set_pipeline(pipeline);
                        pass.draw(range.clone(), 0..1);
                    }
                }
            }
            at = end;
        }
    }
}

#[cfg(test)]
#[path = "hud/coverage_tests.rs"]
mod coverage_tests;

#[cfg(test)]
mod tests;
