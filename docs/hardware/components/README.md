# Component notes

These notes cover the parts the firmware talks to or depends on: registers,
timings, pin tables and how each part is wired on OPad's boards. Each
statement cites a datasheet section. They are plain text, so anyone, AI tools
included, can search them without opening a PDF.

The vendor PDFs are copyrighted, so the repository does not hold them (it is
REUSE compliant, see AGENTS.md §5). `scripts/fetch_datasheets.sh` downloads
them into `docs/hardware/components/.datasheets/`, which git ignores. The
script checks each file against the sha256 recorded here when there is one.

| Part | Where | Note | Datasheet |
|---|---|---|---|
| TI ADS7028 | HE V3 module, U4 (ADC) | [ads7028.md](ads7028.md) | SBAS978A |
