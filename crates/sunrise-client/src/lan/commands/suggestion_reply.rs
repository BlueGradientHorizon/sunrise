//! Brigadier command completion replies at the play transport boundary.

use crate::command::{Suggestion, merge_suggestions};
use crate::lan::guest::Guest;
use sunrise_core::{
    nbt,
    vanilla_bytes::{Writer, varint_size},
};
use sunrise_protocol::{
    buf::{MAX_STRING_UTF16, write_utf8},
    connection::Framing,
    frame::{MAX_FRAME, MAX_UNCOMPRESSED},
    text::literal,
};

impl Guest {
    /// Send the post-hook completion reply, compacted at this guest's transport boundary.
    pub fn send_command_suggestions(
        &self,
        input: &str,
        id: i32,
        slash: bool,
        suggestions: Vec<Suggestion>,
    ) {
        if self.participates_online() {
            match frame_command_suggestions(
                input,
                id,
                slash,
                suggestions,
                self.sender.payload_framing(),
            ) {
                Ok(contents) => self.sender.send_frame_contents(contents),
                Err(error) => log::warn!(
                    "command suggestions for {} could not be framed: {error}",
                    self.name
                ),
            }
        }
    }
}

/// Project the final Brigadier suggestions onto this connection's wire packet. Start with
/// valid byte ranges, then expand, deduplicate and sort exactly as for local suggestions.
fn frame_command_suggestions(
    command: &str,
    id: i32,
    slash: bool,
    mut suggestions: Vec<Suggestion>,
    framing: Framing,
) -> sunrise_protocol::Result<Vec<u8>> {
    suggestions.retain(|s| {
        if command.get(s.range.0..s.range.1).is_some() {
            true
        } else {
            log::warn!(
                "command suggestion discarded: invalid byte range {:?}",
                s.range
            );
            false
        }
    });
    let ((start, end), suggestions) = merge_suggestions(command, suggestions);
    let mut header = Writer::new();
    header.varint(sunrise_protocol::ids::play::clientbound::COMMAND_SUGGESTIONS);
    header.varint(id);
    header.varint((command[..start].encode_utf16().count() + usize::from(slash)) as i32);
    header.varint(command[start..end].encode_utf16().count() as i32);

    // Varint21LengthFieldPrepender / CompressionDecoder: a necessary bound before compressing.
    // The codec below also checks the encoded length for this exact compression mode.
    let body_limit = match framing {
        Framing::Memory => usize::MAX,
        Framing::Network(None) => MAX_FRAME,
        Framing::Network(Some(_)) => MAX_UNCOMPRESSED,
    };
    let mut entries = Writer::new();
    let mut ends = vec![0];
    for suggestion in suggestions {
        // ServerCommandSuggestionsProvider.limitSuggestionCount, after sorting.
        if ends.len() > 1000 {
            log::warn!("command suggestions trimmed to vanilla's 1,000-entry packet limit");
            break;
        }
        let mut entry = Writer::new();
        if let Err(error) = write_utf8(&mut entry, &suggestion.text, MAX_STRING_UTF16) {
            log::warn!("command suggestion text discarded: {error}");
            continue;
        }
        entry.bool(suggestion.tooltip.is_some());
        if let Some(tip) = suggestion.tooltip {
            match nbt::write_network(&literal(&tip), nbt::Limits::network()) {
                Ok(bytes) => entry.bytes(&bytes),
                Err(error) => {
                    log::warn!("command suggestion tooltip discarded: {error}");
                    continue;
                }
            }
        }
        let len = entries.data.len() + entry.data.len();
        if header.data.len() + varint_size(ends.len() as i32) + len > body_limit {
            log::warn!("command suggestions trimmed to fit the connection's packet body");
            break;
        }
        entries.bytes(&entry.data);
        ends.push(len);
    }
    let frame = |count: usize| {
        let mut body = Writer::new();
        body.bytes(&header.data);
        body.varint(count as i32);
        body.bytes(&entries.data[..ends[count]]);
        framing.contents_owned(body.data)
    };
    let count = ends.len() - 1;
    match frame(count) {
        Err(error) if error.frame_limit().is_some() => {
            // No packet has been enqueued. Size the sorted prefix with the negotiated codec;
            // retain the actual frame contents instead of compressing the chosen packet twice.
            let (mut fits, mut refused) = (0, count);
            let mut contents = frame(0)?;
            while fits + 1 < refused {
                let mid = fits.midpoint(refused);
                match frame(mid) {
                    Ok(candidate) => {
                        fits = mid;
                        contents = candidate;
                    }
                    Err(error) if error.frame_limit().is_some() => refused = mid,
                    Err(error) => return Err(error),
                }
            }
            log::warn!("command suggestions trimmed from {count} to {fits}: {error}");
            Ok(contents)
        }
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Framing, MAX_FRAME, MAX_STRING_UTF16, MAX_UNCOMPRESSED, Suggestion,
        frame_command_suggestions,
    };
    use sunrise_protocol::{
        buf::Reader,
        frame::{self, Body, Inflater},
        ids::play::clientbound::COMMAND_SUGGESTIONS,
    };

    #[test]
    fn suggestion_packet_uses_final_subtoken_range_and_preserves_its_tooltip() {
        let reply = frame_command_suggestions(
            "go 😀abc",
            17,
            true,
            vec![
                Suggestion::new((7, 10), "zebra").with_tooltip("last"),
                Suggestion::new((8, 10), "ardvark").with_tooltip("first"),
                Suggestion::new((4, 10), "invalid inside the emoji").with_tooltip("ignored"),
            ],
            Framing::Network(None),
        )
        .unwrap();
        let mut r = Reader::new(&reply);
        assert_eq!(r.varint().unwrap(), COMMAND_SUGGESTIONS);
        assert_eq!(r.varint().unwrap(), 17);
        assert_eq!(r.varint().unwrap(), 6); // slash + `go ` + the surrogate pair
        assert_eq!(r.varint().unwrap(), 3); // `abc`
        assert_eq!(r.varint().unwrap(), 2);
        for (text, tip) in [("aardvark", "first"), ("zebra", "last")] {
            assert_eq!(r.string(MAX_STRING_UTF16).unwrap(), text);
            assert!(r.bool().unwrap());
            assert_eq!(r.nbt().unwrap(), super::literal(tip));
        }
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn suggestions_send_a_sorted_prefix_through_the_negotiated_transport_limits() {
        // Each entry is a legal maximal STRING_UTF8. Together the entries exceed both
        // Varint21's frame length and CompressionDecoder's uncompressed length.
        for (threshold, limit) in [(None, MAX_FRAME), (Some(256), MAX_UNCOMPRESSED)] {
            let values = (0..100)
                .rev()
                .map(|idx| {
                    let text = format!("{idx:02}{}", "界".repeat(MAX_STRING_UTF16 - 2));
                    Suggestion::new((0, 0), text)
                })
                .collect();
            let contents =
                frame_command_suggestions("", 1, false, values, Framing::Network(threshold))
                    .unwrap();
            assert!(contents.len() <= MAX_FRAME);
            let decoded = frame::body(&contents, threshold, &mut Inflater::default()).unwrap();
            let data = match &decoded {
                Body::Plain(offset) => &contents[*offset..],
                Body::Inflated(bytes) => bytes.as_slice(),
            };
            let mut r = Reader::new(data);
            assert_eq!(r.varint().unwrap(), COMMAND_SUGGESTIONS);
            assert_eq!(r.varint().unwrap(), 1);
            assert_eq!(r.varint().unwrap(), 0);
            assert_eq!(r.varint().unwrap(), 0);
            const ENTRY: usize = (MAX_STRING_UTF16 - 2) * 3 + 2 + 3 + 1;
            let expected = (limit - 5) / ENTRY;
            assert_eq!(r.varint().unwrap(), expected as i32);
            for idx in 0..expected {
                assert!(
                    r.string(MAX_STRING_UTF16)
                        .unwrap()
                        .starts_with(&format!("{idx:02}"))
                );
                assert!(!r.bool().unwrap());
            }
            assert_eq!(r.remaining(), 0);
        }
    }
}
