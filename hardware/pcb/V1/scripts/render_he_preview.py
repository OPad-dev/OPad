#!/usr/bin/env python3
"""Composite the generated he_input_v1 Gerbers into board preview images.

    python3 hardware/pcb/V1/scripts/render_he_preview.py

Writes ``HE/production/he_input_v1-bottom.png`` and ``-top.png``.  These are 2D
artwork composites of the real Gerber files, not 3D renders: they exist so the
layout can be checked at a glance without opening a CAD tool, and because the
bottom view is the one that matters here (every component is on the bottom).

The bottom image is mirrored, so it shows the board as you physically see it
with the solder side facing you.
"""

import math
import os
import sys
import zipfile

try:
    from PIL import Image, ImageChops, ImageDraw, ImageOps
except ImportError:                                      # pragma: no cover
    sys.exit("render_he_preview.py needs Pillow (pip install pillow)")

import he_board as B
from verify_he_gerbers import rasterise

V1 = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(V1, "HE", "production")
ZIP = os.path.join(OUT, "%s-gerbers.zip" % B.PROJECT)

SUBSTRATE = (26, 68, 40)
COPPER = (38, 94, 52)
PAD = (218, 176, 84)
SILK = (236, 238, 235)
HOLE = (16, 18, 16)
EDGE = (12, 14, 12)


def load(zf, name):
    return zf.read("%s-%s" % (B.PROJECT, name)).decode()


def composite(zf, side):
    copper = rasterise(load(zf, "B_Cu.gbl" if side == "B" else "F_Cu.gtl"))
    mask = rasterise(load(zf, "B_Mask.gbs" if side == "B" else "F_Mask.gts"))
    silk = rasterise(load(zf, "B_Silkscreen.gbo" if side == "B" else "F_Silkscreen.gto"))
    edge = rasterise(load(zf, "Edge_Cuts.gm1"))

    size = copper.img.size
    img = Image.new("RGB", size, (0, 0, 0))

    # board body: fill inside the outline
    body = Image.new("1", size, 0)
    d = ImageDraw.Draw(body)
    pts = []
    r = B.CORNER_R
    for cx, cy, a0, a1 in ((r, r, 180, 270),
                           (B.BOARD_W - r, r, 270, 360),
                           (B.BOARD_W - r, B.BOARD_H - r, 0, 90),
                           (r, B.BOARD_H - r, 90, 180)):
        for k in range(17):
            a = math.radians(a0 + (a1 - a0) * k / 16.0)
            pts.append(copper.px(cx + r * math.cos(a), cy + r * math.sin(a)))
    d.polygon(pts, fill=1)

    img.paste(SUBSTRATE, (0, 0), body)
    img.paste(COPPER, (0, 0), copper.img)
    # A pad is copper that the solder mask opens over.
    img.paste(PAD, (0, 0), ImageChops.logical_and(copper.img, mask.img))

    img.paste(SILK, (0, 0), silk.img)
    img.paste(EDGE, (0, 0), edge.img)

    # drill holes
    holes = Image.new("1", size, 0)
    hd = ImageDraw.Draw(holes)
    for hx, hy, dia in B.npth_holes():
        rr = dia / 2.0
        hd.ellipse([copper.px(hx - rr, hy + rr), copper.px(hx + rr, hy - rr)], fill=1)
    for _net, (vx, vy) in B.VIAS:
        rr = B.VIA_DRILL / 2.0
        hd.ellipse([copper.px(vx - rr, vy + rr), copper.px(vx + rr, vy - rr)], fill=1)
    img.paste(HOLE, (0, 0), holes)

    # outside the board is transparent-ish background
    outside = ImageOps.invert(body.convert("L")).point(lambda v: 255 if v > 127 else 0, "1")
    img.paste((0, 0, 0), (0, 0), outside)

    if side == "B":
        img = ImageOps.mirror(img)
    return img


def main():
    if not os.path.exists(ZIP):
        sys.exit("%s not found; run generate_he_gerbers.py first" % ZIP)
    with zipfile.ZipFile(ZIP) as zf:
        for side, name in (("B", "bottom"), ("F", "top")):
            img = composite(zf, side)
            path = os.path.join(OUT, "%s-%s.png" % (B.PROJECT, name))
            img.save(path)
            print("wrote %s (%dx%d)" % (os.path.relpath(path, V1), img.width, img.height))


if __name__ == "__main__":
    main()
