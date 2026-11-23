use super::*;
use crate::game::{in_own_process, native_entity_refs::tests::with_client_game};

#[test]
#[ignore = "daily"]
fn tutorial_toast_bolds_only_the_rebound_key() {
    in_own_process(
        "game::tutorial::toast_tests::tutorial_toast_bolds_only_the_rebound_key",
        || {
            with_client_game(|game| {
                game.settings.keys.set("key.inventory", "key.keyboard.z");
                game.settings
                    .keys
                    .set("key.socialInteractions", "key.keyboard.x");
                for (hint, key) in [(Hint::OpenInventory, 'Z'), (Hint::SocialInteractions, 'X')] {
                    let toast = game.tutorial_toast(hint, None).unwrap();
                    let spans = toast.lines[toast.title_lines..]
                        .iter()
                        .flat_map(|(line, _)| &line.spans);
                    let mut highlighted = String::new();
                    let mut surrounding = String::new();
                    for span in spans {
                        if span.style.bold == Some(true) {
                            assert_ne!(span.style.italic, Some(true));
                            assert_eq!(span.style.color, None);
                            highlighted.push_str(&span.text);
                        } else {
                            surrounding.push_str(&span.text);
                        }
                    }
                    assert_eq!(highlighted, key.to_string());
                    assert!(surrounding.chars().any(char::is_alphabetic));
                }
            });
        },
    );
}
