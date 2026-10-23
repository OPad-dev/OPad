#!/usr/bin/env python3
"""Route the HE V2 board that generate_he_v2.py placed, with Freerouting.

1. Copy the board without zones: Freerouting treats copper pours and rule
   areas as obstacles, and then cannot reach the sensors or the ADC.
2. Export Specctra DSN, run Freerouting (single-threaded: its multi-threaded
   optimiser is known to leave clearance violations), import the SES into
   the board with its zones.
3. Add the GND stitching vias (generate_he_v2.STITCH) that clear the routed
   copper, and refill the zones.

pcbnew's Python crashes when one process loads a second board, so each step
that loads a board runs in a process of its own.

    python3 hardware/pcb/V2/scripts/route_he_v2.py [passes]    (default 200)
Needs Java and Freerouting 1.9.0 (FREEROUTING_JAR, default ~/.kicad-mcp/freerouting-1.9.0.jar). 2.1.0
routes this board completely and then hangs without writing the result.
"""

import math
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
BOARD = os.path.join(os.path.dirname(HERE), "HE", "he_input_v2.kicad_pcb")
JAR = os.environ.get("FREEROUTING_JAR", os.path.expanduser("~/.kicad-mcp/freerouting-1.9.0.jar"))
VIA, DRILL = 0.6, 0.3
CLEAR = 0.15 + 0.05       # net clearance plus a margin, via edge to foreign copper
HOLE_CLEAR = 0.25         # copper to a non-plated hole


def step_export(tmp):
    import pcbnew
    b = pcbnew.LoadBoard(BOARD)
    for z in list(b.Zones()):
        b.Remove(z)
    path = os.path.join(tmp, "nozones.kicad_pcb")
    b.Save(path)


def step_dsn(tmp):
    import pcbnew
    b = pcbnew.LoadBoard(os.path.join(tmp, "nozones.kicad_pcb"))
    assert pcbnew.ExportSpecctraDSN(b, os.path.join(tmp, "board.dsn")), "DSN export failed"


def step_import(tmp):
    import pcbnew
    b = pcbnew.LoadBoard(BOARD)
    assert pcbnew.ImportSpecctraSES(b, os.path.join(tmp, "board.ses")), "SES import failed"
    b.Save(BOARD)


def seg_dist(p, a, b):
    ax, ay = a
    bx, by = b
    dx, dy = bx - ax, by - ay
    t = 0.0 if dx == dy == 0 else max(0.0, min(1.0, ((p[0] - ax) * dx + (p[1] - ay) * dy) / (dx * dx + dy * dy)))
    return math.hypot(p[0] - ax - t * dx, p[1] - ay - t * dy)


def step_stitch(_tmp):
    import pcbnew
    sys.path.insert(0, HERE)
    from generate_he_v2 import STITCH
    mm = pcbnew.ToMM
    b = pcbnew.LoadBoard(BOARD)
    gnd = b.FindNet("/GND")
    # Routed copper, read from the file: pcbnew's track iterator is broken
    # under Python 3.14 ('SwigPyIterator' object has no attribute 'next')
    import re
    text = open(BOARD).read()
    copper = []   # (kind, geometry, half width) of every foreign-net object
    for m in re.finditer(r'\(segment\s+\(start ([-\d.]+) ([-\d.]+)\)\s+\(end ([-\d.]+) ([-\d.]+)\)\s+'
                         r'\(width ([\d.]+)\).*?\(net "([^"]*)"\)', text, re.S):
        if m.group(6) != "/GND":
            copper.append(("seg", ((float(m.group(1)), float(m.group(2))), (float(m.group(3)), float(m.group(4)))),
                           float(m.group(5)) / 2))
    for m in re.finditer(r'\(via\s+\(at ([-\d.]+) ([-\d.]+)\)\s+\(size ([\d.]+)\).*?\(net "([^"]*)"\)', text, re.S):
        if m.group(4) != "/GND":
            copper.append(("pt", (float(m.group(1)), float(m.group(2))), float(m.group(3)) / 2))
    holes = []
    for fp in b.GetFootprints():
        for pad in fp.Pads():
            box = pad.GetBoundingBox()
            rect = (mm(box.GetLeft()), mm(box.GetTop()), mm(box.GetRight()), mm(box.GetBottom()))
            if pad.GetAttribute() == pcbnew.PAD_ATTRIB_NPTH:
                holes.append(((mm(pad.GetPosition().x), mm(pad.GetPosition().y)), mm(pad.GetDrillSize().x) / 2))
            elif pad.GetNetCode() != gnd.GetNetCode():
                copper.append(("rect", rect, 0.0))
    added = 0
    for x, y in STITCH:
        ok = True
        for kind, geo, half in copper:
            if kind == "pt":
                d = math.hypot(x - geo[0], y - geo[1]) - half
            elif kind == "seg":
                d = seg_dist((x, y), *geo) - half
            else:
                x0, y0, x1, y1 = geo
                d = math.hypot(max(x0 - x, 0, x - x1), max(y0 - y, 0, y - y1))
            if d < VIA / 2 + CLEAR:
                ok = False
                break
        ok = ok and all(math.hypot(x - c[0], y - c[1]) - r >= VIA / 2 + HOLE_CLEAR for c, r in holes)
        if not ok:
            print("  stitching via at (%.2f, %.2f) skipped: too close to routed copper" % (x, y))
            continue
        v = pcbnew.PCB_VIA(b)
        v.SetPosition(pcbnew.VECTOR2I(pcbnew.FromMM(x), pcbnew.FromMM(y)))
        v.SetWidth(pcbnew.FromMM(VIA))
        v.SetDrill(pcbnew.FromMM(DRILL))
        v.SetNet(gnd)
        b.Add(v)
        added += 1
    print("  %d of %d stitching vias added" % (added, len(STITCH)))
    pcbnew.ZONE_FILLER(b).Fill(b.Zones())
    b.Save(BOARD)


STEPS = {"export": step_export, "dsn": step_dsn, "import": step_import, "stitch": step_stitch}


def main():
    sys.stdout.reconfigure(line_buffering=True)
    if len(sys.argv) == 3 and sys.argv[1] in STEPS:
        STEPS[sys.argv[1]](sys.argv[2])
        return
    passes = sys.argv[1] if len(sys.argv) > 1 else "200"
    tmp = tempfile.mkdtemp(prefix="he_v2_route_")

    def sub(name):
        subprocess.run([sys.executable, os.path.abspath(__file__), name, tmp], check=True)

    sub("export")
    sub("dsn")
    print("routing (freerouting, %s passes)..." % passes)
    r = subprocess.run(["java", "-jar", JAR, "-de", os.path.join(tmp, "board.dsn"), "-do", os.path.join(tmp, "board.ses"),
                        "-mp", passes, "-mt", "1"], capture_output=True, text=True)
    summary = [l for l in (r.stdout + r.stderr).splitlines() if "was completed" in l]
    print("  " + (" / ".join(x.split("INFO")[-1].strip() for x in summary) if summary else "no summary; exit %d" % r.returncode))
    sub("import")
    sub("stitch")
    print("routed:", BOARD)


if __name__ == "__main__":
    main()
