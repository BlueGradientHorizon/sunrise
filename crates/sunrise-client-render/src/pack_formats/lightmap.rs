//! OptiFine custom lightmaps (`sunrise_optifine::lightmap`) on the engine's
//! [`crate::custom_lightmap::Lightmaps`]: a pack's pictures, decoded once with the pack, and
//! each frame's texture from them. A pack without `optifine/lightmap` has none.

use crate::custom_lightmap::{Input, Lightmaps, Pixels};
use crate::resource_source::ResourceSource;
use sunrise_optifine::lightmap::{self as optifine, Files, Picture};

/// A pack's decoded lightmaps.
pub struct CustomLightmaps(optifine::Lightmaps);

/// The widest picture read: 4096 columns of 64 rows (a lightmap is a few dozen columns).
const MAX_WIDTH: u32 = 4096;
/// The most bytes read of one file, before it is decoded.
const MAX_BYTES: u64 = 4 << 20;

/// A PNG's width and height, from the header's first 24 bytes; none for anything else.
fn png_size(head: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if head.len() < 24 || &head[..8] != SIGNATURE || &head[12..16] != b"IHDR" {
        return None;
    }
    let number =
        |at: usize| u32::from_be_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]]);
    Some((number(16), number(20)))
}

/// The picture at `path` as width, height and RGBA, read only when its header gives a lightmap's
/// size within the limits; none with a warning otherwise.
fn decode(root: &ResourceSource, path: &str) -> Option<(u32, u32, Vec<u8>)> {
    let Some((width, height)) = root.read_head(path, 24).ok().as_deref().and_then(png_size) else {
        log::warn!("{path}: a lightmap is a PNG");
        return None;
    };
    if !Picture::valid_size(width, height) || width > MAX_WIDTH {
        log::warn!("{path}: a lightmap is 1 to {MAX_WIDTH} pixels wide and 32 or 64 tall");
        return None;
    }
    match root
        .read_limited(path, MAX_BYTES)
        .map_err(Into::into)
        .and_then(|bytes| crate::hud::decode_image_bytes(&bytes))
    {
        Ok(image) if (image.0, image.1) == (width, height) => Some(image),
        Ok(_) => {
            log::warn!("{path}: the picture's size is not its header's");
            None
        }
        Err(e) => {
            log::warn!("{path}: {e:#}");
            None
        }
    }
}

impl CustomLightmaps {
    /// The lightmaps of the overlay at `root`, for the worlds that have one
    /// (`sunrise_optifine::lightmap::WORLDS`). A picture that does not decode is dropped with a
    /// warning.
    pub fn load(root: &ResourceSource) -> Self {
        let files = Files::scan(root);
        if files.is_empty() {
            return Self(Default::default());
        }
        Self(optifine::Lightmaps::load(&files, |path| decode(root, path)))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Lightmaps for CustomLightmaps {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn has(&self, world: i32) -> bool {
        self.0.has(world)
    }

    fn pixels(&self, world: i32, input: &Input) -> Option<Pixels> {
        self.0.pixels(
            world,
            &optifine::Input {
                sky: input.sky,
                lightning: input.weather.lightning,
                flicker: input.flicker,
                gamma: input.gamma,
                night_vision: input.night_vision,
                darkness: input.darkness,
                rain: input.weather.rain,
                thunder: input.weather.thunder,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(width: u32, height: u32) -> Vec<u8> {
        let mut head = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        head.extend(width.to_be_bytes());
        head.extend(height.to_be_bytes());
        head
    }

    #[test]
    fn the_header_gives_the_size_before_decoding() {
        assert_eq!(png_size(&header(16, 32)), Some((16, 32)));
        assert_eq!(png_size(&header(16, 32)[..20]), None);
        assert_eq!(png_size(b"GIF89a.................."), None);
    }

    #[test]
    fn oversized_or_misshapen_pictures_are_refused_from_the_header() {
        // Headers alone: a decode would fail, so a refusal here comes from the header.
        let dir =
            std::env::temp_dir().join(format!("sunrise-lightmap-pack-{}", std::process::id()));
        let files = [
            ("too_wide.png", header(MAX_WIDTH + 1, 32)),
            ("too_tall.png", header(16, 33)),
            ("not_png.png", b"not a png at all, just bytes".to_vec()),
        ];
        std::fs::create_dir_all(&dir).unwrap();
        for (name, bytes) in &files {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
        let root = ResourceSource::directory(dir.clone());
        for (name, _) in &files {
            assert!(decode(&root, name).is_none(), "{name}");
        }
        std::fs::remove_dir_all(dir).ok();
    }
}
