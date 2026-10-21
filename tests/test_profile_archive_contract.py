"""Cross-platform QA for the untrusted profile-package contract (not app tests)."""

import json
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts" / "qa"))
from profile_archive import ProfileArchiveError, validate_profile_archive  # noqa: E402


VALID = ROOT / "fixtures" / "valid" / "example.swprofile"
BAD_TRAVERSAL = ROOT / "fixtures" / "invalid" / "bad_archive.zip"


def make_package(changes=None, extra=None):
    with zipfile.ZipFile(VALID) as source:
        entries = {name: source.read(name) for name in source.namelist()}
    for name, value in (changes or {}).items():
        entries[name] = json.dumps(value).encode()
    entries.update(extra or {})
    handle = tempfile.NamedTemporaryFile(suffix=".swprofile", delete=False)
    handle.close()
    with zipfile.ZipFile(handle.name, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for name, value in entries.items():
            archive.writestr(name, value)
    return Path(handle.name)


class ProfileArchiveContractTests(unittest.TestCase):
    def test_valid_fixture_is_accepted(self):
        result = validate_profile_archive(VALID)
        self.assertEqual(result["manifest.json"]["format"], "swprofile")

    def test_checked_in_zip_slip_fixture_is_rejected(self):
        with self.assertRaisesRegex(ProfileArchiveError, "unsafe member path"):
            validate_profile_archive(BAD_TRAVERSAL)

    def test_invalid_layout_contract_is_rejected(self):
        package = make_package(changes={
            "layouts.json": {
                "schema_version": 1,
                "layouts": [{"breakpoint": "desktop", "cards": [{
                    "resource_id": "927e0560-6ef3-452f-a6be-5ad7a5683b6c",
                    "x": 11, "y": 0, "w": 2, "h": 1,
                }]}],
            }
        })
        try:
            with self.assertRaisesRegex(ProfileArchiveError, "12-column grid"):
                validate_profile_archive(package)
        finally:
            package.unlink(missing_ok=True)

    def test_javascript_target_is_rejected_by_domain_policy(self):
        resources = json.loads(zipfile.ZipFile(VALID).read("resources.json"))
        resources["resources"][0]["type"] = "url"
        resources["resources"][0]["targets"][0].update(kind="url", value="javascript:alert(1)")
        package = make_package(changes={"resources.json": resources})
        try:
            with self.assertRaisesRegex(ProfileArchiveError, "only HTTPS"):
                validate_profile_archive(package)
        finally:
            package.unlink(missing_ok=True)

    def test_unlisted_executable_member_is_rejected(self):
        package = make_package(extra={"payload.ps1": b"Write-Output unsafe"})
        try:
            with self.assertRaisesRegex(ProfileArchiveError, "executable content"):
                validate_profile_archive(package)
        finally:
            package.unlink(missing_ok=True)

    def test_unknown_non_executable_member_is_rejected(self):
        package = make_package(extra={"notes.txt": b"unexpected"})
        try:
            with self.assertRaisesRegex(ProfileArchiveError, "unexpected archive member"):
                validate_profile_archive(package)
        finally:
            package.unlink(missing_ok=True)

    def test_bad_zip_is_rejected(self):
        with tempfile.NamedTemporaryFile(suffix=".swprofile") as package:
            package.write(b"not a zip")
            package.flush()
            with self.assertRaisesRegex(ProfileArchiveError, "not a valid ZIP"):
                validate_profile_archive(package.name)


if __name__ == "__main__":
    unittest.main()
