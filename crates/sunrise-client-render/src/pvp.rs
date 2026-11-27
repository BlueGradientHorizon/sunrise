//! Chat presentation settings.

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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pvp {
    pub chat: ChatOptions,
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
}
pub fn rows() -> Vec<Result<Opt, &'static str>> {
    use Opt::*;
    vec![
        Err("sunrise.pvp.header.chat"),
        Ok(ChatTimestamps),
        Ok(ChatCompact),
        Ok(ChatCopy),
        Ok(ChatSearch),
    ]
}
impl Opt {
    pub fn switch(self, p: &Pvp) -> Option<bool> {
        Some(match self {
            Opt::ChatTimestamps => p.chat.timestamps,
            Opt::ChatCompact => p.chat.compact,
            Opt::ChatCopy => p.chat.copy,
            Opt::ChatSearch => p.chat.search,
        })
    }
    pub fn cycle(self, p: &mut Pvp, _back: bool) {
        match self {
            Opt::ChatTimestamps => p.chat.timestamps = !p.chat.timestamps,
            Opt::ChatCompact => p.chat.compact = !p.chat.compact,
            Opt::ChatCopy => p.chat.copy = !p.chat.copy,
            Opt::ChatSearch => p.chat.search = !p.chat.search,
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
}
