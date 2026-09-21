# OPad ESP32-S3 — 3D Models & Enclosure CAD Reference

This directory contains the ready-to-print/modify OPad enclosure for the **Waveshare ESP32-S3-Touch-LCD-2** and standard **MX mechanical switches**. Everything here is original OPad work under the repository's [MIT License](../../LICENSE).

---

## 1. Directory Structure

```
hardware/3d/
└── custom_case/
    └── V1/                                 # Enclosure (~22° screen deck, recessed glass pocket)
        ├── osupad_enclosure.scad           # Parametric OpenSCAD enclosure integrating the Waveshare LCD + 2x MX keys
        ├── osupad_case_top.stl
        ├── osupad_case_bottom.stl
        └── osupad_preview.png
```

Third-party models (the Waveshare board model and other keypads) are not stored in this repository, since they are not MIT licensed. Download them from their authors if you want them as a modelling reference; see section 4.

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

### Option A: Use the OpenSCAD Parametric Generator (Recommended)
1. Open [`hardware/3d/custom_case/V1/osupad_enclosure.scad`](custom_case/V1/osupad_enclosure.scad) in [OpenSCAD](https://openscad.org/).
2. Change `PART = "top_case";` or `PART = "bottom_plate";`.
3. Press `F6` to render, then `F7` to export directly to `.stl`.
4. All dimensions, wall thicknesses, and tolerances are fully configurable as variables at the top of the file.

### Option B: Model Around the Board in FreeCAD, Fusion 360 or Onshape
1. Download the official board STEP model from the [Waveshare Wiki](https://www.waveshare.com/wiki/ESP32-S3-Touch-LCD-2) (not redistributed here).
2. Import it next to an export of the OPad case, or model around the dimensions in section 2.
3. Export as `.stl` or `.step` for slicing.

---

## 4. External References (not included, not MIT)
- **Waveshare ESP32-S3-Touch-LCD-2**: official 3D CAD model and 2D drawings on the [Waveshare Wiki](https://www.waveshare.com/wiki/ESP32-S3-Touch-LCD-2). No redistribution license is stated, so the files are not included here.
- **`milk-crate`**: 2-key macropad by *somepin* ([GitHub](https://github.com/somepin/milk-crate), MIT License).
- **`osu-keypad`**: 2-key keypad by *clay53* ([GitHub](https://github.com/clay53/Osu-Keypad), MIT License).
- **`osu-clicker`**: 2-key hotswap keypad by *kameHame HA* ([Printables #943460](https://www.printables.com/model/943460-osu-keypad), CC BY-NC 4.0, non-commercial only).
