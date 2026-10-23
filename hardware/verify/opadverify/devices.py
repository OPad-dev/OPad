"""Physical pin models of every part, typed in from the datasheets.

Each model gives the contact centre of every pin in the datasheet's TOP view
(X right, Y down, mm, body centre at the origin) and the pin names. The
checker never takes a pin position from a KiCad footprint: it asks whether
the footprint's pads are a rotated (not mirrored) copy of this drawing with
the same numbers. That is the check V1.0 failed.

`source` says where the numbers come from. `independent` is False for the
few parts whose model could only be confirmed by a board that was built and
works, not by a drawing read for this file; the report lists them.
"""

from dataclasses import dataclass, field


@dataclass
class Device:
    name: str
    pins: dict                    # number -> (x, y), datasheet top view
    names: dict                   # function name -> number
    source: str
    independent: bool = True
    polar: bool = True            # False: the two ends are interchangeable
    tol: float = 0.25             # mm, contact centre to pad centre
    extra_pads: tuple = ()        # pad numbers in the footprint not modelled (mechanical tabs)
    notes: str = ""


def _qfn16(pitch=0.5, c=1.3):
    pins = {}
    offs = [-0.75, -0.25, 0.25, 0.75]
    for i, y in enumerate(offs):
        pins[str(1 + i)] = (-c, y)            # left, top to bottom
    for i, x in enumerate(offs):
        pins[str(5 + i)] = (x, c)             # bottom, left to right
    for i, y in enumerate(reversed(offs)):
        pins[str(9 + i)] = (c, y)             # right, bottom to top
    for i, x in enumerate(reversed(offs)):
        pins[str(13 + i)] = (x, -c)           # top, right to left
    pins["17"] = (0.0, 0.0)
    return pins


DEVICES = {
    "DRV5055": Device(
        "TI DRV5055 (DBZ, SOT-23-3)",
        pins={"1": (-1.0, -0.95), "2": (-1.0, 0.95), "3": (1.0, 0.0)},
        names={"VCC": "1", "OUT": "2", "GND": "3"},
        source="TI SBAS640C (rev. June 2026) Figure 4-1 'DBZ Package 3-Pin SOT-23 Top View' "
               "(pin 1 VCC top-left, pin 2 OUT bottom-left, pin 3 GND right) and Table 4-1; "
               "pitch 0.95 mm, lead span 2.37 mm"),
    "ADS7038": Device(
        "TI ADS7038 (RTE, WQFN-16 3x3, exposed pad)",
        pins=_qfn16(),
        names={"AIN2": "1", "AIN3": "2", "AIN4": "3", "AIN5": "4", "AIN6": "5", "AIN7": "6",
               "AVDD": "7", "DECAP": "8", "GND": "9", "DVDD": "10", "CS": "11", "SDO": "12",
               "SCLK": "13", "SDI": "14", "AIN0": "15", "AIN1": "16", "EP": "17"},
        source="TI SBAS979C Figure 5-1 'RTE Package, 16-Pin WQFN, Top View' and Table 5-1 "
               "(also ADS7038-Q1 SBASA05A: same pin table); 0.5 mm pitch, lead centres 1.3 mm "
               "from the body centre"),
    "R0603": Device(
        "0603 two-terminal chip (resistor, capacitor, ferrite bead)",
        pins={"1": (-0.8, 0.0), "2": (0.8, 0.0)}, names={}, polar=False,
        source="EIA 0603 (1.6 x 0.8 mm), terminal centres about +-0.8 mm"),
    "TESTPOINT": Device("Bare test pad", pins={"1": (0.0, 0.0)}, names={}, polar=False,
                        source="a single pad"),
    "JST_SH_8_SMD": Device(
        "JST SM08B-SRSS-TB (SH, 1.0 mm, 8 contacts, side entry)",
        pins={str(k): (k - 4.5, 0.0) for k in range(1, 9)},
        names={str(k): str(k) for k in range(1, 9)},
        extra_pads=("MP",), independent=False,
        source="contacts in one row at 1.0 mm pitch; which end is pin 1 matches the built V1 "
               "boards, which work with a same-direction cable (JST's drawing was not "
               "readable here)",
        notes="Being replaced by FFC (decision H10)."),
    "KAILH_HOTSWAP": Device(
        "Kailh CPG151101S11 MX hot-swap socket, under the board",
        pins={"1": (7.085, -2.54), "2": (-5.842, -5.08)}, names={}, polar=False,
        independent=False,
        source="the socket's two solder tabs as in the footprint of the built MX V1, which works "
               "(Kailh's drawing was not read for this file); a switch has no pin order"),
    "PINSOCKET_1x14": Device(
        "1x14 female header, 2.54 mm",
        pins={str(k): (0.0, (k - 1) * 2.54) for k in range(1, 15)},
        names={str(k): str(k) for k in range(1, 15)}, tol=0.1, independent=False,
        source="one row at 2.54 mm; which end is pin 1 is set by the Waveshare's headers and "
               "confirmed by the built carrier V1"),
}
