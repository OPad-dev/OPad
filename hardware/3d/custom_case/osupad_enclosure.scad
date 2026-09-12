// =============================================================================
//  osu!pad ESP32-S3 Screwless Ergonomic Enclosure
//  - Flat front deck (0°) for 2x mechanical MX switches (Z & X)
//  - 12.5° Angled rear deck for Waveshare 2.0" Touch LCD (Landscape)
//  - USB-C port cutout with lead-in on top-right of right side wall
//  - 100% Screwless Snap-Fit Cantilever closure mechanism
//  - Internal PCB support cradle & anti-slip rubber foot recesses
// =============================================================================

$fn = 60; // Smooth curves

// --- Dimensions & Clearances (mm) ---

// MX Mechanical Switch Specifications
MX_CUTOUT_W         = 14.0;   // Cherry MX hole width
MX_CUTOUT_H         = 14.0;   // Cherry MX hole height
MX_PLATE_T          = 1.5;    // Plate thickness for switch retention clips
MX_PITCH            = 19.05;  // Standard mechanical keycap 1u pitch

// Waveshare ESP32-S3-Touch-LCD-2 Specifications (Landscape)
// Front Glass Lens: 58.8mm (X) x 37.1mm (Y) x 1.1mm (Z)
LCD_LENS_X          = 59.4;   // Glass lens length (+0.6mm tolerance)
LCD_LENS_Y          = 37.7;   // Glass lens width (+0.6mm tolerance)
LCD_LENS_R          = 2.8;    // Glass lens corner radius
LCD_POCKET_DEPTH    = 1.2;    // Recessed flush pocket depth

// Through-cutout for PCB & Module insertion
PCB_THROUGH_X       = 50.0;   // 48.2mm PCB length + tolerance
PCB_THROUGH_Y       = 36.2;   // 35.0mm PCB width + tolerance

// Overall Case Outer Dimensions
CASE_W              = 76.0;   // Total width (X axis)
CASE_L              = 92.0;   // Total length (Y axis)
FRONT_ZONE_L        = 38.0;   // Length of flat key switch zone
REAR_ZONE_L         = 54.0;   // Length of angled display zone (38 to 92mm)
H_FLAT              = 16.0;   // Height of the flat key deck
H_REAR              = 28.0;   // Height at the very rear (gives ~12.5° tilt)
CORNER_R            = 5.0;    // Outer corner fillet radius
WALL_T              = 2.2;    // Outer shell wall thickness
TOLERANCE           = 0.20;   // FDM 3D printing snap-fit clearance

// Snap-fit parameters
SNAP_BEAD_H         = 0.65;   // Snap bead protrusion outward
SNAP_TAB_W          = 9.0;    // Width of flexible cantilever snap tab
SNAP_Z              = 2.6;    // Height above base where snap locks

// Part Selector: "assembly", "top_case", "bottom_plate"
PART = "assembly";

// Calculated Deck Incline Angle
DECK_TILT = atan2(H_REAR - H_FLAT, REAR_ZONE_L); // 12.5288°

// --- Helper Modules ---

module rounded_box(w, l, h, r) {
    hull() {
        translate([r, r, 0]) cylinder(h=h, r=r);
        translate([w - r, r, 0]) cylinder(h=h, r=r);
        translate([r, l - r, 0]) cylinder(h=h, r=r);
        translate([w - r, l - r, 0]) cylinder(h=h, r=r);
    }
}

// 2-tier outer shell hull (Flat front + Angled rear)
module case_outer_hull(extra_inset=0) {
    w = CASE_W - 2 * extra_inset;
    l = CASE_L - 2 * extra_inset;
    r = max(1.0, CORNER_R - extra_inset);

    translate([extra_inset, extra_inset, 0])
    hull() {
        // Flat front deck (Z = H_FLAT)
        translate([r, r, 0]) cylinder(h=H_FLAT, r=r);
        translate([w - r, r, 0]) cylinder(h=H_FLAT, r=r);
        translate([r, FRONT_ZONE_L - extra_inset, 0]) cylinder(h=H_FLAT, r=r);
        translate([w - r, FRONT_ZONE_L - extra_inset, 0]) cylinder(h=H_FLAT, r=r);

        // Angled rear deck (sloping to H_REAR at the back)
        translate([r, l - r, 0]) cylinder(h=H_REAR, r=r);
        translate([w - r, l - r, 0]) cylinder(h=H_REAR, r=r);
    }
}

// MX Switch Snap-in Cutout with retention clip tabs
module mx_switch_cutout() {
    union() {
        // 14.0 x 14.0 mm through-hole for switch body
        translate([-MX_CUTOUT_W/2, -MX_CUTOUT_H/2, -5])
            cube([MX_CUTOUT_W, MX_CUTOUT_H, 15]);

        // North & South clip notches (Cherry/Gateron clip tabs)
        translate([-14.6/2, -3.8/2, -0.1])
            cube([14.6, 3.8, MX_PLATE_T + 0.2]);
        translate([-3.8/2, -14.6/2, -0.1])
            cube([3.8, 14.6, MX_PLATE_T + 0.2]);

        // Underside clearance for switch base & wiring
        translate([-16.0/2, -16.0/2, -15])
            cube([16.0, 16.0, 15 - MX_PLATE_T]);
    }
}

// Screen Cutout for Waveshare Touch LCD
module screen_cutout() {
    union() {
        // 1. Recessed flush pocket for glass lens (outer surface)
        translate([-LCD_LENS_X/2, -LCD_LENS_Y/2, -LCD_POCKET_DEPTH])
            rounded_box(LCD_LENS_X, LCD_LENS_Y, 10, LCD_LENS_R);

        // 2. Through-cutout for PCB and display module into internal cavity
        translate([-PCB_THROUGH_X/2, -PCB_THROUGH_Y/2, -15])
            cube([PCB_THROUGH_X, PCB_THROUGH_Y, 16]);

        // 3. Right-side exit channel connecting to USB-C port
        translate([PCB_THROUGH_X/2 - 2.0, -10.0, -15])
            cube([15.0, 20.0, 16]);
    }
}

// USB-C side wall cutout with 45° lead-in chamfer
module usbc_cutout() {
    union() {
        // Core through-port (14.5mm wide in Y x 7.5mm tall in Z)
        hull() {
            translate([0, -3.75, 0]) rotate([0, 90, 0]) cylinder(h=20, r=3.75, center=true);
            translate([0, 3.75, 0]) rotate([0, 90, 0]) cylinder(h=20, r=3.75, center=true);
        }

        // Exterior lead-in flare / chamfer for wide cable heads
        translate([6.0, 0, 0])
        hull() {
            translate([0, -4.5, 0]) rotate([0, 90, 0]) cylinder(h=8, r=5.0, center=true);
            translate([0, 4.5, 0]) rotate([0, 90, 0]) cylinder(h=8, r=5.0, center=true);
        }
    }
}

// --- Top Case Module ---

module top_case() {
    difference() {
        // 1. Outer Solid Case
        case_outer_hull(0);

        // 2. Main Internal Hollow Cavity (leaves WALL_T perimeter walls)
        translate([WALL_T, WALL_T, -1])
            hull() {
                translate([CORNER_R/2, CORNER_R/2, 0])
                    cylinder(h=H_FLAT - 2.8, r=CORNER_R/2);
                translate([CASE_W - 2*WALL_T - CORNER_R/2, CORNER_R/2, 0])
                    cylinder(h=H_FLAT - 2.8, r=CORNER_R/2);
                translate([CORNER_R/2, FRONT_ZONE_L - WALL_T, 0])
                    cylinder(h=H_FLAT - 2.8, r=CORNER_R/2);
                translate([CASE_W - 2*WALL_T - CORNER_R/2, FRONT_ZONE_L - WALL_T, 0])
                    cylinder(h=H_FLAT - 2.8, r=CORNER_R/2);

                translate([CORNER_R/2, CASE_L - 2*WALL_T - CORNER_R/2, 0])
                    cylinder(h=H_REAR - 2.8, r=CORNER_R/2);
                translate([CASE_W - 2*WALL_T - CORNER_R/2, CASE_L - 2*WALL_T - CORNER_R/2, 0])
                    cylinder(h=H_REAR - 2.8, r=CORNER_R/2);
            }

        // 3. Snap-Fit Internal Latch Grooves (depth 0.8mm into inner wall)
        // Left & Right walls (at Y = 20.0mm front and Y = 83.0mm rear, clear of USB-C)
        for (y_pos = [20.0, 83.0]) {
            // Left inner wall groove
            translate([WALL_T - 0.8, y_pos - (SNAP_TAB_W + 1.0)/2, SNAP_Z])
                cube([1.0, SNAP_TAB_W + 1.0, 1.4]);
            // Right inner wall groove
            translate([CASE_W - WALL_T - 0.2, y_pos - (SNAP_TAB_W + 1.0)/2, SNAP_Z])
                cube([1.0, SNAP_TAB_W + 1.0, 1.4]);
        }
        // Front & Rear walls (1 on each side)
        // Front inner wall groove
        translate([CASE_W/2 - (SNAP_TAB_W + 1.0)/2, WALL_T - 0.8, SNAP_Z])
            cube([SNAP_TAB_W + 1.0, 1.0, 1.4]);
        // Rear inner wall groove
        translate([CASE_W/2 - (SNAP_TAB_W + 1.0)/2, CASE_L - WALL_T - 0.2, SNAP_Z])
            cube([SNAP_TAB_W + 1.0, 1.0, 1.4]);

        // 4. MX Switch Cutouts on FLAT Front Deck (0° horizontal)
        // Key 1 (Z) - Left
        translate([CASE_W/2 - MX_PITCH/2, 19.0, H_FLAT])
            mx_switch_cutout();

        // Key 2 (X) - Right
        translate([CASE_W/2 + MX_PITCH/2, 19.0, H_FLAT])
            mx_switch_cutout();

        // 5. Waveshare 2.0" LCD on ANGLED Rear Deck
        // Deck surface point at Y = 64.0mm is at Z = H_FLAT + 26.0 * tan(DECK_TILT) = 21.78mm
        translate([CASE_W/2, FRONT_ZONE_L + 26.0, H_FLAT + 26.0 * tan(DECK_TILT)])
            rotate([DECK_TILT, 0, 0])
                screen_cutout();

        // 6. USB-C Port Cutout on RIGHT SIDE WALL (aligned with board)
        // Center at Y = 65.8mm, Z = 13.6mm
        translate([CASE_W - WALL_T/2, 65.8, 13.6])
            usbc_cutout();

        // 7. Rear Pry Notch (for opening with a coin or pick without damage)
        translate([CASE_W/2 - 6.0, CASE_L - 2.0, -0.1])
            cube([12.0, 4.0, 2.2]);
    }
}

// --- Bottom Plate Module (100% Screwless Snap-Fit) ---

module bottom_plate() {
    inner_w = CASE_W - 2 * WALL_T - 2 * TOLERANCE;
    inner_l = CASE_L - 2 * WALL_T - 2 * TOLERANCE;
    rim_r   = max(1.0, CORNER_R - WALL_T - TOLERANCE);
    rim_h   = 4.6; // Rim height above base

    union() {
        // 1. Base Plate Bottom Shell (flush with desk and outer case)
        difference() {
            // Main bottom plate
            rounded_box(CASE_W, CASE_L, 2.0, CORNER_R);

            // 4x Non-slip rubber foot circular recesses (10mm dia, 1.0mm deep)
            for (pos = [[13, 13], [CASE_W - 13, 13], [13, CASE_L - 13], [CASE_W - 13, CASE_L - 13]]) {
                translate([pos[0], pos[1], -0.1])
                    cylinder(h=1.1, r=5.0);
            }

            // Rear pry notch coin relief
            translate([CASE_W/2 - 6.0, CASE_L - 3.0, -0.1])
                cube([12.0, 5.0, 1.4]);
        }

        // 2. Interlocking Perimeter Rim with Vertical Cantilever Relief Cuts
        translate([WALL_T + TOLERANCE, WALL_T + TOLERANCE, 2.0])
        difference() {
            // Solid outer rim flange
            rounded_box(inner_w, inner_l, rim_h, rim_r);

            // Hollow interior (1.4mm rim wall thickness)
            translate([1.4, 1.4, -0.1])
                rounded_box(inner_w - 2.8, inner_l - 2.8, rim_h + 1.0, max(0.5, rim_r - 1.4));

            // Vertical relief cuts on sides of each cantilever snap tab
            // Left & Right tabs (at Y = 20.0mm and Y = 83.0mm)
            for (y_pos = [20.0 - (WALL_T + TOLERANCE), 83.0 - (WALL_T + TOLERANCE)]) {
                // Left wall relief cuts (two 1.0mm slits)
                translate([-0.5, y_pos - SNAP_TAB_W/2 - 1.0, -0.1])
                    cube([2.5, 1.0, rim_h + 1.0]);
                translate([-0.5, y_pos + SNAP_TAB_W/2, -0.1])
                    cube([2.5, 1.0, rim_h + 1.0]);

                // Right wall relief cuts (two 1.0mm slits)
                translate([inner_w - 2.0, y_pos - SNAP_TAB_W/2 - 1.0, -0.1])
                    cube([2.5, 1.0, rim_h + 1.0]);
                translate([inner_w - 2.0, y_pos + SNAP_TAB_W/2, -0.1])
                    cube([2.5, 1.0, rim_h + 1.0]);
            }
            // Front & Rear tabs
            // Front wall relief cuts
            translate([inner_w/2 - SNAP_TAB_W/2 - 1.0, -0.5, -0.1])
                cube([1.0, 2.5, rim_h + 1.0]);
            translate([inner_w/2 + SNAP_TAB_W/2, -0.5, -0.1])
                cube([1.0, 2.5, rim_h + 1.0]);
            // Rear wall relief cuts
            translate([inner_w/2 - SNAP_TAB_W/2 - 1.0, inner_l - 2.0, -0.1])
                cube([1.0, 2.5, rim_h + 1.0]);
            translate([inner_w/2 + SNAP_TAB_W/2, inner_l - 2.0, -0.1])
                cube([1.0, 2.5, rim_h + 1.0]);
        }

        // 3. Cantilever Snap Beads (with lead-in chamfer and retaining shoulder)
        // Height aligned with top case latch groove at Z_top = SNAP_Z (2.6mm) -> Z_plate = 2.0 + 2.6 = 4.6mm
        // Overlap by 0.5mm into the tab wall to guarantee 100% manifold solid!
        // Left & Right beads (at Y = 20.0mm and Y = 83.0mm)
        for (y_pos = [20.0, 83.0]) {
            // Left bead (protrudes towards -X)
            translate([WALL_T + TOLERANCE - SNAP_BEAD_H, y_pos - SNAP_TAB_W/2, 2.0 + SNAP_Z])
                hull() {
                    cube([SNAP_BEAD_H + 0.5, SNAP_TAB_W, 0.4]);
                    translate([SNAP_BEAD_H * 0.4, 0, 0.7])
                        cube([SNAP_BEAD_H * 0.6 + 0.5, SNAP_TAB_W, 0.3]);
                }
            // Right bead (protrudes towards +X)
            translate([CASE_W - WALL_T - TOLERANCE - 0.5, y_pos - SNAP_TAB_W/2, 2.0 + SNAP_Z])
                hull() {
                    cube([SNAP_BEAD_H + 0.5, SNAP_TAB_W, 0.4]);
                    translate([0, 0, 0.7])
                        cube([SNAP_BEAD_H * 0.6 + 0.5, SNAP_TAB_W, 0.3]);
                }
        }
        // Front & Rear beads
        // Front bead (protrudes towards -Y)
        translate([CASE_W/2 - SNAP_TAB_W/2, WALL_T + TOLERANCE - SNAP_BEAD_H, 2.0 + SNAP_Z])
            hull() {
                cube([SNAP_TAB_W, SNAP_BEAD_H + 0.5, 0.4]);
                translate([0, SNAP_BEAD_H * 0.4, 0.7])
                    cube([SNAP_TAB_W, SNAP_BEAD_H * 0.6 + 0.5, 0.3]);
            }
        // Rear bead (protrudes towards +Y)
        translate([CASE_W/2 - SNAP_TAB_W/2, CASE_L - WALL_T - TOLERANCE - 0.5, 2.0 + SNAP_Z])
            hull() {
                cube([SNAP_TAB_W, SNAP_BEAD_H + 0.5, 0.4]);
                translate([0, 0, 0.7])
                    cube([SNAP_TAB_W, SNAP_BEAD_H * 0.6 + 0.5, 0.3]);
            }

        // 4. Screen Rear Support Pillars (aligned with Waveshare brass standoffs)
        // Front PCB edge support (at Y = 49.5mm, h = 6.8mm)
        translate([18.6, 49.5, 2.0])
            cylinder(h=6.8, r1=2.5, r2=2.0);
        translate([60.8, 49.5, 2.0])
            cylinder(h=6.8, r1=2.5, r2=2.0);

        // Rear PCB edge support (at Y = 78.5mm, h = 13.2mm)
        translate([18.6, 78.5, 2.0])
            cylinder(h=13.2, r1=2.5, r2=2.0);
        translate([60.8, 78.5, 2.0])
            cylinder(h=13.2, r1=2.5, r2=2.0);
    }
}

// --- Render Selection ---
if (PART == "assembly") {
    color([0.22, 0.22, 0.25, 0.95]) top_case();
    translate([0, 0, -4.0]) color([0.15, 0.15, 0.16, 1.0]) bottom_plate();
} else if (PART == "top_case") {
    top_case();
} else if (PART == "bottom_plate") {
    bottom_plate();
}
