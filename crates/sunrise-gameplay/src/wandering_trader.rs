//! `WanderingTraderSpawner`'s clock and the `WanderingTraderData` saved with the world.
use sunrise_core::nbt::Tag;

const TICK_DELAY: i32 = 1200;
const SPAWN_DELAY: i32 = 24000;
const MIN_CHANCE: i32 = 25;
const MAX_CHANCE: i32 = 75;

/// The spawner's clock and the `WanderingTraderData` saved with the world.
pub struct Spawner {
    tick_delay: i32,
    spawn_delay: i32,
    chance: i32,
    loaded: bool,
    dirty: bool,
}
impl Default for Spawner {
    fn default() -> Self {
        Self {
            tick_delay: TICK_DELAY,
            spawn_delay: SPAWN_DELAY,
            chance: MIN_CHANCE,
            loaded: false,
            dirty: false,
        }
    }
}
impl Spawner {
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }
    /// Reads the file's `data` compound (`None` when there is no file).
    pub fn load(&mut self, data: Option<&Tag>) {
        let get = |k: &str, d: i32| match data.and_then(|d| d.get(k)) {
            Some(Tag::Int(v)) => *v,
            _ => d,
        };
        self.spawn_delay = get("spawn_delay", SPAWN_DELAY);
        self.chance = get("spawn_chance", MIN_CHANCE);
        self.loaded = true;
    }
    /// The file to write, `None` unless loaded and changed.
    pub fn to_save(&self) -> Option<Tag> {
        if !self.loaded || !self.dirty {
            return None;
        }
        Some(Tag::compound([
            (
                "data",
                Tag::compound([
                    ("spawn_delay", Tag::Int(self.spawn_delay)),
                    ("spawn_chance", Tag::Int(self.chance)),
                ]),
            ),
            ("DataVersion", Tag::Int(sunrise_world::DATA_VERSION)),
        ]))
    }
    /// The file was written.
    pub fn saved(&mut self) {
        self.dirty = false;
    }
    /// One tick of the countdowns: the chance to roll against once the spawn delay runs out.
    pub fn due(&mut self) -> Option<i32> {
        self.tick_delay -= 1;
        if self.tick_delay > 0 {
            return None;
        }
        self.tick_delay = TICK_DELAY;
        self.spawn_delay -= TICK_DELAY;
        self.dirty = true;
        if self.spawn_delay > 0 {
            return None;
        }
        self.spawn_delay = SPAWN_DELAY;
        let chance = self.chance;
        self.chance = (chance + 25).clamp(MIN_CHANCE, MAX_CHANCE);
        Some(chance)
    }
    /// A trader appeared: the chance starts over.
    pub fn spawned(&mut self) {
        self.chance = MIN_CHANCE;
    }
}
