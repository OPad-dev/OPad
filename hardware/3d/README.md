# osu!pad ESP32-S3 — 3D Models & Enclosure CAD Reference

This directory contains 3D CAD models, official hardware blueprints, and ready-to-print/modify enclosure files for the **Waveshare ESP32-S3-Touch-LCD-2** and standard **MX mechanical switches**.

---

## 1. Directory Structure

```
hardware/3d/
├── waveshare_board/
│   ├── esp32-s3-touch-lcd-2_20241108.stp   # Official 15.2 MB 3D STEP solid model from Waveshare
│   ├── ESP32-S3-Touch-LCD-2-20241108.pdf   # Official mechanical dimension drawing
│   ├── ESP32-S3-Touch-LCD-2-20241108.dwg   # Official AutoCAD / CAD layout drawing
│   └── dims-1.png                          # High-resolution 2D mechanical blueprint image
│
├── reference_keypads/
│   ├── milkcrate-plate-mount.step          # Open-source 2-key MX switch plate mount (STEP)
│   ├── milkcrate-plate-mount.stl           # 2-key MX switch plate (STL)
│   ├── milkcrate-pcb-mount.step            # 2-key MX switch base mount (STEP)
│   ├── milkcrate-pcb-mount.stl             # 2-key MX switch base mount (STL)
│   ├── osu_keypad_clay53.FCStd             # 2-key osu! keypad native FreeCAD parametric project
│   ├── osu_case_top_kamehameha.stl         # Printables #943460 2-key case top (STL)
│   └── osu_case_bottom_kamehameha.stl      # Printables #943460 2-key case bottom (STL)
│
└── custom_case/
    ├── V1/                                 # Original enclosure (~22° screen deck, recessed glass pocket)
    │   ├── osupad_enclosure.scad
    │   ├── osupad_case_top.stl
    │   ├── osupad_case_bottom.stl
    │   └── osupad_preview.png
    └── V2/                                 # Current enclosure (lower, 30° deck, glass on top, ballast + grip base)
        ├── osupad_enclosure.scad
        ├── osupad_case_top.stl
        ├── osupad_case_bottom.stl
        └── osupad_preview.png
```

V1 and V2 parts are **not interchangeable**: print the top case and bottom plate from the same folder.

---

## 2. Hardware Dimensions Reference

### Waveshare ESP32-S3-Touch-LCD-2
* **Outer Glass Lens**: `37.10 mm (W) × 58.80 mm (L) × 1.10 mm (H)` with `4× R2.60 mm` corner fillet (drawing). The STEP model's glass edge measures `37.52 × 59.22 mm`, so a pocket needs more clearance than the drawing suggests.
* **Under the glass**: touch layer, LCD and PCB all fit inside `35.02 × 48.22 mm`; the USB-C receptacle sticks out `1.18 mm` past the PCB edge
* **Active Display Viewport**: `30.99 mm (W) × 41.20 mm (L)`
* **PCB Substrate**: `35.00 mm (W) × 48.20 mm (L)`
* **Mounting Holes**: `4× M2.0` screw holes (pitch: `29.00 mm (X) × 42.20 mm (Y)`)
* **Total Module Depth**: `9.70 mm` (from glass front to back of PCB connectors)
* **USB-C Port**: Centered on bottom edge (`18.9 mm` cutout width)

### Mechanical MX Key Switches (Cherry, Gateron, Kailh)
* **Plate Cutout**: `14.0 mm × 14.0 mm`
* **Plate Thickness**: `1.5 mm` (critical for clip retention)
* **Standard Key Pitch**: `19.05 mm` center-to-center

---

## 3. How to Use & Modify These Models

### Option A: Edit the STEP Models in FreeCAD or Fusion 360 (Recommended)
1. Open [`hardware/3d/reference_keypads/milkcrate-plate-mount.step`](reference_keypads/milkcrate-plate-mount.step) in **FreeCAD**, **Fusion 360**, or **Onshape**.
2. Import the official board model [`hardware/3d/waveshare_board/esp32-s3-touch-lcd-2_20241108.stp`](waveshare_board/esp32-s3-touch-lcd-2_20241108.stp).
3. Align the board above the 2 switch cutouts and perform a boolean cut or extrude the perimeter walls to create an integrated enclosure.
4. Export as `.stl` or `.step` for slicing.

### Option B: Use OpenSCAD Parametric Generator
1. Open [`hardware/3d/custom_case/V2/osupad_enclosure.scad`](custom_case/V2/osupad_enclosure.scad) in [OpenSCAD](https://openscad.org/).
2. Change `PART = "top_case";` or `PART = "bottom_plate";`.
3. Press `F6` to render, then `F7` to export directly to `.stl`.
4. All dimensions, wall thicknesses, and tolerances are fully configurable as variables at the top of the file.

---

## 4. V2 Top Case: Lower Profile, 30° Screen, Glass on Top

| | V1 | V2 |
|---|---|---|
| Key plate height above desk | 17 mm | **13 mm** |
| Screen angle | ~22° | **30°** |
| Total height | 37 mm | **34.8 mm** |
| Glass | recessed pocket | **rests on the deck** |

- **Lower keys:** the key deck is set by the switches. Under the plate top sits either the MX body plus pins (hand wired), or a hot-swap PCB with sockets, both about 8.5 mm. That leaves about 2.5 mm above the base. See §6.
- **Lower screen:** the glass starts right behind the keys. How low it can go is set by the board's header pins: their tips end 1.6 mm above the base. The slope stops just behind the glass and continues as a flat rear shelf. Keep the pins in mind if you ever desolder the headers: without them the screen could sit much lower.
- **Glass on top:**
  - V1 recessed the glass into a pocket only ~0.1 mm larger than the real glass, so the black border landed on the pocket edge. V2 has no pocket.
  - The module body drops through a cutout sized from the Waveshare STEP model (0.4 mm clearance per side), and the glass rests on the deck around it: about 4 mm at each short end, 0.6 mm along each long side.
- **Securing the screen:** nothing clamps it. Stick the black border down with thin double-sided tape (0.2–0.5 mm, e.g. VHB). The V1 support pillars are gone, because they would push the glass off the deck.
- **USB-C port:** on the receptacle's axis and tilted with the board. The hole clears a 12.5 × 7 mm plug body.
- **Inner ceiling:** follows the outer surface at a constant deck thickness (2.8 mm on the slope, 3.0 mm on the key deck).
- **Fit checks:** done against the Waveshare STEP model, MX switch bodies and the reserved key-PCB space (below). No collisions. The glass rests on the deck, and the tightest gap is 1.4 mm from the header pins to the bottom plate.
- **Parameters:** `H_FLAT`, `DECK_TILT`, `SCREEN_Y` and `SCREEN_REAR_MARGIN` drive the whole shape. After changing them, re-check the tray positions.

## 5. V2 Bottom Plate: Ballast Trays & Deskmat Grip

The outer dimensions are unchanged (76 × 86 mm footprint, 2.0 mm base, same snap fit).

### Ballast trays
Six open-top trays sit on the inside of the bottom plate, placed only where nothing else lives: clear of the switches, the reserved key-PCB space, the LCD board with its pin headers, the USB-C plug path, and the flexing snap tabs. The top case keeps at least 1.2 mm of clearance above every tray.

| Tray | Inner size (mm) | Notes |
|---|---|---|
| Front-left / front-right corners | 8.8 × 17.0 × 6.8 | Beside the switches, outside the key-PCB space |
| Band behind the keys | 61.0 × 5.6 × 6.8 | Ends before the board's front header pins |
| Left side of the screen board | 4.1 × 34.8 × 10 | |
| Right side | 3.5 × 18.3 × 10 | Behind the USB-C plug path |
| Rear bank under the shelf | 61.0 × 7.3 × 14 | M8 nuts fit standing on edge |

Total inner volume is about **12.7 cm³**. Approximate added weight when filled:

| Fill | Added weight |
|---|---|
| Steel BBs set in epoxy | ~65 g |
| Lead shot set in epoxy | ~90 g |
| Tungsten putty | ~125 g |

Always fix the ballast in place (epoxy, glue, or putty). Loose metal rattles and can short the board. Keep the fill at or below the tray rim.

### Anti-slip tread
- **Tread:** the bottom face has a diagonal diamond knurl (3 mm pitch, grooves 1.2 mm wide and 0.8 mm deep). The small sharp diamond edges bite into cloth and hybrid deskmats.
- **Printing:** the grooves narrow by one step per 0.2 mm layer, so the plate still prints flat on the bed with no supports. Print it at **0.2 mm layer height** so the steps line up. Keep the first layer well tuned so the grooves don't close up.
- **Pads:** the four Ø10 × 1.0 mm recesses are kept for optional silicone or rubber pads. 1 mm pads sit flush with the tread, so pads and tread grip together.

All tray positions and tread values are parameters at the top of `V2/osupad_enclosure.scad` (`BALLAST_TRAYS`, `TREAD_*`, `PAD_*`).

## 6. Future Hot-Swap Key PCB

V2 already keeps space for it (`KEY_PCB_KEEPOUT`): a 40 × 21 mm area under the keys (x 18–58, y 6–27), from the PCB top at 5.0 mm below the plate top down to the socket bottoms.

Recommended design:
- **Board:** a small 2-key PCB (KiCad, ordered from JLCPCB or similar), 1.6 mm FR4, about 38 × 19 mm. Switch centres 19.05 mm apart, matching `MX_PITCH`, at the plate cutout positions.
- **Sockets:** Kailh MX hot-swap sockets (CPG151101S11) on the underside. No diodes are needed, because each key has its own GPIO. Leave out RC debounce capacitors: the firmware's eager debounce is faster, and capacitors add latency.
- **Mounting:** plate mount. The switches clip into the printed 1.5 mm plate, and the PCB sits 5.0 mm below the plate top (the MX standard). Screw the PCB to 2–4 small bosses under the key deck with M2 screws into heat-set inserts. That way the PCB stays put when a switch is pushed in or pulled out. If a socket still flexes, add a support rib from the bottom plate under it.
- **Connector:** a 3-pin JST-SH or JST-PH on the PCB, with a short cable to the board. On the board end, solder to header P2 (GPIO14, GPIO9, GND), or use a low-profile right-angle 1×3 housing. A straight Dupont housing does not fit under the screen.

Once the PCB outline and mounting holes are fixed, the bosses can be added to the top case.

---

## 7. Upstream Credits & Licenses
- **Waveshare**: Official 3D CAD model and 2D blueprints ([Waveshare Wiki](https://www.waveshare.com/wiki/ESP32-S3-Touch-LCD-2))
- **`milk-crate`**: 2-Key Macropad by *somepin* ([GitHub](https://github.com/somepin/milk-crate), MIT License)
- **`osu-keypad`**: 2-Key Keypad by *clay53* ([GitHub](https://github.com/clay53/Osu-Keypad), MIT License)
- **`osu-clicker`**: 2-Key Hotswap Keypad by *kameHame HA* ([Printables #943460](https://www.printables.com/model/943460-osu-keypad), CC BY-NC 4.0)
