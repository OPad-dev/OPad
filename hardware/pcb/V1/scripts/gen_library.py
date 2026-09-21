#!/usr/bin/env python3
"""Generate OPad's standard parts in lib/: six footprints and the symbol library.

Writes, from the tables below:

- lib/osupad.pretty/{C_0603_1608Metric, R_0603_1608Metric, SOT-23,
  JST_SH_SM08B-SRSS-TB_1x08-1MP_P1.00mm_Horizontal,
  PinSocket_1x14_P2.54mm_Vertical, MountingHole_2.2mm_M2}.kicad_mod
- lib/osupad.kicad_sym (every symbol: R, C, SW_Push, Conn_01x08, Conn_01x14,
  DRV5055, TestPoint)

The other footprints in lib/osupad.pretty (Kailh hot-swap, the Hall switch
positions, the test pad) are drawn by hand and are not touched.

Pad and pin tables are the parts' land patterns and connection points, taken
from the component datasheets (and IPC-7351 nominal for the 0603 chips); they
are kept identical to what the V1 boards were designed with, so the boards stay
valid. Every drawing (fab body, silkscreen, pin-1 markers, courtyards, symbol
graphics) is computed here, so the whole library is original OPad work under
the repository's MIT license.

No KiCad needed. Run from anywhere:

    python3 hardware/pcb/V1/scripts/gen_library.py
"""

import math
import os
import uuid

V1 = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FP_DIR = os.path.join(V1, "lib", "osupad.pretty")
SYM_FILE = os.path.join(V1, "lib", "osupad.kicad_sym")

NAMESPACE = uuid.UUID("5b1d0c3e-6f0a-4a57-9b43-0bad0c0ffee0")

SILK_W = 0.12
SILK_CLEAR = 0.2        # silkscreen to copper
FAB_W = 0.1
CRTYD_W = 0.05
CRTYD_MARGIN = 0.25
MODEL_DIR = "${KICAD10_3DMODEL_DIR}"


# ---------------------------------------------------------------------------
# S-expression output
# ---------------------------------------------------------------------------

class Sym(str):
    """A bare (unquoted) token."""


def q(text):
    return '"%s"' % text.replace("\\", "\\\\").replace('"', '\\"')


def num(x):
    x = round(x, 6)
    if x == 0:
        x = 0.0
    return Sym(("%f" % x).rstrip("0").rstrip("."))


def dump(node, depth=0):
    """KiCad-style layout: leaf lists inline, nested lists one child per line."""
    tabs = "\t" * depth
    head = [a for a in node if not isinstance(a, list)]
    kids = [a for a in node if isinstance(a, list)]
    text = tabs + "(" + " ".join(str(a) for a in head)
    if not kids:
        return text + ")"
    return text + "\n" + "\n".join(dump(k, depth + 1) for k in kids) + "\n" + tabs + ")"


def uid(*parts):
    return [Sym("uuid"), q(str(uuid.uuid5(NAMESPACE, "/".join(parts))))]


def xy(tag, x, y):
    return [Sym(tag), num(x), num(y)]


def stroke(width, kind="solid"):
    return [Sym("stroke"), [Sym("width"), num(width)], [Sym("type"), Sym(kind)]]


def font(size, thickness=None):
    f = [Sym("font"), [Sym("size"), num(size), num(size)]]
    if thickness:
        f.append([Sym("thickness"), num(thickness)])
    return [Sym("effects"), f]


# ---------------------------------------------------------------------------
# Footprints
# ---------------------------------------------------------------------------

def smd(number, x, y, w, h, rratio=0.25):
    return dict(number=number, kind="smd", shape="roundrect", x=x, y=y, w=w, h=h,
                layers=["F.Cu", "F.Mask", "F.Paste"], rratio=rratio)


def tht(number, x, y, size, drill, shape="circle"):
    return dict(number=number, kind="thru_hole", shape=shape, x=x, y=y, w=size, h=size,
                drill=drill, layers=["*.Cu", "*.Mask"])


FOOTPRINTS = [
    dict(
        name="C_0603_1608Metric",
        descr="Capacitor, 0603 (1608 metric) SMD chip. Pads: IPC-7351 nominal land pattern.",
        tags="capacitor 0603 1608",
        attr=["smd"],
        body=(-0.8, -0.4, 0.8, 0.4),
        pin1=False,
        model="Capacitor_SMD.3dshapes/C_0603_1608Metric.step",
        pads=[smd("1", -0.775, 0, 0.9, 0.95), smd("2", 0.775, 0, 0.9, 0.95)],
    ),
    dict(
        name="R_0603_1608Metric",
        descr="Resistor, 0603 (1608 metric) SMD chip. Pads: IPC-7351 nominal land pattern.",
        tags="resistor 0603 1608",
        attr=["smd"],
        body=(-0.8, -0.4125, 0.8, 0.4125),
        pin1=False,
        model="Resistor_SMD.3dshapes/R_0603_1608Metric.step",
        pads=[smd("1", -0.825, 0, 0.8, 0.95), smd("2", 0.825, 0, 0.8, 0.95)],
    ),
    dict(
        name="SOT-23",
        descr="SOT-23 (JEDEC TO-236AB) 3-pin SMD package, e.g. the TI DRV5055 Hall sensor. "
              "Pads sized for reflow and hand soldering.",
        tags="SOT-23 TO-236AB 3-pin",
        attr=["smd"],
        body=(-1.45, -0.65, 1.45, 0.65),
        pin1=True,
        model=None,
        pads=[smd("1", -0.95, -1, 0.7, 1), smd("2", 0.95, -1, 0.7, 1), smd("3", 0, 1, 0.7, 1)],
    ),
    dict(
        name="JST_SH_SM08B-SRSS-TB_1x08-1MP_P1.00mm_Horizontal",
        descr="JST SH series 8-pin 1.00 mm pitch connector, SM08B-SRSS-TB, horizontal SMD, "
              "with two mechanical pads. Pads per the JST SH datasheet "
              "(https://www.jst-mfg.com/product/pdf/eng/eSH.pdf).",
        tags="connector JST SH 1.00mm 8-pin horizontal",
        attr=["smd"],
        body=(-5, -1.675, 5, 2.575),
        pin1=True,
        model="Connector_JST.3dshapes/JST_SH_SM08B-SRSS-TB_1x08-1MP_P1.00mm_Horizontal.step",
        pads=[smd(str(i + 1), -3.5 + i, -2, 0.6, 1.55) for i in range(8)]
        + [smd("MP", -4.8, 1.875, 1.2, 1.8, 0.208333), smd("MP", 4.8, 1.875, 1.2, 1.8, 0.208333)],
    ),
    dict(
        name="PinSocket_1x14_P2.54mm_Vertical",
        descr="Female pin header, 1 row x 14 pins, 2.54 mm pitch, vertical, through hole.",
        tags="pin socket header female 1x14 2.54mm vertical THT",
        attr=["through_hole"],
        body=(-1.27, -1.27, 1.27, 34.29),
        pin1=True,
        model="Connector_PinSocket_2.54mm.3dshapes/PinSocket_1x14_P2.54mm_Vertical.step",
        pads=[tht(str(i + 1), 0, 2.54 * i, 1.7, 1, "rect" if i == 0 else "circle")
              for i in range(14)],
    ),
    dict(
        name="MountingHole_2.2mm_M2",
        descr="Mounting hole for an M2 screw: 2.2 mm non-plated hole, no copper. "
              "The outer circle is the M2 screw head (3.8 mm).",
        tags="mounting hole M2",
        attr=["exclude_from_pos_files", "exclude_from_bom"],
        hole=dict(drill=2.2, head=3.8),
        pads=[],
    ),
]


def pad_box(p, grow=0.0):
    return (p["x"] - p["w"] / 2 - grow, p["y"] - p["h"] / 2 - grow,
            p["x"] + p["w"] / 2 + grow, p["y"] + p["h"] / 2 + grow)


def clip_segment(a, b, boxes):
    """Parts of the axis-aligned segment a-b that lie outside every box."""
    horizontal = a[1] == b[1]
    lo, hi = sorted((a[0], b[0]) if horizontal else (a[1], b[1]))
    fixed = a[1] if horizontal else a[0]
    cuts = []
    for x0, y0, x1, y1 in boxes:
        if horizontal and y0 <= fixed <= y1:
            cuts.append((x0, x1))
        elif not horizontal and x0 <= fixed <= x1:
            cuts.append((y0, y1))
    pieces, start = [], lo
    for c0, c1 in sorted(cuts):
        if c1 <= start or c0 >= hi:
            continue
        if c0 > start:
            pieces.append((start, c0))
        start = max(start, c1)
    if start < hi:
        pieces.append((start, hi))
    out = []
    for s, e in pieces:
        if e - s < 0.2:
            continue
        out.append(((s, fixed), (e, fixed)) if horizontal else ((fixed, s), (fixed, e)))
    return out


def round_out(v, step=0.05):
    return math.copysign(math.ceil(abs(v) / step - 1e-9) * step, v)


def fp_line(name, key, a, b, layer, width):
    return [Sym("fp_line"), xy("start", *a), xy("end", *b), stroke(width),
            [Sym("layer"), q(layer)], uid(name, key)]


def footprint(fp):
    name = fp["name"]
    pads = fp["pads"]
    items = []
    bbox = []

    if "hole" in fp:
        r_hole, r_head = fp["hole"]["drill"] / 2, fp["hole"]["head"] / 2
        r_crtyd = round_out(r_head + CRTYD_MARGIN)
        for key, r, layer, width in (("head", r_head, "Cmts.User", 0.15),
                                     ("crtyd", r_crtyd, "F.CrtYd", CRTYD_W)):
            items.append([Sym("fp_circle"), xy("center", 0, 0), xy("end", r, 0), stroke(width),
                          [Sym("fill"), Sym("no")], [Sym("layer"), q(layer)], uid(name, key)])
        top, bottom = -r_crtyd, r_crtyd
        pad_nodes = [[Sym("pad"), q(""), Sym("np_thru_hole"), Sym("circle"), xy("at", 0, 0),
                      [Sym("size"), num(2 * r_hole), num(2 * r_hole)],
                      [Sym("drill"), num(2 * r_hole)],
                      [Sym("layers"), q("*.Cu"), q("*.Mask")], uid(name, "pad")]]
    else:
        bx0, by0, bx1, by1 = fp["body"]
        pad1 = pads[0]
        # Fab: body outline, pin-1 corner chamfered
        corners = [(bx0, by0), (bx1, by0), (bx1, by1), (bx0, by1)]
        if fp["pin1"]:
            i = min(range(4), key=lambda k: math.dist(corners[k], (pad1["x"], pad1["y"])))
            c = min(0.5, 0.25 * min(bx1 - bx0, by1 - by0))
            cx, cy = corners[i]
            prev, nxt = corners[i - 1], corners[(i + 1) % 4]
            step = lambda p: (cx + math.copysign(c, p[0] - cx) if p[0] != cx else cx,
                              cy + math.copysign(c, p[1] - cy) if p[1] != cy else cy)
            corners[i:i + 1] = [step(prev), step(nxt)]
        items.append([Sym("fp_poly"), [Sym("pts")] + [xy("xy", *p) for p in corners],
                      stroke(FAB_W), [Sym("fill"), Sym("no")], [Sym("layer"), q("F.Fab")],
                      uid(name, "fab")])

        # Silkscreen: body outline pushed out by half a line, kept clear of copper
        o = SILK_W / 2 + 0.05
        sx0, sy0, sx1, sy1 = bx0 - o, by0 - o, bx1 + o, by1 + o
        keepout = [pad_box(p, SILK_CLEAR + SILK_W / 2) for p in pads]
        edges = [((sx0, sy0), (sx1, sy0)), ((sx1, sy0), (sx1, sy1)),
                 ((sx0, sy1), (sx1, sy1)), ((sx0, sy0), (sx0, sy1))]
        n = 0
        for a, b in edges:
            for s, e in clip_segment(a, b, keepout):
                items.append(fp_line(name, "silk%d" % n, s, e, "F.SilkS", SILK_W))
                bbox += [s, e]
                n += 1

        # Pin-1 dot on the silkscreen, left of pad 1 (outside the outline when
        # pad 1 sits inside the body, as on the pin socket)
        if fp["pin1"]:
            r = 0.15
            if by0 <= pad1["y"] <= by1:
                dot = (sx0 - SILK_W / 2 - 0.15 - r, pad1["y"])
            else:
                dot = (pad1["x"] - pad1["w"] / 2 - SILK_CLEAR - r - 0.05, pad1["y"])
            items.append([Sym("fp_circle"), xy("center", *dot), xy("end", dot[0] + r, dot[1]),
                          stroke(0.1), [Sym("fill"), Sym("yes")], [Sym("layer"), q("F.SilkS")],
                          uid(name, "pin1")])
            bbox += [(dot[0] - r, dot[1] - r), (dot[0] + r, dot[1] + r)]

        # Courtyard: everything above plus a margin, on a 0.05 mm grid
        for p in pads:
            x0, y0, x1, y1 = pad_box(p)
            bbox += [(x0, y0), (x1, y1)]
        bbox += [(bx0, by0), (bx1, by1)]
        cx0 = round_out(min(p[0] for p in bbox) - CRTYD_MARGIN)
        cy0 = round_out(min(p[1] for p in bbox) - CRTYD_MARGIN)
        cx1 = round_out(max(p[0] for p in bbox) + CRTYD_MARGIN)
        cy1 = round_out(max(p[1] for p in bbox) + CRTYD_MARGIN)
        items.append([Sym("fp_rect"), xy("start", cx0, cy0), xy("end", cx1, cy1),
                      stroke(CRTYD_W), [Sym("fill"), Sym("no")], [Sym("layer"), q("F.CrtYd")],
                      uid(name, "crtyd")])
        top, bottom = cy0, cy1

        pad_nodes = []
        for i, p in enumerate(pads):
            node = [Sym("pad"), q(p["number"]), Sym(p["kind"]), Sym(p["shape"]),
                    xy("at", p["x"], p["y"]), [Sym("size"), num(p["w"]), num(p["h"])]]
            if "drill" in p:
                node.append([Sym("drill"), num(p["drill"])])
            node.append([Sym("layers")] + [q(layer) for layer in p["layers"]])
            if p["kind"] == "thru_hole":
                node.append([Sym("remove_unused_layers"), Sym("no")])
            if p["shape"] == "roundrect":
                node.append([Sym("roundrect_rratio"), num(p["rratio"])])
            node.append(uid(name, "pad%d" % i))
            pad_nodes.append(node)

    ref_size = 1.0 if bottom - top > 3 else 0.8
    fab_size = max(0.4, min(1.0, 0.4 * (bottom - top)))
    props = [
        ("Reference", "REF**", top - 0.3 - ref_size / 2, "F.SilkS", ref_size, False),
        ("Value", name, bottom + 0.3 + ref_size / 2, "F.Fab", ref_size, False),
        ("Datasheet", "", 0, "F.Fab", 1.27, True),
        ("Description", "", 0, "F.Fab", 1.27, True),
    ]
    node = [Sym("footprint"), q(name),
            [Sym("version"), Sym("20240108")],
            [Sym("generator"), q("osupad")],
            [Sym("generator_version"), q("8.0")],
            [Sym("layer"), q("F.Cu")],
            [Sym("descr"), q(fp["descr"])],
            [Sym("tags"), q(fp["tags"])]]
    for pname, value, y, layer, size, hidden in props:
        prop = [Sym("property"), q(pname), q(value), xy("at", 0, y) + [num(0)],
                [Sym("layer"), q(layer)]]
        if hidden:
            prop.append([Sym("hide"), Sym("yes")])
        prop.append(uid(name, "prop", pname))
        prop.append(font(size, None if hidden else round(size * 0.15, 3)))
        node.append(prop)
    node.append([Sym("attr")] + [Sym(a) for a in fp["attr"]])
    node += items
    node.append([Sym("fp_text"), Sym("user"), q("${REFERENCE}"), xy("at", 0, (top + bottom) / 2 if "hole" not in fp else 0) + [num(0)],
                 [Sym("layer"), q("F.Fab")], uid(name, "fabref"),
                 font(fab_size, round(fab_size * 0.15, 3))])
    node += pad_nodes
    if fp.get("model"):
        node.append([Sym("model"), q("%s/%s" % (MODEL_DIR, fp["model"])),
                     [Sym("offset"), [Sym("xyz"), num(0), num(0), num(0)]],
                     [Sym("scale"), [Sym("xyz"), num(1), num(1), num(1)]],
                     [Sym("rotate"), [Sym("xyz"), num(0), num(0), num(0)]]])
    return dump(node) + "\n"


# ---------------------------------------------------------------------------
# Symbols
# ---------------------------------------------------------------------------

def pin(kind, x, y, angle, length, name="", number="1"):
    return [Sym("pin"), Sym(kind), Sym("line"), xy("at", x, y) + [num(angle)],
            [Sym("length"), num(length)],
            [Sym("name"), q(name), font(1.27)],
            [Sym("number"), q(number), font(1.27)]]


def rect(x0, y0, x1, y1, width=0.254, fill="none"):
    return [Sym("rectangle"), xy("start", x0, y0), xy("end", x1, y1),
            [Sym("stroke"), [Sym("width"), num(width)], [Sym("type"), Sym("default")]],
            [Sym("fill"), [Sym("type"), Sym(fill)]]]


def polyline(points, width=0.254):
    return [Sym("polyline"), [Sym("pts")] + [xy("xy", *p) for p in points],
            [Sym("stroke"), [Sym("width"), num(width)], [Sym("type"), Sym("default")]],
            [Sym("fill"), [Sym("type"), Sym("none")]]]


def circle(x, y, r, width=0.254, fill="none"):
    return [Sym("circle"), xy("center", x, y), [Sym("radius"), num(r)],
            [Sym("stroke"), [Sym("width"), num(width)], [Sym("type"), Sym("default")]],
            [Sym("fill"), [Sym("type"), Sym(fill)]]]


def connector(n):
    top = 2.54 * ((n - 1) // 2)
    return dict(
        name="Conn_01x%02d" % n, ref="J", value="Conn_01x%02d" % n,
        ref_at=(0, top + 2.54), value_at=(0, top - 2.54 * n),
        descr="Single-row connector, %d pins" % n, keywords="connector header socket",
        fp_filters="*_1x%02d_*" % n,
        pin_numbers_hidden=False, pin_names=(1.016, True), in_pos_files=True, in_bom=True,
        graphics=[rect(-1.27, top + 1.27, 1.27, top - 2.54 * n + 1.27, fill="background")]
        + [polyline([(-1.27, top - 2.54 * i), (-0.508, top - 2.54 * i)], 0.1524) for i in range(n)],
        pins=[pin("passive", -5.08, top - 2.54 * i, 0, 3.81, "Pin_%d" % (i + 1), str(i + 1))
              for i in range(n)],
    )


SYMBOLS = [
    dict(
        name="R", ref="R", value="R", ref_at=(2.032, 0, 90), value_at=(0, 0, 90),
        fp_at=(-1.778, 0, 90),
        descr="Resistor", keywords="R res resistor", fp_filters="R_*",
        pin_numbers_hidden=True, pin_names=(0, False), in_pos_files=True, in_bom=True,
        graphics=[rect(-0.889, -2.54, 0.889, 2.54)],
        pins=[pin("passive", 0, 3.81, 270, 1.27, "", "1"),
              pin("passive", 0, -3.81, 90, 1.27, "", "2")],
    ),
    dict(
        name="C", ref="C", value="C", ref_at=(0.635, 2.54), value_at=(0.635, -2.54),
        fp_at=(0.9652, -3.81),
        descr="Capacitor, non-polarised", keywords="cap capacitor", fp_filters="C_*",
        pin_numbers_hidden=True, pin_names=(0.254, False), in_pos_files=True, in_bom=True,
        graphics=[polyline([(-1.905, 1.016), (1.905, 1.016)], 0.381),
                  polyline([(-1.905, -1.016), (1.905, -1.016)], 0.381)],
        pins=[pin("passive", 0, 3.81, 270, 2.794, "", "1"),
              pin("passive", 0, -3.81, 90, 2.794, "", "2")],
    ),
    dict(
        name="SW_Push", ref="SW", value="SW_Push", ref_at=(1.27, 2.54), value_at=(0, -1.524),
        fp_at=(0, 5.08), ds_at=(0, 5.08),
        descr="Push button, normally open, two pins",
        keywords="switch push button momentary normally-open", fp_filters=None,
        pin_numbers_hidden=True, pin_names=(1.016, True), in_pos_files=True, in_bom=True,
        graphics=[circle(-2.032, 0, 0.508), circle(2.032, 0, 0.508),
                  polyline([(-2.794, 1.524), (2.794, 1.524)]),
                  polyline([(0, 1.524), (0, 3.302)])],
        pins=[pin("passive", -5.08, 0, 0, 2.54, "1", "1"),
              pin("passive", 5.08, 0, 180, 2.54, "2", "2")],
    ),
    connector(8),
    connector(14),
    dict(
        name="DRV5055", ref="U", value="DRV5055", ref_at=(0, 6.35), value_at=(0, -6.35),
        footprint="osupad:SOT-23",
        datasheet="https://www.ti.com/lit/ds/symlink/drv5055.pdf",
        descr="TI DRV5055 ratiometric linear Hall effect sensor, SOT-23",
        keywords="hall magnetic linear sensor", fp_filters=None,
        pin_numbers_hidden=False, pin_names=(1.016, False), in_pos_files=True, in_bom=True,
        graphics=[rect(-6.35, 3.81, 6.35, -3.81, fill="background")],
        pins=[pin("power_in", -8.89, 2.54, 0, 2.54, "VCC", "1"),
              pin("output", 8.89, 0, 180, 2.54, "OUT", "2"),
              pin("power_in", -8.89, -2.54, 0, 2.54, "GND", "3")],
    ),
    dict(
        name="TestPoint", ref="TP", value="TestPoint", ref_at=(0, 3.81), value_at=(0, -2.54),
        footprint="osupad:TestPoint_Pad_1.0x1.0mm",
        descr="SMD probe pad for a reserved connector pin", keywords="test point probe pad",
        fp_filters=None,
        pin_numbers_hidden=True, pin_names=(0.762, True), in_pos_files=None, in_bom=False,
        graphics=[circle(0, 2.032, 0.762)],
        pins=[pin("passive", 0, -2.54, 90, 3.81, "~", "1")],
    ),
]


def sym_prop(name, value, at, hidden):
    x, y = at[0], at[1]
    angle = at[2] if len(at) > 2 else 0
    node = [Sym("property"), q(name), q(value), xy("at", x, y) + [num(angle)],
            [Sym("show_name"), Sym("no")], [Sym("do_not_autoplace"), Sym("no")]]
    if hidden:
        node.append([Sym("hide"), Sym("yes")])
    node.append(font(1.27))
    return node


def symbol(s):
    name = s["name"]
    node = [Sym("symbol"), q(name)]
    if s["pin_numbers_hidden"]:
        node.append([Sym("pin_numbers"), [Sym("hide"), Sym("yes")]])
    else:
        node.append([Sym("pin_numbers"), [Sym("hide"), Sym("no")]])
    offset, names_hidden = s["pin_names"]
    pn = [Sym("pin_names"), [Sym("offset"), num(offset)]]
    if names_hidden:
        pn.append([Sym("hide"), Sym("yes")])
    node.append(pn)
    node.append([Sym("exclude_from_sim"), Sym("no")])
    node.append([Sym("in_bom"), Sym("yes" if s["in_bom"] else "no")])
    node.append([Sym("on_board"), Sym("yes")])
    if s["in_pos_files"] is not None:
        node.append([Sym("in_pos_files"), Sym("yes" if s["in_pos_files"] else "no")])
    node.append([Sym("duplicate_pin_numbers_are_jumpers"), Sym("no")])
    node.append(sym_prop("Reference", s["ref"], s["ref_at"], False))
    node.append(sym_prop("Value", s["value"], s["value_at"], False))
    node.append(sym_prop("Footprint", s.get("footprint", ""), s.get("fp_at", (0, 0)), True))
    node.append(sym_prop("Datasheet", s.get("datasheet", ""), s.get("ds_at", (0, 0)), True))
    node.append(sym_prop("Description", s["descr"], (0, 0), True))
    node.append(sym_prop("ki_keywords", s["keywords"], (0, 0), True))
    if s["fp_filters"]:
        node.append(sym_prop("ki_fp_filters", s["fp_filters"], (0, 0), True))
    node.append([Sym("symbol"), q(name + "_0_1")] + s["graphics"])
    node.append([Sym("symbol"), q(name + "_1_1")] + s["pins"])
    node.append([Sym("embedded_fonts"), Sym("no")])
    return node


def main():
    for fp in FOOTPRINTS:
        path = os.path.join(FP_DIR, fp["name"] + ".kicad_mod")
        with open(path, "w", encoding="utf8") as f:
            f.write(footprint(fp))
        print("wrote", os.path.relpath(path, V1))
    lib = [Sym("kicad_symbol_lib"), [Sym("version"), Sym("20251024")],
           [Sym("generator"), q("osupad")], [Sym("generator_version"), q("10.0")]]
    lib += [symbol(s) for s in SYMBOLS]
    with open(SYM_FILE, "w", encoding="utf8") as f:
        f.write(dump(lib) + "\n")
    print("wrote", os.path.relpath(SYM_FILE, V1))


if __name__ == "__main__":
    main()
