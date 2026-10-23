#!/usr/bin/env python3
"""Check and export the OPad V2 boards for JLCPCB / PCBWay.

The same checks and outputs as V1's export_production.py (ERC, DRC with
schematic parity and refilled zones, stop on any problem; Gerbers, drills,
BOM, CPL, PCBWay variant, PDFs, STEP, renders), which this reuses, for the
boards in hardware/pcb/V2/.

Usage: python3 hardware/pcb/V2/scripts/export_v2.py [HE]   (default: all)
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
V2 = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(os.path.dirname(V2), "V1", "scripts"))
import export_production as ep  # noqa: E402

ep.V1 = V2
ep.BOARDS = [("HE", "he_input_v2")]
# Every footprint is placed straight from V2/lib, so a library mismatch here
# is a real difference and is not accepted
ep.ACCEPTED_WARNINGS = set()
ep.ACCEPTED_ERC = set()
# JLCPCB's WQFN-16 model has pin 1 in the opposite corner: its preview put
# the pin-1 marker diagonally across from the silkscreen dot (2026-10-23)
ep.JLC_CPL_FIXES["WQFN-16-1EP_3x3mm_P0.5mm_EP1.68mm"] = 180
ep.PCBWAY_NOTES["WQFN-16-1EP_3x3mm_P0.5mm_EP1.68mm"] = "BOTTOM side. Exposed pad soldered to GND; pin 1 at the silkscreen dot"
ep.PCBWAY_NOTES["SOT-23"] = "BOTTOM side. Single lead (pin 3) towards the board's front edge, as in the CPL"

if __name__ == "__main__":
    ep.main()
