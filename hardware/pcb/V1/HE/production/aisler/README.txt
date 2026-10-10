osuPad Hall Effect input module V1.1 for AISLER
==================================================

Upload he_input_v1.kicad_pcb (native KiCad board, zones filled, parts carry
MPN and MFG fields) or he_input_v1-odb.zip (ODB++ of the same board).  AISLER
reads the components from either; no BOM or placement file is needed.

Board options
  2 layers, 1.6 mm FR-4, 1 oz copper, HASL or ENIG (the design meets the
  "2 layer 1.6 mm HASL" rules: track 0.25 / spacing 0.2 mm, vias 0.7 mm with
  a 0.3 mm hole, copper to edge 0.3 mm), 52.0 x 24.0 mm, green mask.

Assembly
  Every component is on the BOTTOM side, all SMD:
    J1        JST SM08B-SRSS-TB(LF)(SN) (MODULE, LCSC C160407)
    U1,U2     Texas Instruments DRV5055A3QDBZR (DRV5055A3, LCSC C266128)
    C1,C2,C3  YAGEO CC0603KRX7R9BB104 (100nF, LCSC C14663)
    R1        UNI-ROYAL 0603WAF1003T5E (100k, LCSC C25803)
    R2        UNI-ROYAL 0603WAF4702T5E (47k, LCSC C25819)
  TP1-TP3 are bare probe pads, H1/H2 mounting holes and SW1/SW2 the switch
  positions: they are board features (excluded from BOM, DNP), not parts.
  The Gateron magnetic switches go in later by hand.

Check in the AISLER part view before paying
  U1/U2 (SOT-23): the single lead (pin 3, GND) points to the front edge (the
  "OSUPAD HE" text), the two leads to the connector; pin 1 (VCC) is next to
  the silkscreen dot.  A SOT-23 cannot be placed mirrored, so if the leads
  sit on the pads the orientation is right.
  J1: the 8 contacts face the inside of the board, the cable opening the edge.

Source: hardware/pcb/V1/scripts/he_board.py (single source of truth); these
files were written by export_he_aisler.py.
