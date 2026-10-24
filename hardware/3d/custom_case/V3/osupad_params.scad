// =============================================================================
//  OPad V3 enclosure: shared parameters
//  Included by osupad_enclosure.scad and tools/fit_check.scad.
//
//  For the V3 boards (hardware/pcb/V3): the carrier with its USB-C on a tab past
//  the Waveshare's USB end, and the three-key MX / HE modules on one 14-way FFC.
//  Built on V1.1 (hardware/3d/custom_case/V1.1); what changed is marked "V3:".
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
// V3: 84 mm wide (V1.1: 76) for the 71.05 mm module and the carrier's USB-C tab,
// which reaches 38.6 mm right of the glass centre
CASE_W      = 84.0;
WALL        = 2.4;    // 3 perimeters at 0.4 mm (design-guide minimum)
CORNER_R    = 4.5;
BASE_T      = 2.0;    // bottom plate floor
TOL         = 0.20;   // FDM fit clearance

// --- Key deck (MX / HE input module V3) --------------------------------------
MX_CUT          = 14.0;
PLATE_T         = 1.5;
MX_PITCH        = 19.05;
KEY_Y           = 16.5;
KEY_X           = [for (i = [-1 : 1]) CASE_W/2 + i * MX_PITCH];   // V3: three keys
KEY_TOP_T       = 3.8;    // deck skin away from the 16 x 16 switch reliefs
MX_PLATE_TO_PCB = 5.0;
MODULE_PCB_T    = 1.6;
MODULE_BELOW    = 1.85;   // Kailh socket under the MX module
MODULE_J1_BELOW = 2.5;    // V3: FFC connector J1 (JUSHUO AFA07, 2.5 mm), the lowest part of either module
// Module frame -> world: KiCad X, Y of hardware/pcb/V3/{MX,HE} (keys at Y 83.5,
// middle key at X 147.525, rear edge Y 69)
function mod_to_world(p) = [CASE_W/2 + (p[0] - 147.525), KEY_Y + (83.5 - p[1])];
MODULE_HOLES    = [for (p = [[115.5, 72.5], [179.55, 72.5], [115.5, 89.5], [179.55, 89.5]]) mod_to_world(p)];
MODULE_OUTLINE  = [CASE_W/2 - 35.525, 7, CASE_W/2 + 35.525, 31];   // x0, y0, x1, y1 (71.05 x 24)
// J1 body: 21.06 x 5.3 mm at the rear edge, centred on the middle key, mouth 0.3 mm inside the edge
MODULE_J1       = [CASE_W/2 - 10.6, 31 - 0.3 - 5.0, CASE_W/2 + 10.6, 31 + 0.2];
J1_SLOT_Z       = 1.25;   // cable entry below the module PCB

// Screwless module mount: pins under the key deck locate the module through its
// four corner holes (V3: four, decision H16), and posts on the bottom plate
// clamp it up against the deck.
MODULE_FLOOR_CLR = 0.5;   // J1, the lowest part of either module, above the floor
MODULE_PIN_D     = 1.9;   // in the 2.2 mm NPTH holes
MODULE_PCB_BOT   = BASE_T + MODULE_J1_BELOW + MODULE_FLOOR_CLR;
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

// V3: the stack goes in from below and the glass bears on a lip round the window,
// with foam tape between them; depths below are from the deck surface, so the
// glass front is SINK below it
LIP         = 1.2;    // lip over the glass edge (the glass border is 3.4 mm or more round the display)
LIP_T       = 1.0;    // lip thickness
GASKET      = 0.5;    // foam tape on the lip's underside...
GASKET_C    = 0.3;    // ...compressed to this by the carrier posts
SINK        = LIP_T + GASKET_C;

WS_PCB_XC   = 0.21;   // Waveshare PCB centre relative to the glass centre (deck x)
WS_PCB_X    = 48.22;
WS_PCB_Y    = 35.02;
WS_BACK     = SINK + 7.2;   // deck surface -> Waveshare PCB back (7.2 mm behind the glass front)
HDR_PLASTIC = 2.5;    // male header plastic on the Waveshare
THROUGH_Y   = 35.6;
WS_USB_X    = 25.5;   // the Waveshare's own USB-C face (deck x): V3 covers it, the case has no opening there
WS_HOLES    = [[21.31, 14.5], [21.31, -14.5], [-20.89, 14.5], [-20.89, -14.5]];

// Controller carrier V3 (hardware/pcb/V3/Carrier) on 8.5 mm sockets.
// Carrier KiCad frame -> deck: x = CARRIER_EDGE_X + (Y - 100), a = X - 117.5
SOCKET_H    = 8.5;
CARRIER_TOP = WS_BACK + HDR_PLASTIC + SOCKET_H;   // 18.2
CARRIER_T   = 1.6;
CARRIER_BOT = CARRIER_TOP + CARRIER_T;            // 19.8
CARRIER_EDGE_X = WS_PCB_XC - 24.1;                // non-USB end (deck x)
function car_to_deck(p) = [CARRIER_EDGE_X + (p[1] - 100), p[0] - 117.5];
TAIL        = 1.0;    // socket pin tails, clipped
ROW_A       = 15.24;  // socket rows
CARRIER_A   = 17.5;
CARRIER_TAB_X = car_to_deck([0, 162.5])[0];        // V3: right end of the USB-C tab (deck x)
CARRIER_TAB_A0 = 118 - 117.5;                      // the tab spans a from here to CARRIER_A
POST_GAP    = 0.5;    // bottom-plate posts stop this far under the carrier

// V3: the carrier's USB-C (HRO TYPE-C-31-M-12) on the carrier TOP, mouth on the rear edge
USB_C       = car_to_deck([130.916, 154.0]);      // receptacle footprint origin (deck x, a)
USB_H       = 3.26;                                // receptacle height above the carrier
USB_D       = CARRIER_TOP - USB_H/2;               // receptacle axis, depth behind the glass front
USB_MOUTH_A = CARRIER_A;                           // mouth on the rear edge
USB_W       = 8.94;

// V3: module connector J_MOD (FFC, 2.5 mm tall) on the carrier TOP at the non-USB end,
// cable leaving towards the left wall between the carrier and the Waveshare
JMOD        = car_to_deck([117.5, 104.9]);         // contact row centre
JMOD_H      = 2.5;
FFC_W       = 15.0;   // 14-way, 1.0 mm FFC: (14 + 1) x 1.0 mm wide
FFC_T       = 0.6;    // 0.3 mm cable, 0.6 mm allowed (stiffeners, waviness)

// V3: 1.4 mm (V1.1: 0.6) so the FFC can pass under the front of the stack
FLOOR_CLR   = 1.4;    // lowest stack point above the floor

// Lowest point of the stack decides how high the screen sits
function _low(a, d) = a * sin(TILT) - d * cos(TILT);
DZ_LOW  = min(_low(-(ROW_A + 0.9), CARRIER_BOT + TAIL),
              _low(-CARRIER_A, CARRIER_BOT));
Z_C     = BASE_T + FLOOR_CLR - DZ_LOW;           // glass centre height
Y_C     = POD_Y0 + FRAME_A * cos(TILT);          // glass centre Y
H_REAR  = Z_C + FRAME_A * sin(TILT);             // highest point of the case

// Rear: the wall has to clear the carrier's rear-bottom edge
_RC_Y   = Y_C + CARRIER_A * cos(TILT) + CARRIER_BOT * sin(TILT);
_RC_Z   = Z_C + CARRIER_A * sin(TILT) - CARRIER_BOT * cos(TILT);
CASE_L  = ceil((_RC_Y + 0.8 + WALL) * 2) / 2;
Z_KNEE  = _RC_Z + 1.0;   // rear wall is vertical up to here, then leans with the deck

// --- Snap fit ----------------------------------------------------------------
SNAP_BEAD_H = 0.35;
SNAP_BEAD_L = 14.0;
SNAP_Z      = 2.6;    // above the top case's underside
SNAP_SIDE_Y = [CASE_L * 0.3, CASE_L * 0.7];
SNAP_END_X  = [CASE_W * 0.3, CASE_W * 0.7];   // front and rear walls, clear of the pry notch and the USB-C
RIM_H       = 4.6;
RIM_T       = 1.6;

// --- Feet ----------------------------------------------------------------------
FOOT_D      = 12.7;   // soft silicone bumpers, 12.7 mm (1/2")
FOOT_REC    = 0.6;    // locating recess depth
FOOT_POS    = [[10.5, 10.5], [CASE_W - 10.5, 10.5], [10.5, CASE_L - 10.5], [CASE_W - 10.5, CASE_L - 10.5]];

module at_deck() translate([CASE_W/2, Y_C, Z_C]) rotate([TILT, 0, 0]) children();
function deck_to_world(p) = [CASE_W/2 + p[0],
                             Y_C + p[1]*cos(TILT) - p[2]*sin(TILT),
                             Z_C + p[1]*sin(TILT) + p[2]*cos(TILT)];

// --- The module cable (FFC), as a ribbon through these points (world) ----------
// From J_MOD out of the carrier's left end, down past the stack, across the floor to the
// middle, one 45 deg fold, then straight forward into the module's J1. The cable's width
// lies along the first segment's normal until the fold; see tools/reference_models.scad.
FFC_FLOOR_Z = BASE_T + 0.35;
JMOD_EXIT   = deck_to_world([CARRIER_EDGE_X - 1.5, JMOD[1], -(CARRIER_TOP - 1.2)]);
FFC_DROP_X  = CASE_W/2 + CARRIER_EDGE_X - 4.5;     // where it turns down, left of the carrier
FFC_FOLD    = [CASE_W/2, JMOD_EXIT[1]];            // the 45 deg fold, under the carrier's window
J1_ENTRY    = [CASE_W/2, 31 + 0.2, MODULE_PCB_BOT - J1_SLOT_Z];
FFC_RUN_Y   = Y_C + (CARRIER_TOP - 1.2) * sin(TILT);   // centre line of the run across the floor (world y)

// Cable channel on the bottom plate along the floor part of the route: low walls with
// small lips the cable is pressed in under. Low enough to pass under the front of the
// screen stack (FLOOR_CLR above the floor there).
GUIDE_H     = 1.0;    // wall height above the floor
GUIDE_T     = 1.2;    // wall thickness
GUIDE_GAP   = 0.4;    // cable to wall, each side
GUIDE_LIP   = 0.6;    // lip over the cable's edge
GUIDE_LIP_T = 0.3;
GUIDE_END_Y = 36.0;   // the forward run stops here; the cable rises into J1 after it

echo(str("V3: case ", CASE_W, " x ", CASE_L, " mm, key deck ", KEY_DECK_H, " mm, rear ", H_REAR,
         " mm, glass centre (Y ", Y_C, ", Z ", Z_C, ")"));
