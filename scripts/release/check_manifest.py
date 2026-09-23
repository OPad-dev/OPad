#!/usr/bin/env python3
"""Checks a signed release manifest against the files a release publishes.

Every artifact the manifest lists must be in the release directory, with the
sha256 (and size, when recorded) the manifest gives, and in SHA256SUMS when
that file is present. A manifest built over anything else (a local rebuild, a
dist/ that lost its packages) fails here instead of at the user's updater.

    check_manifest.py <release-dir> [--verify-signature] [--public-key KEY]

--verify-signature also checks opad-manifest.json.minisig with minisign,
against MANIFEST_PUBLIC_KEY from desktop/crates/opad-update/src/verify.rs
unless --public-key is given.
"""

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys

MANIFEST = "opad-manifest.json"
REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
VERIFY_RS = os.path.join(REPO_ROOT, "desktop", "crates", "opad-update", "src", "verify.rs")


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def compiled_public_key():
    with open(VERIFY_RS, encoding="utf-8") as f:
        m = re.search(r'MANIFEST_PUBLIC_KEY: &str = "([^"]+)"', f.read())
    if not m:
        sys.exit(f"check_manifest: MANIFEST_PUBLIC_KEY not found in {VERIFY_RS}")
    return m.group(1)


def read_sums(path):
    sums = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            parts = line.split()
            if len(parts) == 2:
                sums[parts[1].lstrip("*")] = parts[0].lower()
    return sums


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("release_dir")
    ap.add_argument("--verify-signature", action="store_true")
    ap.add_argument("--public-key", help="minisign public key (default: the one compiled into the app)")
    args = ap.parse_args()

    manifest_path = os.path.join(args.release_dir, MANIFEST)
    with open(manifest_path, "rb") as f:
        manifest = json.loads(f.read())

    problems = []
    if args.verify_signature:
        key = args.public_key or compiled_public_key()
        result = subprocess.run(
            ["minisign", "-V", "-q", "-P", key, "-m", manifest_path],
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            problems.append(f"{MANIFEST}: signature does not verify: {result.stderr.strip()}")

    sums_path = os.path.join(args.release_dir, "SHA256SUMS")
    sums = read_sums(sums_path) if os.path.exists(sums_path) else None

    checked = 0
    for name, component in sorted(manifest.get("components", {}).items()):
        for artifact in component.get("artifacts", []):
            file_name = artifact["url"].rstrip("/").rsplit("/", 1)[-1]
            label = f"{name} {artifact.get('target')}/{artifact.get('kind')}: {file_name}"
            path = os.path.join(args.release_dir, file_name)
            if not os.path.isfile(path):
                problems.append(f"{label}: not in the release")
                continue
            expected = artifact["sha256"].strip().lower()
            actual = sha256(path)
            if actual != expected:
                problems.append(f"{label}: sha256 {actual}, manifest says {expected}")
            size = artifact.get("size")
            if size is not None and os.path.getsize(path) != size:
                problems.append(f"{label}: {os.path.getsize(path)} bytes, manifest says {size}")
            if sums is not None and sums.get(file_name) != expected:
                problems.append(f"{label}: SHA256SUMS says {sums.get(file_name)}")
            checked += 1

    if checked == 0:
        problems.append(f"{MANIFEST} lists no artifacts")

    if problems:
        for p in problems:
            print(f"✗ {p}", file=sys.stderr)
        sys.exit(1)
    print(f"✓ {checked} manifest artifact(s) match the release files"
          + (" and the signature verifies" if args.verify_signature else ""))


if __name__ == "__main__":
    main()
