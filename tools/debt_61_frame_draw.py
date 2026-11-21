#!/usr/bin/env python3
"""One-shot migration: pass frame draw state and wrap pose map entries in place."""

from pathlib import Path
from textwrap import dedent, indent


SRC = Path(__file__).resolve().parents[1] / "crates/sunrise-client/src"


def main() -> None:
    edits: dict[str, str] = {}

    def replace(path: str, old: str, new: str, count: int = 1) -> None:
        if path not in edits:
            edits[path] = (SRC / path).read_text(encoding="utf-8")
        source = edits[path]
        actual = source.count(old)
        if actual != count:
            raise SystemExit(f"{path}: expected {count} matches, got {actual}: {old!r}")
        edits[path] = source.replace(old, new)

    def lines(path: str, spaces: int, old: str, new: str, count: int = 1) -> None:
        padding = " " * spaces
        replace(
            path,
            "\n" + indent(dedent(old).strip("\n"), padding) + "\n",
            "\n" + indent(dedent(new).strip("\n"), padding) + "\n",
            count,
        )

    render = "mobs/render.rs"
    invisible = "mobs/invisible_body.rs"
    pose_tests = "mobs/render_pose_tests.rs"
    skin = "mobs/skin.rs"
    living = "game/mobs/creatures/living_draw.rs"
    perspective = "game/interface/perspective.rs"
    gui = "game/interface/gui_entity.rs"
    native_tests = "game/modding/native_humanoid_setup_anim_tests.rs"
    anim = "mobs/anim.rs"

    for path, count in (
        (render, 6),
        (invisible, 2),
        (pose_tests, 2),
        (living, 1),
        (perspective, 1),
        (gui, 1),
        (native_tests, 2),
    ):
        replace(path, "render_with_hidden", "render_with_frame", count)

    replace(
        render,
        """/// [`render_with`], with the parts a mod's `render_player_pre` hid left out of the body (after Skin
/// Customization) and the layers on its own model; the armour in `extra` and the other layers keep
/// every part. `parts`, given while the mods' post hooks want them, gets the humanoid parts as the
/// body was drawn ([`PosedParts`](mod_api::PosedParts)); it stays None for a mob that isn't a
/// humanoid or isn't drawn.""",
        """/// Inputs and captured output for one render of a living model.
#[derive(Default)]
pub struct FrameDraw {
    /// Parts to omit from the body and the layers on its own model.
    pub hidden: mod_api::HiddenParts,
    /// Whether the post hooks or mod layers need the posed parts.
    pub capture_parts: bool,
    /// The humanoid pose from the draw, when requested.
    pub parts: Option<mod_api::PosedParts>,
}

/// [`render_with`], applying `frame.hidden` to the body after Skin Customization
/// and to the layers on its own model, but not to armour or the other layers.
/// For a humanoid, `frame.capture_parts` keeps the same pose for post hooks and mod layers.""",
    )
    replace(
        render,
        "/// `hidden` and `parts` as for [`render_with_frame`].",
        "/// `frame` is used as in [`render_with_frame`].",
    )
    replace(
        render,
        """/// (`HumanoidArmorLayer`: the armour in `extra`). `parts` as for [`render_with_frame`], posed as
/// the layers draw over the unseen body.""",
        """/// (`HumanoidArmorLayer`: the armour in `extra`). This ignores `frame.hidden`;
/// if requested, the captured pose is the one under the layers of the unseen body.""",
    )
    lines(
        render,
        4,
        "hidden: mod_api::HiddenParts,\n"
        "mut parts: Option<&mut Option<mod_api::PosedParts>>,",
        "frame: &mut FrameDraw,",
    )
    for path, count in ((render, 2), (invisible, 1)):
        lines(
            path,
            4,
            "hidden: mod_api::HiddenParts,\n"
            "parts: Option<&mut Option<mod_api::PosedParts>>,",
            "frame: &mut FrameDraw,",
            count,
        )
    lines(render, 4, "parts: Option<&mut Option<mod_api::PosedParts>>,", "frame: &mut FrameDraw,")
    lines(render, 8, "Default::default(),\nNone,\nout,", "&mut FrameDraw::default(),\nout,", 2)
    lines(render, 20, "hidden,\nparts.as_deref_mut(),", "frame,")
    lines(render, 8, "hidden,\nparts,", "frame,", 2)
    lines(render, 12, "hidden,\nparts,", "frame,")
    lines(render, 8, "Default::default(),\nparts,", "frame,")
    replace(
        render,
        "let body_poses: &Poses = if hidden.is_empty() {",
        "let body_poses: &Poses = if paint.is_none() || frame.hidden.is_empty() {",
    )
    replace(render, "hide_parts(&mut hidden_poses, &hidden)", "hide_parts(&mut hidden_poses, &frame.hidden)")
    lines(
        render,
        4,
        """
        if let Some(parts) = parts
            && anim::extracts_humanoid(m)
        {
            *parts = posed_parts(body_poses);
        }
        """,
        """
        if frame.capture_parts && anim::extracts_humanoid(m) {
            frame.parts = posed_parts(body_poses);
        }
        """,
    )
    lines(render, 12, "let (mut out, mut parts) = (Vec::new(), None);", """
        let mut out = Vec::new();
        let mut frame = FrameDraw {
            hidden,
            capture_parts: true,
            ..Default::default()
        };
        """)
    lines(render, 16, "hidden,\nSome(&mut parts),", "&mut frame,")
    replace(
        render,
        '(out.len(), parts.expect("a humanoid"))',
        '(out.len(), frame.parts.expect("a humanoid"))',
    )

    replace(invisible, "    Atlas, Mob, Part,", "    Atlas, FrameDraw, Mob, Part,")
    replace(invisible, "extra, hidden, parts, out,", "extra, frame, out,")
    lines(invisible, 8, "hidden,\nparts,", "frame,")

    lines(pose_tests, 8, "let (mut out, mut parts) = (Vec::new(), None);", """
        let mut out = Vec::new();
        let mut frame = FrameDraw {
            capture_parts: keep,
            ..Default::default()
        };
        """)
    lines(pose_tests, 12, "Default::default(),\nkeep.then_some(&mut parts),", "&mut frame,")
    lines(pose_tests, 12, "parts,", "frame.parts,")
    for route in ("skinned", "layered"):
        lines(pose_tests, 4, f"let mut {route} = None;", f"""
            let mut {route} = FrameDraw {{
                capture_parts: true,
                ..Default::default()
            }};
            """)
        replace(pose_tests, f"Some(&mut {route}),", f"&mut {route},")
        replace(pose_tests, f"assert_eq!({route}, Some(parts),", f"assert_eq!({route}.parts, Some(parts),")
    lines(pose_tests, 8, "Default::default(),\n&mut skinned,", "&mut skinned,")
    for spaces, count in ((12, 2), (16, 1)):
        lines(pose_tests, spaces, "hidden,\nNone,", """
            &mut FrameDraw {
                hidden,
                ..Default::default()
            },
            """, count)
    lines(skin, 24, "Default::default(),\nNone,\n&mut gpu,", "&mut render::FrameDraw::default(),\n&mut gpu,")

    replace(living, "use super::*;", "use super::*;\nuse sunrise_client::mobs::render::FrameDraw;")
    lines(living, 8, "let (mut skinned, mut parts, added) = (None, None, self.mod_layers(mob));", """
        let (mut skinned, added) = (None, self.mod_layers(mob));
        let mut frame_draw = FrameDraw {
            hidden,
            capture_parts: listened || added.is_some(),
            ..Default::default()
        };
        """)
    capture_living = "(listened || added.is_some()).then_some(&mut parts),"
    lines(living, 16, "hidden,\n" + capture_living, "&mut frame_draw,")
    lines(living, 20, "hidden,\n" + capture_living, "&mut frame_draw,", 2)
    lines(living, 20, capture_living, "&mut frame_draw,")
    replace(living, "        // HDR output:", "        let parts = frame_draw.parts;\n        // HDR output:")

    replace(
        perspective,
        "mobs::{Anim, Brain, Mob, MobType},",
        "mobs::{Anim, Brain, Mob, MobType, render::FrameDraw},",
    )
    lines(perspective, 8, "let (mut parts, layers) = (None, self.mod_layers(m));", """
        let layers = self.mod_layers(m);
        let mut frame_draw = FrameDraw {
            hidden,
            capture_parts: listened || layers.is_some(),
            ..Default::default()
        };
        """)
    capture_player = "(listened || layers.is_some()).then_some(&mut parts),"
    lines(perspective, 16, "hidden,\n" + capture_player, "&mut frame_draw,")
    lines(perspective, 16, capture_player, "&mut frame_draw,")
    replace(
        perspective,
        "        // AvatarRenderer.shouldRenderLayers: a spectator draws no layers.",
        "        let parts = frame_draw.parts;\n"
        "        // AvatarRenderer.shouldRenderLayers: a spectator draws no layers.",
    )
    replace(perspective, "&armour, None, &mut verts,", "&armour, &mut FrameDraw::default(), &mut verts,")
    replace(perspective, "&armour, None, &mut out,", "&armour, &mut FrameDraw::default(), &mut out,")
    lines(perspective, 16, "armour,\nNone,\nout,", "armour,\n&mut FrameDraw::default(),\nout,")

    replace(gui, "use sunrise_client::{hud::HudBatch, mesher::Vertex, mobs::Mob};", """use sunrise_client::{
    hud::HudBatch,
    mesher::Vertex,
    mobs::{Mob, render::FrameDraw},
};""")
    lines(gui, 8, "let mut parts = None;", """
        let mut frame_draw = FrameDraw {
            capture_parts: added.is_some(),
            ..Default::default()
        };
        """)
    lines(gui, 16, "Default::default(),\nadded.is_some().then_some(&mut parts),", "&mut frame_draw,")
    replace(
        gui,
        "self.mod_layer_geometry(mob, None, parts, added,",
        "self.mod_layer_geometry(mob, None, frame_draw.parts, added,",
    )

    lines(native_tests, 20, "let mut parts = None;", """
        let mut draw = render::FrameDraw {
            capture_parts: true,
            ..Default::default()
        };
        """)
    lines(native_tests, 28, "Default::default(),\nSome(&mut parts),", "&mut draw,", 2)
    lines(native_tests, 28, "Some(&mut parts),", "&mut draw,")
    replace(native_tests, "if let Some(parts) = parts {", "if let Some(parts) = draw.parts {")
    lines(
        native_tests,
        32,
        "Default::default(),\nNone,\n&mut baseline,",
        "&mut render::FrameDraw::default(),\n&mut baseline,",
    )

    replace(anim, "/// Part poses for one draw, keyed by part name.", """#[derive(Clone, Copy)]
struct PoseEntry {
    pose: [f32; 9],
}

/// Part poses for one draw, keyed by part name.""")
    replace(anim, "FxHashMap<u64, [f32; 9]>", "FxHashMap<u64, PoseEntry>", 2)
    replace(
        anim,
        "map.entry(key(&p.name)).or_insert(p.pose);",
        "map.entry(key(&p.name)).or_insert(PoseEntry { pose: p.pose });",
    )
    lines(
        anim,
        12,
        "Some(p) => p,\nNone => &mut self.dummy,",
        "Some(entry) => &mut entry.pose,\nNone => &mut self.dummy,",
    )
    replace(anim, "self.map.get(&key(name)).copied()", "self.map.get(&key(name)).map(|entry| entry.pose)")
    replace(
        anim,
        "self.map.get(&key(&part.name)).copied().unwrap_or(part.pose)",
        "self.pose_of(&part.name).unwrap_or(part.pose)",
    )
    replace(anim, ".map(|n| (key(n), [0.0; 9]))", ".map(|n| (key(n), PoseEntry { pose: [0.0; 9] }))", 10)

    for path, updated in edits.items():
        (SRC / path).write_text(updated, encoding="utf-8")
    print(f"Updated {len(edits)} Rust files")


if __name__ == "__main__":
    main()
