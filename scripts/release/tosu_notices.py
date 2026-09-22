#!/usr/bin/env python3
"""Write THIRD_PARTY_NOTICES.txt for the bundled tosu.

The standalone tosu binary is tosu's own code plus its npm production
dependencies plus a Node.js 24 runtime (with the OpenSSL, ICU, libuv, V8, ...
that Node embeds). Each of those licences asks for its notice to ship with the
binary, and the set changes with every tosu version, so the file is generated
from the source tree the binary was built from rather than kept by hand.

    tosu_notices.py --src build/tosu-src --node-license node-LICENSE \
                    --version 4.26.2 --out build/tosu/THIRD_PARTY_NOTICES.txt
"""

import argparse
import json
import pathlib
import subprocess

LICENSE_NAMES = ("license", "licence", "copying", "notice")


def license_texts(pkg_dir: pathlib.Path) -> list[str]:
    texts = []
    for f in sorted(pkg_dir.iterdir()):
        if f.is_file() and f.name.lower().split(".")[0].split("-")[0] in LICENSE_NAMES:
            texts.append(f.read_text(encoding="utf-8", errors="replace").strip())
    return texts


def production_packages(src: pathlib.Path) -> list[dict]:
    raw = subprocess.run(
        # "tosu..." takes in the deps of its workspace packages (server, common, ...)
        ["pnpm", "licenses", "list", "--prod", "--filter", "tosu...", "--json"],
        cwd=src,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    packages = []
    for license_id, entries in json.loads(raw).items():
        for e in entries:
            packages.append({**e, "license": e.get("license") or license_id})
    return sorted(packages, key=lambda p: p["name"])


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--src", required=True, type=pathlib.Path)
    ap.add_argument("--node-license", required=True, type=pathlib.Path)
    ap.add_argument("--version", required=True)
    ap.add_argument("--out", required=True, type=pathlib.Path)
    args = ap.parse_args()

    rule = "=" * 78
    out = [
        f"Third-party notices for tosu {args.version} as bundled with OPad",
        rule,
        "",
        "tosu itself is LGPL-3.0; see LICENSE and NOTICE next to this file. The",
        "bundled binary also contains the components below.",
        "",
        rule,
        "Node.js 24 runtime (embedded in the standalone tosu binary)",
        rule,
        "",
        "Node.js's LICENSE, which also covers the libraries Node embeds: OpenSSL,",
        "ICU, libuv, V8, llhttp, nghttp2, zlib and others.",
        "",
        args.node_license.read_text(encoding="utf-8").strip(),
        "",
    ]

    for pkg in production_packages(args.src):
        versions = ", ".join(pkg.get("versions", []))
        out += [rule, f"{pkg['name']} {versions}".rstrip(), f"License: {pkg['license']}"]
        if pkg.get("homepage"):
            out.append(f"Homepage: {pkg['homepage']}")
        out += [rule, ""]
        texts = []
        for path in pkg.get("paths", []):
            texts = license_texts(pathlib.Path(path))
            if texts:
                break
        out += texts or [f"(No licence file in the package; its declared licence is {pkg['license']}.)"]
        out.append("")

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("\n".join(out) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
