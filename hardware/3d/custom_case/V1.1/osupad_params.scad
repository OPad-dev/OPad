// =============================================================================
//  OPad V1.1 enclosure: shared parameters
//  Included by osupad_enclosure.scad and tools/fit_check.scad.
//
//  World frame: origin at the front-left bottom corner of the bottom plate,
//  X to the right, Y toward the screen (rear), Z up. The top case sits on the
//  bottom plate's base, so its underside is at z = BASE_T.
//
//  Deck frame (at_deck): origin at the centre of the screen glass, on the deck
//  surface. x = case X, y ("a") up the slope, z along the deck normal, so a
//  point D mm behind the glass front is at z = -D.
// =============================================================================

$fn = 64;

// --- Outline -----------------------------------------------------------------
CASE_W      = 76.0;
WALL        = 2.4;    // 3 perimeters at 0.4 mm (design-guide minimum)
CORNER_R    = 4.5;
BASE_T      = 2.0;    // bottom plate floor
TOL         = 0.20;   // FDM fit clearance

// --- Key deck (MX / HE input module) ----------------------------------------
MX_CUT          = 14.0;
PLATE_T         = 1.5;
MX_PITCH        = 19.05;
KEY_Y           = 16.5;
KEY_X           = [CASE_W/2 - MX_PITCH/2, CASE_W/2 + MX_PITCH/2];   // 28.475 / 47.525
KEY_TOP_T       = 3.8;    // deck skin away from the 16 x 16 switch reliefs
MX_PLATE_TO_PCB = 5.0;
MODULE_PCB_T    = 1.6;
MODULE_BELOW    = 1.85;   // deepest part under the module PCB away from J1 (Kailh socket)
MODULE_J1_BELOW = 2.9;    // JST SH J1 at the module's rear edge
MODULE_HOLES    = [[15.5, 16.5], [60.5, 16.5]];   // M2, hardware/pcb/V1 README
MODULE_OUTLINE  = [12, 7, 64, 31];                // x0, y0, x1, y1
MODULE_J1       = [32.5, 25.6, 43.5, 31.0];       // J1 body footprint

// Screwless module mount: pins under the key deck locate the module through its
// two M2 holes, and posts on the bottom plate clamp it up against the deck.
MODULE_FLOOR_CLR = 0.5;   // J1, the lowest part of either module, above the floor
MODULE_PIN_D     = 1.9;   // in the 2.2 mm NPTH holes
MODULE_PCB_BOT   = BASE_T + MODULE_J1_BELOW + MODULE_FLOOR_CLR;
MODULE_POSTS     = [[22, 10], [54, 10]];   // extra clamp posts, front strip (no parts on MX or HE)
MODULE_PCB_TOP  = MODULE_PCB_BOT + MODULE_PCB_T;
KEY_DECK_H      = MODULE_PCB_TOP + MX_PLATE_TO_PCB;         // world z of the plate top

// --- Screen pod ----------------------------------------------------------------
TILT        = 16;     // deck angle
Y_KEY_END   = 32;     // key deck ends, screen pod begins
POD_Y0      = 35;     // front edge of the pod's top face
POD_TOP_T   = 3.0;    // pod skin (the glass ledge is POD_TOP_T - POCKET_D thick)

// Waveshare ESP32-S3-Touch-LCD-2, from the 2024-11-08 drawing and STEP model
GLASS_X     = 59.22;  // the STEP glass is larger than the drawing's 58.8 x 37.1
GLASS_Y     = 37.52;
GLASS_T     = 1.10;
GLASS_R     = 2.60;
POCKET_CLR  = 0.30;
TAPE_T      = 0.15;   // double-sided tape under the glass edge
POCKET_X    = GLASS_X + 2*POCKET_CLR;
POCKET_Y    = GLASS_Y + 2*POCKET_CLR;
POCKET_R    = GLASS_R + POCKET_CLR;
POCKET_D    = GLASS_T + TAPE_T;          // glass front ends flush with the frame
BORDER      = 3.5;                       // frame between pocket and pod edge
FRAME_A     = POCKET_Y/2 + BORDER;       // half extent of the pod face along the slope

WS_PCB_XC   = 0.21;   // Waveshare PCB centre relative to the glass centre (deck x)
WS_PCB_X    = 48.22;
WS_PCB_Y    = 35.02;
WS_BACK     = 7.2;    // glass front -> Waveshare PCB back
HDR_PLASTIC = 2.5;    // male header plastic on the Waveshare
THROUGH_X   = 48.8;   // deck opening the module and carrier pass through
THROUGH_Y   = 35.6;
USB_D       = 8.37;   // USB-C receptacle centre below the glass front
USB_X       = 25.5;   // receptacle face (deck x)
WS_HOLES    = [[21.31, 14.5], [21.31, -14.5], [-20.89, 14.5], [-20.89, -14.5]];

// Controller carrier V1 (hardware/pcb/V1/Carrier) on 8.5 mm sockets
SOCKET_H    = 8.5;
CARRIER_TOP = WS_BACK + HDR_PLASTIC + SOCKET_H;   // 18.2
CARRIER_T   = 1.6;
CARRIER_BOT = CARRIER_TOP + CARRIER_T;            // 19.8
TAIL        = 1.0;    // socket pin tails, clipped
JMOD_H      = 2.9;
ROW_A       = 15.24;  // socket rows
CARRIER_A   = 17.5;
POST_GAP    = 0.5;    // bottom-plate posts stop this far under the carrier

FLOOR_CLR   = 0.6;    // lowest stack point above the floor

// Lowest point of the stack decides how high the screen sits
function _low(a, d) = a * sin(TILT) - d * cos(TILT);
DZ_LOW  = min(_low(-(ROW_A + 0.9), CARRIER_BOT + TAIL),
              _low(-CARRIER_A, CARRIER_BOT),
              _low(-5.5, CARRIER_BOT + JMOD_H));
Z_C     = BASE_T + FLOOR_CLR - DZ_LOW;           // glass centre height
Y_C     = POD_Y0 + FRAME_A * cos(TILT);          // glass centre Y
H_REAR  = Z_C + FRAME_A * sin(TILT);             // highest point of the case

// Rear: the wall has to clear the carrier's rear-bottom edge
_RC_Y   = Y_C + CARRIER_A * cos(TILT) + CARRIER_BOT * sin(TILT);
_RC_Z   = Z_C + CARRIER_A * sin(TILT) - CARRIER_BOT * cos(TILT);
CASE_L  = ceil((_RC_Y + 0.8 + WALL) * 2) / 2;
Z_KNEE  = _RC_Z + 1.0;   // rear wall is vertical up to here, then leans with the deck

// --- Snap fit ----------------------------------------------------------------
// The bottom plate's rim is rigid; small beads at mid-span of each wall click
// into grooves, and the long top-case walls (free along their bottom edge)
// take the flex. Estimated strain in the walls 1-2 %, far below PLA/PETG
// limits, and the bending stays low enough across the layer lines.
SNAP_BEAD_H = 0.35;
SNAP_BEAD_L = 14.0;
SNAP_Z      = 2.6;    // above the top case's underside
SNAP_SIDE_Y = [CASE_L * 0.3, CASE_L * 0.7];
SNAP_END_X  = [CASE_W * 0.3, CASE_W * 0.7];   // front and rear walls, clear of the pry notch
RIM_H       = 4.6;
RIM_T       = 1.6;

// --- Feet ----------------------------------------------------------------------
FOOT_D      = 12.7;   // soft silicone bumpers, 12.7 mm (1/2")
FOOT_REC    = 0.6;    // locating recess depth
FOOT_POS    = [[10.5, 10.5], [CASE_W - 10.5, 10.5], [10.5, CASE_L - 10.5], [CASE_W - 10.5, CASE_L - 10.5]];

module at_deck() translate([CASE_W/2, Y_C, Z_C]) rotate([TILT, 0, 0]) children();

echo(str("V1.1: key deck ", KEY_DECK_H, " mm, rear ", H_REAR, " mm, length ", CASE_L,
         " mm, glass centre (Y ", Y_C, ", Z ", Z_C, ")"));
