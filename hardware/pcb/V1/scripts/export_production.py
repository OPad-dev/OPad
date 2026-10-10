#!/usr/bin/env python3
"""Check and export the OPad V1 boards for JLCPCB / PCBWay.

For each board this runs ERC, DRC with schematic parity (and stops on any error), then writes
<board dir>/production/:
  <project>-schematic.pdf, <project>-pcb.pdf, <project>.step, top/bottom renders, DRC/ERC reports
<board dir>/production/jlcpcb/:
  <project>-gerbers.zip     Gerbers + Excellon drill files (upload this for the bare PCB)
  <project>-BOM-JLCPCB.csv  Comment, Designator, Footprint, LCSC Part #, MPN
  <project>-CPL-JLCPCB.csv  Designator, Mid X, Mid Y, Layer, Rotation
and <board dir>/production/pcbway/:
  <project>-gerbers-PCBWay.zip   same Gerbers with PCBWay's "WayWayWay" order-number marker
  <project>-BOM-PCBWay.csv       PCBWay BOM template columns
  <project>-centroid-PCBWay.csv  Designator, Mid X, Mid Y, Layer, Rotation

AISLER reads the parts from the board itself: export_aisler.py writes that package.

Usage: python3 hardware/pcb/V1/scripts/export_production.py [MX] [HE] [Carrier]   (default: all)
"""

import csv
import json
import os
import shutil
import subprocess
import sys
import tempfile
import zipfile
from collections import OrderedDict

V1 = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BOARDS = [("MX", "mx_input_v1"), ("HE", "he_input_v1"), ("Carrier", "controller_carrier_v1")]
GERBER_LAYERS = "F.Cu,B.Cu,F.Paste,B.Paste,F.Silkscreen,B.Silkscreen,F.Mask,B.Mask,Edge.Cuts"

# JLCPCB places a part by its body centre and its own 0 deg orientation, while KiCad's position
# file gives the footprint origin. Footprints listed here are written at the centre of their pads,
# with this many degrees added to the KiCad rotation. The pin sockets have their origin on pin 1
# and lie along X in JLCPCB's library, so without this the preview shows them across the board.
JLC_CPL_FIXES = {
    "PinSocket_1x14_P2.54mm_Vertical": 90,
}

# DRC warnings that may be accepted. The MX board's footprint copies were placed
# before lib/osupad.pretty was rewritten by gen_library.py (same pads, different
# description and graphics), and that board is built and working, so the copies
# in the board are the reference and the mismatch is informational.
ACCEPTED_WARNINGS = {"lib_footprint_mismatch"}
# The same for the symbols embedded in the MX schematic (R, C) against the
# regenerated lib/osupad.kicad_sym: pins and numbers match, graphics differ.
ACCEPTED_ERC = {"lib_symbol_mismatch"}


def run(*args):
    result = subprocess.run(args, capture_output=True, text=True)
    if result.returncode != 0:
        sys.exit("command failed: %s\n%s%s" % (" ".join(args), result.stdout, result.stderr))
    return result.stdout


def check(sch, pcb, out):
    erc_path = os.path.join(out, "erc.json")
    drc_path = os.path.join(out, "drc.json")
    run("kicad-cli", "sch", "erc", "--severity-all", "--format", "json", "-o", erc_path, sch)
    run("kicad-cli", "pcb", "drc", "--schematic-parity", "--refill-zones", "--severity-all",
        "--format", "json", "-o", drc_path, pcb)
    erc = json.load(open(erc_path))
    drc = json.load(open(drc_path))
    problems = []
    for sheet in erc["sheets"]:
        problems += ["ERC %s: %s" % (v["type"], v["description"]) for v in sheet["violations"]
                     if v["severity"] == "error" or v["type"] not in ACCEPTED_ERC]
    for v in drc["violations"]:
        if v["severity"] == "error" or v["type"] not in ACCEPTED_WARNINGS:
            problems.append("DRC %s %s: %s" % (v["severity"], v["type"],
                                               " / ".join(i["description"] for i in v["items"])))
    problems += ["DRC unconnected: " + " / ".join(i["description"] for i in v["items"])
                 for v in drc["unconnected_items"]]
    problems += ["parity %s: %s" % (v["type"], v["description"]) for v in drc["schematic_parity"]]
    accepted = sum(1 for v in drc["violations"] if v["type"] in ACCEPTED_WARNINGS and v["severity"] != "error")
    accepted += sum(1 for sheet in erc["sheets"] for v in sheet["violations"]
                    if v["type"] in ACCEPTED_ERC and v["severity"] != "error")
    return problems, accepted


def bom_and_cpl(pcb, out, project):
    tmp = tempfile.mkdtemp()
    pos_path = os.path.join(tmp, "pos.csv")
    run("kicad-cli", "pcb", "export", "pos", "--format", "csv", "--units", "mm", "--side", "both",
        "--use-drill-file-origin", "--exclude-dnp", "-o", pos_path, pcb)

    # Footprint fields (LCSC, MPN) come from the board file itself
    import pcbnew
    board = pcbnew.LoadBoard(pcb)
    fields = {}
    origin = board.GetDesignSettings().GetAuxOrigin()
    centres = {}
    for fp in board.GetFootprints():
        name = fp.GetFPID().GetLibItemName().wx_str()
        if name in JLC_CPL_FIXES:
            box = pcbnew.BOX2I()
            for pad in fp.Pads():
                box.Merge(pad.GetPosition())
            c = box.GetCenter()
            centres[fp.GetReference()] = (pcbnew.ToMM(c.x - origin.x), pcbnew.ToMM(origin.y - c.y),
                                          JLC_CPL_FIXES[name])
        if fp.IsExcludedFromBOM():
            continue
        fields[fp.GetReference()] = (fp.GetValue(), fp.GetFPID().GetLibItemName().wx_str(),
                                     fp.GetFieldText("LCSC"), fp.GetFieldText("MPN"))

    groups = OrderedDict()
    for ref in sorted(fields, key=lambda r: (r.rstrip("0123456789"), int("0" + "".join(c for c in r if c.isdigit())))):
        groups.setdefault(fields[ref], []).append(ref)
    with open(os.path.join(out, project + "-BOM-JLCPCB.csv"), "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["Comment", "Designator", "Footprint", "LCSC Part #", "MPN", "Quantity"])
        for (value, footprint, lcsc, mpn), refs in groups.items():
            w.writerow([value, ",".join(refs), footprint, lcsc, mpn, len(refs)])

    with open(pos_path) as src, open(os.path.join(out, project + "-CPL-JLCPCB.csv"), "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["Designator", "Mid X", "Mid Y", "Layer", "Rotation"])
        for row in csv.DictReader(src):
            x, y, rot = float(row["PosX"]), float(row["PosY"]), float(row["Rot"])
            if row["Ref"] in centres:
                x, y, extra = centres[row["Ref"]]
                rot = (rot + extra) % 360
            w.writerow([row["Ref"], "%.4fmm" % x, "%.4fmm" % y,
                        "Top" if row["Side"] == "top" else "Bottom", "%.1f" % rot])
    shutil.rmtree(tmp)


def gerbers(pcb, out, project, zip_name=None):
    tmp = tempfile.mkdtemp()
    run("kicad-cli", "pcb", "export", "gerbers", "--layers", GERBER_LAYERS, "--use-drill-file-origin",
        "--subtract-soldermask", "-o", tmp + "/", pcb)
    run("kicad-cli", "pcb", "export", "drill", "--format", "excellon", "--drill-origin", "plot",
        "--excellon-units", "mm", "--excellon-separate-th",
        "-o", tmp + "/", pcb)
    with zipfile.ZipFile(os.path.join(out, zip_name or project + "-gerbers.zip"), "w", zipfile.ZIP_DEFLATED) as z:
        for name in sorted(os.listdir(tmp)):
            z.write(os.path.join(tmp, name), name)
    shutil.rmtree(tmp)


# Assembly notes for the fab, per footprint
PCBWAY_NOTES = {
    "Kailh_CPG151101S11_MX_Hotswap_Bottom": "Hot-swap socket on BOTTOM side. Two metal tabs on the two pads; plastic body over the 3.05 mm holes",
    "JST_SH_SM08B-SRSS-TB_1x08-1MP_P1.00mm_Horizontal": "BOTTOM side. Cable opening faces the nearest board edge",
    "PinSocket_1x14_P2.54mm_Vertical": "TOP side (side marked WAVESHARE ON THIS SIDE), solder on bottom. Keep perpendicular to the board",
}


def pcbway(pcb, out, project):
    """PCBWay variant: order-number marker swapped to WayWayWay, BOM in PCBWay's template."""
    import pcbnew
    pw = os.path.join(out, "pcbway")
    os.makedirs(pw, exist_ok=True)
    tmp = tempfile.mkdtemp()
    with open(pcb) as f:
        text = f.read()
    assert '"JLCJLCJLCJLC"' in text, "order-number marker not found"
    tmp_pcb = os.path.join(tmp, project + ".kicad_pcb")
    with open(tmp_pcb, "w") as f:
        f.write(text.replace('"JLCJLCJLCJLC"', '"WayWayWay"'))
    board = pcbnew.LoadBoard(pcb)
    gerbers(tmp_pcb, pw, project, project + "-gerbers-PCBWay.zip")
    shutil.rmtree(tmp)

    rows = OrderedDict()
    for fp in sorted(board.GetFootprints(), key=lambda f: f.GetReference()):
        if fp.IsExcludedFromBOM():
            continue
        footprint = fp.GetFPID().GetLibItemName().wx_str()
        mpn = fp.GetFieldText("MPN")
        maker, _, part = mpn.partition(" ")
        kind = "THT" if fp.GetAttributes() & pcbnew.FP_THROUGH_HOLE else "SMD"
        key = (maker, part, fp.GetValue(), footprint, kind)
        rows.setdefault(key, []).append(fp.GetReference())
    with open(os.path.join(pw, project + "-BOM-PCBWay.csv"), "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["Item #", "Designator", "Qty", "Manufacturer", "Mfg Part #", "Description / Value",
                    "Package/Footprint", "Type", "Your Instructions / Notes"])
        for i, ((maker, part, value, footprint, kind), refs) in enumerate(rows.items(), 1):
            w.writerow([i, ",".join(refs), len(refs), maker, part, value, footprint, kind,
                        PCBWAY_NOTES.get(footprint, "Bottom side" if "R_" in footprint or "C_" in footprint else "")])
    shutil.copy(os.path.join(out, "jlcpcb", project + "-CPL-JLCPCB.csv"),
                os.path.join(pw, project + "-centroid-PCBWay.csv"))


def main():
    failed = False
    wanted = sys.argv[1:] or [d for d, _ in BOARDS]
    for directory, project in BOARDS:
        if directory not in wanted:
            continue
        base = os.path.join(V1, directory, project)
        sch, pcb = base + ".kicad_sch", base + ".kicad_pcb"
        out = os.path.join(V1, directory, "production")
        os.makedirs(out, exist_ok=True)
        problems, accepted = check(sch, pcb, out)
        print("%s: %d problem(s), %d accepted warning(s)" % (project, len(problems), accepted))
        for p in problems:
            print("  " + p)
        if problems:
            failed = True
            continue
        jlc = os.path.join(out, "jlcpcb")
        os.makedirs(jlc, exist_ok=True)
        gerbers(pcb, jlc, project)
        bom_and_cpl(pcb, jlc, project)
        pcbway(pcb, out, project)
        run("kicad-cli", "sch", "export", "pdf", "-o", os.path.join(out, project + "-schematic.pdf"), sch)
        run("kicad-cli", "pcb", "export", "pdf", "--layers",
            "F.Cu,B.Cu,F.Silkscreen,B.Silkscreen,F.Fab,B.Fab,Edge.Cuts", "--mode-multipage",
            "-o", os.path.join(out, project + "-pcb.pdf"), pcb)
        run("kicad-cli", "pcb", "export", "step", "--subst-models", "--force",
            "-o", os.path.join(out, project + ".step"), pcb)
        for side in ("top", "bottom"):
            run("kicad-cli", "pcb", "render", "--side", side, "--width", "1600", "--height", "1100",
                "--quality", "basic", "-o", os.path.join(out, "%s-%s.png" % (project, side)), pcb)
        print("  exported to " + out)
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
