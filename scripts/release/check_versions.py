#!/usr/bin/env python3
"""Checks that every place holding a release version agrees.

The app version lives in desktop/Cargo.toml ([workspace.package]) and in the
Cargo.lock entry of every workspace crate; the firmware version in
firmware/CMakeLists.txt (PROJECT_VER). release-please bumps the first and
the last through release-please-config.json and records them in
.release-please-manifest.json; release-please.yml then brings Cargo.lock to
the new version on the app's release PR (`cargo update -w`). A lock left
behind would make `cargo build --locked` fail at release time; this fails
first.

    check_versions.py               check the repository
    check_versions.py --tag TAG     also check that TAG (v1.2.0 or
                                    firmware-v1.2.0) is the version in the tree,
                                    and print which component it releases
"""

import argparse
import json
import os
import re
import sys
import tomllib

REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
DESKTOP = os.path.join(REPO_ROOT, "desktop")

# https://semver.org/#is-there-a-suggested-regular-expression-regex-to-check-a-semver-string
SEMVER = re.compile(
    r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)"
    r"(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?"
    r"(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$"
)
# esp_app_desc_t.version is char[32], NUL included
FIRMWARE_VERSION_MAX = 31


def load_toml(path):
    with open(path, "rb") as f:
        return tomllib.load(f)


def app_version():
    return load_toml(os.path.join(DESKTOP, "Cargo.toml"))["workspace"]["package"]["version"]


def firmware_version():
    with open(os.path.join(REPO_ROOT, "firmware", "CMakeLists.txt"), encoding="utf-8") as f:
        m = re.search(r'^set\(PROJECT_VER "([^"]*)"\)', f.read(), re.MULTILINE)
    return m.group(1) if m else None


def workspace_crates():
    members = load_toml(os.path.join(DESKTOP, "Cargo.toml"))["workspace"]["members"]
    return {load_toml(os.path.join(DESKTOP, m, "Cargo.toml"))["package"]["name"] for m in members}


def lock_versions(names):
    lock = load_toml(os.path.join(DESKTOP, "Cargo.lock"))
    return {p["name"]: p["version"] for p in lock["package"] if p["name"] in names and "source" not in p}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--tag", help="a release tag to check against the tree")
    args = parser.parse_args()

    errors = []
    app = app_version()
    fw = firmware_version()

    if not SEMVER.match(app):
        errors.append(f"desktop/Cargo.toml: app version {app!r} is not SemVer")
    if fw is None:
        errors.append('firmware/CMakeLists.txt: no set(PROJECT_VER "...") line')
    else:
        if not SEMVER.match(fw):
            errors.append(f"firmware/CMakeLists.txt: firmware version {fw!r} is not SemVer")
        if len(fw) > FIRMWARE_VERSION_MAX:
            errors.append(f"firmware version {fw!r} is longer than {FIRMWARE_VERSION_MAX} characters (esp_app_desc_t)")

    with open(os.path.join(REPO_ROOT, ".release-please-manifest.json"), encoding="utf-8") as f:
        manifest = json.load(f)
    if manifest.get(".") != app:
        errors.append(f".release-please-manifest.json: app is {manifest.get('.')!r}, desktop/Cargo.toml says {app!r}")
    if manifest.get("firmware") != fw:
        errors.append(f".release-please-manifest.json: firmware is {manifest.get('firmware')!r}, CMakeLists.txt says {fw!r}")

    crates = workspace_crates()
    for name, version in sorted(lock_versions(crates).items()):
        if version != app:
            errors.append(f"desktop/Cargo.lock: {name} is {version}, the workspace is {app}")

    component = None
    if args.tag:
        if args.tag.startswith("firmware-v"):
            component, expected = "firmware", fw
            version = args.tag[len("firmware-v"):]
        elif args.tag.startswith("v"):
            component, expected = "app", app
            version = args.tag[1:]
        else:
            errors.append(f"tag {args.tag!r} is neither vX.Y.Z (app) nor firmware-vX.Y.Z")
            version = expected = None
        if component and version != expected:
            errors.append(f"tag {args.tag} releases {component} {version}, but the tree is at {expected}")

    if errors:
        for e in errors:
            print(f"ERROR: {e}", file=sys.stderr)
        return 1
    print(f"Versions OK: app {app}, firmware {fw}, {len(crates)} workspace crates")
    if component:
        print(f"component={component}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
