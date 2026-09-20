#!/usr/bin/env python3
"""Single source of truth for the osuPad Hall Effect input module (he_input_v1).

Everything downstream -- Gerbers, drills, BOM, CPL, and the KiCad schematic and
board -- is derived from the tables in this module, so the fabrication data and
the editable KiCad files cannot drift apart.

Coordinate system
-----------------
Board coordinates: millimetres, origin at the **bottom-left** corner of the
outline, X to the right (0 .. 52), Y **up** (0 .. 24).  Y up matches Gerber and
Excellon, so the fabrication writers use these numbers directly.

KiCad's editor Y axis points down, and this project places the module at
X = 112 .. 164, Y = 69 .. 93 (see ``case_to_pcb`` in generate_boards.py), so::

    kicad_x = 112 + board_x
    kicad_y =  93 - board_y

Bottom-side placement
---------------------
Every component is on the bottom copper (B.Cu) so JLCPCB assembles one side.
The footprints in ``lib/osupad.pretty`` are drawn on F.Cu with KiCad's Y-down
convention.  Flipping a footprint to the back negates the local Y, and
converting to this module's Y-up board frame negates it again, so for a
bottom-side part at 0 degrees::

    board_pad = part_centre + footprint_local

with no sign changes at all.  That identity is verified against the already
fabricated MX module in ``selftest()``.

Architecture (see also V1/README.md)
------------------------------------
Architecture A, bottom-side SMT on solid FR-4: there is **no** centre stem hole
at the key centres.  U1 and U2 sit directly under the switch centres on the
bottom copper, and a 3.0 x 3.0 mm copper keepout on both layers keeps the
ground pour out of the magnetic path so the stem magnet's flux is not damped by
eddy currents.

This is not a compromise forced by single-side assembly; it is what the switch
standard requires.  In a Lekker / Gateron KS-20 class magnetic switch the magnet
sits at the centre of the switch and reaches the bottom face of the housing, so
there is no MX-style centre pole and no room for a component on the top side
either.  What does stick out is two plastic alignment pins, which pass straight
through the PCB -- hence the 1.75 mm peg holes and the solid centre.  Wooting
describe exactly this in their Lekker update #5 (part 2) and moved their own
sensors to the PCB bottom for the same reason, compensating for the extra
distance with a stronger magnet and a more sensitive sensor variant.

These switches are deliberately NOT MX-mechanical compatible.  A real MX switch
has a centre pole and metal pins and does not belong on this board; that is what
the MX module (same outline, same connector) is for.
"""

import math

# ---------------------------------------------------------------------------
# Identity
# ---------------------------------------------------------------------------

PROJECT = "he_input_v1"
TITLE = "osuPad Hall Effect Input Module V1"
REV = "V1.0"
DATE = "2026-09-20"
COMPANY = "osuPad"

# ---------------------------------------------------------------------------
# Outline and mechanics
# ---------------------------------------------------------------------------

BOARD_W = 52.0
BOARD_H = 24.0
CORNER_R = 1.5
THICKNESS = 1.6

KEY1 = (16.475, 9.5)          # 19.05 mm MX pitch, centred on the board
KEY2 = (35.525, 9.5)

MOUNT_HOLES = [(3.5, 9.5), (48.5, 9.5)]        # M2 clearance, 2.2 mm NPTH
MOUNT_DRILL = 2.2

# Plate-mount alignment pegs, +/- 5.08 mm from each key centre. There is no
# 4.0 mm centre hole: that is where the Hall sensor lives.
PEG_OFFSET = 5.08
PEG_DRILL = 1.75
PEG_HOLES = [(KEY1[0] - PEG_OFFSET, KEY1[1]), (KEY1[0] + PEG_OFFSET, KEY1[1]),
             (KEY2[0] - PEG_OFFSET, KEY2[1]), (KEY2[0] + PEG_OFFSET, KEY2[1])]

# No copper pour within this square around each sensor, on either layer.
SENSOR_KEEPOUT = 3.0
KEEPOUTS = [(KEY1[0] - SENSOR_KEEPOUT / 2, KEY1[1] - SENSOR_KEEPOUT / 2,
             KEY1[0] + SENSOR_KEEPOUT / 2, KEY1[1] + SENSOR_KEEPOUT / 2),
            (KEY2[0] - SENSOR_KEEPOUT / 2, KEY2[1] - SENSOR_KEEPOUT / 2,
             KEY2[0] + SENSOR_KEEPOUT / 2, KEY2[1] + SENSOR_KEEPOUT / 2)]

# ---------------------------------------------------------------------------
# Design rules (JLCPCB / PCBWay standard 2-layer, with margin)
# ---------------------------------------------------------------------------

CLEARANCE = 0.2               # copper to copper, different nets
EDGE_CLEARANCE = 0.3          # copper to board outline
HOLE_CLEARANCE = 0.25         # copper to NPTH hole wall
HOLE_TO_HOLE = 0.4            # hole wall to hole wall
TRACK_W = 0.25                # signal
POWER_W = 0.3                 # 3V3 / GND
VIA_DIA = 0.6
VIA_DRILL = 0.3
MASK_EXPAND = 0.05            # solder mask opening, per side
POUR_GAP = 0.3                # pour to foreign copper
THERMAL_GAP = 0.3             # pour to own-net pad before the spokes
THERMAL_SPOKE = 0.4
SILK_W = 0.15
SILK_H = 0.8
EDGE_W = 0.1

# ---------------------------------------------------------------------------
# Footprints: pads as (number, (local_x, local_y), (w, h))
# Taken from lib/osupad.pretty, which is the library KiCad also uses.
# ---------------------------------------------------------------------------

FP_C0603 = "C_0603_1608Metric"
FP_R0603 = "R_0603_1608Metric"
FP_SOT23 = "SOT-23"
FP_JST8 = "JST_SH_SM08B-SRSS-TB_1x08-1MP_P1.00mm_Horizontal"
FP_TESTPOINT = "TestPoint_Pad_1.0x1.0mm"

FOOTPRINTS = {
    FP_C0603: [("1", (-0.775, 0.0), (0.9, 0.95)),
               ("2", (0.775, 0.0), (0.9, 0.95))],
    FP_R0603: [("1", (-0.825, 0.0), (0.8, 0.95)),
               ("2", (0.825, 0.0), (0.8, 0.95))],
    # SOT-23 / TO-236AB: pins 1 and 2 on one side, pin 3 opposite.
    FP_SOT23: [("1", (-0.95, -1.0), (0.7, 1.0)),
               ("2", (0.95, -1.0), (0.7, 1.0)),
               ("3", (0.0, 1.0), (0.7, 1.0))],
    FP_JST8: [("%d" % (i + 1), (-3.5 + i, -2.0), (0.6, 1.55)) for i in range(8)]
             + [("MP1", (-4.8, 1.875), (1.2, 1.8)),
                ("MP2", (4.8, 1.875), (1.2, 1.8))],
    FP_TESTPOINT: [("1", (0.0, 0.0), (1.0, 1.0))],
}

# Courtyard half-extents (w/2, h/2) used for placement checking only.
COURTYARD = {
    FP_C0603: (1.48, 0.73),
    FP_R0603: (1.48, 0.73),
    FP_SOT23: (1.70, 1.75),
    FP_JST8: (5.90, 3.28),
    FP_TESTPOINT: (0.75, 0.75),
}

# ---------------------------------------------------------------------------
# Netlist and placement
# ---------------------------------------------------------------------------

NET_3V3, NET_GND, NET_IN1, NET_IN2, NET_ID = "3V3", "GND", "IN1", "IN2", "ID"

# Module connector pin map, identical on every osuPad input module.
MODULE_PINOUT = {
    "1": NET_3V3,
    "2": NET_GND,
    "3": NET_IN1,   # GPIO10 / ADC1_CH9
    "4": NET_IN2,   # GPIO7  / ADC1_CH6
    "5": NET_ID,    # GPIO8  / ADC1_CH7
    "6": "IO6",     # GPIO6, spare -> TP1
    "7": "IO4",     # GPIO4, spare -> TP2
    "8": "IO2",     # GPIO2, spare -> TP3
}


class Part:
    def __init__(self, ref, value, footprint, centre, rot, pins,
                 lcsc, mpn, manufacturer, description, sch, assembled=True):
        self.ref = ref
        self.value = value
        self.footprint = footprint
        self.centre = centre
        self.rot = rot
        self.pins = pins                  # {pad number: net name or None}
        self.lcsc = lcsc
        self.mpn = mpn
        self.manufacturer = manufacturer
        self.description = description
        self.sch = sch                    # schematic sheet position, mm
        self.side = "bottom"
        # Probe pads are bare copper: they belong on the board and in the
        # schematic, but nothing is placed on them, so they stay out of the
        # BOM, the centroid file and the paste layer.
        self.assembled = assembled

    def pad_pos(self, number):
        for num, (lx, ly), _size in FOOTPRINTS[self.footprint]:
            if num == number:
                return _place(self.centre, self.rot, lx, ly)
        raise KeyError("%s has no pad %s" % (self.ref, number))

    def pads(self):
        """(number, centre, (w, h), net) in board coordinates."""
        out = []
        for num, (lx, ly), (w, h) in FOOTPRINTS[self.footprint]:
            pos = _place(self.centre, self.rot, lx, ly)
            sw, sh = (w, h) if self.rot % 180 == 0 else (h, w)
            key = "MP" if num.startswith("MP") else num
            out.append((num, pos, (sw, sh), self.pins.get(key)))
        return out


def _place(centre, rot, lx, ly):
    """Footprint-local -> board coordinates for a bottom-side part."""
    a = math.radians(rot)
    ca, sa = math.cos(a), math.sin(a)
    return (round(centre[0] + lx * ca - ly * sa, 4),
            round(centre[1] + lx * sa + ly * ca, 4))


# J1 is placed exactly where the MX module puts it so one cable fits both
# modules: signal pins inward at Y = 18.825, retention tabs at Y = 22.700,
# cable mouth facing out towards Y = 24 (the screen / controller).
J1_CENTRE = (26.0, 20.825)

# Sensitivity variants. All share the SOT-23 pinout and this footprint, so the
# board never changes -- only which reel JLCPCB loads. Sensitivity is quoted at
# VCC = 3.3 V (the datasheet numbers are for 5 V and scale by 3.3/5).
#   A1  C962987   ~66 mV/mT, linear to about +/-20 mT  -- best resolution, clips soonest
#   A2  C266131   ~33 mV/mT, linear to about +/-40 mT  -- recommended middle ground
#   A3  C266128   ~16 mV/mT, linear to about +/-80 mT  -- cannot clip, coarsest
# The magnet reads through 1.6 mm of FR-4 here, so the field at the die is well
# below a top-side design's. Wooting hit the same geometry and moved to a more
# sensitive part; measure the swing on the first batch before buying a reel.
SENSOR_VARIANTS = {"A1": "C962987", "A2": "C266131", "A3": "C266128"}
SENSOR_VARIANT = "A3"

PARTS = [
    Part("J1", "MODULE", FP_JST8, J1_CENTRE, 0,
         dict(MODULE_PINOUT, MP=NET_GND),
         "C160407", "SM08B-SRSS-TB(LF)(SN)", "JST",
         "Module connector to controller carrier (JST SH 8-pin, horizontal)",
         (60.96, 60.96)),

    Part("U1", "DRV5055" + SENSOR_VARIANT, FP_SOT23, KEY1, 180,
         {"1": NET_3V3, "2": NET_IN1, "3": NET_GND},
         SENSOR_VARIANTS[SENSOR_VARIANT], "DRV5055%sQDBZR" % SENSOR_VARIANT,
         "Texas Instruments",
         "Ratiometric linear Hall effect sensor, key 1 (left)",
         (130.81, 50.8)),

    Part("U2", "DRV5055" + SENSOR_VARIANT, FP_SOT23, KEY2, 180,
         {"1": NET_3V3, "2": NET_IN2, "3": NET_GND},
         SENSOR_VARIANTS[SENSOR_VARIANT], "DRV5055%sQDBZR" % SENSOR_VARIANT,
         "Texas Instruments",
         "Ratiometric linear Hall effect sensor, key 2 (right)",
         (130.81, 76.2)),

    Part("C1", "100nF", FP_C0603, (18.5, 18.825), 0,
         {"1": NET_GND, "2": NET_3V3},
         "C14663", "CC0603KRX7R9BB104", "YAGEO",
         "3V3 bulk decoupling at the module connector",
         (182.88, 63.5)),

    Part("C2", "100nF", FP_C0603, (19.0, 12.0), 0,
         {"1": NET_GND, "2": NET_3V3},
         "C14663", "CC0603KRX7R9BB104", "YAGEO",
         "Local supply decoupling for U1",
         (105.41, 50.8)),

    Part("C3", "100nF", FP_C0603, (38.05, 12.0), 0,
         {"1": NET_GND, "2": NET_3V3},
         "C14663", "CC0603KRX7R9BB104", "YAGEO",
         "Local supply decoupling for U2",
         (105.41, 76.2)),

    Part("R1", "100k", FP_R0603, (33.5, 21.0), 0,
         {"1": NET_ID, "2": NET_3V3},
         "C25803", "0603WAF1003T5E", "UNI-ROYAL",
         "Module ID divider, top (HE V1 = 100k/47k = 1.06 V)",
         (157.48, 50.8)),

    Part("R2", "47k", FP_R0603, (33.5, 19.0), 0,
         {"1": NET_ID, "2": NET_GND},
         "C25819", "0603WAF4702T5E", "UNI-ROYAL",
         "Module ID divider, bottom",
         (157.48, 76.2)),
]



# Spare connector pins brought to probe pads instead of being left dangling:
# J1-6/7/8 are GPIO6/4/2, reserved for an SPI variant of this module.
def _probe_pad(ref, net, x, sch_y):
    return Part(ref, net, FP_TESTPOINT, (x, 16.5), 0, {"1": net},
                "", "", "", "Probe pad for reserved connector pin " + net,
                (33.02, sch_y), assembled=False)


PARTS += [_probe_pad("TP1", "IO6", 27.5, 50.8),
          _probe_pad("TP2", "IO4", 29.2, 63.5),
          _probe_pad("TP3", "IO2", 30.9, 76.2)]

TEST_PADS = [(p.ref, p.centre, p.pins["1"]) for p in PARTS if not p.assembled]

PARTS_BY_REF = {p.ref: p for p in PARTS}

# ---------------------------------------------------------------------------
# Routing
# ---------------------------------------------------------------------------
# Signals and the local supply stubs live on B.Cu with the parts; the 3V3
# distribution bus runs on F.Cu, where the only other object is the ground
# pour.  That keeps every analog trace on one layer and avoids crossings.

P = lambda ref, pad: PARTS_BY_REF[ref].pad_pos(pad)

VIA_3V3_J1 = (19.275, 17.2)     # below C1, feeds the F.Cu bus
VIA_3V3_C2 = (19.775, 13.3)
VIA_3V3_C3 = (38.825, 13.3)
VIA_3V3_R1 = (35.5, 21.0)

TRACKS = [
    # ---- 3V3 -------------------------------------------------------------
    (NET_3V3, [P("J1", "1"), P("C1", "2")], POWER_W, "B"),
    (NET_3V3, [P("C1", "2"), VIA_3V3_J1], POWER_W, "B"),
    (NET_3V3, [VIA_3V3_C2, P("C2", "2"), (P("C2", "2")[0], 10.5), P("U1", "1")], POWER_W, "B"),
    (NET_3V3, [VIA_3V3_C3, P("C3", "2"), (P("C3", "2")[0], 10.5), P("U2", "1")], POWER_W, "B"),
    (NET_3V3, [P("R1", "2"), VIA_3V3_R1], POWER_W, "B"),
    # F.Cu distribution bus
    (NET_3V3, [VIA_3V3_C2, (VIA_3V3_C2[0], 17.2), VIA_3V3_J1], POWER_W, "F"),
    (NET_3V3, [VIA_3V3_J1, (VIA_3V3_C3[0], 17.2), VIA_3V3_C3], POWER_W, "F"),
    (NET_3V3, [(VIA_3V3_R1[0], 17.2), VIA_3V3_R1], POWER_W, "F"),

    # ---- analog outputs --------------------------------------------------
    (NET_IN1, [P("U1", "2"), (P("U1", "2")[0], 16.0), (24.5, 16.0), P("J1", "3")], TRACK_W, "B"),
    (NET_IN2, [P("U2", "2"), (P("U2", "2")[0], 14.0), (25.5, 14.0), P("J1", "4")], TRACK_W, "B"),

    # ---- module ID divider ----------------------------------------------
    (NET_ID, [P("J1", "5"), (26.5, 20.3), (32.675, 20.3)], TRACK_W, "B"),
    (NET_ID, [P("R1", "1"), P("R2", "1")], TRACK_W, "B"),

    # ---- spare pins to probe pads ---------------------------------------
    ("IO6", [P("J1", "6"), P("TP1", "1")], TRACK_W, "B"),
    ("IO4", [P("J1", "7"), (28.5, 17.5), P("TP2", "1")], TRACK_W, "B"),
    ("IO2", [P("J1", "8"), (29.5, 17.8), P("TP3", "1")], TRACK_W, "B"),
    # (TP2 / TP3 are reached on a short diagonal from the connector row)

    # ---- ground stubs out of the sensor keepouts ------------------------
    (NET_GND, [P("U1", "3"), (KEY1[0], 7.0)], POWER_W, "B"),
    (NET_GND, [P("U2", "3"), (KEY2[0], 7.0)], POWER_W, "B"),
    (NET_GND, [P("J1", "2"), (23.5, 20.8)], POWER_W, "B"),
]

VIAS = [
    (NET_3V3, VIA_3V3_J1),
    (NET_3V3, VIA_3V3_C2),
    (NET_3V3, VIA_3V3_C3),
    (NET_3V3, VIA_3V3_R1),
    (NET_GND, (KEY1[0], 7.0)),
    (NET_GND, (KEY2[0], 7.0)),
    (NET_GND, (23.5, 20.8)),
    # pour stitching, keeping both ground planes at one potential
    (NET_GND, (3.0, 3.0)), (NET_GND, (3.0, 21.0)),
    (NET_GND, (49.0, 3.0)), (NET_GND, (49.0, 21.0)),
    (NET_GND, (10.0, 3.0)), (NET_GND, (42.0, 3.0)),
    (NET_GND, (26.0, 3.0)), (NET_GND, (26.0, 6.5)),
    (NET_GND, (8.0, 15.0)), (NET_GND, (44.0, 15.0)),
    (NET_GND, (8.0, 20.5)), (NET_GND, (44.0, 20.5)),
    (NET_GND, (13.0, 6.5)), (NET_GND, (39.0, 6.5)),
]

# Ground pads that connect to the pour through a thermal relief rather than
# solid copper, so hand rework and reflow do not fight the plane.
THERMAL_PADS = [("C1", "1"), ("C2", "1"), ("C3", "1"), ("R2", "2"),
                ("J1", "2"), ("J1", "MP1"), ("J1", "MP2")]

# ---------------------------------------------------------------------------
# Silkscreen (bottom side reads mirrored through the board)
# ---------------------------------------------------------------------------

JLC_MARKER = ("JLCJLCJLCJLC", 8.5, 22.1)
PCBWAY_MARKER = ("WayWayWay", 8.5, 22.1)

BOTTOM_SILK = [
    ("OSUPAD HE " + REV, 26.0, 2.2, 0.9, "center"),
    ("KEY1", KEY1[0], 4.6, 0.8, "center"),
    ("KEY2", KEY2[0], 4.6, 0.8, "center"),
    ("U1", 13.2, 9.5, 0.7, "center"),
    ("U2", 38.8, 9.5, 0.7, "center"),
    ("C1", 18.5, 20.4, 0.7, "center"),
    ("C2", 19.0, 13.6, 0.7, "center"),
    ("C3", 38.05, 13.6, 0.7, "center"),
    ("R1", 37.0, 21.0, 0.7, "center"),
    ("R2", 37.0, 19.0, 0.7, "center"),
    ("1", 22.5, 17.3, 0.7, "center"),          # J1 pin 1
    ("TO CARRIER", 41.5, 22.4, 0.7, "center"),
    ("3V3 GND IN1 IN2 ID", 26.0, 11.6, 0.7, "center"),
]

TOP_SILK = [
    ("OSUPAD HE " + REV, 26.0, 21.6, 0.9, "center"),
    ("KEY1", KEY1[0], 13.6, 0.8, "center"),
    ("KEY2", KEY2[0], 13.6, 0.8, "center"),
    ("HALL SENSOR UNDER BOARD - DO NOT DRILL", 26.0, 1.4, 0.7, "center"),
]

# ---------------------------------------------------------------------------
# Derived collections
# ---------------------------------------------------------------------------


def all_pads():
    """(ref, number, centre, size, net, is_test_pad)."""
    out = []
    for part in PARTS:
        for num, pos, size, net in part.pads():
            out.append((part.ref, num, pos, size, net, not part.assembled))
    return out


def npth_holes():
    """(x, y, diameter) for every non-plated hole."""
    return ([(x, y, MOUNT_DRILL) for x, y in MOUNT_HOLES]
            + [(x, y, PEG_DRILL) for x, y in PEG_HOLES])


def outline_corners():
    """Outline corner centres, counter-clockwise from bottom-left."""
    r = CORNER_R
    return [(r, r), (BOARD_W - r, r), (BOARD_W - r, BOARD_H - r), (r, BOARD_H - r)]


def bom_groups():
    """[(value, [refs], footprint, lcsc, mpn, manufacturer, description)]"""
    groups = {}
    order = []
    for part in PARTS:
        if not part.assembled:
            continue
        key = (part.value, part.footprint, part.lcsc, part.mpn, part.manufacturer)
        if key not in groups:
            groups[key] = []
            order.append(key)
        groups[key].append(part)
    out = []
    for key in order:
        members = groups[key]
        value, footprint, lcsc, mpn, manufacturer = key
        out.append((value, [m.ref for m in members], footprint, lcsc, mpn,
                    manufacturer, members[0].description))
    return out


# ---------------------------------------------------------------------------
# Geometry helpers
# ---------------------------------------------------------------------------


def _clamp(v, lo, hi):
    return lo if v < lo else (hi if v > hi else v)


def _seg_seg_dist(p, q, r, s):
    """Shortest distance between segments pq and rs."""
    def sub(a, b):
        return (a[0] - b[0], a[1] - b[1])

    def dot(a, b):
        return a[0] * b[0] + a[1] * b[1]

    u, v, w = sub(q, p), sub(s, r), sub(p, r)
    a, b, c, d, e = dot(u, u), dot(u, v), dot(v, v), dot(u, w), dot(v, w)
    den = a * c - b * b
    if den > 1e-12:
        sc = _clamp((b * e - c * d) / den, 0.0, 1.0)
    else:
        sc = 0.0
    tc = (b * sc + e) / c if c > 1e-12 else 0.0
    tc = _clamp(tc, 0.0, 1.0)
    sc = _clamp((b * tc - d) / a, 0.0, 1.0) if a > 1e-12 else 0.0
    dx = w[0] + sc * u[0] - tc * v[0]
    dy = w[1] + sc * u[1] - tc * v[1]
    return math.hypot(dx, dy)


def _point_rect_dist(p, rect):
    x0, y0, x1, y1 = rect
    dx = max(x0 - p[0], 0.0, p[0] - x1)
    dy = max(y0 - p[1], 0.0, p[1] - y1)
    return math.hypot(dx, dy)


def _seg_rect_dist(p, q, rect):
    x0, y0, x1, y1 = rect
    if _point_inside(p, rect) or _point_inside(q, rect):
        return 0.0
    edges = [((x0, y0), (x1, y0)), ((x1, y0), (x1, y1)),
             ((x1, y1), (x0, y1)), ((x0, y1), (x0, y0))]
    return min(_seg_seg_dist(p, q, a, b) for a, b in edges)


def _point_inside(p, rect):
    x0, y0, x1, y1 = rect
    return x0 <= p[0] <= x1 and y0 <= p[1] <= y1


def _rect_rect_dist(a, b):
    ax0, ay0, ax1, ay1 = a
    bx0, by0, bx1, by1 = b
    dx = max(bx0 - ax1, 0.0, ax0 - bx1)
    dy = max(by0 - ay1, 0.0, ay0 - by1)
    return math.hypot(dx, dy)


def pad_rect(centre, size):
    return (centre[0] - size[0] / 2.0, centre[1] - size[1] / 2.0,
            centre[0] + size[0] / 2.0, centre[1] + size[1] / 2.0)


def _outline_segments():
    """Straight edges plus arc chords, used for copper-to-edge checking."""
    r = CORNER_R
    c = outline_corners()
    segs = [((r, 0.0), (BOARD_W - r, 0.0)),
            ((BOARD_W, r), (BOARD_W, BOARD_H - r)),
            ((BOARD_W - r, BOARD_H), (r, BOARD_H)),
            ((0.0, BOARD_H - r), (0.0, r))]
    for cx, cy in c:
        sx = 1 if cx > BOARD_W / 2 else -1
        sy = 1 if cy > BOARD_H / 2 else -1
        for k in range(8):
            a0 = math.radians(k * 90.0 / 8)
            a1 = math.radians((k + 1) * 90.0 / 8)
            segs.append(((cx + sx * r * math.cos(a0), cy + sy * r * math.sin(a0)),
                         (cx + sx * r * math.cos(a1), cy + sy * r * math.sin(a1))))
    return segs


# ---------------------------------------------------------------------------
# Design rule check
# ---------------------------------------------------------------------------


class Item:
    """A piece of copper: a rectangle (pad) or a capsule (track / via)."""

    def __init__(self, kind, net, layer, name, rect=None, seg=None, width=0.0):
        self.kind = kind
        self.net = net
        self.layer = layer
        self.name = name
        self.rect = rect
        self.seg = seg
        self.width = width

    def distance_to(self, other):
        if self.kind == "rect" and other.kind == "rect":
            return _rect_rect_dist(self.rect, other.rect)
        if self.kind == "rect" and other.kind == "cap":
            return _seg_rect_dist(other.seg[0], other.seg[1], self.rect) - other.width / 2.0
        if self.kind == "cap" and other.kind == "rect":
            return _seg_rect_dist(self.seg[0], self.seg[1], other.rect) - self.width / 2.0
        return (_seg_seg_dist(self.seg[0], self.seg[1], other.seg[0], other.seg[1])
                - self.width / 2.0 - other.width / 2.0)

    def distance_to_point(self, p):
        if self.kind == "rect":
            return _point_rect_dist(p, self.rect)
        return _seg_seg_dist(self.seg[0], self.seg[1], p, p) - self.width / 2.0

    def distance_to_seg(self, a, b):
        if self.kind == "rect":
            return _seg_rect_dist(a, b, self.rect)
        return _seg_seg_dist(self.seg[0], self.seg[1], a, b) - self.width / 2.0


def copper_items():
    items = []
    for ref, num, pos, size, net, _tp in all_pads():
        items.append(Item("rect", net, "B", "%s pad %s" % (ref, num),
                          rect=pad_rect(pos, size)))
    for net, points, width, layer in TRACKS:
        for a, b in zip(points, points[1:]):
            if a == b:
                continue
            items.append(Item("cap", net, layer, "%s track" % net,
                              seg=(a, b), width=width))
    for net, (x, y) in VIAS:
        for layer in ("F", "B"):
            items.append(Item("cap", net, layer, "%s via" % net,
                              seg=((x, y), (x, y)), width=VIA_DIA))
    return items


def check():
    """Return a list of human-readable rule violations (empty means clean)."""
    errors = []
    items = copper_items()

    # 1. copper to copper, different nets, same layer
    for i in range(len(items)):
        for j in range(i + 1, len(items)):
            a, b = items[i], items[j]
            if a.layer != b.layer or a.net == b.net:
                continue
            d = a.distance_to(b)
            if d < CLEARANCE - 1e-6:
                errors.append("clearance %.3f mm (<%.2f) between %s [%s] and %s [%s] on %s.Cu"
                              % (d, CLEARANCE, a.name, a.net, b.name, b.net, a.layer))

    # 2. copper to non-plated holes
    for hx, hy, dia in npth_holes():
        for it in items:
            d = it.distance_to_point((hx, hy)) - dia / 2.0
            if d < HOLE_CLEARANCE - 1e-6:
                errors.append("hole clearance %.3f mm (<%.2f) between NPTH d%.2f at (%.3f, %.3f) and %s [%s]"
                              % (d, HOLE_CLEARANCE, dia, hx, hy, it.name, it.net))

    # 3. copper to board edge
    for a, b in _outline_segments():
        for it in items:
            d = it.distance_to_seg(a, b)
            if d < EDGE_CLEARANCE - 1e-6:
                errors.append("edge clearance %.3f mm (<%.2f) for %s [%s]"
                              % (d, EDGE_CLEARANCE, it.name, it.net))

    # 4. hole to hole
    holes = [(x, y, d) for x, y, d in npth_holes()] + [(x, y, VIA_DRILL) for _n, (x, y) in VIAS]
    for i in range(len(holes)):
        for j in range(i + 1, len(holes)):
            ax, ay, ad = holes[i]
            bx, by, bd = holes[j]
            d = math.hypot(ax - bx, ay - by) - ad / 2.0 - bd / 2.0
            if d < HOLE_TO_HOLE - 1e-6:
                errors.append("hole-to-hole %.3f mm (<%.2f) between (%.3f, %.3f) and (%.3f, %.3f)"
                              % (d, HOLE_TO_HOLE, ax, ay, bx, by))

    # 5. courtyard overlap between placed parts
    boxes = []
    for part in PARTS:
        cw, ch = COURTYARD[part.footprint]
        if part.rot % 180 != 0:
            cw, ch = ch, cw
        boxes.append((part.ref, pad_rect(part.centre, (cw * 2, ch * 2))))
    for i in range(len(boxes)):
        for j in range(i + 1, len(boxes)):
            if _rect_rect_dist(boxes[i][1], boxes[j][1]) <= 0.0:
                errors.append("courtyard overlap between %s and %s" % (boxes[i][0], boxes[j][0]))

    # 6. every pad reachable: each net with more than one pad must appear in TRACKS
    routed = set()
    for net, points, _w, _l in TRACKS:
        routed.add(net)
    for ref, num, _pos, _size, net, _tp in all_pads():
        if net in (None, NET_GND):
            continue
        if net not in routed:
            errors.append("net %s (%s pad %s) has no track" % (net, ref, num))

    # 7. the sensors must sit on solid FR-4
    for cx, cy in (KEY1, KEY2):
        for hx, hy, dia in npth_holes():
            if math.hypot(hx - cx, hy - cy) < dia / 2.0 + 0.5:
                errors.append("hole d%.2f at (%.3f, %.3f) intrudes on the sensor at (%.3f, %.3f)"
                              % (dia, hx, hy, cx, cy))

    # 8. silkscreen must stay off solder mask openings and off the board edge
    import strokefont
    pad_boxes = [(ref, num, pad_rect(pos, (size[0] + 2 * MASK_EXPAND, size[1] + 2 * MASK_EXPAND)))
                 for ref, num, pos, size, _net, _tp in all_pads()]
    silk = [(t, x, y, h, a, "B") for t, x, y, h, a in BOTTOM_SILK]
    silk += [(t, x, y, h, a, "F") for t, x, y, h, a in TOP_SILK]
    silk += [(JLC_MARKER[0], JLC_MARKER[1], JLC_MARKER[2], SILK_H, "left", "B")]
    for text, tx, ty, height, anchor, layer in silk:
        polys = strokefont.strokes(text, tx, ty, height, anchor=anchor)
        xs = [p[0] for poly in polys for p in poly]
        ys = [p[1] for poly in polys for p in poly]
        if not xs:
            continue
        box = (min(xs) - SILK_W / 2, min(ys) - SILK_W / 2,
               max(xs) + SILK_W / 2, max(ys) + SILK_W / 2)
        if layer == "B":
            for ref, num, pbox in pad_boxes:
                if _rect_rect_dist(box, pbox) <= 0.0:
                    errors.append("silk %r overlaps the mask opening of %s pad %s"
                                  % (text, ref, num))
        if (box[0] < EDGE_W or box[1] < EDGE_W
                or box[2] > BOARD_W - EDGE_W or box[3] > BOARD_H - EDGE_W):
            errors.append("silk %r runs off the board outline (%.3f, %.3f)-(%.3f, %.3f)"
                          % (text, box[0], box[1], box[2], box[3]))

    return errors


def selftest():
    """Assert the placement invariants this module's transform depends on."""
    problems = []
    j1 = PARTS_BY_REF["J1"]
    if j1.pad_pos("1") != (22.5, 18.825):
        problems.append("J1 pin 1 at %s, expected (22.5, 18.825) to match the MX module"
                        % (j1.pad_pos("1"),))
    if j1.pad_pos("MP1") != (21.2, 22.7):
        problems.append("J1 MP tab at %s, expected (21.2, 22.7)" % (j1.pad_pos("MP1"),))
    if j1.pad_pos("8") != (29.5, 18.825):
        problems.append("J1 pin 8 at %s, expected (29.5, 18.825)" % (j1.pad_pos("8"),))
    for ref, key in (("U1", KEY1), ("U2", KEY2)):
        part = PARTS_BY_REF[ref]
        if part.centre != key:
            problems.append("%s is at %s, not on the key centre %s" % (ref, part.centre, key))
    return problems


if __name__ == "__main__":
    import sys
    bad = selftest()
    for line in bad:
        print("SELFTEST:", line)
    errs = check()
    for line in errs:
        print("DRC:", line)
    print("%s: %d selftest problem(s), %d rule violation(s)" % (PROJECT, len(bad), len(errs)))
    sys.exit(1 if (bad or errs) else 0)
