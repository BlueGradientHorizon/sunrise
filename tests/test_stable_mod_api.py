from contextlib import chdir
import hashlib
import io
import json
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from tools.ci import nightly

PREVIOUS = "26.3-0.1.10"
OLD, HEAD = "a" * 40, "b" * 40
DECLARATION = {"id": "behaviour:example", "hash": "declaration", "version": 1}
SURFACE = {"schema": 3, "hash": "surface", "behaviours": [DECLARATION]}
ADDITIVE = {**SURFACE, "behaviours": [DECLARATION, {**DECLARATION, "id": "behaviour:new"}]}
CONDITIONAL = {**SURFACE, "behaviours": [{**DECLARATION, "version": 2}]}
BREAKING = {**SURFACE, "behaviours": []}


def release(tag, *, draft=False, prerelease=False, body=""):
    return {"tag_name": tag, "draft": draft, "prerelease": prerelease, "published_at": "2026-11-20T10:00:00Z",
            "body": body}


def function(signature="signature", source="source", span=(5, 9)):
    return {"module": "module", "file": "module.rs", "span": span, "signature_hash": signature, "sha256": source}


def sdk_assets(sha, surface, content_api, functions, *, function_header=None):
    header = {"schema": 1, "build": {"commit": sha}}
    index_body = {"functions": functions, "crates": {}, "refused": {}}
    build_data = io.BytesIO()
    with tarfile.open(fileobj=build_data, mode="w:xz") as tar:
        for name, fields in (("functions", index_body | (function_header or {})), ("internals", {"internals": []}),
                             ("loops", {"loops": []})):
            data = json.dumps(header | fields).encode()
            member = tarfile.TarInfo(f"./{name}.json")
            member.size = len(data)
            tar.addfile(member, io.BytesIO(data))
    assets = {nightly.SDK_SURFACE: json.dumps(surface).encode(), nightly.SDK_BUILD_DATA: build_data.getvalue()}
    index = {"schema": 1, "build": {"commit": sha}, "content_api": content_api,
             "surface": {"previous": "c" * 40},  # A Nightly SDK is not a stable baseline.
             "assets": {name: {"sha256": hashlib.sha256(data).hexdigest(), "size": len(data)}
                        for name, data in assets.items()}}
    assets[nightly.SDK_INDEX] = json.dumps(index).encode()
    return assets


class StableModApiTests(unittest.TestCase):
    def setUp(self):
        directory = self.enterContext(tempfile.TemporaryDirectory())
        self.notes = Path(directory) / "nightly-notes.md"
        self.notes.write_text("Existing notes.\n")

    def test_same_line_accepts_only_changes_allowed_by_the_version_step(self):
        cases = (
            ("26.3-0.1.11", SURFACE, "identical", "1", None),
            ("26.3-0.1.11", ADDITIVE, "additive", "1", "patch release.*additive"),
            ("26.3-0.2.0", ADDITIVE, "additive", "1", None),
            ("26.3-0.2.0", CONDITIONAL, "conditional", "1", "minor release.*conditional"),
            ("26.3-0.2.0", BREAKING, "breaking", "1", "minor release.*breaking"),
            ("26.3-0.1.11", SURFACE, "identical", "2", "patch release.*content_api changed"),
            ("26.3-0.2.0", ADDITIVE, "additive", "2", "minor release.*content_api changed"),
        )
        for tag, surface, verdict, content_api, error in cases:
            with self.subTest(tag=tag, verdict=verdict, content_api=content_api):
                old_functions = {"body_change": function(), "signature_change": function(),
                                 "span_move": function(), "removed": function()}
                new_functions = {"body_change": function(source="new body"),
                                 "signature_change": function(signature="new signature"),
                                 "span_move": function(span=(7, 11)), "added": function()}
                assets = {OLD: sdk_assets(OLD, SURFACE, "1", old_functions),
                          HEAD: sdk_assets(HEAD, surface, content_api, new_functions)}

                def verified(sha, *names):
                    return {name: assets[sha][name] for name in names}

                def compare(args, *, check):
                    self.assertTrue(check)
                    before, after, option, report = args[args.index("surface-diff") + 1:]
                    self.assertEqual(json.loads(Path(before).read_text()), SURFACE)
                    self.assertEqual(json.loads(Path(after).read_text()), surface)
                    self.assertEqual(option, "--json")
                    Path(report).write_text(json.dumps({"verdict": verdict}))

                pages = [[release("26.3-0.1.9"), release("26.2-2.0.1"), release(tag),
                          release("26.3-0.2.0", draft=True), release("26.3-0.1.20", prerelease=True),
                          release(f"sdk-{HEAD}")], [release(PREVIOUS)]]
                tags = {tag: HEAD, PREVIOUS: OLD}
                original = "Existing notes.\n"
                self.notes.write_text(original)
                with mock.patch.object(nightly.train, "gh_json", return_value=pages), \
                        mock.patch.object(nightly.train, "origin_tags", return_value=tags), \
                        mock.patch.object(nightly, "git"), \
                        mock.patch.object(nightly, "require_stable_lineage") as lineage, \
                        mock.patch.object(nightly, "released_assets", side_effect=verified), \
                        mock.patch.object(nightly.subprocess, "run", side_effect=compare) as diff:
                    if error:
                        with self.assertRaisesRegex(SystemExit, error):
                            nightly.stable_mod_api(HEAD, tag, self.notes)
                        self.assertEqual(self.notes.read_text(), original)
                    else:
                        nightly.stable_mod_api(HEAD, tag, self.notes)
                        self.assertEqual(self.notes.read_text(), original + f"\n### Changed mixin targets\n\n"
                                         f"Compared with `{PREVIOUS}`:\n\n- Added: `added`\n"
                                         "- Changed: `body_change`\n- Removed: `removed`\n"
                                         "- Changed: `signature_change`\n")
                    lineage.assert_called_once_with(HEAD, tag, OLD, PREVIOUS)
                    diff.assert_called_once()

    def test_a_break_glass_release_keeps_its_parents_surface_judged_by_the_parents_own_stage(self):
        tag = "26.3-0.1.11"
        tags = {tag: HEAD, PREVIOUS: OLD}
        calls = []

        def compare(args, *, check):
            calls.append(args)
            Path(args[-1]).write_text(json.dumps({"verdict": "identical"}))

        for surface, error in ((SURFACE, None), ({**SURFACE, "hash": "other"}, "changes surface.json from 26.3-0.1.10")):
            assets = {OLD: sdk_assets(OLD, SURFACE, "1", {}), HEAD: sdk_assets(HEAD, surface, "1", {})}
            calls.clear()
            with self.subTest(surface=surface), \
                    mock.patch.dict(nightly.os.environ, {"BREAK_GLASS": "true"}), \
                    mock.patch.object(nightly.train, "gh_json", return_value=[[release(PREVIOUS)]]), \
                    mock.patch.object(nightly.train, "origin_tags", return_value=tags), \
                    mock.patch.object(nightly, "git") as git, \
                    mock.patch.object(nightly, "require_stable_lineage"), \
                    mock.patch.object(nightly, "released_assets", side_effect=lambda sha, *names: assets[sha]), \
                    mock.patch.object(nightly.subprocess, "run", side_effect=compare):
                if error:
                    with self.assertRaisesRegex(SystemExit, error):
                        nightly.stable_mod_api(HEAD, tag, self.notes)
                    self.assertEqual(calls, [], "refused before any stage runs")
                    continue
                nightly.stable_mod_api(HEAD, tag, self.notes)
                worktree = next(call.args for call in git.call_args_list if call.args[0] == "worktree")
                self.assertEqual((worktree[:4], worktree[-1]), (("worktree", "add", "--quiet", "--detach"), OLD))
                [args] = calls
                manifest = args[args.index("--manifest-path") + 1]
                self.assertEqual(manifest, str(Path(worktree[4]) / "Cargo.toml"), "the parent's sunrise-stage judges")
                self.assertLess(args.index("--manifest-path"), args.index("--"))

    def test_a_break_glass_release_needs_a_published_parent_on_its_line(self):
        for pages, tags, tag, error in (
                ([[]], {}, "26.3-0.1.1", "needs a published stable release to compare with"),
                ([[release(PREVIOUS)]], {PREVIOUS: OLD}, "26.3-1.0.0", "is a patch of 26.3-0.1.10, not 26.3-1.0.0")):
            with self.subTest(tag=tag), mock.patch.dict(nightly.os.environ, {"BREAK_GLASS": "true"}), \
                    mock.patch.object(nightly.train, "gh_json", return_value=pages), \
                    mock.patch.object(nightly.train, "origin_tags", return_value=tags), \
                    mock.patch.object(nightly, "git"), \
                    mock.patch.object(nightly, "released_assets") as inputs, \
                    self.assertRaisesRegex(SystemExit, error):
                nightly.stable_mod_api(HEAD, tag, self.notes)
            inputs.assert_not_called()

    def test_first_release_does_not_need_an_earlier_sdk(self):
        with mock.patch.object(nightly.train, "gh_json", return_value=[[release("26.2-0.1.3")]]), \
                mock.patch.object(nightly.train, "origin_tags", return_value={"26.2-0.1.3": OLD}), \
                mock.patch.object(nightly, "released_assets") as inputs:
            nightly.stable_mod_api(HEAD, "26.3-0.1.0", self.notes)
        inputs.assert_not_called()
        self.assertIn("No earlier stable release to compare.", self.notes.read_text())

    def test_new_major_only_compares_mixin_targets(self):
        old = sdk_assets(OLD, SURFACE, "1", {"target": function()})
        new = sdk_assets(HEAD, BREAKING, "2", {"target": function(source="new body")})
        with mock.patch.object(nightly.train, "gh_json", return_value=[[release(PREVIOUS)]]), \
                mock.patch.object(nightly.train, "origin_tags", return_value={PREVIOUS: OLD}), \
                mock.patch.object(nightly, "git"), \
                mock.patch.object(nightly, "require_stable_lineage") as lineage, \
                mock.patch.object(nightly, "released_assets", side_effect=[old, new]) as inputs, \
                mock.patch.object(nightly.subprocess, "run") as diff:
            nightly.stable_mod_api(HEAD, "26.3-1.0.0", self.notes)
        diff.assert_not_called()
        lineage.assert_not_called()
        self.assertEqual(inputs.call_args_list, [mock.call(sha, nightly.SDK_INDEX, nightly.SDK_BUILD_DATA)
                                                for sha in (OLD, HEAD)])
        self.assertIn(f"Compared with `{PREVIOUS}`:\n\n- Changed: `target`", self.notes.read_text())

    def test_previous_stable_missing_a_tag_stops_the_comparison(self):
        with mock.patch.object(nightly.train, "gh_json", return_value=[[release(PREVIOUS)]]), \
                mock.patch.object(nightly.train, "origin_tags", return_value={}):
            with self.assertRaisesRegex(SystemExit, f"the published release {PREVIOUS} has no tag"):
                nightly.stable_mod_api(HEAD, "26.3-0.1.11", self.notes)

    def test_the_parent_is_the_newest_published_release_and_an_orphan_tag_above_it_refuses(self):
        pages = [[release("26.3-0.1.9"), release(PREVIOUS, body=f"x\n{nightly.BREAK_GLASS_SECTION}\ny"),
                  release("26.3-0.2.0", draft=True), release("26.4-0.1.0"), release("26.3-0.1.11")]]
        tags = {"26.3-0.1.9": "9" * 40, PREVIOUS: OLD, "26.3-0.1.11": HEAD, "26.4-0.1.0": "4" * 40}
        with mock.patch.object(nightly.train, "gh_json", return_value=pages), mock.patch.object(nightly, "git") as git:
            with mock.patch.object(nightly.train, "origin_tags", return_value=tags):
                parent = nightly.stable_parent("26.3-0.1.11")
                self.assertEqual(parent, nightly.train.Release(PREVIOUS, OLD, "2026-11-20T10:00:00Z", True))
                git.assert_called_once_with("fetch", "--quiet", "--no-tags", "origin", OLD)
                self.assertEqual(nightly.stable_parent().tag, "26.3-0.1.11", "no bound: the newest")
                self.assertEqual(nightly.stable_parent("26.3-0.1.10").break_glass, False)
            # An aborted run's tag, or one pushed by hand, above the newest published release: no release may be
            # judged against the wrong parent. The draft has no tag and doesn't count.
            for orphan in ("26.3-0.1.12", "26.3-0.2.0", "26.3-1.0.0"):
                with self.subTest(orphan=orphan), \
                        mock.patch.object(nightly.train, "origin_tags", return_value={**tags, orphan: "e" * 40}), \
                        self.assertRaisesRegex(SystemExit, f"{orphan} is a tag with no published release above 26.3-0.1.11"):
                    nightly.stable_parent("26.3-1.1.0")
            with mock.patch.object(nightly.train, "origin_tags", return_value={"26.3-0.1.0": "e" * 40}), \
                    mock.patch.object(nightly.train, "gh_json", return_value=[[release("26.4-0.1.0")]]), \
                    self.assertRaisesRegex(SystemExit, "26.3-0.1.0 is a tag with no published release above every"):
                nightly.stable_parent("26.3-0.1.1")
            with mock.patch.object(nightly.train, "origin_tags", return_value={"26.3-0.1.0": "e" * 40}), \
                    mock.patch.object(nightly.train, "gh_json", return_value=[[]]):
                self.assertIsNone(nightly.stable_parent("26.3-0.1.0"), "the first release's own tag is not an orphan")

    def test_download_only_returns_assets_verified_under_their_sdk_tag(self):
        files = {nightly.SDK_INDEX: b"index", nightly.SDK_SURFACE: b"surface"}

        def download(*args):
            folder = Path(args[args.index("--dir") + 1])
            for name, data in files.items():
                (folder / name).write_bytes(data)
            (folder / "SHA256SUMS").write_text("".join(
                f"{hashlib.sha256(data).hexdigest()}  {name}\n" for name, data in files.items()))

        with mock.patch.object(nightly, "gh", side_effect=download), \
                mock.patch.object(nightly.subprocess, "run") as verify:
            verify.return_value = mock.Mock(returncode=0)
            self.assertEqual(nightly.released_assets(HEAD, *files), files)
            args = verify.call_args.args[0]
            self.assertEqual(args[args.index("--tag") + 1:-1],
                             [f"sdk-{HEAD}", "--only", nightly.SDK_INDEX, "--only", nightly.SDK_SURFACE])
            verify.return_value = mock.Mock(returncode=1, stderr="bad signature")
            with self.assertRaisesRegex(SystemExit, "signature does not check"):
                nightly.released_assets(HEAD, *files)

    def test_an_sdk_asset_must_match_its_index(self):
        assets = sdk_assets(OLD, SURFACE, "1", {})
        assets[nightly.SDK_SURFACE] += b"\n"
        with mock.patch.object(nightly, "released_assets", return_value=assets):
            with self.assertRaisesRegex(SystemExit, "surface.json does not match sdk.json"):
                nightly.stable_sdk_inputs(OLD, self.notes.with_name("surface.json"))

    def test_function_index_must_name_its_build_and_have_a_supported_schema(self):
        cases = (({"build": {"commit": HEAD}}, f"functions.json describes game build {HEAD}, not {OLD}"),
                 ({"schema": 2}, "functions.json has build-data schema 2; expected 1"))
        for header, error in cases:
            with self.subTest(header=header), \
                    mock.patch.object(nightly, "released_assets",
                                      return_value=sdk_assets(OLD, SURFACE, "1", {}, function_header=header)), \
                    self.assertRaisesRegex(SystemExit, error):
                nightly.stable_sdk_inputs(OLD, None)

    def test_same_line_accepts_pr_merges_of_backports_but_not_new_main_history(self):
        repo, checkout = (self.notes.with_name(name) for name in ("upstream", "checkout"))
        repo.mkdir()
        git = nightly.git
        with chdir(repo):
            git("init", "--initial-branch=main")
            for key, default in (("user.name", "Release Test"), ("user.email", "release-test@example.invalid")):
                git("config", key, git("config", "--default", default, "--get", key))
            # The base is shared; the upstream-only change makes the cherry-pick a different commit.
            git("commit", "--allow-empty", "-m", "initial")
            unrelated = git("rev-parse", "HEAD")
            git("commit", "--allow-empty", "-m", "previous stable")
            previous = git("rev-parse", "HEAD")
            git("branch", "release")
            git("commit", "--allow-empty", "-m", "main-only change")
            Path("fix").touch()
            git("add", "fix")
            git("commit", "-m", "fix")
            main = git("rev-parse", "HEAD")
            git("switch", "-c", "backport", previous)
            git("cherry-pick", main)
            git("switch", "release")
            git("merge", "--no-ff", "backport", "-m", "backport PR")
            candidate = git("rev-parse", "HEAD")
            git("switch", "-c", "imported")
            git("merge", "--no-ff", "main", "-m", "bring in main")
            imported = git("rev-parse", "HEAD")
        git("clone", "--depth", "1", "--branch", "release", repo.as_uri(), str(checkout))
        with chdir(checkout):
            tag = "26.3-0.1.11"
            nightly.require_stable_lineage(candidate, tag, previous, PREVIOUS)
            self.assertEqual(git("rev-parse", "--is-shallow-repository"), "false")
            with self.assertRaisesRegex(SystemExit, "brings in main history"):
                nightly.require_stable_lineage(imported, tag, previous, PREVIOUS)
            with self.assertRaisesRegex(SystemExit, "does not descend from"):
                nightly.require_stable_lineage(unrelated, tag, previous, PREVIOUS)

    def test_large_notes_keep_every_target_in_a_linked_file(self):
        target = "module::" + "x" * 125_000
        nightly.append_mixin_notes(self.notes, "26.3-0.1.11", PREVIOUS, {}, {target: ("signature", "source")})
        listing = self.notes.with_name("changed-mixin-targets.md")
        self.assertIn(f"- Added: `{target}`", listing.read_text())
        self.assertIn(f"/releases/download/26.3-0.1.11/{listing.name}", self.notes.read_text())
