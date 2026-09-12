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
    └── osupad_enclosure.scad               # Parametric OpenSCAD enclosure integrating the Waveshare LCD + 2x MX keys
```

---

## 2. Hardware Dimensions Reference

### Waveshare ESP32-S3-Touch-LCD-2
* **Outer Glass Lens**: `37.10 mm (W) × 58.80 mm (L) × 1.10 mm (H)` with `4× R2.60 mm` corner fillet
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
1. Open [`hardware/3d/custom_case/osupad_enclosure.scad`](custom_case/osupad_enclosure.scad) in [OpenSCAD](https://openscad.org/).
2. Change `PART = "top_case";` or `PART = "bottom_plate";`.
3. Press `F6` to render, then `F7` to export directly to `.stl`.
4. All dimensions, wall thicknesses, and tolerances are fully configurable as variables at the top of the file.

---

## 4. Upstream Credits & Licenses
- **Waveshare**: Official 3D CAD model and 2D blueprints ([Waveshare Wiki](https://www.waveshare.com/wiki/ESP32-S3-Touch-LCD-2))
- **`milk-crate`**: 2-Key Macropad by *somepin* ([GitHub](https://github.com/somepin/milk-crate), MIT License)
- **`osu-keypad`**: 2-Key Keypad by *clay53* ([GitHub](https://github.com/clay53/Osu-Keypad), MIT License)
- **`osu-clicker`**: 2-Key Hotswap Keypad by *kameHame HA* ([Printables #943460](https://www.printables.com/model/943460-osu-keypad), CC BY-NC 4.0)
