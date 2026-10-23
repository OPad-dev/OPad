#!/usr/bin/env python3
"""Record what the USB bus does around HE phantom presses, until Ctrl-C.

docs/history/2026-10-22-he-phantom-presses.md: the phantoms beat at 154.07 ms
against the ADC's 4006.41 frames/s, which fits a 1 kHz source ~20 ppm off the
pad's crystal (the PC's USB frame clock) rather than anything on the pad's own
clock. This script tests that from the host, silently, in the foreground:

  - every USB packet to and from the pad (usbmon, us timestamps)   -> U lines
  - every key event the pad sends (evdev, both keyboard interfaces) -> K lines
  - opad-daemon switched on and off in alternating phases, so there are long
    windows with and without serial (CDC) traffic                  -> P lines
  - realtime/monotonic clock pairs to line the two sources up       -> T lines
  - the pad's he.* debug lines, copied from the daemon's he-debug.log
    (on phases only: nothing reads the pad with the daemon off)     -> L lines

A phantom shows up as a K press released within a few ms. The HE debug
firmware stamps each edge with its USB frame number (he.usb) and logs once a
second where a USB frame starts on its own clock (he.sof), which places every
phantom within its USB frame.

    sudo python3 scripts/he_phantom_probe.py [--phase-min 15]

Root is needed for usbmon and the input devices. The log goes to
~/.local/share/opad/he-probe-<date>.log of the user who ran sudo. Nothing is
printed; Ctrl-C stops it and leaves the daemon as it found it.
"""

import argparse
import datetime
import glob
import os
import pwd
import signal
import struct
import subprocess
import sys
import threading
import time

VID, PID = "303a", "4001"
KBD_GLOBS = ("/dev/input/by-id/usb-*OPad*-event-kbd",)
EVENT = struct.Struct("llHHi")  # struct input_event on 64-bit Linux
EV_KEY = 1


class Log:
    def __init__(self, path, uid, gid):
        self.f = open(path, "a", buffering=1)
        os.chown(path, uid, gid)
        self.lock = threading.Lock()

    def write(self, kind, text):
        with self.lock:
            self.f.write(f"{kind} {text}\n")


def find_pad():
    """(bus, dev) of the pad from sysfs, or None."""
    for d in glob.glob("/sys/bus/usb/devices/*"):
        try:
            with open(d + "/idVendor") as v, open(d + "/idProduct") as p:
                if v.read().strip() == VID and p.read().strip() == PID:
                    with open(d + "/busnum") as b, open(d + "/devnum") as n:
                        return int(b.read()), int(n.read())
        except OSError:
            continue
    return None


class Pad:
    def __init__(self):
        self.lock = threading.Lock()
        self.where = None

    def get(self):
        with self.lock:
            return self.where

    def set(self, where):
        with self.lock:
            changed = where != self.where
            self.where = where
        return changed


def usbmon_reader(log, pad):
    while True:
        where = pad.get()
        if where is None:
            time.sleep(1)
            continue
        bus, dev = where
        try:
            with open(f"/sys/kernel/debug/usb/usbmon/{bus}u") as f:
                for line in f:
                    parts = line.split(None, 4)
                    if len(parts) < 4:
                        continue
                    addr = parts[3].split(":")  # e.g. Bi:1:127:3
                    if len(addr) == 4 and int(addr[2]) == dev and int(addr[1]) == bus:
                        log.write("U", line.rstrip("\n").split(None, 1)[1])
                    if pad.get() != where:
                        break
        except (OSError, ValueError) as e:
            log.write("E", f"usbmon bus {bus}: {e}")
            time.sleep(2)


def evdev_reader(log, pattern):
    while True:
        paths = sorted(glob.glob(pattern))
        if not paths:
            time.sleep(1)
            continue
        # One thread per interface: the pad may send key 2 through either
        threads = [threading.Thread(target=evdev_one, args=(log, p), daemon=True) for p in paths]
        for t in threads:
            t.start()
        for t in threads:
            t.join()


def evdev_one(log, path):
    name = "if03" if "-if03-" in path else "if00"
    try:
        with open(path, "rb", buffering=0) as f:
            log.write("D", f"evdev {name} open {os.path.realpath(path)}")
            while True:
                data = f.read(EVENT.size)
                if len(data) < EVENT.size:
                    break
                sec, usec, typ, code, value = EVENT.unpack(data)
                if typ == EV_KEY:
                    log.write("K", f"{name} rt_us={sec * 1000000 + usec} code={code} value={value}")
    except OSError as e:
        log.write("D", f"evdev {name} closed: {e}")
    time.sleep(1)


def pad_log_reader(log, path):
    """Copy new he.* lines from the daemon's log, following its rotation."""
    f, pos = None, 0
    while True:
        try:
            if f is None:
                f = open(path, errors="replace")
                f.seek(0, os.SEEK_END)
                pos = f.tell()
            line = f.readline()
            if line.endswith("\n"):
                pos = f.tell()
                if " he." in line:
                    log.write("L", line.rstrip("\n"))
                continue
            f.seek(pos)  # a partial line: read it again once complete
            if os.path.getsize(path) < pos:
                f.close()
                f = open(path, errors="replace")  # rotated: the new file from its start
                pos = 0
        except OSError:
            if f is not None:
                f.close()
            f = None
        time.sleep(0.2)


def as_user(user, *args):
    pw = pwd.getpwnam(user)
    env = {
        "XDG_RUNTIME_DIR": f"/run/user/{pw.pw_uid}",
        "DBUS_SESSION_BUS_ADDRESS": f"unix:path=/run/user/{pw.pw_uid}/bus",
        "PATH": "/usr/bin:/bin",
        "HOME": pw.pw_dir,
    }
    return subprocess.run(["runuser", "-u", user, "--", *args], env=env,
                          stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)


def daemon(user, action):
    return as_user(user, "systemctl", "--user", action, "opad-daemon").stdout.strip()


def clock_pair(log):
    rt = time.clock_gettime_ns(time.CLOCK_REALTIME) // 1000
    mono = time.clock_gettime_ns(time.CLOCK_MONOTONIC) // 1000
    log.write("T", f"rt_us={rt} mono_us={mono}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--phase-min", type=float, default=15.0)
    args = ap.parse_args()

    user = os.environ.get("SUDO_USER")
    if os.geteuid() != 0 or not user:
        sys.exit("run with sudo")
    pw = pwd.getpwnam(user)
    out_dir = os.path.join(pw.pw_dir, ".local/share/opad")
    path = os.path.join(out_dir, f"he-probe-{datetime.datetime.now():%Y-%m-%d-%H%M}.log")
    log = Log(path, pw.pw_uid, pw.pw_gid)

    subprocess.run(["modprobe", "usbmon"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    was_active = daemon(user, "is-active") == "active"
    log.write("H", f"start {datetime.datetime.now().isoformat()} phase_min={args.phase_min} "
                   f"daemon_was_active={was_active} usbmon_ts=(mono_s&0xFFF)*1e6+us evdev_ts=realtime")

    pad = Pad()
    threading.Thread(target=usbmon_reader, args=(log, pad), daemon=True).start()
    threading.Thread(target=pad_log_reader, args=(log, os.path.join(out_dir, "he-debug.log")),
                     daemon=True).start()
    for pattern in KBD_GLOBS:
        threading.Thread(target=evdev_reader, args=(log, pattern), daemon=True).start()

    # Ctrl-C ends the loop below; restoring the daemon happens in finally
    stop = threading.Event()
    signal.signal(signal.SIGINT, lambda *_: stop.set())
    signal.signal(signal.SIGTERM, lambda *_: stop.set())

    phase_s = args.phase_min * 60
    want_on = was_active  # first phase keeps the daemon as found
    phase_end = 0.0
    next_clock = 0.0
    try:
        while not stop.is_set():
            now = time.monotonic()
            where = find_pad()
            if pad.set(where):
                log.write("D", f"pad {'bus %d dev %d' % where if where else 'absent'}")
            if now >= phase_end:
                if phase_end:
                    want_on = not want_on
                phase_end = now + phase_s
                daemon(user, "start" if want_on else "stop")
                log.write("P", f"daemon {'on' if want_on else 'off'} rt_us={time.time_ns() // 1000}")
            elif int(now) % 5 == 0:
                # Something else (the GUI) may have changed it: put it back
                active = daemon(user, "is-active") == "active"
                if active != want_on:
                    log.write("P", f"daemon found {'on' if active else 'off'}, setting it back")
                    daemon(user, "start" if want_on else "stop")
            if now >= next_clock:
                clock_pair(log)
                next_clock = now + 10
            stop.wait(1)
    finally:
        daemon(user, "start" if was_active else "stop")
        log.write("H", f"stop {datetime.datetime.now().isoformat()} daemon restored")
        log.f.close()
    os._exit(0)  # reader threads block in read(); don't wait for them


if __name__ == "__main__":
    main()
