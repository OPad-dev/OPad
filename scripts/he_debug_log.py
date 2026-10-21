#!/usr/bin/env python3
"""Analyse the HE debug firmware's log (CONFIG_OSUPAD_HE_DEBUG, he-debug.log).

Built while chasing phantom key presses on an HE pad
(docs/history/2026-10-22-he-phantom-presses.md). A press counts as a phantom
when its reading jumped more than SPIKE counts in the single frame (250 us)
before it: a finger moves a key ~10 counts per frame at most.

    scripts/he_debug_log.py summary  [LOG]          per boot: real and phantom presses
    scripts/he_debug_log.py table    [LOG] [BOOT]   per second around phantoms
    scripts/he_debug_log.py rhythm   [LOG] [BOOT]   which period the phantoms line up on
    scripts/he_debug_log.py spikes   [LOG] [BOOT]   every phantom with its 12 frames

LOG defaults to ~/.local/share/opad/he-debug.log (.gz is read too); BOOT is
the boot number from `summary` (default: the last). Plain Python 3, no deps.
"""

import collections
import gzip
import math
import os
import re
import sys

SPIKE = 60
EDGE = re.compile(r"he\.edge\s+\[\s*(\d+) ms\] (K\d) (DOWN|UP) (\S+) raw([+-]\d+)")
PRE = re.compile(r"he\.pre\s+\[\s*(\d+) ms\] (K\d)((?: \S+)+)")
CTX = re.compile(r"he\.ctx\s+\[\s*(\d+) ms\] state (\w+)(.*)")


def read(path):
    opener = gzip.open if path.endswith(".gz") else open
    with opener(path, "rt", errors="replace") as fh:
        return fh.read().splitlines()


def boots(lines):
    """Line ranges, one per pad boot ('debug log started')."""
    starts = [i for i, l in enumerate(lines) if "debug log started" in l] or [0]
    return [(s, e) for s, e in zip(starts, starts[1:] + [len(lines)])]


def presses(lines, lo, hi):
    """(ms, key, phantom, jump, state, wall, frames) for every DOWN edge."""
    out, state = [], "idle"
    for i in range(lo, min(hi, len(lines) - 1)):
        c = CTX.search(lines[i])
        if c:
            state = c.group(2)
        e = EDGE.search(lines[i])
        if not e or e.group(3) != "DOWN":
            continue
        p = PRE.search(lines[i + 1])
        if not p:
            continue
        v = [int(x) if x != "x" else 0 for x in p.group(3).split()]
        jump = v[-1] - v[-2]
        out.append((int(e.group(1)), e.group(2), jump > SPIKE, jump, state, lines[i][11:19], v))
    return out


def events(ps):
    """Phantom times, both keys in the same 3 ms counted once."""
    ev = []
    for t in sorted(p[0] for p in ps if p[2]):
        if not ev or t - ev[-1] > 3:
            ev.append(t)
    return ev


def main():
    args = sys.argv[1:]
    if not args or args[0] not in ("summary", "table", "rhythm", "spikes"):
        sys.exit(__doc__)
    cmd = args[0]
    path = args[1] if len(args) > 1 else os.path.expanduser("~/.local/share/opad/he-debug.log")
    lines = read(path)
    bs = boots(lines)
    if cmd == "summary":
        for n, (lo, hi) in enumerate(bs):
            ps = presses(lines, lo, hi)
            ph = [p for p in ps if p[2]]
            by = collections.Counter(p[4] for p in ph)
            print(f"boot {n}  {lines[lo][11:19]}  real {sum(1 for p in ps if not p[2]):5d}  "
                  f"phantom {len(ph):5d}  by state {dict(by)}")
        return
    n = int(args[2]) if len(args) > 2 else len(bs) - 1
    lo, hi = bs[n]
    ps = presses(lines, lo, hi)
    if cmd == "spikes":
        for t, k, ph, jump, st, wall, v in ps:
            if ph:
                print(f"{wall} {t:9d} ms {k} {st:8s} jump {jump:+4d}  frames {' '.join(f'{x:+d}' for x in v)}")
        return
    if cmd == "rhythm":
        ev = events(ps)
        if len(ev) < 5:
            sys.exit("too few phantoms for a rhythm")

        def align(period):
            c = sum(math.cos(2 * math.pi * x / period) for x in ev)
            s = sum(math.sin(2 * math.pi * x / period) for x in ev)
            return math.hypot(c, s) / len(ev)

        print(f"{len(ev)} phantom events, {ev[0] / 1000:.1f}..{ev[-1] / 1000:.1f} s (1.0 = all on one grid)")
        best = sorted(((align(p / 100), p / 100) for p in range(200, 20001)), reverse=True)[:5]
        for a, p in best:
            print(f"  best  {p:7.2f} ms  {a:.3f}")
        for p in (5, 10, 16.667, 20, 33.333, 40):
            print(f"  check {p:7.3f} ms  {align(p):.3f}")
        return
    # table
    ph = collections.Counter(p[0] // 1000 for p in ps if p[2])
    real = collections.Counter(p[0] // 1000 for p in ps if not p[2])
    ctx = {}
    for i in range(lo, hi):
        c = CTX.search(lines[i])
        if c and "temp" in c.group(3):
            ctx[int(c.group(1)) // 1000] = "state " + c.group(2) + c.group(3)
    for s in sorted(set(ph) | {x + d for x in ph for d in (-1, 1)}):
        print(f"{s:6d}s phantom {ph.get(s, 0):3d} real {real.get(s, 0):3d} | {ctx.get(s + 1, '')}")


if __name__ == "__main__":
    main()
