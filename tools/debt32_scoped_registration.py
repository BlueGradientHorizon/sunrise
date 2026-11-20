#!/usr/bin/env python3
"""Rewrite debt 32's collect calls after the scoped-registration generator lands.

Run one slice per commit: debt32_scoped_registration.py {component,plain}.
Pass --dry-run to show the diff without writing any files.
"""

import argparse
import difflib
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HOOKS = "crates/sunrise-mod-vocab/src/hooks"
CLIENT = "crates/sunrise-client"
GAME = f"{CLIENT}/src/game"
BOOK = "docs/modding/src/how-to/client/render-pipelines.md"
LOAD_STATE = "crates/sunrise-mod-api/src/load_state.rs"

# Each declaration is still at the catalog's default version (1).
DECLARATIONS = {
    "component": [
        ("component_types", "both", "types", "RegisterEvent"),
    ],
    "plain": [
        ("create_skull_models", "client", "models", "EntityRenderersEvent.CreateSkullModels"),
        ("map_decoration_types", "both", "types", "RegisterEvent"),
        (
            "register_entity_spectator_shaders",
            "client",
            "shaders",
            "RegisterEntitySpectatorShadersEvent",
        ),
        ("register_fluid_models", "client", "models", "RegisterFluidModelsEvent"),
        ("register_render_pipelines", "client", "pipelines", "RegisterRenderPipelinesEvent"),
        ("register_texture_atlases", "client", "atlases", "RegisterTextureAtlasesEvent"),
    ],
}

# Match only the named handler with the given event type. Counts are per body,
# so the two fluid listeners in mods/mod.rs can't make up for one another.
LISTENERS = {
    "component_types": ("ComponentTypesEvent", {
        "crates/sunrise-mod-api-gen/dispatch-tests/src/fixtures/base.rs": (1,),
    }),
    "create_skull_models": ("CreateSkullModelsEvent", {
        f"{CLIENT}/tests/it/mod_api_hooks.rs": (1,),
    }),
    "register_entity_spectator_shaders": ("RegisterEntitySpectatorShadersEvent", {
        f"{HOOKS}/register_entity_spectator_shaders.rs": (1,),
        f"{GAME}/modding/native_spectator_shader_tests.rs": (2,),
    }),
    "register_fluid_models": ("RegisterFluidModelsEvent", {
        f"{GAME}/modding/mods/mod.rs": (1, 1),
        f"{GAME}/world/packs/mod.rs": (1,),
    }),
    "register_render_pipelines": ("RegisterRenderPipelinesEvent", {
        f"{HOOKS}/register_render_pipelines.rs": (1,),
        BOOK: (1,),
    }),
    "register_texture_atlases": ("RegisterTextureAtlasesEvent", {
        f"{GAME}/interface/client_reload/texture_atlases.rs": (2,),
    }),
}


def replace_once(source, old, new, label):
    count = source.count(old)
    if count != 1:
        raise ValueError(f"{label}: expected one exact match, found {count}")
    return source.replace(old, new, 1)


def rewrite_declaration(source, hook, side, field, neoforge):
    fields = [f'side = "{side}"', 'contract = "collect"', f'out = "{field}"']
    if hook == "register_render_pipelines":
        fields.append('collect_validator = "crate::client::render_pipeline::validate_render_pipelines"')
    fields.extend(['context = "registration"', f'neoforge = "{neoforge}"'])
    start = "    #[hook(\n        "
    end = f"\n    )]\n    fn {hook}();"
    old = start + ",\n        ".join(fields) + end
    fields.insert(3, 'registration_append = "register"')
    fields.append("version = 2")
    new = start + ",\n        ".join(fields) + end
    return replace_once(source, old, new, f"#[hook] for {hook}")


def rewrite_pushes(source, scope, field, counts, label):
    matches = list(re.finditer(scope, source, re.MULTILINE | re.DOTALL))
    if len(matches) != len(counts):
        raise ValueError(f"{label}: expected {len(counts)} bodies, found {len(matches)}")
    access = rf"\bevent\s*\.\s*{field}\b"
    push = r"(?m)^([ \t]*)" + access + r"\s*\.\s*push(?=\s*\()"
    for number, (match, expected) in reversed(list(enumerate(zip(matches, counts), 1))):
        body = match["body"]
        rewritten, pushes = re.subn(push, r"\1event.register", body)
        accesses = len(re.findall(access, body))
        if (pushes, accesses) != (expected, expected):
            raise ValueError(
                f"{label} body {number}: expected {expected} event.{field}.push calls and "
                f"no other event.{field} access, found {pushes} pushes in {accesses} accesses"
            )
        start, end = match.span("body")
        source = source[:start] + rewritten + source[end:]
    return source


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("slice", choices=DECLARATIONS)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    originals = {}
    edits = {}

    def source(path):
        if path not in originals:
            originals[path] = (ROOT / path).read_bytes().decode("utf-8")
        return edits.get(path, originals[path])

    # Prepare and validate the entire slice before writing any file.
    try:
        for hook, side, field, neoforge in DECLARATIONS[args.slice]:
            path = f"{HOOKS}/{hook}.rs"
            edits[path] = rewrite_declaration(source(path), hook, side, field, neoforge)
            if hook not in LISTENERS:
                continue
            event, listeners = LISTENERS[hook]
            scope = (
                rf"^(?P<indent> *)fn {hook}\(\n"
                r"(?P=indent)    &self,\n"
                r"(?P=indent)    \w+: &(?:mod_api::)?RegistrationContext,\n"
                rf"(?P=indent)    event: &mut (?:mod_api::)?{event}<'_>,\n"
                r"(?P=indent)\) \{\n(?P<body>.*?)^(?P=indent)\}$"
            )
            for path, counts in listeners.items():
                edits[path] = rewrite_pushes(source(path), scope, field, counts, hook)
        if args.slice == "component":
            path = LOAD_STATE
            # This listener returns the probe answers in their original order.
            signature = (
                "            fn component_types(\n"
                "                &self,\n"
                "                cx: &RegistrationContext,\n"
                "                event: &mut ComponentTypesEvent<'_>,\n"
                "            ) {\n"
            )
            end = "            }\n"
            edits[path] = replace_once(
                source(path),
                signature + "                *event.types = probe(cx);\n" + end,
                signature
                + "                for answer in probe(cx) {\n"
                "                    event.register(answer);\n"
                "                }\n"
                + end,
                "component_types probe assignment",
            )
        else:
            path = BOOK
            # The two abbreviated examples are standalone fenced Rust snippets.
            scope = r"^```rust,ignore\n(?P<body>event\s*\.\s*pipelines\b.*?)^```$"
            edits[path] = rewrite_pushes(
                source(path), scope, "pipelines", (1, 1), "pipeline examples"
            )
    except (OSError, ValueError) as error:
        parser.exit(1, f"{path}: {error}\n")

    for path, new in edits.items():
        if args.dry_run:
            sys.stdout.writelines(
                difflib.unified_diff(
                    originals[path].splitlines(keepends=True),
                    new.splitlines(keepends=True),
                    fromfile=f"a/{path}",
                    tofile=f"b/{path}",
                )
            )
        else:
            (ROOT / path).write_bytes(new.encode("utf-8"))


if __name__ == "__main__":
    main()
