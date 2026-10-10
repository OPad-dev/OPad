osuPad Hall Effect Input Module V1 for AISLER
=============================================

Upload he_input_v1.kicad_pcb (native KiCad board, zones filled, parts carry
MPN and MFG fields) or he_input_v1-odb.zip (ODB++ of the same board).  AISLER
reads the components from either; no BOM or placement file is needed.

Board options
  2 layers, 1.6 mm FR-4, 1 oz copper, HASL or ENIG, green mask.
  Meets AISLER's "2 layer 1.6 mm HASL" rules: track 0.25 / spacing 0.2 mm,
  vias 0.7 mm with a 0.3 mm hole (0.2 mm ring), copper to edge 0.3 mm.

Assembly
  Every part is on the BOTTOM side (SMD). TP1-TP3 are bare probe pads, H1/H2
  mounting holes and SW1/SW2 the switch positions: board features (DNP), not
  parts. The Gateron magnetic switches go in later by hand.
  U1/U2 (SOT-23): the single lead (pin 3, GND) points to the front edge (the
  "OSUPAD HE" text), the two leads to the connector; pin 1 (VCC) is next to
  the silkscreen dot. A SOT-23 cannot be placed mirrored, so if the leads sit
  on the pads the orientation is right.
    U1,U2        Texas Instruments DRV5055A3QDBZR (DRV5055A3, bottom, SMD, LCSC C266128)
    C2,C3,C1     YAGEO CC0603KRX7R9BB104 (100nF, bottom, SMD, LCSC C14663)
    J1           JST SM08B-SRSS-TB(LF)(SN) (MODULE, bottom, SMD, LCSC C160407)
    R1           UNI-ROYAL 0603WAF1003T5E (100k, bottom, SMD, LCSC C25803)
    R2           UNI-ROYAL 0603WAF4702T5E (47k, bottom, SMD, LCSC C25819)

Source: hardware/pcb/V1 (he_board.py); these files were written by
scripts/export_aisler.py.
