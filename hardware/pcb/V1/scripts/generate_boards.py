#!/usr/bin/env python3
"""Generate the osuPad V1 KiCad projects (schematic + PCB) from one netlist spec per board.

Boards:
  MX/mx_input_v1           two-key MX hot-swap input module (sits under the key plate)
  PCB Base/controller_carrier_v1   Waveshare ESP32-S3-Touch-LCD-2 header carrier (C-shaped)

This is a one-shot generator: it overwrites the .kicad_sch/.kicad_pcb/.kicad_pro files.
Once the boards are edited by hand in KiCad, edit them there and do not re-run this.

Run with the system Python that ships KiCad's pcbnew module:
  python3 hardware/pcb/V1/scripts/generate_boards.py
"""

import json
import math
import os
import re
import uuid

import pcbnew

V1 = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FP_LIB = os.path.join(V1, "lib", "osupad.pretty")
SYM_LIB = os.path.join(V1, "lib", "osupad.kicad_sym")
REV = "V1.0"
DATE = "2026-09-14"

mm = pcbnew.FromMM


def pt(x, y):
    return pcbnew.VECTOR2I(mm(x), mm(y))


def uid():
    return str(uuid.uuid4())


# -----------------------------------------------------------------------------
# Netlist spec
# -----------------------------------------------------------------------------

class Part:
    def __init__(self, ref, symbol, value, footprint, pins, sch, pcb, side="top", rot=0,
                 lcsc="", mpn="", description=""):
        self.ref = ref
        self.symbol = symbol          # symbol name in lib/osupad.kicad_sym
        self.value = value
        self.footprint = footprint    # footprint name in lib/osupad.pretty
        self.pins = pins              # {pin/pad number: net name or None (no connect)}
        self.sch = sch                # schematic position (mm, on the 2.54 grid)
        self.pcb = pcb                # board position (mm)
        self.side = side
        self.rot = rot
        self.lcsc = lcsc
        self.mpn = mpn
        self.description = description
        self.uuid = uid()


# Module connector J1 / J_MOD (JST SH 8-pin). Pin order is chosen so both boards route
# on two layers without crossings; every signal is an ADC1-capable GPIO for the Hall board.
MODULE_PINOUT = {
    "1": "3V3",
    "2": "GND",
    "3": "IN1",    # GPIO10  KEY1 (MX)  / analog 1 (Hall)
    "4": "IN2",    # GPIO7   KEY2 (MX)  / analog 2 (Hall)
    "5": "ID",     # GPIO8   module ID voltage divider (ADC)
    "6": "IO6",    # GPIO6   spare, e.g. SPI MISO
    "7": "IO4",    # GPIO4   spare, e.g. SPI MOSI / CS
    "8": "IO2",    # GPIO2   spare, e.g. SPI SCK
}

JST = dict(symbol="Conn_01x08", footprint="JST_SH_SM08B-SRSS-TB_1x08-1MP_P1.00mm_Horizontal",
           lcsc="C160407", mpn="JST SM08B-SRSS-TB(LF)(SN)")


# -----------------------------------------------------------------------------
# Footprint and board helpers
# -----------------------------------------------------------------------------

class BoardBuilder:
    def __init__(self, project):
        self.project = project
        self.board = pcbnew.BOARD()
        self.nets = {}
        self.fps = {}

    def net(self, name):
        # Root-sheet labels become "/NAME" nets; unconnected symbol pins keep KiCad's own name
        if not name.startswith("unconnected-"):
            name = "/" + name
        if name not in self.nets:
            ni = pcbnew.NETINFO_ITEM(self.board, name)
            self.board.Add(ni)
            self.nets[name] = ni
        return self.nets[name]

    def outline(self, points, radius):
        """Closed Edge.Cuts polygon with every 90 degree corner filleted."""
        n = len(points)
        segs = []
        for i in range(n):
            v = points[i]
            p = points[i - 1]
            q = points[(i + 1) % n]
            d1 = unit(v[0] - p[0], v[1] - p[1])
            d2 = unit(q[0] - v[0], q[1] - v[1])
            t1 = (v[0] - radius * d1[0], v[1] - radius * d1[1])
            t2 = (v[0] + radius * d2[0], v[1] + radius * d2[1])
            c = (t1[0] + radius * d2[0], t1[1] + radius * d2[1])
            m = unit(v[0] - c[0], v[1] - c[1])
            mid = (c[0] + radius * m[0], c[1] + radius * m[1])
            segs.append((t1, mid, t2))
        for i in range(n):
            t1, mid, t2 = segs[i]
            arc = pcbnew.PCB_SHAPE(self.board, pcbnew.SHAPE_T_ARC)
            arc.SetArcGeometry(pt(*t1), pt(*mid), pt(*t2))
            self._edge(arc)
            line = pcbnew.PCB_SHAPE(self.board, pcbnew.SHAPE_T_SEGMENT)
            line.SetStart(pt(*t2))
            line.SetEnd(pt(*segs[(i + 1) % n][0]))
            self._edge(line)

    def _edge(self, shape):
        shape.SetLayer(pcbnew.Edge_Cuts)
        shape.SetWidth(mm(0.1))
        self.board.Add(shape)

    def part(self, part, sheetfile):
        fp = pcbnew.FootprintLoad(FP_LIB, part.footprint)
        fp.SetFPID(pcbnew.LIB_ID("osupad", part.footprint))
        self.board.Add(fp)
        fp.SetReference(part.ref)
        fp.SetValue(part.value)
        fp.SetPath(pcbnew.KIID_PATH("/" + part.uuid))
        fp.SetSheetfile(sheetfile)
        fp.SetSheetname("")
        for key, value in (("Description", part.description), ("LCSC", part.lcsc), ("MPN", part.mpn)):
            fp.SetField(key, value)
            fp.GetField(key).SetVisible(False)
        # Reference designators go on the fab layer; the silkscreen only carries hand-placed labels
        fp.Reference().SetLayer(pcbnew.F_Fab)
        fp.SetPosition(pt(*part.pcb))
        if part.side == "bottom":
            fp.Flip(fp.GetPosition(), pcbnew.FLIP_DIRECTION_LEFT_RIGHT)
        fp.SetOrientationDegrees(part.rot)
        for pad in fp.Pads():
            number = pad.GetNumber()
            if not number:
                continue
            name = part.pins.get(number)
            if name is None and part.symbol.startswith("Conn_") and number.isdigit():
                name = "unconnected-(%s-Pin_%s-Pad%s)" % (part.ref, number, number)
            if name:
                pad.SetNet(self.net(name))
        self.fps[part.ref] = fp
        return fp

    def pad(self, ref, number):
        """Centre of a placed pad in mm, so tracks end exactly on pads after flips/rotations."""
        p = next(p for p in self.fps[ref].Pads() if p.GetNumber() == number).GetPosition()
        return (round(pcbnew.ToMM(p.x), 4), round(pcbnew.ToMM(p.y), 4))

    def hole(self, ref, x, y):
        fp = pcbnew.FootprintLoad(FP_LIB, "MountingHole_2.2mm_M2")
        fp.SetFPID(pcbnew.LIB_ID("osupad", "MountingHole_2.2mm_M2"))
        self.board.Add(fp)
        fp.SetPosition(pt(x, y))
        fp.SetReference(ref)
        fp.SetBoardOnly(True)
        fp.SetExcludedFromBOM(True)
        fp.SetExcludedFromPosFiles(True)
        fp.Reference().SetVisible(False)
        return fp

    def track(self, net, points, width, layer):
        for a, b in zip(points, points[1:]):
            t = pcbnew.PCB_TRACK(self.board)
            t.SetStart(pt(*a))
            t.SetEnd(pt(*b))
            t.SetWidth(mm(width))
            t.SetLayer(layer)
            t.SetNet(self.net(net))
            self.board.Add(t)

    def via(self, net, x, y, size=0.6, drill=0.3):
        v = pcbnew.PCB_VIA(self.board)
        v.SetPosition(pt(x, y))
        v.SetWidth(mm(size))
        v.SetDrill(mm(drill))
        v.SetNet(self.net(net))
        self.board.Add(v)

    def zone(self, net, layer, rect, clearance=0.3):
        z = pcbnew.ZONE(self.board)
        z.SetLayer(layer)
        z.SetNet(self.net(net))
        z.SetLocalClearance(mm(clearance))
        z.SetMinThickness(mm(0.25))
        z.SetPadConnection(pcbnew.ZONE_CONNECTION_THERMAL)
        z.SetThermalReliefGap(mm(0.3))
        z.SetThermalReliefSpokeWidth(mm(0.4))
        z.SetIslandRemovalMode(pcbnew.ISLAND_REMOVAL_MODE_ALWAYS)
        z.SetIsFilled(False)
        ol = z.Outline()
        ol.NewOutline()
        x0, y0, x1, y1 = rect
        for x, y in ((x0, y0), (x1, y0), (x1, y1), (x0, y1)):
            ol.Append(mm(x), mm(y))
        self.board.Add(z)

    def text(self, s, x, y, layer, size=0.8, thickness=0.15, angle=0):
        t = pcbnew.PCB_TEXT(self.board)
        t.SetText(s)
        t.SetPosition(pt(x, y))
        t.SetLayer(layer)
        t.SetTextSize(pcbnew.VECTOR2I(mm(size), mm(size)))
        t.SetTextThickness(mm(thickness))
        t.SetTextAngleDegrees(angle)
        if layer in (pcbnew.B_SilkS, pcbnew.B_Fab):
            t.SetMirrored(True)
        self.board.Add(t)

    def save(self, directory, title, aux_origin):
        ds = self.board.GetDesignSettings()
        for attr, value in DESIGN_RULES.items():
            setattr(ds, attr, mm(value))
        nc = ds.m_NetSettings.GetDefaultNetclass()
        nc.SetClearance(mm(0.2))
        nc.SetTrackWidth(mm(0.25))
        nc.SetViaDiameter(mm(0.6))
        nc.SetViaDrill(mm(0.3))
        ds.SetAuxOrigin(pt(*aux_origin))
        ds.SetGridOrigin(pt(*aux_origin))
        tb = self.board.GetTitleBlock()
        tb.SetTitle(title)
        tb.SetRevision(REV)
        tb.SetDate(DATE)
        tb.SetCompany("osuPad")
        filler = pcbnew.ZONE_FILLER(self.board)
        filler.Fill(self.board.Zones())
        path = os.path.join(directory, self.project + ".kicad_pcb")
        self.board.Save(path)
        return path


# JLCPCB / PCBWay standard 2-layer capabilities, with margin (mm)
DESIGN_RULES = {
    "m_MinClearance": 0.15,
    "m_CopperEdgeClearance": 0.3,
    "m_HoleClearance": 0.25,
    "m_HoleToHoleMin": 0.4,          # Kailh MX hot-swap land pattern: 0.44 mm between pin hole and peg hole
    "m_TrackMinWidth": 0.15,
    "m_ViasMinSize": 0.5,
    "m_ViasMinAnnularWidth": 0.13,
    "m_MinThroughDrill": 0.3,
    "m_MinSilkTextHeight": 0.8,
    "m_MinSilkTextThickness": 0.15,
}


def unit(dx, dy):
    length = math.hypot(dx, dy)
    return (dx / length, dy / length)


# -----------------------------------------------------------------------------
# Project files
# -----------------------------------------------------------------------------

def write_project(directory, project):
    pro = {
        "board": {
            "design_settings": {
                "defaults": {
                    "board_outline_line_width": 0.1,
                    "copper_line_width": 0.2,
                    "silk_line_width": 0.12,
                    "silk_text_size_h": 0.8,
                    "silk_text_size_v": 0.8,
                    "silk_text_thickness": 0.15,
                },
                "rules": {
                    # JLCPCB / PCBWay standard 2-layer capabilities, with margin
                    "min_clearance": 0.15,
                    "min_connection": 0.0,
                    "min_copper_edge_clearance": 0.3,
                    "min_hole_clearance": 0.25,
                    "min_hole_to_hole": 0.4,
                    "min_microvia_diameter": 0.2,
                    "min_microvia_drill": 0.1,
                    "min_resolved_spokes": 1,
                    "min_silk_clearance": 0.0,
                    "min_text_height": 0.8,
                    "min_text_thickness": 0.15,
                    "min_through_hole_diameter": 0.3,
                    "min_track_width": 0.15,
                    "min_via_annular_width": 0.13,
                    "min_via_diameter": 0.5,
                    "solder_mask_to_copper_clearance": 0.0,
                    "use_height_for_length_calcs": True,
                },
                "track_widths": [0.0, 0.25, 0.4],
                "via_dimensions": [{"diameter": 0.0, "drill": 0.0}, {"diameter": 0.6, "drill": 0.3}],
            },
        },
        "boards": [],
        "libraries": {"pinned_footprint_libs": [], "pinned_symbol_libs": []},
        "meta": {"filename": project + ".kicad_pro", "version": 3},
        "net_settings": {
            "classes": [{
                "name": "Default",
                "clearance": 0.2,
                "track_width": 0.25,
                "via_diameter": 0.6,
                "via_drill": 0.3,
                "microvia_diameter": 0.3,
                "microvia_drill": 0.1,
                "diff_pair_gap": 0.25,
                "diff_pair_via_gap": 0.25,
                "diff_pair_width": 0.2,
                "line_style": 0,
                "wire_width": 6,
                "bus_width": 12,
                "pcb_color": "rgba(0, 0, 0, 0.000)",
                "schematic_color": "rgba(0, 0, 0, 0.000)",
                "priority": 2147483647,
            }],
            "meta": {"version": 4},
        },
        "pcbnew": {"page_layout_descr_file": ""},
        "schematic": {"page_layout_descr_file": ""},
        "sheets": [["", ""]],
        "text_variables": {},
    }
    with open(os.path.join(directory, project + ".kicad_pro"), "w") as f:
        json.dump(pro, f, indent=2)
    with open(os.path.join(directory, "fp-lib-table"), "w") as f:
        f.write('(fp_lib_table\n\t(version 7)\n\t(lib (name "osupad") (type "KiCad") '
                '(uri "${KIPRJMOD}/../lib/osupad.pretty") (options "") (descr "osuPad project footprints"))\n)\n')
    with open(os.path.join(directory, "sym-lib-table"), "w") as f:
        f.write('(sym_lib_table\n\t(version 7)\n\t(lib (name "osupad") (type "KiCad") '
                '(uri "${KIPRJMOD}/../lib/osupad.kicad_sym") (options "") (descr "osuPad project symbols"))\n)\n')


# -----------------------------------------------------------------------------
# Schematic
# -----------------------------------------------------------------------------

def sexpr_block(text, start):
    depth = 0
    for i in range(start, len(text)):
        if text[i] == "(":
            depth += 1
        elif text[i] == ")":
            depth -= 1
            if depth == 0:
                return text[start:i + 1]
    raise ValueError("unbalanced s-expression")


def load_symbols():
    text = open(SYM_LIB).read()
    symbols = {}
    for name in ("R", "C", "SW_Push", "Conn_01x08", "Conn_01x14"):
        block = sexpr_block(text, text.index('(symbol "%s"' % name))
        pins = {}
        for m in re.finditer(r'\(pin\s+\w+\s+\w+\s*\(at ([-\d.]+) ([-\d.]+) ([-\d.]+)\).*?\(number "([^"]+)"',
                             block, re.S):
            pins[m.group(4)] = (float(m.group(1)), float(m.group(2)), int(float(m.group(3))))
        embedded = block.replace('(symbol "%s"' % name, '(symbol "osupad:%s"' % name, 1)
        symbols[name] = (embedded, pins)
    return symbols


def prop(name, value, x, y, hide=False, angle=0):
    h = " (hide yes)" if hide else ""
    return ('\t\t(property "%s" "%s" (at %.2f %.2f %d)%s (effects (font (size 1.27 1.27))))\n'
            % (name, value, x, y, angle, h))


def write_schematic(directory, project, title, parts, notes):
    symbols = load_symbols()
    root = uid()
    out = ['(kicad_sch\n\t(version 20250114)\n\t(generator "eeschema")\n\t(generator_version "9.0")\n',
           '\t(uuid "%s")\n\t(paper "A4")\n' % root,
           '\t(title_block (title "%s") (date "%s") (rev "%s") (company "osuPad"))\n' % (title, DATE, REV),
           '\t(lib_symbols\n']
    for name in sorted({p.symbol for p in parts}):
        out.append("\t\t" + symbols[name][0] + "\n")
    out.append("\t)\n")

    for i, note in enumerate(notes):
        out.append('\t(text "%s" (exclude_from_sim no) (at 20.32 %.2f 0) (effects (font (size 1.27 1.27)) '
                   '(justify left bottom)) (uuid "%s"))\n' % (note, 160.02 + i * 3.81, uid()))

    for p in parts:
        sx, sy = p.sch
        pins = symbols[p.symbol][1]
        vertical = p.symbol in ("R", "C")
        out.append('\t(symbol (lib_id "osupad:%s") (at %.2f %.2f 0) (unit 1) (exclude_from_sim no) '
                   '(in_bom yes) (on_board yes) (dnp no) (uuid "%s")\n' % (p.symbol, sx, sy, p.uuid))
        if vertical:
            out.append(prop("Reference", p.ref, sx + 2.54, sy - 1.27))
            out.append(prop("Value", p.value, sx + 2.54, sy + 1.27))
        elif p.symbol == "SW_Push":
            out.append(prop("Reference", p.ref, sx, sy - 5.08))
            out.append(prop("Value", p.value, sx, sy + 3.81))
        else:
            top = max(y for _, y, _ in pins.values())
            bottom = min(y for _, y, _ in pins.values())
            out.append(prop("Reference", p.ref, sx, sy - top - 3.81))
            out.append(prop("Value", p.value, sx, sy - bottom + 3.81))
        out.append(prop("Footprint", "osupad:" + p.footprint, sx, sy, hide=True))
        out.append(prop("Datasheet", "", sx, sy, hide=True))
        out.append(prop("Description", p.description, sx, sy, hide=True))
        out.append(prop("LCSC", p.lcsc, sx, sy, hide=True))
        out.append(prop("MPN", p.mpn, sx, sy, hide=True))
        for number in pins:
            out.append('\t\t(pin "%s" (uuid "%s"))\n' % (number, uid()))
        out.append('\t\t(instances (project "%s" (path "/%s" (reference "%s") (unit 1))))\n\t)\n'
                   % (project, root, p.ref))

        for number, (px, py, angle) in pins.items():
            x, y = sx + px, sy - py
            net = p.pins.get(number)
            if net is None:
                out.append('\t(no_connect (at %.2f %.2f) (uuid "%s"))\n' % (x, y, uid()))
                continue
            label_angle = (angle + 180) % 360
            justify = {0: "left bottom", 90: "left bottom", 180: "right bottom", 270: "right bottom"}[label_angle]
            out.append('\t(label "%s" (at %.2f %.2f %d) (fields_autoplaced yes) (effects (font (size 1.27 1.27)) '
                       '(justify %s)) (uuid "%s"))\n' % (net, x, y, label_angle, justify, uid()))

    out.append('\t(sheet_instances (path "/" (page "1")))\n)\n')
    with open(os.path.join(directory, project + ".kicad_sch"), "w") as f:
        f.write("".join(out))


# -----------------------------------------------------------------------------
# MX input module
# -----------------------------------------------------------------------------
# PCB coordinates: X = case X + 100, Y = 100 - case Y (KiCad Y points down, so the rear of
# the case, towards the screen, is up in the editor). Case coordinates are the ones in
# hardware/3d/custom_case/V1/osupad_enclosure.scad (origin: front-left outer corner).

def case_to_pcb(x, y):
    return (100.0 + x, 100.0 - y)


def build_mx():
    project = "mx_input_v1"
    directory = os.path.join(V1, "MX")
    sheet = project + ".kicad_sch"

    sw1 = case_to_pcb(38.0 - 19.05 / 2, 16.5)   # (128.475, 83.5)
    sw2 = case_to_pcb(38.0 + 19.05 / 2, 16.5)   # (147.525, 83.5)
    j1 = case_to_pcb(38.0, 27.825)              # mouth at the rear board edge

    mod_pins = {n: (net if net in ("3V3", "GND", "IN1", "IN2", "ID") else None) for n, net in MODULE_PINOUT.items()}
    parts = [
        Part("J1", JST["symbol"], "MODULE", JST["footprint"], mod_pins, (60.96, 60.96), j1, "bottom", 0,
             JST["lcsc"], JST["mpn"], "Module connector to controller carrier (JST SH 8-pin)"),
        Part("SW1", "SW_Push", "KEY1", "Kailh_CPG151101S11_MX_Hotswap_Bottom", {"1": "GND", "2": "IN1"},
             (132.08, 50.8), sw1, "bottom", 180, "C41430893", "Kailh CPG151101S11",
             "MX hot-swap socket, left key"),
        Part("SW2", "SW_Push", "KEY2", "Kailh_CPG151101S11_MX_Hotswap_Bottom", {"1": "IN2", "2": "GND"},
             (132.08, 76.2), sw2, "bottom", 180, "C41430893", "Kailh CPG151101S11",
             "MX hot-swap socket, right key"),
        Part("R1", "R", "10k", "R_0603_1608Metric", {"1": "3V3", "2": "IN1"},
             (106.68, 50.8), (131.0, 75.2), "bottom", 0, "C25804", "UNI-ROYAL 0603WAF1002T5E",
             "KEY1 pull-up"),
        Part("R2", "R", "10k", "R_0603_1608Metric", {"1": "IN2", "2": "3V3"},
             (106.68, 76.2), (145.5, 75.2), "bottom", 0, "C25804", "UNI-ROYAL 0603WAF1002T5E",
             "KEY2 pull-up"),
        Part("R3", "R", "100k", "R_0603_1608Metric", {"1": "ID", "2": "3V3"},
             (157.48, 50.8), (150.5, 72.0), "bottom", 0, "C25803", "UNI-ROYAL 0603WAF1003T5E",
             "Module ID divider, top (MX V1 = 100k/10k = 0.30 V)"),
        Part("R4", "R", "10k", "R_0603_1608Metric", {"1": "ID", "2": "GND"},
             (157.48, 76.2), (150.5, 74.0), "bottom", 0, "C25804", "UNI-ROYAL 0603WAF1002T5E",
             "Module ID divider, bottom"),
        Part("C1", "C", "100nF", "C_0603_1608Metric", {"1": "GND", "2": "3V3"},
             (182.88, 63.5), (130.4, 72.0), "bottom", 0, "C14663", "YAGEO CC0603KRX7R9BB104",
             "3V3 decoupling at the module connector"),
    ]

    write_schematic(directory, project, "osuPad MX Input Module V1", parts, [
        "Module connector pinout (same on every input module):",
        "1 3V3 | 2 GND | 3 IN1 = GPIO10 | 4 IN2 = GPIO7 | 5 ID = GPIO8 | 6 GPIO6 | 7 GPIO4 | 8 GPIO2",
        "MX V1: keys switch IN1/IN2 to GND (10k pull-ups). ID = 3V3 x 10k/(100k+10k) = 0.30 V.",
        "Sockets, J1 and all passives are on the bottom side; the switches sit on top under the plate.",
    ])

    bb = BoardBuilder(project)
    x0, y0 = case_to_pcb(12.0, 31.0)
    x1, y1 = case_to_pcb(64.0, 7.0)
    bb.outline([(x0, y0), (x1, y0), (x1, y1), (x0, y1)], 1.5)
    for p in parts:
        bb.part(p, sheet)
    bb.hole("H1", *case_to_pcb(15.5, 16.5))
    bb.hole("H2", *case_to_pcb(60.5, 16.5))

    B, F = pcbnew.B_Cu, pcbnew.F_Cu
    P = bb.pad
    j = {n: P("J1", n) for n in MODULE_PINOUT}
    # IN1: J1-3 -> SW1 pad 2, and R1 pad 2 -> SW1 pad 2
    bb.track("IN1", [j["3"], (j["3"][0], 76.0), P("SW1", "2")], 0.25, B)
    bb.track("IN1", [P("R1", "2"), (133.3, 76.7), P("SW1", "2")], 0.25, B)
    # IN2: J1-4 -> SW2 pad 1, and R2 pad 1 -> SW2 pad 1
    sw2_in = P("SW2", "1")
    bb.track("IN2", [j["4"], (j["4"][0], 76.2), (sw2_in[0], 79.14), sw2_in], 0.25, B)
    bb.track("IN2", [P("R2", "1"), (141.4, 78.5), sw2_in], 0.25, B)
    # ID: J1-5 -> R3 pad 1 -> R4 pad 1
    bb.track("ID", [j["5"], (j["5"][0], 72.0), P("R3", "1"), P("R4", "1")], 0.25, B)
    # 3V3: J1-1 -> C1 pad 2 -> via; R1/R2/R3 reach a top-side 3V3 track through vias
    c1_3v3 = P("C1", "2")
    bb.track("3V3", [j["1"], (j["1"][0], 72.9), (c1_3v3[0] + 0.9, c1_3v3[1]), c1_3v3, (c1_3v3[0], 70.7)], 0.3, B)
    bb.via("3V3", c1_3v3[0], 70.7)
    r1_3v3 = P("R1", "1")
    bb.track("3V3", [r1_3v3, (128.9, r1_3v3[1])], 0.3, B)
    bb.via("3V3", 128.9, r1_3v3[1])
    r2_3v3 = P("R2", "2")
    bb.track("3V3", [r2_3v3, (147.3, r2_3v3[1])], 0.3, B)
    bb.via("3V3", 147.3, r2_3v3[1])
    r3_3v3 = P("R3", "2")
    bb.track("3V3", [r3_3v3, (152.6, r3_3v3[1])], 0.3, B)
    bb.via("3V3", 152.6, r3_3v3[1])
    bb.track("3V3", [(128.9, r1_3v3[1]), (c1_3v3[0], 70.7), (146.0, 70.7), (147.3, 72.0), (147.3, r2_3v3[1])], 0.3, F)
    bb.track("3V3", [(147.3, 72.0), (152.6, r3_3v3[1])], 0.3, F)
    # GND: pads to vias into both ground pours
    bb.track("GND", [j["2"], (j["2"][0], 75.8)], 0.3, B)
    bb.via("GND", j["2"][0], 75.8)
    c1_gnd = P("C1", "1")
    bb.track("GND", [c1_gnd, (c1_gnd[0], 70.7)], 0.3, B)
    bb.via("GND", c1_gnd[0], 70.7)
    r4_gnd = P("R4", "2")
    bb.track("GND", [r4_gnd, (152.6, r4_gnd[1])], 0.3, B)
    bb.via("GND", 152.6, r4_gnd[1])
    sw1_gnd = P("SW1", "1")
    bb.track("GND", [sw1_gnd, (sw1_gnd[0], 78.9)], 0.4, B)
    bb.via("GND", sw1_gnd[0], 78.9)
    sw2_gnd = P("SW2", "2")
    bb.track("GND", [sw2_gnd, (sw2_gnd[0], 76.5)], 0.4, B)
    bb.via("GND", sw2_gnd[0], 76.5)
    for x, y in ((114.0, 71.0), (162.0, 71.0), (114.0, 91.0), (162.0, 91.0), (138.0, 91.0), (138.0, 86.5)):
        bb.via("GND", x, y)
    for layer in (F, B):
        bb.zone("GND", layer, (x0, y0, x1, y1))

    S, Fab = pcbnew.B_SilkS, pcbnew.B_Fab
    bb.text("osuPad MX module " + REV, 138.0, 89.6, S, 0.9)
    bb.text("1 3V3  2 GND  3 IN1  4 IN2  5 ID", j1[0], 76.9, Fab, 0.8)
    bb.text("KEY1", sw1[0] - 4.0, 87.5, S, 0.8)
    bb.text("KEY2", sw2[0] + 4.0, 87.5, S, 0.8)
    bb.text("JLCJLCJLCJLC", 120.5, 70.9, S, 0.8)
    bb.text("to carrier", j1[0] + 9.8, 70.6, S, 0.8)

    bb.save(directory, "osuPad MX Input Module V1", (x0, y1))
    write_project(directory, project)
    return directory, project


# -----------------------------------------------------------------------------
# Controller carrier
# -----------------------------------------------------------------------------
# PCB coordinates: the carrier seen from its TOP side, the side that faces the back of the
# Waveshare board. Waveshare outline 35.0 x 48.2 mm at (100,100)-(135,148.2), USB-C end at
# the bottom (Y = 148.2). Seen from this side P1 is on the right and P2 on the left, both with
# pin 1 at the top (7.59 mm from the non-USB edge), rows 30.48 mm apart (Waveshare STEP model).

def build_carrier():
    project = "controller_carrier_v1"
    directory = os.path.join(V1, "PCB Base")
    sheet = project + ".kicad_sch"

    p1x, p2x, pin1y = 132.74, 102.26, 107.59
    header_y = {n: pin1y + 2.54 * (n - 1) for n in range(1, 15)}

    p1_nets = {1: "IO2", 2: "IO4", 3: "IO6", 8: "ID", 9: "IN2", 10: "IN1"}
    p2_nets = {1: "3V3", 2: "GND"}
    p1_names = ["GPIO2", "GPIO4", "GPIO6", "GPIO16", "GPIO17", "GPIO18", "GPIO21", "GPIO8", "GPIO7",
                "GPIO10", "GPIO20 USB_D+", "GPIO19 USB_D-", "GND", "5V"]
    p2_names = ["3V3", "GND", "GPIO43 TXD", "GPIO44 RXD", "GPIO47", "GPIO48", "GPIO15", "GPIO13",
                "GPIO11", "GPIO12", "GPIO14", "GPIO9", "GND", "VBAT"]

    j_mod = (117.5, 103.2)
    parts = [
        Part("J_P1", "Conn_01x14", "WAVESHARE_P1", "PinSocket_1x14_P2.54mm_Vertical",
             {str(n): p1_nets.get(n) for n in range(1, 15)}, (152.4, 76.2), (p1x, pin1y), "top", 0,
             "C2897377", "HCTL PM254-1-14-Z-8.5", "Female socket for Waveshare header P1 (8.5 mm)"),
        Part("J_P2", "Conn_01x14", "WAVESHARE_P2", "PinSocket_1x14_P2.54mm_Vertical",
             {str(n): p2_nets.get(n) for n in range(1, 15)}, (60.96, 76.2), (p2x, pin1y), "top", 0,
             "C2897377", "HCTL PM254-1-14-Z-8.5", "Female socket for Waveshare header P2 (8.5 mm)"),
        Part("J_MOD", JST["symbol"], "MODULE", JST["footprint"], dict(MODULE_PINOUT), (106.68, 116.84),
             j_mod, "bottom", 0, JST["lcsc"], JST["mpn"], "Input module connector (JST SH 8-pin)"),
    ]

    notes = ["P1 header (GPIO names): " + ", ".join("%d %s" % (i + 1, n) for i, n in enumerate(p1_names)),
             "P2 header (GPIO names): " + ", ".join("%d %s" % (i + 1, n) for i, n in enumerate(p2_names)),
             "Module connector: 1 3V3 | 2 GND | 3 IN1 = GPIO10 | 4 IN2 = GPIO7 | 5 ID = GPIO8 | 6 GPIO6 | 7 GPIO4 | 8 GPIO2",
             "All six module signals are ADC1-capable (ADC continuous mode on the ESP32-S3 is ADC1 only).",
             "Passive carrier: the module ID divider lives on each input module."]
    write_schematic(directory, project, "osuPad Controller Carrier V1", parts, notes)

    bb = BoardBuilder(project)
    bb.outline([(100.0, 100.0), (135.0, 100.0), (135.0, 148.2), (128.0, 148.2), (128.0, 113.5),
                (105.2, 113.5), (105.2, 148.2), (100.0, 148.2)], 1.0)
    for p in parts:
        bb.part(p, sheet)
    for i, (x, y) in enumerate(((103.0, 103.0), (132.0, 103.0), (103.0, 145.2), (132.0, 145.2))):
        bb.hole("H%d" % (i + 1), x, y)

    B, F = pcbnew.B_Cu, pcbnew.F_Cu
    jx = {n: bb.pad("J_MOD", n)[0] for n in MODULE_PINOUT}
    jy = bb.pad("J_MOD", "1")[1]
    # Power from P2 (left rail)
    bb.track("3V3", [(jx["1"], jy), (jx["1"], header_y[1]), (p2x, header_y[1])], 0.4, B)
    bb.track("GND", [(jx["2"], jy), (jx["2"], header_y[2]), (p2x, header_y[2])], 0.4, B)
    # Short signals to P1 pins 1-3 on the bottom layer
    bb.track("IO2", [(jx["8"], jy), (jx["8"], header_y[1]), (p1x, header_y[1])], 0.25, B)
    bb.track("IO4", [(jx["7"], jy), (jx["7"], header_y[2]), (p1x, header_y[2])], 0.25, B)
    bb.track("IO6", [(jx["6"], jy), (jx["6"], header_y[3]), (p1x, header_y[3])], 0.25, B)
    # Long signals to P1 pins 8-10 drop to the top layer and run down the inside of the right rail
    for pin, header_pin, lane_y, rail_x in (("5", 8, 108.0, 129.9), ("4", 9, 108.8, 129.3), ("3", 10, 109.6, 128.7)):
        net = MODULE_PINOUT[pin]
        bb.track(net, [(jx[pin], jy), (jx[pin], 107.0)], 0.25, B)
        bb.via(net, jx[pin], 107.0)
        bb.track(net, [(jx[pin], 107.0), (jx[pin] + 1.0, lane_y) if pin == "5" else (jx[pin], lane_y),
                       (rail_x, lane_y), (rail_x, header_y[header_pin]), (p1x, header_y[header_pin])], 0.25, F)

    S = pcbnew.B_SilkS
    bb.text("osuPad carrier " + REV, 117.5, 111.6, S, 0.8)
    bb.text("1 3V3  2 GND  3 IN1  4 IN2  5 ID  6 IO6  7 IO4  8 IO2", 117.5, 101.3, pcbnew.B_Fab, 0.8)
    bb.text("P2", p2x + 1.5, 116.0, S, 0.8, 0.15, 90)
    bb.text("P1", p1x - 3.3, 118.5, S, 0.8, 0.15, 90)
    bb.text("USB-C END", 130.4, 139.0, S, 0.8, 0.15, 90)
    bb.text("JLCJLCJLCJLC", 130.4, 124.0, S, 0.8, 0.15, 90)
    bb.text("WAVESHARE ON THIS SIDE", 117.5, 111.6, pcbnew.F_SilkS, 0.8)

    bb.save(directory, "osuPad Controller Carrier V1", (100.0, 148.2))
    write_project(directory, project)
    return directory, project


# -----------------------------------------------------------------------------
# Mechanical reference (case coordinates, top view, mm)
# -----------------------------------------------------------------------------

def write_dxf():
    entities = []

    def line(layer, a, b):
        entities.append("0\nLINE\n8\n%s\n10\n%.3f\n20\n%.3f\n30\n0\n11\n%.3f\n21\n%.3f\n31\n0\n"
                        % (layer, a[0], a[1], b[0], b[1]))

    def rect(layer, x0, y0, x1, y1):
        for a, b in (((x0, y0), (x1, y0)), ((x1, y0), (x1, y1)), ((x1, y1), (x0, y1)), ((x0, y1), (x0, y0))):
            line(layer, a, b)

    def circle(layer, x, y, r):
        entities.append("0\nCIRCLE\n8\n%s\n10\n%.3f\n20\n%.3f\n30\n0\n40\n%.3f\n" % (layer, x, y, r))

    rect("CASE", 0, 0, 76, 86)
    for x in (38.0 - 19.05 / 2, 38.0 + 19.05 / 2):
        rect("MX_PLATE_CUTOUT", x - 7, 16.5 - 7, x + 7, 16.5 + 7)
        circle("SW_CENTER", x, 16.5, 0.5)
    rect("MX_PCB", 12, 7, 64, 31)
    for x in (15.5, 60.5):
        circle("MX_PCB_M2_HOLE", x, 16.5, 1.1)
    rect("MX_PCB_J1_BODY_BOTTOM", 33.0, 26.15, 43.0, 30.4)
    with open(os.path.join(V1, "mechanical_reference_case_coords.dxf"), "w") as f:
        f.write("0\nSECTION\n2\nENTITIES\n" + "".join(entities) + "0\nENDSEC\n0\nEOF\n")


if __name__ == "__main__":
    for d, p in (build_mx(), build_carrier()):
        print("wrote", os.path.join(d, p))
    write_dxf()
