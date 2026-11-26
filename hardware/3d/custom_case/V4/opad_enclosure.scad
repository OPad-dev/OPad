// =============================================================================
//  OPad V4 enclosure (for the V3 boards, hardware/pcb/V3)
//  - V4: V3's case inside a faceted exterior ("Stealth"): side walls in two facets
//    with a V-groove along the crease, front corners cut off, a three-plane rise from
//    the keys to the screen; the cavity keeps WALL inside it everywhere. Everything
//    below is otherwise V3's
//  - Low flat key deck for the three-key MX / Hall Effect input module
//  - 16 deg screen pod for the Waveshare ESP32-S3-Touch-LCD-2 on the V3
//    carrier (8.5 mm sockets); as V1.1, the stack drops in from the top and the
//    glass sits in a pocket, flush with the frame
//  - V3: USB-C on its own board (hardware/pcb/V3/USB), flat on the floor in the
//    rear-right corner, through the rear wall; the Waveshare's own USB port is closed
//  - Fully screwless, assembled into the top case upside down: the module hangs on four
//    pins on bosses under the key deck, and the USB board on two pins on the rear-wall
//    blocks; posts on the bottom plate take the pin tips and clamp both; ribs on the
//    plate carry the MX sockets when the switches are pushed in; three
//    supports catch the screen stack 0.5 mm under the carrier; the USB board takes the
//    plug's push on stops and the rear wall; the bottom plate snaps on
//  - Recesses for four 12.7 mm silicone feet
//
//  Parts: PART = "top_case" | "bottom_plate" | "assembly" | "side_by_side"
//  All parameters: opad_params.scad. Clearance checks: tools/check_fit.py
// =============================================================================

include <opad_params.scad>

PART = "assembly";

// V3: posts at the right-front and left-rear holes only: the module cable runs at the
// left-front one and the USB board lies under the right-rear one
CARRIER_POST_HOLES = [WS_HOLES[1], WS_HOLES[2]];
// ...so the third support presses the rear rail left of the USB board, clear of the
// socket tails (deck x, a ranges)
CARRIER_RAIL_PAD = [[14.0, USB_X0 - TOL - CASE_W/2], [10.7, 14.1]];

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

// --- V4 exterior -----------------------------------------------------------------

// Plan: front corners cut off (FRONT_CUT), rear corners rounded as V3's
module plan2d() hull() {
    polygon([[FRONT_CUT[0], 0], [CASE_W - FRONT_CUT[0], 0], [CASE_W, FRONT_CUT[1]], [0, FRONT_CUT[1]]]);
    for (x = [CORNER_R, CASE_W - CORNER_R]) translate([x, CASE_L - CORNER_R]) circle(r = CORNER_R);
}

// Cross-section (x, z) under a top-line height z: the side walls, the two facets each
// side (SIDE_FACETS) and the top; down to z_bot
module section(z, z_bot) polygon(concat([[0, z_bot], [CASE_W, z_bot]],
    [for (f = SIDE_FACETS) [CASE_W - f[0], z + f[1]]],
    [for (i = [len(SIDE_FACETS) - 1 : -1 : 0]) [SIDE_FACETS[i][0], z + SIDE_FACETS[i][1]]]));

// Sweep a section along TOP_LINE: the hull of each pair of neighbouring slices
module sweep_top_line(inset = 0) for (i = [0 : len(TOP_LINE) - 2]) hull() for (p = [TOP_LINE[i], TOP_LINE[i + 1]])
    translate([0, p[0], 0]) rotate([90, 0, 0]) linear_extrude(0.01)
        if (inset == 0) section(p[1], 0); else offset(delta = -inset) section(p[1], -20);

// V-grooves along the crease between the two facets, both sides
module creases() let (c = SIDE_FACETS[1])
    for (x = [c[0], CASE_W - c[0]], i = [0 : len(TOP_LINE) - 2]) hull() for (p = [TOP_LINE[i], TOP_LINE[i + 1]])
        translate([0, p[0], 0]) rotate([90, 0, 0]) linear_extrude(0.01)
            translate([x, p[1] + c[1]]) rotate(45) square(CREASE_V, center = true);

// --- Top case ------------------------------------------------------------------

module outer_shell() difference() {
    intersection() {
        sweep_top_line();
        translate([0, 0, BASE_T]) linear_extrude(H_REAR + 10) plan2d();
    }
    creases();
}

// V4: everything WALL inside the outside, sideways (facets) and in plan (cut corners)
module inner_clip() intersection() {
    sweep_top_line(WALL);
    translate([0, 0, -1]) linear_extrude(H_REAR + 10) offset(delta = -WALL) plan2d();
}

module inner_cavity() intersection() { inner_cavity_v3(); inner_clip(); }

module inner_cavity_v3() {
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
    // opening the Waveshare + carrier pass through (glass rests on the ledge around it),
    // widened round the Waveshare's own USB-C
    translate([0, 0, -POD_TOP_T - 5]) linear_extrude(5 + POD_TOP_T - POCKET_D + 0.01) {
        translate([WS_PCB_XC - THROUGH_X/2, -THROUGH_Y/2]) square([THROUGH_X, THROUGH_Y]);
        translate([0, -WS_USB_A - 0.4]) square([WS_PCB_XC + WS_USB_X + 0.4, 2*WS_USB_A + 0.8]);
    }
}

// V3: the USB board's jack through the rear wall, level; stadium for a 12 x 6.5 mm plug overmold
module usb_opening()
    translate([USB_XC, CASE_L - WALL - 0.3, USB_AXIS_Z]) rotate([-90, 0, 0])
        linear_extrude(WALL + 1) hull() for (s = [-1, 1]) translate([s * 2.75, 0]) circle(d = USB_OPEN_D);

// V3: blocks on the rear wall either side of the jack that clamp the USB board's rear
// corners down onto its posts, with pins through its two holes into the posts; a tab on
// the right wall over its front-right corner; and a rib along its left edge. With the
// case upside down for assembly, the board sits on the pins, level on the blocks and tab
USB_HOLD_L = 5.0;   // how far the blocks reach forward over the board
module usb_holddown() {
    z = USB_TOP + USB_CLAMP;
    difference() {
        union() {
            for (x = [[USB_X0, USB_XC - USB_JACK_W/2 - 0.4], [USB_XC + USB_JACK_W/2 + 0.4, CASE_W - WALL + 0.01]])
                translate([x[0], USB_REAR - USB_HOLD_L, z]) cube([x[1] - x[0], CASE_L - WALL - USB_REAR + USB_HOLD_L + 0.01, 4.0]);
            translate([USB_X0 - TOL - 0.8, USB_REAR - 3.5, USB_BOT + 0.3])
                cube([0.8, CASE_L - WALL - USB_REAR + 3.51, z - USB_BOT - 0.3 + 0.01]);
            translate([USB_X0 - TOL - 0.8, USB_REAR - 3.5, z]) cube([TOL + 0.81, CASE_L - WALL - USB_REAR + 3.51, 4.0]);
            // a tab on the right wall over the board's front-right corner (clear of J2 and
            // its parts): with the blocks it holds the board level while the case is upside down
            translate([USB_X0 + USB_BW - 2.8, USB_FRONT + 0.2, z]) cube([2.8 + TOL + 0.01, 5.5, 4.0]);
        }
    }
    pin_l = USB_T + USB_CLAMP + USB_PIN_INTO;
    for (h = USB_HOLES) translate([h[0], h[1], z + 0.01 - pin_l]) {
        translate([0, 0, 0.3]) cylinder(h = pin_l - 0.3, d = USB_PIN_D);
        cylinder(h = 0.31, d1 = USB_PIN_D - 0.6, d2 = USB_PIN_D);   // lead-in
    }
}

module key_cutouts() for (x = KEY_X) {
    translate([x - MX_CUT/2, KEY_Y - MX_CUT/2, KEY_DECK_H - 5]) cube([MX_CUT, MX_CUT, 10]);
    // 1.5 mm plate for the switch clips, relief underneath
    translate([x - 8, KEY_Y - 8, BASE_T - 1]) cube([16, 16, KEY_DECK_H - PLATE_T - BASE_T + 1]);
}

// Screwless module mount, top half: bosses that the module PCB bears against, with
// pins through its four corner holes to locate it
module module_bosses() for (h = MODULE_HOLES) translate([h[0], h[1], 0]) {
    translate([0, 0, MODULE_PCB_TOP]) cylinder(h = KEY_DECK_H - KEY_TOP_T - MODULE_PCB_TOP + 0.5, r = 3.2);
    pin_l = MODULE_PCB_T + MODULE_PIN_INTO;
    translate([0, 0, MODULE_PCB_TOP - pin_l]) {
        translate([0, 0, 0.3]) cylinder(h = pin_l - 0.3 + 0.01, d = MODULE_PIN_D);
        cylinder(h = 0.31, d1 = MODULE_PIN_D - 0.6, d2 = MODULE_PIN_D);   // lead-in chamfer
    }
}

module snap_grooves() {
    z = BASE_T + SNAP_Z - 0.1;
    d = SNAP_BEAD_H + 0.15;   // groove depth into the wall
    l = SNAP_BEAD_L + 1;
    for (y = SNAP_SIDE_Y) translate([WALL - d, y - l/2, z]) cube([d + 0.1, l, 1.3]);
    for (y = SNAP_RIGHT_Y) translate([CASE_W - WALL - 0.1, y - l/2, z]) cube([d + 0.1, l, 1.3]);
    for (x = SNAP_END_X) translate([x - l/2, WALL - d, z]) cube([l, d + 0.1, 1.3]);
    for (x = SNAP_REAR_X) translate([x - l/2, CASE_L - WALL - 0.1, z]) cube([l, d + 0.1, 1.3]);
}

module top_case() {
    difference() {
        intersection() {   // V4: nothing inside stands out through the facets
            union() {
                difference() { outer_shell(); inner_cavity(); }
                module_bosses();
                usb_holddown();
            }
            outer_shell();
        }
        usb_opening();
        screen_cutouts();
        key_cutouts();
        snap_grooves();
        // rear pry notch
        translate([NOTCH_X - 6, CASE_L - 2, BASE_T - 0.1]) cube([12, 4, 2.2]);
    }
}

// --- Bottom plate ----------------------------------------------------------------

// Screwless module mount, bottom half: posts that clamp the module PCB up against the
// key deck bosses, with holes (chamfered) that take the pin tips
module module_posts() {
    h = MODULE_PCB_BOT - BASE_T + 0.01;
    for (p = MODULE_HOLES) translate([p[0], p[1], BASE_T - 0.01]) difference() {
        cylinder(h = h, r = 3.0);
        translate([0, 0, h - MODULE_PIN_INTO - 0.5]) cylinder(h = MODULE_PIN_INTO + 1, d = MODULE_PIN_HOLE);
        translate([0, 0, h - 0.5]) cylinder(h = 0.51, d1 = MODULE_PIN_HOLE, d2 = MODULE_PIN_HOLE + 0.8);
    }
}

// As V1.1, supports topped parallel to the carrier and POST_GAP below it: they catch
// the stack if the glass's tape lets go (the glass pocket locates it). V3: posts under
// two of its M2 holes and a pad under the rear rail
function carrier_hole_world(h) = deck_to_world([h[0], h[1], -CARRIER_BOT]);
module below_carrier() at_deck() translate([-CASE_W, -CASE_L, -CARRIER_BOT - POST_GAP - 60]) cube([2*CASE_W, 2*CASE_L, 60]);
module carrier_posts() {
    let (x = CARRIER_RAIL_PAD[0], y0 = deck_to_world([0, CARRIER_RAIL_PAD[1][0], -CARRIER_BOT])[1],
         y1 = deck_to_world([0, CARRIER_RAIL_PAD[1][1], -CARRIER_BOT])[1])
        intersection() {
            translate([CASE_W/2 + x[0], y0, BASE_T - 0.01]) cube([x[1] - x[0], y1 - y0, 40]);
            below_carrier();
        }
    for (h = CARRIER_POST_HOLES) let (p = carrier_hole_world(h))
        intersection() {
            translate([p[0], p[1], BASE_T - 0.01]) cylinder(h = 40, r = 3.0);
            below_carrier();
        }
}

// Ribs under the MX sockets: three bars across each key, SOCKET_RIB_GAP under the sockets
module socket_ribs() {
    h = MODULE_PCB_BOT - MODULE_BELOW - SOCKET_RIB_GAP - BASE_T;
    for (k = KEY_X, dy = [-5.5, 0, 5.5])
        translate([k - 8.0, KEY_Y + dy - 0.6, BASE_T - 0.01]) cube([15.0, 1.2, h + 0.01]);
}

// V3: the USB board's seat: two posts under its holes (it is clamped onto them by the
// top case's blocks), with holes (chamfered) that take the blocks' pin tips, and stops at
// its front corners, clear of J2's cable, that take the plug's push; the rear wall takes the pull
module usb_seat() {
    for (h = USB_HOLES) translate([h[0], h[1], BASE_T - 0.01]) difference() {
        cylinder(h = USB_STANDOFF + 0.01, r = 2.0);
        translate([0, 0, USB_STANDOFF - USB_PIN_INTO - 0.3]) cylinder(h = USB_PIN_INTO + 1, d = USB_PIN_HOLE);
        translate([0, 0, USB_STANDOFF - 0.4]) cylinder(h = 0.41, d1 = USB_PIN_HOLE, d2 = USB_PIN_HOLE + 0.6);
    }
    for (x = [USB_X0, USB_X0 + USB_BW - 3.0])
        translate([x, USB_FRONT - TOL - 2.0, BASE_T - 0.01]) cube([3.0, 2.0, USB_TOP + 0.8 - BASE_T]);
}

// Snap beads on the rigid rim: flat retaining face below, lead-in ramp above
module snap_bead(len) hull() {
    cube([len, SNAP_BEAD_H + 0.5, 0.4]);
    translate([0, 0, 0.9]) cube([len, 0.5, 0.1]);
}

module snap_beads() {
    z = BASE_T + SNAP_Z;
    o = WALL + TOL;   // rim outer face
    for (y = SNAP_SIDE_Y) translate([o + 0.5, y - SNAP_BEAD_L/2, z]) rotate([0, 0, 90]) snap_bead(SNAP_BEAD_L);
    for (y = SNAP_RIGHT_Y) translate([CASE_W - o - 0.5, y + SNAP_BEAD_L/2, z]) rotate([0, 0, -90]) snap_bead(SNAP_BEAD_L);
    for (x = SNAP_END_X) translate([x + SNAP_BEAD_L/2, o + 0.5, z]) rotate([0, 0, 180]) snap_bead(SNAP_BEAD_L);
    for (x = SNAP_REAR_X) translate([x - SNAP_BEAD_L/2, CASE_L - o - 0.5, z]) snap_bead(SNAP_BEAD_L);
}

module bottom_plate() {
    difference() {
        union() {
            linear_extrude(BASE_T) plan2d();   // V4: the top case's plan
            // rigid perimeter rim: it locates the plate, the beads click into the walls;
            // V3: open where the USB board lies against the rear and right walls;
            // V4: it follows the cut front corners
            difference() {
                translate([0, 0, BASE_T - 0.01]) linear_extrude(RIM_H) difference() {
                    offset(delta = -(WALL + TOL)) plan2d();
                    offset(delta = -(WALL + TOL + RIM_T)) plan2d();
                }
                translate([USB_X0 - 1.0, USB_FRONT - 2.5, BASE_T]) cube([CASE_W, CASE_L, RIM_H + 1]);
                // V4: and in the cut front corners, where the module's front corners come
                // within its width of the wall
                for (x = [-1, CASE_W - FRONT_CUT[0] - 4]) translate([x, -1, BASE_T]) cube([FRONT_CUT[0] + 5, FRONT_CUT[1] + 3, RIM_H + 1]);
            }
            snap_beads();
            module_posts();
            socket_ribs();
            carrier_posts();
            usb_seat();
        }
        // silicone foot recesses
        for (p = FOOT_POS) translate([p[0], p[1], -0.01]) cylinder(h = FOOT_REC + 0.01, d = FOOT_D + 0.4);
        // rear pry notch relief
        translate([NOTCH_X - 6, CASE_L - 3, -0.1]) cube([12, 5, 1.4]);
    }
}

// --- Render selection ------------------------------------------------------------

if (PART == "top_case")          translate([0, 0, -BASE_T]) top_case();
else if (PART == "bottom_plate") bottom_plate();
else if (PART == "side_by_side") {   // both parts apart: top case upright, bottom plate to its right
    color([0.35, 0.45, 0.60]) top_case();
    color([0.18, 0.20, 0.24]) translate([CASE_W + 15, 0, 0]) bottom_plate();
}
else if (PART == "assembly") {
    color([0.35, 0.45, 0.60]) top_case();
    color([0.18, 0.20, 0.24]) bottom_plate();
}
