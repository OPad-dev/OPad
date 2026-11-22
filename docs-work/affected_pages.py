#!/usr/bin/env python3
"""List the docs pages that cite files changed since a commit.

    python3 docs-work/affected_pages.py <base> [<head>]

Run from the repo root. A page is affected when one of its
<!-- src: --> comments or repo-root links (/path/to/file) names a file
that changed between <base> and <head> (default: HEAD). docs/history/
is ignored. Output: one line per page, with the changed files it cites.
"""
import re
import subprocess
import sys
from pathlib import Path

if len(sys.argv) not in (2, 3):
    sys.exit(__doc__)
base = sys.argv[1]
head = sys.argv[2] if len(sys.argv) == 3 else "HEAD"

changed = set(
    subprocess.run(
        ["git", "diff", "--name-only", base, head, "--", ".", ":!docs"],
        check=True, capture_output=True, text=True,
    ).stdout.split()
)

cite_re = re.compile(r"<!--\s*src:\s*(.*?)\s*-->|\]\((/[^)\s#]+)", re.S)
hits = {}
for page in sorted(Path("docs").rglob("*.md")):
    if page.parts[1] == "history":
        continue
    cited = set()
    for m in cite_re.finditer(page.read_text(encoding="utf-8")):
        items = m.group(1).split(",") if m.group(1) else [m.group(2)]
        for item in items:
            path = item.strip().partition("#")[0].lstrip("/").rstrip("/")
            if not path:
                continue
            cited |= {f for f in changed if f == path or f.startswith(path + "/")}
    if cited:
        hits[str(page)] = sorted(cited)

for page, files in hits.items():
    print(f"{page}: {', '.join(files)}")
print(f"\n{len(changed)} changed file(s), {len(hits)} affected page(s)", file=sys.stderr)
