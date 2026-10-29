// =============================================================================
//  OPad V3 enclosure (for the V3 boards, hardware/pcb/V3)
//  - Low flat key deck for the three-key MX / Hall Effect input module
//  - 16 deg screen pod for the Waveshare ESP32-S3-Touch-LCD-2 on the V3
//    carrier (8.5 mm sockets). V3: the stack goes in from below and the glass
//    bears on a 1.2 mm lip round the window
//  - V3: USB-C on its own board (hardware/pcb/V3/USB), flat on the floor in the
//    rear-right corner, through the rear wall; the Waveshare's own USB port is closed
//  - Fully screwless: the module is located by four pins under the key deck
//    and clamped by posts on the bottom plate; two posts with pins in the
//    carrier's M2 holes and a third support hold the screen stack up against the
//    lip; the USB board sits on two pins, is clamped from above by blocks on the
//    rear wall and takes the plug's push on stops and the rear wall; the bottom
//    plate snaps on
//  - V3: channels with small lips on the bottom plate hold both cables (FFC) on the floor
//  - Recesses for four 12.7 mm silicone feet
//
//  Parts: PART = "top_case" | "bottom_plate" | "assembly"
//  All parameters: osupad_params.scad. Clearance checks: tools/check_fit.py
// =============================================================================

include <osupad_params.scad>

PART = "assembly";

CAR_PIN_D = 1.9;  // in the carrier's 2.2 mm M2 holes
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
    // V3: window = glass minus the lip; the glass sits in a pocket under the lip, open to the cavity
    translate([0, 0, -LIP_T - 0.01]) linear_extrude(10) rrect2d(GLASS_X - 2*LIP, GLASS_Y - 2*LIP, max(0.5, GLASS_R - LIP));
    translate([0, 0, -POD_TOP_T - 1]) linear_extrude(1 + POD_TOP_T - LIP_T) rrect2d(POCKET_X, POCKET_Y, POCKET_R);
}

// V3: the USB board's jack through the rear wall, level; stadium for a 12 x 6.5 mm plug overmold
module usb_opening()
    translate([USB_XC, CASE_L - WALL - 0.3, USB_AXIS_Z]) rotate([-90, 0, 0])
        linear_extrude(WALL + 1) hull() for (s = [-1, 1]) translate([s * 2.75, 0]) circle(d = USB_OPEN_D);

// V3: blocks on the rear wall either side of the jack that clamp the USB board's rear
// corners down onto its pins (recesses take the pin tips), and a rib along its left edge
// that locates it while the case is upside down for assembly
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
        }
        for (h = USB_HOLES) translate([h[0], h[1], z - 0.01]) cylinder(h = 1.0, d = USB_PIN_D + 0.7);
    }
}

module key_cutouts() for (x = KEY_X) {
    translate([x - MX_CUT/2, KEY_Y - MX_CUT/2, KEY_DECK_H - 5]) cube([MX_CUT, MX_CUT, 10]);
    // 1.5 mm plate for the switch clips, relief underneath
    translate([x - 8, KEY_Y - 8, BASE_T - 1]) cube([16, 16, KEY_DECK_H - PLATE_T - BASE_T + 1]);
}

// Screwless module mount, top half: bosses that the module PCB bears against,
// with pins through its four corner holes to locate it
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
    for (y = SNAP_SIDE_Y) translate([WALL - d, y - l/2, z]) cube([d + 0.1, l, 1.3]);
    for (y = SNAP_RIGHT_Y) translate([CASE_W - WALL - 0.1, y - l/2, z]) cube([d + 0.1, l, 1.3]);
    for (x = SNAP_END_X) translate([x - l/2, WALL - d, z]) cube([l, d + 0.1, 1.3]);
    for (x = SNAP_REAR_X) translate([x - l/2, CASE_L - WALL - 0.1, z]) cube([l, d + 0.1, 1.3]);
}

module top_case() {
    difference() {
        union() {
            difference() { outer_shell(); inner_cavity(); }
            module_bosses();
            usb_holddown();
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

// Screwless module mount, bottom half: posts that clamp the module PCB up
// against the key deck bosses (tubes at the holes take the pin tips)
module module_posts() {
    h = MODULE_PCB_BOT - BASE_T + 0.01;
    for (p = MODULE_HOLES) translate([p[0], p[1], BASE_T - 0.01]) difference() {
        cylinder(h = h, r = 3.0);
        translate([0, 0, 0.8]) cylinder(h = h, d = MODULE_PIN_D + 0.7);
    }
}

// V3: posts under two of the carrier's M2 holes, topped parallel to the carrier
// and pressing it up (the foam gasket on the lip takes the tolerance), with a pin
// into each hole to locate the stack; a third support presses the rear rail
function carrier_hole_world(h) = deck_to_world([h[0], h[1], -CARRIER_BOT]);
module below_carrier() at_deck() translate([-CASE_W, -CASE_L, -CARRIER_BOT - 60]) cube([2*CASE_W, 2*CASE_L, 60]);
module carrier_posts() {
    let (x = CARRIER_RAIL_PAD[0], y0 = deck_to_world([0, CARRIER_RAIL_PAD[1][0], -CARRIER_BOT])[1],
         y1 = deck_to_world([0, CARRIER_RAIL_PAD[1][1], -CARRIER_BOT])[1])
        intersection() {
            translate([CASE_W/2 + x[0], y0, BASE_T - 0.01]) cube([x[1] - x[0], y1 - y0, 40]);
            below_carrier();
        }
    for (h = CARRIER_POST_HOLES) let (p = carrier_hole_world(h)) {
        intersection() {
            translate([p[0], p[1], BASE_T - 0.01]) cylinder(h = 40, r = 3.0);
            below_carrier();
        }
        at_deck() translate([h[0], h[1], -CARRIER_BOT - 0.01]) {
            cylinder(h = CARRIER_T + 0.5, d = CAR_PIN_D);
            translate([0, 0, CARRIER_T + 0.5]) cylinder(h = 0.3, d1 = CAR_PIN_D, d2 = CAR_PIN_D - 0.6);
        }
    }
}

// V3: L-shaped channel holding the module cable on the floor: across from the drop
// beside the stack to the middle, round the 45 deg fold, then forward towards J1
module guide_wall(x0, y0, x1, y1, lip_dir) {
    translate([x0, y0, BASE_T - 0.01]) cube([x1 - x0, y1 - y0, GUIDE_H + 0.01]);
    // lip on the inside of the top edge: [dx, dy] points into the channel
    lx = lip_dir[0] > 0 ? x1 : lip_dir[0] < 0 ? x0 - GUIDE_LIP : x0;
    ly = lip_dir[1] > 0 ? y1 : lip_dir[1] < 0 ? y0 - GUIDE_LIP : y0;
    translate([lx, ly, BASE_T + GUIDE_H - GUIDE_LIP_T])
        cube([lip_dir[0] != 0 ? GUIDE_LIP : x1 - x0, lip_dir[1] != 0 ? GUIDE_LIP : y1 - y0, GUIDE_LIP_T]);
}
module ffc_guide() {
    w = FFC_W/2 + GUIDE_GAP;        // channel half width
    x_start = FFC_DROP_X + 3.5;     // after the cable's bend onto the floor
    cx = FFC_FOLD[0];
    ry = FFC_RUN_Y;
    difference() {
        union() {
            // run across: rear wall to the outer corner of the fold, front wall up to the forward run
            guide_wall(x_start, ry + w, cx + w + GUIDE_T, ry + w + GUIDE_T, [0, -1]);
            guide_wall(x_start, ry - w - GUIDE_T, cx - w, ry - w, [0, 1]);
            // run forward: both side walls from the corner down to GUIDE_END_Y
            guide_wall(cx - w - GUIDE_T, GUIDE_END_Y, cx - w, ry - w, [1, 0]);
            guide_wall(cx + w, GUIDE_END_Y, cx + w + GUIDE_T, ry + w + GUIDE_T, [-1, 0]);
        }
        // no lips over the fold, where the cable lies doubled
        translate([cx - w - 1, ry - w - 1, BASE_T + GUIDE_H - GUIDE_LIP_T - 0.01])
            cube([2*w + 2, 2*w + 2, GUIDE_LIP_T + 0.1]);
    }
}

// V3: the USB cable's channel on the floor, straight back towards J2
module usb_guide() {
    w = FFC8_W/2 + GUIDE_GAP;
    guide_wall(USB_XC - w - GUIDE_T, USB_CH_Y[0], USB_XC - w, USB_CH_Y[1], [1, 0]);
    guide_wall(USB_XC + w, USB_CH_Y[0], USB_XC + w + GUIDE_T, USB_CH_Y[1], [-1, 0]);
}

// V3: the USB board's seat: two posts with pins through its holes (it is clamped onto
// them by the top case's blocks), and stops at its front corners, clear of J2's cable,
// that take the plug's push; the rear wall takes the pull
module usb_seat() {
    for (h = USB_HOLES) translate([h[0], h[1], BASE_T - 0.01]) {
        cylinder(h = USB_STANDOFF + 0.01, r = 2.0);
        translate([0, 0, USB_STANDOFF]) cylinder(h = USB_T + 0.3, d = USB_PIN_D);
        translate([0, 0, USB_STANDOFF + USB_T + 0.3]) cylinder(h = 0.3, d1 = USB_PIN_D, d2 = USB_PIN_D - 0.6);
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
    inner_w = CASE_W - 2*WALL - 2*TOL;
    inner_l = CASE_L - 2*WALL - 2*TOL;
    rim_r   = max(1.0, CORNER_R - WALL - TOL);
    difference() {
        union() {
            rbox(0, 0, CASE_W, CASE_L, 0, BASE_T, CORNER_R);
            // rigid perimeter rim: it locates the plate, the beads click into the walls;
            // V3: open where the USB board lies against the rear and right walls
            difference() {
                translate([WALL + TOL, WALL + TOL, BASE_T - 0.01]) difference() {
                    rbox(0, 0, inner_w, inner_l, 0, RIM_H, rim_r);
                    rbox(RIM_T, RIM_T, inner_w - RIM_T, inner_l - RIM_T, -0.1, RIM_H + 1, max(0.5, rim_r - RIM_T));
                }
                translate([USB_X0 - 1.0, USB_FRONT - 2.5, BASE_T]) cube([CASE_W, CASE_L, RIM_H + 1]);
            }
            snap_beads();
            module_posts();
            carrier_posts();
            ffc_guide();
            usb_guide();
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
else if (PART == "assembly") {
    color([0.35, 0.45, 0.60]) top_case();
    color([0.18, 0.20, 0.24]) bottom_plate();
}
