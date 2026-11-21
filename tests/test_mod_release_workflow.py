"""Offline asset collection and resume tests for the generated native workflow's shared steps."""

import pathlib
import sys
import tempfile
import unittest
from unittest.mock import patch


TEMPLATES = pathlib.Path(__file__).resolve().parents[1] / "crates/cargo-sunrise-mod/template/native"
sys.path.insert(0, str(TEMPLATES))
from sunrise_release import Batch, LINUX, REPO, json_write, sha, sign
from sunrise_release_github import missing_assets
sys.path.pop(0)


class Resume(unittest.TestCase):
    def test_repo_names_accept_registry_components_but_not_dot_paths(self):
        for repo in ("test/_example", "test/-example", "test/.example", f"{'a' * 39}/{'b' * 100}"):
            self.assertIsNotNone(REPO.fullmatch(repo), repo)
        for repo in ("./project", "../project", "test/.", "test/..", f"{'a' * 40}/b", f"a/{'b' * 101}"):
            self.assertIsNone(REPO.fullmatch(repo), repo)

    def test_only_missing_assets_are_uploaded_after_every_overlap_matches(self):
        rows = [
            {"name": "source.smod", "state": "uploaded", "digest": "exact source bytes"},
            {"name": "unrelated.md", "state": "uploaded", "digest": "unrelated bytes"},
        ]
        expected = {"source.smod": "exact source bytes", "source.smod.minisig": "signature bytes"}
        self.assertEqual(missing_assets(expected, rows, lambda row: row["digest"]), ["source.smod.minisig"])
        rows[0]["digest"] = "different source bytes"
        with self.assertRaises(SystemExit):
            missing_assets(expected, rows, lambda row: row["digest"])

    def test_duplicate_names_do_not_have_a_safe_resume(self):
        row = {"name": "source.smod", "state": "uploaded", "digest": "release bytes"}
        with self.assertRaises(SystemExit):
            missing_assets({"source.smod": "release bytes"}, [row, dict(row)], lambda asset: asset["digest"])


class Collection(unittest.TestCase):
    def setUp(self):
        self.work = pathlib.Path(self.enterContext(tempfile.TemporaryDirectory()))
        plan = self.work / "plan"
        plan.mkdir()
        (plan / "releases.json").write_text("[]", encoding="utf-8")
        json_write(plan / "intent.json", {"commit": "b" * 40, "repo": "test/example"})
        json_write(plan / "plan.json", {"schema": 1, "tool_commit": "a" * 40, "plan": {
            "id": "example", "author_tag": "v1.0.0", "mode": "initial", "source_asset": "example-1.0.0.smod",
            "descriptor": "example-1.0.0+26.3-0.1.0.binaries.json", "hosts": [
                {"name": LINUX, "archive": "client.tar.xz", "source_producer": True},
                {"name": f"{LINUX}-server", "archive": "server.tar.xz", "source_producer": False},
            ],
        }})
        self.batch = Batch(self.work)

    def test_an_unknown_schema_names_the_tool_that_wrote_it(self):
        json_write(self.batch.shared / "plan.json", {"schema": 7, "tool_commit": "f" * 40})
        with self.assertRaisesRegex(SystemExit, f"schema 7 from tool commit {'f' * 40}"):
            Batch(self.work)

    def test_missing_host_blocks_signing_before_writing_any_output(self):
        client = self.work / "host-results" / LINUX
        client.mkdir(parents=True)
        (client / "client.tar.xz").touch()
        (client / self.batch.source).touch()
        with patch.object(self.batch, "check_tool"), self.assertRaises(SystemExit):
            sign(self.batch, self.work)
        self.assertFalse(self.batch.signed.exists())

    def test_a_saved_batch_is_pinned_to_both_the_plan_and_each_file(self):
        batch = self.batch
        batch.assets.mkdir(parents=True)
        for name in batch.release_names():
            (batch.assets / name).write_bytes(name.encode())
        expected = {name: sha(batch.assets / name) for name in batch.release_names()}
        json_write(batch.signed / "batch.json", {"intent": batch.intent, "sha256": expected})
        self.assertEqual(batch.check_signed(), expected)
        (batch.assets / batch.source).write_bytes(b"changed bytes")
        with self.assertRaises(SystemExit):
            batch.check_signed()
        (batch.assets / batch.source).write_bytes(batch.source.encode())
        batch.intent["releases"] = "different release list"
        with self.assertRaises(SystemExit):
            batch.check_signed()


if __name__ == "__main__":
    unittest.main()
