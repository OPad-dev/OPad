#!/usr/bin/env python3
"""Write the editable KiCad files for he_input_v1 from ``he_board.py``.

    python3 hardware/pcb/V1/scripts/generate_he_kicad.py

No KiCad installation is needed: the footprints are read from
``lib/osupad.pretty`` and the symbols from ``lib/osupad.kicad_sym`` as
s-expressions, transformed, and written into the board and schematic.

File format
-----------
The board is written in the KiCad 8 board syntax (``version 20240108``), which
KiCad 8, 9 and 10 all open; 9 and 10 upgrade it in place on the first save.
The schematic is KiCad 9 syntax (``version 20250114``), matching what
``generate_boards.py`` emits, because it embeds the shared symbol library and
that library is saved by KiCad 10.

Copper pours are written as zone **outlines** with their clearance, thermal
relief and minimum-width settings, plus two rule areas that forbid a pour over
the Hall sensors.  KiCad fills them from those rules when the board is opened
(Edit > Fill All Zones, or any DRC run).  The shipped Gerbers already contain
the filled result; see generate_he_gerbers.py.
"""

import json
import os
import uuid as _uuid

import he_board as B

V1 = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FP_LIB = os.path.join(V1, "lib", "osupad.pretty")
SYM_LIB = os.path.join(V1, "lib", "osupad.kicad_sym")
HE_DIR = os.path.join(V1, "HE")

# Board coordinates -> KiCad editor coordinates (see he_board's module docstring).
OX, OY = 112.0, 93.0


def kx(x):
    return round(OX + x, 4)


def ky(y):
    return round(OY - y, 4)


def uid(seed=None):
    """Deterministic UUIDs so regenerating does not churn the files."""
    if seed is None:
        return str(_uuid.uuid4())
    return str(_uuid.uuid5(_uuid.NAMESPACE_URL, "osupad/he_input_v1/" + seed))


# ---------------------------------------------------------------------------
# s-expression parsing
# ---------------------------------------------------------------------------

def parse(text):
    """Parse an s-expression into nested lists; atoms stay as source strings."""
    stack = [[]]
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c == "(":
            node = []
            stack[-1].append(node)
            stack.append(node)
            i += 1
        elif c == ")":
            stack.pop()
            i += 1
        elif c == '"':
            j = i + 1
            while j < n:
                if text[j] == "\\":
                    j += 2
                    continue
                if text[j] == '"':
                    break
                j += 1
            stack[-1].append(text[i:j + 1])
            i = j + 1
        elif c.isspace():
            i += 1
        else:
            j = i
            while j < n and not text[j].isspace() and text[j] not in '()"':
                j += 1
            stack[-1].append(text[i:j])
            i = j
    return stack[0][0]


def dump(node, depth=0):
    if isinstance(node, str):
        return node
    pad = "\t" * depth
    head = node[0] if node and isinstance(node[0], str) else ""
    simple = all(isinstance(c, str) for c in node)
    if simple:
        return "%s(%s)" % (pad, " ".join(node))
    parts = ["%s(%s" % (pad, head)]
    rest = node[1:]
    inline = []
    while rest and isinstance(rest[0], str):
        inline.append(rest.pop(0))
    if inline:
        parts[0] += " " + " ".join(inline)
    for child in rest:
        parts.append(dump(child, depth + 1))
    parts.append("%s)" % pad)
    return "\n".join(parts)


def find(node, name):
    for child in node:
        if isinstance(child, list) and child and child[0] == name:
            return child
    return None


def find_all(node, name):
    return [c for c in node if isinstance(c, list) and c and c[0] == name]


def drop(node, names):
    return [c for c in node
            if not (isinstance(c, list) and c and c[0] in names)]


# ---------------------------------------------------------------------------
# Footprint transform: library (F.Cu, Y down) -> board bottom side
# ---------------------------------------------------------------------------

COORD_NODES = {"start", "end", "center", "mid", "xy"}
FLIP_LAYER = {"F.Cu": "B.Cu", "F.Mask": "B.Mask", "F.Paste": "B.Paste",
              "F.SilkS": "B.SilkS", "F.Fab": "B.Fab", "F.CrtYd": "B.CrtYd"}


def _flip_layer_token(tok):
    raw = tok.strip('"')
    return '"%s"' % FLIP_LAYER.get(raw, raw)


def _negate_y(node, rot):
    """Negate Y on every coordinate in the tree (the bottom-side flip)."""
    if isinstance(node, str):
        return node
    head = node[0] if node and isinstance(node[0], str) else ""
    out = [node[0]] if node else []
    rest = node[1:]

    if head in COORD_NODES or head == "at":
        nums = []
        while rest and isinstance(rest[0], str):
            nums.append(rest.pop(0))
        if head == "xy":
            for k in range(0, len(nums), 2):
                nums[k + 1] = _neg(nums[k + 1])
        else:
            if len(nums) >= 2:
                nums[1] = _neg(nums[1])
            if head == "at" and rot:
                if len(nums) >= 3:
                    nums[2] = "%g" % ((float(nums[2]) + rot) % 360)
                else:
                    nums.append("%g" % (rot % 360))
        out.extend(nums)
    elif head == "layer":
        while rest and isinstance(rest[0], str):
            out.append(_flip_layer_token(rest.pop(0)))
    elif head == "layers":
        while rest and isinstance(rest[0], str):
            out.append(_flip_layer_token(rest.pop(0)))
    else:
        while rest and isinstance(rest[0], str):
            out.append(rest.pop(0))

    for child in rest:
        out.append(_negate_y(child, rot))
    return out


def _neg(token):
    value = -float(token)
    if value == 0:
        value = 0.0
    text = "%g" % value
    return text


def place_footprint(part, nets):
    path = os.path.join(FP_LIB, part.footprint + ".kicad_mod")
    tree = parse(open(path, encoding="utf8").read())
    assert tree[0] == "footprint"
    assert not find_all(tree, "fp_arc"), \
        "%s has arcs; the Y flip would reverse their winding" % part.footprint

    body = drop(tree[2:], {"version", "generator", "generator_version", "layer",
                           "property", "attr", "model"})
    body = [_negate_y(child, part.rot) for child in body]

    out = ["footprint", '"osupad:%s"' % part.footprint,
           ["layer", '"B.Cu"'],
           ["uuid", '"%s"' % uid("fp/" + part.ref)],
           ["at", "%g" % kx(part.centre[0]), "%g" % ky(part.centre[1])]
           + (["%g" % (part.rot % 360)] if part.rot % 360 else [])]

    descr = find(tree, "descr")
    if descr:
        out.append(descr)
    tags = find(tree, "tags")
    if tags:
        out.append(tags)

    angle = "%g" % ((180 + part.rot) % 360)
    fields = [("Reference", part.ref, "B.Fab", 1.43),
              ("Value", part.value, "B.Fab", -1.43),
              ("Datasheet", "", "B.Fab", 0.0),
              ("Description", part.description, "B.Fab", 0.0),
              ("LCSC", part.lcsc, "B.Fab", 0.0),
              ("MPN", ("%s %s" % (part.manufacturer, part.mpn)).strip(), "B.Fab", 0.0)]
    for name, value, layer, dy in fields:
        prop = ["property", '"%s"' % name, '"%s"' % value,
                ["at", "0", "%g" % dy, angle],
                ["layer", '"%s"' % layer]]
        if name != "Reference" and name != "Value":
            prop.append(["hide", "yes"])
        prop.append(["uuid", '"%s"' % uid("prop/%s/%s" % (part.ref, name))])
        prop.append(["effects", ["font", ["size", "1", "1"], ["thickness", "0.15"]],
                     ["justify", "mirror"]])
        out.append(prop)

    out.append(["path", '"/%s"' % uid("sym/" + part.ref)])
    out.append(["sheetfile", '"%s.kicad_sch"' % B.PROJECT])
    attrs = ["attr", "smd"]
    if not part.assembled:
        attrs += ["exclude_from_pos_files", "exclude_from_bom"]
    out.append(attrs)
    out.extend(body)

    # nets on the pads
    for pad in find_all(out, "pad"):
        number = pad[1].strip('"')
        key = "MP" if number.startswith("MP") else number
        net = part.pins.get(key)
        if number.startswith("MP"):
            pad[1] = '"MP"'
        if net:
            index, label = nets[net]
            pad.append(["net", str(index), '"%s"' % label])
        pad.append(["uuid", '"%s"' % uid("pad/%s/%s" % (part.ref, number))])
    return out


# ---------------------------------------------------------------------------
# Board
# ---------------------------------------------------------------------------

LAYERS = """	(layers
		(0 "F.Cu" signal)
		(31 "B.Cu" signal)
		(32 "B.Adhes" user "B.Adhesive")
		(33 "F.Adhes" user "F.Adhesive")
		(34 "B.Paste" user)
		(35 "F.Paste" user)
		(36 "B.SilkS" user "B.Silkscreen")
		(37 "F.SilkS" user "F.Silkscreen")
		(38 "B.Mask" user)
		(39 "F.Mask" user)
		(40 "Dwgs.User" user "User.Drawings")
		(41 "Cmts.User" user "User.Comments")
		(42 "Eco1.User" user "User.Eco1")
		(43 "Eco2.User" user "User.Eco2")
		(44 "Edge.Cuts" user)
		(45 "Margin" user)
		(46 "B.CrtYd" user "B.Courtyard")
		(47 "F.CrtYd" user "F.Courtyard")
		(48 "B.Fab" user)
		(49 "F.Fab" user)
	)"""


def net_table():
    order = ["3V3", "GND", "IN1", "IN2", "ID", "IO6", "IO4", "IO2"]
    return {name: (i + 1, "/" + name) for i, name in enumerate(order)}


def edge_cuts():
    """Four straight edges plus four 1.5 mm corner arcs."""
    out = []
    r = B.CORNER_R
    w, h = B.BOARD_W, B.BOARD_H
    lines = [((r, 0.0), (w - r, 0.0)),
             ((w, r), (w, h - r)),
             ((w - r, h), (r, h)),
             ((0.0, h - r), (0.0, r))]
    for i, (a, b) in enumerate(lines):
        out.append(["gr_line",
                    ["start", "%g" % kx(a[0]), "%g" % ky(a[1])],
                    ["end", "%g" % kx(b[0]), "%g" % ky(b[1])],
                    ["stroke", ["width", "%g" % B.EDGE_W], ["type", "default"]],
                    ["layer", '"Edge.Cuts"'],
                    ["uuid", '"%s"' % uid("edge/line/%d" % i)]])

    import math
    corners = [((r, r), 180, 270), ((w - r, r), 270, 360),
               ((w - r, h - r), 0, 90), ((r, h - r), 90, 180)]
    for i, ((cx, cy), a0, a1) in enumerate(corners):
        pts = []
        for a in (a0, (a0 + a1) / 2.0, a1):
            t = math.radians(a)
            pts.append((cx + r * math.cos(t), cy + r * math.sin(t)))
        # KiCad's Y is mirrored, so the arc runs the other way round
        start, mid, end = pts[2], pts[1], pts[0]
        out.append(["gr_arc",
                    ["start", "%g" % kx(start[0]), "%g" % ky(start[1])],
                    ["mid", "%g" % kx(mid[0]), "%g" % ky(mid[1])],
                    ["end", "%g" % kx(end[0]), "%g" % ky(end[1])],
                    ["stroke", ["width", "%g" % B.EDGE_W], ["type", "default"]],
                    ["layer", '"Edge.Cuts"'],
                    ["uuid", '"%s"' % uid("edge/arc/%d" % i)]])
    return out


def silk_texts():
    out = []
    for i, (text, x, y, height, _anchor) in enumerate(B.BOTTOM_SILK):
        out.append(_text(text, x, y, height, "B.SilkS", "silk/b/%d" % i, mirror=True))
    marker, mx, my = B.JLC_MARKER
    width = len(marker) * strokes_advance(B.SILK_H)
    out.append(_text(marker, mx + width / 2.0, my, B.SILK_H, "B.SilkS",
                     "silk/b/marker", mirror=True))
    for i, (text, x, y, height, _anchor) in enumerate(B.TOP_SILK):
        out.append(_text(text, x, y, height, "F.SilkS", "silk/f/%d" % i))
    return out


def strokes_advance(height):
    import strokefont
    return strokefont.ADVANCE * (height / strokefont.CAP)


def _text(text, x, y, height, layer, seed, mirror=False):
    effects = ["effects", ["font", ["size", "%g" % height, "%g" % height],
                           ["thickness", "%g" % B.SILK_W]]]
    if mirror:
        effects.append(["justify", "mirror"])
    return ["gr_text", '"%s"' % text,
            ["at", "%g" % kx(x), "%g" % ky(y), "0"],
            ["layer", '"%s"' % layer],
            ["uuid", '"%s"' % uid(seed)],
            effects]


def tracks_and_vias(nets):
    out = []
    for t, (net, points, width, layer) in enumerate(B.TRACKS):
        index, _label = nets[net]
        kicad_layer = "F.Cu" if layer == "F" else "B.Cu"
        for s, (a, b) in enumerate(zip(points, points[1:])):
            if a == b:
                continue
            out.append(["segment",
                        ["start", "%g" % kx(a[0]), "%g" % ky(a[1])],
                        ["end", "%g" % kx(b[0]), "%g" % ky(b[1])],
                        ["width", "%g" % width],
                        ["layer", '"%s"' % kicad_layer],
                        ["net", str(index)],
                        ["uuid", '"%s"' % uid("seg/%d/%d" % (t, s))]])
    for v, (net, pos) in enumerate(B.VIAS):
        index, _label = nets[net]
        out.append(["via",
                    ["at", "%g" % kx(pos[0]), "%g" % ky(pos[1])],
                    ["size", "%g" % B.VIA_DIA],
                    ["drill", "%g" % B.VIA_DRILL],
                    ["layers", '"F.Cu"', '"B.Cu"'],
                    ["net", str(index)],
                    ["uuid", '"%s"' % uid("via/%d" % v)]])
    return out


def zones(nets):
    out = []
    gnd, gnd_name = nets["GND"]
    corners = [(0.0, 0.0), (B.BOARD_W, 0.0), (B.BOARD_W, B.BOARD_H), (0.0, B.BOARD_H)]
    pts = ["pts"] + [["xy", "%g" % kx(x), "%g" % ky(y)] for x, y in corners]
    for layer in ("F.Cu", "B.Cu"):
        out.append(["zone",
                    ["net", str(gnd)],
                    ["net_name", '"%s"' % gnd_name],
                    ["layer", '"%s"' % layer],
                    ["uuid", '"%s"' % uid("zone/" + layer)],
                    ["name", '"GND pour %s"' % layer],
                    ["hatch", "edge", "0.5"],
                    ["connect_pads", ["clearance", "%g" % B.POUR_GAP]],
                    ["min_thickness", "%g" % 0.25],
                    ["filled_areas_thickness", "no"],
                    ["fill", "yes",
                     ["thermal_gap", "%g" % B.THERMAL_GAP],
                     ["thermal_bridge_width", "%g" % B.THERMAL_SPOKE],
                     ["island_removal_mode", "1"],
                     ["island_area_min", "1"]],
                    ["polygon", pts]])

    for i, (x0, y0, x1, y1) in enumerate(B.KEEPOUTS):
        box = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
        kpts = ["pts"] + [["xy", "%g" % kx(x), "%g" % ky(y)] for x, y in box]
        out.append(["zone",
                    ["net", "0"],
                    ["net_name", '""'],
                    ["layers", '"F.Cu"', '"B.Cu"'],
                    ["uuid", '"%s"' % uid("keepout/%d" % i)],
                    ["name", '"Hall sensor U%d: no pour in the magnetic path"' % (i + 1)],
                    ["hatch", "full", "0.5"],
                    ["keepout",
                     ["tracks", "allowed"],
                     ["vias", "allowed"],
                     ["pads", "allowed"],
                     ["copperpour", "not_allowed"],
                     ["footprints", "allowed"]],
                    ["placement", ["enabled", "no"], ["sheetname", '""']],
                    ["fill", ["thermal_gap", "0.5"], ["thermal_bridge_width", "0.5"]],
                    ["polygon", kpts]])
    return out


def write_board():
    nets = net_table()
    lines = ["(kicad_pcb",
             '\t(version 20240108)',
             '\t(generator "osupad")',
             '\t(generator_version "8.0")',
             "\t(general",
             "\t\t(thickness %g)" % B.THICKNESS,
             "\t\t(legacy_teardrops no)",
             "\t)",
             '\t(paper "A4")',
             "\t(title_block",
             '\t\t(title "%s")' % B.TITLE,
             '\t\t(date "%s")' % B.DATE,
             '\t\t(rev "%s")' % B.REV,
             '\t\t(company "%s")' % B.COMPANY,
             "\t)",
             LAYERS,
             "\t(setup",
             "\t\t(pad_to_mask_clearance %g)" % B.MASK_EXPAND,
             "\t\t(allow_soldermask_bridges_in_footprints no)",
             "\t\t(aux_axis_origin %g %g)" % (OX, ky(0)),
             "\t\t(grid_origin %g %g)" % (OX, ky(0)),
             "\t)",
             '\t(net 0 "")']
    for name, (index, label) in sorted(nets.items(), key=lambda kv: kv[1][0]):
        lines.append('\t(net %d "%s")' % (index, label))

    for part in B.PARTS:
        lines.append(dump(place_footprint(part, nets), 1))
    for hole, (x, y) in enumerate(B.MOUNT_HOLES):
        lines.append(dump(mounting_hole("H%d" % (hole + 1), x, y), 1))
    for key, (x, y) in enumerate((B.KEY1, B.KEY2)):
        lines.append(dump(switch_position("SW%d" % (key + 1), x, y), 1))
    for node in edge_cuts() + silk_texts() + tracks_and_vias(nets) + zones(nets):
        lines.append(dump(node, 1))
    lines.append(")")

    path = os.path.join(HE_DIR, B.PROJECT + ".kicad_pcb")
    with open(path, "w", newline="\n", encoding="utf8") as f:
        f.write("\n".join(lines) + "\n")
    return path


def mounting_hole(ref, x, y):
    return ["footprint", '"osupad:MountingHole_2.2mm_M2"',
            ["layer", '"F.Cu"'],
            ["uuid", '"%s"' % uid("hole/" + ref)],
            ["at", "%g" % kx(x), "%g" % ky(y)],
            ["descr", '"Mounting hole 2.2 mm, M2, no annular ring"'],
            ["property", '"Reference"', '"%s"' % ref,
             ["at", "0", "-3.15", "0"], ["layer", '"F.SilkS"'], ["hide", "yes"],
             ["uuid", '"%s"' % uid("hole/ref/" + ref)],
             ["effects", ["font", ["size", "1", "1"], ["thickness", "0.15"]]]],
            ["property", '"Value"', '"MountingHole_2.2mm_M2"',
             ["at", "0", "3.15", "0"], ["layer", '"F.Fab"'],
             ["uuid", '"%s"' % uid("hole/val/" + ref)],
             ["effects", ["font", ["size", "1", "1"], ["thickness", "0.15"]]]],
            ["attr", "board_only", "exclude_from_pos_files", "exclude_from_bom"],
            ["pad", '""', "np_thru_hole", "circle",
             ["at", "0", "0"],
             ["size", "%g" % B.MOUNT_DRILL, "%g" % B.MOUNT_DRILL],
             ["drill", "%g" % B.MOUNT_DRILL],
             ["layers", '"F&B.Cu"', '"*.Mask"'],
             ["uuid", '"%s"' % uid("hole/pad/" + ref)]]]


def switch_position(ref, x, y):
    """Plate-mount peg holes only: the key centre stays solid FR-4."""
    part = B.Part(ref, "KEY" + ref[-1], "SW_MX_Hall_PlateMount_SolidCentre",
                  (x, y), 0, {}, "", "", "", "MX magnetic switch position", (0, 0),
                  assembled=False)
    node = place_footprint(part, {})
    node = drop(node, {"attr"})
    node.append(["attr", "board_only", "exclude_from_pos_files", "exclude_from_bom",
                 "allow_missing_courtyard"])
    return node


# ---------------------------------------------------------------------------
# Schematic
# ---------------------------------------------------------------------------

def block(text, start):
    depth = 0
    for i in range(start, len(text)):
        if text[i] == "(":
            depth += 1
        elif text[i] == ")":
            depth -= 1
            if depth == 0:
                return text[start:i + 1]
    raise ValueError("unbalanced s-expression")


def load_symbols(names):
    import re
    text = open(SYM_LIB, encoding="utf8").read()
    symbols = {}
    for name in names:
        raw = block(text, text.index('(symbol "%s"' % name))
        pins = {}
        for m in re.finditer(r'\(pin\s+\w+\s+\w+\s*\(at ([-\d.]+) ([-\d.]+) ([-\d.]+)\)'
                             r'.*?\(number "([^"]+)"', raw, re.S):
            pins[m.group(4)] = (float(m.group(1)), float(m.group(2)), int(float(m.group(3))))
        embedded = raw.replace('(symbol "%s"' % name, '(symbol "osupad:%s"' % name, 1)
        symbols[name] = (embedded, pins)
    return symbols


SYMBOL_OF = {B.FP_C0603: "C", B.FP_R0603: "R", B.FP_SOT23: "DRV5055",
             B.FP_JST8: "Conn_01x08", B.FP_TESTPOINT: "TestPoint"}

NOTES = [
    "osuPad Hall Effect (Rapid Trigger) input module V1 - two DRV5055 linear Hall sensors",
    "Module connector pinout, identical on every osuPad input module:",
    "1 3V3 | 2 GND | 3 IN1 = GPIO10 (ADC1_CH9) | 4 IN2 = GPIO7 (ADC1_CH6) | "
    "5 ID = GPIO8 (ADC1_CH7) | 6 GPIO6 | 7 GPIO4 | 8 GPIO2",
    "U1/U2 output a ratiometric analog voltage centred on VCC/2; the firmware "
    "tracks the peak and applies the rapid-trigger threshold.",
    "Module ID: 3V3 x 47k / (100k + 47k) = 1.06 V, read on GPIO8 and decoded as "
    "\"Hall Effect module V1\".",
    "Everything is on the bottom side. U1 and U2 sit on the key centres over solid "
    "FR-4, with no copper pour in the magnetic path.",
    "J1-6/7/8 are reserved for an SPI variant and are brought out to probe pads "
    "TP1..TP3 instead of being left floating.",
]


def prop(name, value, x, y, hide=False, angle=0):
    return ('\t\t(property "%s" "%s" (at %.2f %.2f %d)%s '
            '(effects (font (size 1.27 1.27))))\n'
            % (name, value, x, y, angle, " (hide yes)" if hide else ""))


def write_schematic():
    symbols = load_symbols(sorted({SYMBOL_OF[p.footprint] for p in B.PARTS}))
    root = uid("sheet/root")
    out = ['(kicad_sch\n\t(version 20250114)\n\t(generator "osupad")\n'
           '\t(generator_version "9.0")\n',
           '\t(uuid "%s")\n\t(paper "A4")\n' % root,
           '\t(title_block (title "%s") (date "%s") (rev "%s") (company "%s"))\n'
           % (B.TITLE, B.DATE, B.REV, B.COMPANY),
           "\t(lib_symbols\n"]
    for name in sorted(symbols):
        out.append("\t\t" + symbols[name][0] + "\n")
    out.append("\t)\n")

    for i, note in enumerate(NOTES):
        out.append('\t(text "%s" (exclude_from_sim no) (at 20.32 %.2f 0) '
                   '(effects (font (size 1.27 1.27)) (justify left bottom)) '
                   '(uuid "%s"))\n' % (note, 160.02 + i * 4.06, uid("note/%d" % i)))

    for part in B.PARTS:
        name = SYMBOL_OF[part.footprint]
        sx, sy = part.sch
        pins = symbols[name][1]
        out.append('\t(symbol (lib_id "osupad:%s") (at %.2f %.2f 0) (unit 1) '
                   '(exclude_from_sim no) (in_bom %s) (on_board yes) (dnp no) '
                   '(uuid "%s")\n'
                   % (name, sx, sy, "yes" if part.assembled else "no",
                      uid("sym/" + part.ref)))
        if name in ("R", "C"):
            out.append(prop("Reference", part.ref, sx + 2.54, sy - 1.27))
            out.append(prop("Value", part.value, sx + 2.54, sy + 1.27))
        else:
            top = max(y for _x, y, _a in pins.values())
            bottom = min(y for _x, y, _a in pins.values())
            out.append(prop("Reference", part.ref, sx, sy - top - 3.81))
            out.append(prop("Value", part.value, sx, sy - bottom + 3.81))
        out.append(prop("Footprint", "osupad:" + part.footprint, sx, sy, hide=True))
        out.append(prop("Datasheet", "", sx, sy, hide=True))
        out.append(prop("Description", part.description, sx, sy, hide=True))
        out.append(prop("LCSC", part.lcsc, sx, sy, hide=True))
        out.append(prop("MPN", ("%s %s" % (part.manufacturer, part.mpn)).strip(),
                        sx, sy, hide=True))
        for number in pins:
            out.append('\t\t(pin "%s" (uuid "%s"))\n'
                       % (number, uid("pin/%s/%s" % (part.ref, number))))
        out.append('\t\t(instances (project "%s" (path "/%s" (reference "%s") '
                   '(unit 1))))\n\t)\n' % (B.PROJECT, root, part.ref))

        for number, (px, py, angle) in pins.items():
            x, y = sx + px, sy - py
            key = "MP" if number.startswith("MP") else number
            net = part.pins.get(key)
            if net is None:
                out.append('\t(no_connect (at %.2f %.2f) (uuid "%s"))\n'
                           % (x, y, uid("nc/%s/%s" % (part.ref, number))))
                continue
            label_angle = (angle + 180) % 360
            justify = {0: "left bottom", 90: "left bottom",
                       180: "right bottom", 270: "right bottom"}[label_angle]
            out.append('\t(label "%s" (at %.2f %.2f %d) (fields_autoplaced yes) '
                       '(effects (font (size 1.27 1.27)) (justify %s)) (uuid "%s"))\n'
                       % (net, x, y, label_angle, justify,
                          uid("label/%s/%s" % (part.ref, number))))

    out.append('\t(sheet_instances (path "/" (page "1")))\n)\n')
    path = os.path.join(HE_DIR, B.PROJECT + ".kicad_sch")
    with open(path, "w", newline="\n", encoding="utf8") as f:
        f.write("".join(out))
    return path


# ---------------------------------------------------------------------------
# Project files
# ---------------------------------------------------------------------------

def write_project():
    """.kicad_pro plus the two library tables, matching the sibling boards."""
    pro = {
        "board": {
            "design_settings": {
                "defaults": {
                    "board_outline_line_width": B.EDGE_W,
                    "copper_line_width": 0.2,
                    "silk_line_width": 0.12,
                    "silk_text_size_h": B.SILK_H,
                    "silk_text_size_v": B.SILK_H,
                    "silk_text_thickness": B.SILK_W,
                },
                "rules": {
                    "min_clearance": 0.15,
                    "min_connection": 0.0,
                    "min_copper_edge_clearance": B.EDGE_CLEARANCE,
                    "min_hole_clearance": B.HOLE_CLEARANCE,
                    "min_hole_to_hole": B.HOLE_TO_HOLE,
                    "min_microvia_diameter": 0.2,
                    "min_microvia_drill": 0.1,
                    "min_resolved_spokes": 1,
                    "min_silk_clearance": 0.0,
                    "min_text_height": B.SILK_H,
                    "min_text_thickness": B.SILK_W,
                    "min_through_hole_diameter": 0.3,
                    "min_track_width": 0.15,
                    "min_via_annular_width": 0.13,
                    "min_via_diameter": 0.5,
                    "solder_mask_to_copper_clearance": 0.0,
                    "use_height_for_length_calcs": True,
                },
                "track_widths": [0.0, B.TRACK_W, 0.4],
                "via_dimensions": [{"diameter": 0.0, "drill": 0.0},
                                   {"diameter": B.VIA_DIA, "drill": B.VIA_DRILL}],
            },
        },
        "boards": [],
        "libraries": {"pinned_footprint_libs": [], "pinned_symbol_libs": []},
        "meta": {"filename": B.PROJECT + ".kicad_pro", "version": 3},
        "net_settings": {
            "classes": [{
                "name": "Default",
                "clearance": B.CLEARANCE,
                "track_width": B.TRACK_W,
                "via_diameter": B.VIA_DIA,
                "via_drill": B.VIA_DRILL,
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
    path = os.path.join(HE_DIR, B.PROJECT + ".kicad_pro")
    with open(path, "w", newline="\n", encoding="utf8") as f:
        json.dump(pro, f, indent=2)
        f.write("\n")

    tables = [("fp-lib-table", "fp_lib_table", "osupad.pretty",
               "osuPad project footprints"),
              ("sym-lib-table", "sym_lib_table", "osupad.kicad_sym",
               "osuPad project symbols")]
    for filename, root, target, descr in tables:
        body = ['(%s' % root,
                '\t(version 7)',
                '\t(lib (name "osupad") (type "KiCad") '
                '(uri "${KIPRJMOD}/../lib/%s") (options "") (descr "%s"))' % (target, descr),
                ')']
        with open(os.path.join(HE_DIR, filename), "w", newline="\n") as f:
            f.write("\n".join(body) + "\n")
    return path


# ---------------------------------------------------------------------------

def main():
    problems = B.selftest()
    errors = B.check()
    for line in problems + errors:
        print("BLOCKED:", line)
    if problems or errors:
        raise SystemExit("refusing to write KiCad files")

    board = write_board()
    sch = write_schematic()
    pro = write_project()
    for path in (sch, board, pro):
        print("wrote %s (%d bytes)" % (os.path.relpath(path, V1),
                                       os.path.getsize(path)))


if __name__ == "__main__":
    main()
