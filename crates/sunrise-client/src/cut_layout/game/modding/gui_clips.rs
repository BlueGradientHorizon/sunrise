

/// Through the game's generated dispatch: the context a drawing hook gets clips what the mod
/// draws, names its refusals, and closes a clip the mod left open when the call returns.
#[cfg(test)]
mod dispatch_tests {
    use mod_api::{ComponentHoverEvent, GuiClipRefusal, Hooks, ModEntry};
    use mod_platform::testing::with_mods;
    use std::sync::Mutex;
    use sunrise_client::hud::HudBatch;
    use sunrise_game_data::text::{Style, Text};

    /// What `pop_clip` answered past the last open clip, each call.
    static REFUSED: Mutex<Vec<Result<(), GuiClipRefusal>>> = Mutex::new(Vec::new());

    /// Draws a large square inside a closed clip, and another inside one it leaves open.
    struct Clipper;
    #[mod_api::__subscriptions]
    impl Hooks for Clipper {
        fn component_hover(
            &self,
            _: &dyn mod_api::ClientHost,
            event: &mut ComponentHoverEvent<'_>,
        ) -> mod_api::Flow {
            event.draw.push_clip([10.0, 10.0, 5.0, 5.0]).unwrap();
            event.draw.fill([0.0, 0.0, 100.0, 100.0], 0xff00ff00);
            event.draw.pop_clip().unwrap();
            REFUSED.lock().unwrap().push(event.draw.pop_clip());
            event.draw.push_clip([20.0, 20.0, 4.0, 4.0]).unwrap();
            event.draw.fill([0.0, 0.0, 100.0, 100.0], 0xff0000ff);
            mod_api::Flow::Cancel
        }
    }

    #[test]
    #[ignore = "daily"]
    fn a_hook_call_clips_what_its_mod_draws_and_closes_what_it_leaves_open() {
        let text = Text::from(serde_json::json!({
            "text": "x", "hover_event": { "action": "show_text", "value": "hi" }
        }));
        let style: Style = text.spans[0].style.clone();
        let mods: mod_api::Mods = vec![ModEntry::new("clipper", Box::new(Clipper))].into();
        let mut batch = HudBatch::scratch([320, 240]);
        with_mods(mods, || {
            crate::game::server_side::native_entity_refs::tests::with_client_game(|game| {
                game.component_hover_effect(&mut batch, &style, (40.0, 30.0));
            });
        });
        assert_eq!(*REFUSED.lock().unwrap(), [Err(GuiClipRefusal::NoneOpen)]);
        let inside = |color: u32, [x0, y0, x1, y1]: [f32; 4]| {
            let quads: Vec<_> = batch.vertices.iter().filter(|v| v.color == color).collect();
            assert_eq!(quads.len(), 6, "{color:x}");
            assert!(
                quads
                    .iter()
                    .all(|v| (x0..=x1).contains(&v.pos[0]) && (y0..=y1).contains(&v.pos[1]))
            );
        };
        inside(batch.vertices[0].color, [10.0, 10.0, 15.0, 15.0]);
        inside(batch.vertices[6].color, [20.0, 20.0, 24.0, 24.0]);
    }
}