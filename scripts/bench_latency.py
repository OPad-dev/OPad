#!/usr/bin/env python3
"""
OPad Latency & Jitter Measurement Tool (§33)
Measures interval regularity, polling consistency, and jitter on USB HID key events.
"""

import sys
import time
import os
import glob
import struct
import math

def find_osupad_event_device():
    # Search /dev/input/by-id or by-path
    matches = glob.glob("/dev/input/by-id/*OPad*") + glob.glob("/dev/input/by-id/*osu*")
    if matches:
        return matches[0]
    
    # Fallback to search through /sys/class/input/event*/device/name
    for event_path in glob.glob("/sys/class/input/event*"):
        name_file = os.path.join(event_path, "device", "name")
        if os.path.exists(name_file):
            try:
                with open(name_file, "r") as f:
                    name = f.read().strip()
                    if "OPad" in name or "GFerreiroS" in name:
                        event_node = "/dev/input/" + os.path.basename(event_path)
                        return event_node
            except Exception:
                pass
    return None

def main():
    print("=== OPad Latency & Jitter Benchmark ===")
    device_node = find_osupad_event_device()
    if not device_node:
        print("Note: OPad input event device node not found under /dev/input/by-id.")
        print("Available event nodes:")
        for node in sorted(glob.glob("/dev/input/event*")):
            print(f"  {node}")
        if len(sys.argv) > 1:
            device_node = sys.argv[1]
        else:
            print("Usage: python3 bench_latency.py [/dev/input/eventX]")
            return

    print(f"Sampling events from: {device_node}")
    print("Press Key 1 or Key 2 rapidly for 5-10 seconds to sample polling interval...")

    # Linux input_event struct: timeval (16 bytes on 64-bit), type (2 bytes), code (2 bytes), value (4 bytes) = 24 bytes
    EVENT_FORMAT = "llHHi"
    EVENT_SIZE = struct.calcsize(EVENT_FORMAT)

    intervals_ms = []
    last_time = None

    try:
        with open(device_node, "rb") as f:
            while len(intervals_ms) < 100:
                data = f.read(EVENT_SIZE)
                if not data:
                    break
                sec, usec, ev_type, code, value = struct.unpack(EVENT_FORMAT, data)
                
                # EV_KEY = 1
                if ev_type == 1:
                    timestamp = sec + (usec / 1000000.0)
                    if last_time is not None:
                        delta_ms = (timestamp - last_time) * 1000.0
                        if delta_ms < 50.0: # Filter out long pauses between taps
                            intervals_ms.append(delta_ms)
                            print(f"  Sample #{len(intervals_ms)}: {delta_ms:0.3f} ms (val={value})")
                    last_time = timestamp
    except PermissionError:
        print(f"Permission denied reading {device_node}. Run with sudo or ensure user is in input group.")
        return
    except KeyboardInterrupt:
        pass

    if intervals_ms:
        n = len(intervals_ms)
        mean = sum(intervals_ms) / n
        variance = sum((x - mean) ** 2 for x in intervals_ms) / n
        std_dev = math.sqrt(variance)
        min_val = min(intervals_ms)
        max_val = max(intervals_ms)

        print("\n--- Latency Benchmark Results ---")
        print(f"Total Samples:      {n}")
        print(f"Mean Interval:      {mean:0.3f} ms")
        print(f"Min Interval:       {min_val:0.3f} ms")
        print(f"Max Interval:       {max_val:0.3f} ms")
        print(f"Standard Deviation: {std_dev:0.3f} ms (Target < 0.25 ms)")
        if max_val <= 2.5:
            print("✓ PASS: 1000 Hz USB polling and debounce jitter within specification.")
        else:
            print("⚠ NOTICE: Max interval exceeded 2.5 ms, check debounce lockout settings.")
    else:
        print("No samples recorded.")

if __name__ == "__main__":
    main()
