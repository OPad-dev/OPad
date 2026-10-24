// Assembly render with the parts inside (for the README pictures)
include <../osupad_enclosure.scad>
include <reference_models.scad>
PART = "none";
SHOW_CASE = true;
NO_TOP = false;  // hide the top case (and the glass) to show the inside
CUT = false;     // cut the case open on the left to show the cable
module stuff() {
    if (!NO_TOP) color([0.1, 0.1, 0.1]) ref("ws_glass");
    color([0.15, 0.4, 0.2]) { ref("carrier"); ref("mod_pcb"); }
    color([0.8, 0.8, 0.8]) { ref("usb_c"); ref("jmod"); ref("mod_j1"); }
    color([0.9, 0.6, 0.1]) ref("cable");
    color([0.95, 0.95, 0.95]) ref("keycaps");
    color([0.3, 0.3, 0.35]) ref("usb_plug");
}
stuff();
if (SHOW_CASE) difference() {
    union() { if (!NO_TOP) color([0.35, 0.45, 0.60]) top_case(); color([0.18, 0.20, 0.24]) bottom_plate(); }
    if (CUT) translate([-1, -1, -1]) cube([CASE_W / 2 + 1, CASE_L + 2, 60]);
}
