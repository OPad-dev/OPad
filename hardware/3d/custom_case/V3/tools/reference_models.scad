// =============================================================================
//  Reference models for the V3 fit check: what goes inside the case, as simple
//  envelopes in world coordinates. Include after osupad_params.scad.
//  Sources: Waveshare 2024-11-08 drawing + STEP (waveshare_back.scad),
//  hardware/pcb/V3 (carrier, USB board, MX and HE modules; KiCad coordinates converted
//  with car_to_deck / usb_to_world / mod_to_world in osupad_params.scad).
// =============================================================================

include <waveshare_back.scad>

MX_BOT = MODULE_PCB_BOT;

module dbox(x0, x1, a0, a1, d0, d1) translate([x0, a0, -d1]) cube([x1 - x0, a1 - a0, d1 - d0]);

// Module part at KiCad (x, y) of the module board, w x l mm, h mm under the PCB
module mod_part(p, w, l, h) let (q = mod_to_world(p)) translate([q[0] - w/2, q[1] - l/2, MX_BOT - h]) cube([w, l, h]);
// USB board part over KiCad [x0, x1] x [y0, y1] of the USB board, from z0 to z1 above its top
module usb_part(x0, x1, y0, y1, z0, z1) let (a = usb_to_world([x0, y1]), b = usb_to_world([x1, y0]))
    translate([a[0], a[1], USB_TOP + z0]) cube([b[0] - a[0], b[1] - a[1], z1 - z0]);

// --- The FFC as a ribbon: a list of [point, width direction, thickness direction] ---
function unit(v) = v / norm(v);
DECK_A = [0, cos(TILT), sin(TILT)];        // deck "a" axis in world
DECK_N = [0, -sin(TILT), cos(TILT)];       // deck normal in world
X = [1, 0, 0]; Y = [0, 1, 0]; Z = [0, 0, 1];
_D0 = deck_to_world([CARRIER_EDGE_X + 0.25, JMOD[1], -(CARRIER_TOP - 1.2)]);  // just outside J_MOD's mouth
_D1 = deck_to_world([CARRIER_EDGE_X - 2.5, JMOD[1], -(CARRIER_TOP - 1.2)]);
_YD = FFC_RUN_Y;
RIBBON = [
    [_D0, DECK_A, DECK_N],
    [_D1, DECK_A, DECK_N],
    [[FFC_DROP_X, _YD, _D1[2] - 2.5], DECK_A, X],                    // bent down past the carrier's end
    [[FFC_DROP_X, _YD, FFC_FLOOR_Z + 2.5], Y, X],                    // straight down, untwisting to level
    [[FFC_DROP_X + 2.5, _YD, FFC_FLOOR_Z], Y, Z],                    // onto the floor
    [[FFC_FOLD[0] - FFC_W/2, _YD, FFC_FLOOR_Z], Y, Z],               // across the floor to the middle
];
RIBBON2 = [                                                          // after the 45 deg fold
    [[FFC_FOLD[0], _YD - FFC_W/2, FFC_FLOOR_Z], X, Z],
    [[J1_ENTRY[0], 36.0, FFC_FLOOR_Z], X, Z],
    [[J1_ENTRY[0], 33.0, J1_ENTRY[2]], X, Z],
    [J1_ENTRY, X, Z],
];
// The USB cable (9 mm): out of J_USB, the 45 deg fold past its stiffened end, forward beside the
// stack (a slight skew lines it up with J2), down in front of the board, back along the floor, up into J2
function _uw(x, a) = deck_to_world([x, a, -USB_RUN_D]);
URIBBON = [
    [_uw(JUSB_MOUTH_X + 0.25, JUSB_A), DECK_A, DECK_N],
    [_uw(USB_FOLD_X - FFC8_W/2, JUSB_A), DECK_A, DECK_N],
];
URIBBON2 = [
    [_uw(USB_FOLD_X, JUSB_A - FFC8_W/2), X, DECK_N],
    [_uw(USB_XC - CASE_W/2, USB_DROP_A), X, DECK_N],
    [[USB_XC, USB_DROP_Y - 2.0, FFC_FLOOR_Z + 2.5], X, Y],              // curled down
    [[USB_XC, USB_DROP_Y + 1.0, FFC_FLOOR_Z], X, Z],                    // onto the floor, heading back
    [[USB_XC, USB_CH_Y[1], FFC_FLOOR_Z], X, Z],
    [[USB_XC, USB_FRONT - FFC_STIFF - 0.5, J2_ENTRY_Z], X, Z],
    [[USB_XC, USB_FRONT, J2_ENTRY_Z], X, Z],
];
module ribbon_slab(r, wd = FFC_W) let (p = r[0], w = unit(r[1]) * wd / 2, t = unit(r[2]) * FFC_T / 2)
    polyhedron(points = [p - w - t, p + w - t, p + w + t, p - w + t,
                         p - w - t + 0.01*cross(r[1], r[2]), p + w - t + 0.01*cross(r[1], r[2]),
                         p + w + t + 0.01*cross(r[1], r[2]), p - w + t + 0.01*cross(r[1], r[2])],
               faces = [[0,1,2,3],[4,5,1,0],[7,6,5,4],[5,6,2,1],[6,7,3,2],[7,4,0,3]]);
module ribbon(rs, wd = FFC_W) for (i = [0 : len(rs) - 2]) hull() { ribbon_slab(rs[i], wd); ribbon_slab(rs[i + 1], wd); }

module ref(name) {
    // --- screen stack (deck frame) ---
    if (name == "gasket") at_deck()          // foam tape on the lip's underside, compressed
        translate([0, 0, -SINK]) linear_extrude(GASKET_C) difference() {
            rrect2d(GLASS_X, GLASS_Y, GLASS_R);
            rrect2d(GLASS_X - 2*LIP, GLASS_Y - 2*LIP, max(0.5, GLASS_R - LIP));
        }
    if (name == "ws_glass") at_deck()
        translate([0, 0, -SINK - GLASS_T]) linear_extrude(GLASS_T)
            offset(GLASS_R) square([GLASS_X - 2*GLASS_R, GLASS_Y - 2*GLASS_R], center = true);
    if (name == "ws_module") at_deck()     // glass to PCB back, inside the glass outline
        translate([WS_PCB_XC - WS_PCB_X/2, -WS_PCB_Y/2, -WS_BACK]) cube([WS_PCB_X, WS_PCB_Y, WS_BACK - SINK - GLASS_T]);
    if (name == "ws_back") at_deck() for (b = WS_BACK_BOXES) dbox(b[0] + WS_PCB_XC, b[1] + WS_PCB_XC, b[2], b[3], b[4] + SINK, b[5] + SINK);
    if (name == "sockets") at_deck() for (s = [-1, 1])
        dbox(WS_PCB_XC - 17.78, WS_PCB_XC + 17.78, s*ROW_A - 1.27, s*ROW_A + 1.27, WS_BACK + HDR_PLASTIC, CARRIER_TOP);
    // V3 carrier: V1 outline with the window, open at the USB end round the lug; M2 holes open
    if (name == "carrier") at_deck() difference() {
        dbox(CARRIER_EDGE_X, CARRIER_END_X, -CARRIER_A, CARRIER_A, CARRIER_TOP, CARRIER_BOT);
        dbox(CARRIER_WIN[0], CARRIER_END_X + 0.1, CARRIER_WIN[1], CARRIER_WIN[2], CARRIER_TOP - 1, CARRIER_BOT + 1);
        for (h = WS_HOLES) translate([h[0], h[1], -CARRIER_BOT - 1]) cylinder(h = CARRIER_T + 2, d = 2.2);
    }
    if (name == "carrier") at_deck()
        dbox(CARRIER_LUG[0], CARRIER_END_X, CARRIER_LUG[1], CARRIER_LUG[2], CARRIER_TOP, CARRIER_BOT);
    if (name == "tails") at_deck()         // socket pin tails under the carrier
        for (s = [-1, 1]) dbox(WS_PCB_XC - 17.2, WS_PCB_XC + 17.2, s*ROW_A - 0.9, s*ROW_A + 0.9, CARRIER_BOT, CARRIER_BOT + TAIL);
    if (name == "carrier_top_parts") at_deck() {   // SMD parts on the carrier top, at most 1.2 mm (SOT-23-6)
        dbox(CARRIER_EDGE_X + 5.5, CARRIER_EDGE_X + 15.0, -11.0, 15.0, CARRIER_TOP - 1.2, CARRIER_TOP);
        dbox(car_to_deck([0, 137.0])[0], car_to_deck([0, 140.2])[0], 11.2, 13.0, CARRIER_TOP - 1.0, CARRIER_TOP);  // FB1
    }
    if (name == "jusb") at_deck()            // J_USB on the lug, housing and fitting nails, mouth towards the USB end
        dbox(JUSB_MOUTH_X - 4.8, JUSB_MOUTH_X, JUSB_A - 7.15, JUSB_A + 7.15, CARRIER_TOP - JMOD_H, CARRIER_TOP);
    if (name == "jusb_latch") at_deck()     // its slide latch pulled out to open
        dbox(JUSB_MOUTH_X, JUSB_MOUTH_X + 1.8, JUSB_A - 6.6, JUSB_A + 6.6, CARRIER_TOP - JMOD_H, CARRIER_TOP);

    // --- USB board V3 (world frame) ---
    if (name == "usb_pcb") difference() {
        hull() for (x = [USB_X0 + 1, USB_X0 + USB_BW - 1], y = [USB_FRONT + 1, USB_REAR - 1])   // 1.0 mm corners
            translate([x, y, USB_BOT]) cylinder(h = USB_T, r = 1.0);
        for (h = USB_HOLES) translate([h[0], h[1], USB_BOT - 1]) cylinder(h = USB_T + 2, d = 2.2);
    }
    if (name == "usb_jack")                  // shell, mouth 0.4 mm past the rear edge
        translate([USB_XC - USB_JACK_W/2, USB_REAR + 0.4 - USB_JACK_L, USB_TOP]) cube([USB_JACK_W, USB_JACK_L, USB_JACK_H]);
    if (name == "usb_legs")                  // its four shell legs through the board, 1.0 mm out underneath
        for (p = [[105.675, 105.79], [114.325, 105.79], [105.675, 101.609], [114.325, 101.609]]) let (q = usb_to_world(p))
            translate([q[0] - 0.5, q[1] - 1.05, USB_BOT - 1.0]) cube([1.0, 2.1, 1.0 + USB_T]);
    if (name == "usb_j2") {                  // AFA07-S08 housing and nails, mouth 0.3 mm inside the front edge
        usb_part(103.47, 116.53, 116.9, 121.7, 0, JMOD_H);
        usb_part(102.85, 117.15, 117.8, 120.8, 0, 0.9);
    }
    if (name == "usb_j2_latch") usb_part(103.4, 116.6, 121.7, 123.5, 0, JMOD_H);
    if (name == "usb_parts") {               // 0603s 0.9 mm, SRV05-4 1.45 mm, B5817W 1.35 mm (courtyards)
        for (c = [[115.86, 118.94, 107.95, 109.64], [112.25, 115.34, 107.95, 109.64], [116.06, 119.14, 105.70, 107.30],
                  [101.06, 104.14, 107.95, 109.64], [100.86, 103.94, 105.66, 107.34], [104.66, 107.75, 108.00, 109.59],
                  [117.81, 119.39, 112.06, 115.14]])
            usb_part(c[0], c[1], c[2], c[3], 0, 0.9);
        usb_part(107.86, 112.14, 109.20, 112.80, 0, 1.45);
        usb_part(112.20, 117.00, 112.86, 114.94, 0, 1.35);
    }
    if (name == "usb_plug")                  // 12 x 6.5 mm overmold, from the mouth out through the rear wall
        translate([USB_XC, USB_REAR + 0.4 + 0.3, USB_AXIS_Z]) rotate([-90, 0, 0])
            linear_extrude(30) hull() for (s = [-1, 1]) translate([s * 2.75, 0]) circle(d = 6.5);
    if (name == "jmod") at_deck()            // J_MOD on the carrier top, cable entry at the left end
        dbox(CARRIER_EDGE_X + 0.3, CARRIER_EDGE_X + 5.6, JMOD[1] - 10.6, JMOD[1] + 10.6, CARRIER_TOP - JMOD_H, CARRIER_TOP);
    if (name == "jmod_latch") at_deck()     // the slide latch pulled out to open
        dbox(CARRIER_EDGE_X - 1.5, CARRIER_EDGE_X + 0.3, JMOD[1] - 9.6, JMOD[1] + 9.6, CARRIER_TOP - JMOD_H, CARRIER_TOP);

    // --- input module V3 (world frame); MX and HE share outline, holes and J1 ---
    if (name == "mod_pcb") difference() {
        translate([MODULE_OUTLINE[0], MODULE_OUTLINE[1], MX_BOT])
            cube([MODULE_OUTLINE[2] - MODULE_OUTLINE[0], MODULE_OUTLINE[3] - MODULE_OUTLINE[1], MODULE_PCB_T]);
        for (h = MODULE_HOLES) translate([h[0], h[1], MX_BOT - 1]) cylinder(h = MODULE_PCB_T + 2, d = 2.2);   // M2 NPTH
    }
    if (name == "mod_j1")
        translate([MODULE_J1[0], MODULE_J1[1], MX_BOT - MODULE_J1_BELOW])
            cube([MODULE_J1[2] - MODULE_J1[0], MODULE_J1[3] - MODULE_J1[1], MODULE_J1_BELOW]);
    if (name == "mx_sockets") for (k = KEY_X)   // Kailh courtyard under each key
        translate([k - 8.65, KEY_Y - 7.5, MX_BOT - MODULE_BELOW]) cube([16.15, 15.0, MODULE_BELOW]);
    if (name == "mx_parts") for (p = [[132.6, 71.4], [132.6, 73.0], [132.6, 74.6], [161.0, 71.4], [161.0, 73.0], [161.0, 74.6]])
        mod_part(p, 1.8, 1.0, 0.95);
    if (name == "mx_switch_pins") for (k = KEY_X) {   // centre pole and pegs through the PCB
        translate([k, KEY_Y, MX_BOT - 1.4]) cylinder(h = 1.4, r = 2.0);
        for (s = [-1, 1]) translate([k + s*5.08, KEY_Y, MX_BOT - 1.4]) cylinder(h = 1.4, r = 0.9);
    }
    if (name == "he_parts") {
        for (k = KEY_X) {
            translate([k - 1.6, KEY_Y - 1.6, MX_BOT - 1.2]) cube([3.2, 3.2, 1.2]);   // DRV5055
            for (s = [-1, 1]) translate([k + s*5.08, KEY_Y, MX_BOT - 1.1]) cylinder(h = 1.1, r = 0.9);   // switch pegs
        }
        mod_part([168.0, 73.6], 3.4, 3.4, 0.8);                    // ADC
        mod_part([133.2, 77.4], 3.2, 3.0, 1.45);                   // LDO
        mod_part([160.6, 74.0], 3.2, 3.0, 1.45);                   // ESD
        for (p = [[134.4, 73.6], [136.0, 73.6], [136.0, 77.4], [141.6, 76.6], [141.6, 78.4], [145.0, 76.6],
                  [169.0, 77.3], [167.4, 77.3], [165.8, 77.3], [164.2, 73.0], [164.2, 76.6],
                  [172.0, 75.6], [173.6, 75.6], [175.2, 75.6], [172.0, 71.8], [173.6, 71.8], [175.2, 71.8]])
            mod_part(p, 1.8, 1.8, 0.95);
        for (k = KEY_X) translate([k - 3.0 - 0.5, KEY_Y + 2.5 - 0.9, MX_BOT - 0.95]) cube([1.0, 1.8, 0.95]);
    }
    if (name == "switches") for (k = KEY_X) {
        translate([k - 7, KEY_Y - 7, MODULE_PCB_TOP]) cube([14, 14, KEY_DECK_H - PLATE_T - MODULE_PCB_TOP]);
        translate([k - 7.8, KEY_Y - 7.8, KEY_DECK_H + 0.01]) cube([15.6, 15.6, 6.6]);
    }
    if (name == "keycaps") for (k = KEY_X)   // skirt at full travel, conservative
        translate([k - 9.1, KEY_Y - 9.1, KEY_DECK_H + 1.5]) cube([18.2, 18.2, 14]);

    // --- the module cable (FFC, 15 mm wide), with its 45 deg fold under the carrier's window ---
    if (name == "cable") {
        ribbon(RIBBON);
        ribbon(RIBBON2);
        translate([FFC_FOLD[0] - FFC_W/2, _YD - FFC_W/2, FFC_FLOOR_Z - FFC_T/2]) cube([FFC_W, FFC_W, 2 * FFC_T]);
    }
    // --- the USB cable (FFC, 9 mm wide), with its 45 deg fold beside the stack ---
    if (name == "usb_cable") {
        ribbon(URIBBON, FFC8_W);
        ribbon(URIBBON2, FFC8_W);
        at_deck() translate([USB_FOLD_X - FFC8_W/2, JUSB_A - FFC8_W/2, -USB_RUN_D - FFC_T/2]) cube([FFC8_W, FFC8_W, 2 * FFC_T]);
    }
}

REF_NAMES = ["gasket", "ws_glass", "ws_module", "ws_back", "sockets", "carrier", "tails", "carrier_top_parts",
             "jusb", "jusb_latch", "jmod", "jmod_latch", "mod_pcb", "mod_j1",
             "mx_sockets", "mx_parts", "mx_switch_pins", "he_parts", "switches", "keycaps", "cable",
             "usb_pcb", "usb_jack", "usb_legs", "usb_j2", "usb_j2_latch", "usb_parts", "usb_plug", "usb_cable"];

// Pairs of reference parts that must not touch each other either (the case is checked against all)
REF_PAIRS = [["cable", "carrier"], ["cable", "tails"], ["cable", "sockets"], ["cable", "ws_back"],
             ["cable", "jmod"], ["cable", "usb_c"], ["cable", "mx_sockets"], ["cable", "he_parts"],
             ["cable", "mx_parts"], ["cable", "mod_pcb"], ["cable", "carrier_top_parts"],
             ["jmod", "ws_back"], ["jmod", "sockets"], ["carrier_top_parts", "ws_back"],
             ["jusb", "ws_back"], ["jusb", "sockets"], ["jusb", "carrier_top_parts"],
             ["usb_cable", "carrier"], ["usb_cable", "jusb"], ["usb_cable", "ws_back"], ["usb_cable", "ws_module"],
             ["usb_cable", "sockets"], ["usb_cable", "usb_pcb"], ["usb_cable", "usb_jack"], ["usb_cable", "usb_j2"],
             ["usb_cable", "usb_parts"], ["usb_cable", "mod_pcb"], ["usb_cable", "cable"], ["usb_cable", "carrier_top_parts"],
             ["usb_jack", "carrier"], ["usb_j2", "carrier"], ["usb_j2", "tails"], ["usb_parts", "carrier"],
             ["usb_pcb", "carrier"], ["usb_pcb", "tails"], ["usb_jack", "tails"], ["usb_plug", "carrier"],
             ["usb_pcb", "cable"], ["usb_j2_latch", "carrier"]];
