# V3 modules: handoff (2026-11-20)

This is the state of the `feat/v3-modules` work, written so another session can
pick it up. It is a snapshot, not current guidance: the code and `git log` win.

## Where things are

- **Worktree:** `../osu-pad-esp32-v3` (from the main checkout), branch
  `feat/v3-modules`, based on `main` at `1502499`.
- **Commits so far:**
  - `9d63eb6` docs(hardware): component notes, starting with the ADS7028
  - `9afb870` feat(he): Hall Effect V3 reads three keys over SPI, and key 3 is configurable
  - `1f62d72` feat(display): the key cards and totals follow the game keys, and Diagnostics tests key 3
  - plus the commit that adds this file
- **Not pushed, not merged.** AGENTS.md §2: fast-forward into `main` once the
  owner says so, then delete the branch and the worktree.
- **The V3 hardware files live only on branch `hw/redesign`** (worktree
  `../osu-pad-esp32-hw`), for example `hardware/pcb/V3/README.md` and
  `hardware/verify/golden/he_v3.toml`. The code cites them as "on hw/redesign".
  Open question for the owner: merge `hw/redesign` before or after this branch.
- **Installed on the owner's machine:** the desktop app from this branch
  (`make install-user`). The pad (`OSUPAD-3CDC75701678`) runs the
  `9afb870`-era firmware, flashed with `opadctl flash`. The `1f62d72` firmware
  is built but **not flashed**.

## Hardware on the bench

- Waveshare ESP32-S3-Touch-LCD-2 on the V3 carrier, with the USB-C on the
  external V3 USB board (it works).
- HE V3 module with switches fitted. Keys 1 and 2 are one switch model, key 3
  another from the same maker. FFC straight: the ID comes on GPIO8.
- Mounting: the Waveshare's USB-C points to the same end as the carrier's
  8-way FFC connector, and IO2 goes in the carrier's top-right square pad. It
  was once mounted 180° round, which put 5 V on 3V3; it survived, and every
  test passes.
- Measured:
  - module ID 1.655 V;
  - ADS7028 clean up to 40 MHz SCLK;
  - sensors at rest about 2198 codes with keys fitted, noise 0.8 codes rms;
  - frame-to-HID latency p50 40 µs, p99.9 70 µs, plus up to 250 µs of frame
    wait (same as V1).

## What works (tested on the pad)

- **Board test of the HE V3**, `opadctl board-test` and the PCB Test tab: ADC
  answers, register walk, CAL, SCLK sweep, the three sensors, BOR. All pass.
- **HE V3 key input** for keys 1, 2 and 3. Three-key calibration works on the
  pad and from the app. The keys type Z/X/C.
- **Key 3 HID usage and the game keys**: set in the app's Settings, saved on
  the pad (`cfg_ext`) and in the app's database (migration v17).
- **`opadctl status`** shows K3 and the game keys.

## Done but not run on the pad

- `1f62d72`:
  - the pad's key cards follow the game keys;
  - totals and KPS add up the game keys only;
  - key card labels;
  - the Key 3 card in the Diagnostics tester.
- The magnet test UI (`opadctl board-test --magnet` and the PCB Test card).
  The owner never ran it.
- Turned-over FFC (ID on GPIO7, swapped pins). No hardware test yet.
- MX V3 (2 keys on a straight cable). No board exists yet.

## Next steps, in order

1. **Flash `1f62d72`:** `opadctl flash firmware/build/opad-firmware.bin`
   (rebuild first with ESP-IDF v5.5.2). Then check:
   - with game keys 1 and 3, the second key card shows C and its counts;
   - the Diagnostics Key 3 card;
   - totals.
2. **JSON backups:** add `config.key3` (char), `config.game_keys` and
   `stats.lifetime_key3`. All `#[serde(default)]`, in
   `desktop/crates/opad-model/src/lib.rs` (`JsonBackup*`, around line 895), plus
   import in `daemon/src/ipc_handlers.rs` (around 1046, where `kept` preserves
   them today). I was about to start this.
3. **ADS7028 CRC** on the streaming link (the brief requires it). The datasheet
   section read so far (SBAS978A 8.3.6) is vague on the frame layout with CRC
   on. Read the CRC figures, or probe the chip. See
   `docs/hardware/components/ads7028.md`.
4. **Tap rate with three game keys:** the daemon merges the third key into the
   second finger (`DeviceConfig::game_key_slot`). The pad gives a third game
   key no card. Decide with the owner whether a third card or sources
   (`pad.k3_*`, new UI source ids, wire format) are wanted.
5. **Product default:** `CONFIG_OSUPAD_HE_V3` defaults to y; V1's
   `CONFIG_OSUPAD_HE_BACKEND` stays n. Confirm with the owner.
6. Update `docs/testing-checklist.md`: HW-07 has results; add rows for key 3,
   game keys and calibration with three keys.
7. Merge, after the owner's go.

## Design decisions made with the owner

- **Key 3 sends 'C' by default.** It can be changed or emptied, and empty sends
  nothing.
- **Counters:** key 3 has its own lifetime counter, not added to the others.
  The **game keys** (default 1+2, any combination) decide what counts toward
  totals, the tap rate and map stats. The pad only puts game keys in the press
  log the app's tap rate uses.
- **HID:** key 3 rides on the endpoint the other game key does not use, so two
  game keys never wait on each other (`usb_hid_set_key3`).
- **An uncalibrated key 3 locks a V3 pad** (decision D7), like keys 1 and 2.
- **Storage must survive an OTA rollback.** The v7 config blob never grows.
  Key 3 data lives in separate NVS records:
  - `cfg_ext`;
  - `he_cfg3`;
  - `cal3_a` and `cal3_b` in namespace `osupad_cal3` (V3 calibration only).
- **Component datasheets:** facts go in `docs/hardware/components/` as text.
  The PDFs are not committed (REUSE); `scripts/fetch_datasheets.sh` downloads
  them.
- **Commits:** author GFerreiroS, trailer `AI: assisted`, no AI co-author.
  Commit each tested step without asking. Ask before pushing, merging or
  flashing.

## Where the code is

- **Firmware**
  - Module ID: `input/module_id.[ch]` (pure).
  - Board: `boards/.../board.c` (ID on GPIO8 or GPIO7, `board_keys_set_detached`).
  - Pins: `board_pins.h` (`BOARD_HE3_*`).
  - ADS7028: `input/ads7028*.{c,h}` (driver), `input/he_spi_source.c` (GPTimer
    and sampler task), `input/he_spi_frame.c` (pure decode).
  - Backend: `input/he_backend.c`, with `process_frame` shared by V1 and V3,
    `source_*`, and `he_backend_run_paused`.
  - Key 3 through the HE stack: `HE_KEYS_MAX = 3` in `he_params.h`, plus
    `he_cal*`, `he_health`, `he_cal_samples.h`, `he_cal_ui.c`.
  - Board test: `diag/he_adc_test.c`.
  - Game keys on the screen: `input/game_keys.h`, `ui/ui_port.c`.
  - HID: `usb/usb_hid.c`.
  - Config: `config/device_config.c` (`cfg_ext`, `he_cfg3`).
  - Counters: `counters/counters.c` (`key3_cnt`).
- **Protocol:** `protocol/osupad.proto`. Fields added:
  - `HelloAck.key_count`;
  - `DeviceStatus.lifetime_key3` and `map_key3`;
  - `HeCalibrationStatus.key3_*`;
  - `ConfigPayload.key3_hid_usage`, `key3_he` and `game_keys`;
  - `HE_CAL_STAGE_KEY3`;
  - `HeLiveSample.key3_*`;
  - `BoardTestResult.cable_reversed` and `he_adc`;
  - `InputModule` values 4 and 5.

  Regenerate with `scripts/gen_proto.sh`, using a venv with `protobuf`,
  `grpcio-tools` and `nanopb==0.4.9.1` on PATH. The IDF python env lacks
  protobuf.
- **Desktop**
  - Board test model and checks: `opad-model/src/board_test.rs`.
  - `DeviceConfig` key 3 and game keys, `Key3Counters`: `opad-model/src/lib.rs`.
  - Mapping: `opad-device/src/lib.rs`.
  - Daemon: `daemon/src/runtime.rs` (`key3` state, `pad_presses` slot mapping).
  - Storage: `opad-storage` (v17).
  - GUI: `gui/src/keys.rs` (3 keys), `pages.rs` (Settings key 3 and game keys,
    dashboard K3, calibration), `diagnostics.rs` (PCB Test suite, magnet test,
    Key 3 tester).
  - CLI: `cli/src/main.rs` (board-test groups and `--magnet`, status).

## Checks

- `make check` passes at `1f62d72`: fmt, clippy `-D warnings`, workspace tests,
  27 firmware host tests.
- Firmware build: `cd firmware && idf.py build`. It is already set to esp32s3
  in the worktree.
