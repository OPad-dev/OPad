#!/usr/bin/env python3
"""Check the V3 case against everything that goes inside it.

Each reference part in tools/reference_models.scad (Waveshare stack, carrier,
input module, plugs, cable) is intersected with the top case and the bottom
plate; the two case parts are also intersected with each other, and the
pairs of reference parts in REF_PAIRS (the cable against the stack and the
module, the carrier's top-side parts against the Waveshare) with each other.
Any overlap above 0.01 mm^3 is a failure. Needs OpenSCAD; no Python packages.

Usage: python3 hardware/3d/custom_case/V3/tools/check_fit.py [-j JOBS] [-D NAME=VALUE ...]
"""

import argparse
import concurrent.futures
import os
import re
import subprocess
import sys
import tempfile

TOOLS = os.path.dirname(os.path.abspath(__file__))
SCAD = os.path.join(TOOLS, "fit_check.scad")
LIMIT = 0.01  # mm^3


def ref_names():
    text = open(os.path.join(TOOLS, "reference_models.scad")).read()
    body = re.search(r"REF_NAMES\s*=\s*\[(.*?)\];", text, re.S).group(1)
    return re.findall(r'"([^"]+)"', body)


def ref_pairs():
    text = open(os.path.join(TOOLS, "reference_models.scad")).read()
    body = re.search(r"REF_PAIRS\s*=\s*\[(.*)\];", text, re.S).group(1)
    return re.findall(r'\["([^"]+)",\s*"([^"]+)"\]', body)


def stl_volume(path):
    """Volume of an ASCII STL (sum of signed tetrahedra)."""
    vol, tri = 0.0, []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if line.startswith("vertex"):
                tri.append([float(v) for v in line.split()[1:4]])
                if len(tri) == 3:
                    (ax, ay, az), (bx, by, bz), (cx, cy, cz) = tri
                    vol += (ax * (by * cz - bz * cy) - ay * (bx * cz - bz * cx) + az * (bx * cy - by * cx)) / 6.0
                    tri = []
    return abs(vol)


def run(ref, case, defines=()):
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "x.stl")
        extra = [a for d in defines for a in ("-D", d)]
        target = ["-D", 'CASE="ref"', "-D", 'REF2="%s"' % case[4:]] if case.startswith("ref:") else ["-D", 'CASE="%s"' % case]
        r = subprocess.run(["openscad", "-o", out, "-D", 'REF="%s"' % ref] + target + extra + [SCAD],
                           capture_output=True, text=True)
        log = r.stdout + r.stderr
        if "Current top level object is empty" in log or not os.path.exists(out):
            if r.returncode != 0 and "empty" not in log:
                return ref, case, None, log.strip().splitlines()[-1]
            return ref, case, 0.0, ""
        return ref, case, stl_volume(out), ""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("-j", "--jobs", type=int, default=os.cpu_count() or 4)
    ap.add_argument("-D", dest="defines", action="append", default=[],
                    help="override a parameter, e.g. -D TILT=14")
    args = ap.parse_args()
    jobs = [(r, c) for r in ref_names() for c in ("top", "bottom")]
    jobs += [("top", "bottom")]
    jobs += [(a, "ref:" + b) for a, b in ref_pairs()]
    failed = False
    # every reference part and both case parts must render with some volume, or "clear" means nothing
    with concurrent.futures.ThreadPoolExecutor(args.jobs) as ex:
        for ref, _, vol, err in ex.map(lambda r: run(r, "none", args.defines), ref_names()):
            if not vol or vol < 0.5:
                print("%-20s renders empty (%s): its checks mean nothing" % (ref, err or "%.2f mm^3" % (vol or 0)))
                failed = True
    with concurrent.futures.ThreadPoolExecutor(args.jobs) as ex:
        for ref, case, vol, err in sorted(ex.map(lambda j: run(*j, args.defines), jobs)):
            if vol is None:
                status, failed = "ERROR " + err, True
            elif vol > LIMIT:
                status, failed = "OVERLAP %.2f mm^3" % vol, True
            else:
                status = "clear"
            print("%-20s vs %-7s %s" % (ref, case, status))
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
