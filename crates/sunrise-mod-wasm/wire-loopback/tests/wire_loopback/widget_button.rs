use super::*;
use mod_api::{ButtonSprites, DrawContext, Text, text::TextFormError};

#[test]
fn widget_button_carries_optional_art_from_wasm_to_the_host() {
    #[derive(Default)]
    struct Draw(Vec<Option<ButtonSprites>>);
    impl DrawContext for Draw {
        fn widget_button(
            &mut self,
            rect: [f32; 4],
            label: &Text,
            active: bool,
            highlighted: bool,
            sprites: Option<&ButtonSprites>,
        ) -> Result<(), TextFormError> {
            assert_eq!(rect, [2.0, 3.0, 14.0, 15.0]);
            assert_eq!(*label, Text::literal("Run"));
            assert!(!active);
            assert!(highlighted);
            self.0.push(sprites.cloned());
            Ok(())
        }
    }

    mod_api::__private::initialize().unwrap();
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let mut draw = Draw::default();
    let mut cx = Box::new(boundary::Cx::default());
    let mut frame = boundary::Frame::default();
    frame.set_draw_context_mut(&mut draw);
    cx.frames.push(frame);
    let pointer = &mut *cx as *mut boundary::Cx;
    guest::set_transport(Some(Rc::new(move |id, bytes| {
        let mut out = Writer::new();
        // SAFETY: the context and its frame's draw remain live for this synchronous transport.
        unsafe { boundary::serve(id, pointer, bytes, &mut out) }.unwrap();
        out.into_bytes()
    })));
    let mut art = ButtonSprites::new("test:normal");
    art.highlighted = Some("test:highlighted".into());
    art.disabled = Some("test:disabled".into());
    art.label_offset = [-2.0, 1.5];
    for sprites in [None, Some(&art)] {
        guest::Guest
            .widget_button(
                [2.0, 3.0, 14.0, 15.0],
                &Text::literal("Run"),
                false,
                true,
                sprites,
            )
            .unwrap();
    }
    guest::set_transport(None);
    drop(cx);
    assert_eq!(draw.0, [None, Some(art)]);
}
