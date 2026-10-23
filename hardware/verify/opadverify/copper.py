"""Copper connectivity rebuilt from the fab files themselves (Gerbers + Excellon).

This reads exactly what JLCPCB receives, not the KiCad board: every flash,
track, arc and region on each copper layer is drawn in order, with dark and
clear polarity, into one geometry per layer. Each separate piece of copper
is a conductor; plated holes join the pieces they pass through on both
layers. Net names written by KiCad into the Gerbers (X2 attributes) are not
used for connectivity, only to label the report.
"""

import io
import math
import zipfile

import gerbonara as gb
from gerbonara import graphic_primitives as gp
from shapely import STRtree
from shapely.geometry import LineString, Point, Polygon, box
from shapely.affinity import rotate as srotate
from shapely.ops import unary_union

ARC_STEP = math.radians(5)


def _arc_points(x1, y1, x2, y2, cx, cy, clockwise):
    a1 = math.atan2(y1 - cy, x1 - cx)
    a2 = math.atan2(y2 - cy, x2 - cx)
    r = math.hypot(x1 - cx, y1 - cy)
    if clockwise:
        while a2 >= a1:
            a2 -= 2 * math.pi
    else:
        while a2 <= a1:
            a2 += 2 * math.pi
    n = max(2, int(abs(a2 - a1) / ARC_STEP) + 1)
    return [(cx + r * math.cos(a1 + (a2 - a1) * i / n), cy + r * math.sin(a1 + (a2 - a1) * i / n))
            for i in range(n + 1)]


def _shape(p):
    if isinstance(p, gp.Circle):
        return Point(p.x, p.y).buffer(p.r, 32)
    if isinstance(p, gp.Rectangle):
        r = box(p.x - p.w / 2, p.y - p.h / 2, p.x + p.w / 2, p.y + p.h / 2)
        return srotate(r, p.rotation, origin=(p.x, p.y), use_radians=True) if p.rotation else r
    if isinstance(p, gp.Line):
        if p.width <= 0:
            return None
        return LineString([(p.x1, p.y1), (p.x2, p.y2)]).buffer(p.width / 2, 16)
    if isinstance(p, gp.Arc):
        pts = _arc_points(p.x1, p.y1, p.x2, p.y2, p.cx, p.cy, p.clockwise)
        return LineString(pts).buffer(max(p.width, 1e-4) / 2, 16)
    if isinstance(p, gp.ArcPoly):
        pts = []
        for (a, b), arc in zip(zip(p.outline, p.outline[1:] + p.outline[:1]),
                               list(p.arc_centers) + [None] * len(p.outline)):
            if arc and arc[1] is not None:
                cw, (cx, cy) = arc
                pts += _arc_points(a[0], a[1], b[0], b[1], cx, cy, cw)[:-1]
            else:
                pts.append(a)
        if len(pts) < 3:
            return None
        poly = Polygon(pts)
        return poly if poly.is_valid else poly.buffer(0)
    raise TypeError("unknown primitive %r" % (p,))


def _net_attr(obj):
    attrs = getattr(obj, "attrs", None)
    if not isinstance(attrs, dict):     # gerbonara leaves a dataclass Field on regions
        return None
    for k in (".N", "N"):
        if k in attrs:
            v = attrs[k]
            return v[0] if isinstance(v, (list, tuple)) else v
    return None


def render_layer(gerber):
    """Return (geometry, labels): the copper of one layer and (point, net) samples."""
    geom = Polygon()
    pending, pending_dark = [], True
    labels = []

    def flush():
        nonlocal geom, pending
        if pending:
            u = unary_union(pending)
            geom = geom.union(u) if pending_dark else geom.difference(u)
            pending = []

    for obj in gerber.objects:
        net = _net_attr(obj)
        for prim in obj.to_primitives(gb.utils.MM):
            s = _shape(prim)
            if s is None or s.is_empty:
                continue
            dark = prim.polarity_dark
            if dark != pending_dark:
                flush()
                pending_dark = dark
            pending.append(s)
            if net and dark:
                labels.append((s.representative_point(), net))
    flush()
    return geom, labels


class Copper:
    """Conductors of a 2-layer board: query which conductor covers a point."""

    def __init__(self, zip_path):
        files = {}
        with zipfile.ZipFile(zip_path) as z:
            for name in z.namelist():
                files[name] = z.read(name)
        self.files = files

        def find(suffix):
            hits = [n for n in files if n.endswith(suffix)]
            return hits[0] if hits else None

        def gerber(suffix):
            n = find(suffix)
            return gb.GerberFile.from_string(files[n].decode(), filename=n) if n else None

        self.layer_names = {"F": find("-F_Cu.gtl"), "B": find("-B_Cu.gbl")}
        self.pieces, self.labels = {}, {}
        for side in "FB":
            g = gerber("-F_Cu.gtl" if side == "F" else "-B_Cu.gbl")
            geom, labels = render_layer(g) if g else (Polygon(), [])
            parts = list(geom.geoms) if hasattr(geom, "geoms") else ([geom] if not geom.is_empty else [])
            self.pieces[side] = parts
            self.labels[side] = labels
        self.mask = {}
        self.paste = {}
        for side, ms, ps in (("F", "-F_Mask.gts", "-F_Paste.gtp"), ("B", "-B_Mask.gbs", "-B_Paste.gbp")):
            for store, suffix in ((self.mask, ms), (self.paste, ps)):
                g = gerber(suffix)
                store[side] = render_layer(g)[0] if g else Polygon()
        self.trees = {s: STRtree(self.pieces[s]) for s in "FB"}
        edge = gerber("-Edge_Cuts.gm1")
        self.outline = None
        if edge:
            lines = []
            for obj in edge.objects:
                for prim in obj.to_primitives(gb.utils.MM):
                    if isinstance(prim, gp.Line):
                        lines.append(((prim.x1, prim.y1), (prim.x2, prim.y2)))
                    elif isinstance(prim, gp.Arc):
                        pts = _arc_points(prim.x1, prim.y1, prim.x2, prim.y2, prim.cx, prim.cy, prim.clockwise)
                        lines += list(zip(pts, pts[1:]))
            from shapely.ops import polygonize
            snap = lambda q: (round(q[0], 3), round(q[1], 3))
            polys = list(polygonize([LineString([snap(a), snap(b)]) for a, b in lines if snap(a) != snap(b)]))
            if polys:
                self.outline = max(polys, key=lambda q: q.area)
        self.holes = []          # (x, y, diameter, plated)
        for suffix, plated in (("-PTH.drl", True), ("-NPTH.drl", False)):
            n = find(suffix)
            if not n:
                continue
            d = gb.ExcellonFile.from_string(files[n].decode(), filename=n)
            for o in d.objects:
                if hasattr(o, "x") and hasattr(o, "aperture"):
                    self.holes.append((o.x, o.y, o.aperture.diameter, plated))
                elif hasattr(o, "x1"):     # routed slot: both ends
                    self.holes.append(((o.x1 + o.x2) / 2, (o.y1 + o.y2) / 2, o.aperture.diameter, plated))
        self._join()

    def piece_at(self, side, x, y):
        pt = Point(x, y)
        for i in self.trees[side].query(pt):
            if self.pieces[side][i].covers(pt):
                return int(i)
        return None

    def _join(self):
        parent = {}

        def find(a):
            while parent.setdefault(a, a) != a:
                parent[a] = parent[parent[a]]
                a = parent[a]
            return a

        for side in "FB":
            for i in range(len(self.pieces[side])):
                find((side, i))
        self.unplated_on_copper = []
        for x, y, dia, plated in self.holes:
            f, b = self.piece_at("F", x, y), self.piece_at("B", x, y)
            if plated:
                if f is not None and b is not None:
                    parent[find(("F", f))] = find(("B", b))
            elif f is not None or b is not None:
                self.unplated_on_copper.append((x, y, dia))
        self._find = find
        roots = sorted({find(k) for k in list(parent)})
        self.conductor_ids = {r: n for n, r in enumerate(roots)}

    def conductor(self, side, x, y):
        i = self.piece_at(side, x, y)
        return None if i is None else self.conductor_ids[self._find((side, i))]

    def conductor_names(self):
        """KiCad's X2 net names per conductor (report only)."""
        out = {}
        for side in "FB":
            for pt, net in self.labels[side]:
                c = self.conductor(side, pt.x, pt.y)
                if c is not None:
                    out.setdefault(c, set()).add(net)
        return out

    def describe(self, cid):
        """Where a conductor is: its layers, bounding box and area (mm)."""
        parts = []
        for side in "FB":
            for i, g in enumerate(self.pieces[side]):
                if self.conductor_ids[self._find((side, i))] == cid:
                    x0, y0, x1, y1 = g.bounds
                    parts.append("%s.Cu (%.2f, %.2f)-(%.2f, %.2f), %.3f mm2" % (side, x0, y0, x1, y1, g.area))
        return "; ".join(parts)

    def gaps(self, below):
        """(gap, side, conductor a, conductor b, point) for every pair of
        different conductors closer than `below` mm on one layer."""
        out = []
        for side in "FB":
            pieces = self.pieces[side]
            ids = [self.conductor_ids[self._find((side, i))] for i in range(len(pieces))]
            for i, g in enumerate(pieces):
                for j in self.trees[side].query(g.buffer(below)):
                    j = int(j)
                    if j <= i or ids[i] == ids[j]:
                        continue
                    d = g.distance(pieces[j])
                    if d < below:
                        from shapely.ops import nearest_points
                        pa, _ = nearest_points(g, pieces[j])
                        out.append((d, side, ids[i], ids[j], (pa.x, pa.y)))
        return sorted(out)

    def edge_clearance(self):
        """Smallest distance from any copper to the board outline, per layer."""
        if self.outline is None:
            return {}
        ring = self.outline.exterior
        out = {}
        for side in "FB":
            ds = [(g.distance(ring), g.representative_point()) if self.outline.contains(g)
                  else (0.0, g.difference(self.outline).representative_point())
                  for g in self.pieces[side]]
            if ds:
                d, pt = min(ds, key=lambda t: t[0])
                out[side] = (d, (pt.x, pt.y))
        return out

    def opening(self, kind, side, x, y):
        store = self.mask if kind == "mask" else self.paste
        return store[side].covers(Point(x, y))

    def plated_hole_at(self, x, y, tol=0.05):
        return [h for h in self.holes if h[3] and math.hypot(h[0] - x, h[1] - y) <= tol]
