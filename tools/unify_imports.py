#!/usr/bin/env python3
"""One import form for tools/ and tools/ci/ (sunrise-org/sunrise#6068, entry 1).

Rewrites, with the ast and tokenize modules, every way a tool reaches a sibling module into the form #6065 settled:
the repository root on sys.path in the import block (`ROOT = ...; sys.path.insert(0, str(ROOT))`), then
`from tools import a, b` (or `from tools.ci import x`), so a sibling loads once, as `tools.<name>`, however the file
is run. It replaces `try: from . import x / except ImportError: import x`, `if __package__: ... else: ...` splits,
and bare sibling imports (`import lab`, `from lab import free_port`).

    python3 tools/unify_imports.py            # rewrite in place, print what it skipped
    python3 tools/unify_imports.py --check    # rewrite nothing, list the files that would change

A file with something the script cannot decide (a comment inside an import block, handler imports that differ from
the body's, a `ROOT` that is not the repository root) is left unchanged and listed.
"""
import argparse
import ast
import io
import sys
import tokenize
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
TOOLS = REPO / "tools"
TOOL_NAMES = {p.stem for p in TOOLS.glob("*.py")}
CI_NAMES = {p.stem for p in (TOOLS / "ci").glob("*.py")}
ROOT_EXPRS = {
    "Path(__file__).resolve().parents[1]": 1,
    "Path(__file__).resolve().parent.parent": 1,
    "pathlib.Path(__file__).resolve().parent.parent": 1,
    "Path(__file__).resolve().parents[2]": 2,
}
MAX_LINE = 120


class Skip(Exception):
    pass


class Unit:
    """One file: source, offsets, and the edits made to it."""

    def __init__(self, path):
        self.path = path
        self.in_ci = path.parent.name == "ci"
        self.depth = 2 if self.in_ci else 1
        self.text = path.read_text(encoding="utf-8")
        self.lines = self.text.splitlines(keepends=True)
        self.tree = ast.parse(self.text)
        self.starts = [0]
        for line in self.lines:
            self.starts.append(self.starts[-1] + len(line))
        self.comments = [(self.starts[row - 1] + col, text) for (row, col), text in self.comment_positions()]
        self.edits = []
        self.runs = []  # (top-level statement, in a def/class?)

    def comment_positions(self):
        for token in tokenize.generate_tokens(io.StringIO(self.text).readline):
            if token.type == tokenize.COMMENT:
                yield token.start, token.string

    def offset(self, row, col):
        line = self.lines[row - 1]
        return self.starts[row - 1] + len(line.encode("utf-8")[:col].decode("utf-8"))

    def span(self, node):
        return self.offset(node.lineno, node.col_offset), self.offset(node.end_lineno, node.end_col_offset)

    # --- what an import statement names -----------------------------------------------------------------------
    def sibling_module(self, top):
        if self.in_ci and top in CI_NAMES:
            return "tools.ci"
        if top in TOOL_NAMES:
            return "tools"
        return None

    def items(self, node):
        """[(module, name, asname)] an import statement loads from the tools package, or None if it is not one."""
        if isinstance(node, ast.ImportFrom):
            if node.level:
                base = {1: "tools.ci" if self.in_ci else "tools", 2: "tools"}[node.level]
                module = base + ("." + node.module if node.module else "")
            else:
                top = (node.module or "").split(".")[0]
                package = self.sibling_module(top)
                if package is None:
                    return None
                module = f"{package}.{node.module}"
            return [(module, alias.name, alias.asname) for alias in node.names]
        found = []
        for alias in node.names:
            package = self.sibling_module(alias.name.split(".")[0])
            if package is None:
                if found:
                    raise Skip(f"line {node.lineno}: import of a sibling and a library in one statement")
                return None
            if "." in alias.name:
                raise Skip(f"line {node.lineno}: dotted import {alias.name}")
            found.append((package, alias.name, alias.asname))
        return found

    def branch_items(self, body, where):
        """The items of a branch of imports; a sys.path.insert statement is allowed (the script adds its own)."""
        out = []
        for stmt in body:
            if isinstance(stmt, (ast.Import, ast.ImportFrom)):
                got = self.items(stmt)
                if got is None:
                    raise Skip(f"line {stmt.lineno}: {where} imports something that is not a tools module")
                out.extend(got)
            elif isinstance(stmt, ast.Expr) and ast.unparse(stmt).startswith("sys.path."):
                continue
            else:
                raise Skip(f"line {stmt.lineno}: {where} holds more than imports")
        return out

    def converted(self, node):
        """The items replacing a statement, or None when the statement is not an import to rewrite."""
        if isinstance(node, (ast.Import, ast.ImportFrom)):
            return self.items(node)
        if isinstance(node, ast.Try):
            if not any(handler.type is not None and "ImportError" in ast.dump(handler.type) for handler in node.handlers):
                return None
            if len(node.handlers) != 1 or node.orelse or node.finalbody:
                raise Skip(f"line {node.lineno}: try with more than one handler, else or finally")
            if not all(isinstance(s, (ast.Import, ast.ImportFrom)) for s in node.body):
                return None  # a try of something else that happens to catch ImportError
            first, second = node.body, node.handlers[0].body
        elif isinstance(node, ast.If):
            test = ast.unparse(node.test)
            if test != "__package__":
                return None
            first, second = node.body, node.orelse
        else:
            return None
        if not all(isinstance(s, (ast.Import, ast.ImportFrom)) or ast.unparse(s).startswith("sys.path.") for s in second):
            return None
        wanted = self.branch_items(first, "the package branch")
        fallback = self.branch_items(second, "the script branch")
        if sorted(wanted, key=str) != sorted(fallback, key=str):
            raise Skip(f"line {node.lineno}: the two branches load different names")
        return wanted

    # --- finding runs ------------------------------------------------------------------------------------------
    def find(self, body, lazy):
        run = []

        def flush():
            if run:
                self.runs.append((list(run), lazy))
                run.clear()

        for stmt in body:
            items = self.converted(stmt)
            if items is not None:
                if run and stmt.lineno != run[-1][0].end_lineno + 1:
                    flush()
                run.append((stmt, items))
                continue
            flush()
            inner_lazy = lazy or isinstance(stmt, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef, ast.Lambda))
            for field in ("body", "orelse", "finalbody"):
                child = getattr(stmt, field, None)
                if isinstance(child, list) and child and isinstance(child[0], ast.stmt):
                    self.find(child, inner_lazy)
            for handler in getattr(stmt, "handlers", []):
                self.find(handler.body, inner_lazy)
            for case in getattr(stmt, "cases", []):
                self.find(case.body, inner_lazy)
        flush()

    def emit(self, items, indent):
        statements = []
        for module, name, asname in items:
            alias = f"{name} as {asname}" if asname else name
            if statements and statements[-1][0] == module and name != "*" and statements[-1][1][-1] != "*":
                statements[-1][1].append(alias)
            else:
                statements.append((module, [alias]))
        out = []
        for module, aliases in statements:
            line = f"from {module} import {', '.join(aliases)}"
            if len(indent) + len(line) > MAX_LINE:
                inner = ",\n".join(f"{indent}    {alias}" for alias in aliases)
                line = f"from {module} import (\n{inner},\n{indent})"
            out.append(line)
        return f"\n{indent}".join(out)

    def rewrite_runs(self):
        for run, lazy in self.runs:
            first, last = run[0][0], run[-1][0]
            start, end = self.offset(first.lineno, first.col_offset), self.offset(last.end_lineno, last.end_col_offset)
            # `# noqa` marks on the import lines, and the comment beside a try/if header, explained the fallback
            if any(start <= c < end and not text.startswith("# noqa") and not self.on_header(c, run) for c, text in self.comments):
                raise Skip(f"line {first.lineno}: a comment inside the import block")
            indent = " " * first.col_offset
            items = [item for _, got in run for item in got]
            self.edits.append((start, end, self.emit(items, indent)))

    def on_header(self, offset, run):
        return any(isinstance(stmt, (ast.Try, ast.If)) and self.offset(stmt.lineno, 0) <= offset < self.starts[stmt.body[0].lineno - 1]
                   for stmt, _ in run)

    # --- the sys.path line --------------------------------------------------------------------------------------
    def path_setup(self):
        body = self.tree.body
        eager = [top for top in body for run, lazy in self.runs if not lazy and any(s is top or self.inside(top, s) for s, _ in run)]
        if not self.runs:
            return
        wanted_root = self.depth
        root_stmt = None
        for stmt in body:
            if isinstance(stmt, ast.Assign) and len(stmt.targets) == 1 and ast.unparse(stmt.targets[0]) == "ROOT":
                if ROOT_EXPRS.get(ast.unparse(stmt.value)) != wanted_root:
                    raise Skip(f"line {stmt.lineno}: ROOT is not the repository root")
                root_stmt = stmt
        existing = [s for s in body if ast.unparse(s) == "sys.path.insert(0, str(ROOT))"]
        stale = []
        for stmt in body:
            if not (isinstance(stmt, ast.Expr) and ast.unparse(stmt).startswith("sys.path.")):
                continue
            if stmt in existing:
                continue
            text = ast.unparse(stmt)
            if "'tools'" in text or text.endswith("resolve().parent))") or "dirname" in text:
                if any(o is not stmt and o.lineno <= stmt.end_lineno and o.end_lineno >= stmt.lineno for o in body):
                    raise Skip(f"line {stmt.lineno}: the sys.path edit shares its line with another statement")
                stale.append(stmt)
            else:
                raise Skip(f"line {stmt.lineno}: a sys.path edit the script does not know: {text}")
        if eager:
            at = self.offset(eager[0].lineno, eager[0].col_offset)
        else:
            imports = [s for s in body if isinstance(s, (ast.Import, ast.ImportFrom))]
            at = self.starts[imports[-1].end_lineno] if imports else self.starts[body[0].lineno - 1]
        before = [s for s in body if self.starts[s.end_lineno] <= at]
        for stmt in stale:
            # a stale tools-directory line goes whole, with its line
            self.edits.append((self.starts[stmt.lineno - 1], self.starts[stmt.end_lineno], ""))
        ready_root = root_stmt is not None and self.starts[root_stmt.end_lineno] <= at
        ready_insert = any(self.starts[s.end_lineno] <= at for s in existing)
        bound = set()
        for stmt in before:
            if isinstance(stmt, (ast.Import, ast.ImportFrom)):
                for alias in stmt.names:
                    bound.add((alias.asname or alias.name).split(".")[0])
        imports = ""
        if "sys" not in bound:
            imports += "import sys\n"
        expr = ast.unparse(root_stmt.value) if root_stmt is not None else f"Path(__file__).resolve().parents[{wanted_root}]"
        if not ready_root:
            if expr.startswith("Path(") and "Path" not in bound:
                imports += "from pathlib import Path\n"
            if expr.startswith("pathlib.") and "pathlib" not in bound:
                imports += "import pathlib\n"
        lines = ""
        if not ready_root:
            if root_stmt is not None:
                self.edits.append((self.starts[root_stmt.lineno - 1], self.starts[root_stmt.end_lineno], ""))
            lines += f"ROOT = {expr}\n"
        if not ready_insert:
            for stmt in existing:
                self.edits.append((self.starts[stmt.lineno - 1], self.starts[stmt.end_lineno], ""))
            lines += "sys.path.insert(0, str(ROOT))\n"
        text = imports + ("\n" if imports and lines else "") + lines
        if not text:
            return
        if not eager:
            text = "\n" + text
            if not self.text[at:].startswith("\n"):
                text += "\n"
        self.edits.append((at, at, text))

    @staticmethod
    def inside(top, node):
        return any(child is node for child in ast.walk(top))

    def run(self):
        self.find(self.tree.body, False)
        if not self.runs:
            return None
        self.rewrite_runs()
        self.path_setup()
        out = self.text
        for start, end, text in sorted(self.edits, key=lambda e: (e[0], e[1]), reverse=True):
            out = out[:start] + text + out[end:]
        ast.parse(out)
        return out


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true", help="rewrite nothing")
    args = parser.parse_args()
    paths = sorted([*TOOLS.glob("*.py"), *(TOOLS / "ci").glob("*.py")])
    changed, skipped = 0, []
    for path in paths:
        if path.name in {"unify_imports.py", "__init__.py"}:
            continue
        unit = Unit(path)
        try:
            out = unit.run()
        except Skip as why:
            skipped.append(f"{path.relative_to(REPO)}: {why}")
            continue
        if out is None or out == unit.text:
            continue
        changed += 1
        if args.check:
            print(path.relative_to(REPO))
        else:
            path.write_text(out, encoding="utf-8")
    print(f"changed {changed} files, skipped {len(skipped)}", file=sys.stderr)
    for line in skipped:
        print(f"skipped {line}", file=sys.stderr)


if __name__ == "__main__":
    main()
