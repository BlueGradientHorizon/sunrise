#!/usr/bin/env python3
"""Migrate the engine from the old JavaString tuple field to its accessors.

Run from any directory: python3 tools/migrate_java_string_storage.py

This is a one-off codemod for the storage change. Rustc's privacy diagnostics
identify .0 sites by actual type, so unrelated tuple fields are not touched.
Each check sees farther through the workspace dependency graph as those sites
are rewritten. Review the remaining compiler errors for context-dependent edits
(such as an owned closure result or a destructured tuple constructor).
"""

import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
# Core is already migrated. Java properties has an unrelated type with the same name.
SKIP = (Path("crates/sunrise-core"), Path("crates/sunrise-java-properties"))
CONSTRUCTOR = re.compile(rb"\bJavaString(?=\s*\()")
MUTATORS = re.compile(rb"\.(append|as_mut_slice|clear|drain|extend|extend_from_slice|insert|"
                      rb"pop|push|remove|resize|retain|reverse|sort|truncate)\s*\(")
PRIVATE_FIELD = re.compile(r"field `0` of struct `(?:\w+::)*JavaString` is private")
BORROW = re.compile(rb"&(mut\s+)?([a-zA-Z_]\w*(?:\.[a-zA-Z_]\w*)*\.)$")


def constructors():
    files = subprocess.check_output(["git", "ls-files", "-z", "--", "*.rs"], cwd=ROOT)
    for file in files.decode().split("\0"):
        if not file or any(Path(file).is_relative_to(crate) for crate in SKIP):
            continue
        path = ROOT / file
        source = path.read_bytes()
        changed = CONSTRUCTOR.sub(b"JavaString::from", source)
        if file == "crates/sunrise-mod-vocab/src/wire/wire_types.rs":
            changed = changed.replace(b"take_u16s().map(Self)", b"take_u16s().map(Self::from)")
        if changed != source:
            path.write_bytes(changed)


def fields(diagnostics):
    sites = {}
    for line in diagnostics:
        event = json.loads(line)
        if event["reason"] != "compiler-message":
            continue
        message = event["message"]
        if not PRIVATE_FIELD.fullmatch(message["message"]):
            continue
        for span in message["spans"]:
            if span["is_primary"] and span["expansion"] is None:
                file = Path(span["file_name"])
                sites.setdefault(file, set()).add((span["byte_start"], span["byte_end"]))
    count = 0
    for file, spans in sites.items():
        path = ROOT / file
        source = path.read_bytes()
        for start, end in sorted(spans, reverse=True):
            if source[start:end] != b"0" or source[start - 1:start] != b".":
                raise ValueError(f"unexpected JavaString field span: {file}:{start}:{end}")
            rest = source[end:]
            line_start = source.rfind(b"\n", 0, start) + 1
            borrow = BORROW.search(source[line_start:start])
            if borrow and not rest.lstrip().startswith((b".", b"[")):
                start = line_start + borrow.start()
                replacement = borrow[2] + (b"units_mut()" if borrow[1] else b"units()")
            elif rest.startswith(b".as_slice()"):
                end += len(b".as_slice()")
                replacement = b"units()"
            elif rest.startswith(b".clone()"):
                end += len(b".clone()")
                replacement = b"units().to_vec()"
            elif rest.startswith(b".into_iter()"):
                replacement = b"into_units()"
            elif MUTATORS.match(rest):
                replacement = b"units_mut()"
            else:
                replacement = b"units()"
            source = source[:start] + replacement + source[end:]
            count += 1
        path.write_bytes(source)
    return count


def main():
    constructors()
    while True:
        check = subprocess.run(
            ["cargo", "check", "--workspace", "--all-targets", "--keep-going", "--message-format=json"],
            cwd=ROOT, stdout=subprocess.PIPE, text=True,
        )
        count = fields(check.stdout.splitlines())
        print(f"Rewrote {count} JavaString field accesses", flush=True)
        if not count:
            for line in check.stdout.splitlines():
                event = json.loads(line)
                if event["reason"] == "compiler-message" and event["message"]["level"] == "error":
                    print(event["message"]["rendered"], end="")
            return check.returncode


if __name__ == "__main__":
    raise SystemExit(main())
