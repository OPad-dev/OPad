#!/usr/bin/env python3
"""REUSE compliance, allowing exactly one known exception.

firmware/main/ui/easter_egg_gif.h is a third-party GIF of unknown licence,
kept deliberately and marked LicenseRef-Unknown-ThirdParty. The REUSE tool
rejects any LicenseRef containing "Unknown" by design, so plain `reuse lint`
can never pass. This runs it and fails on every other finding.

    python3 scripts/check_reuse.py      (needs `reuse` on PATH, or pipx)
"""
import json
import shutil
import subprocess
import sys

ALLOWED_BAD_LICENSES = {"LicenseRef-Unknown-ThirdParty"}

cmd = ["reuse", "lint", "--json"] if shutil.which("reuse") else ["pipx", "run", "reuse", "lint", "--json"]
report = json.loads(subprocess.run(cmd, capture_output=True, text=True).stdout)

problems = {}
for kind, items in report["non_compliant"].items():
    if kind == "bad_licenses":
        items = sorted(set(items) - ALLOWED_BAD_LICENSES)
    if items:
        problems[kind] = items

if problems:
    for kind, items in problems.items():
        print(f"{kind}:")
        for item in items:
            print(f"  {item}")
    sys.exit(1)
print(f"REUSE: {len(report['files'])} files OK (known exception: {', '.join(sorted(ALLOWED_BAD_LICENSES))})")
