"""Daily binary-level test, selected by the sunrise-mod-api-gen integration test."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest import mock

from test_stable_mod_api import ADDITIVE, BREAKING, CONDITIONAL, HEAD, OLD, PREVIOUS, SURFACE, sdk_assets
from tools.ci import nightly

CHANGES = Path("crates/sunrise-mod-vocab/changes")


class StableModApiIntegration(unittest.TestCase):
    def test_patch_and_minor_use_the_parent_stage_verdict_without_note_waivers(self):
        stage = str(Path(os.environ["SUNRISE_STAGE_TOOL"]).resolve(strict=True))
        directory = Path(self.enterContext(tempfile.TemporaryDirectory()))
        notes = directory / "release.md"
        parent = nightly.train.Release(PREVIOUS, OLD, "")
        run_stage = subprocess.run
        for verdict, candidate, passes in (
                ("identical", SURFACE, {"patch", "minor"}),
                ("additive", ADDITIVE, {"minor"}),
                ("conditional", CONDITIONAL, set()),
                ("breaking", BREAKING, set())):
            surfaces = {OLD: SURFACE, HEAD: candidate}
            assets = {sha: sdk_assets(sha, surface, "1", {}) for sha, surface in surfaces.items()}
            for step, tag in (("patch", "26.3-0.1.11"), ("minor", "26.3-0.2.0")):
                reports = []
                notes.write_text("Release notes.\n")

                def unpack(sha, tree):
                    # Substitute small schema-3 exports for the full source checkout. The differ still runs for real.
                    (tree / CHANGES).mkdir(parents=True)
                    (tree / "surface.json").write_text(json.dumps(surfaces[sha]))
                    if sha == HEAD and verdict in {"conditional", "breaking"}:
                        (tree / CHANGES / "example.toml").write_text(
                            'ids = ["behaviour:example"]\nwhy = "The engine changed."\nmigrate = "Update the mod."\n')

                def run(args, **kwargs):
                    self.assertEqual(args[0], stage)
                    if args[1] == "surface":
                        shutil.copyfile(Path(args[2]) / "surface.json", args[3])
                        return subprocess.CompletedProcess(args, 0, "", "")
                    self.assertEqual(args[1], "surface-diff")
                    if Path(args[3]).name == "after.json":
                        return run_stage(args, **kwargs)  # the signed surface, read against the candidate's export
                    result = run_stage(args, **kwargs)
                    reports.append(json.loads(Path(args[-1]).read_bytes())["verdict"])
                    if verdict in {"conditional", "breaking"}:
                        # A valid note covers the change in the normal PR gate, but not in the stable release gate.
                        work = kwargs["cwd"]
                        noted = run_stage([stage, "surface-diff", *args[2:4], "--gate",
                                           "--notes", str(work / "candidate-tree" / CHANGES),
                                           "--base-notes", str(work / "parent-tree" / CHANGES)],
                                          capture_output=True, text=True)
                        self.assertEqual(noted.returncode, 0, noted.stdout + noted.stderr)
                    return result

                with self.subTest(step=step, verdict=verdict), \
                        mock.patch.dict(os.environ, {"BREAK_GLASS": "false"}), \
                        mock.patch.object(nightly, "stable_parent", return_value=parent), \
                        mock.patch.object(nightly, "require_stable_lineage"), \
                        mock.patch.object(nightly, "require_unchanged_content_schema"), \
                        mock.patch.object(nightly, "released_assets", side_effect=lambda sha, *names: assets[sha]), \
                        mock.patch.object(nightly, "parent_stage", return_value=stage), \
                        mock.patch.object(nightly, "unpack", side_effect=unpack), \
                        mock.patch.object(nightly.subprocess, "run", side_effect=run):
                    if step in passes:
                        nightly.stable_mod_api(HEAD, tag, notes)
                        self.assertIn("No mixin targets changed.", notes.read_text())
                    else:
                        with self.assertRaisesRegex(SystemExit, f"{step} release.*mod API is {verdict}"):
                            nightly.stable_mod_api(HEAD, tag, notes)
                        self.assertEqual(notes.read_text(), "Release notes.\n")
                    self.assertEqual(reports, [verdict])

    def test_a_signed_surface_the_sdk_reader_refuses_is_refused(self):
        # Every exported record is kept, but the real reader refuses the file: a repeated id, a missing hash, a
        # schema that is no integer.
        stage = str(Path(os.environ["SUNRISE_STAGE_TOOL"]).resolve(strict=True))
        notes = Path(self.enterContext(tempfile.TemporaryDirectory())) / "release.md"
        surfaces = {OLD: SURFACE, HEAD: ADDITIVE}
        hashless = {key: value for key, value in ADDITIVE.items() if key != "hash"}
        repeated = {**ADDITIVE, "behaviours": ADDITIVE["behaviours"] + [{**ADDITIVE["behaviours"][1], "hash": "other"}]}
        run_stage = subprocess.run

        def unpack(sha, tree):
            tree.mkdir()
            (tree / "surface.json").write_text(json.dumps(surfaces[sha]))

        def run(args, **kwargs):
            if args[1] == "surface":
                shutil.copyfile(Path(args[2]) / "surface.json", args[3])
                return subprocess.CompletedProcess(args, 0, "", "")
            return run_stage(args, **kwargs)

        for signed, error in ((repeated, "lists behaviour:new twice"), (hashless, "missing field `hash`"),
                              ({**ADDITIVE, "schema": 3.0}, "floating point `3.0`")):
            assets = {OLD: sdk_assets(OLD, SURFACE, "1", {}), HEAD: sdk_assets(HEAD, signed, "1", {})}
            notes.write_text("Release notes.\n")
            with self.subTest(error=error), mock.patch.dict(os.environ, {"BREAK_GLASS": "false"}), \
                    mock.patch.object(nightly, "stable_parent", return_value=nightly.train.Release(PREVIOUS, OLD, "")), \
                    mock.patch.object(nightly, "require_stable_lineage"), \
                    mock.patch.object(nightly, "require_unchanged_content_schema"), \
                    mock.patch.object(nightly, "released_assets", side_effect=lambda sha, *names: assets[sha]), \
                    mock.patch.object(nightly, "parent_stage", return_value=stage), \
                    mock.patch.object(nightly, "unpack", side_effect=unpack), \
                    mock.patch.object(nightly.subprocess, "run", side_effect=run), \
                    self.assertRaisesRegex(SystemExit, f"cannot read its signed surface.json: .*{error}"):
                nightly.stable_mod_api(HEAD, "26.3-0.2.0", notes)
            self.assertEqual(notes.read_text(), "Release notes.\n")


if __name__ == "__main__":
    unittest.main()
