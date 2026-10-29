"""Follow every module pin, and the USB board's jack, through its FFC (both ways round) and the carrier
to the ESP32.

The boards are checked one by one by opadverify against their intended
netlists; this checks that the intended netlists fit together:

- power and ground land on the same supply whichever way the cable is in
  (a reversed FFC maps contact k to 15 - k);
- every module signal reaches its own ESP32 GPIO through the carrier, and the
  ID always lands on an ADC1 pin (the firmware reads it on GPIO8, or on GPIO7
  with a reversed cable, then swaps the other pins in the GPIO matrix);
- the USB jack's D+ and D- reach GPIO20 and GPIO19 (the ESP32-S3's USB), and
  its filtered 5 V the Waveshare's 5 V pin, with the 8-way cable either way
  round (a reversed cable maps contact k to 9 - k);
- the module ID voltages, with 1 % resistors, a 3 % 3V3 and the ESP32-S3 ADC's
  error, stay clear of the firmware's cut-offs.

Run: .tools/venv/bin/python hardware/verify/tests/test_system.py
"""

import os
import sys
import tomllib

HERE = os.path.dirname(os.path.abspath(__file__))
GOLDEN = os.path.join(os.path.dirname(HERE), "golden")
sys.path.insert(0, os.path.dirname(HERE))
from opadverify import devices as D  # noqa: E402

# Waveshare headers (AGENTS.md section 6): pin -> GPIO number or supply
WAVESHARE = {
    "J_P1": {1: 2, 2: 4, 3: 6, 4: 16, 5: 17, 6: 18, 7: 21, 8: 8, 9: 7, 10: 10, 11: 20, 12: 19, 13: "GND", 14: "5V"},
    "J_P2": {1: "3V3", 2: "GND", 3: 43, 4: 44, 5: 47, 6: 48, 7: 15, 8: 13, 9: 11, 10: 12, 11: 14, 12: 9,
             13: "GND", 14: "VBAT"},
}
ADC1 = set(range(1, 11))          # ESP32-S3 ADC1 channels are GPIO1-GPIO10
SERIES = ("R", "FB")              # series parts a signal passes through on the carrier (33R, ferrites)

# Module ID classes and the firmware cut-offs between them (brief 4.2)
CUTS = [0.12, 0.68, 1.35, 1.95, 2.6]


def load(name):
    return tomllib.load(open(os.path.join(GOLDEN, name + ".toml"), "rb"))["part"]


def pin_nets(spec):
    dev = D.DEVICES[spec["device"]]
    if "pins" in spec:
        return {dev.names.get(k, k): v for k, v in spec["pins"].items()}
    return {"1": spec["nets"][0], "2": spec["nets"][1]}


def carrier_lines(parts):
    """Carrier net -> what it reaches at the Waveshare: a GPIO number or a supply name."""
    # union nets joined by series parts
    parent = {}

    def find(n):
        while parent.setdefault(n, n) != n:
            n = parent[n]
        return n

    for ref, spec in parts.items():
        if ref.startswith(SERIES) and "nets" in spec and spec["nets"][0] != "GND" and spec["nets"][1] != "GND":
            parent[find(spec["nets"][0])] = find(spec["nets"][1])
    reach = {}
    for ref, table in WAVESHARE.items():
        for pin, net in pin_nets(parts[ref]).items():
            if net != "NC":
                reach.setdefault(find(net), set()).add(table[int(pin)])
    return {net: reach.get(find(net), set()) for net in {n for s in parts.values() for n in pin_nets(s).values()}}


def main():
    carrier = load("carrier_v3")
    lines = carrier_lines(carrier)
    jmod = {int(k): v for k, v in pin_nets(carrier["J_MOD"]).items()}
    failures = 0

    def fail(msg):
        nonlocal failures
        failures += 1
        print("FAIL  " + msg)

    for module in ("mx_v3", "he_v3"):
        parts = load(module)
        j1 = {int(k): v for k, v in pin_nets(parts["J1"]).items()}
        for cable, mapping in (("straight", lambda k: k), ("reversed", lambda k: 15 - k)):
            used = {}
            for k, net in sorted(j1.items()):
                c = jmod[mapping(k)]
                reach = lines.get(c, set())
                gpios = {r for r in reach if isinstance(r, int)}
                supplies = {r for r in reach if isinstance(r, str)}
                if net == "NC":
                    continue
                if net in ("GND", "3V3", "5V"):
                    want = {"5V": {"5V"}, "3V3": {"3V3"}, "GND": {"GND"}}[net]
                    if supplies != want or gpios:
                        fail("%s, %s cable: module %s (contact %d) meets carrier %s %s"
                             % (module, cable, net, k, c, sorted(map(str, reach))))
                    continue
                if len(gpios) != 1 or supplies:
                    fail("%s, %s cable: module %s (contact %d) meets carrier %s %s, not one GPIO"
                         % (module, cable, net, k, c, sorted(map(str, reach))))
                    continue
                g = gpios.pop()
                if g in used.values():
                    fail("%s, %s cable: GPIO%d gets two module signals" % (module, cable, g))
                used[net] = g
                if net == "ID" and g not in ADC1:
                    fail("%s, %s cable: ID lands on GPIO%d, not an ADC1 pin" % (module, cable, g))
            print("ok    %-6s %-8s cable: %s" % (module, cable, ", ".join("%s=GPIO%d" % kv for kv in sorted(used.items()))))

    # The USB board: its jack, through the 8-way cable and the carrier, to the ESP32's USB pins
    usb = load("usb_v3")
    jusb = {int(k): v for k, v in pin_nets(carrier["J_USB"]).items()}
    j2 = {int(k): v for k, v in pin_nets(usb["J2"]).items()}
    jack = {v for k, v in pin_nets(usb["J1"]).items()}
    want_usb = {"USB_DP": {20}, "USB_DN": {19}, "5V": {"5V"}, "GND": {"GND"}}
    for cable, mapping in (("straight", lambda k: k), ("reversed", lambda k: 9 - k)):
        seen, before = {}, failures
        for k, net in sorted(j2.items()):
            c = jusb[mapping(k)]
            reach = lines.get(c, set())
            if reach != want_usb.get(net):
                fail("usb_v3, %s cable: USB board %s (contact %d) meets carrier %s %s"
                     % (cable, net, k, c, sorted(map(str, reach))))
            seen[net] = reach
        for net in ("USB_DP", "USB_DN"):
            if net not in jack:
                fail("usb_v3: the jack has no %s" % net)
        if failures == before:
            print("ok    usb_v3 %-8s cable: %s" % (cable, ", ".join(
                "%s=%s" % (n, "/".join("GPIO%d" % r if isinstance(r, int) else r for r in sorted(seen[n], key=str)))
                for n in ("USB_DP", "USB_DN", "5V", "GND"))))

    # ID voltages against the cut-offs: 1 % resistors, 3V3 +-3 %, ADC +-50 mV (worst case)
    for name, top, bottom in (("MX V3", 47e3, 100e3), ("HE V3", 100e3, 100e3)):
        lo = 3.3 * 0.97 * bottom * 0.99 / (bottom * 0.99 + top * 1.01) - 0.05
        hi = 3.3 * 1.03 * bottom * 1.01 / (bottom * 1.01 + top * 0.99) + 0.05
        inside = [c for c in CUTS if lo <= c <= hi]
        margin = min(min(abs(lo - c), abs(hi - c)) for c in CUTS)
        if inside:
            fail("%s ID spans %.2f-%.2f V, across the cut-off %s" % (name, lo, hi, inside))
        else:
            print("ok    %s ID %.2f-%.2f V worst case, %.2f V from the nearest cut-off" % (name, lo, hi, margin))

    print("%d failure(s)" % failures)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
