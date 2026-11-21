#!/usr/bin/env python3
"""Every tools/port_map/ row that names a Sunrise function still finds it.

`python3 tools/port_ledger.py where Class.method` is where a parity fix starts (AGENTS.md), and it answers
with the rows of `tools/port_map/*.tsv`. A `port` or `inline` row's third column names the Sunrise function
that ports the method, as `path::owner::fn`. Nothing read that column, so a refactor that moved a function
and left its row behind turned the ledger's answer into a file that no longer holds it.

A row resolves when its file exists and its source mentions the function name. `tools/port_map_known_stale.txt`
lists the rows that do not resolve today, one `map file<TAB>vanilla name<TAB>path::owner::fn` per line, and it
only shrinks (as `crates/sunrise-mod-api-gen/unwinding-methods.txt` does): a row that does not resolve and is
not listed fails, a listed row that resolves now (or whose row is gone) fails until its line is deleted, and a
line added since `--base` fails. A row's path is part of its key, so repointing one row into another dead path
is a new row, and fails.

    python3 tools/ci/check_port_map_paths.py                 # the check
    python3 tools/ci/check_port_map_paths.py --base SHA      # and nothing added to the list since SHA
    python3 tools/ci/check_port_map_paths.py --refresh       # delete the lines that resolve now

Fix a row by pointing it at the function's new home: the file that declares `fn name`, or the `impl` block it
moved into. `python3 tools/port_ledger.py map-check FILE` checks the vanilla side and the same Rust side
against a built ledger.
"""

import argparse
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MAP = "tools/port_map/*.tsv"
LIST = "tools/ci/port_map_known_stale.txt"
# The verdicts whose third column names a Rust function; `missing`, `home` and `skip:<reason>` name none.
NAMED = ("port", "inline")


def rows(root=ROOT):
    """(map file, line number, vanilla name, `path::owner::fn`) of every row that names a Rust function."""
    for path in sorted(root.glob(MAP)):
        text = path.read_text(encoding="utf-8", errors="replace")
        for number, line in enumerate(text.splitlines(), 1):
            if not line.strip() or line.lstrip().startswith("#"):
                continue
            parts = [part.strip() for part in line.split("\t")]
            if len(parts) >= 3 and parts[1] in NAMED and "::" in parts[2]:
                yield path.relative_to(root).as_posix(), number, parts[0], parts[2]


def unresolved(refs, root=ROOT):
    """Identity -> why, for each row whose file is missing or does not name its function.

    An identity is the row's `(map file, vanilla name, path::owner::fn)`: the line number is left out because
    rows move down their file, and the path is in because repointing a row is writing a new one.
    """
    sources, out = {}, {}
    for map_file, _, name, where in refs:
        file_, _, rest = where.partition("::")
        fn = rest.rpartition("::")[2]
        target = root / file_
        if not target.is_file():
            out[(map_file, name, where)] = f"no file {file_}"
            continue
        if file_ not in sources:
            sources[file_] = target.read_text(encoding="utf-8", errors="replace")
        if not re.search(rf"\b{re.escape(fn)}\b", sources[file_]):
            out[(map_file, name, where)] = f"{file_} does not mention {fn}"
    return out


def listed(text):
    """The identities of a list file's lines, comments and blank lines left out."""
    return {tuple(line.split("\t")) for line in text.splitlines()
            if line.strip() and not line.lstrip().startswith("#")}


def base_listed(base):
    """The list as it was at `base`, or None when the file is new there."""
    known = subprocess.run(["git", "-C", str(ROOT), "cat-file", "-e", f"{base}^{{commit}}"],
                           capture_output=True, text=True)
    if known.returncode != 0:
        sys.exit(f"git cannot read {base}, so the list at the base cannot be read:\n{known.stderr}")
    was = subprocess.run(["git", "-C", str(ROOT), "show", f"{base}:{LIST}"],
                         capture_output=True, text=True)
    return listed(was.stdout) if was.returncode == 0 else None


def report(unexplained, obsolete=(), lines=None):
    """The failure text: each row at its map file and line, and what to do about it."""
    lines = lines or {}
    problems = []
    for identity, why in sorted(unexplained.items()):
        map_file, name, where = identity
        problems.append(f"{map_file}:{lines.get(identity, '?')}: {name} -> {where}: {why}")
    for _, name, where in sorted(obsolete):
        problems.append(f"tools/ci/port_map_known_stale.txt: {name} -> {where}: this row resolves now; "
                        f"delete its line (or run --refresh)")
    return problems


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--base", help="a commit whose list this one may not grow past")
    parser.add_argument("--refresh", action="store_true", help="delete the list lines that resolve now")
    args = parser.parse_args(argv)

    refs = list(rows())
    stale = unresolved(refs)
    path = ROOT / LIST
    known = listed(path.read_text(encoding="utf-8")) if path.exists() else set()
    unexplained = {identity: why for identity, why in stale.items() if identity not in known}
    obsolete = known - set(stale)

    problems = report(unexplained, obsolete if not args.refresh else ())
    if args.base:
        was = base_listed(args.base)
        problems += [f"tools/ci/port_map_known_stale.txt: {name} -> {where}: added since {args.base}; "
                     f"the list only shrinks" for _, name, where in (sorted(known - was) if was is not None else [])]
    if problems:
        counted = [f"{len(unexplained)} row(s) name a function that is not there"] * bool(unexplained)
        counted += [f"{len(obsolete)} listed row(s) resolve now"] * bool(obsolete)
        print(f"port map: {', '.join(counted)}:", file=sys.stderr)
        print("\n".join(problems), file=sys.stderr)
        return 1
    if args.refresh and obsolete:
        remaining = sorted("\t".join(identity) for identity in known - obsolete)
        path.write_text("".join(line + "\n" for line in remaining), encoding="utf-8")
        print(f"port map: {len(obsolete)} line(s) removed from {LIST}.")
        known -= obsolete
    print(f"port map: {len(refs)} row(s) name a Rust function, {len(known)} listed as not resolving yet.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
