// Intersection of one reference part with one case part; empty means clear.
// Driven by check_fit.py:  openscad -D 'REF="carrier"' -D 'CASE="bottom"' ...
// include (not use) so that -D overrides reach the case parts too
include <../osupad_enclosure.scad>
include <reference_models.scad>
PART = "none";

REF  = "carrier";
CASE = "top";   // "top" | "bottom" | "case" (both) | "none" | "ref" (against REF2)
REF2 = "";

module case_part(c) {
    if (c == "top" || c == "case")    top_case();
    if (c == "bottom" || c == "case") bottom_plate();
}

if (CASE == "none") ref(REF);
else if (CASE == "ref") intersection() { ref(REF); ref(REF2); }
else if (REF == "top") intersection() { top_case(); case_part(CASE); }
else intersection() { ref(REF); case_part(CASE); }
