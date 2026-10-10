OPad Controller Carrier V1 for AISLER
=====================================

Upload controller_carrier_v1.kicad_pcb (native KiCad board, zones filled, parts carry
MPN and MFG fields) or controller_carrier_v1-odb.zip (ODB++ of the same board).  AISLER
reads the components from either; no BOM or placement file is needed.

Board options
  2 layers, 1.6 mm FR-4, 1 oz copper, HASL or ENIG, green mask.
  Vias are 0.6 mm with a 0.3 mm hole (0.15 mm ring): below AISLER's HASL
  rule of 0.2 mm, so order ENIG or ask whether they accept it.

Assembly
  J_MOD (JST SH) is the only SMD part, on the BOTTOM side. J_P1/J_P2 are
  through-hole sockets on the TOP side (the side marked WAVESHARE ON THIS
  SIDE); order them as THT or solder them yourself.
    J_P2         HCTL PM254-1-14-Z-8.5 (WAVESHARE_P2, top, THT, LCSC C2897377)
    J_P1         HCTL PM254-1-14-Z-8.5 (WAVESHARE_P1, top, THT, LCSC C2897377)
    J_MOD        JST SM08B-SRSS-TB(LF)(SN) (MODULE, bottom, SMD, LCSC C160407)

Source: hardware/pcb/V1 (generate_boards.py); these files were written by
scripts/export_aisler.py.
