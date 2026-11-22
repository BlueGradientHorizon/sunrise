#!/usr/bin/env python3
"""Apply the P3-16 step 5 mechanical removals to its step 4 base."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RENDER = "crates/sunrise-client-render/src/particles"
VOCAB = "crates/sunrise-mod-vocab/src"


def edit(path, transform):
    file = ROOT / path
    old = file.read_text()
    new = transform(old)
    assert new != old, path
    file.write_text(new)


def replace(text, old, new, count=1):
    assert text.count(old) == count, old
    return text.replace(old, new)


def cut(text, start, end):
    first = text.index(start)
    last = text.index(end, first)
    return text[:first] + text[last:]


def remove_particle_clones(text):
    text = replace(text, "#[derive(Clone, Debug)]\npub struct Particle {", "#[derive(Debug)]\npub struct Particle {")
    start = "let mut ordinary = Particle::blank(ParticleType::Flame, [0.0; 3]);\n"
    for indent in (" " * 16, " " * 12):
        text = replace(
            text,
            f"{indent}{start}{indent}ordinary.lifetime = 100;\n"
            f"{indent}engine.particles = vec![ordinary.clone(); 12_288];",
            f"{indent}let ordinary = || {{\n{indent}    {start}"
            f"{indent}    ordinary.lifetime = 100;\n{indent}    ordinary\n{indent}}};\n"
            f"{indent}engine.particles = (0..12_288).map(|_| ordinary()).collect();",
        )
    text = replace(text, "engine.push(ordinary.clone())", "engine.push(ordinary())")
    text = replace(text, "engine.push(ordinary);", "engine.push(ordinary());")
    fields = ("velocity", "lifetime", "gravity", "friction", "size", "rgba", "behaviour")
    text = replace(text, "        let original = particle.clone();", "\n".join(
        f"        let {field} = particle.{field};" for field in fields
    ))
    for field in fields:
        text = replace(text, f"assert_eq!(particle.{field}, original.{field})", f"assert_eq!(particle.{field}, {field})")
    return text


def remove_obsolete_tests(text):
    text = replace(text, "use super::ROLL_LIMIT;\n", "")
    text = cut(text, "/// Counts its ticks, so a test", "/// One engine tick of a single provider particle")
    text = cut(text, "/// Counts its calls and the particles each held.", "fn engine_with(")
    text = cut(text, "#[test]\nfn a_providers_particle_moves_changes_frame", "/// A particle a provider poses")
    text = cut(text, "#[test]\nfn rolls_at_the_accepted_limit", "fn lit_chunk()")
    text = replace(
        text, "    let mut copy = p.clone();",
        '''    let mut other = e
        .controlled("example:orbit", [0.0; 3], true, Moving(0))
        .unwrap();
    other.lifetime = 2;''',
    )
    text = replace(text, "&mut copy,", "&mut other,")
    text = replace(text, "assert_eq!(copy.position, [1.0, 0.0, 0.0]); // controller state cloned independently", "assert_eq!(other.position, [1.0, 0.0, 0.0]);")
    text = replace(text, "    e.particles.push(p.clone());", "    e.particles.push(p);")
    text = replace(text, "    e.push(copy);", "    e.push(other);")
    text = replace(text, '''    e.tick_particle(&mut p, &level);
    assert!(p.removed);
    assert_eq!(p.position, [3.0, 0.0, 0.0]);
''', "")
    text = replace(text, "    assert_eq!(e.particles[0].age, 2);", '''    assert_eq!(e.particles[0].age, 2);
    let mut p = e.particles.pop().unwrap();
    e.tick_particle(&mut p, &level);
    assert!(p.removed);
    assert_eq!(p.position, [3.0, 0.0, 0.0]);
    e.particles.push(p);''')
    return text


if __name__ == "__main__":
    edit(RENDER + ".rs", remove_particle_clones)
    edit(RENDER + "/native/tests.rs", remove_obsolete_tests)
    edit(RENDER + "/native.rs", lambda s: cut(
        replace(s, "    fn clone_box(&self) -> Box<dyn Controller>;\n", ""),
        "    fn clone_box(&self) -> Box<dyn Controller> {", "pub(super) type Factories",
    ).replace("pub(super) type Factories", "}\n\npub(super) type Factories", 1))
    edit(VOCAB + "/wire/mod.rs", lambda s: cut(
        s, "    #[test]\n    fn provider_particle_state_survives_the_wire()", "    #[test]\n    fn values_survive_a_round_trip()",
    ))
