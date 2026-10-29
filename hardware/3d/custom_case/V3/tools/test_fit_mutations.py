#!/usr/bin/env python3
"""Break the V3 case on purpose and check that check_fit.py notices.

Each case overrides one parameter so that something real would collide, and
expects check_fit to report an OVERLAP on the named pair. The unmodified
case must be clear. Needs OpenSCAD; about two minutes on a fast machine.

Usage: python3 hardware/3d/custom_case/V3/tools/test_fit_mutations.py
"""

import os
import subprocess
import sys

TOOLS = os.path.dirname(os.path.abspath(__file__))

CASES = [
    ("unmodified case", [], None),
    ("stack 0.6 mm off the floor (V1.1's value): the cable cannot pass under it", ["FLOOR_CLR=0.6"], "cable"),
    ("case 80 mm wide: the USB cable runs into the right-front carrier post", ["CASE_W=80"], "usb_cable"),
    ("glass pocket smaller than the glass", ["POCKET_CLR=-0.2"], "ws_glass"),
    ("sockets 2.5 mm shorter: the carrier's top-side parts hit the Waveshare's back", ["SOCKET_H=6.0"], "ws_back"),
    ("cable channel too narrow for the 15 mm cable", ["GUIDE_GAP=-0.3"], "cable"),
    ("cable channel walls 2 mm high: they hit the socket tails under the stack", ["GUIDE_H=2.0", "GUIDE_LIP_T=0.3"], "tails"),
    ("module pins wider than the module's 2.2 mm holes", ["MODULE_PIN_D=2.5"], "mod_pcb"),
    ("carrier pins wider than the carrier's 2.2 mm holes", ["CAR_PIN_D=2.5"], "carrier"),
    ("USB board 0.6 mm higher: J2 hits the front of the carrier's lug", ["USB_STANDOFF=2.1"], "usb_j2"),
    ("USB board 1.1 mm lower: the jack's shell legs hit the floor", ["USB_STANDOFF=0.4"], "usb_legs"),
    ("USB opening smaller than the plug's overmold", ["USB_OPEN_D=6.0"], "usb_plug"),
    ("USB hold-down blocks 0.5 mm into the board", ["USB_CLAMP=-0.5"], "usb_pcb"),
]


def main():
    bad = 0
    for name, defines, expect in CASES:
        args = [sys.executable, os.path.join(TOOLS, "check_fit.py")]
        for d in defines:
            args += ["-D", d]
        r = subprocess.run(args, capture_output=True, text=True)
        overlaps = [l for l in r.stdout.splitlines() if "OVERLAP" in l or "ERROR" in l or "empty" in l]
        if expect is None:
            ok = r.returncode == 0 and not overlaps
        else:
            ok = r.returncode != 0 and any(l.startswith(expect + " ") or (" " + expect + " ") in l or
                                          ("ref:" + expect) in l for l in overlaps)
        print("%s  %s" % ("PASS" if ok else "FAIL", name))
        for l in overlaps[:6]:
            print("        " + l)
        bad += not ok
    print("%d of %d cases behave as expected" % (len(CASES) - bad, len(CASES)))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
