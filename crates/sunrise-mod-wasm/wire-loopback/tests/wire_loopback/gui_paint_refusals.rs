use super::*;
use mod_api::client::hud_layout::GuiDraw;
use mod_api::text::TextFormError;
use mod_api::{DrawContext, GuiPaintError};

fn refusals() -> Vec<GuiPaintError> {
    vec![
        GuiPaintError::Text(TextFormError::TooDeep),
        GuiPaintError::ItemSlot { index: 3, slot: 41 },
        GuiPaintError::ItemNotFinite { index: 0 },
        GuiPaintError::BlitTexture {
            index: 7,
            texture: "test:gui/panel".into(),
        },
        GuiPaintError::BlitSize { index: u32::MAX },
        GuiPaintError::Sprite {
            index: 2,
            sprite: "test:mob_effect/missing".into(),
        },
        GuiPaintError::SpriteRegion { index: 4 },
        GuiPaintError::SpriteBlend {
            index: 5,
            blend: mod_api::client::hud_layout::Blend::Vignette,
        },
    ]
}

#[test]
fn gui_paint_refusals_round_trip_on_the_wire() {
    for refusal in refusals() {
        let mut out = Writer::new();
        out.put(&refusal);
        assert_eq!(
            Reader::decode::<GuiPaintError>(out.bytes()).unwrap(),
            refusal
        );
    }
}

/// The game's side: refuses each buffer in turn with the next refusal, recording the buffers.
#[derive(Default)]
struct Draw {
    refusals: Vec<GuiPaintError>,
    painted: Vec<Vec<GuiDraw>>,
}
impl DrawContext for Draw {
    fn gui_paint(&mut self, draws: &[GuiDraw]) -> Result<(), GuiPaintError> {
        self.painted.push(draws.to_vec());
        if self.refusals.is_empty() {
            Ok(())
        } else {
            Err(self.refusals.remove(0))
        }
    }
}

#[test]
fn wire_loopback_a_guests_paint_gets_the_games_named_refusal_back() {
    mod_api::__private::initialize().unwrap();
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let mut draw = Draw {
        refusals: refusals(),
        ..Draw::default()
    };
    let mut cx = Box::new(boundary::Cx::default());
    let mut frame = boundary::Frame::default();
    frame.set_draw_context_mut(&mut draw);
    cx.frames.push(frame);
    let pointer = &mut *cx as *mut boundary::Cx;
    guest::set_transport(Some(Rc::new(move |id, bytes| {
        let mut out = Writer::new();
        // SAFETY: the context and its frame outlive this synchronous transport.
        unsafe { boundary::serve(id, pointer, bytes, &mut out) }.unwrap();
        out.into_bytes()
    })));
    let item = GuiDraw::Item {
        slot: 41,
        x: 20,
        y: 20,
        seed: 0,
        scale: [2.0, 2.0],
        pivot: [28.0, 28.0],
    };
    let answers: Vec<_> = (0..9)
        .map(|_| guest::Guest.gui_paint(&[item.clone()]))
        .collect();
    guest::set_transport(None);
    drop(cx);
    let mut want: Vec<_> = refusals().into_iter().map(Err).collect();
    want.push(Ok(()));
    assert_eq!(answers, want);
    assert_eq!(draw.painted, vec![vec![item]; 9]);
}
