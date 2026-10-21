"""Cross-platform QA for the untrusted profile-package contract (not app tests)."""

import json
import stat
import sys
import tempfile
import unittest
import warnings
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


def make_special_package(member_name, *, symlink=False, duplicate=False):
    handle = tempfile.NamedTemporaryFile(suffix=".swprofile", delete=False)
    handle.close()
    with zipfile.ZipFile(VALID) as source, zipfile.ZipFile(handle.name, "w") as archive:
        for entry in source.infolist():
            archive.writestr(entry, source.read(entry.filename))
        if symlink:
            entry = zipfile.ZipInfo(member_name)
            entry.create_system = 3
            entry.external_attr = (stat.S_IFLNK | 0o777) << 16
            archive.writestr(entry, b"outside-target")
        if duplicate:
            with warnings.catch_warnings():
                warnings.simplefilter("ignore", UserWarning)
                archive.writestr(member_name, b"duplicate")
    return Path(handle.name)


def make_unsupported_compression_package():
    package = make_package()
    data = bytearray(package.read_bytes())
    local_header = data.find(b"PK\x03\x04")
    central_header = data.find(b"PK\x01\x02")
    if local_header < 0 or central_header < 0:
        package.unlink(missing_ok=True)
        raise AssertionError("test fixture ZIP headers not found")
    # ZIP compression method is at local-header byte 8 and central-directory byte 10.
    data[local_header + 8:local_header + 10] = (99).to_bytes(2, "little")
    data[central_header + 10:central_header + 12] = (99).to_bytes(2, "little")
    package.write_bytes(data)
    return package


class ProfileArchiveContractTests(unittest.TestCase):
    def test_valid_fixture_is_accepted(self):
        result = validate_profile_archive(VALID)
        self.assertEqual(result["manifest.json"]["format"], "swprofile")

    def test_valid_workspace_sequence_is_accepted_and_inert(self):
        profile = json.loads(zipfile.ZipFile(VALID).read("profile.json"))
        resources = json.loads(zipfile.ZipFile(VALID).read("resources.json"))["resources"]
        resource = resources[0]
        target = resource["targets"][0]
        profile["workspace_sequences"] = [{
            "id": "398cfc89-8589-42d7-9e45-4c683c38a119",
            "name": "Morning workspace",
            "steps": [{
                "id": "fcd5981a-b4ee-4bf8-80e7-4f7cde1dd340",
                "resource_id": resource["id"],
                "target_id": target["id"],
            }],
        }]
        package = make_package(changes={"profile.json": profile})
        try:
            result = validate_profile_archive(package)
            self.assertEqual(result["profile.json"]["workspace_sequences"], profile["workspace_sequences"])
        finally:
            package.unlink(missing_ok=True)

    def test_workspace_sequence_rejects_nonmember_resource(self):
        profile = json.loads(zipfile.ZipFile(VALID).read("profile.json"))
        profile["workspace_sequences"] = [{"id": "398cfc89-8589-42d7-9e45-4c683c38a119", "name": "Bad", "steps": [{"id": "fcd5981a-b4ee-4bf8-80e7-4f7cde1dd340", "resource_id": "fd8526bf-7a24-441e-a52f-1f12af461a33"}]}]
        package = make_package(changes={"profile.json": profile})
        try:
            with self.assertRaisesRegex(ProfileArchiveError, "not a profile member"):
                validate_profile_archive(package)
        finally:
            package.unlink(missing_ok=True)

    def test_workspace_sequence_rejects_target_from_another_resource(self):
        profile = json.loads(zipfile.ZipFile(VALID).read("profile.json"))
        resources = json.loads(zipfile.ZipFile(VALID).read("resources.json"))["resources"]
        profile["workspace_sequences"] = [{"id": "398cfc89-8589-42d7-9e45-4c683c38a119", "name": "Bad target", "steps": [{"id": "fcd5981a-b4ee-4bf8-80e7-4f7cde1dd340", "resource_id": resources[0]["id"], "target_id": "c4b43a0a-bc04-412b-91ad-74cb51ee4873"}]}]
        package = make_package(changes={"profile.json": profile})
        try:
            with self.assertRaisesRegex(ProfileArchiveError, "not active on its resource"):
                validate_profile_archive(package)
        finally:
            package.unlink(missing_ok=True)

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

    def test_unsupported_zip_compression_is_normalized(self):
        package = make_unsupported_compression_package()
        try:
            with self.assertRaisesRegex(ProfileArchiveError, "could not read ZIP member"):
                validate_profile_archive(package)
        finally:
            package.unlink(missing_ok=True)

    def test_corrupted_asset_hash_is_rejected(self):
        asset_name = f"assets/icons/{'a' * 64}.png"
        manifest = json.loads(zipfile.ZipFile(VALID).read("manifest.json"))
        manifest["assets"] = [{"path": asset_name, "sha256": "b" * 64}]
        package = make_package(changes={"manifest.json": manifest}, extra={asset_name: b"synthetic icon"})
        try:
            with self.assertRaisesRegex(ProfileArchiveError, "asset checksum mismatch"):
                validate_profile_archive(package)
        finally:
            package.unlink(missing_ok=True)

    def test_symlink_member_is_rejected(self):
        asset_name = f"assets/icons/{'a' * 64}.png"
        package = make_special_package(asset_name, symlink=True)
        try:
            with self.assertRaisesRegex(ProfileArchiveError, "symbolic link member"):
                validate_profile_archive(package)
        finally:
            package.unlink(missing_ok=True)

    def test_duplicate_member_is_rejected(self):
        package = make_special_package("manifest.json", duplicate=True)
        try:
            with self.assertRaisesRegex(ProfileArchiveError, "duplicate member names"):
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
