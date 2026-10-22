#!/usr/bin/env python3
"""Checks handoff artifacts, never substitutes for application tests."""
import json,sys,zipfile
from pathlib import Path
root=Path(__file__).resolve().parent.parent
required=[
"README.md","AGENTS.md","CODEX_KICKOFF.md","CODEX_RESUME.md",".codex/config.toml",
"docs/PRODUCT_REQUIREMENTS.md","docs/ARCHITECTURE.md","docs/DATA_MODEL.md",
"docs/UX_SPEC.md","docs/SECURITY.md","docs/TEST_AND_ACCEPTANCE.md",
"docs/MULTI_AGENT_WORKFLOW.md","docs/CODEX_RUNTIME_SETUP.md",
"docs/NOTIFICATIONS_AND_RELEASE_GATES.md","docs/GITHUB_SETUP.md",
"backlog/ROADMAP.md","backlog/STATUS.md","backlog/AGENT_STATUS.json",
"backlog/issues.json","releases/v1/readiness.json"]
required += [".codex/agents/"+r+".toml" for r in ["core","ui","qa","research","release"]]
required += ["contracts/"+f+".schema.json" for f in ["manifest","profile","resources","layouts","extension_manifest"]]
required += ["fixtures/valid/"+f+".json" for f in ["manifest","profile","resources","layouts","extension_manifest"]]
required += ["fixtures/valid/example.swprofile","fixtures/invalid/bad_archive.zip"]
missing=[p for p in required if not (root/p).is_file()]
if missing:
 print("FAIL: missing",missing);sys.exit(1)
j=lambda p:json.loads((root/p).read_text(encoding="utf-8"))
issues=j("backlog/issues.json")["issues"]
assert len(issues)==25 and len(set(x["id"] for x in issues))==25
assert all((root/".codex/agents"/(r+".toml")).read_text().find('gpt-6-luna')>=0 for r in ["core","ui","qa","research","release"])
assert j("releases/v1/readiness.json")["gate"]!="PASS"
with zipfile.ZipFile(root/"fixtures/valid/example.swprofile") as z:
 assert {"manifest.json","profile.json","resources.json","layouts.json"}<=set(z.namelist())
with zipfile.ZipFile(root/"fixtures/invalid/bad_archive.zip") as z:
 assert any(".." in n.split("/") for n in z.namelist())
try:
 from jsonschema import Draft202012Validator,FormatChecker
except ImportError:
 print("FAIL: pip install -r requirements-handoff.txt");sys.exit(1)
for name in ["manifest","profile","resources","layouts","extension_manifest"]:
 schema=j("contracts/"+name+".schema.json")
 validator=Draft202012Validator(schema,format_checker=FormatChecker())
 obj=j("fixtures/valid/"+name+".json")
 errors=list(validator.iter_errors(obj))
 assert not errors,(name,errors)
print("PASS: handoff file presence, 25 issue catalog, models, valid schemas and safe fixtures")
print("NOTE: no compiled app or Windows runtime tests have been executed")
