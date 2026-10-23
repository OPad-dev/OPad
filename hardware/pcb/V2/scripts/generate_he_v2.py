#!/usr/bin/env python3
"""Generate the OPad Hall Effect input module V2 (three keys, ADC on the module).

What V2 changes against V1.1 (hardware/pcb/V1/README.md), and why
(docs/history/2026-10-22-he-phantom-presses.md):

- Three keys at 19.05 mm pitch instead of two, for players who do not use the
  middle finger as one of the two.
- The sensors are digitised on the module by a TI ADS7038 (8-channel 12-bit
  SAR, SPI, on-chip averaging to 16 bits). Its reference is its AVDD, which
  also powers the sensors, so the reading is ratiometric: supply noise moves
  the sensor output and the ADC's ruler together. Only SPI crosses the cable,
  so the ESP32-S3's own ADC noise and ground or supply spikes on the cable no
  longer reach the measurement.
- The sensors and AVDD get their own branch of 3V3 behind a ferrite bead with
  bulk and local decoupling; each sensor output has the series-R, shunt-C
  filter TI asks for (DRV5055 datasheet 7.1.3: never a capacitor straight on
  OUT), which is also the charge reservoir for the ADC's 12 pF sampling
  capacitor (ADS7038 datasheet 7.3.1).

The carrier and the MX module do not change. The connector pinout is the V1
one, with the pins the V1 README reserved for an SPI module put to use, and
the module ID is the reserved "Hall SPI" value, 100k/100k = 1.65 V:

    1 3V3 | 2 GND | 3 CS (GPIO10) | 4 spare (GPIO7, probe pad) | 5 ID (GPIO8)
    6 MISO (GPIO6) | 7 MOSI (GPIO4) | 8 SCLK (GPIO2)

This script writes the library, schematic and placed board; route_he_v2.py
routes it. Re-running this script throws the routing away.

Run with the system Python that ships KiCad's pcbnew module:
    python3 hardware/pcb/V2/scripts/generate_he_v2.py
"""

import os
import re
import shutil
import sys

import pcbnew

HERE = os.path.dirname(os.path.abspath(__file__))
V2 = os.path.dirname(HERE)
V1 = os.path.join(os.path.dirname(V2), "V1")
sys.path.insert(0, os.path.join(V1, "scripts"))
import generate_boards as g1  # noqa: E402  (V1 helpers: board builder, project files)

LIB = os.path.join(V2, "lib")
FP_LIB = os.path.join(LIB, "osupad.pretty")
SYM_LIB = os.path.join(LIB, "osupad.kicad_sym")
PROJECT = "he_input_v2"
TITLE = "OPad Hall Effect Input Module V2"
REV = "V2.0"
DATE = "2026-10-23"

g1.FP_LIB = FP_LIB
g1.REV = REV
g1.DATE = DATE

# -----------------------------------------------------------------------------
# Library: V1 parts copied as they are, plus the ADC, its package and a bead
# -----------------------------------------------------------------------------

V1_FOOTPRINTS = ["C_0603_1608Metric", "R_0603_1608Metric", "SOT-23",
                 "JST_SH_SM08B-SRSS-TB_1x08-1MP_P1.00mm_Horizontal", "MountingHole_2.2mm_M2",
                 "SW_MX_Hall_PlateMount_SolidCentre", "TestPoint_Pad_1.0x1.0mm"]
V1_SYMBOLS = ["R", "C", "DRV5055", "Conn_01x08_1MP", "TestPoint", "PWR_FLAG"]


def font(size=1.27):
    return "(effects (font (size %s %s)))" % (size, size)


def wqfn16_footprint():
    """TI RTE0016C (WQFN-16, 3 x 3 mm, 0.5 mm pitch), from the datasheet's
    example board layout: 16 pads 0.6 x 0.24 at 2.8 mm across, exposed pad
    1.68 mm square (pad 17), solder-mask defined by 0.07 mm around each pad,
    paste on the exposed pad 1.55 mm square (85 %)."""
    pads = []
    pitch, across, pw, pl = 0.5, 1.4, 0.24, 0.6
    offs = [-0.75, -0.25, 0.25, 0.75]
    # Pin 1 top-left, counter-clockwise in the top view (datasheet Figure 5-1)
    for i, y in enumerate(offs):                     # 1-4 left side, top to bottom
        pads.append((str(1 + i), -across, y, pl, pw))
    for i, x in enumerate(offs):                     # 5-8 bottom side, left to right
        pads.append((str(5 + i), x, across, pw, pl))
    for i, y in enumerate(reversed(offs)):           # 9-12 right side, bottom to top
        pads.append((str(9 + i), across, y, pl, pw))
    for i, x in enumerate(reversed(offs)):           # 13-16 top side, right to left
        pads.append((str(13 + i), x, -across, pw, pl))
    out = ['(footprint "WQFN-16-1EP_3x3mm_P0.5mm_EP1.68mm"\n\t(version 20240108)\n\t(generator "osupad")\n'
           '\t(generator_version "8.0")\n\t(layer "F.Cu")\n'
           '\t(descr "TI RTE0016C WQFN-16 3x3 mm, 0.5 mm pitch, 1.68 mm exposed pad (ADS7038 datasheet SBAS979C)")\n'
           '\t(tags "WQFN QFN 16 RTE0016C")\n'
           '\t(property "Reference" "REF**" (at 0 -2.6 0) (layer "F.SilkS") (effects (font (size 0.8 0.8) (thickness 0.12))))\n'
           '\t(property "Value" "WQFN-16" (at 0 2.6 0) (layer "F.Fab") (effects (font (size 0.8 0.8) (thickness 0.12))))\n'
           '\t(attr smd)\n']
    for x0, y0, x1, y1 in ((-1.5, -1.5, 1.5, -1.5), (1.5, -1.5, 1.5, 1.5), (1.5, 1.5, -1.5, 1.5), (-1.5, 1.5, -1.5, -1.5)):
        out.append('\t(fp_line (start %s %s) (end %s %s) (stroke (width 0.1) (type solid)) (layer "F.Fab"))\n'
                   % (x0, y0, x1, y1))
    for x0, y0, x1, y1 in ((-2.0, -2.0, 2.0, -2.0), (2.0, -2.0, 2.0, 2.0), (2.0, 2.0, -2.0, 2.0), (-2.0, 2.0, -2.0, -2.0)):
        out.append('\t(fp_line (start %s %s) (end %s %s) (stroke (width 0.05) (type solid)) (layer "F.CrtYd"))\n'
                   % (x0, y0, x1, y1))
    # Silkscreen corner marks and a pin-1 dot, clear of the pads
    for sx, sy in ((1, -1), (1, 1), (-1, 1)):
        out.append('\t(fp_line (start %s %s) (end %s %s) (stroke (width 0.12) (type solid)) (layer "F.SilkS"))\n'
                   % (sx * 1.6, sy * 1.6, sx * 1.15, sy * 1.6))
        out.append('\t(fp_line (start %s %s) (end %s %s) (stroke (width 0.12) (type solid)) (layer "F.SilkS"))\n'
                   % (sx * 1.6, sy * 1.6, sx * 1.6, sy * 1.15))
    out.append('\t(fp_circle (center -1.95 -1.6) (end -1.8 -1.6) (stroke (width 0.12) (type solid)) (fill yes) (layer "F.SilkS"))\n')
    for n, x, y, w, h in pads:
        out.append('\t(pad "%s" smd roundrect (at %s %s) (size %s %s) (layers "F.Cu" "F.Paste" "F.Mask") '
                   '(roundrect_rratio 0.2) (solder_mask_margin 0.07))\n' % (n, x, y, w, h))
    out.append('\t(pad "17" smd rect (at 0 0) (size 1.68 1.68) (layers "F.Cu" "F.Paste" "F.Mask") '
               '(solder_mask_margin 0.07) (solder_paste_margin -0.065))\n')
    out.append(')\n')
    return "".join(out)


def bead_footprint():
    """0603 ferrite bead: the 0603 resistor land pattern under its own name."""
    text = open(os.path.join(V1, "lib", "osupad.pretty", "R_0603_1608Metric.kicad_mod")).read()
    text = text.replace('(footprint "R_0603_1608Metric"', '(footprint "L_0603_1608Metric"', 1)
    text = text.replace('"R_0603_1608Metric"', '"L_0603_1608Metric"')
    text = re.sub(r'\(descr "[^"]*"\)', '(descr "Ferrite bead / inductor, 0603 (1608 metric) SMD chip")', text, count=1)
    return text


def pin(kind, name, number, x, y, angle):
    return ('\t\t\t(pin %s line (at %s %s %d) (length 2.54) (name "%s" %s) (number "%s" %s))\n'
            % (kind, x, y, angle, name, font(), number, font()))


def symbol(name, ref, value, footprint, description, body, pins, datasheet=""):
    props = "".join('\t\t(property "%s" "%s" (at 0 %s 0) %s(%s))\n'
                    % (k, v, y, "(hide yes) " if hide else "", font()[1:-1])
                    for k, v, y, hide in (("Reference", ref, body[1] + 1.27, False),
                                          ("Value", value, body[3] - 1.27, False),
                                          ("Footprint", footprint, 0, True),
                                          ("Datasheet", datasheet, 0, True),
                                          ("Description", description, 0, True)))
    return ('\t(symbol "%s"\n\t\t(pin_names (offset 1.016))\n\t\t(exclude_from_sim no) (in_bom yes) (on_board yes)\n%s'
            '\t\t(symbol "%s_0_1"\n\t\t\t(rectangle (start %s %s) (end %s %s) (stroke (width 0.254) (type default)) '
            '(fill (type background)))\n\t\t)\n\t\t(symbol "%s_1_1"\n%s\t\t)\n\t)\n'
            % (name, props, name, body[0], body[1], body[2], body[3], name, "".join(pins)))


def ads7038_symbol():
    left = [("AIN0/GPIO0", "15"), ("AIN1/GPIO1", "16"), ("AIN2/GPIO2", "1"), ("AIN3/GPIO3", "2"),
            ("AIN4/GPIO4", "3"), ("AIN5/GPIO5", "4"), ("AIN6/GPIO6", "5"), ("AIN7/GPIO7", "6")]
    pins = [pin("bidirectional", n, num, -12.7, 8.89 - 2.54 * i, 0) for i, (n, num) in enumerate(left)]
    right = [("AVDD", "7", "power_in"), ("DVDD", "10", "power_in"), ("DECAP", "8", "passive"),
             ("CS", "11", "input"), ("SCLK", "13", "input"), ("SDI", "14", "input"), ("SDO", "12", "output")]
    pins += [pin(kind, n, num, 12.7, 8.89 - 2.54 * i, 180) for i, (n, num, kind) in enumerate(right)]
    pins.append(pin("power_in", "GND", "9", 12.7, -10.16, 180))
    pins.append(pin("passive", "EP", "17", 12.7, -12.7, 180))
    return symbol("ADS7038", "U", "ADS7038", "osupad:WQFN-16-1EP_3x3mm_P0.5mm_EP1.68mm",
                  "TI ADS7038 8-channel 12-bit SAR ADC, SPI, WQFN-16", (-10.16, 11.43, 10.16, -15.24), pins,
                  "https://www.ti.com/lit/ds/symlink/ads7038.pdf")


def bead_symbol():
    pins = [pin("passive", "1", "1", 0, 3.81, 270), pin("passive", "2", "2", 0, -3.81, 90)]
    return symbol("FerriteBead", "FB", "FerriteBead", "osupad:L_0603_1608Metric", "Ferrite bead",
                  (-1.016, 1.27, 1.016, -1.27), pins)


def write_library():
    os.makedirs(FP_LIB, exist_ok=True)
    for name in V1_FOOTPRINTS:
        shutil.copy(os.path.join(V1, "lib", "osupad.pretty", name + ".kicad_mod"), FP_LIB)
    with open(os.path.join(FP_LIB, "WQFN-16-1EP_3x3mm_P0.5mm_EP1.68mm.kicad_mod"), "w") as f:
        f.write(wqfn16_footprint())
    with open(os.path.join(FP_LIB, "L_0603_1608Metric.kicad_mod"), "w") as f:
        f.write(bead_footprint())
    text = open(os.path.join(V1, "lib", "osupad.kicad_sym")).read()
    blocks = [g1.sexpr_block(text, text.index('(symbol "%s"' % n)) for n in V1_SYMBOLS]
    with open(SYM_LIB, "w") as f:
        f.write('(kicad_symbol_lib\n\t(version 20251024)\n\t(generator "osupad")\n\t(generator_version "10.0")\n')
        for b in blocks:
            f.write("\t" + b + "\n")
        f.write(ads7038_symbol())
        f.write(bead_symbol())
        f.write(")\n")


# -----------------------------------------------------------------------------
# Netlist and placement
# -----------------------------------------------------------------------------
# KiCad coordinates, the V1 HE frame (V1/HE/he_input_v1.kicad_pcb): Y down,
# the rear edge (cable side, towards the screen) at Y = 69, the keys on
# Y = 83.5. The board is 24 mm deep like V1 and wide enough for three keys
# with V1's 16.475 mm from each outer key centre to the board edge.

PITCH = 19.05
KEY_Y = 83.5
X0, Y0 = 112.0, 69.0
KEYS = [X0 + 16.475 + PITCH * i for i in range(3)]     # 128.475, 147.525, 166.575
X1, Y1 = KEYS[2] + 16.475, Y0 + 24.0                    # 183.05, 93.0
HOLES = [(X0 + 3.5, KEY_Y), (X1 - 3.5, KEY_Y)]
J1_POS = (KEYS[1], 72.175)                             # centred on the middle key, mouth at the rear edge

# ADS7038 channel per key (firmware: input/he_spi.c must match)
KEY_CH = {1: ("AIN5", "4"), 2: ("AIN4", "3"), 3: ("AIN3", "2")}

DRV = dict(lcsc="C266128", mpn="Texas Instruments DRV5055A3QDBZR")
C100N = dict(value="100nF", lcsc="C14663", mpn="YAGEO CC0603KRX7R9BB104")
C1U = dict(value="1uF", lcsc="C15849", mpn="Samsung CL10A105KB8NNNC")
C10U = dict(value="10uF", lcsc="C19702", mpn="Samsung CL10A106KP8NNNC")
C10N = dict(value="10nF", lcsc="C57112", mpn="FH 0603B103K500NT")
R220 = dict(value="220", lcsc="C22962", mpn="UNI-ROYAL 0603WAF2200T5E")
R33 = dict(value="33", lcsc="C23140", mpn="UNI-ROYAL 0603WAF330JT5E")
R100K = dict(value="100k", lcsc="C25803", mpn="UNI-ROYAL 0603WAF1003T5E")
FB = dict(value="600R@100MHz", lcsc="C1002", mpn="Sunlord GZ1608D601TF")
ADC = dict(value="ADS7038", lcsc="C2876384", mpn="Texas Instruments ADS7038QRTERQ1")

PINOUT = {"1": "3V3", "2": "GND", "3": "CS", "4": "SPARE", "5": "ID", "6": "MISO", "7": "MOSI", "8": "SCLK",
          "MP": "GND"}


def parts():
    P = g1.Part
    out = [
        P("J1", "Conn_01x08_1MP", "MODULE", "JST_SH_SM08B-SRSS-TB_1x08-1MP_P1.00mm_Horizontal", dict(PINOUT),
          (50.8, 50.8), J1_POS, "bottom", 0, "C160407", "JST SM08B-SRSS-TB(LF)(SN)",
          "Module connector to controller carrier (JST SH 8-pin), V1 pinout with the SPI pins in use"),
        P("C1", "C", C100N["value"], "C_0603_1608Metric", {"1": "GND", "2": "3V3"}, (25.4, 76.2),
          (144.4, 77.4), "bottom", 90, C100N["lcsc"], C100N["mpn"], "3V3 decoupling at the connector"),
        P("FB1", "FerriteBead", FB["value"], "L_0603_1608Metric", {"1": "3V3", "2": "AVDD"}, (38.1, 76.2),
          (140.8, 77.4), "bottom", 90, FB["lcsc"], FB["mpn"],
          "Sensor and ADC analog supply filter (600R at 100 MHz, 0.45R DC)"),
        P("C2", "C", C10U["value"], "C_0603_1608Metric", {"1": "GND", "2": "AVDD"}, (45.72, 88.9),
          (139.0, 77.4), "bottom", 90, C10U["lcsc"], C10U["mpn"], "AVDD bulk, after the ferrite bead"),
        P("R1", "R", R100K["value"], "R_0603_1608Metric", {"1": "3V3", "2": "ID"}, (25.4, 101.6),
          (150.2, 77.4), "bottom", 90, R100K["lcsc"], R100K["mpn"],
          "Module ID divider, top (HE V2 = 100k/100k = 1.65 V, the 'Hall SPI' ID)"),
        P("R2", "R", R100K["value"], "R_0603_1608Metric", {"1": "ID", "2": "GND"}, (25.4, 114.3),
          (151.8, 77.4), "bottom", 90, R100K["lcsc"], R100K["mpn"], "Module ID divider, bottom"),
        P("TP1", "TestPoint", "SPARE", "TestPoint_Pad_1.0x1.0mm", {"1": "SPARE"}, (50.8, 88.9),
          (147.0, 77.4), "bottom", 0, "", "", "Probe pad for the spare connector pin 4 (GPIO7)"),
        P("U4", "ADS7038", ADC["value"], "WQFN-16-1EP_3x3mm_P0.5mm_EP1.68mm",
          {"7": "AVDD", "10": "3V3", "8": "DECAP", "11": "CS", "13": "SCLK", "14": "MOSI", "12": "SDO",
           "9": "GND", "17": "GND", KEY_CH[1][1]: "AIN_K1", KEY_CH[2][1]: "AIN_K2", KEY_CH[3][1]: "AIN_K3"},
          (101.6, 63.5), (158.5, 73.0), "bottom", 0, ADC["lcsc"], ADC["mpn"],
          "8-channel 12-bit SAR ADC, SPI; AVDD is its reference (ratiometric with the sensors)"),
        P("R3", "R", R33["value"], "R_0603_1608Metric", {"1": "SDO", "2": "MISO"}, (127.0, 50.8),
          (155.0, 72.6), "bottom", 90, R33["lcsc"], R33["mpn"], "SDO series termination over the cable"),
        P("C5", "C", C1U["value"], "C_0603_1608Metric", {"1": "GND", "2": "3V3"}, (142.24, 76.2),
          (155.0, 76.4), "bottom", 90, C1U["lcsc"], C1U["mpn"], "ADS7038 DVDD decoupling"),
        P("C4", "C", C1U["value"], "C_0603_1608Metric", {"1": "GND", "2": "DECAP"}, (134.62, 76.2),
          (157.75, 76.7), "bottom", 90, C1U["lcsc"], C1U["mpn"], "ADS7038 DECAP"),
        P("C3", "C", C1U["value"], "C_0603_1608Metric", {"1": "GND", "2": "AVDD"}, (127.0, 76.2),
          (159.3, 76.7), "bottom", 90, C1U["lcsc"], C1U["mpn"], "ADS7038 AVDD (reference) decoupling"),
    ]
    # Sensors with their local decoupling, and per key a filter column right
    # of the ADC: the 10 nF shunt on top, the 220 R series below it, so the
    # sensor line enters from below and the filtered node faces the ADC
    for k in (1, 2, 3):
        x = KEYS[k - 1]
        sy = 63.5 + 25.4 * (k - 1)
        col = 161.8 + 1.6 * (k - 1)
        out += [
            P("U%d" % k, "DRV5055", "DRV5055A3", "SOT-23", {"1": "AVDD", "2": "OUT_K%d" % k, "3": "GND"},
              (190.5, sy), (x, KEY_Y), "bottom", 180, DRV["lcsc"], DRV["mpn"],
              "Ratiometric linear Hall sensor, key %d" % k),
            P("C%d" % (5 + k), "C", C100N["value"], "C_0603_1608Metric", {"1": "GND", "2": "AVDD"},
              (215.9, sy), (x - 2.525, 80.8), "bottom", 0, C100N["lcsc"], C100N["mpn"],
              "Local supply decoupling, key %d sensor" % k),
            P("R%d" % (4 + k), "R", R220["value"], "R_0603_1608Metric", {"1": "OUT_K%d" % k, "2": "AIN_K%d" % k},
              (165.1, sy), (col, 75.6), "bottom", 90, R220["lcsc"], R220["mpn"],
              "Key %d output filter, series (never a capacitor straight on OUT)" % k),
            P("C%d" % (8 + k), "C", C10N["value"], "C_0603_1608Metric", {"1": "GND", "2": "AIN_K%d" % k},
              (165.1, sy + 12.7), (col, 71.8), "bottom", 90, C10N["lcsc"], C10N["mpn"],
              "Key %d output filter, shunt; charge reservoir for the ADC sample" % k),
        ]
    return out


# Which pad of a vertical two-pad part must face the rear edge (smaller Y)
PAD_TOWARDS_REAR = {"R5": "2", "R6": "2", "R7": "2", "C9": "1", "C10": "1", "C11": "1",
                    "R3": "1", "C1": "2", "FB1": "1", "C2": "2", "R1": "1", "R2": "1",
                    "C3": "2", "C4": "2", "C5": "2"}

# ADS7038 pins with no connection: their schematic net names, for parity
ADC_PIN_NAMES = {"15": "AIN0{slash}GPIO0", "16": "AIN1{slash}GPIO1", "1": "AIN2{slash}GPIO2",
                 "2": "AIN3{slash}GPIO3", "3": "AIN4{slash}GPIO4", "4": "AIN5{slash}GPIO5",
                 "5": "AIN6{slash}GPIO6", "6": "AIN7{slash}GPIO7"}


# -----------------------------------------------------------------------------
# Schematic
# -----------------------------------------------------------------------------

def write_schematic(directory, parts_list):
    text = open(SYM_LIB).read()
    sym = {}
    for name in {p.symbol for p in parts_list} | {"PWR_FLAG"}:
        block = g1.sexpr_block(text, text.index('(symbol "%s"' % name))
        pins = {}
        for m in re.finditer(r'\(pin\s+\w+\s+\w+\s*\(at ([-\d.]+) ([-\d.]+) ([-\d.]+)\).*?\(number "([^"]+)"',
                             block, re.S):
            pins[m.group(4)] = (float(m.group(1)), float(m.group(2)), int(float(m.group(3))))
        sym[name] = (block.replace('(symbol "%s"' % name, '(symbol "osupad:%s"' % name, 1), pins)
    root = g1.uid()
    out = ['(kicad_sch\n\t(version 20250114)\n\t(generator "eeschema")\n\t(generator_version "9.0")\n',
           '\t(uuid "%s")\n\t(paper "A3")\n' % root,
           '\t(title_block (title "%s") (date "%s") (rev "%s") (company "OPad"))\n' % (TITLE, DATE, REV),
           '\t(lib_symbols\n']
    for name in sorted(sym):
        out.append("\t\t" + sym[name][0] + "\n")
    out.append("\t)\n")
    notes = [
        "Module connector (V1 pinout): 1 3V3 | 2 GND | 3 CS = GPIO10 | 4 spare = GPIO7 | 5 ID = GPIO8 | "
        "6 MISO = GPIO6 | 7 MOSI = GPIO4 | 8 SCLK = GPIO2",
        "ID = 3V3 x 100k/(100k+100k) = 1.65 V: the 'Hall SPI' module ID reserved in V1/README.md.",
        "Ratiometric: the ADS7038 reference is AVDD, the same filtered rail that powers the three sensors.",
        "Key channels: key 1 = %s, key 2 = %s, key 3 = %s." % (KEY_CH[1][0], KEY_CH[2][0], KEY_CH[3][0]),
        "Every part is on the bottom side; the switches sit on top under the plate.",
    ]
    for i, note in enumerate(notes):
        out.append('\t(text "%s" (exclude_from_sim no) (at 20.32 %.2f 0) (effects (font (size 1.27 1.27)) '
                   '(justify left bottom)) (uuid "%s"))\n' % (note, 250.0 + i * 3.81, g1.uid()))

    flags = [("#FLG01", "3V3", (25.4, 30.48)), ("#FLG02", "GND", (35.56, 30.48)), ("#FLG03", "AVDD", (45.72, 30.48))]
    for ref, net, (sx, sy) in flags:
        out.append('\t(symbol (lib_id "osupad:PWR_FLAG") (at %.2f %.2f 0) (unit 1) (exclude_from_sim no) '
                   '(in_bom no) (on_board yes) (dnp no) (uuid "%s")\n' % (sx, sy, g1.uid()))
        out.append(g1.prop("Reference", ref, sx, sy - 3.81, hide=True))
        out.append(g1.prop("Value", "PWR_FLAG", sx, sy - 5.08))
        out.append(g1.prop("Footprint", "", sx, sy, hide=True))
        out.append('\t\t(pin "1" (uuid "%s"))\n' % g1.uid())
        out.append('\t\t(instances (project "%s" (path "/%s" (reference "%s") (unit 1))))\n\t)\n' % (PROJECT, root, ref))
        out.append('\t(label "%s" (at %.2f %.2f 0) (effects (font (size 1.27 1.27)) (justify left bottom)) (uuid "%s"))\n'
                   % (net, sx, sy, g1.uid()))

    for p in parts_list:
        sx, sy = p.sch
        pins = sym[p.symbol][1]
        out.append('\t(symbol (lib_id "osupad:%s") (at %.2f %.2f 0) (unit 1) (exclude_from_sim no) '
                   '(in_bom %s) (on_board yes) (dnp %s) (uuid "%s")\n'
                   % (p.symbol, sx, sy, "no" if p.symbol == "TestPoint" else "yes",
                      "yes" if p.symbol == "TestPoint" else "no", p.uuid))
        top = max(y for _, y, _ in pins.values())
        bottom = min(y for _, y, _ in pins.values())
        out.append(g1.prop("Reference", p.ref, sx + 3.81, sy - top - 2.54))
        out.append(g1.prop("Value", p.value, sx + 3.81, sy - bottom + 2.54))
        out.append(g1.prop("Footprint", "osupad:" + p.footprint, sx, sy, hide=True))
        out.append(g1.prop("Datasheet", "", sx, sy, hide=True))
        out.append(g1.prop("Description", p.description, sx, sy, hide=True))
        out.append(g1.prop("LCSC", p.lcsc, sx, sy, hide=True))
        out.append(g1.prop("MPN", p.mpn, sx, sy, hide=True))
        for number in pins:
            out.append('\t\t(pin "%s" (uuid "%s"))\n' % (number, g1.uid()))
        out.append('\t\t(instances (project "%s" (path "/%s" (reference "%s") (unit 1))))\n\t)\n'
                   % (PROJECT, root, p.ref))
        for number, (px, py, angle) in pins.items():
            x, y = sx + px, sy - py
            net = p.pins.get(number)
            if net is None:
                out.append('\t(no_connect (at %.2f %.2f) (uuid "%s"))\n' % (x, y, g1.uid()))
                continue
            label_angle = (angle + 180) % 360
            justify = {0: "left bottom", 90: "left bottom", 180: "right bottom", 270: "right bottom"}[label_angle]
            out.append('\t(label "%s" (at %.2f %.2f %d) (effects (font (size 1.27 1.27)) (justify %s)) (uuid "%s"))\n'
                       % (net, x, y, label_angle, justify, g1.uid()))
    out.append('\t(sheet_instances (path "/" (page "1")))\n)\n')
    with open(os.path.join(directory, PROJECT + ".kicad_sch"), "w") as f:
        f.write("".join(out))


# -----------------------------------------------------------------------------
# Board
# -----------------------------------------------------------------------------

def check_orientation(bb):
    """The two parts that broke V1.0 and the cable: same pad layout as the built V1.1 board."""
    v1 = {"U": {"1": (-0.95, -1.0), "2": (0.95, -1.0), "3": (0.0, 1.0)},
          "J1": {str(i + 1): (-3.5 + i, 2.0) for i in range(8)}}
    for ref, want in [("U1", v1["U"]), ("U2", v1["U"]), ("U3", v1["U"]), ("J1", v1["J1"])]:
        fp = bb.fps[ref]
        c = fp.GetPosition()
        for pad in fp.Pads():
            if pad.GetNumber() in want:
                q = pad.GetPosition()
                got = (round(pcbnew.ToMM(q.x - c.x), 3), round(pcbnew.ToMM(q.y - c.y), 3))
                assert got == want[pad.GetNumber()], "%s pad %s at %s, V1.1 has %s" % (
                    ref, pad.GetNumber(), got, want[pad.GetNumber()])


# GND stitching vias: the front edge, between the keys and holes on the key
# line, and the empty ends of the rear strip. The top pour has no pads of
# its own, so without them it is one big island and KiCad removes it.
# route_he_v2.py adds the ones that clear the routing, after routing: the
# router wastes its time on them otherwise
STITCH = ([(x, 92.0) for x in (114.0, 120.0, 126.0, 133.0, 140.0, 155.0, 162.0, 170.0, 177.0, 181.0)]
          + [(x, KEY_Y) for x in (119.8, 138.0, 157.05, 175.2)]
          + [(115.0, 72.0), (115.0, 78.0), (121.0, 78.0), (127.0, 78.0), (134.0, 78.0), (137.0, 74.0)]
          + [(170.0, 72.0), (176.0, 72.0), (181.0, 72.0), (170.0, 78.0), (176.0, 78.0), (181.0, 78.0)])


def keepout(bb, x, y, half):
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
    z.SetZoneName("sensor keepout")
    bb.board.Add(z)


def write_board(directory, parts_list):
    sheet = PROJECT + ".kicad_sch"
    bb = g1.BoardBuilder(PROJECT)
    bb.outline([(X0, Y0), (X1, Y0), (X1, Y1), (X0, Y1)], 1.5)
    for p in parts_list:
        fp = bb.part(p, sheet)
        if p.symbol == "TestPoint":
            # Bare copper: no part, no paste, not in the BOM or the placement file
            fp.SetExcludedFromBOM(True)
            fp.SetExcludedFromPosFiles(True)
            fp.SetDNP(True)
    check_orientation(bb)
    for ref, number in PAD_TOWARDS_REAR.items():
        fp = bb.fps[ref]
        pads = {p.GetNumber(): p.GetPosition().y for p in fp.Pads()}
        if pads[number] > min(pads.values()):
            fp.SetOrientationDegrees(fp.GetOrientationDegrees() + 180)
    for pad in bb.fps["U4"].Pads():
        if not pad.GetNetname():
            pad.SetNet(bb.net("unconnected-(U4-%s-Pad%s)" % (ADC_PIN_NAMES[pad.GetNumber()], pad.GetNumber())))
    for i, (x, y) in enumerate(HOLES):
        bb.hole("H%d" % (i + 1), x, y)
    # Plate-mount peg holes of each magnetic switch (V1 footprint, holes only)
    for k, x in enumerate(KEYS, 1):
        fp = pcbnew.FootprintLoad(FP_LIB, "SW_MX_Hall_PlateMount_SolidCentre")
        fp.SetFPID(pcbnew.LIB_ID("osupad", "SW_MX_Hall_PlateMount_SolidCentre"))
        bb.board.Add(fp)
        fp.SetPosition(g1.pt(x, KEY_Y))   # top side: the switch sits there, the pegs go through
        fp.SetReference("SW%d" % k)
        fp.SetBoardOnly(True)
        fp.SetExcludedFromBOM(True)
        fp.SetExcludedFromPosFiles(True)
        fp.Reference().SetVisible(False)
        keepout(bb, x, KEY_Y, 1.5)
    # The ADC's exposed pad: four vias into the top ground plane (datasheet
    # 9.4.1: short, low-impedance ground); tented, as TI's note 5 asks
    ux, uy = bb.pad("U4", "17")
    for dx in (-0.45, 0.45):
        for dy in (-0.45, 0.45):
            bb.via("GND", ux + dx, uy + dy, 0.6, 0.3)
    for layer in (pcbnew.F_Cu, pcbnew.B_Cu):
        bb.zone("GND", layer, (X0, Y0, X1, Y1), clearance=0.3)

    S, Fab = pcbnew.B_SilkS, pcbnew.B_Fab
    bb.text("OPad HE module " + REV, KEYS[1], 91.0, S, 0.9)
    for k, x in enumerate(KEYS, 1):
        bb.text("KEY%d" % k, x, 88.4, S, 0.8)
    bb.text("JLCJLCJLCJLC", 123.0, 71.0, S, 0.8)
    bb.text("1 3V3 2 GND 3 CS 4 NC 5 ID 6 MISO 7 MOSI 8 SCLK", J1_POS[0], 67.0, Fab, 0.8)
    bb.text("to carrier", 131.0, 71.0, S, 0.8)
    path = bb.save(directory, TITLE, (X0, Y1))
    # 0.5 mm pitch QFN: 0.2 mm tracks and 0.15 mm clearance (JLCPCB's
    # standard 2-layer process does 0.1 / 0.1), in the board and the project
    board = pcbnew.LoadBoard(path)
    nc = board.GetDesignSettings().m_NetSettings.GetDefaultNetclass()
    nc.SetClearance(pcbnew.FromMM(NET_CLEARANCE))
    nc.SetTrackWidth(pcbnew.FromMM(NET_TRACK))
    board.Save(path)
    return path


NET_CLEARANCE = 0.15
NET_TRACK = 0.2


def write_project(directory):
    import json
    g1.write_project(directory, PROJECT)
    path = os.path.join(directory, PROJECT + ".kicad_pro")
    pro = json.load(open(path))
    pro["net_settings"]["classes"][0].update(clearance=NET_CLEARANCE, track_width=NET_TRACK)
    pro["board"]["design_settings"]["track_widths"] = [0.0, NET_TRACK, 0.3, 0.4]
    with open(path, "w") as f:
        json.dump(pro, f, indent=2)


def main():
    write_library()
    directory = os.path.join(V2, "HE")
    os.makedirs(directory, exist_ok=True)
    plist = parts()
    write_schematic(directory, plist)
    path = write_board(directory, plist)
    write_project(directory)   # its lib tables point at ${KIPRJMOD}/../lib, which is V2/lib here
    print("wrote", path)


if __name__ == "__main__":
    main()
