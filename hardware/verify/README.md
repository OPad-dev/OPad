# Board checker (`opadverify`)

Checks a board's **fab files** (the Gerbers, drill files, CPL and BOM that go to
JLCPCB) against a **hand-written intended netlist** and **pin drawings typed in from
the datasheets**. It does not trust KiCad: connectivity is rebuilt from the Gerber
copper and the plated holes, and every footprint is compared with the datasheet drawing.

It exists because HE V1.0 shipped with a mirrored SOT-23 footprint. Its
connectivity was right, so ERC, DRC and the assembler all passed it
(`hardware/pcb/V1/README.md`, V1.0 erratum). This checker fails V1.0 for that reason.

## Run it

```
python3 -m venv .tools/venv && .tools/venv/bin/pip install -r hardware/verify/requirements.txt
cd hardware/verify
../../.tools/venv/bin/python -m opadverify golden/he_v2.toml        # -v lists every pass too
../../.tools/venv/bin/python tests/test_mutations.py                 # proves each check can fail
../../.tools/venv/bin/python tests/test_system.py                    # the boards fit together through the cable
```

Exit status 0 means no ERROR.

## What it checks

| Check | Catches |
|---|---|
| parts | a part on the board missing from the intended netlist, or the other way round |
| handedness | a footprint whose pads are a **mirror image** of the datasheet's top view (V1.0), or do not match it at all |
| landing | a pin with no copper, no solder-mask opening, no paste, or no plated hole under it |
| nets | **OPEN** (one intended net in several pieces of copper), **SHORT** (two nets on one piece), NC pins touching anything |
| copper | copper that touches no pin (floating islands) |
| spacing | gaps between different nets below the fab minimum (0.1 mm, error) or the design rule (0.15 mm, warning); copper closer than 0.2 mm to the board edge |
| assembly | CPL position, side and rotation against the footprint (including the `JLC_CPL_FIXES` offsets); BOM LCSC number against the intended one; the LCSC number's maker part number and stock in JLCPCB's catalogue |
| jlc | **JLCPCB's own footprint** of each LCSC part (from LCSC's API), placed where the CPL says, puts every pin on our pad for that pin (`jlc.py`) |
| holes | mounting and switch-peg holes at the intended position and size |

The pin models (`opadverify/devices.py`) say where their numbers come from.
Three are confirmed only by built, working boards, not by a drawing: the JST SH
connector, the Kailh hot-swap socket and the 1×14 socket. `-v` lists them.

## JLCPCB's placement

JLCPCB never sees our footprints. It places the footprint from its own
EasyEDA/LCSC library at the CPL's position and rotation. `jlc.py` fetches that
footprint and does the same. On the bottom side the model is mirrored left-right
and then rotated. The evidence is HE V2's U4: at CPL 0°, JLCPCB's preview showed
pin 1 in the corner diagonally opposite ours, and this model predicts exactly
that (the test cases check it both ways). Which way a bottom-side rotation
turns is not established, so a polarised bottom part at 90° or 270° gets a warning.

Run on the V1 boards, it finds that JLCPCB's models of the JST connector, the
DRV5055 and the Kailh socket do **not** land on our pads with the CPLs as
written (different zero orientation or origin in their library). Those boards
work, so those parts were turned in the preview at order time (the V1 README
tells the buyer to do so), or the convention above is wrong for them.
New boards are exported so that the check passes with no turning in the preview.

## What it cannot check

- **What JLCPCB's engineers change** during their review. Compare the preview
  with the positions the checker prints (`-v`) before paying.
- **Whether the intended netlist is the right circuit.** It checks the board against
  the intent. The intent itself (in `golden/`) has to be read against the datasheets.

## Results (2026-10-23)

| Board | Result |
|---|---|
| HE V1.0 (from git, `07d7414^`) | **fails**: U1 and U2 footprints are the mirror image of the DRV5055 |
| HE V1.1 (built, works) | passes; one accepted finding: a 0.25 mm copper strip past the rear edge at J1 |
| MX V1 (built, works) | passes; warns about two 0.19 mm² floating copper slivers |
| Carrier V1 (built, works) | passes |
| HE V2 (not built) | passes |
| Carrier V3, MX V3, HE V3 (not built) | pass (one warning each on HE and carrier: no LCSC footprint for the 0603 ferrite) |
| `tests/test_system.py` | the module cable both ways round, from every module pin to its ESP32 pin, and the ID margins: pass |
| `tests/test_mutations.py` on HE V2 | 12 of 12 cases behave: 10 deliberate faults caught (cut track, short, missing vias, mirrored footprint, swapped SPI pins, ADC rotated in the CPL, no mask opening, wrong LCSC number, missing hole) and JLCPCB's ADC model lands at 180° but not at 0° |

With the JLCPCB check on, every V1 board and HE V2 report their JST connector
(and HE: DRV5055, MX: Kailh socket) as misplaced by JLCPCB's model, as described above.
