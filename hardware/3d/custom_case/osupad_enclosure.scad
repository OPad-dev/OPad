// =============================================================================
//  osu!pad ESP32-S3 Parametric Enclosure
//  Enclosure for Waveshare ESP32-S3-Touch-LCD-2 and 2x Cherry/Gateron/Kailh MX Switches
// =============================================================================

// --- Configuration Parameters (all units in millimeters) ---
$fn = 60; // Circle smoothness

// MX Switch Specifications
MX_CUTOUT_W         = 14.0;   // Standard Cherry MX cutout width
MX_CUTOUT_H         = 14.0;   // Standard Cherry MX cutout height
MX_PLATE_THICKNESS  = 1.5;    // Standard retention clip thickness (1.5mm)
MX_SPACING          = 19.05;  // Standard 1u key spacing (center to center)

// Waveshare 2.0" LCD ESP32-S3 Board Specifications (from official blueprint)
WS_LENS_W           = 37.5;   // Glass lens width (+0.4mm tolerance)
WS_LENS_L           = 59.2;   // Glass lens length (+0.4mm tolerance)
WS_LENS_CORNER_R    = 2.6;    // Glass lens corner radius
WS_VA_W             = 31.5;   // Active view area width
WS_VA_L             = 41.5;   // Active view area length
WS_PCB_W            = 35.4;   // PCB width (+0.4mm tolerance)
WS_PCB_L            = 48.6;   // PCB length (+0.4mm tolerance)
WS_MOUNT_X_DIST     = 29.0;   // M2 mounting hole horizontal pitch
WS_MOUNT_Y_DIST     = 42.2;   // M2 mounting hole vertical pitch
WS_TOTAL_THICKNESS  = 10.0;   // Board + LCD depth clearance

// Enclosure Dimensions
CASE_W              = 78.0;   // Outer width
CASE_L              = 105.0;  // Outer length
CASE_H_FRONT        = 18.0;   // Front height (ergonomic low profile)
CASE_H_BACK         = 28.0;   // Back height (gives ~8° ergonomic incline)
WALL_THICKNESS      = 2.4;    // Outer shell wall thickness
CORNER_RADIUS       = 6.0;    // Outer enclosure corner fillet

// Part selector: "assembly", "top_case", "bottom_plate"
PART = "assembly";

module rounded_rect(w, l, h, r) {
    hull() {
        translate([r, r, 0]) cylinder(h=h, r=r);
        translate([w - r, r, 0]) cylinder(h=h, r=r);
        translate([r, l - r, 0]) cylinder(h=h, r=r);
        translate([w - r, l - r, 0]) cylinder(h=h, r=r);
    }
}

// Standard MX switch socket cutout with side retention notches
module mx_switch_cutout() {
    union() {
        // Main 14x14 mm square through-hole
        translate([-MX_CUTOUT_W/2, -MX_CUTOUT_H/2, -5])
            cube([MX_CUTOUT_W, MX_CUTOUT_H, 15]);

        // Side clip relief notches (for Cherry MX snap-in tabs)
        translate([-14.5/2, -3.5/2, 0])
            cube([14.5, 3.5, MX_PLATE_THICKNESS + 0.2]);
        translate([-3.5/2, -14.5/2, 0])
            cube([3.5, 14.5, MX_PLATE_THICKNESS + 0.2]);

        // Underside body clearance
        translate([-16/2, -16/2, -12])
            cube([16, 16, 12 - MX_PLATE_THICKNESS]);
    }
}

// Waveshare 2.0" LCD pocket & view window
module waveshare_screen_cutout() {
    union() {
        // Active view window cutout (through top plate)
        translate([-WS_VA_L/2, -WS_VA_W/2, -5])
            cube([WS_VA_L, WS_VA_W, 10]);

        // Recessed pocket for glass lens (flush or 0.8mm recessed)
        translate([-WS_LENS_L/2, -WS_LENS_W/2, -1.2])
            rounded_rect(WS_LENS_L, WS_LENS_W, 3, WS_LENS_CORNER_R);

        // Internal cavity for PCB and components
        translate([-WS_PCB_L/2 - 1, -WS_PCB_W/2 - 1, -WS_TOTAL_THICKNESS - 1.2])
            cube([WS_PCB_L + 2, WS_PCB_W + 2, WS_TOTAL_THICKNESS]);
    }
}

// USB-C cable access port
module usbc_port_cutout() {
    hull() {
        translate([-6, 0, -3.5]) rotate([90, 0, 0]) cylinder(h=15, r=3.5, center=true);
        translate([6, 0, -3.5]) rotate([90, 0, 0]) cylinder(h=15, r=3.5, center=true);
    }
}

// Top Case Enclosure
module top_case() {
    difference() {
        // Solid wedge enclosure with rounded corners
        hull() {
            // Front low profile edge
            translate([CORNER_RADIUS, CORNER_RADIUS, 0])
                cylinder(h=CASE_H_FRONT, r=CORNER_RADIUS);
            translate([CASE_W - CORNER_RADIUS, CORNER_RADIUS, 0])
                cylinder(h=CASE_H_FRONT, r=CORNER_RADIUS);

            // Back inclined edge
            translate([CORNER_RADIUS, CASE_L - CORNER_RADIUS, 0])
                cylinder(h=CASE_H_BACK, r=CORNER_RADIUS);
            translate([CASE_W - CORNER_RADIUS, CASE_L - CORNER_RADIUS, 0])
                cylinder(h=CASE_H_BACK, r=CORNER_RADIUS);
        }

        // Hollow interior cavity (leave 2.4mm walls & 3.0mm top plate)
        translate([WALL_THICKNESS, WALL_THICKNESS, -1])
            hull() {
                translate([CORNER_RADIUS/2, CORNER_RADIUS/2, 0])
                    cylinder(h=CASE_H_FRONT - 3.0, r=CORNER_RADIUS/2);
                translate([CASE_W - 2*WALL_THICKNESS - CORNER_RADIUS/2, CORNER_RADIUS/2, 0])
                    cylinder(h=CASE_H_FRONT - 3.0, r=CORNER_RADIUS/2);
                translate([CORNER_RADIUS/2, CASE_L - 2*WALL_THICKNESS - CORNER_RADIUS/2, 0])
                    cylinder(h=CASE_H_BACK - 3.0, r=CORNER_RADIUS/2);
                translate([CASE_W - 2*WALL_THICKNESS - CORNER_RADIUS/2, CASE_L - 2*WALL_THICKNESS - CORNER_RADIUS/2, 0])
                    cylinder(h=CASE_H_BACK - 3.0, r=CORNER_RADIUS/2);
            }

        // 2x MX Switch Cutouts (Bottom Front Area - ergonomic wrist position)
        // Key 1 (Z)
        translate([CASE_W/2 - MX_SPACING/2, 28, CASE_H_FRONT - 2.5])
            mx_switch_cutout();

        // Key 2 (X)
        translate([CASE_W/2 + MX_SPACING/2, 28, CASE_H_FRONT - 2.5])
            mx_switch_cutout();

        // Waveshare Screen Cutout (Top Rear Area - oriented landscape)
        translate([CASE_W/2, 70, CASE_H_BACK - 3.5])
            waveshare_screen_cutout();

        // Rear USB-C Port Cutout
        translate([CASE_W/2, CASE_L, 8])
            usbc_port_cutout();

        // 4x Bottom screw assembly holes (M3 counterbore)
        for (pos = [[8, 8], [CASE_W - 8, 8], [8, CASE_L - 8], [CASE_W - 8, CASE_L - 8]]) {
            translate([pos[0], pos[1], -1])
                cylinder(h=12, r=1.6); // M3 tap hole
        }
    }
}

// Bottom Base Plate
module bottom_plate() {
    difference() {
        // Solid bottom plate with lip
        rounded_rect(CASE_W - 0.4, CASE_L - 0.4, 2.8, CORNER_RADIUS - 0.2);

        // 4x M3 Countersunk Screws
        for (pos = [[8, 8], [CASE_W - 8, 8], [8, CASE_L - 8], [CASE_W - 8, CASE_L - 8]]) {
            translate([pos[0], pos[1], -1])
                cylinder(h=6, r=1.7); // M3 clearance
            translate([pos[0], pos[1], 1.2])
                cylinder(h=3, r1=1.7, r2=3.2); // Countersink
        }

        // 4x Anti-slip rubber foot circular recesses (10mm diameter, 1.2mm deep)
        for (pos = [[16, 16], [CASE_W - 16, 16], [16, CASE_L - 16], [CASE_W - 16, CASE_L - 16]]) {
            translate([pos[0], pos[1], -0.1])
                cylinder(h=1.2, r=5.0);
        }
    }
}

// Render selection
if (PART == "assembly") {
    color([0.2, 0.2, 0.2, 0.9]) top_case();
    translate([0.2, 0.2, -4]) color([0.1, 0.1, 0.1, 0.95]) bottom_plate();
} else if (PART == "top_case") {
    top_case();
} else if (PART == "bottom_plate") {
    bottom_plate();
}
