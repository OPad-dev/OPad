#!/usr/bin/env python3
"""Package he_input_v1 for AISLER, which reads the parts from the board itself.

    python3 hardware/pcb/V1/scripts/export_he_aisler.py

AISLER takes a native KiCad board or ODB++ and reads every component from it
(Reference, Value, Footprint, and the MPN / MFG fields the generator writes),
so no separate BOM or centroid file is needed.  Writes HE/production/aisler/:

  he_input_v1.kicad_pcb        the board with its copper zones filled and
                               saved by KiCad (upload this one)
  he_input_v1.kicad_pro        its design rules (only needed to run DRC)
  he_input_v1-odb.zip          the same board as ODB++, the alternative upload
  he_input_v1-schematic.pdf    for reference
  he_input_v1-BOM.csv          generic BOM (Reference, Qty, Value, MFG, MPN,
                               LCSC) for checking the parts list by eye
  README.txt                   order options and what to check
  he_input_v1-aisler.zip       all of the above in one archive

Needs kicad-cli (KiCad 9 or later for the ODB++ export).  Runs the board's own
checks first and refuses to export if any fails.
"""

import os
import shutil
import subprocess
import sys
import zipfile

import he_board as B

V1 = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
HE = os.path.join(V1, "HE")
OUT = os.path.join(HE, "production", "aisler")


def run(args):
    proc = subprocess.run(args, capture_output=True, text=True)
    if proc.returncode != 0:
        sys.exit("%s failed:\n%s%s" % (" ".join(args[:3]), proc.stdout, proc.stderr))
    return proc.stdout


def bom_csv():
    rows = ["Reference,Quantity,Value,Footprint,MFG,MPN,LCSC,Description"]
    for value, refs, footprint, lcsc, mpn, manufacturer, descr in B.bom_groups():
        rows.append('"%s",%d,%s,%s,%s,%s,%s,"%s"'
                    % (",".join(refs), len(refs), value, footprint, manufacturer, mpn,
                       lcsc, descr))
    return "\n".join(rows) + "\n"


README = """osuPad Hall Effect input module %(rev)s for AISLER
==================================================

Upload he_input_v1.kicad_pcb (native KiCad board, zones filled, parts carry
MPN and MFG fields) or he_input_v1-odb.zip (ODB++ of the same board).  AISLER
reads the components from either; no BOM or placement file is needed.

Board options
  2 layers, 1.6 mm FR-4, 1 oz copper, HASL or ENIG (the design meets the
  "2 layer 1.6 mm HASL" rules: track 0.25 / spacing 0.2 mm, vias 0.7 mm with
  a 0.3 mm hole, copper to edge 0.3 mm), %(w).1f x %(h).1f mm, green mask.

Assembly
  Every component is on the BOTTOM side, all SMD:
%(bom)s
  TP1-TP3 are bare probe pads, H1/H2 mounting holes and SW1/SW2 the switch
  positions: they are board features (excluded from BOM, DNP), not parts.
  The Gateron magnetic switches go in later by hand.

Check in the AISLER part view before paying
  U1/U2 (SOT-23): the single lead (pin 3, GND) points to the front edge (the
  "OSUPAD HE" text), the two leads to the connector; pin 1 (VCC) is next to
  the silkscreen dot.  A SOT-23 cannot be placed mirrored, so if the leads
  sit on the pads the orientation is right.
  J1: the 8 contacts face the inside of the board, the cable opening the edge.

Source: hardware/pcb/V1/scripts/he_board.py (single source of truth); these
files were written by export_he_aisler.py.
"""


def main():
    problems = B.selftest()
    errors = B.check()
    for line in problems + errors:
        print("BLOCKED:", line)
    if problems or errors:
        raise SystemExit("refusing to export")
    cli = shutil.which("kicad-cli")
    if not cli:
        sys.exit("kicad-cli not found")

    if os.path.isdir(OUT):
        shutil.rmtree(OUT)
    os.makedirs(OUT)

    board = os.path.join(OUT, B.PROJECT + ".kicad_pcb")
    shutil.copy(os.path.join(HE, B.PROJECT + ".kicad_pcb"), board)
    # The design rules live in the project file; without it next to the board
    # KiCad would judge the copy by its defaults (0.5 mm edge clearance).
    project = os.path.join(OUT, B.PROJECT + ".kicad_pro")
    shutil.copy(os.path.join(HE, B.PROJECT + ".kicad_pro"), project)
    # Fill the zones and let KiCad save the board, so the uploaded file shows
    # the pours without AISLER having to fill them; then check that saved copy.
    report = os.path.join(OUT, "drc.tmp.json")
    run([cli, "pcb", "drc", "--refill-zones", "--save-board", "--format", "json",
         "-o", report, board])
    run([cli, "pcb", "drc", "--format", "json", "--severity-error",
         "--exit-code-violations", "-o", report, board])
    os.remove(report)

    odb = os.path.join(OUT, B.PROJECT + "-odb.zip")
    run([cli, "pcb", "export", "odb", "--check-zones", "-o", odb, board])

    pdf = os.path.join(OUT, B.PROJECT + "-schematic.pdf")
    run([cli, "sch", "export", "pdf", "-o", pdf, os.path.join(HE, B.PROJECT + ".kicad_sch")])

    bom = os.path.join(OUT, B.PROJECT + "-BOM.csv")
    with open(bom, "w", newline="\n") as f:
        f.write(bom_csv())

    lines = []
    for value, refs, _fp, lcsc, mpn, manufacturer, _d in B.bom_groups():
        lines.append("    %-9s %s %s (%s, LCSC %s)" % (",".join(refs), manufacturer, mpn, value, lcsc))
    readme = os.path.join(OUT, "README.txt")
    with open(readme, "w", newline="\n") as f:
        f.write(README % dict(rev=B.REV, w=B.BOARD_W, h=B.BOARD_H, bom="\n".join(lines)))

    archive = os.path.join(OUT, B.PROJECT + "-aisler.zip")
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as z:
        for path in (board, project, odb, pdf, bom, readme):
            z.write(path, os.path.basename(path))
    for path in (board, project, odb, pdf, bom, readme, archive):
        print("wrote %s (%d bytes)" % (os.path.relpath(path, V1), os.path.getsize(path)))


if __name__ == "__main__":
    main()
