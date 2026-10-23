"""Check one board's fab files against its hand-written intended netlist.

    python -m opadverify golden/he_v1_1.toml [--pcb F] [--gerbers F] [--cpl F] [--bom F]

Checks, in order (each problem is an ERROR unless it says WARN):

1. parts:      every footprint with pads is in the intended netlist, and back
2. handedness: each footprint's pads are a rotated, NOT mirrored, copy of the
               datasheet drawing with the same pin numbers (devices.py)
3. landing:    every pin of every part lands on copper in the Gerbers, with a
               solder-mask opening, paste for SMD pins, a plated hole for THT
4. nets:       pins meant to be joined share one piece of copper (else OPEN);
               pins of different nets never share one (else SHORT); NC pins
               touch nothing else
5. copper:     WARN for copper that no pin touches (floating islands)
6. assembly:   every assembled part is in the CPL at its footprint centre with
               the expected rotation and side, and in the BOM with the
               intended LCSC number
6b. jlc:       JLCPCB's own footprint of each LCSC part, placed where the CPL
               says, puts every pin on our pad for that pin (jlc.py)
7. holes:      mechanical holes at the intended positions and sizes

Exit status 0 only if there is no ERROR.
"""

import argparse
import csv
import math
import os
import sys
import tomllib

from . import board as B
from . import devices as D
from . import jlc
from .copper import Copper


class Report:
    def __init__(self, title):
        self.title = title
        self.lines = []
        self.errors = 0
        self.warnings = 0

    def error(self, section, msg):
        self.errors += 1
        self.lines.append(("ERROR", section, msg))

    def warn(self, section, msg):
        self.warnings += 1
        self.lines.append(("WARN", section, msg))

    def info(self, section, msg):
        self.lines.append(("info", section, msg))

    def print(self, verbose=False):
        print("== %s" % self.title)
        for level, section, msg in self.lines:
            if level != "info" or verbose:
                print("  %-5s %-10s %s" % (level, section, msg))
        print("  -> %d error(s), %d warning(s)" % (self.errors, self.warnings))


# ---------------------------------------------------------------------------

def _transforms():
    """The 8 symmetries of the square as (mirror, quarter turns)."""
    for mirror in (False, True):
        for q in range(4):
            yield mirror, q


def _apply(pt, mirror, q):
    x, y = pt
    if mirror:
        x = -x
    for _ in range(q):
        x, y = -y, x          # quarter turn, screen convention (Y down)
    return x, y


def fit(model, pads):
    """Best proper and best mirrored fit of numbered model pins onto pads.

    Returns {False: (dev, transform), True: (dev, transform)}; dev is the
    worst pin-to-pad distance after the best translation."""
    nums = sorted(model)
    best = {}
    for mirror, q in _transforms():
        m = [_apply(model[n], mirror, q) for n in nums]
        p = [pads[n] for n in nums]
        tx = sum(a[0] - b[0] for a, b in zip(p, m)) / len(m)
        ty = sum(a[1] - b[1] for a, b in zip(p, m)) / len(m)
        dev = max(math.hypot(a[0] - b[0] - tx, a[1] - b[1] - ty) for a, b in zip(p, m))
        if mirror not in best or dev < best[mirror][0]:
            best[mirror] = (dev, (q * 90, (tx, ty)))
    return best


# ---------------------------------------------------------------------------

def _read_csv(path):
    with open(path, newline="", encoding="utf-8-sig") as f:
        return list(csv.DictReader(f))


def _mm(v):
    return float(str(v).replace("mm", "").strip())


def run(golden_path, pcb=None, gerbers=None, cpl=None, bom=None, verbose=False, no_jlc=False):
    gdir = os.path.dirname(os.path.abspath(golden_path))
    g = tomllib.load(open(golden_path, "rb"))
    meta = g["board"]

    def path(key, override):
        return override or os.path.normpath(os.path.join(gdir, meta[key]))

    rep = Report(meta["title"])
    origin, fps = B.load(path("pcb", pcb))
    cu = Copper(path("gerbers", gerbers))
    ox, oy = origin

    def to_gerber(x, y):
        return x - ox, oy - y

    golden = g.get("part", {})
    by_ref = {f.ref: f for f in fps}

    # 1. parts -------------------------------------------------------------
    electrical = {f.ref for f in fps if any(p.kind in ("smd", "thru_hole") for p in f.pads)}
    for ref in sorted(electrical - set(golden)):
        rep.error("parts", "%s is on the board but not in the intended netlist" % ref)
    for ref in sorted(set(golden) - set(by_ref)):
        rep.error("parts", "%s is in the intended netlist but not on the board" % ref)

    pin_conductor = {}      # (ref, pin number) -> conductor id
    pin_net = {}            # (ref, pin number) -> intended net
    for ref in sorted(set(golden) & set(by_ref)):
        spec, fp = golden[ref], by_ref[ref]
        dev = D.DEVICES[spec["device"]]
        if not dev.independent:
            rep.info("handedness", "%s: %s pin model confirmed only by a built board (%s)"
                     % (ref, dev.name, dev.source))

        # intended net of every modelled pin
        if "pins" in spec:
            for fname, net in spec["pins"].items():
                num = dev.names.get(fname, fname)
                if num not in dev.pins:
                    rep.error("parts", "%s: intended netlist names pin %r, which %s does not have"
                              % (ref, fname, dev.name))
                    continue
                pin_net[(ref, num)] = net
            missing = set(dev.pins) - {dev.names.get(k, k) for k in spec["pins"]}
            for num in sorted(missing):
                rep.error("parts", "%s pin %s has no intended net (write NC if it is unused)" % (ref, num))
        elif "nets" in spec:          # two-terminal, either way round
            a, b = spec["nets"]
            pin_net[(ref, "1")] = a
            pin_net[(ref, "2")] = b

        # 2. handedness ----------------------------------------------------
        pads = {}
        for p in fp.pads:
            if p.number and p.number not in dev.extra_pads:
                pads.setdefault(p.number, p.local)
        absent = sorted(set(dev.pins) - set(pads))
        if absent:
            rep.error("handedness", "%s: footprint %s has no pad %s" % (ref, fp.lib, ", ".join(absent)))
            continue
        extra = sorted(set(pads) - set(dev.pins))
        if extra:
            rep.error("handedness", "%s: footprint has pads %s that %s does not have"
                      % (ref, ", ".join(extra), dev.name))
        best = fit(dev.pins, pads)
        proper, mirrored = best[False], best[True]
        if proper[0] <= dev.tol:
            rep.info("handedness", "%s: pads match the datasheet drawing turned %d deg (worst %.3f mm)"
                     % (ref, proper[1][0], proper[0]))
        elif not dev.polar and mirrored[0] <= dev.tol:
            rep.info("handedness", "%s: two-terminal part, mirrored fit is fine" % ref)
        elif mirrored[0] <= dev.tol:
            rep.error("handedness", "%s: footprint %s is the MIRROR IMAGE of %s (%s). The pads only "
                      "match the datasheet drawing flipped over, so pins would land on the wrong pads"
                      % (ref, fp.lib, dev.name, dev.source.split(";")[0]))
        else:
            rep.error("handedness", "%s: footprint %s does not match %s: worst pin %.2f mm off"
                      % (ref, fp.lib, dev.name, proper[0]))

        # 3. landing -------------------------------------------------------
        for num in sorted(dev.pins, key=lambda n: (len(n), n)):
            for pad in fp.pad(num):
                gx, gy = to_gerber(*pad.board)
                if pad.kind == "smd":
                    side = "B" if fp.bottom else "F"
                    c = cu.conductor(side, gx, gy)
                    if c is None:
                        rep.error("landing", "%s pin %s: no copper at (%.3f, %.3f) on %s.Cu"
                                  % (ref, num, gx, gy, side))
                        continue
                    if not cu.opening("mask", side, gx, gy):
                        rep.error("landing", "%s pin %s: no solder-mask opening" % (ref, num))
                    wants_paste = spec.get("paste", dev.name != D.DEVICES["TESTPOINT"].name)
                    has_paste = cu.opening("paste", side, gx, gy)
                    if wants_paste and not has_paste:
                        rep.error("landing", "%s pin %s: no solder paste" % (ref, num))
                    if not wants_paste and has_paste:
                        rep.warn("landing", "%s pin %s: has paste but is not meant to be soldered" % (ref, num))
                elif pad.kind == "thru_hole":
                    cf, cb = cu.conductor("F", gx, gy), cu.conductor("B", gx, gy)
                    if not cu.plated_hole_at(gx, gy):
                        rep.error("landing", "%s pin %s: no plated hole at (%.3f, %.3f)" % (ref, num, gx, gy))
                    if cf is None or cb is None or cf != cb:
                        rep.error("landing", "%s pin %s: hole is not copper on both layers" % (ref, num))
                    c = cf if cf is not None else cb
                    if c is None:
                        continue
                else:
                    continue
                prev = pin_conductor.get((ref, num))
                if prev is not None and prev != c:
                    rep.error("nets", "%s pin %s: its pads are on different copper" % (ref, num))
                pin_conductor[(ref, num)] = c

    # 4. nets ----------------------------------------------------------------
    # Two-terminal parts fit either way round: take the order that agrees
    # with the copper the polar pins already pinned down
    anchored = {}
    for key, net in pin_net.items():
        spec = golden.get(key[0], {})
        if "pins" in spec and key in pin_conductor and net != "NC":
            anchored.setdefault(net, set()).add(pin_conductor[key])
    for _ in range(3):
        for ref, spec in golden.items():
            if "nets" not in spec or (ref, "1") not in pin_conductor or (ref, "2") not in pin_conductor:
                continue
            a, b = spec["nets"]
            c1, c2 = pin_conductor[(ref, "1")], pin_conductor[(ref, "2")]
            straight = (c1 in anchored.get(a, ())) + (c2 in anchored.get(b, ()))
            swapped = (c1 in anchored.get(b, ())) + (c2 in anchored.get(a, ()))
            if swapped > straight:
                a, b = b, a
            pin_net[(ref, "1")], pin_net[(ref, "2")] = a, b
            if straight or swapped:
                anchored.setdefault(a, set()).add(c1)
                anchored.setdefault(b, set()).add(c2)
    names = cu.conductor_names()
    net_cond = {}
    for key, net in pin_net.items():
        if key in pin_conductor and net != "NC":
            net_cond.setdefault(net, {}).setdefault(pin_conductor[key], []).append(key)
    for net, conds in sorted(net_cond.items()):
        if len(conds) > 1:
            groups = ["{%s}" % ", ".join("%s.%s" % k for k in sorted(v)) for v in conds.values()]
            rep.error("nets", "OPEN: net %s is split into %d pieces of copper: %s"
                      % (net, len(conds), " | ".join(groups)))
    cond_net = {}
    for key, c in pin_conductor.items():
        cond_net.setdefault(c, {}).setdefault(pin_net.get(key, "?"), []).append(key)
    for c, nets in sorted(cond_net.items()):
        real = {n: v for n, v in nets.items() if n != "NC"}
        if len(real) > 1:
            rep.error("nets", "SHORT: one piece of copper joins nets %s (%s)" % (
                ", ".join(sorted(real)),
                "; ".join("%s: %s" % (n, ", ".join("%s.%s" % k for k in sorted(v))) for n, v in sorted(real.items()))))
        if "NC" in nets and (len(nets["NC"]) > 1 or real):
            rep.error("nets", "NC pin(s) %s touch other pins: %s" % (
                ", ".join("%s.%s" % k for k in nets["NC"]),
                ", ".join("%s.%s" % k for v in real.values() for k in v)))
    for net, conds in sorted(net_cond.items()):
        for c in conds:
            label = names.get(c)
            if label and len(label) == 1:
                rep.info("nets", "%s: KiCad calls this copper %s" % (net, next(iter(label))))
            elif label:
                rep.warn("nets", "%s: KiCad's net labels on this copper disagree: %s" % (net, sorted(label)))

    # 5. floating copper -----------------------------------------------------
    used = set(pin_conductor.values())
    for ref in set(golden) & set(by_ref):     # mechanical tabs (MP) may stand alone
        fp = by_ref[ref]
        for pad in fp.pads:
            if pad.number in D.DEVICES[golden[ref]["device"]].extra_pads and pad.kind == "smd":
                c = cu.conductor("B" if fp.bottom else "F", *to_gerber(*pad.board))
                if c is not None:
                    used.add(c)
    for cid in sorted(set(cu.conductor_ids.values()) - used):
        rep.warn("copper", "copper that touches no pin (KiCad label: %s): %s"
                 % (", ".join(sorted(names.get(cid, {"none"}))), cu.describe(cid)))
    for x, y, d in cu.unplated_on_copper:
        rep.warn("copper", "unplated %.2f mm hole at (%.2f, %.2f) goes through copper" % (d, x, y))

    # 5b. manufacturability of the copper --------------------------------------
    rules = g.get("rules", {})
    fab_min = rules.get("fab_min_gap", 0.1)          # JLCPCB 2-layer standard process
    design_min = rules.get("design_min_gap", 0.15)
    for d, side, a, b, (x, y) in cu.gaps(design_min):
        what = "%s.Cu gap %.3f mm between %s and %s near (%.2f, %.2f)" % (
            side, d, "/".join(sorted(names.get(a, {"?"}))), "/".join(sorted(names.get(b, {"?"}))), x, y)
        if d < fab_min - 1e-6:
            rep.error("spacing", what + " (below the fab's %.2f mm)" % fab_min)
        elif d < design_min - 0.005:
            rep.warn("spacing", what + " (below the design's %.2f mm)" % design_min)
    edge_min = rules.get("edge_min", 0.2)            # JLCPCB: copper to routed edge
    edge_want = rules.get("edge_design", 0.3)
    for side, (d, (x, y)) in cu.edge_clearance().items():
        if d < edge_min - 0.01:
            rep.error("spacing", "%s.Cu copper %.3f mm from the board edge near (%.2f, %.2f), "
                      "needs %.2f mm" % (side, d, x, y, edge_min))
        elif d < edge_want - 0.01:
            rep.warn("spacing", "%s.Cu copper %.3f mm from the board edge near (%.2f, %.2f)"
                     % (side, d, x, y))
        else:
            rep.info("spacing", "%s.Cu copper is at least %.3f mm from the board edge" % (side, d))

    # 6. assembly ------------------------------------------------------------
    cpl_path = cpl or (meta.get("cpl") and os.path.normpath(os.path.join(gdir, meta["cpl"])))
    bom_path = bom or (meta.get("bom") and os.path.normpath(os.path.join(gdir, meta["bom"])))
    rows = {r["Designator"]: r for r in _read_csv(cpl_path)} if cpl_path else {}
    bom_rows = {}
    if bom_path:
        for r in _read_csv(bom_path):
            for ref in r["Designator"].replace(" ", "").split(","):
                bom_rows[ref] = r
    for ref in sorted(set(golden) & set(by_ref)):
        spec, fp = golden[ref], by_ref[ref]
        assembled = spec.get("assembled", True)
        if not cpl_path:
            break
        if not assembled:
            if ref in rows:
                rep.error("assembly", "%s is in the CPL but is not meant to be assembled" % ref)
            continue
        if ref not in rows:
            rep.error("assembly", "%s is missing from the CPL" % ref)
            continue
        r = rows[ref]
        if spec.get("cpl_centre") == "pads":
            pts = [p.board for p in fp.pads if p.kind in ("smd", "thru_hole")]
            cx = (min(p[0] for p in pts) + max(p[0] for p in pts)) / 2
            cy = (min(p[1] for p in pts) + max(p[1] for p in pts)) / 2
        else:
            cx, cy = fp.pos
        ex, ey = to_gerber(cx, cy)
        gx, gy = _mm(r["Mid X"]), _mm(r["Mid Y"])
        side = "Bottom" if fp.bottom else "Top"
        if r["Layer"].strip().lower() != side.lower():
            rep.error("assembly", "%s CPL side %s, footprint is on %s" % (ref, r["Layer"], side))
        # With cpl_from = "jlc" the position and rotation follow JLCPCB's model (section jlc checks them)
        if meta.get("cpl_from") != "jlc":
            if math.hypot(gx - ex, gy - ey) > 0.01:
                rep.error("assembly", "%s CPL position (%.3f, %.3f), footprint centre is (%.3f, %.3f)"
                          % (ref, gx, gy, ex, ey))
            want = (fp.angle + spec.get("cpl_rotation_offset", 0)) % 360
            got = float(r["Rotation"]) % 360
            if abs((got - want + 180) % 360 - 180) > 0.01:
                rep.error("assembly", "%s CPL rotation %.1f, expected %.1f" % (ref, got, want))
        dev = D.DEVICES[spec["device"]]
        first = min(dev.pins, key=lambda n: (len(n), n))
        if dev.polar and len(dev.pins) > 1 and fp.pad(first):
            p1 = fp.pad(first)[0]
            rep.info("assembly", "%s: in JLCPCB's preview, pin 1 of %s must sit on the pad at "
                     "(%.2f, %.2f) mm from the board's lower-left corner, seen from the top"
                     % (ref if first == "1" else "%s (pin %s)" % (ref, first), dev.name, *to_gerber(*p1.board)))
        if bom_path:
            b = bom_rows.get(ref)
            if not b:
                rep.error("assembly", "%s is missing from the BOM" % ref)
            elif spec.get("lcsc") and b["LCSC Part #"].strip() != spec["lcsc"]:
                rep.error("assembly", "%s BOM has LCSC %s, intended %s"
                          % (ref, b["LCSC Part #"], spec["lcsc"]))
            if spec.get("lcsc") and spec.get("mpn"):
                cat = jlc.catalog(spec["lcsc"])
                if cat is None:
                    rep.error("assembly", "%s: %s is not in JLCPCB's catalogue snapshot" % (ref, spec["lcsc"]))
                elif spec["mpn"].lower() not in cat["mfr_part"].lower():
                    rep.error("assembly", "%s: %s is %s (%s), intended %s" % (
                        ref, spec["lcsc"], cat["mfr_part"], cat["manufacturer"], spec["mpn"]))
                elif cat["stock"] < spec.get("need", 10):
                    rep.warn("assembly", "%s: %s has only %d in stock" % (ref, spec["lcsc"], cat["stock"]))
    for ref in sorted(set(rows) - set(golden)):
        rep.error("assembly", "%s is in the CPL but not in the intended netlist" % ref)

    # 6b. JLCPCB's own model at the CPL placement -----------------------------
    if cpl_path and not no_jlc:
        for ref in sorted(set(golden) & set(by_ref) & set(rows)):
            spec, fp = golden[ref], by_ref[ref]
            lcsc = spec.get("lcsc")
            if not lcsc:
                continue
            dev = D.DEVICES[spec["device"]]
            try:
                model = jlc.product(lcsc)
            except Exception as e:      # network or LCSC trouble is not a pass
                cat = jlc.catalog(lcsc)
                if cat and not dev.polar:
                    rep.warn("jlc", "%s: LCSC lists no footprint for %s (%s, JLCPCB %s part); a two-terminal "
                             "part, so only its position is unchecked" % (ref, lcsc, cat["mfr_part"], cat["library_type"]))
                else:
                    rep.error("jlc", "%s: cannot load JLCPCB's model of %s (%s)" % (ref, lcsc, e))
                continue
            r = rows[ref]
            cx, cy, rot = _mm(r["Mid X"]), _mm(r["Mid Y"]), float(r["Rotation"])
            bottom = r["Layer"].strip().lower() == "bottom"
            ours = [(p.number, to_gerber(*p.board)) for p in fp.pads if p.kind in ("smd", "thru_hole")]
            names = getattr(dev, "jlc_names", None) or {}
            bad = []
            for mp in model["pads"]:
                if mp.get("layer") not in ("1", "11"):
                    continue
                x, y = jlc.place(mp, cx, cy, rot, bottom)
                near = min(ours, key=lambda o: math.hypot(o[1][0] - x, o[1][1] - y))
                dist = math.hypot(near[1][0] - x, near[1][1] - y)
                want = names.get(mp["number"], mp["number"])
                if dist > 0.5:
                    bad.append("pad %s lands on no pad (nearest %s, %.2f mm away)" % (mp["number"], near[0], dist))
                elif want in dev.pins and dev.polar and near[0] != want:
                    bad.append("pin %s lands on our pad %s" % (want, near[0]))
            if bad:
                rep.error("jlc", "%s (%s): with the CPL as written, JLCPCB's model is misplaced: %s"
                          % (ref, model["productModel"], "; ".join(bad[:4]) + (" ..." if len(bad) > 4 else "")))
            else:
                rep.info("jlc", "%s: JLCPCB's model of %s lands pin for pin on our pads" % (ref, model["productModel"]))
                if bottom and dev.polar and rot % 180 and len(dev.pins) > 1:
                    rep.warn("jlc", "%s: bottom-side part at %.0f deg; which way JLCPCB turns bottom parts "
                             "is not established, confirm in the preview" % (ref, rot))

    # 7. holes ---------------------------------------------------------------
    for h in g.get("hole", []):
        x, y = to_gerber(h["x"], h["y"])
        hits = [o for o in cu.holes if math.hypot(o[0] - x, o[1] - y) < 0.02]
        if not hits:
            rep.error("holes", "no hole at (%.2f, %.2f) for %s" % (x, y, h.get("what", "?")))
        elif abs(hits[0][2] - h["d"]) > 0.02 or hits[0][3] != h.get("plated", False):
            rep.error("holes", "hole for %s is %.2f mm %s, intended %.2f mm %s"
                      % (h.get("what", "?"), hits[0][2], "plated" if hits[0][3] else "unplated",
                         h["d"], "plated" if h.get("plated") else "unplated"))

    # accepted findings: known, explained, and harmless on a built board
    for acc in g.get("accept", []):
        for i, (level, section, msg) in enumerate(rep.lines):
            if level == "ERROR" and section == acc["section"] and acc["match"] in msg:
                rep.lines[i] = ("WARN", section, msg + " [accepted: %s]" % acc["why"])
                rep.errors -= 1
                rep.warnings += 1
    return rep


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("golden")
    ap.add_argument("--pcb")
    ap.add_argument("--gerbers")
    ap.add_argument("--cpl")
    ap.add_argument("--bom")
    ap.add_argument("-v", "--verbose", action="store_true")
    ap.add_argument("--no-jlc", action="store_true", help="skip the JLCPCB model check (needs network)")
    a = ap.parse_args(argv)
    rep = run(a.golden, a.pcb, a.gerbers, a.cpl, a.bom, a.verbose, a.no_jlc)
    rep.print(a.verbose)
    return 1 if rep.errors else 0


if __name__ == "__main__":
    sys.exit(main())
