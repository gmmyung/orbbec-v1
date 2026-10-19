"""Exercise release decisions without credentials or writes to crates.io."""
import hashlib
import importlib.util
import io
import os
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import urllib.error

spec = importlib.util.spec_from_file_location("publisher", Path(__file__).resolve().parents[1] / "scripts/publish-crates.py")
publisher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publisher)


def archive(path, content, vcs=b"commit one"):
    path.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(path, "w:gz") as tar:
        for name, data in [("src/lib.rs", content), (".cargo_vcs_info.json", vcs)]:
            info = tarfile.TarInfo(f"{path.stem}/{name}")
            info.size = len(data)
            tar.addfile(info, io.BytesIO(data))


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.target = Path(self.tmp.name)
        self.packages = {name: {"version": "0.1.0"} for name in publisher.ORDER}
        for name in publisher.ORDER:
            archive(self.target / "package" / f"{name}-0.1.0.crate", b"same library")

    def test_missing_versions_follow_dependency_order_and_need_a_token(self):
        with patch.object(publisher, "registry_version", return_value=None):
            plan = publisher.build_plan(self.packages, self.target)
        self.assertEqual([p["name"] for p in plan], list(publisher.ORDER))
        with patch.dict(os.environ, {}, clear=True), patch.object(publisher.subprocess, "run") as run:
            with self.assertRaisesRegex(ValueError, "No publish token"):
                publisher.publish(plan, "cargo")
            run.assert_not_called()

    def test_registry_errors_are_not_treated_as_new_crates(self):
        error = urllib.error.HTTPError("https://crates.io", 503, "unavailable", {}, None)
        with patch.object(publisher, "fetch", side_effect=error):
            with self.assertRaises(urllib.error.HTTPError):
                publisher.registry_version("orbbec-v1", "0.1.0")

    def test_matching_payload_with_different_commit_can_be_skipped(self):
        remote = self.target / "remote.crate"
        archive(remote, b"same library", vcs=b"commit two")
        data = remote.read_bytes()
        existing = {"yanked": False, "checksum": hashlib.sha256(data).hexdigest()}
        with patch.object(publisher, "registry_version", return_value=existing), patch.object(publisher, "fetch", return_value=data):
            plan = publisher.build_plan(self.packages, self.target)
        self.assertTrue(all(p["action"] == "skip-matching-version" for p in plan))

    def test_changed_published_payload_aborts_before_uploading(self):
        remote = self.target / "remote.crate"
        archive(remote, b"different library")
        data = remote.read_bytes()
        existing = {"yanked": False, "checksum": hashlib.sha256(data).hexdigest()}
        with patch.object(publisher, "registry_version", return_value=existing), patch.object(publisher, "fetch", return_value=data), patch.object(publisher.subprocess, "run") as run:
            with self.assertRaisesRegex(ValueError, "different contents"):
                publisher.build_plan(self.packages, self.target)
            run.assert_not_called()

    def test_publish_passes_credentials_only_through_the_environment(self):
        plan = [{"name": name, "action": "publish"} for name in publisher.ORDER]
        with patch.dict(os.environ, {"CARGO_REGISTRY_TOKEN": "test-only-placeholder"}), patch.object(publisher.subprocess, "run") as run:
            publisher.publish(plan, "cargo")
        self.assertEqual([c.args[0][-1] for c in run.call_args_list], list(publisher.ORDER))
        for call in run.call_args_list:
            self.assertNotIn("--token", call.args[0])
            self.assertNotIn("test-only-placeholder", call.args[0])


if __name__ == "__main__":
    unittest.main()
