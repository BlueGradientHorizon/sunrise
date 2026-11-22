#!/usr/bin/env python3
"""Reapply the P3-16 direct-spawn signature migration to the R3 review base."""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GAME = "crates/sunrise-client/src/game"
RENDER = "crates/sunrise-client-render/src/particles"


def add_scope(source, receiver_pattern, method, scope, expected):
    """Append an argument to the explicitly selected calls at this base."""
    calls = list(re.finditer(rf"{receiver_pattern}\s*\.{method}\(", source))
    if len(calls) != expected:
        raise ValueError(f"Expected {expected} {receiver_pattern}.{method} calls, got {len(calls)}")
    for call in reversed(calls):
        depth = 1
        end = call.end()
        # These call arguments contain no unmatched parentheses in strings or comments.
        while depth:
            depth += (source[end] == "(") - (source[end] == ")")
            end += 1
        args = source[call.end() : end - 1].rstrip()
        args = args.removesuffix(",").rstrip()
        if args.startswith("\n"):
            indent = re.match(r"\n( +)", args).group(1)
            args += f",\n{indent}{scope},\n{indent[4:]}"
        else:
            args += f", {scope}"
        source = source[:call.end()] + args + source[end - 1:]
    return source


def replace(source, old, new, expected=1):
    if source.count(old) != expected:
        raise ValueError(f"Expected {expected} occurrences of {old!r}")
    return source.replace(old, new)


edits = {}
for path, method, count in (
    ("blocks/decorated_pot.rs", "send_particles", 1),
    ("entities/armor_stands.rs", "send_particles", 1),
    ("entities/cushions.rs", "send_particles", 1),
    ("entities/melee.rs", "send_particles", 1),
    ("items/deflect.rs", "send_particles", 1),
    ("items/fishing.rs", "send_particles", 1),
    ("items/spear.rs", "send_particles", 1),
    ("mobs/creatures/mod.rs", "send_particles", 6),
    ("network/lan_particles.rs", "send_particles", 1),
    ("items/totem.rs", "tracking_emitter_for", 1),
    ("native_hooks/shared/particle_count_tests.rs", "tracking_emitter", 1),
    ("network/lan/mod.rs", "tracking_emitter", 1),
    ("network/remote/interaction_actions.rs", "tracking_emitter", 1),
    ("network/remote/ui.rs", "tracking_emitter", 1),
    ("interface/render_control.rs", "create", 2),
    ("administration/commands/world_cmds.rs", "create", 2),
):
    path = ROOT / GAME / path
    source = path.read_text()
    scope = "sunrise_client::particles::SpawnScope::BARE"
    if path.name == "lan_particles.rs":
        scope = "SpawnScope::BARE"
    edits[path] = add_scope(source, r"(?:self|game)\s*\.particles", method, scope, count)

probe = ROOT / "crates/sunrise-client/examples/probe/emitter_particles_probe.rs"
source = add_scope(probe.read_text(), "engine", "tracking_emitter", "SpawnScope::BARE", 1)
source = add_scope(source, "engine", "tracking_emitter_for", "SpawnScope::BARE", 1)
edits[probe] = replace(source, "    particles::{", "    particles::{\n        SpawnScope,")

path = ROOT / f"{RENDER}.rs"
source = path.read_text()
source = replace(source, "self.create_scoped(options, p, v, scope);", "self.create(options, p, v, scope);")
source = replace(source, "        self.tracking_emitter_for(options, target, body, 3);", "        self.tracking_emitter_for(options, target, body, 3, scope);")
source = replace(source, "        body: TrackingBody,\n    ) {\n        self.tracking_emitter_for", "        body: TrackingBody,\n        scope: SpawnScope<'_, '_>,\n    ) {\n        self.tracking_emitter_for")
source = replace(source, "        life: u32,\n    ) {\n        self.emit(&options, body, SpawnScope::BARE);", "        life: u32,\n        scope: SpawnScope<'_, '_>,\n    ) {\n        self.emit(&options, body, scope);")
source = replace(source, "self.create(&flash, s.at, [0.0; 3]);", "self.create(&flash, s.at, [0.0; 3], SpawnScope::BARE);")
edits[path] = add_scope(source, "engine", "create", "SpawnScope::BARE", 4)

path = ROOT / f"{RENDER}/spawn.rs"
source = path.read_text()
wrapper = """    pub fn create(&mut self, options: &ParticleOptions, p: [f64; 3], v: [f64; 3]) {
        self.create_scoped(options, p, v, SpawnScope::BARE);
    }

    pub(super) fn create_scoped("""
source = replace(source, wrapper, "    pub fn create(")
source = replace(source, "        count: u32,", "        count: u32,\n        scope: SpawnScope<'_, '_>,")
source = replace(source, "self.add(options, at, v);", "self.add_with(options, at, v, ParticleSpawnFlags::default(), scope);")
edits[path] = replace(source, "self.add(options, p, v);", "self.add_with(options, p, v, ParticleSpawnFlags::default(), scope);")

path = ROOT / f"{RENDER}/polytone.rs"
source = add_scope(path.read_text(), "engine", "create", "SpawnScope::BARE", 1)
edits[path] = replace(source, "    ParticleType, ParticleUpdate, PendingParticle, Sprite,", "    ParticleType, ParticleUpdate, PendingParticle, SpawnScope, Sprite,")

for path, source in edits.items():
    path.write_text(source)
