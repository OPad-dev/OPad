"""JLCPCB's own part models, and where the assembler would put each pin.

JLCPCB places the footprint from its EasyEDA/LCSC library at the CPL's
position and rotation; it never sees our footprint. The model comes from
LCSC's product API (`edaSvgInfo.pcbSvg`) and is cached in
.tools/jlc_cache/ (not committed: it is LCSC's data).

Placement convention, from the evidence in this repo:
- top side: model rotated counter-clockwise by the CPL rotation;
- bottom side: model mirrored left-right (X -> -X as seen from the top),
  then rotated. Evidence: HE V2's U4 at CPL 0 deg, bottom: JLCPCB's preview put
  pin 1 diagonally opposite our pin-1 corner (docs/specs/hardware-redesign-
  brief.md 5.4). Mirroring top-bottom instead would have put it on the same
  corner. Which way a bottom-side rotation turns is NOT established, so a
  polarised bottom part at 90 or 270 deg is reported as unverified.
"""

import json
import math
import os
import re
import subprocess
import time

CACHE = os.path.normpath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".tools", "jlc_cache"))
API = "https://wmsc.lcsc.com/ftps/wm/product/detail?productCode="
MIL10 = 0.254          # EasyEDA canvas unit (10 mil) in mm


def _get(url):
    out = subprocess.run(["curl", "-s", "-L", "--max-time", "30", "-A", "Mozilla/5.0", url],
                         capture_output=True, text=True, check=True).stdout
    return out


def product(lcsc):
    """LCSC product record (cached): model, maker, package, stock, footprint SVG URL."""
    os.makedirs(CACHE, exist_ok=True)
    path = os.path.join(CACHE, lcsc + ".json")
    if os.path.exists(path):
        return json.load(open(path))
    r = json.loads(_get(API + lcsc))["result"]
    if not r:
        raise LookupError("LCSC has no part %s" % lcsc)
    keep = {k: r.get(k) for k in ("productCode", "productModel", "brandNameEn", "encapStandard",
                                  "productNameEn", "stockNumber", "pdfUrl", "edaSvgInfo")}
    keep["fetched"] = time.strftime("%Y-%m-%d")
    svg = _get("https:" + r["edaSvgInfo"]["pcbSvg"]) if r.get("edaSvgInfo") else ""
    keep["pads"] = parse_pads(svg) if svg else []
    json.dump(keep, open(path, "w"), indent=1)
    return keep


def parse_pads(svg):
    """Pads of an EasyEDA footprint SVG: [{number, x, y, w, h}] in mm, Y down, from the model origin."""
    m = re.search(r'<g c_origin="([-\d.]+),([-\d.]+)"', svg)
    ox, oy = float(m.group(1)), float(m.group(2))
    pads = []
    for g in re.finditer(r'<g c_partid="part_pad"([^>]*)>', svg):
        a = dict(re.findall(r'([\w]+)\s*="([^"]*)"', g.group(1)))
        x, y = (float(v) for v in a["c_origin"].split(","))
        pads.append({"number": a.get("number", ""), "x": round((x - ox) * MIL10, 4),
                     "y": round((y - oy) * MIL10, 4),
                     "w": round(float(a.get("c_width", 0)) * MIL10, 4),
                     "h": round(float(a.get("c_height", 0)) * MIL10, 4),
                     "layer": a.get("layerid")})
    return pads


def place(pad, cx, cy, rotation, bottom):
    """Where JLCPCB puts a model pad, in Gerber coordinates (mm, Y up)."""
    x, y = pad["x"], -pad["y"]            # model to Y up
    if bottom:
        x = -x
    a = math.radians(rotation)
    return (cx + x * math.cos(a) - y * math.sin(a), cy + x * math.sin(a) + y * math.cos(a))


CATALOG = os.path.expanduser("~/.local/share/kicad-mcp/jlcpcb_parts.db")


def catalog(lcsc):
    """JLCPCB's catalogue entry (local snapshot), or None: {mfr_part, manufacturer, package,
    library_type, stock, description}."""
    import sqlite3
    if not os.path.exists(CATALOG):
        return None
    con = sqlite3.connect(CATALOG)
    row = con.execute("select mfr_part, manufacturer, package, library_type, stock, description "
                      "from components where lcsc = ?", (lcsc,)).fetchone()
    con.close()
    if not row:
        return None
    return dict(zip(("mfr_part", "manufacturer", "package", "library_type", "stock", "description"), row))
