#!/usr/bin/env python3
"""Check and export the OPad V3 boards (V1's export_production.py, with a JLCPCB-model CPL).

The checks and files are V1's: ERC, DRC with schematic parity and refilled
zones (stop on any problem), Gerbers, drills, BOM, CPL, PCBWay variant, PDFs,
STEP, renders. The difference is the CPL: instead of KiCad's footprint origin
and rotation plus hand-kept fixes, every LCSC part is placed by fitting
JLCPCB's own footprint of that part (LCSC product API, hardware/verify/
opadverify/jlc.py) onto our pads, so its preview needs no turning. Parts
LCSC has no footprint for keep KiCad's position (two-terminal passives only;
anything else stops the export).

    python3 hardware/pcb/V3/scripts/export_v3.py [Carrier] [MX] [HE]   (default: all)
"""

import csv
import math
import os
import shutil
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
V3 = os.path.dirname(HERE)
ROOT = os.path.normpath(os.path.join(V3, "..", "..", ".."))
sys.path.insert(0, os.path.join(os.path.dirname(V3), "V1", "scripts"))
sys.path.insert(0, os.path.join(ROOT, "hardware", "verify"))
import export_production as ep  # noqa: E402
from opadverify import board as B  # noqa: E402
from opadverify import jlc  # noqa: E402

ep.V1 = V3
ep.BOARDS = [("Carrier", "carrier_v3"), ("MX", "mx_input_v3"), ("HE", "he_input_v3")]
ep.ACCEPTED_WARNINGS = set()
ep.ACCEPTED_ERC = set()
TOL = 0.35        # mm, model pad centre to our pad centre


def fit(model, pads, bottom):
    """Best (unordered, worst, rotation, x, y) placing the model's pads onto ours.

    pads: [(number, (x, y))] in Gerber mm. A numbered fit (model pad n on our
    pad n) is preferred; an unordered one (every model pad on some pad of ours)
    is only for parts whose pads are interchangeable, and is reported as such."""
    ours = {}
    for n, xy in pads:
        ours.setdefault(n, xy)
    every = [xy for _, xy in pads]
    best = None
    for rot in (0, 90, 180, 270):
        placed = {i: jlc.place(p, 0, 0, rot, bottom) for i, p in enumerate(model)}
        pairs = [(placed[i], ours[p["number"]]) for i, p in enumerate(model) if p["number"] in ours]
        for numbered in (True, False):
            if numbered:
                if len(pairs) < max(2, 0.6 * len(model)):
                    continue
                tx = sum(o[0] - m[0] for m, o in pairs) / len(pairs)
                ty = sum(o[1] - m[1] for m, o in pairs) / len(pairs)
            else:   # interchangeable pads: centroid, then nearest neighbours
                mx = sum(v[0] for v in placed.values()) / len(placed)
                my = sum(v[1] for v in placed.values()) / len(placed)
                ox = sum(v[0] for v in every) / len(every)
                oy = sum(v[1] for v in every) / len(every)
                tx, ty = ox - mx, oy - my
            worst = 0.0
            for i, p in enumerate(model):
                x, y = placed[i][0] + tx, placed[i][1] + ty
                if numbered and p["number"] in ours:
                    d = math.hypot(ours[p["number"]][0] - x, ours[p["number"]][1] - y)
                else:
                    d = min(math.hypot(o[0] - x, o[1] - y) for o in every)
                worst = max(worst, d)
            cand = (not numbered, worst, rot, tx, ty)
            if worst <= TOL and (best is None or cand < best):
                best = cand
    return best


def bom_and_cpl(pcb, out, project):
    original(pcb, out, project)
    origin, fps = B.load(pcb)
    by_ref = {f.ref: f for f in fps}
    lcsc = {}       # from the BOM just written (pcbnew cannot load a second board in one process)
    for row in csv.DictReader(open(os.path.join(out, project + "-BOM-JLCPCB.csv"))):
        for ref in row["Designator"].split(","):
            lcsc[ref] = row["LCSC Part #"]
    path = os.path.join(out, project + "-CPL-JLCPCB.csv")
    rows = list(csv.DictReader(open(path)))
    for r in rows:
        ref = r["Designator"]
        fp = by_ref[ref]
        ours = [(p.number, (p.board[0] - origin[0], origin[1] - p.board[1]))
                for p in fp.pads if p.kind in ("smd", "thru_hole") and p.number]
        try:
            model = [p for p in jlc.product(lcsc[ref])["pads"] if p.get("layer") in ("1", "11")]
        except Exception as e:
            if len(ours) == 2:
                print("  %s: no JLCPCB footprint for %s (%s); two-terminal part, KiCad's position kept"
                      % (ref, lcsc[ref], e))
                continue
            sys.exit("%s: no JLCPCB footprint for %s: %s" % (ref, lcsc[ref], e))
        bottom = r["Layer"] == "Bottom"
        f = fit(model, ours, bottom)
        if len(model) != len(ours):
            sys.exit("%s: JLCPCB's footprint of %s has %d pads, ours %d" % (ref, lcsc[ref], len(model), len(ours)))
        if f is None:
            sys.exit("%s: JLCPCB's footprint of %s does not fit our pads in any rotation" % (ref, lcsc[ref]))
        unordered, worst, rot, tx, ty = f
        if bottom and rot % 180 and len(ours) > 2 and not unordered:
            print("  %s: bottom-side part at %d deg: confirm in JLCPCB's preview" % (ref, rot))
        r["Mid X"], r["Mid Y"], r["Rotation"] = "%.4fmm" % tx, "%.4fmm" % ty, "%.1f" % rot
        print("  %s: JLCPCB model fits at %d deg, worst pad %.2f mm%s"
              % (ref, rot, worst, " (interchangeable pads)" if unordered else ""))
    with open(path, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=["Designator", "Mid X", "Mid Y", "Layer", "Rotation"])
        w.writeheader()
        w.writerows(rows)


original = ep.bom_and_cpl
ep.bom_and_cpl = bom_and_cpl

if __name__ == "__main__":
    ep.main()
