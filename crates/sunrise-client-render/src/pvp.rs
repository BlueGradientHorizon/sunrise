//! PvP and HUD options (`docs/pvp.md`). Kept in
//! `sunrise-extras.txt` with the other extras ([`crate::extras::Extras::pvp`]) and shown on
//! Sunrise Settings > PvP & HUD. Every option is off by default, so the game looks like
//! vanilla until the player changes one, and none of them shows anything vanilla doesn't
//! already tell the player.

/// Solid colours the HUD options pick from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Colour {
    White,
    Red,
    Orange,
    Yellow,
    Green,
    Cyan,
    Blue,
    Magenta,
    Black,
}
impl Colour {
    pub const ALL: [Colour; 9] = [
        Colour::White,
        Colour::Red,
        Colour::Orange,
        Colour::Yellow,
        Colour::Green,
        Colour::Cyan,
        Colour::Blue,
        Colour::Magenta,
        Colour::Black,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Colour::White => "white",
            Colour::Red => "red",
            Colour::Orange => "orange",
            Colour::Yellow => "yellow",
            Colour::Green => "green",
            Colour::Cyan => "cyan",
            Colour::Blue => "blue",
            Colour::Magenta => "magenta",
            Colour::Black => "black",
        }
    }
    fn english(self) -> &'static str {
        match self {
            Colour::White => "White",
            Colour::Red => "Red",
            Colour::Orange => "Orange",
            Colour::Yellow => "Yellow",
            Colour::Green => "Green",
            Colour::Cyan => "Cyan",
            Colour::Blue => "Blue",
            Colour::Magenta => "Magenta",
            Colour::Black => "Black",
        }
    }
    /// The name in the current language.
    pub fn label(self) -> String {
        crate::lang::get_or(
            &format!("sunrise.pvp.colour.{}", self.name()),
            self.english(),
        )
    }
    /// Opaque ARGB.
    pub fn argb(self) -> u32 {
        match self {
            Colour::White => 0xFFFF_FFFF,
            Colour::Red => 0xFFFF_0000,
            Colour::Orange => 0xFFFF_A020,
            Colour::Yellow => 0xFFFF_FF40,
            Colour::Green => 0xFF40_FF40,
            Colour::Cyan => 0xFF40_FFFF,
            Colour::Blue => 0xFF40_80FF,
            Colour::Magenta => 0xFFFF_40FF,
            Colour::Black => 0xFF00_0000,
        }
    }
    fn parse(s: &str) -> Option<Colour> {
        Self::ALL.into_iter().find(|c| c.name() == s)
    }
    fn step_solid(self, back: bool) -> Colour {
        step(&Self::ALL, self, back)
    }
}
/// `percent` of 255, rounded.
fn alpha(percent: u8) -> u32 {
    (u32::from(percent.min(100)) * 255 + 50) / 100
}

/// The next (or previous) value of `all` after `v`, wrapping.
fn step<T: Copy + PartialEq>(all: &[T], v: T, back: bool) -> T {
    let i = all.iter().position(|&x| x == v).unwrap_or(0);
    let n = all.len();
    all[if back { (i + n - 1) % n } else { (i + 1) % n }]
}
/// `v` `by` up or down within `lo..=hi`, wrapping.
fn step_by(v: u8, (lo, hi): (u8, u8), by: u8, back: bool) -> u8 {
    match (back, v) {
        (false, v) if v >= hi => lo,
        (false, v) => (v + by).min(hi),
        (true, v) if v <= lo => hi,
        (true, v) => v.saturating_sub(by).max(lo),
    }
}

/// The HUD widgets (`pvp_hud`), each in a corner of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hud {
    /// The armour worn, head first, with each piece's durability.
    pub armour: bool,
    pub armour_corner: Corner,
    pub armour_percent: bool,
    /// The active effects: icon, name and level, time left. They replace vanilla's icons.
    pub effects: bool,
    pub effects_corner: Corner,
    /// How many of the held item the inventory holds (arrows with a bow or crossbow).
    pub held: bool,
    pub held_corner: Corner,
    /// The latency the server reports for this player (the tab list's bars), in multiplayer.
    pub ping: bool,
    pub ping_corner: Corner,
    /// The block position and the direction faced, as F3 shows them; hidden when F3 is
    /// reduced (`reducedDebugInfo`).
    pub coords: bool,
    pub coords_corner: Corner,
    /// The server's address, in multiplayer.
    pub server: bool,
    pub server_corner: Corner,
    /// Hits in a row the server confirmed, until this player is hurt or 3 seconds pass.
    pub combo: bool,
    pub combo_corner: Corner,
    /// Frames drawn in the last second.
    pub fps: bool,
    pub fps_corner: Corner,
    /// Left and right clicks in the last second.
    pub cps: bool,
    pub cps_corner: Corner,
    /// The movement keys, mouse buttons and jump, lit while held, with clicks per second.
    pub keys: bool,
    pub keys_corner: Corner,
    /// The computer's local time.
    pub clock: bool,
    pub clock_corner: Corner,
    /// How far the last hit landed from the eyes, for 3 seconds; it only measures.
    pub reach: bool,
    pub reach_corner: Corner,
    /// Horizontal speed in blocks per second.
    pub speed: bool,
    pub speed_corner: Corner,
    /// A dark box behind each widget.
    pub background: bool,
    /// The boxes' opacity, in percent.
    pub opacity: u8,
    /// The widgets' text colour (durability keeps its own).
    pub text: Colour,
    /// A shadow under the widgets' text, as vanilla's HUD text has.
    pub shadow: bool,
}
impl Hud {
    /// Whether any widget is on: with none, the HUD draws nothing and reads nothing.
    pub fn any(&self) -> bool {
        [
            self.armour,
            self.effects,
            self.held,
            self.ping,
            self.coords,
            self.server,
            self.combo,
            self.fps,
            self.cps,
            self.keys,
            self.clock,
            self.reach,
            self.speed,
        ]
        .contains(&true)
    }
}
impl Default for Hud {
    fn default() -> Self {
        Self {
            armour: false,
            armour_corner: Corner::BottomRight,
            armour_percent: false,
            effects: false,
            effects_corner: Corner::TopRight,
            held: false,
            held_corner: Corner::BottomRight,
            ping: false,
            ping_corner: Corner::TopLeft,
            coords: false,
            coords_corner: Corner::TopLeft,
            server: false,
            server_corner: Corner::TopLeft,
            combo: false,
            combo_corner: Corner::TopLeft,
            fps: false,
            fps_corner: Corner::TopLeft,
            cps: false,
            cps_corner: Corner::TopLeft,
            keys: false,
            keys_corner: Corner::BottomLeft,
            clock: false,
            clock_corner: Corner::TopRight,
            reach: false,
            reach_corner: Corner::TopLeft,
            speed: false,
            speed_corner: Corner::TopLeft,
            background: true,
            opacity: HUD_OPACITY,
            text: Colour::White,
            shadow: true,
        }
    }
}
pub use crate::extras::Corner;
/// The widgets' boxes at first: the extras' 0x78 alpha, 47%.
pub const HUD_OPACITY: u8 = 47;
pub const HUD_OPACITIES: (u8, u8) = (0, 100);
impl Hud {
    /// The boxes' colour, ARGB.
    pub fn background_argb(&self) -> u32 {
        alpha(self.opacity) << 24
    }
}
fn corner_english(c: Corner) -> &'static str {
    match c {
        Corner::TopLeft => "Top left",
        Corner::TopRight => "Top right",
        Corner::BottomLeft => "Bottom left",
        Corner::BottomRight => "Bottom right",
    }
}
fn corner_label(c: Corner) -> String {
    crate::lang::get_or(
        &format!("sunrise.pvp.corner.{}", c.name()),
        corner_english(c),
    )
}

/// Chat options.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChatOptions {
    /// The local time before each message.
    pub timestamps: bool,
    /// A message repeated in a row shows once with its count.
    pub compact: bool,
    /// A click on a message (where it has no click action) copies it.
    pub copy: bool,
    /// Ctrl+F in the chat searches the chat's messages.
    pub search: bool,
}
impl ChatOptions {
    /// What `chat::Chat` lays its lines out with.
    pub fn qol(&self) -> crate::chat::ChatQol {
        crate::chat::ChatQol {
            timestamps: self.timestamps,
            compact: self.compact,
        }
    }
}

/// Combat feel (`docs/pvp.md`): how fights look. Legacy draws them as 1.8.9 did; what the
/// game sends to a server never changes, so a vanilla server and its anticheat see 26.3.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Combat {
    /// Legacy (1.8.9) instead of Modern.
    pub legacy: bool,
    /// Keep the attack indicator under Legacy (1.8.9 had none).
    pub indicator: bool,
    /// Under Legacy, silence the attack sounds 1.9 added (1.8.9 played none on a hit).
    pub sounds: bool,
}
impl Combat {
    /// Whether this client leaves out sound `name` a server sent: under Legacy with its
    /// sounds, the `entity.player.attack.*` family (strong, weak, crit, sweep, knockback,
    /// nodamage), which 1.9 brought with the attack cooldown.
    pub fn mutes_sound(&self, name: &str) -> bool {
        let name = name.strip_prefix("minecraft:").unwrap_or(name);
        self.legacy && self.sounds && name.starts_with("entity.player.attack.")
    }
    /// Whether this client leaves out particles of `ty` a server sent: under Legacy, the
    /// sweep and the damage hearts, which 1.9 brought (1.8.9's hits show crits only).
    pub fn hides_particle(&self, ty: crate::particles::ParticleType) -> bool {
        use crate::particles::ParticleType as T;
        self.legacy && matches!(ty, T::SweepAttack | T::DamageIndicator)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pvp {
    pub hud: Hud,
    pub chat: ChatOptions,
    /// Where each HUD widget sits and its size.
    pub layout: crate::pvp_hud::Layout,
    /// The key that opens the HUD editor from the game (unbound at first).
    pub editor_key: String,
    pub combat: Combat,
}
/// The HUD editor key's name for the controls (`Extras::set_key`).
pub const HUD_EDITOR_KEY: &str = "sunrise.hud_editor";
impl Default for Pvp {
    fn default() -> Self {
        Self {
            hud: Hud::default(),
            chat: ChatOptions::default(),
            layout: crate::pvp_hud::Layout::default(),
            editor_key: "key.keyboard.unknown".into(),
            combat: Combat::default(),
        }
    }
}
fn flag(value: &str, d: bool) -> bool {
    match value {
        "true" => true,
        "false" => false,
        _ => d,
    }
}
fn num(value: &str, (lo, hi): (u8, u8), d: u8) -> u8 {
    value.parse::<u8>().map_or(d, |v| v.clamp(lo, hi))
}
impl Pvp {
    /// One `key:value` line of `sunrise-extras.txt`; false when the key isn't a PvP one.
    pub fn parse_line(&mut self, key: &str, value: &str) -> bool {
        let u = &mut self.hud;
        match key {
            "hudArmour" => u.armour = flag(value, u.armour),
            "hudArmourCorner" => u.armour_corner = Corner::parse(value, u.armour_corner),
            "hudArmourPercent" => u.armour_percent = flag(value, u.armour_percent),
            "hudEffects" => u.effects = flag(value, u.effects),
            "hudEffectsCorner" => u.effects_corner = Corner::parse(value, u.effects_corner),
            "hudHeld" => u.held = flag(value, u.held),
            "hudHeldCorner" => u.held_corner = Corner::parse(value, u.held_corner),
            "hudBackground" => u.background = flag(value, u.background),
            "hudOpacity" => u.opacity = num(value, HUD_OPACITIES, u.opacity),
            "hudTextColour" => u.text = Colour::parse(value).unwrap_or(u.text),
            "hudShadow" => u.shadow = flag(value, u.shadow),
            "hudPlace" => self.layout.parse(value),
            "hudEditorKey" => {
                if value.starts_with("key.") {
                    self.editor_key = value.to_owned();
                }
            }
            "hudPing" => u.ping = flag(value, u.ping),
            "hudPingCorner" => u.ping_corner = Corner::parse(value, u.ping_corner),
            "hudCoords" => u.coords = flag(value, u.coords),
            "hudCoordsCorner" => u.coords_corner = Corner::parse(value, u.coords_corner),
            "hudServer" => u.server = flag(value, u.server),
            "hudServerCorner" => u.server_corner = Corner::parse(value, u.server_corner),
            "hudCombo" => u.combo = flag(value, u.combo),
            "hudComboCorner" => u.combo_corner = Corner::parse(value, u.combo_corner),
            "hudFps" => u.fps = flag(value, u.fps),
            "hudFpsCorner" => u.fps_corner = Corner::parse(value, u.fps_corner),
            "hudCps" => u.cps = flag(value, u.cps),
            "hudCpsCorner" => u.cps_corner = Corner::parse(value, u.cps_corner),
            "hudKeys" => u.keys = flag(value, u.keys),
            "hudKeysCorner" => u.keys_corner = Corner::parse(value, u.keys_corner),
            "hudClock" => u.clock = flag(value, u.clock),
            "hudClockCorner" => u.clock_corner = Corner::parse(value, u.clock_corner),
            "hudReach" => u.reach = flag(value, u.reach),
            "hudReachCorner" => u.reach_corner = Corner::parse(value, u.reach_corner),
            "hudSpeed" => u.speed = flag(value, u.speed),
            "hudSpeedCorner" => u.speed_corner = Corner::parse(value, u.speed_corner),
            "chatTimestamps" => self.chat.timestamps = flag(value, self.chat.timestamps),
            "chatCompact" => self.chat.compact = flag(value, self.chat.compact),
            "chatCopy" => self.chat.copy = flag(value, self.chat.copy),
            "chatSearch" => self.chat.search = flag(value, self.chat.search),
            "combatFeel" => match value {
                "legacy" => self.combat.legacy = true,
                "modern" => self.combat.legacy = false,
                _ => {}
            },
            "combatIndicator" => self.combat.indicator = flag(value, self.combat.indicator),
            "combatSounds" => self.combat.sounds = flag(value, self.combat.sounds),
            _ => return false,
        }
        true
    }
    /// The PvP lines of `sunrise-extras.txt`.
    pub fn serialize(&self, out: &mut String) {
        use std::fmt::Write;
        let u = &self.hud;
        let _ = write!(
            out,
            "hudArmour:{}\nhudArmourCorner:{}\nhudArmourPercent:{}\nhudEffects:{}\nhudEffectsCorner:{}\nhudHeld:{}\nhudHeldCorner:{}\nhudBackground:{}\n",
            u.armour,
            u.armour_corner.name(),
            u.armour_percent,
            u.effects,
            u.effects_corner.name(),
            u.held,
            u.held_corner.name(),
            u.background,
        );
        let _ = write!(
            out,
            "hudPing:{}\nhudPingCorner:{}\nhudCoords:{}\nhudCoordsCorner:{}\nhudServer:{}\nhudServerCorner:{}\nhudCombo:{}\nhudComboCorner:{}\n",
            u.ping,
            u.ping_corner.name(),
            u.coords,
            u.coords_corner.name(),
            u.server,
            u.server_corner.name(),
            u.combo,
            u.combo_corner.name(),
        );
        let _ = write!(
            out,
            "hudFps:{}\nhudFpsCorner:{}\nhudCps:{}\nhudCpsCorner:{}\nhudKeys:{}\nhudKeysCorner:{}\n",
            u.fps,
            u.fps_corner.name(),
            u.cps,
            u.cps_corner.name(),
            u.keys,
            u.keys_corner.name(),
        );
        let _ = write!(
            out,
            "hudClock:{}\nhudClockCorner:{}\nhudReach:{}\nhudReachCorner:{}\nhudSpeed:{}\nhudSpeedCorner:{}\n",
            u.clock,
            u.clock_corner.name(),
            u.reach,
            u.reach_corner.name(),
            u.speed,
            u.speed_corner.name(),
        );
        let _ = write!(
            out,
            "hudOpacity:{}\nhudTextColour:{}\nhudShadow:{}\n",
            u.opacity,
            u.text.name(),
            u.shadow,
        );
        self.layout.serialize(out);
        let _ = writeln!(out, "hudEditorKey:{}", self.editor_key);
        let c = &self.chat;
        let _ = write!(
            out,
            "chatTimestamps:{}\nchatCompact:{}\nchatCopy:{}\nchatSearch:{}\n",
            c.timestamps, c.compact, c.copy, c.search,
        );
        let c = &self.combat;
        let feel = if c.legacy { "legacy" } else { "modern" };
        let _ = write!(
            out,
            "combatFeel:{feel}\ncombatIndicator:{}\ncombatSounds:{}\n",
            c.indicator, c.sounds
        );
    }
}

/// The options of the PvP & HUD page: each one's id (its language keys are `sunrise.pvp.<id>`
/// and `sunrise.pvp.<id>.tooltip`), English caption and description (en_us.json has the same
/// text; a test keeps them equal).
macro_rules! opts {
    ($($name:ident, $id:literal, $caption:literal, $description:literal;)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Opt { $($name),* }
        impl Opt {
            pub const ALL: &'static [Opt] = &[$(Opt::$name),*];
            pub fn caption_key(self) -> &'static str {
                match self { $(Opt::$name => concat!("sunrise.pvp", ".", $id)),* }
            }
            pub fn description_key(self) -> &'static str {
                match self { $(Opt::$name => concat!("sunrise.pvp", ".", $id, ".tooltip")),* }
            }
            pub fn caption(self) -> &'static str {
                match self { $(Opt::$name => $caption),* }
            }
            pub fn description(self) -> &'static str {
                match self { $(Opt::$name => $description),* }
            }
        }
    };
}
opts! {
    HudArmour, "hud_armour", "Armour",
        "The armour you wear, head first, with how many uses each piece has left.";
    HudArmourCorner, "hud_armour_corner", "Armour corner", "Where the armour shows.";
    HudArmourPercent, "hud_armour_percent", "Durability in %",
        "Uses left as a percent of the piece's maximum instead of a number.";
    HudEffects, "hud_effects", "Effects",
        "Your effects with their names, levels and time left, in place of the icons at the top right.";
    HudEffectsCorner, "hud_effects_corner", "Effects corner", "Where the effects show.";
    HudHeld, "hud_held", "Held item count",
        "How many of the item in your hand your inventory holds; arrows with a bow or crossbow.";
    HudHeldCorner, "hud_held_corner", "Held count corner", "Where the count shows.";
    HudPing, "hud_ping", "Ping", "The latency the server reports for you, in milliseconds. Multiplayer only.";
    HudPingCorner, "hud_ping_corner", "Ping corner", "Where the ping shows.";
    HudCoords, "hud_coords", "Coordinates",
        "Your block position and the direction you face, as F3 shows them. Hidden when the world or server reduces F3.";
    HudCoordsCorner, "hud_coords_corner", "Coordinates corner", "Where the coordinates show.";
    HudServer, "hud_server", "Server address", "The address of the server you play on. Multiplayer only.";
    HudServerCorner, "hud_server_corner", "Server corner", "Where the address shows.";
    HudCombo, "hud_combo", "Combo counter",
        "Hits in a row the server confirmed, until you are hurt or 3 seconds pass without one.";
    HudComboCorner, "hud_combo_corner", "Combo corner", "Where the combo shows.";
    HudFps, "hud_fps", "FPS", "Frames drawn in the last second.";
    HudFpsCorner, "hud_fps_corner", "FPS corner", "Where the FPS shows.";
    HudCps, "hud_cps", "Clicks per second", "Your left and right clicks in the last second.";
    HudCpsCorner, "hud_cps_corner", "CPS corner", "Where the clicks per second show.";
    HudKeys, "hud_keys", "Keystrokes",
        "Your movement keys, mouse buttons and jump, lit while you hold them, with clicks per second.";
    HudKeysCorner, "hud_keys_corner", "Keystrokes corner", "Where the keystrokes show.";
    HudClock, "hud_clock", "Clock", "Your computer's time.";
    HudClockCorner, "hud_clock_corner", "Clock corner", "Where the clock shows.";
    HudReach, "hud_reach", "Reach",
        "How far from your eyes your last hit landed, for 3 seconds. It only measures: your reach stays vanilla's.";
    HudReachCorner, "hud_reach_corner", "Reach corner", "Where the reach shows.";
    HudSpeed, "hud_speed", "Speed", "How fast you move across the ground, in blocks per second.";
    HudSpeedCorner, "hud_speed_corner", "Speed corner", "Where the speed shows.";
    HudBackground, "hud_background", "Widget backgrounds", "A dark box behind each widget.";
    HudOpacity, "hud_opacity", "Background opacity", "How dark the boxes behind the widgets are.";
    HudTextColour, "hud_text_colour", "Text colour",
        "The widgets' text colour. Durability keeps its green to red.";
    HudShadow, "hud_shadow", "Text shadow", "A shadow under the widgets' text, as vanilla's HUD text has.";
    HudEditorKey, "hud_editor_key", "HUD editor key",
        "Opens the HUD editor from the game. Click, then press a key; Escape unbinds it.";
    ChatTimestamps, "chat_timestamps", "Timestamps", "The time each message came in, before it.";
    ChatCompact, "chat_compact", "Compact chat",
        "A message repeated in a row shows once, with how many times it came: (x3).";
    ChatCopy, "chat_copy", "Copy on click",
        "Clicking a message in the open chat copies its text, unless the click does something else.";
    ChatSearch, "chat_search", "Chat search",
        "Ctrl+F in the open chat: what you type shows only the messages holding it. Enter or Ctrl+F again stops.";
    CombatFeel, "combat_feel", "Combat feel",
        "Legacy draws fights as 1.8.9 did: the item in your hand, its swing, eating, the bow and a sword held up while you hold use, without the dip after each hit or the attack indicator. Only the look: what you send a server stays the same.";
    CombatIndicator, "combat_indicator", "Attack indicator in Legacy",
        "Keeps the attack indicator under Legacy, which 1.8.9 didn't have.";
    CombatSounds, "combat_sounds", "1.8 hit sounds",
        "Under Legacy, leaves out the attack sounds 1.9 added (strong, weak, sweep, critical...). Hurt sounds stay.";
}
/// The PvP & HUD page: headers (language keys) and options.
pub fn rows() -> Vec<Result<Opt, &'static str>> {
    use Opt::*;
    let mut rows = vec![Err("sunrise.pvp.header.hud")];
    rows.extend(
        [
            HudArmour,
            HudArmourCorner,
            HudArmourPercent,
            HudEffects,
            HudEffectsCorner,
            HudHeld,
            HudHeldCorner,
            HudPing,
            HudPingCorner,
            HudCoords,
            HudCoordsCorner,
            HudServer,
            HudServerCorner,
            HudCombo,
            HudComboCorner,
            HudFps,
            HudFpsCorner,
            HudCps,
            HudCpsCorner,
            HudKeys,
            HudKeysCorner,
            HudClock,
            HudClockCorner,
            HudReach,
            HudReachCorner,
            HudSpeed,
            HudSpeedCorner,
            HudBackground,
            HudOpacity,
            HudTextColour,
            HudShadow,
            HudEditorKey,
        ]
        .map(Ok),
    );
    rows.push(Err("sunrise.pvp.header.chat"));
    rows.extend([ChatTimestamps, ChatCompact, ChatCopy, ChatSearch].map(Ok));
    rows.push(Err("sunrise.pvp.header.combat"));
    rows.extend([CombatFeel, CombatIndicator, CombatSounds].map(Ok));
    rows
}
impl Opt {
    /// The switch this option belongs to.
    pub fn parent(self) -> Option<Opt> {
        use Opt::*;
        match self {
            HudArmour | HudEffects | HudHeld | HudPing | HudCoords | HudServer | HudCombo
            | HudFps | HudCps | HudKeys | HudClock | HudReach | HudSpeed | HudBackground
            | HudTextColour | HudShadow | HudEditorKey => None,
            HudOpacity => Some(HudBackground),
            HudFpsCorner => Some(HudFps),
            HudCpsCorner => Some(HudCps),
            HudKeysCorner => Some(HudKeys),
            HudClockCorner => Some(HudClock),
            HudReachCorner => Some(HudReach),
            HudSpeedCorner => Some(HudSpeed),
            ChatTimestamps | ChatCompact | ChatCopy | ChatSearch => None,
            CombatFeel | CombatIndicator | CombatSounds => None,
            HudPingCorner => Some(HudPing),
            HudCoordsCorner => Some(HudCoords),
            HudServerCorner => Some(HudServer),
            HudComboCorner => Some(HudCombo),
            HudArmourCorner | HudArmourPercent => Some(HudArmour),
            HudEffectsCorner => Some(HudEffects),
            HudHeldCorner => Some(HudHeld),
        }
    }
    pub fn switch(self, p: &Pvp) -> Option<bool> {
        Some(match self {
            Opt::HudArmour => p.hud.armour,
            Opt::HudArmourPercent => p.hud.armour_percent,
            Opt::HudEffects => p.hud.effects,
            Opt::HudHeld => p.hud.held,
            Opt::HudBackground => p.hud.background,
            Opt::HudShadow => p.hud.shadow,
            Opt::HudPing => p.hud.ping,
            Opt::HudCoords => p.hud.coords,
            Opt::HudServer => p.hud.server,
            Opt::HudCombo => p.hud.combo,
            Opt::HudFps => p.hud.fps,
            Opt::HudCps => p.hud.cps,
            Opt::HudKeys => p.hud.keys,
            Opt::HudClock => p.hud.clock,
            Opt::HudReach => p.hud.reach,
            Opt::HudSpeed => p.hud.speed,
            Opt::ChatTimestamps => p.chat.timestamps,
            Opt::ChatCompact => p.chat.compact,
            Opt::ChatCopy => p.chat.copy,
            Opt::ChatSearch => p.chat.search,
            Opt::CombatIndicator => p.combat.indicator,
            Opt::CombatSounds => p.combat.sounds,
            _ => return None,
        })
    }
    /// The value its button shows, for the options that aren't switches.
    pub fn value(self, p: &Pvp) -> Option<String> {
        Some(match self {
            Opt::HudArmourCorner => corner_label(p.hud.armour_corner),
            Opt::HudEffectsCorner => corner_label(p.hud.effects_corner),
            Opt::HudHeldCorner => corner_label(p.hud.held_corner),
            Opt::HudPingCorner => corner_label(p.hud.ping_corner),
            Opt::HudCoordsCorner => corner_label(p.hud.coords_corner),
            Opt::HudServerCorner => corner_label(p.hud.server_corner),
            Opt::HudComboCorner => corner_label(p.hud.combo_corner),
            Opt::HudFpsCorner => corner_label(p.hud.fps_corner),
            Opt::HudCpsCorner => corner_label(p.hud.cps_corner),
            Opt::HudKeysCorner => corner_label(p.hud.keys_corner),
            Opt::HudClockCorner => corner_label(p.hud.clock_corner),
            Opt::HudReachCorner => corner_label(p.hud.reach_corner),
            Opt::HudSpeedCorner => corner_label(p.hud.speed_corner),
            Opt::HudOpacity => format!("{}%", p.hud.opacity),
            Opt::HudTextColour => p.hud.text.label(),
            Opt::HudEditorKey => crate::keybinds::display_name(&p.editor_key).plain(),
            Opt::CombatFeel => match p.combat.legacy {
                true => crate::lang::get_or("sunrise.pvp.combat_feel.legacy", "Legacy (1.8.9)"),
                false => crate::lang::get_or("sunrise.pvp.combat_feel.modern", "Modern"),
            },
            _ => return None,
        })
    }
    /// A click; `back` (right button, wheel up) goes the other way.
    pub fn cycle(self, p: &mut Pvp, back: bool) {
        let u = &mut p.hud;
        let corner = |c: Corner| step(&Corner::ALL, c, back);
        match self {
            Opt::HudArmour => u.armour = !u.armour,
            Opt::HudArmourCorner => u.armour_corner = corner(u.armour_corner),
            Opt::HudArmourPercent => u.armour_percent = !u.armour_percent,
            Opt::HudEffects => u.effects = !u.effects,
            Opt::HudEffectsCorner => u.effects_corner = corner(u.effects_corner),
            Opt::HudHeld => u.held = !u.held,
            Opt::HudHeldCorner => u.held_corner = corner(u.held_corner),
            Opt::HudBackground => u.background = !u.background,
            Opt::HudOpacity => u.opacity = step_by(u.opacity, HUD_OPACITIES, 10, back),
            Opt::HudTextColour => u.text = u.text.step_solid(back),
            Opt::HudShadow => u.shadow = !u.shadow,
            // Bound through the controls' key capture (`Item::key_name`).
            Opt::HudEditorKey => {}
            Opt::HudPing => u.ping = !u.ping,
            Opt::HudPingCorner => u.ping_corner = corner(u.ping_corner),
            Opt::HudCoords => u.coords = !u.coords,
            Opt::HudCoordsCorner => u.coords_corner = corner(u.coords_corner),
            Opt::HudServer => u.server = !u.server,
            Opt::HudServerCorner => u.server_corner = corner(u.server_corner),
            Opt::HudCombo => u.combo = !u.combo,
            Opt::HudComboCorner => u.combo_corner = corner(u.combo_corner),
            Opt::HudFps => u.fps = !u.fps,
            Opt::HudFpsCorner => u.fps_corner = corner(u.fps_corner),
            Opt::HudCps => u.cps = !u.cps,
            Opt::HudCpsCorner => u.cps_corner = corner(u.cps_corner),
            Opt::HudKeys => u.keys = !u.keys,
            Opt::HudKeysCorner => u.keys_corner = corner(u.keys_corner),
            Opt::HudClock => u.clock = !u.clock,
            Opt::HudClockCorner => u.clock_corner = corner(u.clock_corner),
            Opt::HudReach => u.reach = !u.reach,
            Opt::HudReachCorner => u.reach_corner = corner(u.reach_corner),
            Opt::HudSpeed => u.speed = !u.speed,
            Opt::HudSpeedCorner => u.speed_corner = corner(u.speed_corner),
            Opt::ChatTimestamps => p.chat.timestamps = !p.chat.timestamps,
            Opt::ChatCompact => p.chat.compact = !p.chat.compact,
            Opt::ChatCopy => p.chat.copy = !p.chat.copy,
            Opt::ChatSearch => p.chat.search = !p.chat.search,
            Opt::CombatFeel => p.combat.legacy = !p.combat.legacy,
            Opt::CombatIndicator => p.combat.indicator = !p.combat.indicator,
            Opt::CombatSounds => p.combat.sounds = !p.combat.sounds,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_round_trip_and_cycle_both_ways() {
        let d = Pvp::default();
        let mut text = String::new();
        d.serialize(&mut text);
        let mut back = Pvp::default();
        for line in text.lines() {
            let (k, v) = line.split_once(':').unwrap();
            assert!(back.parse_line(k, v), "{k}");
        }
        assert_eq!(back, d);
        let mut p = Pvp::default();
        for opt in rows().into_iter().flatten() {
            opt.cycle(&mut p, false);
        }
        let c = p.chat;
        assert!(c.timestamps && c.compact && c.copy && c.search);
        let mut text = String::new();
        p.serialize(&mut text);
        let mut read = Pvp::default();
        text.lines().for_each(|l| {
            let (k, v) = l.split_once(':').unwrap();
            read.parse_line(k, v);
        });
        assert_eq!(read, p);
        assert!(!p.parse_line("fullbright", "true"));
    }

    #[test]
    fn every_option_and_value_has_its_english_text() {
        let en = crate::lang::Language::load(&[], "en_us");
        for opt in Opt::ALL {
            assert_eq!(en.get(opt.caption_key()), Some(opt.caption()), "{opt:?}");
            assert_eq!(
                en.get(opt.description_key()),
                Some(opt.description()),
                "{opt:?}"
            );
        }
        for header in rows().into_iter().filter_map(Result::err) {
            assert!(en.get(header).is_some(), "{header}");
        }
        for c in Corner::ALL {
            let key = format!("sunrise.pvp.corner.{}", c.name());
            assert!(en.get(&key).is_some(), "{key}");
        }
        let names = Colour::ALL.map(|c| (format!("sunrise.pvp.colour.{}", c.name()), c.english()));
        for (key, english) in &names {
            assert_eq!(en.get(key), Some(*english), "{key}");
        }
    }

    #[test]
    fn each_corner_label_comes_from_its_own_catalogue_key() {
        let fixture = crate::lang::Language::of(
            "xx_xx",
            Corner::ALL.map(|c| {
                (
                    format!("sunrise.pvp.corner.{}", c.name()),
                    format!("fixture {}", c.name()),
                )
            }),
        );
        let (labels, missed) = crate::lang::scoped(fixture, || Corner::ALL.map(corner_label));
        assert!(missed.is_empty());
        for (c, label) in Corner::ALL.into_iter().zip(labels) {
            assert_eq!(label, format!("fixture {}", c.name()));
        }
    }

    #[test]
    fn legacy_leaves_out_what_1_9_added_to_a_hit() {
        use crate::particles::ParticleType as T;
        let modern = Combat::default();
        let legacy = Combat {
            legacy: true,
            ..Combat::default()
        };
        let sounds = Combat {
            sounds: true,
            ..legacy
        };
        for name in [
            "minecraft:entity.player.attack.strong",
            "entity.player.attack.sweep",
            "minecraft:entity.player.attack.nodamage",
        ] {
            assert!(sounds.mutes_sound(name), "{name}");
            assert!(!legacy.mutes_sound(name), "only with the sounds option");
            assert!(!modern.mutes_sound(name));
        }
        // 1.8.9 had the hurt sound: it stays.
        assert!(!sounds.mutes_sound("minecraft:entity.player.hurt"));
        assert!(legacy.hides_particle(T::SweepAttack) && legacy.hides_particle(T::DamageIndicator));
        assert!(!legacy.hides_particle(T::Crit) && !legacy.hides_particle(T::EnchantedHit));
        assert!(!modern.hides_particle(T::SweepAttack));
    }
}
