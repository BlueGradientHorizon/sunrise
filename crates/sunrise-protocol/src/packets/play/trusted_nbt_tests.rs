use super::*;
use crate::{buf::NBT_QUOTA, packets::configuration};

fn assert_trusted_limits<P: Codec<P> + std::fmt::Debug + PartialEq>(packet: impl Fn(Tag) -> P) {
    // Enough short text fragments to exceed the generic byte quota without
    // exceeding the individual NBT string limit.
    let fragment = "x".repeat(16 * 1024);
    let large = Tag::compound([
        ("text", Tag::String("".into())),
        (
            "extra",
            Tag::List(vec![
                Tag::String(fragment.as_str().into());
                NBT_QUOTA / fragment.len()
            ]),
        ),
    ]);
    let expected = packet(large);
    let mut w = Writer::new();
    P::write(&expected, &mut w);
    assert!(w.data.len() > NBT_QUOTA);
    let mut r = Reader::new(&w.data);
    assert_eq!(P::read(&mut r).unwrap(), expected);
    assert_eq!(r.remaining(), 0);

    // Encode a packet with a unique placeholder, then replace just that NBT
    // field: the normal writer cannot produce a tag past the depth limit.
    let mut marker = Writer::new();
    let placeholder = Tag::String("trusted NBT fixture".into());
    marker.nbt(&placeholder);
    let mut w = Writer::new();
    P::write(&packet(placeholder), &mut w);
    let start = w
        .data
        .windows(marker.data.len())
        .position(|bytes| bytes == marker.data)
        .unwrap();
    // A compound containing 512 unnamed compound children nested in order:
    // root type, 512 (compound type, empty name) entries, 513 End markers.
    let mut deep = vec![10];
    for _ in 0..512 {
        deep.extend([10, 0, 0]);
    }
    deep.extend(std::iter::repeat_n(0, 513));
    w.data.splice(start..start + marker.data.len(), deep);
    let err = P::read(&mut Reader::new(&w.data)).unwrap_err();
    assert!(err.to_string().contains("depth limit exceeded"), "{err}");
}

#[test]
fn chat_uses_trusted_nbt() {
    assert_trusted_limits(|content| SystemChat {
        content,
        overlay: false,
    });
}

#[test]
fn title_subtitle_and_action_bar_use_the_shared_trusted_text_codec() {
    assert_trusted_limits(|text| Text { text });
}

#[test]
fn tab_list_uses_trusted_nbt() {
    assert_trusted_limits(|header| TabList {
        header,
        footer: Tag::String("footer".into()),
    });
}

#[test]
fn teams_and_scores_use_trusted_nbt() {
    assert_trusted_limits(|prefix| SetPlayerTeam {
        name: "red".into(),
        method: 2,
        parameters: Some(TeamParameters {
            display_name: Tag::String("Red".into()),
            prefix,
            suffix: Tag::String("".into()),
            name_tag_visibility: 0,
            collision_rule: 0,
            color: None,
            options: 0,
        }),
        players: vec![],
    });
    assert_trusted_limits(|display| SetScore {
        owner: "player".into(),
        objective: "score".into(),
        score: 1,
        display: Some(display),
        number_format: None,
    });
}

#[test]
fn boss_bar_uses_trusted_nbt() {
    assert_trusted_limits(|name| BossEvent {
        id: 1,
        action: BossAction::UpdateName { name },
    });
}

#[test]
fn block_entity_data_uses_trusted_nbt() {
    assert_trusted_limits(|tag| BlockEntityData {
        pos: [1, 2, 3],
        kind: 0,
        tag,
    });
}

#[test]
fn common_disconnect_uses_trusted_nbt() {
    assert_trusted_limits(|reason| Disconnect { reason });
}

#[test]
fn configuration_and_inline_play_dialogs_use_trusted_nbt() {
    assert_trusted_limits(|dialog| configuration::ShowDialog { dialog });
    assert_trusted_limits(|dialog| ShowDialog {
        dialog: DialogHolder::Direct(dialog),
    });
}

#[test]
fn player_info_uses_trusted_nbt() {
    assert_trusted_limits(|name| PlayerInfoUpdate {
        actions: 1 << 5,
        entries: vec![PlayerInfo {
            uuid: 1,
            display_name: Some(Some(name)),
            ..PlayerInfo::default()
        }],
    });
}

#[test]
fn command_suggestion_tooltips_use_trusted_nbt() {
    assert_trusted_limits(|tooltip| CommandSuggestion {
        text: "help".into(),
        tooltip: Some(tooltip),
    });
}

#[test]
fn server_data_and_open_screen_use_trusted_nbt() {
    assert_trusted_limits(|motd| ServerData { motd, icon: None });
    assert_trusted_limits(|title| OpenScreen {
        container: 1,
        menu: 0,
        title,
    });
}

#[test]
fn advancement_display_uses_trusted_nbt() {
    assert_trusted_limits(|description| UpdateAdvancements {
        reset: false,
        added: vec![Advancement {
            id: "test:advancement".into(),
            parent: None,
            display: Some(AdvancementDisplay {
                title: Tag::String("title".into()),
                description,
                icon: Stack::new(1, 1),
                frame: 0,
                show_toast: false,
                hidden: false,
                background: None,
            }),
            requirements: vec![],
            telemetry: false,
            x: 0.0,
            y: 0.0,
        }],
        removed: vec![],
        progress: vec![],
        show: false,
    });
}

#[test]
fn resource_pack_prompt_uses_trusted_nbt() {
    assert_trusted_limits(|prompt| ResourcePackPush {
        id: 1,
        url: "https://example.invalid/pack.zip".into(),
        hash: "".into(),
        required: false,
        prompt: Some(prompt),
    });
}

#[test]
fn custom_server_link_label_uses_trusted_nbt() {
    assert_trusted_limits(|label| ServerLinks {
        links: vec![configuration::ServerLink {
            kind: configuration::ServerLinkKind::Custom(label),
            link: "https://example.invalid/".into(),
        }],
    });
}
