#!/usr/bin/env python3
"""Make the pre-scope particle constructor callers explicitly context-free.

Run once on the source before P3-16 step 2, then rustfmt the touched files.
"""

import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1] / "crates/sunrise-client-render/src"
SCOPE = "SpawnScope::BARE"


def replace_once(source, before, after):
    if source.count(before) != 1:
        raise SystemExit(f"Expected exactly one anchor: {before!r}")
    return source.replace(before, after)


updates = {}
for relative_path, expected in (
    ("particles.rs", 9),
    ("particles/groups.rs", 2),
    ("particles/native/tests.rs", 3),
    ("particles/spawn.rs", 1),
):
    path = ROOT / relative_path
    source = path.read_text(encoding="utf-8")
    source, single = re.subn(r"(\.make\([^\n]*?, (?:1|seed))\)", rf"\1, {SCOPE})", source)
    source, multi = re.subn(
        r"(\.make\(\n(?:[^\n]*,\n){3})( +)([1357]|seed),?\n(?= +\))",
        rf"\1\2\3,\n\2{SCOPE},\n",
        source,
    )
    if single + multi != expected:
        raise SystemExit(f"{relative_path}: expected {expected} constructor calls, got {single + multi}")
    updates[path] = source

engine = ROOT / "particles.rs"
scope_anchor = "/// One `particlesToAdd` FIFO across render groups: admission shares the engine RNG."
updates[engine] = replace_once(
    updates[engine],
    scope_anchor,
    """/// Factory inputs borrowed only for synchronous construction, not kept in the queue.
struct SpawnScope<'a, 'l> {
    level: Option<&'a ParticleLevel<'l>>,
    mod_options: Option<&'a mod_api::ParticleOptions>,
}

impl SpawnScope<'_, '_> {
    const BARE: Self = Self {
        level: None,
        mod_options: None,
    };
}

""" + scope_anchor,
)
updates[engine] = replace_once(
    updates[engine],
    "        seed: i64,\n    ) -> Option<Particle> {\n        use ParticleType as T;",
    "        seed: i64,\n        scope: SpawnScope<'_, '_>,\n    ) -> Option<Particle> {\n"
    "        use ParticleType as T;",
)
updates[engine] = replace_once(
    updates[engine],
    "            return self.make_named(name.as_ref(), p, v, seed);",
    """            // The batch provider only reads the level during its tick.
            let SpawnScope { level: _level, mod_options: _mod_options } = scope;
            return self.make_named(name.as_ref(), p, v, seed);""",
)

for path, source in updates.items():
    path.write_text(source, encoding="utf-8")
