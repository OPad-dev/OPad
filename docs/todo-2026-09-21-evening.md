# To fix: 2026-09-21 (evening)

Found while testing the installed `opad_1.0.0-rc-1_amd64.deb` on Linux (Ubuntu, Node 18.19.1) with the pad connected.

Symptom: the pad is not detected and tosu never opens.

## Status (updated 2026-09-21, after the fixes)

Done in the working tree (not committed):

| Item | What changed |
|---|---|
| 1. udev | Rule renamed to `packaging/linux/udev/70-opad.rules` (with a comment on why it must sort before 73). Makefile, deb/rpm assets, README, spec updated. `opadctl setup` printed an old, insecure rule (`MODE="0666"`, `GROUP="uucp"`); it now embeds the packaged file with `include_str!`. |
| 2. Logging | `opad-device` warns once per distinct open error, with a hint for permission denied (udev/`getfacl` on Linux, port busy on Windows), and stops repeating "Opening ..." every retry. |
| 3. tosu | Worse than the note below: tosu 4.26.2 needs Node >=24.14 <25 and its native addons are built for Node 24, so a `package.json` alone would not have fixed it on Ubuntu (Node 18). The deb/rpm/AppImage now ship upstream's standalone `pkg` binary (Node 24 built in, no system Node needed): `make tosu` default `TOSU_STANDALONE=1`. Arch keeps the Node wrapper (`TOSU_STANDALONE=0`, no network in `build()`), now with the missing `{"type":"module"}` `package.json`. Also found: the old deb dropped tosu's `dist/assets/` (cargo-deb globs skip directories), fixed by the single binary. |
| 3b. tosu dashboard | Also found: tosu on Linux reads `~/.config/tosu/tosu.env`, which overrides the `OPEN_DASHBOARD_ON_STARTUP=false` env var, while the daemon wrote its `tosu.env` next to the binary. Every launch would have opened the browser once tosu worked. `opad-tosu` now writes the file tosu actually reads. |
| 5.1 / 5.3 / 5.7 | `hardware/3d/reference_keypads/` and `hardware/3d/waveshare_board/` removed; `hardware/3d/README.md` links to them instead. |
| 5.2 | **Kept by decision (2026-09-21):** the original easter-egg GIF stays. A replacement was made and then reverted. |
| 5.4 | The KiCad-derived footprints (0603 R/C, SOT-23, JST SH, pin socket, M2 hole) and the whole symbol library are now written by `hardware/pcb/V1/scripts/gen_library.py`: pads and pins identical (checked automatically), all drawings original. HE generator re-run in a scratch copy: pads, nets, tracks, zones identical. Committed boards left as they are (their embedded copies are covered by KiCad's design exception, and MX/Carrier need `pcbnew` to regenerate). |
| 5.5 | `firmware/main/protocol/nanopb/LICENSE.txt` added (zlib, from upstream 0.4.9.1; vendored sources verified identical to that tag). Kept vendored. |
| 5.6 | Montserrat OFL declared in deb (`copyright` + `Montserrat-OFL.txt`), rpm, the three PKGBUILDs, AppImage and the Windows installer. Also found: the old deb shipped a cargo-deb generated `copyright` with MIT only (no tosu LGPL); it now ships `packaging/linux/deb/copyright`. |
| 6. Rebuild | `make deb` succeeded: `dist/opad_1.0.0-rc-1_amd64.deb` contains `70-opad.rules`, the standalone tosu, the full copyright file. The packaged tosu was run from a read-only directory and stays up. |

Still open:
- Install the new deb (`sudo apt install ./dist/opad_1.0.0-rc-1_amd64.deb`), replug the pad, and check `getfacl /dev/ttyACM0`, `Connected to OPad` in `journalctl --user -u opad-daemon`, and tosu running.
- Firmware not rebuilt here (no ESP-IDF on this machine): build and flash to see the new easter-egg animation.
- Open `lib/` in KiCad once to confirm the regenerated parts load (no KiCad on this machine). Optionally regenerate the boards so their embedded footprints use the new drawings.
- Section 4 (own USB ID): pid.codes PR not sent, firmware still `303a:4001`.
- 5.2: the easter-egg GIF is kept, so it is still third-party with no known license and not covered by MIT. Find its creator and get permission, or accept the risk for pid.codes and selling.
- Git history still contains the removed files (section 5 follow-ups): decide whether to rewrite it.
- Confirm `hardware/3d/custom_case` was not derived from the removed kamehameha models.
- 5.9 (optional): generate a third-party notices file for the Rust crates.
- Unrelated, noticed on the way: `README.md` still uses the old `osupad` names (`osupadctl`, `osupad-daemon.service`, `packaging/linux/desktop/osupad-gui.desktop`) in the install and CLI sections.

## 1. Pad detected by the kernel, but the daemon cannot open the port

**What happens**
- Pad enumerates fine as `303a:4001` "OPad ESP32-S3" on `/dev/ttyACM0` (and `/dev/hidraw3`).
- `/dev/ttyACM0` is `root:dialout 0660` with no ACL for the logged-in user; the user is not in `dialout`.
- The daemon logs `Opening OPad serial port at /dev/ttyACM0` every 1.5 s and never `Connected to OPad`.

**Root cause**
- `packaging/linux/udev/99-opad.rules` tags the device `uaccess`, but the file is numbered `99-`.
- systemd only applies the `uaccess` ACL in `73-seat-late.rules`, which sees tags added *before* it runs. A `99-` rule tags too late, so the ACL is never created.

**Fix**
- Rename to `packaging/linux/udev/70-opad.rules` (any number below 73 works).
- Update every reference:
  - `Makefile:106` (install), `Makefile:158` (user-install hint), `Makefile:168` (uninstall)
  - the `# 99-opad.rules` header comment inside the file
  - check `packaging/linux/deb`, `rpm`, `arch`, `appimage` for the filename too
- Fix the spec, which also has the wrong number and the old name: `docs/specs/packaging-distribution.md` lines 187, 441, 448 (`99-osupad.rules`) should be `70-opad.rules`.
- Keep the rule as is otherwise: `uaccess` only, no `MODE`/`GROUP`, PIDs narrowed to `4001|1001` (per L-2).

**Acceptance**
- After install and a replug, `getfacl /dev/ttyACM0` shows `user:<you>:rw-` with no group change and no relogin.
- `test -r /dev/ttyACM0 && test -w /dev/ttyACM0` succeeds and the daemon logs `Connected to OPad`.

**Quick manual check (no reinstall)**
`sudo setfacl -m u:$USER:rw /dev/ttyACM0` (lasts until the next replug).

## 2. Open failures are invisible

- `desktop/crates/opad-device/src/lib.rs:177` logs `Failed to open port` at `debug!`, but the service runs with `RUST_LOG=info`, so the user only sees the retry loop.
- Log the open error at `warn!`, but only when the error changes or once per plug-in, so the 1.5 s retry does not spam the journal.
- For `PermissionDenied`, say what to check (udev rule / `getfacl`), not just the raw error.
- The `info!("Opening OPad serial port ...")` line (line 166) also repeats every retry; log it once per attempt series.

## 3. tosu crashes on every launch

**What happens**
- The daemon logs `tosu exited with exit status: 1` and relaunches with backoff, forever.
- Running `/usr/lib/opad/tosu/tosu` by hand:
  `SyntaxError: Cannot use import statement outside a module` at `dist/index.js:1` (Node v18.19.1).

**Root cause**
- `dist/index.js` is an ES module, but there is no `package.json` with `"type": "module"` next to it and the file is not `.mjs`, so Node treats it as CommonJS.
- The `tosu` target in the `Makefile` (line ~66-82) copies `packages/tosu/dist/*` and writes a wrapper that runs `exec node .../dist/index.js`. It never creates a `package.json`.

**Fix**
- In the `tosu` target, write `{"type":"module"}` into `$(TOSU_BUILD_DIR)/dist/package.json` (or rename the entry point to `.mjs` and update the wrapper). Make sure the `install`, `install-user`, deb, rpm, appimage and PKGBUILD paths all ship it (the install steps use `cp -r $(TOSU_BUILD_DIR)/dist/*`, so a file in `dist/` is picked up).

**Also check (not verified yet)**
- Whether tosu 4.26.2 needs Node newer than 18. Upstream ships it as a packaged binary; we run it with the system `node`.
- Whether the deb/rpm/PKGBUILD declare a dependency on `nodejs` (and a minimum version). If not, add it.
- If tosu keeps failing, the daemon should log tosu's stderr; today only the exit status is visible.

**Acceptance**
- `/usr/lib/opad/tosu/tosu` starts and stays running.
- No `tosu exited` warnings in `journalctl --user -u opad-daemon`.

## 4. Own USB vendor/product ID (decision + follow-up)

Today the pad reports `303a:4001` (Espressif's vendor ID, a self-picked PID; `firmware/main/usb/usb_descriptors.c:27-28`). Options:

- **A. Own VID from the USB-IF (usb.org):** one-time fee, about $6,000 at last check (verify). Needed for the USB logo / commercial sale. Individual or company application, no technical test.
- **B. Free PID from pid.codes (VID `0x1209`):** for open-source hardware/firmware. Submit a PR to the pid.codes repo describing the project and license. Not for closed-source products. Leading candidate, since OPad is open source.
- **C. PID from Espressif under `303a`:** available for products built on their chips (check their current process).

Decision: **B (pid.codes, VID `0x1209`)**, chosen 2026-09-21. Revisit A only if the pad is sold commercially / needs the USB logo.

### pid.codes prerequisites (checked against http://pid.codes/howto/ on 2026-09-21)

- [x] Public source repository: `https://github.com/OPad-dev/OPad` is public.
- [x] Contains PCB design files and firmware/source: `hardware/pcb`, `firmware/`.
- [x] `LICENSE` file in the repo: MIT.
- [x] **License: keep a single MIT `LICENSE` for everything OPad wrote** (decided 2026-09-21). pid.codes wants hardware and software under recognised licenses; MIT is recognised. The README license section now says MIT covers the firmware, desktop app, PCB designs, enclosure CAD and docs, so the hardware is explicitly covered. A hardware-specific license (e.g. CERN-OHL-P-2.0 in `hardware/LICENSE`) is only a follow-up if the pid.codes reviewers ask for one.
- [ ] **Third-party files that are not MIT and must be replaced or removed before the PR / selling.** See section 5.
- [ ] Reserved ranges: `0x0000-0x0FFF` (tests/generic) and `0x1000-0x1FFF` (InterBiometrics) cannot be requested.

### Proposed ID

- **`1209:4F50`** (`4F 50` = ASCII "OP"). Checked against the pid.codes list on 2026-09-21: not taken. Re-check right before the PR; the first merged PR wins.
- Bootloader stays `303a:1001` (see the list below).

### Submission (PR to `pidcodes/pidcodes.github.com`, not sent yet)

Fork the repo, then add two files.

`org/OPad/index.md` (the directory name cannot contain spaces; the `OPad` org page returned 404 today, so it is free):

```
---
layout: org
title: OPad
site: https://github.com/OPad-dev/OPad
---
OPad is an open-source osu! keypad based on the ESP32-S3, with a desktop companion app.
```

`1209/4F50/index.md`:

```
---
layout: pid
title: OPad ESP32-S3
owner: OPad
license: MIT
site: https://github.com/OPad-dev/OPad
source: https://github.com/OPad-dev/OPad
---
Low-latency osu! keypad. ESP32-S3 running TinyUSB (HID keyboard + CDC serial), with Hall-effect and mechanical switch PCBs. Firmware, PCB designs and the desktop app are all in the linked repository.
```

`license: MIT` is correct as long as the third-party files above are resolved (the PR points reviewers at the repo, so the NC-licensed files would be visible to them).

Commit message suggestion: `Add OPad ESP32-S3 (1209:4F50)`. Wait for the PR to be merged **before** shipping firmware with the new ID, and keep the old `303a:4001` accepted by the daemon for one release so existing pads still connect.

### Order of work

1. Resolve the non-MIT files (section 5). The README license section is already updated.
2. Open the pid.codes PR, wait for merge.
3. Then do the coordinated change below.

Changing the ID is a breaking change, so do it in one coordinated step:
1. Firmware descriptor (`usb_descriptors.c`) and reflash.
2. `ESPRESSIF_VID` and `OSUPAD_APP_PID` in `desktop/crates/opad-device/src/lib.rs` (the daemon detects the pad by these). New firmware needs a new daemon and vice versa; consider accepting both IDs for one release.
3. udev rule (`ATTRS{idVendor}` / `ATTRS{idProduct}`), `docs/specs/packaging-distribution.md`, `docs/recovery.md`, `docs/windows-portability.md`, and the error strings in `desktop/crates/opad-device/src/flash.rs`.
4. The ROM bootloader stays `303a:1001` in download mode regardless (comes from the chip, not our firmware), so the bootloader detection and its udev entry keep the Espressif ID.
5. Windows: driver association is cached per VID/PID; re-verify the driverless install on a clean machine.
6. Optionally also check the manufacturer/product strings ("GFerreiroS" / "OPad ESP32-S3") as a second match.

## 5. Repo license audit: everything that is not OPad-owned MIT

Full audit of all 295 tracked files on 2026-09-21 (`git ls-files`), plus the 696 Rust dependencies (`cargo metadata`). Goal: the repo is 100% open source, modifiable and safe to sell from, with everything OPad wrote under MIT. Nothing has been changed or deleted yet.

**How to read this.** We can only put MIT on work we own. Third-party files keep their author's license; we cannot relicense them. For pid.codes (and for selling), what matters is that *every* file is under an open license and its notice is kept. Permissive non-MIT licenses (zlib, OFL, LGPL for tosu) are acceptable to pid.codes. NonCommercial or "no license" are not. Each item below says whether it blocks and what replaces it. A "strict all-MIT" alternative is given where one exists.

### Summary

| # | What | License today | Blocks pid.codes / selling? | Fix |
|---|---|---|---|---|
| 5.1 | kamehameha case STLs | CC BY-NC 4.0 | **Yes** | Remove |
| 5.2 | Easter-egg GIF in firmware | **Unknown** (internet meme GIF) | **Yes** | Replace with our own animation |
| 5.3 | Waveshare board STEP/PDF/DWG/PNG | **No license stated** | **Yes** unless Waveshare allows it | Remove, link to Waveshare |
| 5.4 | KiCad-derived footprints and symbols | CC-BY-SA 4.0 (KiCad library) | Share-alike, not MIT | Redraw ours, or keep with notice |
| 5.5 | nanopb (vendored C library) | zlib | No, but **its license file is missing** (zlib requires the notice) | Add `LICENSE.txt`, or stop vendoring |
| 5.6 | Montserrat fonts in the GUI | SIL OFL 1.1 | No (notice present) | Keep; add to package credits |
| 5.7 | milk-crate and clay53 reference models | MIT (third party) | No, but **their notice is missing** | Add their MIT notice, or remove |
| 5.8 | tosu (bundled at build, not in repo) | LGPL-3.0 | No (already handled) | Nothing, except the tosu fix in section 3 |
| 5.9 | Rust dependencies (not in repo, compiled in) | MIT/Apache/BSD/Zlib/MPL-2.0 etc. | No | Optional: ship a generated third-party notices file |

Everything else (firmware sources, board code, desktop crates, scripts, packaging, docs, PCB designs, production files, enclosure `.scad`/`.stl`, the icon SVG, the stroke font) was checked and is OPad's own work, so MIT.

### 5.1 Must go: CC BY-NC (not open source, forbids selling)

| File | Size | Origin and license | Action |
|---|---|---|---|
| `hardware/3d/reference_keypads/osu_case_top_kamehameha.stl` | 362 KB | Printables #943460 (kameHame HA), **CC BY-NC 4.0** | Remove from the repo, link to the Printables page instead |
| `hardware/3d/reference_keypads/osu_case_bottom_kamehameha.stl` | 162 KB | same | same |

CC BY-NC forbids commercial use, and NonCommercial licenses are not open source under the OSI/OSHW definitions. Nothing in the build uses them, so removal breaks nothing.

### 5.2 Must replace: easter-egg GIF (unknown origin)

| File | Details |
|---|---|
| `firmware/main/ui/easter_egg_gif.h` (98 KB) | A GIF embedded as a byte array: `// Generated from freaky_67_14f.gif (15656 bytes, 14 frames, 180x93)`. Added in `470a9d5`. The commit gives no source or license. |

A meme GIF from the internet is someone else's copyright with no license, so it cannot be MIT. It is also compiled into every firmware image, so it ships on sold pads.

Replace with an animation we make ourselves (drawn frames, or drawn with LVGL primitives at runtime, which also saves ~15 KB of flash). Keep the same size/frame format so `firmware/main/ui/easter_egg.c` does not change. Also check `scripts/trigger_easter_egg.py` and the desktop side (`TriggerEasterEgg`) only reference the name, not the image. If we know the exact creator and they grant a permissive license, keeping it with a credit is the alternative.

### 5.3 Must resolve: Waveshare files (no license stated)

| File | Size | Origin and license | Action |
|---|---|---|---|
| `hardware/3d/waveshare_board/esp32-s3-touch-lcd-2_20241108.stp` | 15 MB | Waveshare official STEP model, no license given in the repo | Check Waveshare's terms. If redistribution is not clearly allowed: remove and link to their wiki. If we still need a board model for the case, model our own simplified one (outline, holes, USB-C position; dimensions are already in `hardware/3d/README.md`) |
| `hardware/3d/waveshare_board/ESP32-S3-Touch-LCD-2-20241108.pdf` | 146 KB | Waveshare mechanical drawing | same |
| `hardware/3d/waveshare_board/ESP32-S3-Touch-LCD-2-20241108.dwg` | 927 KB | Waveshare CAD drawing | same |
| `hardware/3d/waveshare_board/dims-1.png` | 92 KB | Waveshare dimension image | same |

### 5.4 KiCad library copies: CC-BY-SA 4.0 (share-alike, not MIT)

The KiCad standard libraries are CC-BY-SA 4.0 with an exception: **boards and Gerbers made with them are not affected** (so `*.kicad_pcb`, `*.kicad_sch`, Gerbers, BOMs, STEP exports stay MIT). But the **library files themselves**, copied into our repo, stay CC-BY-SA 4.0.

KiCad-derived (by their `descr` text, the KiCad generator notes and `${KICAD10_3DMODEL_DIR}` model paths):

| File | Evidence |
|---|---|
| `hardware/pcb/V1/lib/osupad.pretty/C_0603_1608Metric.kicad_mod` | KiCad `Capacitor_SMD` footprint and model path |
| `.../R_0603_1608Metric.kicad_mod` | KiCad `Resistor_SMD`, IPC-7351 descr |
| `.../JST_SH_SM08B-SRSS-TB_1x08-1MP_P1.00mm_Horizontal.kicad_mod` | KiCad `Connector_JST` |
| `.../PinSocket_1x14_P2.54mm_Vertical.kicad_mod` | "from Kicad 4.0.7", `Connector_PinSocket_2.54mm` |
| `.../MountingHole_2.2mm_M2.kicad_mod` | "generated by kicad-footprint-generator" |
| `.../SOT-23.kicad_mod` | KiCad `Package_TO_SOT_SMD` name and descr style, probably edited (mentions DRV5055) |
| `hardware/pcb/V1/lib/osupad.kicad_sym` symbols `R`, `C`, `SW_Push`, `Conn_01x08`, `Conn_01x14`, `TestPoint` | KiCad `Device`/`Switch`/`Connector` symbols ("script generated (kicad-library-utils/schlib/autogen") |

Ours (MIT): `SW_MX_Hall_PlateMount_SolidCentre`, `TestPoint_Pad_1.0x1.0mm` (generator `"osupad"`), and very likely `Kailh_CPG151101S11_MX_Hotswap_Bottom`, `SW_MX_Hall_PlateMount` and the `DRV5055` symbol (custom descriptions). Confirm those three.

Options:
- **Strict all-MIT (recommended, small job):** redraw the six footprints and six symbols ourselves from the datasheets (0603 R/C, SOT-23, JST SH 8-pin, 1x14 socket, M2 hole; resistor, capacitor, push button, connectors, test point). They are simple parts. Keep the `${KICAD10_3DMODEL_DIR}` model *references* (a path, not a copy, so no license issue).
- **Or:** keep them and add `hardware/pcb/V1/lib/LICENSE-KiCad` (CC-BY-SA 4.0 + the KiCad exception text) and a line in the README. Open source and acceptable to pid.codes, just not MIT.

### 5.5 nanopb: zlib license, notice missing

| Files | Details |
|---|---|
| `firmware/main/protocol/nanopb/pb.h`, `pb_common.c/.h`, `pb_encode.c/.h`, `pb_decode.c/.h` | Vendored copy of nanopb (Petteri Aimonen), **zlib license**. The upstream `LICENSE.txt` was not copied. `pb_common.c` also contains a UTF-8 routine by Markus Kuhn ("Short code license", MIT-compatible). |
| `firmware/main/protocol/osupad.pb.c/.h` | Generated from our `protocol/osupad.proto` by nanopb 0.4.9.1. Generated output of our schema, so ours (MIT). |

zlib's condition 3 says the notice "may not be removed or altered from any source distribution", so today the repo technically violates it. Fix, pick one:
- **Keep vendored:** add nanopb's `LICENSE.txt` as `firmware/main/protocol/nanopb/LICENSE.txt` (zlib text, "Copyright (c) 2011 Petteri Aimonen"; copy the exact file from nanopb 0.4.9.1 upstream). zlib is permissive and OSI-approved; fine for pid.codes and selling.
- **Strict all-MIT in the repo:** stop vendoring and pull nanopb at build time (ESP-IDF component manager or a CMake `FetchContent` pinned to 0.4.9.1). The firmware binary still contains zlib code, which is allowed.

### 5.6 Montserrat fonts: SIL OFL 1.1 (fine, notice present)

| Files | Details |
|---|---|
| `desktop/gui/assets/fonts/Montserrat-Bold.ttf`, `Montserrat-Medium.ttf` | "Copyright 2011 The Montserrat Project Authors", SIL OFL 1.1. `Montserrat-OFL.txt` is present. Compiled into the GUI binary via `include_bytes!` (`desktop/gui/src/theme.rs:31-32`). |

OFL is a recognised open font license, allows commercial use and embedding. No change required. To do:
- Add Montserrat (OFL-1.1) to `packaging/linux/deb/copyright`, the PKGBUILD `license=()` array, `Cargo.toml` `license` in `desktop/gui` for the deb/rpm (`MIT AND LGPL-3.0-only AND OFL-1.1`), and the Windows installer's license files, since the fonts ship inside the binary.
- A strict all-MIT font does not realistically exist among good UI fonts (almost all open fonts are OFL). Keep OFL.
- Also: the firmware's LVGL build enables LVGL's built-in Montserrat bitmaps (`CONFIG_LV_FONT_MONTSERRAT_12/14/16` in `firmware/sdkconfig.defaults`). They come from the LVGL component (MIT), fetched at build time, not stored in our repo. Mention them in the firmware credits.

### 5.7 milk-crate and clay53 reference models: MIT, but not ours

| File | Origin |
|---|---|
| `hardware/3d/reference_keypads/milkcrate-plate-mount.step/.stl`, `milkcrate-pcb-mount.step/.stl` | somepin/milk-crate, MIT |
| `hardware/3d/reference_keypads/osu_keypad_clay53.FCStd` | clay53/Osu-Keypad, MIT |

These are MIT already, but with *their* copyright. MIT requires their copyright and license notice to stay with the files; today there is only a credit line. Either add each upstream `LICENSE` next to the files (e.g. `reference_keypads/LICENSE-milk-crate`, `LICENSE-osu-keypad`), or remove them: they are only reference material, nothing in the build uses them. Removing is simpler and makes `hardware/` 100% OPad-authored.

### 5.8 tosu (LGPL-3.0)

Not stored in the repo; built from upstream source at package time, with `licenses/tosu/{LICENSE,NOTICE,VERSION}` in the repo and shipped in every package, plus the `OSUPAD_TOSU_PATH` replacement option. This is already correct. It stays LGPL; it cannot be MIT. It is a separate program run as a subprocess, so it does not affect OPad's MIT license.

### 5.9 Rust dependencies (compiled into the desktop binaries, not in the repo)

696 crates. All permissive or dual-licensed with a permissive option: MIT / Apache-2.0 (the vast majority), BSD-3-Clause, Zlib, ISC, Unicode-3.0, CDLA-Permissive-2.0 (`webpki-roots`). `serialport` and `option-ext` are MPL-2.0 (file-level copyleft: fine as long as we do not modify their files; their source is on crates.io). The four crates listing GPL/LGPL (`r-efi`, `self_cell`, `unescaper`) are dual-licensed; we use them under their MIT/Apache option. No blocker.

Nice to have: generate a third-party notices file at release time (`cargo about` or `cargo-deny` with a license allowlist in CI) and ship it with the GUI (MIT/Apache require including their notices in binary distributions).

### Follow-ups when removing

- Update `hardware/3d/README.md`: the directory tree (lines 8-31), the "Option A" instructions (lines 57-58 reference `milkcrate-plate-mount.step` and the Waveshare STEP), and section 4 credits.
- `hardware/3d/custom_case/V1/osupad_enclosure.scad` has no imports, so the case build does not depend on any file above (checked). Still confirm the case design was not derived from the kamehameha STLs; if it was, it must be redrawn.
- **Git history:** deleting the files does not remove them from past commits, so the CC BY-NC files remain downloadable from history. Decide whether that is acceptable or whether to rewrite history (disruptive, changes every commit hash, needs a force-push). For a small project before the first public release, rewriting is cleanest.
- Repo weight: removing `waveshare_board` and the reference keypads would shrink the repo by about 23 MB.
- Once done, update the README license section: list only what remains third-party (tosu, Montserrat, nanopb if still vendored, KiCad library copies if kept).
- Suggested prevention: a `REUSE`-style check (`reuse lint`) or a CI job that fails on files without a known license, so new third-party files cannot slip in again.

### Minimum to be pid.codes-ready

1. 5.1 remove the CC BY-NC STLs.
2. 5.2 the GIF is kept (decision 2026-09-21); its license is still unresolved.
3. 5.3 remove the Waveshare files (or confirm Waveshare allows redistribution).
4. 5.5 add nanopb's `LICENSE.txt`, and 5.7 add or remove the reference models.
5. 5.4 redraw the KiCad-derived parts (for strict MIT) or add the CC-BY-SA notice.

## 6. Rebuild and retest

(Do the ID change in section 4 in the same rebuild only if the decision is made; otherwise keep `303a:4001` for now.)

1. `make deb` (rebuild `dist/opad_1.0.0-rc-1_amd64.deb`).
2. `sudo apt install ./dist/opad_*.deb`, replug the pad.
3. Check: ACL on `/dev/ttyACM0`, `Connected to OPad` in the daemon log, tosu running, GUI shows the pad.

## Notes

- The daemon detects the pad by USB ID `303a:4001` (`OSUPAD_APP_PID` in `desktop/crates/opad-device/src/lib.rs`), the value set in `firmware/main/usb/usb_descriptors.c:28`. `303a` is Espressif's vendor ID and `4001` is a custom PID chosen for OPad. It ignores the ROM bootloader (`303a:1001`) on purpose, because opening that port resets the chip out of download mode.
- Uncommitted change in the working tree, unrelated to the above: `desktop/daemon/src/ipc_handlers.rs` (`device_manager` to `device` in `TriggerEasterEgg`).
