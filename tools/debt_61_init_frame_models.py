#!/usr/bin/env python3
"""Add the frame-layout table to MobData and initialize every existing literal."""

from pathlib import Path
import re


SOURCE = Path(__file__).resolve().parents[1] / "crates/sunrise-client/src"
INITIALIZERS = {
    "game/interface/occlusion_culling/occlusion_culling_tests.rs": 1,
    "game/interface/perspective.rs": 1,
    "mobs/cem.rs": 1,
    "mobs/data.rs": 2,
    "mobs/random_textures.rs": 1,
    "mobs/render.rs": 8,
    "mobs/skin.rs": 1,
}


def main():
    updates = {}
    for relative, expected in INITIALIZERS.items():
        path = SOURCE / relative
        text, count = re.subn(
            r"^([ ]+)cem: (?:Default::default\(\)|std::sync::RwLock::new\(models\.clone\(\)\)),",
            r"\1frame_models: Default::default(),\n\g<0>",
            path.read_text(),
            flags=re.MULTILINE,
        )
        if count != expected or text.count("frame_models: Default::default(),") != expected:
            raise SystemExit(f"{relative}: expected {expected} unmigrated MobData initializers")
        updates[path] = text
    data = SOURCE / "mobs/data.rs"
    anchor = "    /// Replaced when the resource packs change (the rest of `MobData` never is)."
    if updates[data].count(anchor) != 1 or "pub frame_models:" in updates[data]:
        raise SystemExit("mobs/data.rs: expected unmigrated MobData definition")
    updates[data] = updates[data].replace(
        anchor,
        "    /// Baked vanilla frame layouts, empty for resources that do not draw.\n"
        "    pub frame_models: super::render::BakedFrameModels,\n" + anchor,
    )
    for path, text in updates.items():
        path.write_text(text)


if __name__ == "__main__":
    main()
