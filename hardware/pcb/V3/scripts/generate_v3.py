#!/usr/bin/env python3
"""Generate the OPad V3 boards: carrier, MX module and HE module (library, schematics, placed boards).

Design: docs/specs/hardware-redesign-brief.md sections 4.1 and 4.2. In short:

- One cable: a 14-way 1.0 mm FFC between the carrier and the module, wired so
  a reversed cable only swaps signals between ESP32 pins:
      1 GND | 2 5V | 3 3V3 | 4 ID | 5 MISO | 6 GND | 7 SCLK | 8 MOSI | 9 GND |
      10 CS | 11 SPARE | 12 3V3 | 13 5V | 14 GND
- The USB-C is on the carrier, on a tab past the Waveshare's USB end, mouth on
  the rear edge, with ESD, VBUS filtering and a Schottky against back-feed.
- Both modules: three keys at 19.05 mm, four corner holes, everything on the
  bottom. MX: Kailh sockets, ID 2.24 V. HE: DRV5055A3 x3, ADS7038, an LP5907
  LDO from the cable's 5 V for the sensors and the ADC reference, ID 1.65 V.
- Polarised bottom-side parts sit at 0 or 180 deg only (JLCPCB's direction of
  rotation on the bottom is not established, hardware/verify/opadverify/jlc.py).

    python3 hardware/pcb/V3/scripts/generate_v3.py [carrier] [mx] [he]   (default: all)
Then route_v3.py and export_v3.py.
"""

import json
import os
import re
import sys

import pcbnew

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import v3lib  # noqa: E402

V3 = v3lib.V3
sys.path.insert(0, os.path.join(v3lib.PCB, "V1", "scripts"))
import generate_boards as g1  # noqa: E402

REV = "V3.0"
DATE = "2026-10-23"
g1.FP_LIB = v3lib.FP_LIB
g1.REV = REV
g1.DATE = DATE
P = g1.Part

# -----------------------------------------------------------------------------
# Parts (LCSC numbers checked against JLCPCB's catalogue by hardware/verify)
# -----------------------------------------------------------------------------
LCSC = {
    "R10K": ("10k", "C25804", "UNI-ROYAL 0603WAF1002T5E"),
    "R47K": ("47k", "C25819", "UNI-ROYAL 0603WAF4702T5E"),
    "R100K": ("100k", "C25803", "UNI-ROYAL 0603WAF1003T5E"),
    "R5K1": ("5.1k", "C23186", "UNI-ROYAL 0603WAF5101T5E"),
    "R1M": ("1M", "C22935", "UNI-ROYAL 0603WAF1004T5E"),
    "R33": ("33", "C23140", "UNI-ROYAL 0603WAF330JT5E"),
    "R220": ("220", "C22962", "UNI-ROYAL 0603WAF2200T5E"),
    "C100N": ("100nF", "C14663", "YAGEO CC0603KRX7R9BB104"),
    "C1U": ("1uF", "C15849", "Samsung CL10A105KB8NNNC"),
    "C4U7": ("4.7uF", "C19666", "Samsung CL10A475KO8NNNC"),
    "C10U": ("10uF", "C19702", "Samsung CL10A106KP8NNNC"),
    "C10N": ("10nF", "C57112", "FH 0603B103K500NT"),
    "C4N7": ("4.7nF", "C53987", "FH 0603B472K500NT"),
    "FB600": ("600R@100MHz", "C1002", "Sunlord GZ1608D601TF"),
    "FB120": ("120R@100MHz 2A", "C14709", "Murata BLM18PG121SN1D"),
    "DRV": ("DRV5055A3", "C266128", "Texas Instruments DRV5055A3QDBZR"),
    # ADS7028: the ADS7038's sibling (same RTE pinout, registers, averaging and CRC) that JLCPCB
    # stocks for assembly; the ADS7038 (C2876384) is only on LCSC and would need a pre-order
    "ADC": ("ADS7028", "C880341", "Texas Instruments ADS7028IRTER"),
    "LDO": ("LP5907-3.3", "C80670", "Texas Instruments LP5907MFX-3.3/NOPB"),
    "ESD": ("SRV05-4", "C85364", "ProTek SRV05-4-P-T7"),
    "SCHOTTKY": ("B5817W", "C7420328", "R+O B5817W"),
    "USBC": ("TYPE-C-31-M-12", "C165948", "Korean Hroparts TYPE-C-31-M-12"),
    "FFC": ("AFA07-S14FCA-00", "C262720", "JUSHUO AFA07-S14FCA-00"),
    "KAILH": ("CPG151101S11", "C41430893", "Kailh CPG151101S11"),
    # V1's C2897377 is no longer in JLCPCB's assembly stock; same 8.5 mm socket, gold
    "SOCKET": ("FH2.54-09-14PZD", "C7500781", "XUNPU FH2.54-09-14PZD"),
}


def part(ref, kind, symbol, footprint, pins, pcb, side, rot, description, value=None):
    v, lcsc, mpn = LCSC[kind]
    return P(ref, symbol, value or v, footprint, pins, None, pcb, side, rot, lcsc, mpn, description)


def two(ref, kind, a, b, pcb, side, rot, description):
    fp = {"R": "R_0603_1608Metric", "C": "C_0603_1608Metric", "F": "L_0603_1608Metric"}[ref[0]]
    sym = {"R": "R", "C": "C", "F": "FerriteBead"}[ref[0]]
    return part(ref, kind, sym, fp, {"1": a, "2": b}, pcb, side, rot, description)


# The module cable, the same numbering on both ends (brief 4.2)
FFC_PINS = {"1": "GND", "2": "5V", "3": "3V3", "4": "ID", "5": "MISO", "6": "GND", "7": "SCLK",
            "8": "MOSI", "9": "GND", "10": "CS", "11": "SPARE", "12": "3V3", "13": "5V", "14": "GND",
            "MP": "GND"}


# -----------------------------------------------------------------------------
# Schematic (labels on every pin; the board is the drawing that matters)
# -----------------------------------------------------------------------------

def write_schematic(directory, project, title, parts, notes, flags):
    text = open(v3lib.SYM_LIB).read()
    sym = {}
    for name in {p.symbol for p in parts} | {"PWR_FLAG"}:
        block = g1.sexpr_block(text, text.index('(symbol "%s"' % name))
        pins = {}
        for m in re.finditer(r'\(pin\s+\w+\s+\w+\s*\(at ([-\d.]+) ([-\d.]+) ([-\d.]+)\).*?\(number "([^"]+)"',
                             block, re.S):
            pins.setdefault(m.group(4), []).append((float(m.group(1)), float(m.group(2)), int(float(m.group(3)))))
        sym[name] = (block.replace('(symbol "%s"' % name, '(symbol "osupad:%s"' % name, 1), pins)
    root = g1.uid()
    out = ['(kicad_sch\n\t(version 20250114)\n\t(generator "eeschema")\n\t(generator_version "9.0")\n',
           '\t(uuid "%s")\n\t(paper "A2")\n' % root,
           '\t(title_block (title "%s") (date "%s") (rev "%s") (company "OPad"))\n' % (title, DATE, REV),
           '\t(lib_symbols\n']
    for name in sorted(sym):
        out.append("\t\t" + sym[name][0] + "\n")
    out.append("\t)\n")
    for i, note in enumerate(notes):
        out.append('\t(text "%s" (exclude_from_sim no) (at 20.32 %.2f 0) (effects (font (size 1.27 1.27)) '
                   '(justify left bottom)) (uuid "%s"))\n' % (note.replace('"', "'"), 330.0 + i * 3.81, g1.uid()))
    for i, net in enumerate(flags):
        sx, sy = 25.4 + 12.7 * i, 20.32
        ref = "#FLG%02d" % (i + 1)
        out.append('\t(symbol (lib_id "osupad:PWR_FLAG") (at %.2f %.2f 0) (unit 1) (exclude_from_sim no) '
                   '(in_bom no) (on_board yes) (dnp no) (uuid "%s")\n' % (sx, sy, g1.uid()))
        out.append(g1.prop("Reference", ref, sx, sy - 3.81, hide=True))
        out.append(g1.prop("Value", "PWR_FLAG", sx, sy - 5.08))
        out.append(g1.prop("Footprint", "", sx, sy, hide=True))
        out.append('\t\t(pin "1" (uuid "%s"))\n' % g1.uid())
        out.append('\t\t(instances (project "%s" (path "/%s" (reference "%s") (unit 1))))\n\t)\n' % (project, root, ref))
        out.append('\t(label "%s" (at %.2f %.2f 0) (effects (font (size 1.27 1.27)) (justify left bottom)) (uuid "%s"))\n'
                   % (net, sx, sy, g1.uid()))
    for i, p in enumerate(parts):
        sx, sy = 38.1 + 60.96 * (i % 9), 45.72 + 45.72 * (i // 9)
        p.sch = (sx, sy)
        pins = sym[p.symbol][1]
        out.append('\t(symbol (lib_id "osupad:%s") (at %.2f %.2f 0) (unit 1) (exclude_from_sim no) '
                   '(in_bom %s) (on_board yes) (dnp %s) (uuid "%s")\n'
                   % (p.symbol, sx, sy, "no" if p.symbol == "TestPoint" else "yes",
                      "yes" if p.symbol == "TestPoint" else "no", p.uuid))
        ys = [y for v in pins.values() for _, y, _ in v]
        out.append(g1.prop("Reference", p.ref, sx, sy - max(ys) - 3.81))
        out.append(g1.prop("Value", p.value, sx, sy - min(ys) + 3.81))
        out.append(g1.prop("Footprint", "osupad:" + p.footprint, sx, sy, hide=True))
        out.append(g1.prop("Datasheet", "", sx, sy, hide=True))
        out.append(g1.prop("Description", p.description, sx, sy, hide=True))
        out.append(g1.prop("LCSC", p.lcsc, sx, sy, hide=True))
        out.append(g1.prop("MPN", p.mpn, sx, sy, hide=True))
        for number in pins:
            out.append('\t\t(pin "%s" (uuid "%s"))\n' % (number, g1.uid()))
        out.append('\t\t(instances (project "%s" (path "/%s" (reference "%s") (unit 1))))\n\t)\n'
                   % (project, root, p.ref))
        for number, places in pins.items():
            for px, py, angle in places:
                x, y = sx + px, sy - py
                net = p.pins.get(number)
                if net is None:
                    out.append('\t(no_connect (at %.2f %.2f) (uuid "%s"))\n' % (x, y, g1.uid()))
                    continue
                la = (angle + 180) % 360
                justify = {0: "left bottom", 90: "left bottom", 180: "right bottom", 270: "right bottom"}[la]
                out.append('\t(label "%s" (at %.2f %.2f %d) (effects (font (size 1.27 1.27)) (justify %s)) (uuid "%s"))\n'
                           % (net, x, y, la, justify, g1.uid()))
    out.append('\t(sheet_instances (path "/" (page "1")))\n)\n')
    with open(os.path.join(directory, project + ".kicad_sch"), "w") as f:
        f.write("".join(out))


# -----------------------------------------------------------------------------
# Board helpers
# -----------------------------------------------------------------------------

def keepout(bb, x, y, half, name="sensor keepout"):
    z = pcbnew.ZONE(bb.board)
    z.SetIsRuleArea(True)
    z.SetDoNotAllowZoneFills(True)
    z.SetDoNotAllowTracks(False)
    z.SetDoNotAllowVias(False)
    z.SetDoNotAllowPads(False)
    z.SetDoNotAllowFootprints(False)
    z.SetLayerSet(pcbnew.LSET.AllCuMask())
    ol = z.Outline()
    ol.NewOutline()
    for px, py in ((x - half, y - half), (x + half, y - half), (x + half, y + half), (x - half, y + half)):
        ol.Append(pcbnew.FromMM(px), pcbnew.FromMM(py))
    z.SetZoneName(name)
    bb.board.Add(z)


def pads_of(bb, ref):
    return {p.GetNumber(): (pcbnew.ToMM(p.GetPosition().x), pcbnew.ToMM(p.GetPosition().y))
            for p in bb.fps[ref].Pads() if p.GetNumber()}


def pin_names():
    """{symbol: {pin number: pin name}} from the V3 symbol library."""
    text = open(v3lib.SYM_LIB).read()
    out = {}
    for m in re.finditer(r'\(symbol "([^"_]+(?:_[A-Za-z][^"_]*)?)"\n', text):
        block = g1.sexpr_block(text, m.start())
        out[m.group(1)] = {n: name for name, n in re.findall(r'\(name "([^"]*)".*?\(number "([^"]+)"', block, re.S)}
    return out


def build(name, project, title, parts, outline, holes, notes, flags, extra=None, rules=None, stitch=()):
    directory = os.path.join(V3, name)
    os.makedirs(directory, exist_ok=True)
    write_schematic(directory, project, title, parts, notes, flags)
    bb = g1.BoardBuilder(project)
    bb.outline(outline, 1.0 if name == "Carrier" else 1.5)
    sheet = project + ".kicad_sch"
    names = pin_names()
    for p in parts:
        fp = bb.part(p, sheet)
        for pad in fp.Pads():
            n = pad.GetNumber()
            if n and n in names.get(p.symbol, {}) and p.pins.get(n) is None:
                pin = names[p.symbol][n]
                label = "%s-Pad%s" % (p.ref, n) if pin == n else "%s-%s-Pad%s" % (p.ref, pin, n)
                pad.SetNet(bb.net("unconnected-(%s)" % label))
        if p.symbol == "TestPoint":
            fp.SetExcludedFromBOM(True)
            fp.SetExcludedFromPosFiles(True)
            fp.SetDNP(True)
    for i, (x, y) in enumerate(holes):
        bb.hole("H%d" % (i + 1), x, y)
    if extra:
        extra(bb)
    xs = [p[0] for p in outline]
    ys = [p[1] for p in outline]
    for layer in (pcbnew.F_Cu, pcbnew.B_Cu):
        bb.zone("GND", layer, (min(xs), min(ys), max(xs), max(ys)), clearance=0.25)
    aux = (min(xs), max(ys))
    path = bb.save(directory, title, aux)
    b = pcbnew.LoadBoard(path)
    nc = b.GetDesignSettings().m_NetSettings.GetDefaultNetclass()
    nc.SetClearance(pcbnew.FromMM(0.15))
    nc.SetTrackWidth(pcbnew.FromMM(0.2))
    b.Save(path)
    g1.write_project(directory, project)
    pro_path = os.path.join(directory, project + ".kicad_pro")
    pro = json.load(open(pro_path))
    pro["net_settings"]["classes"][0].update(clearance=0.15, track_width=0.2)
    pro["board"]["design_settings"]["track_widths"] = [0.0, 0.2, 0.3, 0.5]
    json.dump(pro, open(pro_path, "w"), indent=2)
    json.dump({"stitch": list(stitch), "power": rules or {}}, open(os.path.join(directory, "route.json"), "w"), indent=1)
    print("wrote", path)
    return path


# -----------------------------------------------------------------------------
# Carrier
# -----------------------------------------------------------------------------
# KiCad frame of the V1 carrier (hardware/pcb/V1/README.md): the Waveshare
# outline is x 100..135, y 100..148.2; P2 (front) at x 102.26, P1 (rear) at
# x 132.74, pin 1 at y 107.59 at the non-USB end (small y). Case: KiCad +X is
# the rear, +Y the right (USB end). The new tab past y 148.2 carries the USB-C
# with its mouth on the rear edge x = 135.

P1X, P2X, PIN1Y = 132.74, 102.26, 107.59
HY = {n: PIN1Y + 2.54 * (n - 1) for n in range(1, 15)}
USB_C = (135.0 - v3lib.USB_EDGE_Y, 154.0)          # mouth on the rear edge
JMOD = (117.5, 100.3 + v3lib.FFC_MOUTH_Y)          # mouth 0.3 mm inside the left edge


def carrier():
    p1 = {1: "GPIO2", 2: "GPIO4", 3: "GPIO6", 8: "GPIO8", 9: "GPIO7", 10: "GPIO10", 11: "USB_DP",
          12: "USB_DN", 13: "GND", 14: "5V"}
    p2 = {1: "3V3", 2: "GND", 13: "GND"}
    jmod = {k: {"ID": "GPIO8", "SPARE": "GPIO7", "MISO": "M_MISO", "SCLK": "M_SCLK", "MOSI": "M_MOSI",
                "CS": "M_CS", "5V": "5V_MOD"}.get(v, v) for k, v in FFC_PINS.items()}
    T = "top"
    parts = [
        part("J_P1", "SOCKET", "Socket14", "PinSocket_1x14_P2.54mm_Vertical",
             {str(n): p1.get(n) for n in range(1, 15)}, (P1X, PIN1Y), T, 0,
             "Socket for Waveshare header P1 (rear)", "WAVESHARE_P1"),
        part("J_P2", "SOCKET", "Socket14", "PinSocket_1x14_P2.54mm_Vertical",
             {str(n): p2.get(n) for n in range(1, 15)}, (P2X, PIN1Y), T, 0,
             "Socket for Waveshare header P2 (front)", "WAVESHARE_P2"),
        part("J_MOD", "FFC", "FFC14", v3lib.FFC14, jmod, JMOD, T, 180,
             "Module cable (14-way FFC), cable leaves towards the case's left wall"),
        part("J_USB", "USBC", "USB_C", v3lib.USBC,
             {"A1B12": "GND", "B1A12": "GND", "A4B9": "VBUS", "B4A9": "VBUS", "A5": "CC1", "B5": "CC2",
              "A6": "USB_DP", "B6": "USB_DP", "A7": "USB_DN", "B7": "USB_DN", "A8": None, "B8": None,
              "S1": "SHIELD", "S2": "SHIELD", "S3": "SHIELD", "S4": "SHIELD"},
             USB_C, T, 90, "USB-C, mouth on the rear edge"),
        two("R2", "R5K1", "CC2", "GND", (124.8, 150.0), T, 0, "CC2 pull-down"),
        part("U1", "ESD", "SRV05-4", v3lib.SOT236,
             {"1": "USB_DP", "2": "GND", "3": "USB_DN", "4": None, "5": "VBUS", "6": None},
             (124.6, 154.0), T, 90, "ESD on D+/D- and VBUS"),
        two("R1", "R5K1", "CC1", "GND", (124.8, 158.0), T, 0, "CC1 pull-down: a USB-C sink"),
        two("C2", "C100N", "VBUS", "GND", (121.2, 150.2), T, 90, "VBUS at the connector"),
        two("FB1", "FB120", "VBUS", "VBUS_F", (121.2, 154.0), T, 90, "VBUS filter, 2 A"),
        two("C3", "C4U7", "VBUS_F", "GND", (121.2, 157.8), T, 90, "VBUS after the filter"),
        part("D1", "SCHOTTKY", "D_Schottky", v3lib.SOD123, {"1": "VBUS_F", "2": "5V"}, (119.2, 154.0), T, 90,
             "No back-feed from the Waveshare's own USB port into this one"),
        two("C1", "C4N7", "SHIELD", "GND", (123.6, 161.2), T, 0, "Shield to ground, RF path"),
        two("R3", "R1M", "SHIELD", "GND", (126.8, 161.2), T, 0, "Shield to ground, DC path"),
        # Module side, on the bridge at the non-USB end
        two("FB2", "FB600", "5V", "5V_MOD", (129.6, 138.6), T, 90, "5 V to the module, filtered (next to P1-14)"),
        two("C4", "C1U", "5V_MOD", "GND", (107.8, 109.4), T, 90, "5 V to the module"),
        two("C5", "C10U", "3V3", "GND", (112.4, 109.4), T, 90, "3V3 to the module, bulk"),
        two("C6", "C100N", "3V3", "GND", (114.2, 109.4), T, 90, "3V3 to the module"),
        part("U2", "ESD", "SRV05-4", v3lib.SOT236,
             {"1": "M_SCLK", "2": "GND", "3": "M_MOSI", "4": "M_MISO", "5": "3V3", "6": "M_CS"},
             (118.0, 110.2), T, 0, "ESD on the module's SPI / key lines"),
        part("U3", "ESD", "SRV05-4", v3lib.SOT236,
             {"1": "GPIO8", "2": "GND", "3": "GPIO7", "4": None, "5": "3V3", "6": None},
             (122.6, 110.2), T, 0, "ESD on the module's ID and spare lines"),
        two("R4", "R33", "GPIO2", "M_SCLK", (129.0, 107.6), T, 0, "Series, GPIO2"),
        two("R5", "R33", "GPIO4", "M_MOSI", (129.0, 109.4), T, 0, "Series, GPIO4"),
        two("R6", "R33", "GPIO6", "M_MISO", (129.0, 111.2), T, 0, "Series, GPIO6"),
        two("R7", "R33", "GPIO10", "M_CS", (129.0, 113.0), T, 0, "Series, GPIO10"),
    ]
    # The tab: the rear rail continues past the Waveshare's USB end to y = 162.5 and widens to x = 118
    outline = [(100.0, 100.0), (135.0, 100.0), (135.0, 162.5), (118.0, 162.5), (118.0, 148.2),
               (128.0, 148.2), (128.0, 115.0), (105.2, 115.0), (105.2, 148.2), (100.0, 148.2)]
    holes = [(103.0, 103.0), (132.0, 103.0), (103.0, 145.2), (132.0, 145.2)]
    notes = ["OPad carrier V3: Waveshare headers, USB-C (rear, past the USB end), module FFC.",
             "Module FFC: 1 GND | 2 5V | 3 3V3 | 4 ID=GPIO8 | 5 MISO=GPIO6 | 6 GND | 7 SCLK=GPIO2 | 8 MOSI=GPIO4 | "
             "9 GND | 10 CS=GPIO10 | 11 SPARE=GPIO7 | 12 3V3 | 13 5V | 14 GND; a reversed cable maps k to 15-k.",
             "USB: VBUS -> FB1 -> D1 -> 5V (Waveshare P1-14); D+ -> P1-11 (GPIO20), D- -> P1-12 (GPIO19)."]

    def extra(bb):
        S, F = pcbnew.F_SilkS, pcbnew.F_Fab
        bb.text("WAVESHARE ON THIS SIDE", 116.5, 114.3, S, 0.8)
        bb.text("OPad carrier " + REV, 116.5, 103.0, pcbnew.B_SilkS, 0.8)
        bb.text("JLCJLCJLCJLC", 116.5, 111.0, pcbnew.B_SilkS, 0.8)
        p = pads_of(bb, "J_MOD")
        assert p["MP"][1] < p["1"][1], "J_MOD: the cable entry must face the left edge"
        u = pads_of(bb, "J_USB")
        assert u["S3"][0] > u["S1"][0], "J_USB: the mouth must face the rear edge"

    stitch = [(101.5, 101.5), (133.5, 101.5), (101.5, 113.5), (106.5, 113.6), (126.5, 113.6),
              (101.5, 120.0), (101.5, 130.0), (101.5, 140.0), (130.0, 120.0), (130.0, 125.0),
              (130.0, 140.0), (119.0, 149.5), (119.0, 161.5), (131.0, 161.5)]
    return build("Carrier", "carrier_v3", "OPad Controller Carrier V3", parts, outline, holes, notes,
                 ["GND", "3V3", "5V", "VBUS"], extra, {"VBUS": 0.4, "VBUS_F": 0.4, "5V": 0.4, "GND": 0.3},
                 stitch)


# -----------------------------------------------------------------------------
# Modules (same frame as V1/V2: rear edge y = 69, keys on y = 83.5)
# -----------------------------------------------------------------------------
PITCH = 19.05
KEY_Y = 83.5
X0, Y0 = 112.0, 69.0
KEYS = [X0 + 16.475 + PITCH * i for i in range(3)]       # 128.475, 147.525, 166.575
X1, Y1 = KEYS[2] + 16.475, Y0 + 24.0                       # 183.05, 93.0
MOD_HOLES = [(X0 + 3.5, Y0 + 3.5), (X1 - 3.5, Y0 + 3.5), (X0 + 3.5, Y1 - 3.5), (X1 - 3.5, Y1 - 3.5)]
J1_POS = (KEYS[1], Y0 + 0.3 + v3lib.FFC_MOUTH_Y)           # mouth 0.3 mm inside the rear edge
MOD_OUTLINE = [(X0, Y0), (X1, Y0), (X1, Y1), (X0, Y1)]


def module_common(bb, label):
    S = pcbnew.B_SilkS
    bb.text(label + " " + REV, KEYS[1], 91.4, S, 0.9)
    for k, x in enumerate(KEYS, 1):
        bb.text("KEY%d" % k, x, 89.0, S, 0.8)
    bb.text("JLCJLCJLCJLC", 124.0, 71.0, S, 0.8)
    p = pads_of(bb, "J1")
    assert p["MP"][1] < p["1"][1], "J1: the cable entry must face the rear edge"


def mx():
    pins = {k: v for k, v in FFC_PINS.items()}
    pins.update({"2": None, "13": None, "7": None, "8": None, "10": "KEY1", "11": "KEY2", "5": "KEY3"})
    parts = [part("J1", "FFC", "FFC14", v3lib.FFC14, pins, J1_POS, "bottom", 0, "Module cable (14-way FFC)")]
    for k, x in enumerate(KEYS, 1):
        parts.append(part("SW%d" % k, "KAILH", "KeySwitch", "Kailh_CPG151101S11_MX_Hotswap_Bottom",
                          {"1": "GND", "2": "KEY%d" % k},
                          (x, KEY_Y), "bottom", 180, "Key %d hot-swap socket" % k, "KEY%d" % k))
    parts += [
        two("R1", "R10K", "3V3", "KEY1", (132.6, 74.6), "bottom", 0, "KEY1 pull-up"),
        two("R2", "R10K", "3V3", "KEY2", (161.0, 73.0), "bottom", 0, "KEY2 pull-up"),
        two("R3", "R10K", "3V3", "KEY3", (161.0, 74.6), "bottom", 0, "KEY3 pull-up"),
        two("R4", "R47K", "3V3", "ID", (132.6, 73.0), "bottom", 0, "ID divider top, 47k"),
        two("R5", "R100K", "ID", "GND", (132.6, 71.4), "bottom", 0, "ID divider bottom, 100k: 2.24 V"),
        two("C1", "C100N", "3V3", "GND", (161.0, 71.4), "bottom", 0, "3V3 at the connector"),
    ]
    notes = ["OPad MX module V3: three Kailh hot-swap keys; KEY1 = cable 10 (GPIO10), KEY2 = 11 (GPIO7), "
             "KEY3 = 5 (GPIO6); ID = 3V3 x 100k / 147k = 2.24 V on cable 4 (GPIO8)."]
    return build("MX", "mx_input_v3", "OPad MX Input Module V3", parts, MOD_OUTLINE, MOD_HOLES, notes,
                 ["GND", "3V3"], lambda bb: module_common(bb, "OPad MX module"), {"GND": 0.4, "3V3": 0.3},
                 [(114.0, 91.5), (126.0, 91.5), (140.0, 91.5), (155.0, 91.5), (170.0, 91.5), (181.0, 91.5),
                  (114.0, 80.0), (181.0, 80.0), (120.0, 71.0), (175.0, 71.0)])


def he():
    pins = dict(FFC_PINS)
    pins.update({"2": "5V", "13": "5V", "11": None})
    B = "bottom"
    parts = [part("J1", "FFC", "FFC14", v3lib.FFC14, pins, J1_POS, B, 0, "Module cable (14-way FFC)")]
    for k, x in enumerate(KEYS, 1):
        parts += [
            part("U%d" % k, "DRV", "DRV5055", "SOT-23", {"1": "AVDD", "2": "OUT_K%d" % k, "3": "GND"},
                 (x, KEY_Y), B, 90, "Ratiometric linear Hall sensor, key %d" % k),
            two("C%d" % (5 + k), "C100N", "AVDD", "GND", (x - 3.0, 81.0), B, 90, "Sensor %d supply" % k),
        ]
    ux, uy = 168.0, 73.6
    parts += [
        part("U4", "ADC", "ADS7038", "WQFN-16-1EP_3x3mm_P0.5mm_EP1.68mm",
             {"7": "AVDD", "10": "3V3", "8": "DECAP", "11": "CS", "13": "SCLK", "14": "MOSI", "12": "SDO",
              "9": "GND", "17": "GND", "4": "AIN_K1", "3": "AIN_K2", "2": "AIN_K3",
              "15": None, "16": None, "1": None, "5": None, "6": None},
             (ux, uy), B, 0, "ADS7028 8-channel 12-bit SAR ADC; AVDD (from the LDO) is its reference"),
        two("C3", "C1U", "AVDD", "GND", (ux + 1.0, uy + 3.7), B, 90, "ADC AVDD (reference)"),
        two("C4", "C1U", "DECAP", "GND", (ux - 0.6, uy + 3.7), B, 90, "ADC DECAP"),
        two("C5", "C1U", "3V3", "GND", (ux - 2.2, uy + 3.7), B, 90, "ADC DVDD"),
        two("R3", "R33", "SDO", "MISO", (ux - 3.8, uy - 0.6), B, 90, "SDO series, over the cable"),
        two("R4", "R100K", "3V3", "CS", (ux - 3.8, uy + 3.0), B, 90, "CS pull-up: the ADC ignores the bus "
            "while the pad boots"),
        part("U6", "ESD", "SRV05-4", v3lib.SOT236,
             {"1": "SCLK", "2": "GND", "3": "MOSI", "4": "MISO", "5": "3V3", "6": "CS"},
             (160.6, 74.0), B, 0, "ESD on the SPI lines"),
        # 5 V -> ferrite -> LDO -> AVDD, left of the connector
        two("FB1", "FB600", "5V", "5V_F", (134.4, 73.6), B, 90, "5 V filter before the LDO"),
        two("C1", "C1U", "5V_F", "GND", (136.0, 73.6), B, 90, "LDO input"),
        part("U5", "LDO", "LP5907", v3lib.SOT235, {"1": "5V_F", "2": "GND", "3": "5V_F", "4": None, "5": "AVDD"},
             (133.2, 77.4), B, 0, "Sensor and ADC reference supply, 3.3 V, low noise"),
        two("C2", "C1U", "AVDD", "GND", (136.0, 77.4), B, 90, "LDO output"),
        two("R1", "R100K", "3V3", "ID", (141.6, 76.6), B, 0, "ID divider top"),
        two("R2", "R100K", "ID", "GND", (141.6, 78.4), B, 0, "ID divider bottom: 1.65 V"),
        two("C12", "C100N", "3V3", "GND", (145.0, 76.6), B, 0, "3V3 at the connector"),
    ]
    for k in (1, 2, 3):
        col = 172.0 + 1.6 * (k - 1)
        parts += [
            two("R%d" % (4 + k), "R220", "OUT_K%d" % k, "AIN_K%d" % k, (col, uy + 2.0), B, 90,
                "Key %d output filter, series (never a capacitor straight on OUT)" % k),
            two("C%d" % (8 + k), "C10N", "AIN_K%d" % k, "GND", (col, uy - 1.8), B, 90,
                "Key %d output filter, shunt; charge reservoir for the ADC sample" % k),
        ]
    notes = ["OPad HE module V3: DRV5055A3 x3 -> 220R/10nF -> ADS7038 (AIN5/4/3 = keys 1/2/3), SPI on the cable.",
             "AVDD = LP5907 3.3 V from the cable's 5 V (filtered): sensors and ADC reference, ratiometric.",
             "ID = 100k/100k = 1.65 V (the SPI class shared with HE V2)."]

    def extra(bb):
        module_common(bb, "OPad HE module")
        for k, x in enumerate(KEYS, 1):      # the switches' two plastic pegs (Lekker standard), holes only
            fp = pcbnew.FootprintLoad(v3lib.FP_LIB, "SW_MX_Hall_PlateMount_SolidCentre")
            fp.SetFPID(pcbnew.LIB_ID("osupad", "SW_MX_Hall_PlateMount_SolidCentre"))
            bb.board.Add(fp)
            fp.SetPosition(g1.pt(x, KEY_Y))
            fp.SetReference("SW%d" % k)
            fp.SetBoardOnly(True)
            fp.SetExcludedFromBOM(True)
            fp.SetExcludedFromPosFiles(True)
            fp.Reference().SetVisible(False)
        # The part that broke V1.0: the pad layout of the built, working V1.1, turned a quarter
        # (JLCPCB then places it at 0/180 deg, jlc.py) but never mirrored. A Hall sensor reads the
        # field through its face, so its turn in the board's plane does not matter.
        v11 = {"1": (-0.95, -1.0), "2": (0.95, -1.0), "3": (0.0, 1.0)}
        turns = [{n: (round(-y, 3) + 0.0, round(x, 3) + 0.0) for n, (x, y) in v11.items()},
                 {n: (round(y, 3) + 0.0, round(-x, 3) + 0.0) for n, (x, y) in v11.items()}]
        for k, x in enumerate(KEYS, 1):
            got = {n: (round(px - x, 3) + 0.0, round(py - KEY_Y, 3) + 0.0)
                   for n, (px, py) in pads_of(bb, "U%d" % k).items()}
            assert got in turns, "U%d pads %s are not V1.1's layout turned a quarter" % (k, got)
        for x in KEYS:
            keepout(bb, x, KEY_Y, 1.5)
        u = pads_of(bb, "U4")
        for dx in (-0.45, 0.45):
            for dy in (-0.45, 0.45):
                bb.via("GND", u["17"][0] + dx, u["17"][1] + dy, 0.6, 0.3)

    return build("HE", "he_input_v3", "OPad Hall Effect Input Module V3", parts, MOD_OUTLINE, MOD_HOLES, notes,
                 ["GND", "3V3", "5V", "5V_F"], extra, {"GND": 0.4, "AVDD": 0.3, "5V": 0.3, "5V_F": 0.3, "3V3": 0.3},
                 [(114.0, 91.5), (126.0, 91.5), (140.0, 91.5), (155.0, 91.5), (170.0, 91.5), (181.0, 91.5),
                  (119.8, KEY_Y), (138.0, KEY_Y), (157.05, KEY_Y), (175.2, KEY_Y), (181.0, 78.0), (114.0, 78.0)])


BOARDS = {"carrier": carrier, "mx": mx, "he": he}

if __name__ == "__main__":
    v3lib.write_library()
    for name in (sys.argv[1:] or list(BOARDS)):
        BOARDS[name]()
