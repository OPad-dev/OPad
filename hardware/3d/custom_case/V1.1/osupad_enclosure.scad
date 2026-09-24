// =============================================================================
//  OPad V1.1 enclosure
//  - Low flat key deck for the MX / Hall Effect input module (2 keys)
//  - 16 deg screen pod for the Waveshare ESP32-S3-Touch-LCD-2 on the V1
//    controller carrier (8.5 mm sockets); glass flush with the frame
//  - Fully screwless: the input module is located by pins under the key deck
//    and clamped by posts on the bottom plate; the bottom plate snaps on
//  - Recesses for four 12.7 mm silicone feet
//
//  Parts: PART = "top_case" | "bottom_plate" | "assembly"
//  All parameters: osupad_params.scad. Clearance checks: tools/check_fit.py
// =============================================================================

include <osupad_params.scad>

PART = "assembly";

// --- Helpers -------------------------------------------------------------------

// Prism over [x0,x1] x [y0,y1] from z0 to z1; corners listed in `round`
// ("fl", "fr", "rl", "rr" = front/rear, left/right) get radius r.
module rbox(x0, y0, x1, y1, z0, z1, r, round = ["fl", "fr", "rl", "rr"]) {
    function has(c) = len([for (k = round) if (k == c) 1]) > 0;
    hull() for (c = [["fl", x0, y0], ["fr", x1, y0], ["rl", x0, y1], ["rr", x1, y1]]) {
        sx = c[1] == x0 ? 1 : -1;
        sy = c[2] == y0 ? 1 : -1;
        if (has(c[0]))
            translate([c[1] + sx*r, c[2] + sy*r, z0]) cylinder(h = z1 - z0, r = r);
        else
            translate([c[1] + (sx > 0 ? 0 : -0.01), c[2] + (sy > 0 ? 0 : -0.01), z0])
                cube([0.01, 0.01, z1 - z0]);
    }
}

// Rounded rectangle, centred, in the XY plane
module rrect2d(x, y, r) offset(r) square([x - 2*r, y - 2*r], center = true);

// --- Top case ------------------------------------------------------------------

module outer_shell() {
    // key deck, rounded at the front; runs a little into the pod
    rbox(0, 0, CASE_W, Y_KEY_END + 4, BASE_T, KEY_DECK_H, CORNER_R, ["fl", "fr"]);
    // screen pod: the tilted face hulled down to a footprint that is vertical up to Z_KNEE
    hull() {
        at_deck() translate([0, 0, -0.01]) linear_extrude(0.01)
            translate([-CASE_W/2, -FRAME_A]) square([CASE_W, 2*FRAME_A - CORNER_R]);
        at_deck() for (sx = [-1, 1])
            translate([sx * (CASE_W/2 - CORNER_R), FRAME_A - CORNER_R, -0.01]) cylinder(h = 0.01, r = CORNER_R);
        rbox(0, Y_KEY_END, CASE_W, CASE_L, BASE_T, Z_KNEE, CORNER_R, ["rl", "rr"]);
    }
}

module inner_cavity() {
    ir = CORNER_R - WALL;
    // under the key deck, open at the bottom; reaches back into the pod for the cable
    rbox(WALL, WALL, CASE_W - WALL, Y_KEY_END + 4, BASE_T - 1, KEY_DECK_H - KEY_TOP_T, ir, ["fl", "fr"]);
    hull() {
        at_deck() translate([-(CASE_W/2 - WALL), -(FRAME_A - WALL), -POD_TOP_T - 0.01])
            cube([CASE_W - 2*WALL, 2*(FRAME_A - WALL) - ir, 0.01]);
        at_deck() for (sx = [-1, 1])
            translate([sx * (CASE_W/2 - WALL - ir), FRAME_A - WALL - ir, -POD_TOP_T - 0.01]) cylinder(h = 0.01, r = ir);
        rbox(WALL, Y_KEY_END + 3, CASE_W - WALL, CASE_L - WALL, BASE_T - 1, Z_KNEE, ir, ["rl", "rr"]);
    }
}

module screen_cutouts() at_deck() {
    // glass pocket: the glass front ends flush with the frame
    translate([0, 0, -POCKET_D]) linear_extrude(10) rrect2d(POCKET_X, POCKET_Y, POCKET_R);
    // opening the Waveshare + carrier pass through (glass rests on the ledge around it)
    translate([WS_PCB_XC - THROUGH_X/2, -THROUGH_Y/2, -POD_TOP_T - 5]) cube([THROUGH_X, THROUGH_Y, 5 + POD_TOP_T - POCKET_D + 0.01]);
    // USB-C: stadium through the right wall, sized for a 12 x 6.5 mm plug overmold
    translate([USB_X - 1, 0, -USB_D]) rotate([0, 90, 0])
        linear_extrude(CASE_W) hull() for (sa = [-1, 1]) translate([0, sa * 3.4]) circle(r = 4.4);
}

module key_cutouts() for (x = KEY_X) {
    translate([x - MX_CUT/2, KEY_Y - MX_CUT/2, KEY_DECK_H - 5]) cube([MX_CUT, MX_CUT, 10]);
    // 1.5 mm plate for the switch clips, relief underneath
    translate([x - 8, KEY_Y - 8, BASE_T - 1]) cube([16, 16, KEY_DECK_H - PLATE_T - BASE_T + 1]);
}

// Screwless module mount, top half: bosses that the module PCB bears against,
// with pins through its two M2 holes to locate it
module module_bosses() for (h = MODULE_HOLES) translate([h[0], h[1], 0]) {
    translate([0, 0, MODULE_PCB_TOP]) cylinder(h = KEY_DECK_H - KEY_TOP_T - MODULE_PCB_TOP + 0.5, r = 3.2);
    pin_l = MODULE_PCB_T + 1.0;
    translate([0, 0, MODULE_PCB_TOP - pin_l]) {
        translate([0, 0, 0.3]) cylinder(h = pin_l - 0.3 + 0.01, d = MODULE_PIN_D);
        cylinder(h = 0.31, d1 = MODULE_PIN_D - 0.6, d2 = MODULE_PIN_D);   // lead-in chamfer
    }
}

module snap_grooves() {
    z = BASE_T + SNAP_Z - 0.1;
    d = SNAP_BEAD_H + 0.15;   // groove depth into the wall
    l = SNAP_BEAD_L + 1;
    for (y = SNAP_SIDE_Y) {
        translate([WALL - d, y - l/2, z]) cube([d + 0.1, l, 1.3]);
        translate([CASE_W - WALL - 0.1, y - l/2, z]) cube([d + 0.1, l, 1.3]);
    }
    for (x = SNAP_END_X) {
        translate([x - l/2, WALL - d, z]) cube([l, d + 0.1, 1.3]);
        translate([x - l/2, CASE_L - WALL - 0.1, z]) cube([l, d + 0.1, 1.3]);
    }
}

module top_case() {
    difference() {
        union() {
            difference() { outer_shell(); inner_cavity(); }
            module_bosses();
        }
        screen_cutouts();
        key_cutouts();
        snap_grooves();
        // rear pry notch
        translate([CASE_W/2 - 6, CASE_L - 2, BASE_T - 0.1]) cube([12, 4, 2.2]);
    }
}

// --- Bottom plate ----------------------------------------------------------------

// Screwless module mount, bottom half: posts that clamp the module PCB up
// against the key deck bosses (tubes at the holes take the pin tips)
module module_posts() {
    h = MODULE_PCB_BOT - BASE_T + 0.01;
    for (p = MODULE_HOLES) translate([p[0], p[1], BASE_T - 0.01]) difference() {
        cylinder(h = h, r = 3.0);
        translate([0, 0, 0.8]) cylinder(h = h, d = MODULE_PIN_D + 0.7);
    }
    for (p = MODULE_POSTS) translate([p[0], p[1], BASE_T - 0.01]) cylinder(h = h, r = 2.5);
}

// Vertical tubes under the carrier's four M2 holes, topped parallel to the
// carrier and POST_GAP below it: they catch the carrier if it works loose from
// the sockets and leave room for the optional M2 spacer screw heads.
function carrier_hole_world(h) = [CASE_W/2 + h[0],
                                  Y_C + h[1]*cos(TILT) + CARRIER_BOT*sin(TILT),
                                  Z_C + h[1]*sin(TILT) - CARRIER_BOT*cos(TILT)];
module carrier_posts() {
    intersection() {
        for (h = WS_HOLES) let (p = carrier_hole_world(h))
            translate([p[0], p[1], BASE_T - 0.01]) difference() {
                cylinder(h = 30, r = 3.7);
                translate([0, 0, -1]) cylinder(h = 32, r = 2.5);
            }
        at_deck() translate([-CASE_W, -CASE_L, -(CARRIER_BOT + POST_GAP) - 60]) cube([2*CASE_W, 2*CASE_L, 60]);
    }
}

// Snap beads on the rigid rim: flat retaining face below, lead-in ramp above
module snap_bead(len) hull() {
    cube([len, SNAP_BEAD_H + 0.5, 0.4]);
    translate([0, 0, 0.9]) cube([len, 0.5, 0.1]);
}

module snap_beads() {
    z = BASE_T + SNAP_Z;
    o = WALL + TOL;   // rim outer face
    for (y = SNAP_SIDE_Y) {
        translate([o + 0.5, y - SNAP_BEAD_L/2, z]) rotate([0, 0, 90]) snap_bead(SNAP_BEAD_L);
        translate([CASE_W - o - 0.5, y + SNAP_BEAD_L/2, z]) rotate([0, 0, -90]) snap_bead(SNAP_BEAD_L);
    }
    for (x = SNAP_END_X) {
        translate([x + SNAP_BEAD_L/2, o + 0.5, z]) rotate([0, 0, 180]) snap_bead(SNAP_BEAD_L);
        translate([x - SNAP_BEAD_L/2, CASE_L - o - 0.5, z]) snap_bead(SNAP_BEAD_L);
    }
}

module bottom_plate() {
    inner_w = CASE_W - 2*WALL - 2*TOL;
    inner_l = CASE_L - 2*WALL - 2*TOL;
    rim_r   = max(1.0, CORNER_R - WALL - TOL);
    difference() {
        union() {
            rbox(0, 0, CASE_W, CASE_L, 0, BASE_T, CORNER_R);
            // rigid perimeter rim: it locates the plate, the beads click into the walls
            translate([WALL + TOL, WALL + TOL, BASE_T - 0.01]) difference() {
                rbox(0, 0, inner_w, inner_l, 0, RIM_H, rim_r);
                rbox(RIM_T, RIM_T, inner_w - RIM_T, inner_l - RIM_T, -0.1, RIM_H + 1, max(0.5, rim_r - RIM_T));
            }
            snap_beads();
            module_posts();
            carrier_posts();
        }
        // silicone foot recesses
        for (p = FOOT_POS) translate([p[0], p[1], -0.01]) cylinder(h = FOOT_REC + 0.01, d = FOOT_D + 0.4);
        // rear pry notch relief
        translate([CASE_W/2 - 6, CASE_L - 3, -0.1]) cube([12, 5, 1.4]);
    }
}

// --- Render selection ------------------------------------------------------------

if (PART == "top_case")          translate([0, 0, -BASE_T]) top_case();
else if (PART == "bottom_plate") bottom_plate();
else if (PART == "assembly") {
    color([0.35, 0.45, 0.60]) top_case();
    color([0.18, 0.20, 0.24]) bottom_plate();
}
