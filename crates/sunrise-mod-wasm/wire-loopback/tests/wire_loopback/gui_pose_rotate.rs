use super::*;
use mod_api::DrawContext;
use mod_api::client::hud_layout::{GuiDraw, PoseOp};

/// The game's side: records each painted buffer it is asked for.
#[derive(Default)]
struct Draw {
    painted: Vec<Vec<GuiDraw>>,
}
impl DrawContext for Draw {
    fn gui_paint(&mut self, draws: &[GuiDraw]) -> Result<(), mod_api::GuiPaintError> {
        self.painted.push(draws.to_vec());
        Ok(())
    }
}

fn turned(angle: f32) -> Vec<GuiDraw> {
    vec![
        GuiDraw::Push(vec![
            PoseOp::Translate(8.0, 12.0),
            PoseOp::Rotate(angle),
            PoseOp::Scale(0.5, 2.0),
        ]),
        GuiDraw::Push(vec![PoseOp::Rotate(-angle)]),
        GuiDraw::Pop,
        GuiDraw::Pop,
    ]
}

#[test]
fn a_rotate_pose_op_round_trips_on_the_wire_with_its_angle_bits() {
    for angle in [std::f32::consts::FRAC_PI_2, -0.0, 1e-30, f32::INFINITY] {
        let draws = turned(angle);
        let mut out = Writer::new();
        out.put(&draws);
        assert_eq!(Reader::decode::<Vec<GuiDraw>>(out.bytes()).unwrap(), draws);
    }
    // A NaN angle crosses unchanged too: the game, not the wire, refuses it.
    let mut out = Writer::new();
    out.put(&PoseOp::Rotate(f32::NAN));
    let PoseOp::Rotate(back) = Reader::decode::<PoseOp>(out.bytes()).unwrap() else {
        panic!("not a rotation");
    };
    assert_eq!(back.to_bits(), f32::NAN.to_bits());
}

#[test]
fn wire_loopback_a_guests_rotated_paint_reaches_the_draw_context() {
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
        // SAFETY: the context and its frame outlive this synchronous transport.
        unsafe { boundary::serve(id, pointer, bytes, &mut out) }.unwrap();
        out.into_bytes()
    })));
    assert_eq!(guest::Guest.gui_paint(&turned(0.75)), Ok(()));
    guest::set_transport(None);
    drop(cx);
    assert_eq!(draw.painted, [turned(0.75)]);
}
