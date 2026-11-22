#!/usr/bin/env python3
"""Mechanical checks for OPad docs/ (run from the repo root).

Errors (exit 1): missing frontmatter title/description, broken links,
broken <!-- src: --> paths or symbols.
Warnings: missing anchors, missing Spanish mirror, heading-count mismatch
between en and es, BOOT/RESET mentions, open TODO(verify)/VERIFY(L3) markers.
"""
import re
import sys
from pathlib import Path

REPO = Path.cwd()
DOCS = REPO / "docs"
SKIP_DIRS = {"history"}
NO_MIRROR = ("specs/", "hardware/components/", "README.md")

link_re = re.compile(r"(?<!!)\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)|!\[[^\]]*\]\(([^)\s]+)\)")
src_re = re.compile(r"<!--\s*src:\s*(.*?)\s*-->", re.S)
head_re = re.compile(r"^(#{1,6})\s+(.*?)\s*#*\s*$")
marker_re = re.compile(r"TODO\(verify\)|VERIFY\(L3\)")
bootreset_re = re.compile(r"\b(BOOT|RESET)\b")

errors, warnings = [], []
markers = 0


def slug(text):
    text = re.sub(r"`|\*|\[|\]\([^)]*\)", "", text).strip().lower()
    text = re.sub(r"[^\w\- ]", "", text, flags=re.U)
    return text.replace(" ", "-")


def strip_code(text):
    return re.sub(r"```.*?```", "", text, flags=re.S)


def pages():
    for p in sorted(DOCS.rglob("*.md")):
        rel = p.relative_to(DOCS)
        if rel.parts[0] in SKIP_DIRS:
            continue
        yield p, rel


def headings(path):
    out = []
    for line in strip_code(path.read_text(encoding="utf-8")).splitlines():
        m = head_re.match(line)
        if m:
            out.append(slug(m.group(2)))
    return out


def frontmatter(text):
    if not text.startswith("---\n"):
        return None
    end = text.find("\n---", 4)
    return text[4:end] if end != -1 else None


for path, rel in pages():
    text = path.read_text(encoding="utf-8")
    where = f"docs/{rel}"
    fm = frontmatter(text)
    if rel.name != "README.md":
        if fm is None:
            errors.append(f"{where}: no frontmatter")
        else:
            for key in ("title", "description"):
                if not re.search(rf"^{key}:\s*\S", fm, re.M):
                    errors.append(f"{where}: frontmatter has no {key}")
    body = strip_code(text)

    for m in link_re.finditer(body):
        target = m.group(1) or m.group(2)
        if re.match(r"^[a-z]+:", target):
            continue
        target_path, _, anchor = target.partition("#")
        if not target_path:
            dest = path
        elif target_path.startswith("/"):
            dest = REPO / target_path.lstrip("/")
        else:
            dest = (path.parent / target_path).resolve()
        if not dest.exists():
            errors.append(f"{where}: broken link {target}")
            continue
        if anchor and dest.suffix == ".md" and anchor.lower() not in headings(dest):
            if not anchor.startswith("L"):
                warnings.append(f"{where}: anchor #{anchor} not found in {dest.relative_to(REPO)}")

    for m in src_re.finditer(text):
        for item in m.group(1).split(","):
            item = item.strip()
            if not item:
                continue
            spath, _, symbol = item.partition("#")
            dest = REPO / spath.lstrip("/")
            if not dest.exists():
                errors.append(f"{where}: src path not found: {spath}")
            elif symbol and dest.is_file() and symbol not in dest.read_text(encoding="utf-8", errors="replace"):
                errors.append(f"{where}: symbol {symbol} not in {spath}")

    for n, line in enumerate(text.splitlines(), 1):
        if bootreset_re.search(line):
            warnings.append(f"{where}:{n}: mentions BOOT/RESET, check it never tells anyone to press them")

    count = len(marker_re.findall(text))
    if count:
        markers += count
        warnings.append(f"{where}: {count} open TODO(verify)/VERIFY(L3) marker(s)")

    is_es = rel.parts[0] == "es"
    if not is_es and not str(rel).startswith(NO_MIRROR):
        if fm and re.search(r"^draft:\s*true", fm, re.M):
            continue
        mirror = DOCS / "es" / rel
        if not mirror.exists():
            warnings.append(f"{where}: no Spanish page at docs/es/{rel}")
        elif len(headings(mirror)) != len(headings(path)):
            warnings.append(f"{where}: en has {len(headings(path))} headings, es has {len(headings(mirror))}")

for w in warnings:
    print("WARN ", w)
for e in errors:
    print("ERROR", e)
print(f"\n{len(errors)} error(s), {len(warnings)} warning(s), {markers} open marker(s)")
sys.exit(1 if errors else 0)
