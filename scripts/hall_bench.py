#!/usr/bin/env python3
"""Record and summarise the Hall bench stream (CONFIG_OSUPAD_HALL_BENCH).

V2-0 of docs/specs/v2-rapid-trigger.md. If opad-daemon is running, the script
asks it to release the port (the PrepareFlash request `opadctl flash` uses) and
holds that connection open; the daemon takes the port back when it closes.

    scripts/hall_bench.py /dev/ttyACM0 10 rest.csv

Writes the 1 ms averages as CSV (ms,key1,key2) and prints, per key, the range
of the 1 ms averages and the spread of the raw conversions the pad reported.
Needs pyserial.
"""
import json
import os
import socket
import statistics
import struct
import sys
import time

import serial


def _ipc(sock, req):
    data = json.dumps(req).encode()
    sock.sendall(struct.pack("<I", len(data)) + data)
    hdr = b""
    while len(hdr) < 4:
        hdr += sock.recv(4 - len(hdr))
    n, buf = struct.unpack("<I", hdr)[0], b""
    while len(buf) < n:
        buf += sock.recv(n - len(buf))
    return json.loads(buf)


def pause_daemon():
    """Ask a running opad-daemon for the port; returns the socket to keep open."""
    run = os.environ.get("XDG_RUNTIME_DIR")
    path = os.path.join(run, "opad", "daemon.sock") if run else f"/tmp/opad-{os.getuid()}/daemon.sock"
    try:
        sock = socket.socket(socket.AF_UNIX)
        sock.connect(path)
    except OSError:
        return None  # no daemon
    ack = _ipc(sock, {"Handshake": {"client_version": "1.0.0-rc", "client_protocol": 1}})
    if "HandshakeAck" not in ack:
        sys.exit(f"daemon refused the handshake: {ack}")
    resp = _ipc(sock, "PrepareFlash")
    if "ReadyForFlash" not in resp:
        sys.exit(f"daemon would not release the port: {resp}")
    time.sleep(0.3)
    return sock


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    port, seconds = sys.argv[1], float(sys.argv[2])
    out = sys.argv[3] if len(sys.argv) > 3 else None

    daemon = pause_daemon()
    rows, spreads = [], {1: [], 2: [], 3: [], 4: []}
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
    for n, lo, hi, mean, sd100, ovf in spreads[3][-3:]:
        print(f"ID raw/s: n {n} min {lo} max {hi} mean {mean} sd {sd100 / 100:.2f}")


if __name__ == "__main__":
    main()
