#!/usr/bin/env python3
"""Migrate the Post wrappers, layer callbacks and draw sites to a borrowed model."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CLIENT = "crates/sunrise-client/src/"
GAME = CLIENT + "game/"
VOCAB = "crates/sunrise-mod-vocab/src/"


def rewrite(path, edits):
    path = ROOT / path
    text = path.read_text()
    for old, new, count in edits:
        if text.count(old) != count:
            raise SystemExit(f"{path}: expected {count} instance(s) of {old!r}")
        text = text.replace(old, new)
    path.write_text(text)


def main():
    for area in (
        GAME + "native_hooks/render_living_post.rs",
        GAME + "native_hooks/render_player_post.rs",
        GAME + "modding/native_entity_layers.rs",
        GAME + "modding/native_entity_layers_tests.rs",
        GAME + "modding/native_living_renderers_tests.rs",
        VOCAB + "hooks/register_entity_layers.rs",
        VOCAB + "client/entity_layers.rs",
        "docs/modding/src/how-to/client/entity-layers.md",
    ):
        edits = [("Option<&PosedParts>", "Option<&dyn FrameModel>", 1)]
        if not area.endswith("register_entity_layers.rs") and not area.endswith(".md"):
            if "native_hooks" not in area:
                edits.append(("PosedParts", "FrameModel", 1))
        if area.endswith("native_entity_layers_tests.rs"):
            pass  # Its callback ignores the parameter already.
        elif area.endswith("native_living_renderers_tests.rs"):
            edits += [("parts: Option", "model: Option", 1), ("parts.is_some()", "model.is_some()", 1)]
        else:
            edits.append(("parts: Option", "model: Option", 1))
        if "native_hooks" in area or area.endswith("modding/native_entity_layers.rs"):
            edits.append(("parts, draw)", "model, draw)", 1))
        rewrite(area, edits)
    rewrite(GAME + "native_hooks/mod.rs", [("PosedParts", "FrameModel", 1)])
    rewrite(
        "crates/sunrise-mod-api-gen/dispatch-tests/src/fixtures/base.rs",
        [("struct PosedParts;", "trait FrameModel {}", 1)],
    )
    rewrite(
        GAME + "modding/native_mob_draw.rs",
        [
            ("PosedParts};", "FrameModel};", 1),
            ("parts: Option<PosedParts>", "model: Option<&dyn FrameModel>", 2),
            ("            let parts = parts.as_ref();\n", "", 1),
            ("&layers, parts,", "&layers, model,", 1),
            ("parts.as_ref(), draw", "model, draw", 2),
        ],
    )
    for area in (
        GAME + "mobs/creatures/living_draw.rs",
        GAME + "interface/perspective.rs",
        GAME + "interface/gui_entity.rs",
    ):
        edits = [("capture_parts:", "capture_model:", 1)]
        if area.endswith("gui_entity.rs"):
            edits.append(("frame_draw.parts,", "frame_draw.model(),", 1))
        else:
            edits.append(("let parts = frame_draw.parts;", "let model = frame_draw.model();", 1))
            edits.append(("parts,", "model,", 2) if area.endswith("living_draw.rs") else (
                "                parts,\n", "                model,\n", 1
            ))
            if area.endswith("perspective.rs"):
                edits.append(("Some(who), parts,", "Some(who), model,", 1))
        rewrite(area, edits)
    for area, count in (
        (CLIENT + "mobs/render.rs", 2),
        (CLIENT + "mobs/render_pose_tests.rs", 3),
        (GAME + "modding/native_humanoid_setup_anim_tests.rs", 1),
    ):
        rewrite(area, [("capture_parts:", "capture_model:", count)])


if __name__ == "__main__":
    main()
