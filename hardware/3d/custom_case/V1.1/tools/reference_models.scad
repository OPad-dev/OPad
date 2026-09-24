// =============================================================================
//  Reference models for the V1.1 fit check: what goes inside the case, as
//  simple envelopes in world coordinates. Include after osupad_params.scad.
//  Sources: Waveshare 2024-11-08 drawing + STEP (waveshare_back.scad),
//  hardware/pcb/V1 (carrier, MX and HE modules).
// =============================================================================

include <waveshare_back.scad>

CARRIER_EDGE_X = WS_PCB_XC - 24.1;   // non-USB end of the carrier (deck x)
MX_BOT = MODULE_PCB_BOT;

// JST SH cable from the module's J1 to the carrier's J_MOD
function deck_to_world(p) = [CASE_W/2 + p[0],
                             Y_C + p[1]*cos(TILT) - p[2]*sin(TILT),
                             Z_C + p[1]*sin(TILT) + p[2]*cos(TILT)];
JMOD_PLUG_END = deck_to_world([CARRIER_EDGE_X - 4.6, 0, -(CARRIER_BOT + 1.45)]);
CABLE_PATH = [[CASE_W/2, 37.5, MX_BOT - 1.5],
              [CASE_W/2, 39.5, 4.6],
              [7.5, 39.5, 4.6],
              [7.5, JMOD_PLUG_END[1], JMOD_PLUG_END[2] - 0.5],
              [JMOD_PLUG_END[0] - 0.5, JMOD_PLUG_END[1], JMOD_PLUG_END[2]]];

module dbox(x0, x1, a0, a1, d0, d1) translate([x0, a0, -d1]) cube([x1 - x0, a1 - a0, d1 - d0]);

module ref(name) {
    // --- screen stack (deck frame) ---
    if (name == "ws_glass") at_deck()
        translate([0, 0, -GLASS_T]) linear_extrude(GLASS_T)
            offset(GLASS_R) square([GLASS_X - 2*GLASS_R, GLASS_Y - 2*GLASS_R], center = true);
    if (name == "ws_module") at_deck()
        dbox(WS_PCB_XC - WS_PCB_X/2, WS_PCB_XC + WS_PCB_X/2, -WS_PCB_Y/2, WS_PCB_Y/2, GLASS_T, WS_BACK);
    if (name == "ws_back") at_deck() for (b = WS_BACK_BOXES) dbox(b[0], b[1], b[2], b[3], b[4], b[5]);
    if (name == "sockets") at_deck() for (s = [-1, 1])
        dbox(WS_PCB_XC - 17.78, WS_PCB_XC + 17.78, s*ROW_A - 1.27, s*ROW_A + 1.27, WS_BACK + HDR_PLASTIC, CARRIER_TOP);
    if (name == "carrier") at_deck() difference() {
        dbox(WS_PCB_XC - 24.1, WS_PCB_XC + 24.1, -CARRIER_A, CARRIER_A, CARRIER_TOP, CARRIER_BOT);
        dbox(WS_PCB_XC + 24.1 - 34.7, WS_PCB_XC + 30, -11.4, 11.4, CARRIER_TOP - 1, CARRIER_BOT + 1);
    }
    if (name == "tails") at_deck() for (s = [-1, 1])
        dbox(WS_PCB_XC - 17.2, WS_PCB_XC + 17.2, s*ROW_A - 0.9, s*ROW_A + 0.9, CARRIER_BOT, CARRIER_BOT + TAIL);
    if (name == "screw_heads_carrier") at_deck() for (h = WS_HOLES)   // optional M2 spacer screws
        translate([h[0], h[1], -(CARRIER_BOT + 1.6)]) cylinder(h = 1.6, r = 1.9);
    if (name == "jmod") at_deck()
        dbox(CARRIER_EDGE_X - 0.1, CARRIER_EDGE_X + 4.5, -5.6, 5.6, CARRIER_BOT, CARRIER_BOT + JMOD_H);
    if (name == "jmod_plug") at_deck()
        dbox(CARRIER_EDGE_X - 4.6, CARRIER_EDGE_X, -4.3, 4.3, CARRIER_BOT + 0.1, CARRIER_BOT + 2.8);
    if (name == "usb_plug") at_deck()   // 12 x 6.5 mm overmold, from the receptacle out through the wall
        dbox(USB_X + 0.3, USB_X + 30, -6, 6, USB_D - 3.25, USB_D + 3.25);

    // --- input module (world frame); MX and HE share outline, holes and J1 ---
    if (name == "mod_pcb") difference() {
        translate([MODULE_OUTLINE[0], MODULE_OUTLINE[1], MX_BOT])
            cube([MODULE_OUTLINE[2] - MODULE_OUTLINE[0], MODULE_OUTLINE[3] - MODULE_OUTLINE[1], MODULE_PCB_T]);
        for (h = MODULE_HOLES) translate([h[0], h[1], MX_BOT - 1]) cylinder(h = MODULE_PCB_T + 2, d = 2.2);   // M2 NPTH
    }
    if (name == "mod_j1")
        translate([MODULE_J1[0], MODULE_J1[1], MX_BOT - MODULE_J1_BELOW])
            cube([MODULE_J1[2] - MODULE_J1[0], MODULE_J1[3] - MODULE_J1[1], MODULE_J1_BELOW]);
    if (name == "mod_plug") translate([33.5, 31.0, MX_BOT - 2.8]) cube([9, 5.5, 2.7]);
    if (name == "mx_sockets") for (k = KEY_X)
        translate([k - 8.4, KEY_Y - 1.0, MX_BOT - MODULE_BELOW]) cube([15.6, 8.5, MODULE_BELOW]);
    if (name == "mx_switch_pins") for (k = KEY_X) {   // centre pole and pegs through the PCB
        translate([k, KEY_Y, MX_BOT - 1.4]) cylinder(h = 1.4, r = 2.0);
        for (s = [-1, 1]) translate([k + s*5.08, KEY_Y, MX_BOT - 1.4]) cylinder(h = 1.4, r = 0.9);
    }
    if (name == "he_parts") {
        for (k = KEY_X) {
            translate([k - 1.6, KEY_Y - 1.6, MX_BOT - 1.2]) cube([3.2, 3.2, 1.2]);   // DRV5055
            for (s = [-1, 1]) translate([k + s*5.08, KEY_Y, MX_BOT - 1.1]) cylinder(h = 1.1, r = 0.9);   // switch pins
        }
        for (p = [[31.0, 19.0], [50.05, 19.0], [30.5, 25.8], [45.5, 28.0], [45.5, 26.0]])
            translate([p[0] - 0.9, p[1] - 0.5, MX_BOT - 0.95]) cube([1.8, 1.0, 0.95]);
    }
    if (name == "switches") for (k = KEY_X) {
        translate([k - 7, KEY_Y - 7, MODULE_PCB_TOP]) cube([14, 14, KEY_DECK_H - PLATE_T - MODULE_PCB_TOP]);
        translate([k - 7.8, KEY_Y - 7.8, KEY_DECK_H + 0.01]) cube([15.6, 15.6, 6.6]);
    }
    if (name == "keycaps") for (k = KEY_X)   // skirt at full travel, conservative
        translate([k - 9.1, KEY_Y - 9.1, KEY_DECK_H + 1.5]) cube([18.2, 18.2, 14]);

    // --- cable ---
    if (name == "cable") for (i = [0 : len(CABLE_PATH) - 2]) hull() {
        translate(CABLE_PATH[i]) sphere(r = 1.4, $fn = 12);
        translate(CABLE_PATH[i + 1]) sphere(r = 1.4, $fn = 12);
    }
}

REF_NAMES = ["ws_glass", "ws_module", "ws_back", "sockets", "carrier", "tails", "screw_heads_carrier",
             "jmod", "jmod_plug", "usb_plug", "mod_pcb", "mod_j1", "mod_plug",
             "mx_sockets", "mx_switch_pins", "he_parts", "switches", "keycaps", "cable"];
