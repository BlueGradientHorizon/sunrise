#!/usr/bin/env python3
"""Separate the batch fixtures' provider setup from their particle spawns (P3-16)."""

import re
from pathlib import Path


path = (Path(__file__).resolve().parents[1] /
        "crates/sunrise-client-render/src/particles/native/tests.rs")
source = path.read_text()


def replace_once(old: str, new: str) -> None:
    global source
    assert source.count(old) == 1, old
    source = source.replace(old, new, 1)


replace_once(
    '''fn provided(e: &mut ParticleEngine, provider: Arc<dyn ParticleProvider>) -> Particle {
    e.providers.insert("example:orbit".into(), provider.clone());
    e.mod_sets.get_mut("example:orbit").unwrap().provider = Some((0, provider));
    e.make(''',
    '''fn engine_with(provider: Arc<dyn ParticleProvider>) -> ParticleEngine {
    let mut e = engine();
    e.providers.insert("example:orbit".into(), provider.clone());
    e.mod_sets.get_mut("example:orbit").unwrap().provider = Some((0, provider));
    e
}
fn provided(e: &mut ParticleEngine) -> Particle {
    e.make(''',
)

# Counting fixtures share one provider for every particle in the engine.
source, count = re.subn(
    r"    let mut e = engine\(\);\n(    let (calls|probe) = Arc::new\((?:Calls|Probe)::default\(\)\);\n)",
    r"\1    let mut e = engine_with(\2.clone());\n",
    source,
)
assert count == 5
source, count = re.subn(r"provided\(&mut e, (?:calls|probe)\.clone\(\)\)", "provided(&mut e)", source)
assert count == 5

# Each invalid tick in the table uses a separate provider; no rebinding at spawn.
for before, after in [
    ("    let mut e = engine();\n    let bad:", "    let bad:"),
    ("        let mut p = provided(&mut e, Arc::new(Broken(tick)));",
     "        let mut e = engine_with(Arc::new(Broken(tick)));\n        let mut p = provided(&mut e);"),
    ("    let mut p = provided(\n        &mut e,\n        Arc::new(Broken(|p| {",
     "    let mut e = engine_with(Arc::new(Broken(|p| {"),
    ("        })),\n    );\n    tick_one", "    })));\n    let mut p = provided(&mut e);\n    tick_one"),
    ("    let mut e = engine();\n    let facings", "    let facings"),
    ("    let mut e = engine();\n    for roll in", "    for roll in"),
    ("        let mut p = provided(&mut e, Arc::new(Set(facing)));",
     "        let mut e = engine_with(Arc::new(Set(facing)));\n        let mut p = provided(&mut e);"),
    ("        let mut p = provided(&mut e, Arc::new(Set(roll)));",
     "        let mut e = engine_with(Arc::new(Set(roll)));\n        let mut p = provided(&mut e);"),
    ("    let mut e = engine();\n    let mut p = provided(&mut e, Arc::new(Replacing));",
     "    let mut e = engine_with(Arc::new(Replacing));\n    let mut p = provided(&mut e);"),
    ("    let mut e = engine();\n    let mut p = provided(&mut e, Arc::new(Pose(facing, rolls.to_vec(), center)));",
     "    let mut e = engine_with(Arc::new(Pose(facing, rolls.to_vec(), center)));\n    let mut p = provided(&mut e);"),
    ("    let mut e = engine();\n    let mut p = provided(&mut e, Arc::new(Probe::default()));",
     "    let mut e = engine_with(Arc::new(Probe::default()));\n    let mut p = provided(&mut e);"),
    ('''    let mut e = engine();
    e.set_block_state_source(counting);
    // A provider is registered, and no particle of it lives.
    e.providers
        .insert("example:orbit".into(), Arc::new(Probe::default()));''',
     '''    let mut e = engine_with(Arc::new(Probe::default()));
    e.set_block_state_source(counting);
    // A provider is registered, and no particle of it lives.'''),
    ("    let mut p = provided(&mut e, Arc::new(Probe::default()));", "    let mut p = provided(&mut e);"),
]:
    replace_once(before, after)

path.write_text(source)
