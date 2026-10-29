"""Shared library for the OPad V3 boards: footprints, symbols and the schematic writer.

Every footprint new in V3 is drawn from its maker's land pattern, and its pad
positions agree with JLCPCB's own model of the LCSC part (LCSC product API,
checked by hardware/verify). Footprints that V1/V2 already proved on built
boards are copied unchanged.

Sources:
- FFC: JUSHUO AFA07-S**F*-** customer drawing (LCSC C262720 and C262714
  datasheets, the same drawing for every contact count): contacts 0.60 x 1.80
  at 1.00 mm, fitting nails 2.60 x 3.00 whose outer edge is 3.65 mm past the
  outer contact, circuit 1 on the left in the top view with the contacts at
  the rear and the FFC entry at the front.
- USB-C: HRO TYPE-C-31-M-12 drawing (LCSC C165948): pin order A1/B12, A4/B9,
  B8, A5, B7, A6, A7, B6, A8, B5, B4/A9, B1/A12 from left to right with the
  pins at the rear and the PCB edge at the front; shield legs 8.65 mm apart,
  4.18 mm from rear to front leg, PCB edge 5.79 mm in front of the rear legs.
- SOT-23-6: ProTek SRV05-4 pin configuration (1 I/O1, 2 GND, 3 I/O2 down the
  left side, 4 I/O3, 5 REF, 6 I/O4 up the right side).
- SOT-23-5: TI LP5907 (SNVS798) DBV: 1 IN, 2 GND, 3 EN on the left, 4 N/C,
  5 OUT on the right.
- SOD-123: B5817W (LCSC C7420328): JLCPCB's model has pad 1 = anode, pad 2 =
  cathode, and this footprint numbers them the same way.
"""

import os
import shutil

HERE = os.path.dirname(os.path.abspath(__file__))
V3 = os.path.dirname(HERE)
PCB = os.path.dirname(V3)
V1LIB = os.path.join(PCB, "V1", "lib")
V2LIB = os.path.join(PCB, "V2", "lib")
LIB = os.path.join(V3, "lib")
FP_LIB = os.path.join(LIB, "osupad.pretty")
SYM_LIB = os.path.join(LIB, "osupad.kicad_sym")

COPIED = {
    V1LIB: ["C_0603_1608Metric", "R_0603_1608Metric", "SOT-23", "MountingHole_2.2mm_M2",
            "SW_MX_Hall_PlateMount_SolidCentre", "Kailh_CPG151101S11_MX_Hotswap_Bottom",
            "PinSocket_1x14_P2.54mm_Vertical"],
    V2LIB: ["L_0603_1608Metric", "WQFN-16-1EP_3x3mm_P0.5mm_EP1.68mm"],
}

FFC14 = "FFC_JUSHUO_AFA07-S14FCA_1x14-1MP_P1.0mm_Horizontal"
FFC8 = "FFC_JUSHUO_AFA07-S08FCA_1x08-1MP_P1.0mm_Horizontal"
USBC = "USB_C_Receptacle_HRO_TYPE-C-31-M-12"
SOT236 = "SOT-23-6"
SOT235 = "SOT-23-5"
SOD123 = "D_SOD-123"

# Pin rows of the FFC, in the footprint frame (Y down, contacts on Y = 0)
FFC_MOUTH_Y = 4.6        # front face of the housing, where the cable goes in
FFC_NAIL = (8.85, 2.2)


def _pad(num, kind, shape, x, y, w, h, drill=None, layers=None, rr=0.25, extra=""):
    if layers is None:
        layers = '"F.Cu" "F.Paste" "F.Mask"' if kind == "smd" else '"*.Cu" "*.Mask"'
    d = ""
    if drill is not None:
        d = (" (drill oval %s %s)" % drill) if isinstance(drill, tuple) else " (drill %s)" % drill
    r = " (roundrect_rratio %s)" % rr if shape == "roundrect" else ""
    return '\t(pad "%s" %s %s (at %s %s) (size %s %s)%s (layers %s)%s%s)\n' % (
        num, kind, shape, round(x, 4), round(y, 4), w, h, d, layers, r, extra)


def _line(layer, a, b, width=0.12):
    return '\t(fp_line (start %s %s) (end %s %s) (stroke (width %s) (type solid)) (layer "%s"))\n' % (
        round(a[0], 3), round(a[1], 3), round(b[0], 3), round(b[1], 3), width, layer)


def _rect(layer, x0, y0, x1, y1, width):
    return "".join(_line(layer, a, b, width) for a, b in (((x0, y0), (x1, y0)), ((x1, y0), (x1, y1)),
                                                         ((x1, y1), (x0, y1)), ((x0, y1), (x0, y0))))


def _footprint(name, descr, tags, body, attr="smd"):
    return ('(footprint "%s"\n\t(version 20240108)\n\t(generator "osupad")\n\t(generator_version "8.0")\n'
            '\t(layer "F.Cu")\n\t(descr "%s")\n\t(tags "%s")\n'
            '\t(property "Reference" "REF**" (at 0 -3 0) (layer "F.Fab") (effects (font (size 0.8 0.8) (thickness 0.12))))\n'
            '\t(property "Value" "%s" (at 0 3 0) (layer "F.Fab") (effects (font (size 0.8 0.8) (thickness 0.12))))\n'
            '\t(attr %s)\n%s)\n' % (name, descr, tags, name, attr, body))


def ffc(n, name, mpn, lcsc):
    """JUSHUO AFA07-S**FCA-00, n contacts: contacts at 1.00 mm, nails 2.35 mm past the outer
    contacts, housing (DIM C) n + 5.05 mm wide (the drawing's land pattern and dimension table)."""
    x1 = (n - 1) / 2.0                   # outer contact
    nail = x1 + 2.35
    half = round((n + 5.05) / 2.0 + 1e-6, 2)     # housing
    edge = nail + 1.3                    # outer edge of a nail pad
    b = ""
    for k in range(1, n + 1):
        b += _pad(k, "smd", "roundrect", k - (n + 1) / 2.0, 0.0, 0.6, 1.8)
    for sx in (-1, 1):
        b += _pad("MP", "smd", "roundrect", sx * nail, FFC_NAIL[1], 2.6, 3.0, rr=0.1)
    b += _rect("F.Fab", -half, -0.2, half, FFC_MOUTH_Y, 0.1)
    b += _line("F.Fab", (-n / 2.0, FFC_MOUTH_Y - 0.6), (n / 2.0, FFC_MOUTH_Y - 0.6), 0.1)   # cable entry
    b += _rect("F.CrtYd", -(edge + 0.45), -1.25, edge + 0.45, 5.1, 0.05)
    b += _line("F.SilkS", (-x1 - 1.2, -1.1), (-x1 - 0.6, -1.1))
    b += ('\t(fp_circle (center %s -1.55) (end %s -1.55) (stroke (width 0.12) (type solid)) (fill yes) '
          '(layer "F.SilkS"))\n' % (-x1, -x1 + 0.15))
    for sx in (-1, 1):
        b += _line("F.SilkS", (sx * (half + 0.07), 0.4), (sx * (half + 0.07), 0.5))
        b += _line("F.SilkS", (sx * (half + 0.07), 3.9), (sx * (half + 0.07), FFC_MOUTH_Y))
    return _footprint(name, "JUSHUO %s FFC/FPC ZIF connector, 1.0 mm pitch, %d contacts, "
                      "bottom contact, 2.5 mm tall, slide lock (LCSC %s). Contacts at the rear, cable "
                      "enters from the front (+Y)." % (mpn, n, lcsc), "FFC FPC ZIF 1.0mm %dP JUSHUO AFA07" % n, b)


def ffc14():
    return ffc(14, FFC14, "AFA07-S14FCA-00", "C262720")


def ffc8():
    return ffc(8, FFC8, "AFA07-S08FCA-00", "C262714")


USB_PINS = [("A1B12", -3.2, 0.6), ("A4B9", -2.4, 0.6), ("B8", -1.75, 0.3), ("A5", -1.25, 0.3),
            ("B7", -0.75, 0.3), ("A6", -0.25, 0.3), ("A7", 0.25, 0.3), ("B6", 0.75, 0.3),
            ("A8", 1.25, 0.3), ("B5", 1.75, 0.3), ("B4A9", 2.4, 0.6), ("B1A12", 3.2, 0.6)]
USB_ROW_Y = -2.473
USB_EDGE_Y = 4.084       # the board edge the receptacle's mouth sits on


def usb_c():
    b = ""
    for name, x, w in USB_PINS:
        b += _pad(name, "smd", "roundrect", x, USB_ROW_Y, w, 1.3, rr=0.15)
    for num, x, y, h, dh in (("S1", -4.325, -1.706, 2.1, 1.7), ("S2", 4.325, -1.706, 2.1, 1.7),
                             ("S3", -4.325, 2.475, 1.8, 1.4), ("S4", 4.325, 2.475, 1.8, 1.4)):
        b += _pad(num, "thru_hole", "oval", x, y, 1.1, h, drill=(0.6, dh))
    for x in (-2.89, 2.89):
        b += _pad("", "np_thru_hole", "circle", x, -1.206, 0.65, 0.65, drill=0.65, layers='"*.Cu" "*.Mask"')
    b += _rect("F.Fab", -4.47, -3.27, 4.47, USB_EDGE_Y + 0.4, 0.1)
    b += _line("Dwgs.User", (-6.0, USB_EDGE_Y), (6.0, USB_EDGE_Y), 0.1)
    b += _rect("F.CrtYd", -5.2, -3.6, 5.2, USB_EDGE_Y + 0.5, 0.05)
    for sx in (-1, 1):
        b += _line("F.SilkS", (sx * 4.6, 0.0), (sx * 4.6, 1.4))
    return _footprint(USBC, "HRO TYPE-C-31-M-12 USB-C 2.0 receptacle, 16 pins, SMD with 4 shield legs "
                      "(LCSC C165948). Pins at the rear; the mouth sits on the board edge at Y = +4.084.",
                      "USB C Type-C receptacle HRO TYPE-C-31-M-12", b)


def sot23_6():
    b = ""
    for n, x, y in ((1, -1.35, -0.95), (2, -1.35, 0), (3, -1.35, 0.95), (4, 1.35, 0.95), (5, 1.35, 0), (6, 1.35, -0.95)):
        b += _pad(n, "smd", "roundrect", x, y, 1.1, 0.6)
    b += _rect("F.Fab", -0.8, -1.45, 0.8, 1.45, 0.1)
    b += _rect("F.CrtYd", -2.1, -1.75, 2.1, 1.75, 0.05)
    b += '\t(fp_circle (center -1.9 -1.6) (end -1.75 -1.6) (stroke (width 0.12) (type solid)) (fill yes) (layer "F.SilkS"))\n'
    b += _line("F.SilkS", (-0.3, -1.55), (0.3, -1.55)) + _line("F.SilkS", (-0.3, 1.55), (0.3, 1.55))
    return _footprint(SOT236, "SOT-23-6, pin 1 top left, 0.95 mm pitch (ProTek SRV05-4 pin configuration)",
                      "SOT-23-6", b)


def sot23_5():
    b = ""
    for n, x, y in ((1, -1.3, -0.95), (2, -1.3, 0), (3, -1.3, 0.95), (4, 1.3, 0.95), (5, 1.3, -0.95)):
        b += _pad(n, "smd", "roundrect", x, y, 1.1, 0.6)
    b += _rect("F.Fab", -0.8, -1.45, 0.8, 1.45, 0.1)
    b += _rect("F.CrtYd", -2.05, -1.75, 2.05, 1.75, 0.05)
    b += '\t(fp_circle (center -1.85 -1.6) (end -1.7 -1.6) (stroke (width 0.12) (type solid)) (fill yes) (layer "F.SilkS"))\n'
    b += _line("F.SilkS", (-0.3, -1.55), (0.3, -1.55)) + _line("F.SilkS", (-0.3, 1.55), (0.3, 1.55))
    return _footprint(SOT235, "SOT-23-5 (TI DBV), pin 1 top left, 0.95 mm pitch", "SOT-23-5 DBV", b)


def sod123():
    b = _pad(1, "smd", "roundrect", -1.61, 0, 0.9, 1.0) + _pad(2, "smd", "roundrect", 1.61, 0, 0.9, 1.0)
    b += _rect("F.Fab", -1.35, -0.8, 1.35, 0.8, 0.1)
    b += _rect("F.CrtYd", -2.35, -1.0, 2.35, 1.0, 0.05)
    b += _line("F.SilkS", (0.6, -0.95), (0.6, 0.95), 0.2)      # cathode band at pad 2
    b += _line("F.SilkS", (-0.9, -0.95), (0.9, -0.95)) + _line("F.SilkS", (-0.9, 0.95), (0.9, 0.95))
    return _footprint(SOD123, "SOD-123 diode, pad 1 anode, pad 2 cathode (band), as JLCPCB's model of "
                      "B5817W C7420328", "SOD-123 diode", b)


def testpad():
    return _footprint("TestPoint_Pad_1.0x1.0mm", "Bare 1.0 mm test pad, no paste", "test point",
                      _pad(1, "smd", "rect", 0, 0, 1.0, 1.0, layers='"F.Cu" "F.Mask"'))


NEW = {FFC14: ffc14, FFC8: ffc8, USBC: usb_c, SOT236: sot23_6, SOT235: sot23_5, SOD123: sod123,
       "TestPoint_Pad_1.0x1.0mm": testpad}


# -----------------------------------------------------------------------------
# Symbols: R and C from V1, everything else a box with typed pins
# -----------------------------------------------------------------------------

def _font():
    return "(effects (font (size 1.27 1.27)))"


def _pin(kind, name, number, x, y, angle):
    return ('\t\t\t(pin %s line (at %s %s %d) (length 2.54) (name "%s" %s) (number "%s" %s))\n'
            % (kind, round(x, 4) + 0.0, round(y, 4) + 0.0, angle, name, _font(), number, _font()))


def box(name, ref, footprint, description, left, right):
    """left/right: [(pin name, number, electrical type)]; returns the symbol block."""
    rows = max(len(left), len(right))
    h = 2.54 * rows
    top = h / 2 + 1.27
    pins = [_pin(t, n, num, -10.16, top - 2.54 * (i + 1), 0) for i, (n, num, t) in enumerate(left)]
    pins += [_pin(t, n, num, 10.16, top - 2.54 * (i + 1), 180) for i, (n, num, t) in enumerate(right)]
    props = "".join('\t\t(property "%s" "%s" (at 0 %s 0) %s(effects (font (size 1.27 1.27))))\n'
                    % (k, v, y, "(hide yes) " if hide else "")
                    for k, v, y, hide in (("Reference", ref, top + 1.27, False),
                                          ("Value", name, -top - 1.27, False),
                                          ("Footprint", "osupad:" + footprint, 0, True),
                                          ("Datasheet", "", 0, True),
                                          ("Description", description, 0, True)))
    return ('\t(symbol "%s"\n\t\t(pin_names (offset 1.016))\n\t\t(exclude_from_sim no) (in_bom yes) (on_board yes)\n%s'
            '\t\t(symbol "%s_0_1"\n\t\t\t(rectangle (start -7.62 %s) (end 7.62 %s) (stroke (width 0.254) (type default)) '
            '(fill (type background)))\n\t\t)\n\t\t(symbol "%s_1_1"\n%s\t\t)\n\t)\n'
            % (name, props, name, top, -top, name, "".join(pins)))


P, I, O, B, W, NC = "passive", "input", "output", "bidirectional", "power_in", "no_connect"

SYMBOLS = {
    "FFC14": box("FFC14", "J", FFC14, "14-way FFC connector, mechanical tabs on MP",
                 [("%d" % k, str(k), P) for k in range(1, 8)],
                 [("%d" % k, str(k), P) for k in range(8, 15)] + [("MP", "MP", P)]),
    "FFC8": box("FFC8", "J", FFC8, "8-way FFC connector, mechanical tabs on MP",
                [("%d" % k, str(k), P) for k in range(1, 5)],
                [("%d" % k, str(k), P) for k in range(5, 9)] + [("MP", "MP", P)]),
    "USB_C": box("USB_C", "J", USBC, "USB-C 2.0 receptacle",
                 [("GND", "A1B12", P), ("VBUS", "A4B9", P), ("SBU2", "B8", P), ("CC1", "A5", P),
                  ("DN2", "B7", P), ("DP1", "A6", P)],
                 [("DN1", "A7", P), ("DP2", "B6", P), ("SBU1", "A8", P), ("CC2", "B5", P),
                  ("VBUS", "B4A9", P), ("GND", "B1A12", P), ("SHIELD", "S1", P), ("SHIELD", "S2", P),
                  ("SHIELD", "S3", P), ("SHIELD", "S4", P)]),
    "SRV05-4": box("SRV05-4", "U", SOT236, "4-line ESD protection array",
                   [("IO1", "1", P), ("GND", "2", P), ("IO2", "3", P)],
                   [("IO4", "6", P), ("REF", "5", P), ("IO3", "4", P)]),
    "LP5907": box("LP5907", "U", SOT235, "250 mA low-noise LDO, 3.3 V",
                  [("IN", "1", W), ("GND", "2", W), ("EN", "3", I)],
                  [("OUT", "5", "power_out"), ("NC", "4", NC)]),
    "D_Schottky": box("D_Schottky", "D", SOD123, "Schottky diode", [("A", "1", P)], [("K", "2", P)]),
    "ADS7038": box("ADS7038", "U", "WQFN-16-1EP_3x3mm_P0.5mm_EP1.68mm", "8-channel 12-bit SAR ADC, SPI",
                   [("AIN0", "15", B), ("AIN1", "16", B), ("AIN2", "1", B), ("AIN3", "2", B),
                    ("AIN4", "3", B), ("AIN5", "4", B), ("AIN6", "5", B), ("AIN7", "6", B)],
                   [("AVDD", "7", W), ("DVDD", "10", W), ("DECAP", "8", P), ("CS", "11", I),
                    ("SCLK", "13", I), ("SDI", "14", I), ("SDO", "12", O), ("GND", "9", W), ("EP", "17", P)]),
    "DRV5055": box("DRV5055", "U", "SOT-23", "Ratiometric linear Hall sensor",
                   [("VCC", "1", W), ("GND", "3", W)], [("OUT", "2", O)]),
    "FerriteBead": box("FerriteBead", "FB", "L_0603_1608Metric", "Ferrite bead", [("1", "1", P)], [("2", "2", P)]),
    "KeySwitch": box("KeySwitch", "SW", "Kailh_CPG151101S11_MX_Hotswap_Bottom", "MX switch in a hot-swap socket",
                     [("1", "1", P)], [("2", "2", P)]),
    "Socket14": box("Socket14", "J", "PinSocket_1x14_P2.54mm_Vertical", "1x14 socket for a Waveshare header",
                    [("%d" % k, str(k), P) for k in range(1, 8)], [("%d" % k, str(k), P) for k in range(8, 15)]),
    "TestPoint": box("TestPoint", "TP", "TestPoint_Pad_1.0x1.0mm", "Bare test pad", [("1", "1", P)], []),
    "PWR_FLAG": None,     # copied from V1
}


def write_library():
    os.makedirs(FP_LIB, exist_ok=True)
    for src, names in COPIED.items():
        for n in names:
            shutil.copy(os.path.join(src, "osupad.pretty", n + ".kicad_mod"), FP_LIB)
    for n, fn in NEW.items():
        open(os.path.join(FP_LIB, n + ".kicad_mod"), "w").write(fn())
    import re
    v1 = open(os.path.join(V1LIB, "osupad.kicad_sym")).read()

    def block(text, name):
        start = text.index('(symbol "%s"' % name)
        depth = 0
        for i in range(start, len(text)):
            if text[i] == "(":
                depth += 1
            elif text[i] == ")":
                depth -= 1
                if depth == 0:
                    return text[start:i + 1]
        raise ValueError(name)

    out = ['(kicad_symbol_lib\n\t(version 20251024)\n\t(generator "osupad")\n\t(generator_version "10.0")\n']
    for n in ("R", "C", "PWR_FLAG"):
        out.append("\t" + block(v1, n) + "\n")
    for n, text in SYMBOLS.items():
        if text:
            out.append(text)
    out.append(")\n")
    open(SYM_LIB, "w").write("".join(out))
