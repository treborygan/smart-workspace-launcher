"""Strict, non-extracting validator for untrusted .swprofile ZIP files.

This small Python utility is for fixture/contract QA. The production Rust importer
must enforce the same limits itself; this module is not the application importer.
"""

from __future__ import annotations

import json
import hashlib
import re
import stat
import zlib
import zipfile
from pathlib import Path, PurePosixPath
from typing import Any

from jsonschema import Draft202012Validator, FormatChecker


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_DIR = ROOT / "contracts"
MAX_ARCHIVE_BYTES = 32 * 1024 * 1024
MAX_TOTAL_UNCOMPRESSED = 128 * 1024 * 1024
MAX_ENTRY_UNCOMPRESSED = 32 * 1024 * 1024
MAX_COMPRESSION_RATIO = 100
MAX_ENTRIES = 2503  # three JSON documents, manifest, and at most 2,500 icons
REQUIRED_JSON = {"manifest.json", "profile.json", "resources.json", "layouts.json"}
EXECUTABLE_SUFFIXES = {
    ".bat", ".cmd", ".com", ".dll", ".exe", ".js", ".msi", ".ps1",
    ".py", ".scr", ".sh", ".vbs", ".wsf",
}


class ProfileArchiveError(ValueError):
    """The archive is unsafe or does not conform to the profile contract."""


def _schema(name: str) -> dict[str, Any]:
    return json.loads((SCHEMA_DIR / name).read_text(encoding="utf-8"))


def _safe_member_name(name: str) -> bool:
    if not name or "\\" in name or name.startswith("/") or re.match(r"^[A-Za-z]:", name):
        return False
    path = PurePosixPath(name)
    return not path.is_absolute() and all(part not in {"", ".", ".."} for part in path.parts)


def _read_member(archive: zipfile.ZipFile, name: str) -> bytes:
    try:
        return archive.read(name)
    except (KeyError, OSError, RuntimeError, EOFError, zlib.error, NotImplementedError, zipfile.BadZipFile) as exc:
        raise ProfileArchiveError(f"could not read ZIP member: {name}") from exc


def _read_json(archive: zipfile.ZipFile, name: str) -> Any:
    try:
        return json.loads(_read_member(archive, name).decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ProfileArchiveError(f"{name} is not valid UTF-8 JSON") from exc


def _validate(schema_name: str, value: Any) -> None:
    validator = Draft202012Validator(_schema(schema_name), format_checker=FormatChecker())
    errors = sorted(validator.iter_errors(value), key=lambda error: list(map(str, error.path)))
    if errors:
        error = errors[0]
        location = "/".join(map(str, error.path)) or "root"
        raise ProfileArchiveError(f"{schema_name}: {location}: {error.message}")


def validate_profile_archive(path: str | Path) -> dict[str, Any]:
    """Validate a package without extracting it and return its validated JSON.

    The result is suitable for an import preview only. Applying changes still needs
    a transaction, collision policy, rollback and user confirmation in the app.
    """
    path = Path(path)
    if path.stat().st_size > MAX_ARCHIVE_BYTES:
        raise ProfileArchiveError("archive exceeds compressed size limit")
    try:
        archive = zipfile.ZipFile(path)
    except (OSError, zipfile.BadZipFile) as exc:
        raise ProfileArchiveError("not a valid ZIP archive") from exc

    with archive:
        entries = archive.infolist()
        if len(entries) > MAX_ENTRIES:
            raise ProfileArchiveError("archive contains too many entries")
        names = [entry.filename for entry in entries]
        if len(names) != len(set(names)):
            raise ProfileArchiveError("archive contains duplicate member names")
        total_size = 0
        for entry in entries:
            if not _safe_member_name(entry.filename):
                raise ProfileArchiveError(f"unsafe member path: {entry.filename!r}")
            mode = entry.external_attr >> 16
            if stat.S_ISLNK(mode):
                raise ProfileArchiveError(f"symbolic link member is not allowed: {entry.filename}")
            if Path(entry.filename).suffix.lower() in EXECUTABLE_SUFFIXES:
                raise ProfileArchiveError(f"executable content is not allowed: {entry.filename}")
            if entry.flag_bits & 0x1:
                raise ProfileArchiveError("encrypted ZIP members are not allowed")
            if entry.file_size > MAX_ENTRY_UNCOMPRESSED:
                raise ProfileArchiveError(f"member exceeds uncompressed size limit: {entry.filename}")
            total_size += entry.file_size
            if total_size > MAX_TOTAL_UNCOMPRESSED:
                raise ProfileArchiveError("archive exceeds total uncompressed size limit")
            if entry.file_size and entry.compress_size == 0:
                raise ProfileArchiveError(f"invalid compression size: {entry.filename}")
            if entry.compress_size and entry.file_size / entry.compress_size > MAX_COMPRESSION_RATIO:
                raise ProfileArchiveError(f"compression ratio exceeds limit: {entry.filename}")

        if not REQUIRED_JSON.issubset(names):
            raise ProfileArchiveError("archive is missing a required JSON document")
        unexpected = set(names) - REQUIRED_JSON
        for name in unexpected:
            if not re.fullmatch(r"assets/icons/[a-f0-9]{64}\.(?:png|ico)", name):
                raise ProfileArchiveError(f"unexpected archive member: {name}")

        docs = {name: _read_json(archive, name) for name in REQUIRED_JSON}
        _validate("manifest.schema.json", docs["manifest.json"])
        _validate("profile.schema.json", docs["profile.json"])
        _validate("resources.schema.json", docs["resources.json"])
        _validate("layouts.schema.json", docs["layouts.json"])

        resources = docs["resources.json"]["resources"]
        resource_ids = [resource["id"] for resource in resources]
        target_ids = [target["id"] for resource in resources for target in resource["targets"]]
        if len(resource_ids) != len(set(resource_ids)) or len(target_ids) != len(set(target_ids)):
            raise ProfileArchiveError("resource and target IDs must be unique")
        known_resource_ids = set(resource_ids)
        if not set(docs["profile.json"]["memberships"]).issubset(known_resource_ids):
            raise ProfileArchiveError("profile membership refers to an unknown resource")
        profile = docs["profile.json"]
        sequence_ids: set[str] = set()
        sequence_step_ids: set[str] = set()
        sequence_names: set[str] = set()
        targets_by_id = {
            target["id"]: (resource["id"], target.get("active", True))
            for resource in resources
            for target in resource["targets"]
        }
        for sequence in profile.get("workspace_sequences", []):
            if sequence["id"] in sequence_ids:
                raise ProfileArchiveError("workspace sequence IDs must be unique")
            sequence_ids.add(sequence["id"])
            folded_name = sequence["name"].casefold()
            if folded_name in sequence_names:
                raise ProfileArchiveError("workspace sequence names must be unique")
            sequence_names.add(folded_name)
            for step in sequence["steps"]:
                if step["id"] in sequence_step_ids:
                    raise ProfileArchiveError("workspace sequence step IDs must be unique")
                sequence_step_ids.add(step["id"])
                if step["resource_id"] not in profile["memberships"]:
                    raise ProfileArchiveError("workspace sequence step resource is not a profile member")
                if "target_id" in step:
                    target = targets_by_id.get(step["target_id"])
                    if target is None or target[0] != step["resource_id"] or not target[1]:
                        raise ProfileArchiveError("workspace sequence step target is not active on its resource")

        for layout in docs["layouts.json"]["layouts"]:
            if any(card["resource_id"] not in known_resource_ids for card in layout["cards"]):
                raise ProfileArchiveError("layout refers to an unknown resource")
            if any(card["x"] + card["w"] > 12 for card in layout["cards"]):
                raise ProfileArchiveError("layout card exceeds the 12-column grid")
        for resource in resources:
            for target in resource["targets"]:
                if target["kind"] == "url" and not target["value"].lower().startswith("https://"):
                    raise ProfileArchiveError("only HTTPS URL targets are allowed")

        manifest_assets = {asset["path"]: asset["sha256"] for asset in docs["manifest.json"].get("assets", [])}
        archive_assets = set(names) - REQUIRED_JSON
        if archive_assets != set(manifest_assets):
            raise ProfileArchiveError("manifest asset list does not match archive contents")
        for name, expected_hash in manifest_assets.items():
            if hashlib.sha256(_read_member(archive, name)).hexdigest() != expected_hash:
                raise ProfileArchiveError(f"asset checksum mismatch: {name}")

        return docs
