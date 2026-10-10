#!/usr/bin/env python3
"""Package the V1 boards for AISLER, which reads the parts from the board itself.

    python3 hardware/pcb/V1/scripts/export_aisler.py [MX] [HE] [Carrier]   (default: all)

AISLER takes a native KiCad board or ODB++ and reads every component from it
(Reference, Value, Footprint and the MPN / MFG fields), so no BOM or centroid
file is needed.  For each board this writes <board dir>/production/aisler/:

  <project>.kicad_pcb        the board with its copper zones filled and saved
                             by KiCad, the JLCPCB order-number text removed,
                             and MPN / MFG as separate fields (upload this one)
  <project>.kicad_pro        its design rules (only needed to run DRC on it)
  <project>-odb.zip          the same board as ODB++, the alternative upload
  <project>-schematic.pdf    for reference
  <project>-BOM.csv          Reference, Qty, Value, MFG, MPN, LCSC, for checking
                             the parts list by eye
  README.txt                 order options and what to check
  <project>-aisler.zip       all of the above in one archive

Needs kicad-cli (KiCad 9 or later for the ODB++ export); no pcbnew.  The copy
must pass KiCad's DRC (with the project's rules) before anything is written.
"""

import os
import shutil
import subprocess
import sys
import zipfile

from generate_he_kicad import parse, dump, find, find_all

V1 = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BOARDS = [("MX", "mx_input_v1"), ("HE", "he_input_v1"), ("Carrier", "controller_carrier_v1")]
ORDER_MARKER = "JLCJLCJLCJLC"

NOTES = {
    "MX": "Switch sockets, J1, R1-R4 and C1 are all on the BOTTOM side (SMD).",
    "HE": "Every part is on the BOTTOM side (SMD). TP1-TP3 are bare probe pads, H1/H2\n"
          "  mounting holes and SW1/SW2 the switch positions: board features (DNP), not\n"
          "  parts. The Gateron magnetic switches go in later by hand.\n"
          "  U1/U2 (SOT-23): the single lead (pin 3, GND) points to the front edge (the\n"
          "  \"OSUPAD HE\" text), the two leads to the connector; pin 1 (VCC) is next to\n"
          "  the silkscreen dot. A SOT-23 cannot be placed mirrored, so if the leads sit\n"
          "  on the pads the orientation is right.",
    "Carrier": "J_MOD (JST SH) is the only SMD part, on the BOTTOM side. J_P1/J_P2 are\n"
               "  through-hole sockets on the TOP side (the side marked WAVESHARE ON THIS\n"
               "  SIDE); order them as THT or solder them yourself.",
}


def run(args):
    proc = subprocess.run(args, capture_output=True, text=True)
    if proc.returncode != 0:
        sys.exit("%s failed:\n%s%s" % (" ".join(args[:4]), proc.stdout, proc.stderr))
    return proc.stdout


def _prop(node, name):
    for p in find_all(node, "property"):
        if len(p) > 2 and p[1].strip('"') == name:
            return p
    return None


def prepare(tree):
    """Strip the JLCPCB order text; give every part bare MPN and MFG fields."""
    out = []
    for node in tree:
        if isinstance(node, list) and node and node[0] == "gr_text" and len(node) > 1 \
                and node[1].strip('"') == ORDER_MARKER:
            continue
        out.append(node)
    tree[:] = out
    for fp in find_all(tree, "footprint"):
        mpn, mfg = _prop(fp, "MPN"), _prop(fp, "MFG")
        if mpn is None or mfg is not None:
            continue
        maker, _sep, part = mpn[2].strip('"').rpartition(" ")
        if not part:
            continue
        mpn[2] = '"%s"' % part
        clone = [c for c in mpn]
        clone[1], clone[2] = '"MFG"', '"%s"' % maker
        for i, c in enumerate(clone):
            if isinstance(c, list) and c and c[0] == "uuid":
                clone[i] = ["uuid", '"%s"' % _uuid("mfg/" + c[1])]
        fp.insert(fp.index(mpn) + 1, clone)


def _uuid(seed):
    import uuid
    return str(uuid.uuid5(uuid.NAMESPACE_URL, "osupad/aisler/" + seed))


def parts(tree):
    """[(refs, value, footprint, mfg, mpn, lcsc, side, kind)] of the assembled parts."""
    groups = {}
    order = []
    for fp in find_all(tree, "footprint"):
        attr = find(fp, "attr") or []
        if "exclude_from_bom" in attr or "dnp" in attr or "board_only" in attr:
            continue
        ref = _prop(fp, "Reference")[2].strip('"')
        layer = find(fp, "layer")[1].strip('"')
        key = (_prop(fp, "Value")[2].strip('"'), fp[1].strip('"').split(":")[-1],
               (_prop(fp, "MFG") or [None, None, '""'])[2].strip('"'),
               (_prop(fp, "MPN") or [None, None, '""'])[2].strip('"'),
               (_prop(fp, "LCSC") or [None, None, '""'])[2].strip('"'),
               "bottom" if layer.startswith("B") else "top",
               "THT" if "through_hole" in attr else "SMD")
        if key not in groups:
            groups[key] = []
            order.append(key)
        groups[key].append(ref)
    def natural(ref):
        head = ref.rstrip("0123456789")
        return (head, int(ref[len(head):] or 0))
    return [(sorted(groups[k], key=natural),) + k for k in order]


def bom_csv(rows):
    out = ["Reference,Quantity,Value,Footprint,MFG,MPN,LCSC,Side,Type"]
    for refs, value, footprint, mfg, mpn, lcsc, side, kind in rows:
        out.append('"%s",%d,%s,%s,%s,%s,%s,%s,%s'
                   % (",".join(refs), len(refs), value, footprint, mfg, mpn, lcsc, side, kind))
    return "\n".join(out) + "\n"


README = """%(title)s for AISLER
%(rule)s

Upload %(project)s.kicad_pcb (native KiCad board, zones filled, parts carry
MPN and MFG fields) or %(project)s-odb.zip (ODB++ of the same board).  AISLER
reads the components from either; no BOM or placement file is needed.

Board options
  2 layers, 1.6 mm FR-4, 1 oz copper, HASL or ENIG, green mask.
  %(fab)s

Assembly
  %(notes)s
%(bom)s

Source: hardware/pcb/V1 (%(generator)s); these files were written by
scripts/export_aisler.py.
"""

FAB = {
    "HE": "Meets AISLER's \"2 layer 1.6 mm HASL\" rules: track 0.25 / spacing 0.2 mm,\n"
          "  vias 0.7 mm with a 0.3 mm hole (0.2 mm ring), copper to edge 0.3 mm.",
    "MX": "Vias are 0.6 mm with a 0.3 mm hole (0.15 mm ring): below AISLER's HASL\n"
          "  rule of 0.2 mm, so order ENIG or ask whether they accept it.",
    "Carrier": "Vias are 0.6 mm with a 0.3 mm hole (0.15 mm ring): below AISLER's HASL\n"
               "  rule of 0.2 mm, so order ENIG or ask whether they accept it.",
}


def export(directory, project):
    cli = shutil.which("kicad-cli")
    if not cli:
        sys.exit("kicad-cli not found")
    src = os.path.join(V1, directory)
    out = os.path.join(src, "production", "aisler")
    if os.path.isdir(out):
        shutil.rmtree(out)
    os.makedirs(out)

    tree = parse(open(os.path.join(src, project + ".kicad_pcb"), encoding="utf8").read())
    title = find(find(tree, "title_block") or [], "title")
    title = title[1].strip('"') if title else project
    prepare(tree)
    board = os.path.join(out, project + ".kicad_pcb")
    with open(board, "w", newline="\n", encoding="utf8") as f:
        f.write(dump(tree) + "\n")
    shutil.copy(os.path.join(src, project + ".kicad_pro"), os.path.join(out, project + ".kicad_pro"))

    # Fill the zones and let KiCad save the board (that also normalises the
    # file), then check the saved copy with the project's rules.
    report = os.path.join(out, "drc.tmp.json")
    run([cli, "pcb", "drc", "--refill-zones", "--save-board", "--format", "json", "-o", report, board])
    run([cli, "pcb", "drc", "--format", "json", "--severity-error", "--exit-code-violations",
         "-o", report, board])
    os.remove(report)
    prl = os.path.join(out, project + ".kicad_prl")
    if os.path.exists(prl):
        os.remove(prl)

    odb = os.path.join(out, project + "-odb.zip")
    run([cli, "pcb", "export", "odb", "--check-zones", "-o", odb, board])
    pdf = os.path.join(out, project + "-schematic.pdf")
    run([cli, "sch", "export", "pdf", "-o", pdf, os.path.join(src, project + ".kicad_sch")])

    rows = parts(parse(open(board, encoding="utf8").read()))
    bom = os.path.join(out, project + "-BOM.csv")
    with open(bom, "w", newline="\n") as f:
        f.write(bom_csv(rows))
    lines = ["    %-12s %s %s (%s, %s, %s%s)" % (",".join(refs), mfg, mpn, value, side, kind,
                                                  ", LCSC " + lcsc if lcsc else "")
             for refs, value, _fp, mfg, mpn, lcsc, side, kind in rows]
    readme = os.path.join(out, "README.txt")
    with open(readme, "w", newline="\n") as f:
        f.write(README % dict(title=title, rule="=" * (len(title) + 11), project=project,
                              fab=FAB[directory], notes=NOTES[directory], bom="\n".join(lines),
                              generator="he_board.py" if directory == "HE" else "generate_boards.py"))

    archive = os.path.join(out, project + "-aisler.zip")
    files = [board, os.path.join(out, project + ".kicad_pro"), odb, pdf, bom, readme]
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as z:
        for path in files:
            z.write(path, os.path.basename(path))
    for path in files + [archive]:
        print("wrote %s (%d bytes)" % (os.path.relpath(path, V1), os.path.getsize(path)))


def main():
    wanted = sys.argv[1:] or [d for d, _ in BOARDS]
    if "HE" in wanted:
        import he_board
        if he_board.selftest() or he_board.check():
            sys.exit("he_board.py reports problems; refusing to export the HE module")
    for directory, project in BOARDS:
        if directory in wanted:
            export(directory, project)


if __name__ == "__main__":
    main()
