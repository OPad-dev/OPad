#!/usr/bin/env python3
"""Record and summarise the Hall bench stream (CONFIG_OSUPAD_HALL_BENCH).

V2-0 of docs/specs/v2-rapid-trigger.md. Stop opad-daemon first: it holds the
port and does not understand this stream.

    systemctl --user stop opad-daemon
    scripts/hall_bench.py /dev/ttyACM0 10 rest.csv

Writes the 1 ms averages as CSV (ms,key1,key2) and prints, per key, the range
of the 1 ms averages and the spread of the raw conversions the pad reported.
Needs pyserial.
"""
import statistics
import sys
import time

import serial


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    port, seconds = sys.argv[1], float(sys.argv[2])
    out = sys.argv[3] if len(sys.argv) > 3 else None

    rows, spreads = [], {1: [], 2: []}
    with serial.Serial(port, 115200, timeout=0.2) as s:
        s.reset_input_buffer()
        end = time.time() + seconds
        buf = b""
        while time.time() < end:
            buf += s.read(4096)
            *lines, buf = buf.split(b"\n")
            for raw in lines:
                f = raw.strip().split(b",")
                try:
                    if f[0] == b"H" and len(f) == 4:
                        rows.append(tuple(int(x) for x in f[1:]))
                    elif f[0] == b"S" and len(f) == 8:
                        spreads[int(f[1])].append([int(x) for x in f[2:]])
                except ValueError:
                    pass  # a protocol frame or a torn line

    if out:
        with open(out, "w") as fh:
            fh.write("ms,key1,key2\n")
            fh.writelines(f"{t},{a},{b}\n" for t, a, b in rows)

    if not rows:
        sys.exit("No H lines: is this a CONFIG_OSUPAD_HALL_BENCH build, and the daemon stopped?")
    span_ms = rows[-1][0] - rows[0][0]
    print(f"{len(rows)} averaged samples over {span_ms} ms ({len(rows) / max(span_ms, 1) * 1000:.0f} per s)")
    for k in (1, 2):
        v = [r[k] for r in rows]
        print(f"key{k} 1ms-avg: min {min(v)} max {max(v)} mean {statistics.mean(v):.1f} "
              f"p-p {max(v) - min(v)} sd {statistics.pstdev(v):.2f}")
        for n, lo, hi, mean, sd100, ovf in spreads[k][-3:]:
            print(f"  raw/s: n {n} min {lo} max {hi} mean {mean} sd {sd100 / 100:.2f} overflows {ovf}")


if __name__ == "__main__":
    main()
