# AGENTS.md

Rules for anyone working on this repository with the help of an AI tool
(Claude Code, Codex, Copilot, Cursor, Gemini, Aider, ...), and for the AI
itself. Human contributors are welcome to follow it too.

OPad is a low-latency two-key osu! keypad on the Waveshare ESP32-S3-Touch-LCD-2,
with a Rust desktop suite (daemon, GUI, `opadctl`) for Linux and Windows 10/11.
See [README.md](README.md) for the overview and [docs/](docs/README.md) for
details.

---

## 1. Product rules (never trade these away)

1. **Latency always wins.** If a feature measurably worsens key latency or
   latency jitter, it is reduced, deferred, frozen during gameplay, or removed.
   Nothing joins the keypress path: the screen, storage, tray and tosu all stay
   off it. If a change touches `firmware/main/input/` or `firmware/main/usb/`,
   say how it affects latency and how you checked.
2. **The pad is a keyboard with no software installed.** It is a 1000 Hz USB
   HID keyboard that needs no driver and no setup on Linux or Windows 10/11.
   The desktop software is optional and only adds features. Never make basic
   typing depend on the daemon, the GUI or a driver.
3. **The app and the pad update independently.** Every protocol or wire-format
   change must work both ways: new app with old firmware, and old app with new
   firmware. Detect and negotiate instead of switching formats (for example,
   the pad answers in the framing the host used). Never tell a user to "update
   both together". State the result in the commit (see §3).
4. **The touchscreen is the third button.** It must give a clean press, a held
   state while the finger stays down, and a clean release, not just tap
   detection. Gestures are planned but not built yet: swipe up/down will set
   the osu! volume; left/right is still to be decided. Don't make design
   choices that would block either. Keep it cheap on CPU (idle on the touch
   interrupt, poll only while a finger is down).

## 2. Workflow

- **Maintainer:** work in a worktree on its own branch, then fast-forward
  merge into `main`.
- **Everyone else:** fork the repository, branch in your fork, and open a pull
  request against `main`.
- **History stays linear:** rebase, no merge commits.
- **Branches only exist while work is happening on them.** After merging,
  delete the branch locally and on the remote, and remove its worktree.
- **Test before you call it done** (see §4). If something could not be tested,
  for example no pad was connected, say so in the PR or commit. Never claim it
  works.

## 3. Commits

- **Sign as yourself.** Commit with your own name and email. An AI tool must
  never make itself the author.
- **Disclose AI use** with one trailer at the end of the message:
  - `AI: assisted`: a person wrote or directed the change and an AI helped;
  - `AI: generated`: an AI wrote all of it, a person reviewed and committed it.

  No trailer means no AI was used. Replace tool-specific attribution lines
  (`Co-Authored-By: <model>`, session links) with this trailer.
- **Conventional commits:** `type(scope): subject`. Write the subject as a
  plain statement of the behaviour, for example
  `fix(ppm): replays are not measured, and tosu-timed peaks need 0.4 s`.
- **Reference the issue or review finding** in the subject: `(#2)`, `(DA#9)`.
- **Protocol/IPC compatibility:** when relevant, add a trailer such as
  `Compatibility: no protocol change.` or a line explaining how old and new
  versions interoperate.

Example:

```
feat(touch): hold state survives a missed release read (#14)

The release needs two empty reads in a row, so one noisy read no
longer ends a hold.

Compatibility: no protocol change.
AI: assisted
```

## 4. Build and test

| What | Command |
|---|---|
| Everything CI checks on the desktop side + firmware host tests | `make check` |
| Desktop | `cd desktop && cargo build` |
| Firmware (ESP-IDF **v5.5.2**) | `cd firmware && idf.py set-target esp32s3 build` |
| Regenerate protobuf | `./scripts/gen_proto.sh` |
| Flash a pad | `opadctl flash firmware/build/opad-firmware.bin` (`--full firmware/build` for recovery) |

`make check` runs `cargo fmt --check`, clippy with `-D warnings`, the workspace
tests and `firmware/test/host/run_tests.sh`. CI additionally:

- builds and tests on Windows (the GUI is a separate job);
- regenerates protobuf and fails on any diff;
- builds the firmware in `espressif/idf:v5.5.2`;
- checks licences (REUSE via `scripts/check_reuse.py`, cargo-deny, third-party
  notices).

The firmware builds on any machine with ESP-IDF v5.5.2. Build it before calling
a firmware change unverified. Changes that touch input, USB, touch or the
screen also need a test on a real pad; `docs/testing-checklist.md` has the rows
to follow.

## 5. Rules that are easy to break

- **Generated protobuf:** `firmware/main/protocol/osupad.pb.c/.h` are
  generated. Edit `protocol/osupad.proto` (and `osupad.options`), then run
  `scripts/gen_proto.sh`. Never hand-edit the generated files. Vendored nanopb
  lives in `firmware/main/protocol/nanopb/`; don't modify it.
- **`sdkconfig.defaults`:** after editing it, delete `firmware/sdkconfig` and
  rebuild. `idf.py fullclean` keeps the old saved config, so the edit silently
  does nothing.
- **Pinned versions:** don't bump these casually:
  - espflash **4.6.0** (checksum in the `Makefile`): a different version once
    silently stopped resetting the S3 out of download mode;
  - tosu **v4.26.2**;
  - LVGL as locked in `firmware/dependencies.lock`.
- **Licensing:** the repo is REUSE compliant. New files need licence info
  (header or `REUSE.toml`). The easter-egg GIF is deliberately the only
  `LicenseRef-Unknown-ThirdParty` file; don't add another.
- **Never commit** `build/`, `target/`, `dist/`, `sdkconfig`,
  `managed_components/`, or anything from `.tools/`.

## 6. Hardware and debugging facts

- **Keys "do nothing" after a pin change:** first compare the GPIO label the
  wire is actually on (silkscreen) with Key Pins in `opadctl status`. Only then
  suspect the firmware. The GUI dropdown names schematic header positions
  ("GPIO14 (P2 pin 11)"), which is easy to mix up with the silkscreen.
- **Serial port busy:** the daemon holds the pad's CDC port (`/dev/ttyACM0`,
  or a COM port on Windows). Stop it, or use `opadctl flash`, which asks it to
  release the port.
- **USB reboot into download mode** (`reboot_to_rom_download()` in
  `firmware/main/usb/usb_cdc.c`, host side in
  `desktop/crates/opad-device/src/flash.rs`). These rules were hard to find:
  - the host must see a real detach (SE0 about 300 ms) and then a re-attach,
    or it fails with `error -71` and the app comes back;
  - set `dp_pullup` back to 1 *before* dropping the pull override, or the board
    vanishes from the bus until a manual BOOT+RESET;
  - call `tud_disconnect()`, never `tinyusb_driver_uninstall()`, which frees
    mutexes other tasks still use;
  - leaving download mode needs an RTC watchdog reset. espflash's own
    `--after watchdog-reset` is a silent no-op on the S3, so the flasher runs
    with `--after no-reset-no-stub` and resets with its own `WRITE_REG`.
- **USB IDs:** app `303a:4001`, ROM bootloader `303a:1001`.
- **VMs:** with the pad passed through to a VM, flashing fails because the
  pad's USB ID changes mid-flash. Test flashing on bare metal.
- **Board:** Waveshare ESP32-S3-Touch-LCD-2. Schematic:
  <https://files.waveshare.com/wiki/ESP32-S3-Touch-LCD-2/ESP32-S3-Touch-LCD-2-SchDoc.pdf>

  | Pin | P1 | P2 |
  |---|---|---|
  | 1 | IO2 | 3V3 |
  | 2 | IO4 | GND |
  | 3 | IO6 | IO43 (UART0) |
  | 4 | IO16 | IO44 (UART0) |
  | 5 | IO17 (pulled down) | IO47 (touch/IMU I2C) |
  | 6 | IO18 | IO48 (touch/IMU I2C) |
  | 7 | IO21 | IO15 |
  | 8 | IO8 | IO13 |
  | 9 | IO7 | IO11 |
  | 10 | IO10 | IO12 |
  | 11 | IO20 (USB D+) | IO14 (Key 1 default) |
  | 12 | IO19 (USB D-) | IO9 (Key 2 default) |
  | 13 | GND | GND |
  | 14 | 5V (raw VBUS) | VBAT |

## 7. Documentation

- `docs/` grew by accretion and is **possibly stale**. The code and `git log`
  are authoritative.
- Every factual claim in a doc comes from a source file read while writing it,
  and cites that path.
- When code and docs disagree, the code wins: fix or delete the doc.
- No placeholder pages.
- Old reviews and snapshots belong in `docs/history/`; they record what
  happened, they are not current guidance.

## 8. How an AI should behave here

- Read the code before stating how it works; don't trust these docs over it.
- Keep changes scoped to the task. Ask before changing the protocol, the
  keypress path, pinned versions or the user-facing UX.
- Match the surrounding code's style and comment density.
- Report results honestly: failing tests, skipped steps and untested hardware
  are said plainly.
- Don't push, open PRs, delete branches or flash hardware unless the person
  you're working with asked for it.
