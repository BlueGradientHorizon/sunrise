//! The light of columns the level does not hold: what vanilla's light storage keeps for them.
//!
//! Storage keeps a section while it or one of its 26 neighbours is non-air
//! (`LayerLightSectionStorage.updateSectionStatus`), whether or not its column is loaded. A
//! column that unloads keeps the last light of the sections a loaded neighbour still touches
//! (`ThreadedLevelLightEngine.updateChunkStatus`, `ClientPacketListener.queueLightRemoval`); a
//! column never loaded has dark sections where a loaded neighbour touches it
//! (`LayerLightSectionStorage.createDataLayer`: light is not enabled there). `World::light` reads
//! both here, for a column with no chunk; a loaded column's light is its own chunk's.

use crate::chunk::{Chunk, Light, index};
use crate::chunk_map::ChunkMap;
use crate::world::World;
use sunrise_world::registry::BlockStateId;

/// The light an unloaded column kept, by section Y (a section coordinate), lowest first.
pub struct Border(Vec<(i32, Light)>);

/// The loaded columns of the eight around a column.
type Around<'a> = [Option<&'a Chunk>; 8];

fn around(chunks: &ChunkMap, (x, z): (i32, i32)) -> Around<'_> {
    let mut out = [None; 8];
    let ring = (-1..=1).flat_map(|dz| (-1..=1).map(move |dx| (dx, dz)));
    for (slot, (dx, dz)) in out.iter_mut().zip(ring.filter(|&d| d != (0, 0))) {
        *slot = chunks.get(&(x + dx, z + dz)).map(|chunk| &**chunk);
    }
    out
}

/// Whether a section of a loaded column in `around` within one of section `y` holds a block that is not `air`.
fn touched(around: &Around, air: BlockStateId, y: i32) -> bool {
    around.iter().flatten().any(|c| {
        (y - 1..=y + 1).any(|t| {
            usize::try_from(t - (c.min_y() >> 4))
                .ok()
                .and_then(|i| c.sections.get(i))
                .is_some_and(|s| !s.is_uniform(air.0))
        })
    })
}

impl World {
    /// The light at a block of a column with no chunk: its retained sections, dark where only a
    /// neighbour touches it, and for the rest `SkyLightSectionStorage.getLightValue`'s rule (the
    /// bottom row of the nearest retained section above, full sky past the highest stored one).
    /// Never loads a chunk.
    #[cold]
    #[inline(never)]
    pub(crate) fn border_light(&self, key: (i32, i32), x: i32, y: i32, z: i32) -> u8 {
        let around = around(&self.chunks, key);
        let retained = self.border.get(&key).map_or(&[][..], |b| &b.0);
        let find = |t: i32| {
            retained
                .binary_search_by_key(&t, |&(y, _)| y)
                .ok()
                .map(|i| &retained[i].1)
        };
        let s = y >> 4;
        if let Some(light) = find(s) {
            return light.get(index(x, y, z));
        }
        let layout = self.shape.column();
        let first = (layout.min_y() >> 4) - 1;
        let mut stored = touched(&around, self.air, s);
        for t in (s + 1).max(first)..=first + layout.sections() as i32 + 1 {
            if let Some(light) = find(t) {
                return light.get(index(x, 0, z)) & 0xF0;
            }
            stored |= touched(&around, self.air, t);
        }
        let light = if stored { 0 } else { 0xF0 };
        light & (0x0F | (u8::from(self.shape.sky_light) * 0xF0))
    }

    /// `chunk` has just left the level: it keeps the light of the sections a loaded neighbour
    /// touches, and the columns around it lose what only it touched.
    pub(crate) fn retain_border(&mut self, key: (i32, i32), chunk: &Chunk) {
        let air = self.air;
        let first = (chunk.min_y() >> 4) - 1;
        let last = first + chunk.sections.len() as i32 + 1;
        let around_key = around(&self.chunks, key);
        let kept: Vec<(i32, Light)> = (first..=last)
            .filter(|&t| touched(&around_key, air, t))
            .map(|t| {
                let stored = match t {
                    _ if t == first => chunk.pad_below.as_ref(),
                    _ if t == last => chunk.pad_above.as_ref(),
                    _ => Some(&chunk.sections[(t - first - 1) as usize]),
                };
                let light = match stored {
                    Some(section) => section.light.clone(),
                    // Not stored: the light the rule gave there.
                    None => {
                        let mut data = Box::new([0u8; 4096]);
                        for (i, v) in data.iter_mut().enumerate() {
                            *v = chunk.light(
                                (i & 15) as i32,
                                t * 16 + (i >> 8) as i32,
                                (i >> 4 & 15) as i32,
                            );
                        }
                        Light::packed(data)
                    }
                };
                (t, light)
            })
            .collect();
        if kept.is_empty() {
            self.border.remove(&key);
        } else {
            self.border.insert(key, Border(kept));
        }
        for dz in -1..=1 {
            for dx in -1..=1 {
                let near = (key.0 + dx, key.1 + dz);
                if near == key {
                    continue;
                }
                if self.chunks.contains_key(&near) {
                    self.border.remove(&near);
                    continue;
                }
                let around = around(&self.chunks, near);
                if let Some(border) = self.border.get_mut(&near) {
                    border.0.retain(|&(t, _)| touched(&around, air, t));
                    if border.0.is_empty() {
                        self.border.remove(&near);
                    }
                }
            }
        }
    }
}
