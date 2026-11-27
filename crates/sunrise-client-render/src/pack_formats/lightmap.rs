//! OptiFine custom lightmaps (`sunrise_optifine::lightmap`) on the engine's
//! [`crate::custom_lightmap::Lightmaps`]: a pack's pictures, decoded once with the pack, and
//! each frame's texture from them. A pack without `optifine/lightmap` has none.

use crate::custom_lightmap::{Input, Lightmaps, Pixels};
use crate::resource_source::ResourceSource;
use sunrise_optifine::lightmap::{self as optifine, Files};

/// A pack's decoded lightmaps.
pub struct CustomLightmaps(optifine::Lightmaps);

/// Pictures past this many texels are left out (a lightmap is a few hundred pixels).
const TEXEL_LIMIT: u64 = 1 << 20;

impl CustomLightmaps {
    /// The lightmaps of the overlay at `root`. A picture that does not decode is dropped with a
    /// warning.
    pub fn load(root: &ResourceSource) -> Self {
        let files = Files::scan(root);
        if files.is_empty() {
            return Self(Default::default());
        }
        Self(optifine::Lightmaps::load(&files, |path| {
            match root
                .read(path)
                .map_err(Into::into)
                .and_then(|bytes| crate::hud::decode_image_bytes(&bytes))
            {
                Ok((width, height, _)) if u64::from(width) * u64::from(height) > TEXEL_LIMIT => {
                    log::warn!("{path}: a lightmap past {TEXEL_LIMIT} pixels is left out");
                    None
                }
                Ok(image) => Some(image),
                Err(e) => {
                    log::warn!("{path}: {e:#}");
                    None
                }
            }
        }))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Lightmaps for CustomLightmaps {
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
