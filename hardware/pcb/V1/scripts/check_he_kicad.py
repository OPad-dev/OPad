#!/usr/bin/env python3
"""Run KiCad's own ERC and DRC on the generated he_input_v1 files.

    python3 hardware/pcb/V1/scripts/check_he_kicad.py

Needs ``kicad-cli`` (KiCad 8 or later) on the PATH.  The zones are refilled
before the DRC so the pours count as copper, and the board is checked against
the schematic (parity).  Exits non-zero on any ERC or DRC *error*; warnings are
listed but do not fail the run.  This complements he_board.py's own rules with
an independent checker; neither can see a mirrored package, which is what the
selftest in he_board.py is for.
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile

import he_board as B

V1 = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
HE = os.path.join(V1, "HE")


def run(args):
    proc = subprocess.run(args, capture_output=True, text=True)
    return proc.returncode, proc.stdout + proc.stderr


def main():
    cli = shutil.which("kicad-cli")
    if not cli:
        sys.exit("kicad-cli not found; install KiCad or skip this check")
    tmp = tempfile.mkdtemp(prefix="he_kicad_")
    sch = os.path.join(HE, B.PROJECT + ".kicad_sch")
    pcb = os.path.join(HE, B.PROJECT + ".kicad_pcb")
    errors = 0

    erc = os.path.join(tmp, "erc.json")
    code, out = run([cli, "sch", "erc", "--format", "json", "--severity-all", "-o", erc, sch])
    if not os.path.exists(erc):
        sys.exit("ERC did not run (does the schematic load?):\n" + out)
    report = json.load(open(erc))
    for sheet in report.get("sheets", []):
        for v in sheet.get("violations", []):
            print("ERC %-7s %s: %s" % (v["severity"], v["type"], v["description"]))
            errors += v["severity"] == "error"

    drc = os.path.join(tmp, "drc.json")
    code, out = run([cli, "pcb", "drc", "--format", "json", "--severity-all",
                     "--refill-zones", "--schematic-parity", "-o", drc, pcb])
    if not os.path.exists(drc):
        sys.exit("DRC did not run:\n" + out)
    report = json.load(open(drc))
    for key in ("violations", "unconnected_items", "schematic_parity"):
        for v in report.get(key, []):
            where = "; ".join(i.get("description", "") for i in v.get("items", []))
            print("DRC %-7s %s: %s [%s]" % (v["severity"], v["type"], v["description"], where))
            errors += v["severity"] == "error"

    shutil.rmtree(tmp, ignore_errors=True)
    print("kicad-cli %s: %d error(s)" % (run([cli, "version"])[1].strip(), errors))
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
