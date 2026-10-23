"""Break the HE V2 fab files on purpose and check the checker notices.

Each case copies the V2 files, makes one kind of mistake a real board could
ship with, and asserts that the checker reports an ERROR in the expected
section. The unmodified files must pass with no error. Run:

    .tools/venv/bin/python hardware/verify/tests/test_mutations.py
"""

import csv
import io
import os
import re
import shutil
import sys
import tempfile
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))

from opadverify import check  # noqa: E402
from opadverify.copper import Copper  # noqa: E402

GOLDEN = os.path.join(os.path.dirname(HERE), "golden", "he_v2.toml")
V2 = os.path.normpath(os.path.join(os.path.dirname(HERE), "..", "pcb", "V2", "HE"))
PCB = os.path.join(V2, "he_input_v2.kicad_pcb")
JLC = os.path.join(V2, "production", "jlcpcb")
GERBERS = os.path.join(JLC, "he_input_v2-gerbers.zip")
CPL = os.path.join(JLC, "he_input_v2-CPL-JLCPCB.csv")
BOM = os.path.join(JLC, "he_input_v2-BOM-JLCPCB.csv")


def read_zip(path):
    with zipfile.ZipFile(path) as z:
        return {n: z.read(n).decode() for n in z.namelist()}


def write_zip(files, path):
    with zipfile.ZipFile(path, "w") as z:
        for n, text in files.items():
            z.writestr(n, text)


def gerber_blocks(text):
    """Split a KiCad Gerber into (attribute context, object line) pairs."""
    return text.split("\n")


def drop_track(files, net):
    """Delete every track (D01 draw) carrying `net` on B.Cu."""
    name = [n for n in files if n.endswith("-B_Cu.gbl")][0]
    out, current, dropped = [], None, 0
    for line in files[name].split("\n"):
        m = re.match(r"%TO\.N,([^*]*)\*%", line)
        if m:
            current = m.group(1)
        elif line.startswith("%TD"):
            current = None
        if current == net and line.endswith("D01*"):
            dropped += 1
            continue
        out.append(line)
    assert dropped, "no track for %s on B.Cu" % net
    files[name] = "\n".join(out)


def add_bridge(files, a, b, width=0.2):
    """Draw a B.Cu track between two points (gerber mm)."""
    name = [n for n in files if n.endswith("-B_Cu.gbl")][0]
    text = files[name]
    ap = "%ADD999C,{:.3f}*%\n".format(width)
    fmt = lambda v: str(int(round(v * 1e6)))
    draw = "D999*\nX{}Y{}D02*\nX{}Y{}D01*\n".format(fmt(a[0]), fmt(a[1]), fmt(b[0]), fmt(b[1]))
    i = text.index("G04 APERTURE END LIST*") if "G04 APERTURE END LIST*" in text else text.index("%LPD*%")
    text = text[:i] + ap + text[i:]
    j = text.rindex("M02*")
    files[name] = text[:j] + draw + text[j:]


def drop_holes_between_layers(files, gerber_zip):
    """Remove every plated hole that joins two different conductors (vias carrying a signal)."""
    cu = Copper(gerber_zip)
    name = [n for n in files if n.endswith("-PTH.drl")][0]
    keep, dropped = [], 0
    tool_lines = files[name].split("\n")
    for line in tool_lines:
        m = re.match(r"X(-?[\d.]+)Y(-?[\d.]+)$", line)
        if m:
            x, y = float(m.group(1)), float(m.group(2))
            f, bb = cu.piece_at("F", x, y), cu.piece_at("B", x, y)
            if f is not None and bb is not None:
                # a via whose two layers would otherwise stay apart
                if len([h for h in cu.holes if h[3]]) and _only_link(cu, x, y):
                    dropped += 1
                    continue
        keep.append(line)
    assert dropped, "no signal via found"
    files[name] = "\n".join(keep)


def _only_link(cu, x, y):
    f, b = cu.piece_at("F", x, y), cu.piece_at("B", x, y)
    links = [h for h in cu.holes if h[3] and cu.piece_at("F", h[0], h[1]) == f and cu.piece_at("B", h[0], h[1]) == b]
    return len(links) == 1


def run(**kw):
    args = dict(pcb=PCB, gerbers=GERBERS, cpl=CPL, bom=BOM, no_jlc=True)
    args.update(kw)
    return check.run(GOLDEN, **args)


def sections(rep):
    return {s for level, s, _ in rep.lines if level == "ERROR"}


def main():
    tmp = tempfile.mkdtemp(prefix="opadverify-")
    results = []

    def case(name, want, **kw):
        rep = run(**kw)
        got = sections(rep)
        ok = (not got) if want is None else (want in got)
        results.append((ok, name, want, got, rep))

    case("unmodified V2 passes", None)

    files = read_zip(GERBERS)
    drop_track(files, "/CS")
    p = os.path.join(tmp, "cut.zip"); write_zip(files, p)
    case("a cut track (CS on B.Cu) is an OPEN", "nets", gerbers=p)

    cu = Copper(GERBERS)
    # pins of two different nets on U4: CS (pin 11) and SDO (pin 12), 0.5 mm apart
    from opadverify import board as B
    origin, fps = B.load(PCB)
    u4 = [f for f in fps if f.ref == "U4"][0]
    to_g = lambda pt: (pt[0] - origin[0], origin[1] - pt[1])
    a, b = to_g(u4.pad("11")[0].board), to_g(u4.pad("12")[0].board)
    files = read_zip(GERBERS); add_bridge(files, a, b)
    p = os.path.join(tmp, "short.zip"); write_zip(files, p)
    case("a solder-bridge-like copper short between CS and SDO is a SHORT", "nets", gerbers=p)

    files = read_zip(GERBERS); drop_holes_between_layers(files, GERBERS)
    p = os.path.join(tmp, "novia.zip"); write_zip(files, p)
    case("missing signal vias split nets", "nets", gerbers=p)

    text = open(PCB).read()
    # mirror U2's footprint: swap its pad 1 and pad 2 positions, like V1.0
    start = text.index('(property "Reference" "U2"')
    fp_start = text.rindex("(footprint", 0, start)
    seg = text[fp_start:text.index("(footprint", start)]
    p1 = re.search(r'\(pad "1" smd roundrect\s*\(at ([-\d.]+) ([-\d.]+)', seg)
    p2 = re.search(r'\(pad "2" smd roundrect\s*\(at ([-\d.]+) ([-\d.]+)', seg)
    new = seg.replace(p1.group(0), p1.group(0).replace("(at %s %s" % p1.groups(), "(at %s %s" % p2.groups()), 1)
    new = new.replace(p2.group(0), p2.group(0).replace("(at %s %s" % p2.groups(), "(at %s %s" % p1.groups()), 1)
    p = os.path.join(tmp, "mirror.kicad_pcb"); open(p, "w").write(text[:fp_start] + new + text[fp_start + len(seg):])
    case("a mirrored SOT-23 footprint (the V1.0 error) is caught", "handedness", pcb=p)

    g = open(GOLDEN).read().replace('SCLK = "SCLK", SDI = "MOSI"', 'SCLK = "MOSI", SDI = "SCLK"')
    gp = os.path.join(os.path.dirname(GOLDEN), "_swapped.toml"); open(gp, "w").write(g)
    try:
        rep = check.run(gp, pcb=PCB, gerbers=GERBERS, cpl=CPL, bom=BOM, no_jlc=True)
        got = sections(rep)
        results.append(("nets" in got, "SCLK and MOSI wired the wrong way round at the ADC", "nets", got, rep))
    finally:
        os.remove(gp)

    rows = list(csv.DictReader(open(CPL)))
    for r in rows:
        if r["Designator"] == "U4":
            r["Rotation"] = "90.0"
    p = os.path.join(tmp, "rot.csv")
    with open(p, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=rows[0].keys()); w.writeheader(); w.writerows(rows)
    case("the ADC rotated 90 deg in the CPL", "assembly", cpl=p)

    files = read_zip(GERBERS)
    name = [n for n in files if n.endswith("-B_Mask.gbs")][0]
    ex, ey = to_g(u4.pad("17")[0].board)        # the exposed pad's mask opening
    flash = "X%dY%dD03*" % (round(ex * 1e6), round(ey * 1e6))
    assert flash in files[name], flash
    files[name] = files[name].replace(flash, "")
    p = os.path.join(tmp, "mask.zip"); write_zip(files, p)
    case("no solder-mask opening on the ADC's exposed pad", "landing", gerbers=p)

    bom = open(BOM).read().replace("C2876384", "C2871580X")
    p = os.path.join(tmp, "bom.csv"); open(p, "w").write(bom)
    case("a wrong LCSC number in the BOM", "assembly", bom=p)

    files = read_zip(GERBERS)
    name = [n for n in files if n.endswith("-NPTH.drl")][0]
    lines = files[name].split("\n")
    first = next(i for i, l in enumerate(lines) if re.match(r"X-?[\d.]+Y", l))
    del lines[first]
    files[name] = "\n".join(lines)
    p = os.path.join(tmp, "hole.zip"); write_zip(files, p)
    case("a missing mounting or peg hole", "holes", gerbers=p)

    # JLCPCB emulation: U4 at 180 deg (after the preview fix) lands; at 0 deg it does not,
    # which is what JLCPCB's preview showed on 2026-10-23
    def u4_jlc_errors(cpl_path):
        rep = check.run(GOLDEN, pcb=PCB, gerbers=GERBERS, cpl=cpl_path, bom=BOM)
        return rep, [m for lvl, sec, m in rep.lines if lvl == "ERROR" and sec == "jlc" and m.startswith("U4")]
    rep, errs = u4_jlc_errors(CPL)
    results.append((not errs, "JLCPCB's ADC model lands with the CPL's 180 deg", None, set(errs), rep))
    rows = list(csv.DictReader(open(CPL)))
    for r in rows:
        if r["Designator"] == "U4":
            r["Rotation"] = "0.0"
    p = os.path.join(tmp, "rot0.csv")
    with open(p, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=rows[0].keys()); w.writeheader(); w.writerows(rows)
    rep, errs = u4_jlc_errors(p)
    results.append((bool(errs), "JLCPCB's ADC model at 0 deg is caught (pin 1 in the far corner)", "jlc",
                    {"jlc"} if errs else set(), rep))

    shutil.rmtree(tmp)
    bad = 0
    for ok, name, want, got, rep in results:
        print("%s  %s  (expected %s, got %s)" % ("PASS" if ok else "FAIL", name, want or "no error",
                                                  ", ".join(sorted(got)) or "no error"))
        if not ok:
            bad += 1
            rep.print()
    print("%d of %d cases behave as expected" % (len(results) - bad, len(results)))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
