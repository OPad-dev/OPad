// Assembly render with the parts inside (for the README pictures)
include <../opad_enclosure.scad>
include <reference_models.scad>
PART = "none";
SHOW_CASE = true;
NO_TOP = true;  // hide the top case (and the glass) to show the inside
CUT = false;     // cut the case open on the left to show the cable
USE_STL = false; // V4: draw the case from the print files (the preview of the CSG shows artefacts)
module top_case_r()     if (USE_STL) translate([0, 0, BASE_T]) import("../opad_case_top.stl"); else top_case();
module bottom_plate_r() if (USE_STL) import("../opad_case_bottom.stl"); else bottom_plate();
module stuff() {
    if (!NO_TOP) color([0.1, 0.1, 0.1]) ref("ws_glass");
    color([0.15, 0.4, 0.2]) { ref("carrier"); ref("mod_pcb"); ref("usb_pcb"); }
    color([0.8, 0.8, 0.8]) { ref("jusb"); ref("jmod"); ref("mod_j1"); ref("usb_jack"); ref("usb_j2"); }
    color([0.9, 0.6, 0.1]) { ref("cable"); ref("usb_cable"); }
    color([0.95, 0.95, 0.95]) ref("keycaps");
    color([0.3, 0.3, 0.35]) ref("usb_plug");
}
stuff();
if (SHOW_CASE) difference() {
    union() { if (!NO_TOP) color([0.20, 0.21, 0.25]) top_case_r(); color([0.12, 0.13, 0.16]) bottom_plate_r(); }
    if (CUT) translate([-1, -1, -1]) cube([CASE_W / 2 + 1, CASE_L + 2, 60]);
}
