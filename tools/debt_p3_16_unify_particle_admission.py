#!/usr/bin/env python3
"""Reapply the P3-16 review migration to the single replaceable doAddParticle.

Run once against the last P3-16 author commit and rustfmt the affected Rust files.
"""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "crates"
RENDER = "sunrise-client-render/src/particles"
CLIENT = "sunrise-client/src/"


def replace(text, old, new, expected=1):
    if text.count(old) != expected:
        raise SystemExit(f"Expected {expected} matches for {old!r}")
    return text.replace(old, new)


edits = {}
for path, scoped_count, with_level_count, imports in (
    (f"{RENDER}.rs", 3, 0, None),
    (f"{RENDER}/spawn.rs", 0, 0, None),
    (f"{RENDER}/explosions.rs", 1, 1, None),
    (f"{RENDER}/native/tests.rs", 0, 4, None),
    (f"{RENDER}/polytone.rs", 0, 2, None),
    (
        "sunrise-client-gameplay/src/remote/projectiles.rs", 0, 1,
        ("ParticleSpawnFlags, ParticleType,", "ParticleSpawnFlags, ParticleType, SpawnScope,"),
    ),
    (
        f"{CLIENT}game/modding/native_client_particles.rs", 0, 1,
        ("{ParticleEngine, ParticleLevel, ParticleSpawnFlags}",
         "{ParticleEngine, ParticleLevel, ParticleSpawnFlags, SpawnScope}"),
    ),
    (
        f"{CLIENT}game/modding/native_scene_live.rs", 0, 1,
        ("ParticleOptions, ParticleSpawnFlags}", "ParticleOptions, ParticleSpawnFlags, SpawnScope}"),
    ),
    (
        f"{CLIENT}game/network/lan_particles.rs", 0, 2,
        ("    ParticleType,", "    ParticleType, SpawnScope,"),
    ),
):
    source = (ROOT / path).read_text()
    if path == f"{RENDER}/spawn.rs":
        start = source.index("    /// Admit a level spawn")
        admission = source[source.index("    pub(super) fn add_scoped", start):source.index("    /// `ParticleEngine.createParticle`", start)]
        source = source[:start] + source[source.index("    /// `ParticleEngine.createParticle`", start):]
        source = replace(source, "    fn create_scoped(", "    pub(super) fn create_scoped(")
    if scoped_count:
        source = replace(source, ".add_scoped(", ".add_with(", scoped_count)
    if with_level_count:
        source = replace(source, ".add_with_level(", ".add_with(", with_level_count)
        source, n = re.subn(
            r"(?m)^( +)(&?level),\n\1(None|[^\n]+\)),\n(?= +\);)",
            r"\1SpawnScope::with_level(\2, \3),\n",
            source,
        )
        if path.endswith("lan_particles.rs"):
            source = replace(source, "at, v, flags, &level, None)", "at, v, flags, SpawnScope::with_level(&level, None))")
            n += 1
        if n != with_level_count:
            raise SystemExit(f"{path}: expected {with_level_count} level argument pairs, found {n}")
    if imports:
        source = replace(source, *imports)
    edits[path] = source

engine = edits[f"{RENDER}.rs"]
engine = replace(engine, "struct SpawnScope<'a, 'l> {\n    level: Option<&'a ParticleLevel<'l>>,\n    mod_options: Option<&'a mod_api::ParticleOptions>,", """pub struct SpawnScope<'a, 'l> {
    /// The live level, absent for built-in spawns that do not lend it.
    pub level: Option<&'a ParticleLevel<'l>>,
    /// Original typed options if the caller already has them.
    pub mod_options: Option<&'a mod_api::ParticleOptions>,""")
engine = replace(engine, "impl SpawnScope<'_, '_> {\n    const BARE: Self = Self {\n        level: None,\n        mod_options: None,\n    };\n}", """impl<'a, 'l> SpawnScope<'a, 'l> {
    /// A built-in spawn that borrows neither the live level nor typed mod options.
    pub const BARE: Self = Self {
        level: None,
        mod_options: None,
    };

    /// Lend the live level for provider construction. If no typed options are supplied,
    /// construction converts the engine request's options.
    pub fn with_level(
        level: &'a ParticleLevel<'l>,
        mod_options: Option<&'a mod_api::ParticleOptions>,
    ) -> Self {
        Self { level: Some(level), mod_options }
    }
}""")
engine = replace(engine, "self.add_with(options, p, v, false, false);", "self.add_with(options, p, v, ParticleSpawnFlags::default(), SpawnScope::BARE);")
start = engine.index("    /// `ClientLevel.addParticle(particle, overrideLimiter, alwaysShow, ...)`")
end = engine.index("    /// `NoxiousGasCloudParticle.spawnNoxiousGasParticle`", start)
engine = engine[:start] + """    /// `ClientLevel.doAddParticle`: apply level admission and construct synchronously,
    /// lending the live level and original mod options when the caller has them.
    #[sunrise_mod_api_macros::replaceable]
""" + replace(admission, "pub(super) fn add_scoped", "pub fn add_with").rstrip() + "\n" + engine[end:]
engine = replace(engine, "            false,\n            true,\n        );\n    }\n    /// `ClientLevel.calculateParticleLevel`", """            ParticleSpawnFlags { always_show: true, ..Default::default() },
            SpawnScope::BARE,
        );
    }
    /// `ClientLevel.calculateParticleLevel`""")
engine = replace(engine, "engine.add_with(&options, [0.5, 1.5, 0.5], [0.0; 3], false, false);", "engine.add(&options, [0.5, 1.5, 0.5], [0.0; 3]);", 2)
edits[f"{RENDER}.rs"] = engine

for path, before, after, old_import, new_import in (
    (
        f"{CLIENT}animate_tick.rs",
        "engine.add_with(&ParticleOptions::simple(ty), p, v, false, true);",
        "engine.add_with(&ParticleOptions::simple(ty), p, v, ParticleSpawnFlags { always_show: true, ..Default::default() }, SpawnScope::BARE);",
        "ParticleOptions, ParticleType as T, along_axis",
        "ParticleOptions, ParticleSpawnFlags, ParticleType as T, SpawnScope, along_axis",
    ),
    (f"{CLIENT}game.rs", None, None, "    particles::Particle,", "    particles::{Particle, ParticleSpawnFlags, SpawnScope},"),
    (f"{CLIENT}game/mobs/dragon.rs", None, None, "particles::{ParticleOptions, ParticleType}", "particles::{ParticleOptions, ParticleSpawnFlags, ParticleType, SpawnScope}"),
):
    source = replace((ROOT / path).read_text(), old_import, new_import)
    if before:
        source = replace(source, before, after)
    source, n = re.subn(
        r"(?m)(\.add_with\(\n[\s\S]*?\n)( +)(true|false),\n\2true,\n(?= +\);)",
        r"\1\2ParticleSpawnFlags { override_limiter: \3, always_show: true },\n\2SpawnScope::BARE,\n",
        source,
    )
    if n != 1:
        raise SystemExit(f"{path}: expected one multiline call, found {n}")
    edits[path] = source

for path, source in edits.items():
    (ROOT / path).write_text(source)
