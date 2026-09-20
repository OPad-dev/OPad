#!/usr/bin/env python3
"""Fabrication data for the osuPad Hall Effect input module (he_input_v1).

Writes RS-274X Gerbers, Excellon drills, a Gerber job file, and the JLCPCB and
PCBWay BOM / centroid files straight from ``he_board.py``.  No KiCad install is
required; ``he_board.check()`` runs first and the writer refuses to emit
anything if the board does not pass its own design rules.

    python3 hardware/pcb/V1/scripts/generate_he_gerbers.py

Ground pours
------------
The pour is not a solid rectangle.  It is emitted as a filled region in dark
polarity, then every foreign pad, track, via and non-plated hole is knocked out
of it with clear polarity (``%LPC*%``) using apertures grown by the pour gap,
and finally the real copper is flashed back on top in dark polarity.  Ground
pads are relieved the same way and reconnected with spokes.  The result carries
true isolation gaps that any fab viewer renders correctly, which a single G36
outline would not.
"""

import datetime
import json
import math
import os
import zipfile

import he_board as B
import strokefont

V1 = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(V1, "HE", "production")
PCBWAY_OUT = os.path.join(OUT, "pcbway")

SOFTWARE = "osuPad he_board.py"
CREATED = datetime.datetime.now().astimezone().replace(microsecond=0).isoformat()

ARC_STEPS = 12          # segments used to polygonise a 90 degree corner


# ---------------------------------------------------------------------------
# Gerber primitives
# ---------------------------------------------------------------------------

def u(v):
    """millimetres -> Gerber 4.6 integer units."""
    return int(round(v * 1e6))


def xy(p):
    return "X%dY%d" % (u(p[0]), u(p[1]))


class Gerber:
    """RS-274X writer with automatic aperture allocation."""

    def __init__(self, function, polarity="Positive"):
        self.function = function
        self.polarity = polarity
        self.apertures = []          # (dcode, definition, attribute)
        self.by_key = {}
        self.body = []
        self.current = None
        self.lp = "D"

    # -- apertures --------------------------------------------------------
    def _aperture(self, key, definition, attribute):
        if key in self.by_key:
            return self.by_key[key]
        dcode = 10 + len(self.apertures)
        self.apertures.append((dcode, definition, attribute))
        self.by_key[key] = dcode
        return dcode

    def circle(self, dia, attribute="Conductor"):
        return self._aperture(("C", round(dia, 6)), "C,%.6f" % dia, attribute)

    def rect(self, w, h, attribute="SMDPad,CuDef"):
        return self._aperture(("R", round(w, 6), round(h, 6)),
                              "R,%.6fX%.6f" % (w, h), attribute)

    # -- state ------------------------------------------------------------
    def select(self, dcode):
        if self.current != dcode:
            self.body.append("D%d*" % dcode)
            self.current = dcode

    def polarity_dark(self):
        if self.lp != "D":
            self.body.append("%LPD*%")
            self.lp = "D"

    def polarity_clear(self):
        if self.lp != "C":
            self.body.append("%LPC*%")
            self.lp = "C"

    def comment(self, text):
        self.body.append("G04 %s*" % text)

    # -- drawing ----------------------------------------------------------
    def flash(self, dcode, p):
        self.select(dcode)
        self.body.append("%sD03*" % xy(p))

    def stroke(self, dcode, points):
        if len(points) < 2:
            return
        self.select(dcode)
        self.body.append("%sD02*" % xy(points[0]))
        for p in points[1:]:
            self.body.append("%sD01*" % xy(p))

    def region(self, points):
        """Filled contour (G36/G37); ``points`` must be closed by the caller."""
        self.body.append("G36*")
        self.body.append("%sD02*" % xy(points[0]))
        for p in points[1:]:
            self.body.append("%sD01*" % xy(p))
        self.body.append("G37*")

    # -- output -----------------------------------------------------------
    def render(self, project):
        out = [
            "%%TF.GenerationSoftware,%s*%%" % SOFTWARE,
            "%%TF.CreationDate,%s*%%" % CREATED,
            "%%TF.ProjectId,%s,%s,%s*%%" % (project, _project_uuid(project), B.REV),
            "%TF.SameCoordinates,Original*%",
            "%%TF.FileFunction,%s*%%" % self.function,
            "%%TF.FilePolarity,%s*%%" % self.polarity,
            "%FSLAX46Y46*%",
            "G04 Gerber Fmt 4.6, Leading zero omitted, Abs format (unit mm)*",
            "%%G04 Created by %s on %s*%%" % (SOFTWARE, CREATED),
            "%MOMM*%",
            "%LPD*%",
            "G01*",
            "G04 APERTURE LIST*",
        ]
        for dcode, definition, attribute in self.apertures:
            if attribute:
                out.append("%%TA.AperFunction,%s*%%" % attribute)
            out.append("%%ADD%d%s*%%" % (dcode, definition))
            if attribute:
                out.append("%TD*%")
        out.append("G04 APERTURE END LIST*")
        out.extend(self.body)
        out.append("M02*")
        return "\n".join(out) + "\n"


def _project_uuid(project):
    """Stable pseudo-UUID so repeated runs produce identical files."""
    digest = 0
    for ch in project:
        digest = (digest * 131 + ord(ch)) & 0xFFFFFFFFFFFFFFFF
    h = "%016x" % digest
    return "%s-%s-4%s-8%s-%s" % (h[:8], h[8:12], h[1:4], h[4:7], (h + h)[:12])


# ---------------------------------------------------------------------------
# Shared geometry
# ---------------------------------------------------------------------------

def outline_points(inset=0.0):
    """Board outline, polygonised, counter-clockwise, closed."""
    r = B.CORNER_R - inset
    w, h = B.BOARD_W - inset, B.BOARD_H - inset
    lo = inset
    centres = [(B.CORNER_R, B.CORNER_R, 180, 270),
               (B.BOARD_W - B.CORNER_R, B.CORNER_R, 270, 360),
               (B.BOARD_W - B.CORNER_R, B.BOARD_H - B.CORNER_R, 0, 90),
               (B.CORNER_R, B.BOARD_H - B.CORNER_R, 90, 180)]
    pts = []
    for cx, cy, a0, a1 in centres:
        for k in range(ARC_STEPS + 1):
            a = math.radians(a0 + (a1 - a0) * k / ARC_STEPS)
            pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    # guard against a degenerate inset
    assert r > 0 and w > lo and h > lo
    pts.append(pts[0])
    return pts


def foreign_items(layer, net):
    return [it for it in B.copper_items() if it.layer == layer and it.net != net]


def thermal_spokes(ref, pad_num):
    """Spoke segments for a ground pad, dropping any that would bridge a gap."""
    part = B.PARTS_BY_REF[ref]
    pos = part.pad_pos(pad_num if not pad_num.startswith("MP") else pad_num)
    size = None
    for num, _lxly, sz in B.FOOTPRINTS[part.footprint]:
        if num == pad_num:
            size = sz
    if part.rot % 180 != 0:
        size = (size[1], size[0])
    hw, hh = size[0] / 2.0, size[1] / 2.0
    reach = B.THERMAL_GAP + 0.15
    candidates = [((pos[0], pos[1]), (pos[0] + hw + reach, pos[1])),
                  ((pos[0], pos[1]), (pos[0] - hw - reach, pos[1])),
                  ((pos[0], pos[1]), (pos[0], pos[1] + hh + reach)),
                  ((pos[0], pos[1]), (pos[0], pos[1] - hh - reach))]
    others = foreign_items("B", B.NET_GND)
    keep = []
    for a, b in candidates:
        probe = B.Item("cap", B.NET_GND, "B", "spoke", seg=(a, b), width=B.THERMAL_SPOKE)
        if all(probe.distance_to(o) >= B.CLEARANCE - 1e-6 for o in others):
            keep.append((a, b))
    return pos, size, keep


# ---------------------------------------------------------------------------
# Copper layers
# ---------------------------------------------------------------------------

def make_copper(layer):
    side = "Top" if layer == "F" else "Bot"
    index = 1 if layer == "F" else 2
    g = Gerber("Copper,L%d,%s" % (index, side))

    pads = [(ref, num, pos, size, net) for ref, num, pos, size, net, _tp in B.all_pads()]
    tracks = [(net, pts, w, ly) for net, pts, w, ly in B.TRACKS if ly == layer]

    # 1. the pour itself, inset from the outline by the edge clearance
    g.comment("ground pour, dark polarity")
    g.polarity_dark()
    g.region(outline_points(B.EDGE_CLEARANCE))

    # 2. knock out everything that is not ground
    g.comment("pour clearances, clear polarity")
    g.polarity_clear()

    for x0, y0, x1, y1 in B.KEEPOUTS:
        d = g.rect(x1 - x0, y1 - y0, "Other,Keepout")
        g.flash(d, ((x0 + x1) / 2.0, (y0 + y1) / 2.0))

    if layer == "B":
        for ref, num, pos, size, net in pads:
            if net == B.NET_GND:
                continue
            d = g.rect(size[0] + 2 * B.POUR_GAP, size[1] + 2 * B.POUR_GAP, "Other,Clearance")
            g.flash(d, pos)

    for net, pts, w, _ly in tracks:
        if net == B.NET_GND:
            continue
        d = g.circle(w + 2 * B.POUR_GAP, "Other,Clearance")
        g.stroke(d, pts)

    for net, pos in B.VIAS:
        if net == B.NET_GND:
            continue
        d = g.circle(B.VIA_DIA + 2 * B.POUR_GAP, "Other,Clearance")
        g.flash(d, pos)

    for hx, hy, dia in B.npth_holes():
        d = g.circle(dia + 2 * B.HOLE_CLEARANCE, "Other,Clearance")
        g.flash(d, (hx, hy))

    # thermal relief rings around ground pads
    if layer == "B":
        for ref, num in B.THERMAL_PADS:
            pos, size, _spokes = thermal_spokes(ref, num)
            d = g.rect(size[0] + 2 * B.THERMAL_GAP, size[1] + 2 * B.THERMAL_GAP,
                       "Other,ThermalRelief")
            g.flash(d, pos)

    # 3. put the real copper back
    g.comment("conductors, dark polarity")
    g.polarity_dark()

    if layer == "B":
        for ref, num in B.THERMAL_PADS:
            _pos, _size, spokes = thermal_spokes(ref, num)
            d = g.circle(B.THERMAL_SPOKE, "Conductor")
            for a, b in spokes:
                g.stroke(d, [a, b])

    if layer == "B":
        for ref, num, pos, size, net in pads:
            d = g.rect(size[0], size[1], "SMDPad,CuDef")
            g.flash(d, pos)

    for net, pts, w, _ly in tracks:
        d = g.circle(w, "Conductor")
        g.stroke(d, pts)

    d_via = g.circle(B.VIA_DIA, "ViaPad")
    for _net, pos in B.VIAS:
        g.flash(d_via, pos)

    return g


# ---------------------------------------------------------------------------
# Mask, paste, silk, outline
# ---------------------------------------------------------------------------

def make_mask(layer):
    side = "Top" if layer == "F" else "Bot"
    g = Gerber("SolderMask,%s" % side, "Negative")
    if layer == "B":
        # Vias are tented (front and back), so only pads open the mask.
        for _ref, _num, pos, size, _net, _tp in B.all_pads():
            d = g.rect(size[0] + 2 * B.MASK_EXPAND, size[1] + 2 * B.MASK_EXPAND,
                       "SMDPad,CuDef")
            g.flash(d, pos)
    return g


def make_paste(layer):
    side = "Top" if layer == "F" else "Bot"
    g = Gerber("Paste,%s" % side)
    if layer == "B":
        # Component pads only: the probe pads take no solder.
        for ref, num, pos, size, net, is_tp in B.all_pads():
            if is_tp:
                continue
            d = g.rect(size[0], size[1], "SMDPad,CuDef")
            g.flash(d, pos)
    return g


def make_silk(layer, order_marker):
    side = "Top" if layer == "F" else "Bot"
    g = Gerber("Legend,%s" % side)
    d = g.circle(B.SILK_W, None)

    items = list(B.TOP_SILK) if layer == "F" else list(B.BOTTOM_SILK)
    polys = []
    for text, tx, ty, height, anchor in items:
        polys += strokefont.strokes(text, tx, ty, height, mirror=(layer == "B"),
                                    anchor=anchor)

    if layer == "B":
        marker, mx, my = order_marker
        polys += strokefont.strokes(marker, mx, my, B.SILK_H, mirror=True, anchor="left")
        # pin 1 of the connector, and the sensor outlines
        for cx, cy in (B.KEY1, B.KEY2):
            h = B.SENSOR_KEEPOUT / 2.0
            polys.append([(cx - h, cy - h), (cx + h, cy - h),
                          (cx + h, cy + h), (cx - h, cy + h), (cx - h, cy - h)])
    else:
        for cx, cy in (B.KEY1, B.KEY2):
            polys.append([(cx - 7.0, cy - 7.0), (cx + 7.0, cy - 7.0),
                          (cx + 7.0, cy + 7.0), (cx - 7.0, cy + 7.0), (cx - 7.0, cy - 7.0)])
            polys.append([(cx - 1.2, cy), (cx + 1.2, cy)])
            polys.append([(cx, cy - 1.2), (cx, cy + 1.2)])

    for poly in polys:
        g.stroke(d, poly)
    return g


def make_edge_cuts():
    g = Gerber("Profile,NP")
    d = g.circle(B.EDGE_W, "Profile")
    g.stroke(d, outline_points())
    return g


# ---------------------------------------------------------------------------
# Excellon
# ---------------------------------------------------------------------------

def _drill_header(function):
    return ["M48",
            "; DRILL file {%s} date %s" % (SOFTWARE, CREATED),
            "; FORMAT={-:-/ absolute / metric / decimal}",
            "; #@! TF.CreationDate,%s" % CREATED,
            "; #@! TF.GenerationSoftware,%s" % SOFTWARE,
            "; #@! TF.FileFunction,%s" % function,
            "FMAT,2",
            "METRIC"]


def make_drill(plated):
    if plated:
        holes = {B.VIA_DRILL: [pos for _net, pos in B.VIAS]}
        out = _drill_header("Plated,1,2,PTH")
        aper = "Plated,PTH,Via"
    else:
        holes = {}
        for x, y, dia in B.npth_holes():
            holes.setdefault(dia, []).append((x, y))
        out = _drill_header("NonPlated,1,2,NPTH")
        aper = "NonPlated,NPTH,ComponentDrill"

    tools = sorted(holes)
    for i, dia in enumerate(tools, start=1):
        out.append("; #@! TA.AperFunction,%s" % aper)
        out.append("T%dC%.3f" % (i, dia))
    out += ["%", "G90", "G05"]
    for i, dia in enumerate(tools, start=1):
        out.append("T%d" % i)
        for x, y in sorted(holes[dia]):
            out.append("X%.3fY%.3f" % (x, y))
    out += ["T0", "M30"]
    return "\n".join(out) + "\n"


# ---------------------------------------------------------------------------
# Job file
# ---------------------------------------------------------------------------

def make_job(files):
    job = {
        "Header": {
            "GenerationSoftware": {"Vendor": "osuPad", "Application": "he_board.py",
                                   "Version": B.REV},
            "CreationDate": CREATED,
        },
        "GeneralSpecs": {
            "ProjectId": {"Name": B.PROJECT, "GUID": _project_uuid(B.PROJECT),
                          "Revision": B.REV},
            "Size": {"X": B.BOARD_W, "Y": B.BOARD_H},
            "LayerNumber": 2,
            "BoardThickness": B.THICKNESS,
            "Finish": "None",
        },
        "DesignRules": [{
            "Layers": "Outer",
            "PadToPad": B.CLEARANCE,
            "PadToTrack": B.CLEARANCE,
            "TrackToTrack": B.CLEARANCE,
            "MinLineWidth": B.TRACK_W,
            "TrackToRegion": B.POUR_GAP,
            "RegionToRegion": B.POUR_GAP,
        }],
        "FilesAttributes": [{"Path": name, "FileFunction": function,
                             "FilePolarity": polarity}
                            for name, function, polarity in files],
        "MaterialStackup": [
            {"Type": "Legend", "Notes": "Top silkscreen"},
            {"Type": "SolderPaste", "Notes": "Top paste"},
            {"Type": "SolderMask", "Thickness": 0.01, "Notes": "Top mask"},
            {"Type": "Copper", "Thickness": 0.035, "Notes": "L1 (F.Cu), ground pour"},
            {"Type": "Dielectric", "Thickness": 1.51, "Material": "FR4"},
            {"Type": "Copper", "Thickness": 0.035, "Notes": "L2 (B.Cu), components"},
            {"Type": "SolderMask", "Thickness": 0.01, "Notes": "Bottom mask"},
            {"Type": "SolderPaste", "Notes": "Bottom paste"},
            {"Type": "Legend", "Notes": "Bottom silkscreen"},
        ],
    }
    return json.dumps(job, indent=2) + "\n"


# ---------------------------------------------------------------------------
# BOM and centroid
# ---------------------------------------------------------------------------

def make_bom_jlcpcb():
    rows = ["Comment,Designator,Footprint,LCSC Part #,MPN,Quantity"]
    for value, refs, footprint, lcsc, mpn, manufacturer, _descr in B.bom_groups():
        designator = ",".join(refs)
        if len(refs) > 1:
            designator = '"%s"' % designator
        rows.append("%s,%s,%s,%s,%s %s,%d"
                    % (value, designator, footprint, lcsc, manufacturer, mpn, len(refs)))
    return "\n".join(rows) + "\n"


def make_cpl():
    rows = ["Designator,Mid X,Mid Y,Layer,Rotation"]
    for part in sorted((p for p in B.PARTS if p.assembled), key=lambda p: p.ref):
        rows.append("%s,%.4fmm,%.4fmm,%s,%.1f"
                    % (part.ref, part.centre[0], part.centre[1],
                       "Bottom", float(part.rot % 360)))
    return "\n".join(rows) + "\n"


def make_bom_pcbway():
    rows = ["Item #,Designator,Qty,Manufacturer,Mfg Part #,Description / Value,"
            "Package/Footprint,Type,Your Instructions / Notes"]
    notes = {
        "J1": "BOTTOM side. Cable opening faces the nearest board edge; "
              "the 8 contacts sit inward, the two retention tabs toward the edge",
        "U1": "BOTTOM side. SOT-23 pin 1 (VCC) is the lower-left lead seen from the bottom; "
              "the part sits exactly on the key centre over solid FR-4",
        "U2": "BOTTOM side. Same orientation as U1",
    }
    for i, (value, refs, footprint, _lcsc, mpn, manufacturer, _d) in enumerate(B.bom_groups(), 1):
        designator = ",".join(refs)
        if len(refs) > 1:
            designator = '"%s"' % designator
        note = notes.get(refs[0], "Bottom side")
        rows.append("%d,%s,%d,%s,%s,%s,%s,SMD,%s"
                    % (i, designator, len(refs), manufacturer, mpn, value, footprint, note))
    return "\n".join(rows) + "\n"


# ---------------------------------------------------------------------------
# Driver
# ---------------------------------------------------------------------------

LAYER_FILES = [
    ("F_Cu.gtl", lambda m: make_copper("F"), "Copper,L1,Top", "Positive"),
    ("B_Cu.gbl", lambda m: make_copper("B"), "Copper,L2,Bot", "Positive"),
    ("F_Mask.gts", lambda m: make_mask("F"), "SolderMask,Top", "Negative"),
    ("B_Mask.gbs", lambda m: make_mask("B"), "SolderMask,Bot", "Negative"),
    ("F_Paste.gtp", lambda m: make_paste("F"), "Paste,Top", "Positive"),
    ("B_Paste.gbp", lambda m: make_paste("B"), "Paste,Bot", "Positive"),
    ("F_Silkscreen.gto", lambda m: make_silk("F", m), "Legend,Top", "Positive"),
    ("B_Silkscreen.gbo", lambda m: make_silk("B", m), "Legend,Bot", "Positive"),
    ("Edge_Cuts.gm1", lambda m: make_edge_cuts(), "Profile,NP", "Positive"),
]


def build(directory, marker, suffix):
    os.makedirs(directory, exist_ok=True)
    written = []
    attrs = []
    for name, factory, function, polarity in LAYER_FILES:
        filename = "%s-%s" % (B.PROJECT, name)
        text = factory(marker).render(B.PROJECT)
        path = os.path.join(directory, filename)
        with open(path, "w", newline="\n") as f:
            f.write(text)
        written.append(path)
        attrs.append((filename, function, polarity))

    for plated, name in ((True, "PTH.drl"), (False, "NPTH.drl")):
        filename = "%s-%s" % (B.PROJECT, name)
        path = os.path.join(directory, filename)
        with open(path, "w", newline="\n") as f:
            f.write(make_drill(plated))
        written.append(path)

    job_path = os.path.join(directory, "%s-job.gbrjob" % B.PROJECT)
    with open(job_path, "w", newline="\n") as f:
        f.write(make_job(attrs))
    written.append(job_path)

    zip_path = os.path.join(directory, "%s-gerbers%s.zip" % (B.PROJECT, suffix))
    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as z:
        for path in written:
            z.write(path, os.path.basename(path))
    for path in written:
        os.remove(path)
    return zip_path


def main():
    problems = B.selftest()
    errors = B.check()
    for line in problems:
        print("SELFTEST:", line)
    for line in errors:
        print("DRC:", line)
    if problems or errors:
        raise SystemExit("refusing to write fabrication data: %d selftest problem(s), "
                         "%d rule violation(s)" % (len(problems), len(errors)))

    os.makedirs(OUT, exist_ok=True)
    os.makedirs(PCBWAY_OUT, exist_ok=True)

    jlc_zip = build(OUT, B.JLC_MARKER, "")
    way_zip = build(PCBWAY_OUT, B.PCBWAY_MARKER, "-PCBWay")

    files = [
        (os.path.join(OUT, "%s-BOM-JLCPCB.csv" % B.PROJECT), make_bom_jlcpcb()),
        (os.path.join(OUT, "%s-CPL-JLCPCB.csv" % B.PROJECT), make_cpl()),
        (os.path.join(PCBWAY_OUT, "%s-BOM-PCBWay.csv" % B.PROJECT), make_bom_pcbway()),
        (os.path.join(PCBWAY_OUT, "%s-centroid-PCBWay.csv" % B.PROJECT), make_cpl()),
    ]
    for path, text in files:
        with open(path, "w", newline="\n") as f:
            f.write(text)

    print("board      %.1f x %.1f mm, 2 layers, %.1f mm" % (B.BOARD_W, B.BOARD_H, B.THICKNESS))
    print("components %d on the bottom side, %d probe pads"
          % (sum(1 for p in B.PARTS if p.assembled), len(B.TEST_PADS)))
    print("holes      %d NPTH, %d plated vias" % (len(B.npth_holes()), len(B.VIAS)))
    print("checks     selftest and DRC clean")
    for path in [jlc_zip, way_zip] + [p for p, _ in files]:
        print("wrote      %s" % os.path.relpath(path, V1))


if __name__ == "__main__":
    main()
