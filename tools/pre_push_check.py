#!/usr/bin/env python3
"""Pre-push & preflight checkup tool for Horizon / Sunrise PRs.

Enforces strict rules learned from recent PR failures and CI static checks:
1. Strict formatting scan (tools/ci/format_scan.py / rustfmt edition 2024 skip_children)
2. Ignored tests policy verification (tools/ci/check_ignored_test_rows.py)
3. Lexicographical byte-order check on touched ignored-tests rows
4. Production unused imports (-D unused_imports with --lib --no-default-features via dark-run)
5. Repository hygiene & public tree compliance (check_public_tree.py, check_api_boundary.py)
6. Hotspot & test-module guards (flags missing #[cfg(test)] on test modules in shared.rs)
7. Broken references check (internals.txt, ignored-tests/, docs/)
8. Test counter integrity before vs after
"""

import argparse
import os
import re
import subprocess
import sys
from pathlib import Path

def run_cmd(cmd, cwd=None, check=False):
    res = subprocess.run(cmd, cwd=cwd, shell=isinstance(cmd, str), capture_output=True, text=True)
    if check and res.returncode != 0:
        print(f"❌ Command failed: {cmd}\n{res.stderr or res.stdout}")
        sys.exit(res.returncode)
    return res

def get_repo_root():
    res = run_cmd("git rev-parse --show-toplevel")
    if res.returncode != 0:
        if Path("horizon-main").is_dir():
            return Path("horizon-main").resolve()
        return Path.cwd()
    return Path(res.stdout.strip())

def count_tests_in_text(text: str) -> int:
    return len(re.findall(r"#\[(?:[\w:]*::)?test(?:\(.*\))?\]", text))

def check_format(repo_root: Path, touched_rs_files: list[str]) -> bool:
    print("🔹 [1/7] Checking code formatting (format_scan / rustfmt edition 2024)...")
    format_scan_script = repo_root / "tools/ci/format_scan.py"
    if format_scan_script.exists():
        res = run_cmd(f"python3 {format_scan_script}", cwd=repo_root)
        if res.returncode != 0:
            print(f"❌ format_scan failed:\n{res.stderr or res.stdout[:800]}")
            return False
    else:
        res = run_cmd("cargo fmt --all -- --check", cwd=repo_root)
        if res.returncode != 0:
            print("❌ cargo fmt failed! Run 'cargo fmt' to fix.")
            return False

    # Specific CI edition 2024 skip_children check for touched game files
    game_files = [f for f in touched_rs_files if "crates/sunrise-client/src/game" in f and Path(repo_root / f).exists()]
    if game_files:
        cmd = ["rustfmt", "--edition", "2024", "--config", "skip_children=true", "--check"] + [str(repo_root / f) for f in game_files]
        res_fmt = run_cmd(cmd, cwd=repo_root)
        if res_fmt.returncode != 0:
            print(f"❌ Strict CI rustfmt (edition 2024) failed on game files: {game_files}")
            print("Run: rustfmt --edition 2024 --config skip_children=true <files>")
            return False

    print("  ✅ Formatting clean.")
    return True

def check_ignored_tests_policy(repo_root: Path, base_ref: str, touched_files: list[str]) -> bool:
    print("🔹 [2/7] Checking ignored-tests policy (check_ignored_test_rows.py & lexicographical order)...")
    check_ignored_script = repo_root / "tools/ci/check_ignored_test_rows.py"
    if check_ignored_script.exists():
        res = run_cmd(f"python3 {check_ignored_script}", cwd=repo_root)
        if res.returncode != 0:
            print(f"❌ check_ignored_test_rows.py failed:\n{res.stderr or res.stdout}")
            return False
        else:
            print(f"  {res.stdout.strip()}")

    # Strict lexicographical ordering check specifically on touched/modified lines in ignored-tests
    for path_str in touched_files:
        if path_str.startswith("tools/ci/ignored-tests/") and path_str.endswith(".txt"):
            f = repo_root / path_str
            if not f.exists():
                continue

            # Identify added or modified lines in this file compared to base_ref
            diff_lines = run_cmd(f"git diff {base_ref}..HEAD -U0 -- {path_str}", cwd=repo_root)
            added_lines = set()
            for l in diff_lines.stdout.splitlines():
                if l.startswith('+') and not l.startswith('+++'):
                    added_lines.add(l[1:].strip())

            if not added_lines:
                continue

            lines = [line.strip() for line in f.read_text(errors='ignore').splitlines() if line.strip() and not line.startswith('#')]
            for i in range(len(lines) - 1):
                if lines[i] in added_lines or lines[i + 1] in added_lines:
                    if lines[i] > lines[i + 1]:
                        print(f"❌ Ignored tests order violation in {f.name} around row {i+1}:")
                        print(f"   '{lines[i]}'")
                        print(f"   > appears before:")
                        print(f"   '{lines[i+1]}'")
                        return False

    print("  ✅ Ignored tests policy and sorting are 100% compliant.")
    return True

def check_unused_imports(repo_root: Path, touched_crates: set[str]) -> bool:
    print(f"🔹 [3/7] Checking production build unused imports on touched crates (-D unused_imports)...")
    dark_run_cmd = os.path.expanduser("~/bin/dark-run") if os.path.exists(os.path.expanduser("~/bin/dark-run")) else "dark-run"
    packages = " ".join(f"-p {crate}" for crate in sorted(touched_crates))
    print(f"  -> checking production build on dark: {packages}...")
    remote_cmd = f"RUSTFLAGS='-D unused_imports' cargo check {packages} --lib --no-default-features"
    res = run_cmd(f"{dark_run_cmd} \"{remote_cmd}\"", cwd=repo_root)
    if res.returncode != 0:
        print(f"❌ Production unused imports detected in {packages}:\n{res.stderr or res.stdout}")
        return False

    print("  ✅ Production imports clean.")
    return True

def check_test_module_annotations(repo_root: Path, touched_rs_files: list[str]) -> bool:
    print("🔹 [4/7] Checking test module annotations (#[cfg(test)]) in touched files (shared.rs, remote/mod.rs, etc.)...")
    # Targets for modularization: shared.rs, remote/mod.rs, or any touched file declaring submodules
    for rel_path in touched_rs_files:
        f_path = repo_root / rel_path
        if not f_path.exists():
            continue

        content = f_path.read_text(errors='ignore')
        lines = content.splitlines()
        in_cfg_test_scope = 0
        for idx, line in enumerate(lines):
            line_str = line.strip()
            # Track block scopes of mod tests / cfg(test)
            if "#[cfg(test)]" in line_str:
                in_cfg_test_scope = 2  # next line or block is under cfg(test)
            elif "mod tests {" in line_str and in_cfg_test_scope > 0:
                in_cfg_test_scope = 50000  # nested inside mod tests block

            # Match module declarations like: mod foo_tests; or pub(super) mod bar_tests;
            if re.match(r'^(pub(\([a-z_]+\))?\s+)?mod\s+[a-zA-Z0-9_]+_tests(;|\s*\{)', line_str):
                # If we're already inside a mod tests block that is cfg(test), nested mods inherit it
                if in_cfg_test_scope > 100:
                    continue

                # Look at previous lines for #[cfg(test)]
                has_cfg_test = False
                for back in range(max(0, idx - 4), idx):
                    if "#[cfg(test)]" in lines[back]:
                        has_cfg_test = True
                        break
                if not has_cfg_test:
                    print(f"❌ Test module declaration '{line_str}' in {rel_path} (L. {idx+1}) is missing #[cfg(test)] annotation!")
                    return False

            if in_cfg_test_scope in (1, 2):
                in_cfg_test_scope -= 1

    print("  ✅ Test module annotations clean across touched files.")
    return True

def check_repository_hygiene(repo_root: Path) -> bool:
    print("🔹 [5/7] Checking repository hygiene and public tree...")
    check_tree = repo_root / "tools/check_public_tree.py"
    if check_tree.exists():
        res = run_cmd(f"python3 {check_tree}", cwd=repo_root)
        if res.returncode != 0:
            print(f"❌ check_public_tree.py failed:\n{res.stderr or res.stdout}")
            return False

    check_api = repo_root / "tools/ci/check_api_boundary.py"
    if check_api.exists():
        res = run_cmd(f"python3 {check_api} --source-only", cwd=repo_root)
        if res.returncode != 0:
            print(f"❌ check_api_boundary.py failed:\n{res.stderr or res.stdout}")
            return False

    print("  ✅ Hygiene and tree checks passed.")
    return True

def check_orphans(repo_root: Path, base_ref: str, touched_files: list[str]) -> bool:
    print("🔹 [6/7] Checking for broken references (internals.txt, ignored-tests, docs)...")
    diff_res = run_cmd(f"git diff --name-status {base_ref}..HEAD", cwd=repo_root)
    moved_or_deleted = []
    for line in diff_res.stdout.splitlines():
        parts = line.split()
        if not parts:
            continue
        status = parts[0]
        if status.startswith('D'):
            moved_or_deleted.append(parts[1])
        elif status.startswith('R'):
            moved_or_deleted.append(parts[1])

    internals_file = repo_root / "crates/sunrise-mod-api-gen/internals.txt"
    ignored_tests_dir = repo_root / "tools/ci/ignored-tests"

    issues = []
    for old_path in moved_or_deleted:
        stem = Path(old_path).stem
        if internals_file.exists():
            text = internals_file.read_text(errors='ignore')
            if stem in text or old_path in text:
                issues.append(f"Old path/stem '{stem}' found in {internals_file}")

        if ignored_tests_dir.exists():
            for f in ignored_tests_dir.glob("*.txt"):
                text = f.read_text(errors='ignore')
                if old_path in text:
                    issues.append(f"Old path '{old_path}' found in {f}")

    if issues:
        print("❌ Broken references detected!")
        for issue in issues:
            print(f"  - {issue}")
        return False

    print("  ✅ No broken references found.")
    return True

def check_test_count(repo_root: Path, base_ref: str, touched_rs_files: list[str]) -> bool:
    print("🔹 [7/7] Verifying test count integrity before/after...")
    for rel_path in touched_rs_files:
        p = repo_root / rel_path
        if not p.exists():
            continue
        
        current_text = p.read_text(errors='ignore')
        current_count = count_tests_in_text(current_text)

        base_content_res = run_cmd(f"git show {base_ref}:{rel_path}", cwd=repo_root)
        if base_content_res.returncode == 0:
            base_count = count_tests_in_text(base_content_res.stdout)
            diff = current_count - base_count
            diff_str = f"+{diff}" if diff > 0 else str(diff)
            print(f"  {rel_path}: {base_count} -> {current_count} tests ({diff_str})")
        else:
            print(f"  {rel_path}: new file with {current_count} tests")

    return True

def main():
    parser = argparse.ArgumentParser(description="Horizon / Sunrise Pre-push Checkup Tool (Ultra-Strict)")
    parser.add_argument("--base", default="HEAD~1", help="Base commit to compare against (default: HEAD~1)")
    parser.add_argument("--skip-clippy", action="store_true", help="Skip clippy linting")
    args = parser.parse_args()

    repo_root = get_repo_root()
    print(f"🔍 Running pre-push checks in {repo_root} (against {args.base})...\n")

    diff_res = run_cmd(f"git diff --name-only {args.base}..HEAD", cwd=repo_root)
    touched_files = [line.strip() for line in diff_res.stdout.splitlines() if line.strip()]
    touched_rs = [f for f in touched_files if f.endswith(".rs")]

    touched_crates = set()
    for f in touched_files:
        if f.startswith("crates/"):
            parts = f.split("/")
            if len(parts) >= 2:
                touched_crates.add(parts[1])

    success = True
    if not check_format(repo_root, touched_rs):
        success = False

    if not check_ignored_tests_policy(repo_root, args.base, touched_files):
        success = False

    if not check_unused_imports(repo_root, touched_crates):
        success = False

    if not check_test_module_annotations(repo_root, touched_rs):
        success = False

    if not check_repository_hygiene(repo_root):
        success = False

    if not check_orphans(repo_root, args.base, touched_files):
        success = False

    check_test_count(repo_root, args.base, touched_rs)

    print("\n" + "="*60)
    if success:
        print("🎉 ALL PRE-PUSH CHECKS PASSED! Ready to push.")
        print("💡 Reminder: If this PR is stacked, ensure base branch matches previous PR.")
        print("💡 Reminder: Do NOT push to a PR that is already in a rollup or approved.")
        sys.exit(0)
    else:
        print("⛔ CHECKS FAILED! Fix the errors above before pushing.")
        sys.exit(1)

if __name__ == "__main__":
    main()
