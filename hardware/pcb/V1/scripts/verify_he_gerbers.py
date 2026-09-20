#!/usr/bin/env python3
"""Verify the generated he_input_v1 Gerbers against the netlist.

This does not re-read ``he_board``'s geometry and pronounce it good: it
rasterises the copper that was actually written into the Gerber files,
including every clear-polarity knockout, labels the connected regions, joins
the two layers through the plated vias, and then checks that

  * every pad and via of a net lands in one single copper region (no opens), and
  * no copper region carries two different nets (no shorts), and
  * the sensor keepouts contain no pour copper, and
  * no copper sits over a non-plated hole.

Run after generate_he_gerbers.py:

    python3 hardware/pcb/V1/scripts/verify_he_gerbers.py

Requires Pillow. Exits non-zero on any failure.
"""

import os
import re
import sys
import zipfile

try:
    from PIL import Image, ImageDraw
except ImportError:                                      # pragma: no cover
    sys.exit("verify_he_gerbers.py needs Pillow (pip install pillow)")

import he_board as B

V1 = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ZIP = os.path.join(V1, "HE", "production", "%s-gerbers.zip" % B.PROJECT)

DPMM = 40                      # raster resolution, pixels per mm
PAD = 2.0                      # mm of margin around the board


# ---------------------------------------------------------------------------
# Gerber raster
# ---------------------------------------------------------------------------

class Raster:
    def __init__(self):
        self.w = int((B.BOARD_W + 2 * PAD) * DPMM)
        self.h = int((B.BOARD_H + 2 * PAD) * DPMM)
        self.img = Image.new("1", (self.w, self.h), 0)
        self.draw = ImageDraw.Draw(self.img)

    def px(self, x, y):
        return ((x + PAD) * DPMM, self.h - (y + PAD) * DPMM)

    def circle(self, c, dia, fill):
        r = dia / 2.0
        self.draw.ellipse([self.px(c[0] - r, c[1] + r), self.px(c[0] + r, c[1] - r)], fill=fill)

    def rect(self, c, w, h, fill):
        self.draw.rectangle([self.px(c[0] - w / 2, c[1] + h / 2),
                             self.px(c[0] + w / 2, c[1] - h / 2)], fill=fill)

    def line(self, a, b, width, fill):
        self.draw.line([self.px(*a), self.px(*b)], fill=fill, width=max(1, int(width * DPMM)))
        self.circle(a, width, fill)
        self.circle(b, width, fill)

    def polygon(self, pts, fill):
        self.draw.polygon([self.px(*p) for p in pts], fill=fill)


def rasterise(text):
    """Replay one Gerber file onto a raster, honouring LPD / LPC."""
    r = Raster()
    apertures = {}
    cur = None
    lp = 1
    x = y = 0.0
    lastx = lasty = 0.0
    region = None

    for raw in text.splitlines():
        line = raw.strip()
        m = re.match(r"%ADD(\d+)([CR]),([\d.]+)(?:X([\d.]+))?\*%$", line)
        if m:
            code = int(m.group(1))
            apertures[code] = (("C", float(m.group(3))) if m.group(2) == "C"
                               else ("R", float(m.group(3)), float(m.group(4))))
            continue
        if line == "%LPD*%":
            lp = 1
            continue
        if line == "%LPC*%":
            lp = 0
            continue
        if line == "G36*":
            region = []
            continue
        if line == "G37*":
            if region and len(region) >= 3:
                r.polygon(region, lp)
            region = None
            continue
        m = re.match(r"D(\d+)\*$", line)
        if m and int(m.group(1)) >= 10:
            cur = int(m.group(1))
            continue
        m = re.match(r"(?:X(-?\d+))?(?:Y(-?\d+))?D0?([123])\*$", line)
        if not m:
            continue
        if m.group(1) is not None:
            x = int(m.group(1)) / 1e6
        if m.group(2) is not None:
            y = int(m.group(2)) / 1e6
        op = m.group(3)
        if region is not None:
            region.append((x, y))
        elif op == "1":
            ap = apertures[cur]
            r.line((lastx, lasty), (x, y), ap[1], lp)
        elif op == "3":
            ap = apertures[cur]
            if ap[0] == "C":
                r.circle((x, y), ap[1], lp)
            else:
                r.rect((x, y), ap[1], ap[2], lp)
        lastx, lasty = x, y
    return r


# ---------------------------------------------------------------------------
# Connected components
# ---------------------------------------------------------------------------

class Union:
    def __init__(self):
        self.parent = {}

    def find(self, a):
        while self.parent.get(a, a) != a:
            self.parent[a] = self.parent.get(self.parent[a], self.parent[a])
            a = self.parent[a]
        return a

    def union(self, a, b):
        ra, rb = self.find(a), self.find(b)
        if ra != rb:
            self.parent[rb] = ra


def label(raster, tag):
    """Run-length connected component labelling. Returns (lookup, union)."""
    w, h = raster.w, raster.h
    px = raster.img.load()
    uf = Union()
    rows = []
    counter = [0]

    prev = []
    for yy in range(h):
        runs = []
        xx = 0
        while xx < w:
            if px[xx, yy]:
                start = xx
                while xx < w and px[xx, yy]:
                    xx += 1
                counter[0] += 1
                name = (tag, counter[0])
                uf.find(name)
                for ps, pe, pname in prev:
                    if ps <= xx - 1 and start <= pe:
                        uf.union(pname, name)
                runs.append((start, xx - 1, name))
            else:
                xx += 1
        rows.append(runs)
        prev = runs
    return rows, uf


def component_at(rows, uf, raster, point):
    """Root label of the copper region covering a board-coordinate point."""
    fx, fy = raster.px(*point)
    ix, iy = int(fx), int(fy)
    if not (0 <= ix < raster.w and 0 <= iy < raster.h):
        return None
    for start, end, name in rows[iy]:
        if start <= ix <= end:
            return uf.find(name)
    return None


# ---------------------------------------------------------------------------
# Checks
# ---------------------------------------------------------------------------

def main():
    if not os.path.exists(ZIP):
        sys.exit("%s not found; run generate_he_gerbers.py first" % ZIP)

    with zipfile.ZipFile(ZIP) as z:
        layers = {}
        for name in ("F_Cu.gtl", "B_Cu.gbl"):
            layers[name[0]] = z.read("%s-%s" % (B.PROJECT, name)).decode()

    failures = []
    rasters, rows, unions = {}, {}, {}
    for side in ("F", "B"):
        rasters[side] = rasterise(layers[side])
        rows[side], unions[side] = label(rasters[side], side)

    # One union-find across both layers, joined at the vias.
    uf = Union()
    for side in ("F", "B"):
        for row in rows[side]:
            for _s, _e, name in row:
                uf.union(unions[side].find(name), name)

    def comp(side, point):
        root = component_at(rows[side], unions[side], rasters[side], point)
        return uf.find(root) if root else None

    for _net, pos in B.VIAS:
        a, b = comp("F", pos), comp("B", pos)
        if a and b:
            uf.union(a, b)
        else:
            failures.append("via at %s does not land on copper on both layers" % (pos,))

    # 1 / 2. net continuity and isolation
    probes = {}
    for ref, num, pos, _size, net, _tp in B.all_pads():
        if net:
            probes.setdefault(net, []).append(("%s.%s" % (ref, num), "B", pos))
    for net, pos in B.VIAS:
        probes.setdefault(net, []).append(("via%s" % (pos,), "B", pos))

    owner = {}
    for net, points in sorted(probes.items()):
        roots = set()
        for name, side, pos in points:
            root = comp(side, pos)
            if root is None:
                failures.append("%s [%s] sits on no copper" % (name, net))
                continue
            roots.add(root)
            if root in owner and owner[root] != net:
                failures.append("SHORT: %s joins %s and %s" % (name, owner[root], net))
            owner[root] = net
        if len(roots) > 1:
            failures.append("OPEN: net %s is split across %d copper regions (%s)"
                            % (net, len(roots), ", ".join(n for n, _s, _p in points)))

    # 3. sensor keepouts hold no pour copper
    for (x0, y0, x1, y1), key in zip(B.KEEPOUTS, (B.KEY1, B.KEY2)):
        pad_boxes = []
        for ref, num, pos, size, _net, _tp in B.all_pads():
            pad_boxes.append(B.pad_rect(pos, (size[0] + 0.35, size[1] + 0.35)))
        track_pts = [(net, pts, w) for net, pts, w, ly in B.TRACKS if ly == "B"]
        stray = 0
        for side in ("F", "B"):
            r = rasters[side]
            px = r.img.load()
            for iy in range(int((y0 + PAD) * DPMM), int((y1 + PAD) * DPMM)):
                for ix in range(int((x0 + PAD) * DPMM), int((x1 + PAD) * DPMM)):
                    yy = r.h - iy
                    if not px[ix, yy]:
                        continue
                    bx, by = ix / DPMM - PAD, iy / DPMM - PAD
                    # copper belonging to the sensor itself is expected
                    near = any(bx0 <= bx <= bx1 and by0 <= by <= by1
                               for bx0, by0, bx1, by1 in pad_boxes)
                    if near:
                        continue
                    if _near_track(bx, by, track_pts, 0.35):
                        continue
                    stray += 1
        if stray > 0:
            failures.append("keepout around %s has %d px of unexpected copper"
                            % (key, stray))

    # 4. no copper over a non-plated hole
    for hx, hy, dia in B.npth_holes():
        for side in ("F", "B"):
            r = rasters[side]
            px = r.img.load()
            hit = 0
            rad = dia / 2.0
            steps = int(rad * DPMM)
            for iy in range(-steps, steps + 1):
                for ix in range(-steps, steps + 1):
                    if ix * ix + iy * iy > steps * steps:
                        continue
                    fx, fy = r.px(hx + ix / DPMM, hy + iy / DPMM)
                    if px[int(fx), int(fy)]:
                        hit += 1
            if hit:
                failures.append("NPTH d%.2f at (%.3f, %.3f) has copper in it on %s.Cu"
                                % (dia, hx, hy, side))

    for line in failures:
        print("FAIL:", line)
    nets = sorted(probes)
    print("verified %d nets (%s) across %d pads and %d vias at %d px/mm"
          % (len(nets), ", ".join(nets), len(B.all_pads()), len(B.VIAS), DPMM))
    print("result: %s" % ("FAILED (%d)" % len(failures) if failures else "clean"))
    return 1 if failures else 0


def _near_track(bx, by, tracks, tol):
    for _net, pts, w in tracks:
        for a, b in zip(pts, pts[1:]):
            probe = B.Item("cap", "x", "B", "t", seg=(a, b), width=w)
            if probe.distance_to_point((bx, by)) <= tol:
                return True
    return False


if __name__ == "__main__":
    sys.exit(main())
