//! Custom lightmaps: `optifine/lightmap/world<N>.png` (`world-1` the Nether, `world0` the
//! Overworld, `world1` the End), and beside it `world<N>_rain.png` and `world<N>_thunder.png` for
//! the weather. A world with one has its light texture (the 16x16 table of sky light by block
//! light every world shader reads) taken from the picture instead of computed.
//!
//! A picture is at least 16 pixels wide and 32 tall. Its rows 0 to 15 are the sky light at levels
//! 0 to 15 and rows 16 to 31 the block light; across, a column is a moment: the sky's from its
//! darkest (left) to its brightest (right), the block light's from the dimmest flicker to the
//! brightest. A frame takes one column of each, blended between the two nearest pixel columns,
//! and adds them per level: sky level `s` and block level `b` light as `sky[s] + block[b]`. With
//! night vision on, a picture 64 tall is read from rows 32 to 63 instead; a shorter one is then
//! not used, and the game computes the texture.
//!
//! In a world with weather (not the Nether or the End), a pack's rain and thunder pictures are
//! blended in by the rain and thunder strength.
//!
//! Written from OptiFine's public documentation of the format (the Custom Lightmaps page of
//! OptiDocs) and checked against OptiFine's behaviour.

use std::collections::BTreeMap;
use sunrise_assets::resource_source::ResourceSource;

/// The light texture's side: light levels 0 to 15.
pub const LEVELS: usize = 16;

/// A light texture: RGBA rows of sky level by block level (`(sky * 16 + block) * 4`).
pub type Pixels = [u8; LEVELS * LEVELS * 4];

/// Below this, a weight (or the gamma option) counts as none.
const EPSILON: f32 = 1.0e-4;

/// The lightmap files of every world a pack gives one, by world number.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Files {
    pub worlds: BTreeMap<i32, WorldFiles>,
}

/// One world's pictures, as resource paths.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct WorldFiles {
    pub base: String,
    pub rain: Option<String>,
    pub thunder: Option<String>,
}

impl Files {
    /// The lightmaps of the overlay at `root`; `optifine/` wins over the legacy `mcpatcher/`.
    pub fn scan(root: &ResourceSource) -> Self {
        let mut out = Self::default();
        for top in ["optifine", "mcpatcher"] {
            let prefix = format!("assets/minecraft/{top}/lightmap/");
            let files: std::collections::BTreeSet<String> =
                root.list(&prefix).into_iter().collect();
            for file in &files {
                let Some(world) = file
                    .strip_prefix(&prefix)
                    .and_then(|name| name.strip_prefix("world"))
                    .and_then(|name| name.strip_suffix(".png"))
                    .and_then(|n| n.parse::<i32>().ok())
                else {
                    continue;
                };
                if out.worlds.contains_key(&world) {
                    continue;
                }
                let beside = |suffix: &str| {
                    let path = format!("{prefix}world{world}{suffix}.png");
                    files.contains(&path).then_some(path)
                };
                out.worlds.insert(
                    world,
                    WorldFiles {
                        base: file.clone(),
                        rain: beside("_rain"),
                        thunder: beside("_thunder"),
                    },
                );
            }
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.worlds.is_empty()
    }
}

/// What a frame's light texture depends on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    /// The sky's brightness, 0 (night) to 1 (noon): the sky light factor.
    pub sky: f32,
    /// Lightning struck lately (the sky flashes): the sky column is the brightest.
    pub lightning: bool,
    /// The block light's flicker, about -0.1 to 0.1 around its rest.
    pub flicker: f32,
    /// The brightness option, 0 (moody) to 1 (bright).
    pub gamma: f32,
    pub night_vision: bool,
    /// The darkness effect's pulse, taken off every channel.
    pub darkness: f32,
    /// The rain and thunder strength, 0 to 1.
    pub rain: f32,
    pub thunder: f32,
}

/// A decoded lightmap picture: linear RGB per pixel, 0 to 1, in rows.
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    width: usize,
    height: usize,
    rgb: Vec<[f32; 3]>,
}

impl Picture {
    /// The picture from its RGBA bytes; none when it is narrower than 16 pixels, shorter than
    /// 32, or the bytes are not `width * height` pixels.
    pub fn from_rgba(width: u32, height: u32, rgba: &[u8]) -> Option<Self> {
        let (width, height) = (width as usize, height as usize);
        if width < LEVELS || height < 2 * LEVELS || rgba.len() != width * height * 4 {
            return None;
        }
        let rgb = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| std::array::from_fn(|k| f32::from(p[k]) / 255.0))
            .collect();
        Some(Self { width, height, rgb })
    }

    /// The 16 rows from `top` at column `x` (in pixels, fractional), each blended between its
    /// two nearest pixel columns.
    fn column(&self, x: f32, top: usize) -> [[f32; 3]; LEVELS] {
        let (lo, hi) = (x.floor() as usize, (x.ceil() as usize).min(self.width - 1));
        std::array::from_fn(|row| {
            let at = |c: usize| self.rgb[(top + row) * self.width + c];
            if lo == hi {
                return at(lo);
            }
            let t = x - lo as f32;
            let (a, b) = (at(lo), at(hi));
            std::array::from_fn(|k| a[k] * (1.0 - t) + b[k] * t)
        })
    }

    /// The light texture this picture gives for `input`; none with night vision on and no
    /// night vision rows.
    pub fn pixels(&self, input: &Input) -> Option<Pixels> {
        let top = if input.night_vision {
            if self.height < 4 * LEVELS {
                return None;
            }
            2 * LEVELS
        } else {
            0
        };
        let last = (self.width - 1) as f32;
        let sky = if input.lightning {
            1.0
        } else {
            (7.0 / 6.0 * (input.sky - 0.2)).clamp(0.0, 1.0)
        };
        let sky = self.column(sky * last, top);
        let block = self.column((input.flicker + 0.5).clamp(0.0, 1.0) * last, top + LEVELS);
        let gamma = input.gamma.clamp(0.0, 1.0);
        let mut out = [0u8; LEVELS * LEVELS * 4];
        for (s, sky) in sky.iter().enumerate() {
            for (b, block) in block.iter().enumerate() {
                let i = (s * LEVELS + b) * 4;
                for k in 0..3 {
                    let mut v = (sky[k] + block[k] - input.darkness).clamp(0.0, 1.0);
                    // The brightness option lifts the dark end the way vanilla's curve does.
                    if gamma > EPSILON {
                        let u = 1.0 - v;
                        v = gamma * (1.0 - u * u * u * u) + (1.0 - gamma) * v;
                    }
                    out[i + k] = (v * 255.0) as u8;
                }
                out[i + 3] = 255;
            }
        }
        Some(out)
    }
}

/// One world's lightmap: its picture, and the rain and thunder ones it blends to.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldLightmap {
    pub base: Picture,
    pub rain: Option<Picture>,
    pub thunder: Option<Picture>,
}

impl WorldLightmap {
    /// The light texture of world `world` for `input`: the base picture's, blended with the rain
    /// and thunder pictures by the weather where the world has weather. A missing thunder picture
    /// is the rain one, and a missing rain picture the base one. None when no picture serves
    /// (night vision without its rows).
    pub fn pixels(&self, world: i32, input: &Input) -> Option<Pixels> {
        let weathered = !matches!(world, -1 | 1) && (self.rain.is_some() || self.thunder.is_some());
        if !weathered || (input.rain <= EPSILON && input.thunder <= EPSILON) {
            return self.base.pixels(input);
        }
        let rain_picture = self.rain.as_ref().unwrap_or(&self.base);
        let thunder_picture = self.thunder.as_ref().unwrap_or(rain_picture);
        // The thunder strength is the storm's share of the rain.
        let thunder = if input.rain > 0.0 {
            input.thunder / input.rain
        } else {
            input.thunder
        };
        let weights = [1.0 - input.rain, input.rain - thunder, thunder];
        let mut sum = [0f32; LEVELS * LEVELS * 3];
        let mut any = false;
        for (picture, weight) in [&self.base, rain_picture, thunder_picture]
            .into_iter()
            .zip(weights)
        {
            if weight <= EPSILON {
                continue;
            }
            let Some(pixels) = picture.pixels(input) else {
                continue;
            };
            any = true;
            for (n, total) in sum.iter_mut().enumerate() {
                *total += f32::from(pixels[n / 3 * 4 + n % 3]) * weight;
            }
        }
        any.then(|| {
            let mut out = [255u8; LEVELS * LEVELS * 4];
            for (n, total) in sum.iter().enumerate() {
                out[n / 3 * 4 + n % 3] = total.clamp(0.0, 255.0) as u8;
            }
            out
        })
    }
}

/// A pack's lightmaps, decoded, by world number.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lightmaps {
    pub worlds: BTreeMap<i32, WorldLightmap>,
}

impl Lightmaps {
    /// The overlay's lightmaps, with `decode` reading a resource path into its width, height
    /// and RGBA bytes. A world whose picture does not decode, or is too small, has none; a rain
    /// or thunder picture that does not is left out.
    pub fn load(
        files: &Files,
        mut decode: impl FnMut(&str) -> Option<(u32, u32, Vec<u8>)>,
    ) -> Self {
        let mut picture = |path: &str| {
            let (width, height, rgba) = decode(path)?;
            let picture = Picture::from_rgba(width, height, &rgba);
            if picture.is_none() {
                log::warn!("{path}: a lightmap is at least 16x32 pixels");
            }
            picture
        };
        let worlds = files
            .worlds
            .iter()
            .filter_map(|(&world, f)| {
                let base = picture(&f.base)?;
                Some((
                    world,
                    WorldLightmap {
                        base,
                        rain: f.rain.as_deref().and_then(&mut picture),
                        thunder: f.thunder.as_deref().and_then(&mut picture),
                    },
                ))
            })
            .collect();
        Self { worlds }
    }

    pub fn is_empty(&self) -> bool {
        self.worlds.is_empty()
    }

    /// Whether world `world` has a lightmap.
    pub fn has(&self, world: i32) -> bool {
        self.worlds.contains_key(&world)
    }

    /// World `world`'s light texture for `input`; none without a lightmap that serves.
    pub fn pixels(&self, world: i32, input: &Input) -> Option<Pixels> {
        self.worlds.get(&world)?.pixels(world, input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `width` by `height` picture whose pixel at (x, y) is `f(x, y)` (grey).
    fn picture(width: u32, height: u32, f: impl Fn(u32, u32) -> u8) -> Picture {
        let mut rgba = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let v = f(x, y);
                rgba.extend([v, v, v, 255]);
            }
        }
        Picture::from_rgba(width, height, &rgba).unwrap()
    }

    fn input() -> Input {
        Input {
            sky: 1.0,
            lightning: false,
            flicker: 0.0,
            gamma: 0.0,
            night_vision: false,
            darkness: 0.0,
            rain: 0.0,
            thunder: 0.0,
        }
    }

    fn red(p: &Pixels, sky: usize, block: usize) -> u8 {
        p[(sky * LEVELS + block) * 4]
    }

    #[test]
    fn too_small_pictures_are_refused() {
        assert!(Picture::from_rgba(15, 32, &[0; 15 * 32 * 4]).is_none());
        assert!(Picture::from_rgba(16, 31, &[0; 16 * 31 * 4]).is_none());
        assert!(Picture::from_rgba(16, 32, &[0; 16 * 32 * 4 - 1]).is_none());
        assert!(Picture::from_rgba(16, 32, &[0; 16 * 32 * 4]).is_some());
    }

    #[test]
    fn sky_and_block_rows_add_up() {
        // Sky rows at 10 per level, block rows at 5 per level.
        let p = picture(16, 32, |_, y| {
            if y < 16 {
                y as u8 * 10
            } else {
                (y - 16) as u8 * 5
            }
        });
        let out = p.pixels(&input()).unwrap();
        assert_eq!(red(&out, 0, 0), 0);
        assert_eq!(red(&out, 3, 0), 30);
        assert_eq!(red(&out, 0, 4), 20);
        assert_eq!(red(&out, 3, 4), 50);
        assert_eq!(out[3], 255);
    }

    #[test]
    fn the_sky_column_follows_the_sky_light() {
        // Each column is its own brightness: x * 17 (0 to 255).
        let p = picture(16, 32, |x, y| if y < 16 { x as u8 * 17 } else { 0 });
        let at = |sky| red(&p.pixels(&Input { sky, ..input() }).unwrap(), 15, 0);
        // A sky factor of 1 (noon) is 14/15 of the way across; only a sky past it, or a flash,
        // reaches the right-hand column.
        assert!((237..=239).contains(&at(1.0)), "{}", at(1.0));
        assert_eq!(at(1.1), 255);
        assert_eq!(at(0.2), 0, "a sky at 0.2 and below is the left-hand one");
        assert_eq!(at(0.0), 0);
        // Lightning lights the sky fully whatever the time.
        let flash = p
            .pixels(&Input {
                sky: 0.0,
                lightning: true,
                ..input()
            })
            .unwrap();
        assert_eq!(red(&flash, 15, 0), 255);
    }

    #[test]
    fn columns_are_blended_between_pixels() {
        // Two columns wide in effect: left 0, everything right of it 255.
        let p = picture(17, 32, |x, y| if y < 16 && x > 0 { 255 } else { 0 });
        // A sky column at x = 0.5 of 16 columns: halfway between pixels 0 and 1.
        let sky = 0.5 / 16.0 * 6.0 / 7.0 + 0.2;
        let v = red(&p.pixels(&Input { sky, ..input() }).unwrap(), 0, 0);
        assert!((126..=128).contains(&v), "{v}");
    }

    #[test]
    fn darkness_and_gamma() {
        let p = picture(16, 32, |_, y| if y < 16 { 128 } else { 0 });
        let dark = p
            .pixels(&Input {
                darkness: 0.25,
                ..input()
            })
            .unwrap();
        assert_eq!(red(&dark, 0, 0), ((128.0 / 255.0 - 0.25) * 255.0) as u8);
        let bright = p
            .pixels(&Input {
                gamma: 1.0,
                ..input()
            })
            .unwrap();
        let v = 128.0f32 / 255.0;
        assert_eq!(
            red(&bright, 0, 0),
            ((1.0 - (1.0 - v).powi(4)) * 255.0) as u8
        );
    }

    #[test]
    fn night_vision_reads_its_own_rows_or_none() {
        assert!(
            picture(16, 32, |_, _| 0)
                .pixels(&Input {
                    night_vision: true,
                    ..input()
                })
                .is_none()
        );
        let p = picture(16, 64, |_, y| if (32..48).contains(&y) { 200 } else { 0 });
        let out = p
            .pixels(&Input {
                night_vision: true,
                ..input()
            })
            .unwrap();
        assert_eq!(red(&out, 5, 5), 200);
        assert_eq!(red(&p.pixels(&input()).unwrap(), 5, 5), 0);
    }

    fn world(base: u8, rain: Option<u8>, thunder: Option<u8>) -> WorldLightmap {
        let flat = |v: u8| picture(16, 32, move |_, y| if y < 16 { v } else { 0 });
        WorldLightmap {
            base: flat(base),
            rain: rain.map(flat),
            thunder: thunder.map(flat),
        }
    }

    #[test]
    fn weather_blends_the_rain_and_thunder_pictures() {
        let w = world(200, Some(100), Some(0));
        let at = |world, rain, thunder| {
            red(
                &w.pixels(
                    world,
                    &Input {
                        rain,
                        thunder,
                        ..input()
                    },
                )
                .unwrap(),
                0,
                0,
            )
        };
        assert_eq!(at(0, 0.0, 0.0), 200);
        assert_eq!(at(0, 1.0, 0.0), 100);
        assert_eq!(at(0, 0.5, 0.0), 150);
        assert_eq!(at(0, 1.0, 1.0), 0);
        // No weather in the Nether or the End.
        assert_eq!(at(-1, 1.0, 1.0), 200);
        assert_eq!(at(1, 1.0, 0.0), 200);
    }

    #[test]
    fn a_missing_thunder_picture_is_the_rain_one() {
        let w = world(200, Some(100), None);
        let out = w
            .pixels(
                0,
                &Input {
                    rain: 1.0,
                    thunder: 1.0,
                    ..input()
                },
            )
            .unwrap();
        assert_eq!(red(&out, 0, 0), 100);
        let only_base = world(200, None, None);
        let out = only_base
            .pixels(
                0,
                &Input {
                    rain: 1.0,
                    thunder: 1.0,
                    ..input()
                },
            )
            .unwrap();
        assert_eq!(red(&out, 0, 0), 200);
    }

    #[test]
    fn files_pair_each_world_with_its_weather() {
        let dir =
            std::env::temp_dir().join(format!("sunrise-optifine-lightmap-{}", std::process::id()));
        let at = |p: &str| {
            let path = dir.join(p);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"").unwrap();
        };
        at("assets/minecraft/optifine/lightmap/world0.png");
        at("assets/minecraft/optifine/lightmap/world0_rain.png");
        at("assets/minecraft/optifine/lightmap/world-1.png");
        at("assets/minecraft/mcpatcher/lightmap/world-1.png");
        at("assets/minecraft/mcpatcher/lightmap/world1.png");
        at("assets/minecraft/optifine/lightmap/notes.txt");
        let files = Files::scan(&ResourceSource::directory(dir.clone()));
        std::fs::remove_dir_all(&dir).ok();
        let o = "assets/minecraft/optifine/lightmap";
        let m = "assets/minecraft/mcpatcher/lightmap";
        assert_eq!(files.worlds.keys().copied().collect::<Vec<_>>(), [-1, 0, 1]);
        assert_eq!(files.worlds[&0].base, format!("{o}/world0.png"));
        assert_eq!(files.worlds[&0].rain, Some(format!("{o}/world0_rain.png")));
        assert_eq!(files.worlds[&0].thunder, None);
        assert_eq!(
            files.worlds[&-1].base,
            format!("{o}/world-1.png"),
            "optifine/ first"
        );
        assert_eq!(files.worlds[&1].base, format!("{m}/world1.png"));
    }

    #[test]
    fn load_drops_what_does_not_decode() {
        let files = Files {
            worlds: [
                (
                    0,
                    WorldFiles {
                        base: "a".into(),
                        rain: Some("bad".into()),
                        thunder: None,
                    },
                ),
                (
                    1,
                    WorldFiles {
                        base: "small".into(),
                        rain: None,
                        thunder: None,
                    },
                ),
            ]
            .into(),
        };
        let maps = Lightmaps::load(&files, |path| match path {
            "a" => Some((16, 32, vec![0; 16 * 32 * 4])),
            "small" => Some((8, 8, vec![0; 8 * 8 * 4])),
            _ => None,
        });
        assert!(maps.has(0) && !maps.has(1));
        assert!(maps.worlds[&0].rain.is_none());
    }
}
