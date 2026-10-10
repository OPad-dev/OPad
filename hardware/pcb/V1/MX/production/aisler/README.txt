osuPad MX Input Module V1 for AISLER
====================================

Upload mx_input_v1.kicad_pcb (native KiCad board, zones filled, parts carry
MPN and MFG fields) or mx_input_v1-odb.zip (ODB++ of the same board).  AISLER
reads the components from either; no BOM or placement file is needed.

Board options
  2 layers, 1.6 mm FR-4, 1 oz copper, HASL or ENIG, green mask.
  Vias are 0.6 mm with a 0.3 mm hole (0.15 mm ring): below AISLER's HASL
  rule of 0.2 mm, so order ENIG or ask whether they accept it.

Assembly
  Switch sockets, J1, R1-R4 and C1 are all on the BOTTOM side (SMD).
    R1,R2,R4     UNI-ROYAL 0603WAF1002T5E (10k, bottom, SMD, LCSC C25804)
    C1           YAGEO CC0603KRX7R9BB104 (100nF, bottom, SMD, LCSC C14663)
    J1           JST SM08B-SRSS-TB(LF)(SN) (MODULE, bottom, SMD, LCSC C160407)
    R3           UNI-ROYAL 0603WAF1003T5E (100k, bottom, SMD, LCSC C25803)
    SW2          Kailh CPG151101S11 (KEY2, bottom, SMD, LCSC C41430893)
    SW1          Kailh CPG151101S11 (KEY1, bottom, SMD, LCSC C41430893)

Source: hardware/pcb/V1 (generate_boards.py); these files were written by
scripts/export_aisler.py.
