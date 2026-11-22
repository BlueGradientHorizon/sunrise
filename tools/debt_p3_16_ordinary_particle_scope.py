#!/usr/bin/env python3
"""Make the scope argument explicit at ordinary ParticleEngine::add call sites.

Run once on the final P3-16 review base. The named and controller spawns are
then given their live-level loans separately; built-ins keep the same admission.
"""

import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CLIENT = "crates/sunrise-client/src"
RENDER = "crates/sunrise-client-render/src"
ENGINE = f"{RENDER}/particles.rs"
GPU_TESTS = f"{RENDER}/gfx/core_particles_tests.rs"
BENCH = f"{RENDER}/particles/tick_bench.rs"

# Most calls target Game.particles. These are the other ParticleEngine bindings.
OTHER_RECEIVERS = {
    f"{CLIENT}/animate_tick.rs": r"\bengine",
    f"{CLIENT}/animate_tick_parts_tests.rs": r"\bstepped",
    f"{CLIENT}/game/interface/client_ambient.rs": r"(?<!\.)\bparticles",
    ENGINE: r"\b(?:self|engine)",
    GPU_TESTS: r"\bengine",
    BENCH: r"\bengine",
}

# These modules already follow the fully-qualified style for spawn scopes.
QUALIFIED = {
    f"{CLIENT}/game/entities/melee.rs",
    f"{CLIENT}/game/mobs/creatures/mod.rs",
    f"{CLIENT}/game/native_hooks/shared/particle_count_tests.rs",
}


def main():
    files = subprocess.check_output(["git", "ls-files", CLIENT], cwd=ROOT, text=True)
    edits = {}
    count = 0
    for name in sorted(set(files.splitlines()) | OTHER_RECEIVERS.keys()):
        if not name.endswith(".rs"):
            continue
        path = ROOT / name
        source = path.read_text()
        had_scope = "SpawnScope" in source
        receivers = [r"\b(?:self|game|this)\s*\.particles"] if name.startswith(CLIENT) else []
        if name in OTHER_RECEIVERS:
            receivers.append(OTHER_RECEIVERS[name])
        calls = list(re.finditer(rf"(?:{'|'.join(receivers)})\s*\.add\(", source))
        if not calls:
            continue
        count += len(calls)
        scope = "SpawnScope::BARE"
        if name in QUALIFIED:
            scope = f"sunrise_client::particles::{scope}"
        for call in reversed(calls):
            # These reviewed argument lists have balanced parentheses, including in strings.
            end = call.end()
            depth = 1
            while depth:
                depth += (source[end] == "(") - (source[end] == ")")
                end += 1
            args = source[call.end() : end - 1].rstrip()
            args = args.removesuffix(",").rstrip()
            if args.startswith("\n"):
                indent = re.match(r"\n( +)", args).group(1)
                args += f",\n{indent}{scope},\n{indent[:-4]}"
            else:
                args += f", {scope}"
            source = source[:call.end()] + args + source[end - 1:]
        if name not in {ENGINE, BENCH, *QUALIFIED, f"{CLIENT}/animate_tick_parts_tests.rs"}:
            # The animation test imports its parent's scope via `use super::*`.
            if not had_scope:
                module = "crate" if name == GPU_TESTS else "sunrise_client"
                particle_use = re.search(rf"(?m)^use {module}::particles::\{{\s*", source)
                if particle_use:
                    at = particle_use.end()
                    source = source[:at] + "SpawnScope, " + source[at:]
                else:
                    at = re.search(r"(?m)^use ", source).start()
                    source = source[:at] + f"use {module}::particles::SpawnScope;\n" + source[at:]
        edits[path] = source

    if count != 135:
        raise SystemExit(f"Expected 135 ordinary particle calls, found {count}")

    engine = ROOT / ENGINE
    old = """    /// Built-ins do not need a level loan. Use [`Self::add_with`] with a live level for native types.
    pub fn add(&mut self, options: &ParticleOptions, p: [f64; 3], v: [f64; 3]) {
        self.add_with(
            options,
            p,
            v,
            ParticleSpawnFlags::default(),
            SpawnScope::BARE,
        );
    }"""
    new = """    /// Lend the live level for named providers; built-ins can use [`SpawnScope::BARE`].
    pub fn add(
        &mut self,
        options: &ParticleOptions,
        p: [f64; 3],
        v: [f64; 3],
        scope: SpawnScope<'_, '_>,
    ) {
        self.add_with(options, p, v, ParticleSpawnFlags::default(), scope);
    }"""
    if edits[engine].count(old) != 1:
        raise SystemExit("Missing expected ParticleEngine::add implementation")
    edits[engine] = edits[engine].replace(old, new)
    for path, source in edits.items():
        path.write_text(source)


if __name__ == "__main__":
    main()
