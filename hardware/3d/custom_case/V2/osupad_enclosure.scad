// =============================================================================
//  osu!pad ESP32-S3 Screwless Ergonomic Enclosure (V2)
//  - Flat front deck (0°) for 2x mechanical MX switches (Z & X)
//  - Steeper 30° rear deck; the Waveshare 2.0" Touch LCD glass rests ON TOP of the deck,
//    with the module body dropping through a snug cutout (dimensions from the Waveshare STEP)
//  - USB-C port in the right side wall, aligned and tilted with the board's receptacle
//  - 100% Screwless Snap-Fit Cantilever closure mechanism
//  - Ballast trays and deskmat anti-slip diamond tread on the bottom plate
// =============================================================================

$fn = 60; // Smooth curves

// --- Dimensions & Clearances (mm) ---

// MX Mechanical Switch Specifications
MX_CUTOUT_W         = 14.0;   // Cherry MX hole width
MX_CUTOUT_H         = 14.0;   // Cherry MX hole height
MX_PLATE_T          = 1.5;    // Plate thickness for switch retention clips
MX_PITCH            = 19.05;  // Standard mechanical keycap 1u pitch

// Waveshare ESP32-S3-Touch-LCD-2, measured from waveshare_board/esp32-s3-touch-lcd-2_20241108.stp
// Landscape: X = along the board's long side (USB-C end towards +X / right wall),
// Y = up the slope, Z = out of the deck. Origin = glass centre on its underside.
LCD_GLASS_X         = 59.22;  // Cover glass (the drawing's 58.8 x 37.1 is the top face; the edge is wider)
LCD_GLASS_Y         = 37.52;
LCD_GLASS_T         = 1.1;
LCD_BODY_X          = 48.22;  // Everything under the glass fits inside the PCB outline...
LCD_BODY_Y          = 35.02;
LCD_BODY_OFFSET_X   = 0.21;   // ...whose centre sits this far towards the USB-C end
LCD_USB_OVERHANG    = 1.18;   // USB-C receptacle sticks out past the PCB edge
LCD_USB_Z           = -7.27;  // Receptacle axis below the glass underside
LCD_FIT             = 0.40;   // Clearance per side for the module body cutout
LCD_CUT_DEPTH       = 10.0;   // Cutout depth below the deck surface (deck is thinner)
SCREEN_Y            = 47.0;   // Glass centre, horizontal distance from the front edge. As low as the
                              // board's header pin tips allow (they end ~1.5 mm above the base)
SCREEN_REAR_MARGIN  = 2.5;    // Slope left behind the glass before the flat rear shelf

// Overall Case Outer Dimensions
CASE_W              = 76.0;   // Total width (X axis)
CASE_L              = 86.0;   // Total length (Y axis)
Y_SPLIT             = 32.0;   // Boundary where flat deck ends and inclination begins
H_FLAT              = 11.0;   // Key deck top = switch plate top (13 mm above the desk with the base).
                              // Under the plate: MX body 5.0 + pins 3.3 (hand-wired) or
                              // PCB 1.6 + hot-swap socket 1.85 -> ~8.5 mm, leaving ~2.5 mm above the base
DECK_TILT           = 30.0;   // Screen deck angle (V1: ~20°)
CORNER_R            = 4.5;    // Outer corner fillet radius
WALL_T              = 2.2;    // Outer shell wall thickness
TOLERANCE           = 0.20;   // FDM 3D printing snap-fit clearance

// Shell profile: flat key deck -> 30° screen slope -> flat rear shelf just behind the glass.
// The outer hull is a hull of corner cylinders, so the slope starts at the front edge of the
// Y_SPLIT cylinders (Y_SPLIT - CORNER_R) and ends at the front edge of the shelf cylinders.
INCLINE_START       = Y_SPLIT - CORNER_R;                     // 27.5 mm
FLAT_DECK_T         = 3.0;    // Key deck thickness (the switch plate itself is thinned to 1.5)
DECK_T              = 2.8;    // Screen deck thickness, measured perpendicular to the slope
SCREEN_Z            = H_FLAT + (SCREEN_Y - INCLINE_START) * tan(DECK_TILT);
H_REAR              = SCREEN_Z + (LCD_GLASS_Y/2 + SCREEN_REAR_MARGIN) * sin(DECK_TILT); // shelf height
Y_SHELF             = INCLINE_START + (H_REAR - H_FLAT) / tan(DECK_TILT);             // shelf start

// Future hot-swap key PCB: space kept free under the keys (see hardware/3d/README.md)
KEY_PCB_KEEPOUT     = [18.0, 6.0, 40.0, 21.0];  // [x, y, w, l] in case coordinates

// Snap-fit parameters
SNAP_BEAD_H         = 0.65;   // Snap bead protrusion outward
SNAP_TAB_W          = 9.0;    // Width of flexible cantilever snap tab
SNAP_Z              = 2.6;    // Height above base where snap locks

// Bottom plate base (outer dimensions unchanged: 76 x 86 x 2.0 mm)
BASE_T              = 2.0;    // Base plate thickness
PAD_R               = 5.0;    // Silicone/rubber pad recess radius (10 mm pads)
PAD_DEPTH           = 1.0;    // Pad recess depth (1 mm pads sit flush with the tread)
PAD_INSET           = 12.0;   // Pad centre distance from the outer edges
PRY_W               = 12.0;   // Rear pry notch relief width
PRY_L               = 5.0;    // Rear pry notch relief length (runs out past the rear edge)
PRY_DEPTH           = 1.4;    // Rear pry notch relief depth

// Deskmat anti-slip tread: diagonal diamond knurl engraved into the bottom face.
// Grooves are widest at the desk and narrow layer by layer, so the diamond lands grow wider
// going up: every layer overhangs < 35 deg and the plate still prints flat on the bed with
// no supports. The small sharp diamond edges bite into cloth and hybrid deskmats.
LAYER_H             = 0.2;    // Print layer height the tread is designed for
TREAD_PITCH         = 3.0;    // Groove spacing (perpendicular), both diagonal directions
TREAD_GROOVE_W      = 1.2;    // Groove width at the desk surface
TREAD_TAPER         = 0.25;   // Groove narrows by this much per layer
TREAD_LAYERS        = 4;      // Groove depth in layers (4 x 0.2 = 0.8 mm)
TREAD_BORDER        = 2.5;    // Smooth rim left around the pattern
TREAD_DEPTH         = TREAD_LAYERS * LAYER_H;

// Internal ballast trays (on the bottom plate, inside the case). Fill with steel BBs or
// lead shot set in epoxy, tungsten putty, or glued M8 nuts / steel washers; never loose
// metal (rattles, and can short the board). Placed only where nothing else lives: clear of
// the switch bodies and their wires, the LCD board and its pin headers, the USB-C plug path
// and the flexing snap tabs.
BALLAST_WALL_T      = 1.2;
BALLAST_LOW_H       = 6.8;    // Under the key deck (its ceiling is 8.0 above the base)
BALLAST_SIDE_H      = 10.0;   // Beside the LCD board
BALLAST_REAR_H      = 14.0;   // Under the rear shelf, behind the board's rear header pins
// [x, y, w, l, h] in bottom plate coordinates (outer tray footprint on top of the base)
BALLAST_TRAYS = [
    [ 6.3,  5.8, 11.2, 19.4, BALLAST_LOW_H],   // Front-left corner, outside the key PCB keepout
    [58.5,  5.8, 11.2, 19.4, BALLAST_LOW_H],   // Front-right corner, outside the key PCB keepout
    [ 6.3, 27.5, 63.4,  8.0, BALLAST_LOW_H],   // Band between the keys and the board's front pins
    [ 6.3, 42.5,  6.5, 37.2, BALLAST_SIDE_H],  // Left side of the LCD board
    [63.8, 59.0,  5.9, 20.7, BALLAST_SIDE_H],  // Right side, behind the USB-C plug path
    [ 6.3, 70.0, 63.4,  9.7, BALLAST_REAR_H],  // Rear bank under the shelf (M8 nuts fit on edge)
];

assert(TREAD_DEPTH < PAD_DEPTH && PAD_DEPTH < PRY_DEPTH && PRY_DEPTH < BASE_T,
       "base plate layers must be ordered: tread < pad recess < pry notch < base");

// Part Selector: "assembly", "top_case", "bottom_plate"
PART = "assembly";

// --- Helper Modules ---

module rounded_box(w, l, h, r) {
    hull() {
        translate([r, r, 0]) cylinder(h=h, r=r);
        translate([w - r, r, 0]) cylinder(h=h, r=r);
        translate([r, l - r, 0]) cylinder(h=h, r=r);
        translate([w - r, l - r, 0]) cylinder(h=h, r=r);
    }
}

// 2-tier outer shell hull (Distinct Flat Front + Distinct Angled Rear)
module case_outer_hull() {
    w = CASE_W;
    l = CASE_L;
    r = CORNER_R;
    y_sh = Y_SHELF + r;   // shelf cylinder centres, so the slope meets the shelf at Y_SHELF

    union() {
        // 1. Flat front key deck
        hull() {
            translate([r, r, 0]) cylinder(h=H_FLAT, r=r);
            translate([w - r, r, 0]) cylinder(h=H_FLAT, r=r);
            translate([r, Y_SPLIT, 0]) cylinder(h=H_FLAT, r=r);
            translate([w - r, Y_SPLIT, 0]) cylinder(h=H_FLAT, r=r);
        }
        // 2. Screen slope at DECK_TILT
        hull() {
            translate([r, Y_SPLIT, 0]) cylinder(h=H_FLAT, r=r);
            translate([w - r, Y_SPLIT, 0]) cylinder(h=H_FLAT, r=r);
            translate([r, y_sh, 0]) cylinder(h=H_REAR, r=r);
            translate([w - r, y_sh, 0]) cylinder(h=H_REAR, r=r);
        }
        // 3. Flat rear shelf
        hull() {
            translate([r, y_sh, 0]) cylinder(h=H_REAR, r=r);
            translate([w - r, y_sh, 0]) cylinder(h=H_REAR, r=r);
            translate([r, l - r, 0]) cylinder(h=H_REAR, r=r);
            translate([w - r, l - r, 0]) cylinder(h=H_REAR, r=r);
        }
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

// Screen cutout in the screen frame (see LCD_* above). The glass is NOT recessed: it rests on
// the deck surface and overlaps the cutout on all sides, so its black border sits on the case.
module screen_cutout() {
    x0 = LCD_BODY_OFFSET_X - LCD_BODY_X/2 - LCD_FIT;
    x1 = LCD_BODY_OFFSET_X + LCD_BODY_X/2 + LCD_USB_OVERHANG + LCD_FIT;
    y  = LCD_BODY_Y/2 + LCD_FIT;
    translate([x0, -y, -LCD_CUT_DEPTH])
        cube([x1 - x0, 2 * y, LCD_CUT_DEPTH + 1.0]);
}

// Places children in the screen frame: glass centre on the deck surface, tilted with the deck
module at_screen() {
    translate([CASE_W/2, SCREEN_Y, SCREEN_Z])
        rotate([DECK_TILT, 0, 0])
            children();
}

// USB-C side wall cutout with lead-in chamfer
module usbc_cutout() {
    union() {
        // Core through-port (17 x 8 mm): clears a square-cornered 12.5 x 7 mm plug body
        hull() {
            translate([0, -4.5, 0]) rotate([0, 90, 0]) cylinder(h=25, r=4.0, center=true);
            translate([0, 4.5, 0]) rotate([0, 90, 0]) cylinder(h=25, r=4.0, center=true);
        }

        // Exterior lead-in flare / chamfer for wide cable heads
        translate([6.0, 0, 0])
        hull() {
            translate([0, -4.5, 0]) rotate([0, 90, 0]) cylinder(h=10, r=5.2, center=true);
            translate([0, 4.5, 0]) rotate([0, 90, 0]) cylinder(h=10, r=5.2, center=true);
        }
    }
}

// --- Top Case Module ---

// Space under the key deck and the screen deck, extruded across the full width
module cavity_ceiling_profile() {
    t = tan(DECK_TILT);
    drop = DECK_T / cos(DECK_TILT);                         // vertical deck thickness on the slope
    z_flat = H_FLAT - FLAT_DECK_T;
    y_knee = INCLINE_START + (drop - FLAT_DECK_T) / t;      // slope ceiling meets flat ceiling
    z_cap = H_REAR - DECK_T;                                // under the flat rear shelf
    y_cap = INCLINE_START + (z_cap - H_FLAT + drop) / t;
    rotate([90, 0, 90])
        linear_extrude(height=CASE_W)
            polygon([[-1, -2], [-1, z_flat], [y_knee, z_flat], [y_cap, z_cap],
                     [CASE_L + 1, z_cap], [CASE_L + 1, -2]]);
}

module top_case() {
    difference() {
        // 1. Outer Solid Case
        case_outer_hull();

        // 2. Main Internal Hollow Cavity: the inner footprint under a ceiling that follows the
        //    outer surface at constant deck thickness (a hull of cylinders gets too thin at 30°)
        intersection() {
            translate([WALL_T, WALL_T, -1])
                rounded_box(CASE_W - 2*WALL_T, CASE_L - 2*WALL_T, H_REAR + 2, CORNER_R/2);
            cavity_ceiling_profile();
        }

        // 3. Snap-Fit Internal Latch Grooves (depth 0.8mm into inner wall)
        // Left & Right walls (front at Y = 17.0mm, rear at Y = 75.0mm, clear of USB-C)
        for (y_pos = [17.0, 75.0]) {
            // Left inner wall groove
            translate([WALL_T - 0.8, y_pos - (SNAP_TAB_W + 1.0)/2, SNAP_Z])
                cube([1.0, SNAP_TAB_W + 1.0, 1.4]);
            // Right inner wall groove
            translate([CASE_W - WALL_T - 0.2, y_pos - (SNAP_TAB_W + 1.0)/2, SNAP_Z])
                cube([1.0, SNAP_TAB_W + 1.0, 1.4]);
        }
        // Front & Rear walls
        translate([CASE_W/2 - (SNAP_TAB_W + 1.0)/2, WALL_T - 0.8, SNAP_Z])
            cube([SNAP_TAB_W + 1.0, 1.0, 1.4]);
        translate([CASE_W/2 - (SNAP_TAB_W + 1.0)/2, CASE_L - WALL_T - 0.2, SNAP_Z])
            cube([SNAP_TAB_W + 1.0, 1.0, 1.4]);

        // 4. MX Switch Cutouts on FLAT Front Deck (0° horizontal, Z = H_FLAT)
        // Key 1 (Z) - Left
        translate([CASE_W/2 - MX_PITCH/2, 16.5, H_FLAT])
            mx_switch_cutout();

        // Key 2 (X) - Right
        translate([CASE_W/2 + MX_PITCH/2, 16.5, H_FLAT])
            mx_switch_cutout();

        // 5. Waveshare 2.0" LCD on the inclined rear deck
        at_screen() screen_cutout();

        // 6. USB-C port in the right side wall, on the receptacle axis and tilted like the board
        at_screen() translate([CASE_W/2 - WALL_T/2, 0, LCD_USB_Z]) usbc_cutout();

        // 7. Rear Pry Notch (for opening with a coin or pick without damage)
        translate([CASE_W/2 - 6.0, CASE_L - 2.0, -0.1])
            cube([12.0, 4.0, 2.2]);
    }
}

// --- Bottom Plate Helpers ---

module plate_outline_2d() {
    translate([CORNER_R, CORNER_R])
        offset(r=CORNER_R) square([CASE_W - 2 * CORNER_R, CASE_L - 2 * CORNER_R]);
}

module pad_recesses_2d() {
    for (pos = [[PAD_INSET, PAD_INSET], [CASE_W - PAD_INSET, PAD_INSET],
                [PAD_INSET, CASE_L - PAD_INSET], [CASE_W - PAD_INSET, CASE_L - PAD_INSET]]) {
        translate(pos) circle(r=PAD_R);
    }
}

module pry_notch_2d() {
    translate([CASE_W/2 - PRY_W/2, CASE_L - PRY_L + 2.0]) square([PRY_W, PRY_L]);
}

// Diagonal groove grid at one groove width, clipped inside the smooth border
module tread_grooves_2d(groove_w) {
    n = ceil((CASE_W + CASE_L) / TREAD_PITCH / 1.4) + 2;
    intersection() {
        offset(delta=-TREAD_BORDER) plate_outline_2d();
        translate([CASE_W/2, CASE_L/2])
        for (a = [45, -45]) rotate(a)
            for (i = [-n : n]) translate([0, i * TREAD_PITCH])
                square([2 * (CASE_W + CASE_L), groove_w], center=true);
    }
}

// The base is built as stacked 2D layers (one per print layer through the tread), which is
// both how it prints and much faster for OpenSCAD than 3D-subtracting hundreds of grooves.
module base_plate_with_tread() {
    union() {
        for (k = [0 : TREAD_LAYERS - 1]) {
            translate([0, 0, k * LAYER_H])
            linear_extrude(height=LAYER_H)
            difference() {
                plate_outline_2d();
                tread_grooves_2d(max(LAYER_H, TREAD_GROOVE_W - k * TREAD_TAPER));
                pad_recesses_2d();
                pry_notch_2d();
            }
        }
        translate([0, 0, TREAD_DEPTH])
        linear_extrude(height=PAD_DEPTH - TREAD_DEPTH)
        difference() { plate_outline_2d(); pad_recesses_2d(); pry_notch_2d(); }

        translate([0, 0, PAD_DEPTH])
        linear_extrude(height=PRY_DEPTH - PAD_DEPTH)
        difference() { plate_outline_2d(); pry_notch_2d(); }

        translate([0, 0, PRY_DEPTH])
        linear_extrude(height=BASE_T - PRY_DEPTH)
        plate_outline_2d();
    }
}

module ballast_tray(w, l, h) {
    difference() {
        rounded_box(w, l, h, 1.5);
        translate([BALLAST_WALL_T, BALLAST_WALL_T, -0.1])
            rounded_box(w - 2 * BALLAST_WALL_T, l - 2 * BALLAST_WALL_T, h + 1.0, 0.6);
    }
}

// --- Bottom Plate Module (100% Screwless Snap-Fit) ---

module bottom_plate() {
    inner_w = CASE_W - 2 * WALL_T - 2 * TOLERANCE;
    inner_l = CASE_L - 2 * WALL_T - 2 * TOLERANCE;
    rim_r   = max(1.0, CORNER_R - WALL_T - TOLERANCE);
    rim_h   = 4.6; // Rim height above base

    union() {
        // 1. Base Plate (flush with desk and outer case): diamond tread, pad recesses, pry notch
        base_plate_with_tread();

        // 1b. Internal ballast trays (open top, floor is the base plate)
        for (t = BALLAST_TRAYS) {
            translate([t[0], t[1], BASE_T])
                ballast_tray(t[2], t[3], t[4]);
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
            // Left & Right tabs (at Y = 17.0mm and Y = 75.0mm)
            for (y_pos = [17.0 - (WALL_T + TOLERANCE), 75.0 - (WALL_T + TOLERANCE)]) {
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
        // Overlap by 0.5mm into the tab wall to guarantee 100% manifold solid
        // Left & Right beads (at Y = 17.0mm and Y = 75.0mm)
        for (y_pos = [17.0, 75.0]) {
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

        // 4. No screen pillars: the glass rests on the deck, so pillars pushing up would lift it
    }
}

// --- Render Selection ---
if (PART == "assembly") {
    color([0.35, 0.45, 0.60, 0.95]) top_case();
    translate([0, 0, -4.0]) color([0.18, 0.20, 0.24, 1.0]) bottom_plate();
} else if (PART == "top_case") {
    top_case();
} else if (PART == "bottom_plate") {
    bottom_plate();
}

