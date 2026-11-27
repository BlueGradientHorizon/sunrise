//! Chat and combat presentation settings.

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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pvp {
    pub chat: ChatOptions,
    pub combat: Combat,
}
fn flag(value: &str, d: bool) -> bool {
    match value {
        "true" => true,
        "false" => false,
        _ => d,
    }
}
impl Pvp {
    pub fn parse_line(&mut self, key: &str, value: &str) -> bool {
        match key {
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
    pub fn serialize(&self, out: &mut String) {
        use std::fmt::Write;
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
pub fn rows() -> Vec<Result<Opt, &'static str>> {
    use Opt::*;
    vec![Err("sunrise.pvp.header.chat"), Ok(ChatTimestamps), Ok(ChatCompact), Ok(ChatCopy), Ok(ChatSearch), Err("sunrise.pvp.header.combat"), Ok(CombatFeel), Ok(CombatIndicator), Ok(CombatSounds)]
}
impl Opt {
    pub fn switch(self, p: &Pvp) -> Option<bool> {
        Some(match self {
            Opt::ChatTimestamps => p.chat.timestamps,
            Opt::ChatCompact => p.chat.compact,
            Opt::ChatCopy => p.chat.copy,
            Opt::ChatSearch => p.chat.search,
            Opt::CombatIndicator => p.combat.indicator,
            Opt::CombatSounds => p.combat.sounds,
            _ => return None,
        })
    }
    pub fn value(self, p: &Pvp) -> Option<String> {
        Some(match self {
            Opt::CombatFeel => match p.combat.legacy {
                true => crate::lang::get_or("sunrise.pvp.combat_feel.legacy", "Legacy (1.8.9)"),
                false => crate::lang::get_or("sunrise.pvp.combat_feel.modern", "Modern"),
            },
            _ => return None,
        })
    }
    pub fn cycle(self, p: &mut Pvp, _back: bool) {
        match self {
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
