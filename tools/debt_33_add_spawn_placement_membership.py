#!/usr/bin/env python3
"""Add the vanilla spawn-placement membership field to MobData and its literals."""

from pathlib import Path
import re


if __name__ == "__main__":
    src = Path(__file__).resolve().parents[1] / "crates/sunrise-client/src"
    counts = {
        "mobs/data.rs": 2,
        "mobs/render.rs": 8,
        "mobs/cem.rs": 1,
        "mobs/random_textures.rs": 1,
        "mobs/skin.rs": 1,
        "game/interface/perspective.rs": 1,
        "game/interface/occlusion_culling/occlusion_culling_tests.rs": 1,
    }
    declaration = "    pub mod_spawns: HashMap<String, mod_api::SpawnPlacementRule>,\n"
    membership = (
        "    /// Entity type IDs with a vanilla spawn placement, before mod registration.\n"
        "    /// `None` means the original membership has not been captured.\n"
        "    pub vanilla_spawn_placements: Option<std::collections::HashSet<String>>,\n"
    )
    updates = {}
    for relative, expected in counts.items():
        path = src / relative
        original = path.read_text(encoding="utf-8")
        if "vanilla_spawn_placements" in original:
            raise SystemExit(f"{relative}: spawn-placement membership already present")
        updated, found = re.subn(
            r"^([ \t]*)mod_spawns: Default::default\(\),$",
            r"\g<0>\n\1vanilla_spawn_placements: None,",
            original,
            flags=re.MULTILINE,
        )
        if found != expected:
            raise SystemExit(f"{relative}: expected {expected} MobData literals, found {found}")
        if relative == "mobs/data.rs":
            if updated.count(declaration) != 1:
                raise SystemExit(f"{relative}: expected exactly one mod_spawns field declaration")
            updated = updated.replace(declaration, declaration + membership)
        updates[path] = updated

    for path, content in updates.items():
        path.write_text(content, encoding="utf-8")
