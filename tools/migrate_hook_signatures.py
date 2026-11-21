#!/usr/bin/env python3
"""One-shot migration from #3636's source-derived hook signatures to catalog declarations.

From the destination checkout, run with --base-worktree pointing at a clean, detached
677d3116bd2 checkout. The script instruments the old emitter temporarily to extract its
resolved targets and the ordered parameters it actually emits; it never uses the new
emitter as the migration oracle. --snapshot reuses that command's capture.
"""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile


BASE = "677d3116bd219bd62761b66f9674386745801a3f"
DECLARATIONS = Path("crates/sunrise-mod-vocab/src/hooks")
GENERATOR = Path("crates/sunrise-mod-api-gen")
MARKER = "HOOK_MIGRATION "
EFFECTS = "mod_api::parts::effects::dispatch_mob_effect_tick"
CAPTURE = r'''
        if std::env::var_os("SUNRISE_MIGRATION_DUMP").is_some() {
            let boundary = |(n, ty): &(String, &Type)| {
                let t = if let Some((draw, _)) = DrawContext::of(contexts, ty) {
                    format!("&mut dyn {}", draw.trait_name)
                } else {
                    ty.to_token_stream().to_string()
                };
                (n.clone(), t)
            };
            eprintln!("HOOK_MIGRATION {}", serde_json::json!({
                "name": name, "target": target, "sig": sig.to_token_stream().to_string(),
                "native_params": sig.inputs.iter().filter_map(|arg| match arg {
                    FnArg::Typed(arg) => Some((arg.pat.to_token_stream().to_string(), arg.ty.to_token_stream().to_string())),
                    FnArg::Receiver(_) => None,
                }).collect::<Vec<_>>(),
                "vis": method.vis.to_token_stream().to_string(), "receiver": receiver_mut,
                "params": carried.iter().map(boundary).collect::<Vec<_>>(),
                "is_carrier": !declared.is_empty(), "is_direct": call.starts_with("dispatch_"),
                "fire": in_game && !declaration_only && (fill || super::has_call(&method.block, &format!("fire_{name}"))),
                "external": !in_game, "phase": hook.context.map(|phase| format!("{phase:?}")),
                "inert": inert_signal, "ticks": ticks, "render": is_render,
            }));
        }
'''

DRIVER = r'''
use std::{fs, path::PathBuf};
use sunrise_mod_api_gen as generator;

fn main() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1).map(PathBuf::from);
    let root = args.next().unwrap();
    let out = args.next().unwrap();
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    generator::generate(&root, &out, generator::Target::Vocab)?;
    generator::generate(&root, &out, generator::Target::Client)?;
    generator::generate_api_parts(&root, &out)?;
    generator::generate_render(&root, &out)?;
    Ok(())
}
'''


def capture(base: Path) -> str:
    rev = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=base, text=True).strip()
    dirty = subprocess.check_output(["git", "status", "--porcelain"], cwd=base, text=True)
    if rev != BASE or dirty:
        raise ValueError(f"expected a clean worktree at {BASE}, got {rev}")
    dispatch = base / GENERATOR / "src/dispatch.rs"
    example = base / GENERATOR / "examples/hook_signatures_migration.rs"
    source = dispatch.read_text()
    anchor = "        if is_render {\n            render_dispatch.push_str(&event_struct);"
    if source.count(anchor) != 1:
        raise ValueError("base emitter has changed")
    example.parent.mkdir(exist_ok=True)
    # Refuse an existing scratch file, even one ignored by Git.
    with example.open("x") as file:
        file.write(DRIVER)
    try:
        dispatch.write_text(source.replace(anchor, CAPTURE + anchor))
        target = base / "target"
        target.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=target) as out:
            result = subprocess.run(
                ["cargo", "run", "--locked", "-p", "sunrise-mod-api-gen", "--example",
                 "hook_signatures_migration", "--", str(base), out],
                cwd=base, env={**os.environ, "SUNRISE_MIGRATION_DUMP": "1"},
                stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
            )
            if result.returncode:
                raise ValueError(result.stderr)
            return result.stderr
    finally:
        example.unlink()
        dispatch.write_text(source)


def compact(source: str) -> str:
    return "".join(source.split())


def parameters(row: dict) -> list[str]:
    args = list(row["native_params"])
    if row["phase"]:
        phase_type = {
            "Registration": "RegistrationContext",
            "DataLoad": "LoadContext",
            "Loop": "LoopContext",
            "Worldgen": "OwningWorldgen",
            "ChunkDisk": "OwningChunkDisk",
        }[row["phase"]]
        if not args or args[0][0] != "context" or not re.fullmatch(
            rf"&(?:\w+::)*{phase_type}(?:<'_>)?", compact(args[0][1])
        ):
            raise ValueError("unmapped phase parameter")
        args.pop(0)
    engine = []
    if row["ticks"]:
        if args.pop(0) != ["ticks", "& mut Ticks"]:
            raise ValueError("unmapped ticks parameter")
        engine.append("#[engine] ticks: &mut Ticks")
    if row["inert"]:
        if args.pop() != ["inert", "& mut bool"]:
            raise ValueError("unmapped inactivity parameter")
        engine.append("#[engine] inert: &mut bool")
    if row["is_carrier"]:
        if len(args) != 1 or args[0][0] != "call" or not re.fullmatch(
            r"(?:(?:super|crate)::(?:\w+::)*|mod_api::__private::)?\w+Call(?:<'_>)?",
            compact(args[0][1]),
        ):
            raise ValueError("unmapped carrier parameter")
        if row["target"] == EFFECTS:
            # The base emitter projects this one handwritten call into two Parts.
            if row["name"] == "apply_effect_tick":
                engine.append("#[engine] tick_count: i32")
            elif row["name"] != "should_apply_effect_tick_this_tick":
                raise ValueError("unknown effect tick projection")
            engine.append("#[engine] make_call: impl FnOnce(EffectKey, i32) -> ApplyEffectTickCall<'_>")
    else:
        draws = {
            "&mutBlockEntityDraw": "&mutdynBlockEntityDrawContext",
            "&mutHudBatch": "&mutdynDrawContext",
            "&mutsunrise_client::world_geometry::WorldGeometry": "&mutdynWorldDrawContext",
        }
        expected = [
            [name, draws.get(compact(ty), compact(re.sub(r"\bBlockStateId\b", "BlockState", ty)))]
            for name, ty in args
        ]
        if expected != [[name, compact(ty)] for name, ty in row["params"]]:
            raise ValueError(f"unmapped boundary: {args} → {row['params']}")
    return [*(f"{name}: {ty}" for name, ty in row["params"]), *engine]


def migrate(text: str, area: str, rows: dict) -> str:
    declarations = list(re.finditer(r"(?m)^    #\[hook\((.*?)\)\][ \t]*$", text, re.S))
    for attribute in reversed(declarations):
        signature = re.search(r"(?m)^    fn (\w+)\(.*?\);", text[attribute.end():], re.S)
        if not signature:
            raise ValueError(f"no hook signature after attribute in {area}")
        name = signature[1]
        row = rows.pop(name)
        try:
            args = parameters(row)
        except (IndexError, ValueError) as error:
            raise ValueError(f"{name}: {error}") from error
        signature_start = attribute.end() + signature.start()
        signature_end = attribute.end() + signature.end()
        args = ",\n        ".join(args)
        form = f"    fn {name}(\n        {args},\n    );" if args else f"    fn {name}();"
        text = text[:signature_start] + form + text[signature_end:]

        attrs = attribute[1]
        prefix = []
        old_target = re.search(r'target\s*=\s*"[^"\n]*"', attrs)
        default = f"sunrise_client::game::native_hooks::{area}::Game<'_>::native_{name}"
        target = f'target = "{row["target"]}"'
        if old_target:
            attrs = attrs[:old_target.start()] + target + attrs[old_target.end():]
        elif row["target"] != default:
            prefix.append(target)
        if not row["external"]:
            if row["receiver"] is None:
                prefix.append('receiver = "associated"')
            elif not row["receiver"]:
                prefix.append('receiver = "read"')
            if not row["fire"]:
                prefix.append('dispatch = "direct"')
            elif not row["is_carrier"]:
                prefix.append('dispatch = "flat"')
            elif args and "mod_api :: __private ::" in row["native_params"][-1][1]:
                prefix.append('dispatch = "api_carrier"')
        if prefix:
            attrs = ", ".join(prefix) + ", " + attrs.lstrip()
        text = text[:attribute.start()] + f"    #[hook({attrs})]" + text[attribute.end():]
    return text


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--base-worktree", type=Path)
    mode.add_argument("--snapshot", type=Path)
    options = parser.parse_args()
    source = (options.snapshot.read_text() if options.snapshot else capture(options.base_worktree.resolve()))
    rows = {}
    for line in source.splitlines():
        if line.startswith(MARKER):
            row = json.loads(line[len(MARKER):])
            if rows.setdefault(row["name"], row) != row:
                raise ValueError(f"inconsistent baseline emissions: {row['name']}")
    outputs = {}
    for path in sorted(DECLARATIONS.glob("*.rs")):
        before = path.read_text()
        after = migrate(before, path.stem, rows)
        if before != after:
            outputs[path] = after
    if rows:
        raise ValueError(f"unused baseline declarations: {', '.join(rows)}")
    for path, content in outputs.items():
        path.write_text(content)
    subprocess.run(["rustfmt", "--edition", "2024", *outputs], check=True)
    print(f"Migrated {len(outputs)} declaration files")


if __name__ == "__main__":
    main()
