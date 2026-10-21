#!/usr/bin/env python3
"""Readiness CONSISTENCY check only: does not establish real Windows test evidence."""
import json
import pathlib
import re
import sys

def main():
    path=pathlib.Path(sys.argv[1] if len(sys.argv)>1 else "releases/v1/readiness.json")
    obj=json.loads(path.read_text(encoding="utf-8"))
    necessary=["gate","windows_smoke","artifact_sha256","release_url","asset_url","verified_at_utc","evidence_refs"]
    missing=[k for k in necessary if k not in obj]
    if missing:
        print("FAIL: missing",missing);return 1
    if obj["gate"] != "PASS":
        print("NOT READY:",obj["gate"]);return 2
    ok=(obj["windows_smoke"]=="PASS" and bool(re.fullmatch(r"[a-fA-F0-9]{64}",obj["artifact_sha256"] or "")) and
       all(isinstance(obj.get(k),str) and obj[k].startswith("https://github.com/") for k in ["release_url","asset_url"]) and
       bool(obj["verified_at_utc"]) and bool(obj["evidence_refs"]))
    print("PASS: shape consistent; independent verification still REQUIRED" if ok else "FAIL: incomplete evidence fields")
    return 0 if ok else 1

if __name__ == "__main__":
    raise SystemExit(main())
