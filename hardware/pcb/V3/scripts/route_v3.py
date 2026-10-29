#!/usr/bin/env python3
"""Route a V3 board that generate_v3.py placed, with Freerouting (V2's method, any board).

1. Copy the board without zones (Freerouting treats pours and rule areas as
   obstacles) and export Specctra DSN, giving the power nets listed in the
   board's route.json their own wider track class.
2. Run Freerouting 1.9.0 single-threaded (2.1.0 routes, then hangs without
   writing; the multi-threaded optimiser leaves clearance errors).
3. Import the SES, add the GND stitching vias from route.json that clear the
   routed copper and the holes, and refill the zones.

Each step that loads a board runs in its own process: pcbnew under Python
3.14 fails when one process loads a second board.

    python3 hardware/pcb/V3/scripts/route_v3.py Carrier|USB|MX|HE [passes]
"""

import json
import math
import os
import re
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
V3 = os.path.dirname(HERE)
PROJECTS = {"Carrier": "carrier_v3", "USB": "usb_v3", "MX": "mx_input_v3", "HE": "he_input_v3"}
JAR = os.environ.get("FREEROUTING_JAR", os.path.expanduser("~/.kicad-mcp/freerouting-1.9.0.jar"))
VIA, DRILL = 0.6, 0.3
CLEAR = 0.15 + 0.05       # net clearance plus a margin, via edge to foreign copper
HOLE_CLEAR = 0.25         # copper to a non-plated hole


def paths(name):
    d = os.path.join(V3, name)
    return os.path.join(d, PROJECTS[name] + ".kicad_pcb"), json.load(open(os.path.join(d, "route.json")))


def step_nozones(name, tmp):
    import pcbnew
    board, _ = paths(name)
    b = pcbnew.LoadBoard(board)
    for z in list(b.Zones()):
        b.Remove(z)
    b.Save(os.path.join(tmp, "nozones.kicad_pcb"))


def step_dsn(name, tmp):
    import pcbnew
    _, cfg = paths(name)
    b = pcbnew.LoadBoard(os.path.join(tmp, "nozones.kicad_pcb"))
    dsn = os.path.join(tmp, "board.dsn")
    assert pcbnew.ExportSpecctraDSN(b, dsn), "DSN export failed"
    text = open(dsn).read()
    m = re.search(r"\(class kicad_default (.*?)\(circuit", text, re.S)
    nets = re.findall(r'"[^"]*"|\S+', m.group(1))
    widths = {}
    for net, w in cfg["power"].items():
        widths.setdefault(w, []).append("/" + net)
    keep = [n for n in nets if n not in sum(widths.values(), [])]
    via = re.search(r'\(use_via "[^"]*"\)', text).group(0)
    extra = "".join("    (class power_%d %s\n      (circuit\n        %s\n      )\n      (rule\n        (width %d)\n"
                    "        (clearance 150)\n      )\n    )\n"
                    % (int(w * 1000), " ".join(n for n in ns if n in nets), via, int(w * 1000))
                    for w, ns in widths.items() if any(n in nets for n in ns))
    text = text[:m.start()] + "(class kicad_default " + " ".join(keep) + "\n      (circuit" + text[m.end():]
    text = edge_keepout(text, EDGE_BAND)
    i = text.index("  (wiring")
    j = text.rindex("  )\n", 0, i)          # end of (network ...)
    text = text[:j] + extra + text[j:]
    open(dsn, "w").write(text)


EDGE_BAND = 0.32          # KiCad's copper-to-edge rule is 0.3 mm; Freerouting does not know it


def edge_keepout(text, band):
    """Add keepout strips along the inside of the DSN boundary (um), one per boundary segment."""
    m = re.search(r"\(boundary\s+\(path pcb 0 ([-\d.\s]+)\)", text)
    v = [float(t) for t in m.group(1).split()]
    pts = list(zip(v[0::2], v[1::2]))
    if pts[0] == pts[-1]:
        pts = pts[:-1]
    area = sum(pts[i][0] * pts[i - 1][1] - pts[i - 1][0] * pts[i][1] for i in range(len(pts)))
    sign = 1 if area > 0 else -1            # inward normal side
    w = band * 1000
    out = []
    for i in range(len(pts)):
        (x0, y0), (x1, y1) = pts[i - 1], pts[i]
        L = math.hypot(x1 - x0, y1 - y0)
        if L < 1:
            continue
        nx, ny = sign * (y1 - y0) / L, -sign * (x1 - x0) / L
        quad = [(x0, y0), (x1, y1), (x1 + nx * w, y1 + ny * w), (x0 + nx * w, y0 + ny * w)]
        out.append('    (keepout "" (polygon signal 0 %s))\n' % " ".join("%.1f %.1f" % q for q in quad))
    i = text.index("    (via")
    return text[:i] + "".join(out) + text[i:]


def step_import(name, tmp):
    import pcbnew
    board, _ = paths(name)
    b = pcbnew.LoadBoard(board)
    assert pcbnew.ImportSpecctraSES(b, os.path.join(tmp, "board.ses")), "SES import failed"
    b.Save(board)


def seg_dist(p, a, b):
    ax, ay = a
    bx, by = b
    dx, dy = bx - ax, by - ay
    t = 0.0 if dx == dy == 0 else max(0.0, min(1.0, ((p[0] - ax) * dx + (p[1] - ay) * dy) / (dx * dx + dy * dy)))
    return math.hypot(p[0] - ax - t * dx, p[1] - ay - t * dy)


def step_stitch(name, _tmp):
    import pcbnew
    board, cfg = paths(name)
    mm = pcbnew.ToMM
    b = pcbnew.LoadBoard(board)
    gnd = b.FindNet("/GND")
    # Routed copper, read from the file: pcbnew's track iterator is broken under Python 3.14
    text = open(board).read()
    copper = []
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
    for x, y in cfg["stitch"]:
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
            print("  stitching via at (%.2f, %.2f) skipped: too close to other copper" % (x, y))
            continue
        v = pcbnew.PCB_VIA(b)
        v.SetPosition(pcbnew.VECTOR2I(pcbnew.FromMM(x), pcbnew.FromMM(y)))
        v.SetWidth(pcbnew.FromMM(VIA))
        v.SetDrill(pcbnew.FromMM(DRILL))
        v.SetNet(gnd)
        b.Add(v)
        added += 1
    print("  %d of %d stitching vias added" % (added, len(cfg["stitch"])))
    pcbnew.ZONE_FILLER(b).Fill(b.Zones())
    b.Save(board)


STEPS = {"nozones": step_nozones, "dsn": step_dsn, "import": step_import, "stitch": step_stitch}


def main():
    sys.stdout.reconfigure(line_buffering=True)
    if len(sys.argv) == 4 and sys.argv[1] in STEPS:
        STEPS[sys.argv[1]](sys.argv[2], sys.argv[3])
        return
    name = sys.argv[1]
    passes = sys.argv[2] if len(sys.argv) > 2 else "200"
    tmp = tempfile.mkdtemp(prefix="v3_route_")
    board = paths(name)[0]
    placed = os.path.join(tmp, "placed.kicad_pcb")
    shutil.copy(board, placed)

    def sub(step):
        subprocess.run([sys.executable, os.path.abspath(__file__), step, name, tmp], check=True)

    # Freerouting is not deterministic and sometimes leaves a net or two; try again from the placed board
    for attempt in range(1, 9):
        shutil.copy(placed, board)
        sub("nozones")
        sub("dsn")
        print("routing %s (freerouting, %s passes, attempt %d)..." % (name, passes, attempt))
        r = subprocess.run(["java", "-jar", JAR, "-de", os.path.join(tmp, "board.dsn"), "-do",
                            os.path.join(tmp, "board.ses"), "-mp", passes, "-mt", "1"], capture_output=True, text=True)
        summary = [l for l in (r.stdout + r.stderr).splitlines() if "was completed" in l]
        print("  " + (" / ".join(x.split("INFO")[-1].strip() for x in summary[-2:]) if summary else
                      "no summary; exit %d" % r.returncode))
        sub("import")
        sub("stitch")
        report = os.path.join(tmp, "drc.json")
        subprocess.run(["kicad-cli", "pcb", "drc", "--refill-zones", "--severity-error", "--format", "json",
                        "-o", report, board], capture_output=True)
        d = json.load(open(report))
        left, errors = len(d["unconnected_items"]), len(d["violations"])
        print("  DRC: %d error(s), %d unconnected" % (errors, left))
        if not left and not errors:
            break
    else:
        sys.exit("routing did not finish cleanly in 8 attempts")
    print("routed:", board)


if __name__ == "__main__":
    main()
