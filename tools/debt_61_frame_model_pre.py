#!/usr/bin/env python3
"""Migrate debt 61 step 6's Pre signatures and unchanged FrameDraw constructors."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CLIENT = "crates/sunrise-client/src"
GEN = "crates/sunrise-mod-api-gen/dispatch-tests/src"


def replace(file, old, new, count=1):
    path = ROOT / file
    source = path.read_text()
    assert source.count(old) == count, (file, old, source.count(old))
    path.write_text(source.replace(old, new))


for kind in ("living", "player"):
    replace(
        f"{CLIENT}/game/native_hooks/render_{kind}_pre.rs",
        "hidden: &mut HiddenParts,",
        "model: &mut dyn FrameModelEdit,",
    )
    replace(
        f"{CLIENT}/game/native_hooks/render_{kind}_pre.rs",
        "draw, hidden, canceled);",
        "draw, model, canceled);",
    )
replace(f"{CLIENT}/game/native_hooks/mod.rs", "FrameModel, GuiLayer, HiddenParts,", "FrameModel, FrameModelEdit, GuiLayer,")
replace(f"{GEN}/fixtures/base.rs", "pub(super) struct HiddenParts;", "pub(super) trait FrameModelEdit: FrameModel {}")
replace(
    f"{CLIENT}/game/interface/gui_entity.rs",
    "FrameDraw {\n            capture_model: !modded && added.is_some(),\n            ..Default::default()\n        }",
    "FrameDraw::new(!modded && added.is_some())",
)
replace(
    f"{CLIENT}/game/modding/native_humanoid_setup_anim_tests.rs",
    "render::FrameDraw {\n                        capture_model: true,\n                        ..Default::default()\n                    }",
    "render::FrameDraw::new(true)",
)
replace(
    f"{CLIENT}/mobs/render_pose_tests.rs",
    "FrameDraw {\n            capture_model,\n            ..Default::default()\n        }",
    "FrameDraw::new(capture_model)",
    count=2,
)
for file, count in (("docs/modding/src/how-to/client/model-drawing.md", 2), ("crates/sunrise-mod-vocab/src/hooks/notes.rs", 4)):
    replace(file, "draw, hidden, canceled)", "draw, model, canceled)", count)
