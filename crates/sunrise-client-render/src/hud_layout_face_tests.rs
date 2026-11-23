//! `GuiDraw::PlayerFace` as [`paint_checked`] draws it: `PlayerFaceExtractor.extractRenderState`
//! from a synthetic skin, a 64 x 64 atlas slot whose face (8, 8) and hat (40, 8) areas are told
//! apart by their UVs. The face comes first, then the hat when asked for, each `size` square under
//! the op's pose and tint; it is upside down only when `flip` is set.
use super::{GuiDraw, GuiFaces, GuiItems, PoseOp, abgr, paint_checked};
use crate::hud::{HudAtlas, HudBatch, Rect};

/// The synthetic skin: a run-time skin slot every atlas has, 64 x 64 as a skin is.
const SKIN: &str = "sunrise/skin0";
const PLAYER: u128 = 0x853c_80ef_3c37_49fd_aa49_938b_674a_dae6;

fn atlas() -> HudAtlas {
    HudAtlas::build(&crate::resource_source::ResourceSource::directory(
        "/nonexistent/sunrise-test-assets".into(),
    ))
    .unwrap()
}

/// Gives [`SKIN`] for every UUID, recording which were asked for.
#[derive(Default)]
struct Faces {
    asked: Vec<u128>,
}
impl GuiFaces for Faces {
    fn skin(&mut self, uuid: u128) -> String {
        self.asked.push(uuid);
        SKIN.to_owned()
    }
}

struct NoItems;
impl GuiItems for NoItems {
    fn item(&mut self, _: &mut HudBatch, _: u32, _: i32, _: i32, _: i32) {
        unreachable!("no item in these buffers")
    }
    fn decorations(&mut self, _: &mut HudBatch, _: u32, _: i32, _: i32) {
        unreachable!("no item in these buffers")
    }
}

fn face(x: i32, y: i32, size: i32, hat: bool, flip: bool, color: u32) -> GuiDraw {
    GuiDraw::PlayerFace {
        uuid: PLAYER,
        x,
        y,
        size,
        hat,
        flip,
        color,
    }
}

/// One drawn quad: its position bounds, the UV at its top-left and bottom-right corners, and
/// its vertex colour.
#[derive(Debug)]
struct Quad {
    pos: [f32; 4],
    top_left: [f32; 2],
    bottom_right: [f32; 2],
    color: u32,
}

fn quads(batch: &HudBatch) -> Vec<Quad> {
    batch
        .vertices
        .chunks(6)
        .map(|q| {
            let min = |i: usize| q.iter().map(|v| v.pos[i]).fold(f32::INFINITY, f32::min);
            let max = |i: usize| q.iter().map(|v| v.pos[i]).fold(f32::NEG_INFINITY, f32::max);
            let pos = [min(0), min(1), max(0), max(1)];
            let at = |x: f32, y: f32| q.iter().find(|v| v.pos == [x, y]).unwrap().uv;
            assert!(q.iter().all(|v| v.color == q[0].color));
            Quad {
                pos,
                top_left: at(pos[0], pos[1]),
                bottom_right: at(pos[2], pos[3]),
                color: q[0].color,
            }
        })
        .collect()
}

/// The atlas UV of skin pixel `(u, v)`.
fn skin_uv(r: &Rect, u: f32, v: f32) -> [f32; 2] {
    [
        r.u0 + (r.u1 - r.u0) * (u / 64.0),
        r.v0 + (r.v1 - r.v0) * (v / 64.0),
    ]
}

fn close(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6
}

fn painted(atlas: &HudAtlas, draws: &[GuiDraw]) -> (Vec<Quad>, Vec<u128>) {
    let mut batch = HudBatch::scratch([854, 480]);
    let mut faces = Faces::default();
    assert_eq!(
        paint_checked(&mut batch, atlas, draws, &mut NoItems, &mut faces),
        Ok(())
    );
    (quads(&batch), faces.asked)
}

#[test]
fn a_face_draws_the_face_then_the_hat_at_its_size_tint_and_pose() {
    let atlas = atlas();
    let r = *atlas.blit_rect(SKIN).unwrap();
    let color = 0x80FF_8040;
    let draws = [
        GuiDraw::Push(vec![PoseOp::Translate(100.0, 10.0)]),
        face(2, 3, 16, true, false, color),
        GuiDraw::Pop,
    ];
    let (quads, asked) = painted(&atlas, &draws);
    assert_eq!(asked, [PLAYER]);
    assert_eq!(quads.len(), 2, "{quads:?}");
    for (quad, u) in quads.iter().zip([8.0, 40.0]) {
        assert_eq!(quad.pos, [102.0, 13.0, 118.0, 29.0]);
        assert_eq!(quad.color, abgr(color));
        assert!(close(quad.top_left, skin_uv(&r, u, 8.0)), "{quad:?}");
        assert!(
            close(quad.bottom_right, skin_uv(&r, u + 8.0, 16.0)),
            "{quad:?}"
        );
    }
}

#[test]
fn without_the_hat_only_the_face_is_drawn_and_flip_turns_it_upside_down() {
    let atlas = atlas();
    let r = *atlas.blit_rect(SKIN).unwrap();
    let (quads, _) = painted(&atlas, &[face(0, 0, 8, false, false, 0xFFFF_FFFF)]);
    assert_eq!(quads.len(), 1);
    assert_eq!(quads[0].pos, [0.0, 0.0, 8.0, 8.0]);
    assert!(close(quads[0].top_left, skin_uv(&r, 8.0, 8.0)));
    assert!(close(quads[0].bottom_right, skin_uv(&r, 16.0, 16.0)));

    // Flipped, the face's bottom row is drawn at the top: the same pixels, rows reversed.
    let (quads, _) = painted(&atlas, &[face(0, 0, 8, true, true, 0xFFFF_FFFF)]);
    assert_eq!(quads.len(), 2);
    for (quad, u) in quads.iter().zip([8.0, 40.0]) {
        assert_eq!(quad.pos, [0.0, 0.0, 8.0, 8.0]);
        assert!(close(quad.top_left, skin_uv(&r, u, 16.0)), "{quad:?}");
        assert!(
            close(quad.bottom_right, skin_uv(&r, u + 8.0, 8.0)),
            "{quad:?}"
        );
    }
}
