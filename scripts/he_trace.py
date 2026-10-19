#!/usr/bin/env python3
"""Record and analyse the raw Hall Effect stream (CONFIG_OSUPAD_HE_TRACE).

For OPEN A/D in docs/specs/v2-hall-effect-decisions.md: the noise every raw
conversion carries, and the rapid-trigger sensitivity a still key survives.
If opad-daemon is running, the port is borrowed from it the way
scripts/hall_bench.py does, and given back on exit.

    scripts/he_trace.py record /dev/ttyACM0 60 rest.bin
    scripts/he_trace.py analyse rest.bin

`record` writes the stream as received and then analyses it. Record each
condition separately, keys untouched or held still for the whole recording
(the analysis assumes nothing moves): at rest, each key held part way down,
a long rest run. `analyse` builds firmware/tools/he_replay.c, which replays
the trace through the firmware's own decision engine. Needs pyserial and gcc.
"""
import os
import subprocess
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
FW = os.path.join(HERE, "..", "firmware")


def record(port, seconds, out):
    import serial
    sys.path.insert(0, HERE)
    from hall_bench import pause_daemon

    daemon = pause_daemon()
    total = 0
    with serial.Serial(port, 115200, timeout=0.2) as s, open(out, "wb") as f:
        s.reset_input_buffer()
        end = time.time() + seconds
        while time.time() < end:
            chunk = s.read(65536)
            f.write(chunk)
            total += len(chunk)
    if daemon:
        daemon.close()
    print(f"{total} bytes in {seconds:g} s ({total / seconds / 1000:.0f} kB/s; a full stream is ~153 kB/s)")
    if total == 0:
        sys.exit("Nothing received: is this a CONFIG_OSUPAD_HE_TRACE build with the HE module fitted?")


def analyse(path):
    with tempfile.TemporaryDirectory() as tmp:
        exe = os.path.join(tmp, "he_replay")
        subprocess.run(["gcc", "-O2", "-Wall", "-Wextra", "-Werror", "-I", os.path.join(FW, "main"),
                        os.path.join(FW, "tools", "he_replay.c"),
                        os.path.join(FW, "main", "input", "he_rt.c"),
                        "-o", exe, "-lm"], check=True)
        subprocess.run([exe, path], check=True)


def main():
    if len(sys.argv) == 5 and sys.argv[1] == "record":
        record(sys.argv[2], float(sys.argv[3]), sys.argv[4])
        analyse(sys.argv[4])
    elif len(sys.argv) == 3 and sys.argv[1] in ("analyse", "analyze"):
        analyse(sys.argv[2])
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
