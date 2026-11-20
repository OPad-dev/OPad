# V3 modules: handoff (2026-11-20)

This is the state of the `feat/v3-modules` work, written so another session can
pick it up. It is a snapshot, not current guidance: the code and `git log` win.

## Where things are

- **Worktree:** `../osu-pad-esp32-v3` (from the main checkout), branch
  `feat/v3-modules`, based on `main` at `1502499`. `git log main..HEAD` lists
  the work.
- **Not pushed, not merged.** AGENTS.md §2: fast-forward into `main` once the
  owner says so, then delete the branch and the worktree.
- **Merge order:** `hw/redesign` goes into `main` first. Another session is
  working on it in `../osu-pad-esp32-hw`; leave that worktree alone. The V3
  hardware files live only there for now (for example
  `hardware/pcb/V3/README.md`, `hardware/verify/golden/he_v3.toml`,
  `mx_v3.toml`), and the code cites them as "on hw/redesign".
- **Installed on the owner's machine:** the desktop app and the firmware from
  this branch's head (`make install-user`, `opadctl flash`).

## Hardware on the bench

- Waveshare ESP32-S3-Touch-LCD-2 on the V3 carrier, USB-C on the V3 USB board.
- Module cable: 14-way FFC, **type B**, contact 1 to contact 1 (ID on GPIO8).
  USB cable: 8-way, type A. Every pad ships with these cables, so the
  turned-over module cable (ID on GPIO7) is not a case to test; the firmware
  detects it but nobody needs it.
- Two modules, swapped with USB out (the pad reads the module at boot):
  - **HE V3**, ID 1.655 V. Sensors at rest about 2198 codes with keys fitted,
    noise 0.8 codes rms; ADS7028 clean to 40 MHz SCLK; sampled at 20 kHz with
    CRC on the SPI link.
  - **MX V3**, ID 2.26-2.27 V. Keys on GPIO10, GPIO7 and GPIO6 (key 3, FFC
    contact 5, pull-up R3).
- Owner's settings: K1 Z, K2 X, K3 C; game keys K1 + K2.

## Measured

| Module | Samples | p50 | p99 | p99.9 | max |
|---|---|---|---|---|---|
| HE V3 (sampler reports from IRAM, `08823ad`) | | 20 µs | | 40 µs | 36 µs |
| MX V3, two-key firmware | 1920 | 30 µs | 60 µs | 60 µs | 54 µs |
| MX V3 with key 3 (`e989d7d`) | 792 | 30 µs | 40 µs | 50 µs | 46 µs |

All device-side, key edge to HID submit, over a map; 0 deferred reports.

## Done and tested on the pad

See `docs/testing-checklist.md` HW-06 to HW-15 and BAK-01 for the rows. In
short: board test of both V3 modules, three HE V3 keys with calibration on the
pad and from the app, three MX V3 keys, key 3's usage and the game keys,
JSON backups with key 3, the Diagnostics key 3 tester, and the key pin
settings hidden on a pad with a module (`HelloAck.input_module`).

**Fixed on the way:** every board test left key 2 held on an MX pad (V1 too),
because opening the module-ID ADC configured GPIO7's channel and turned the
pin analog (`1fab1fe`).

## Open

1. **Dropped host messages.** During an HE map the pad dropped two messages
   (231 and 175 bytes) as the map ended. `5715b9f` makes the next one say
   nanopb's reason, the HostToDevice payload and its first bytes
   (`opadctl monitor`). It has not happened again yet. When it does, fix the
   host or the nanopb limits (`protocol/osupad.options`).
2. **Not run, judged not needed by the owner:** the magnet test (HW-07),
   `evtest` and an empty key 3 (HW-08), the lock with key 3 uncalibrated
   (HW-10), calibration at 20 kHz on the last build, a failed HE sensor, CRC
   during a calibration, and a hand-wired pad for HW-15.
3. **MX V3 on a turned-over cable** is not read (its keys would land on GPIO6,
   GPIO8 and GPIO10). Not needed while every pad ships with the type B cable.
4. **Merge**, after `hw/redesign` and the owner's go.

## Design decisions made with the owner

- **Key 3 sends 'C' by default.** It can be changed or emptied, and empty sends
  nothing.
- **Counters:** key 3 has its own lifetime counter, not added to the others.
  The **game keys** (one or two, default 1+2) decide what counts toward
  totals, the tap rate and map stats. The pad only puts game keys in the press
  log the app's tap rate uses.
- **HID:** key 3 rides on the endpoint the other game key does not use, so two
  game keys never wait on each other (`usb_hid_set_key3`).
- **An uncalibrated key 3 locks an HE V3 pad** (decision D7), like keys 1 and 2.
- **Storage must survive an OTA rollback.** The v7 config blob never grows.
  Key 3 data lives in separate NVS records: `cfg_ext`, `he_cfg3`, and `cal3_a`
  / `cal3_b` in namespace `osupad_cal3` (V3 calibration only).
- **Key pin settings** show only on a pad without a module (hand-wired), or
  with firmware that does not say which module it has.
- **Component datasheets:** facts go in `docs/hardware/components/` as text.
  The PDFs are not committed (REUSE); `scripts/fetch_datasheets.sh` downloads
  them.
- **Commits:** author GFerreiroS, trailer `AI: assisted`, no AI co-author.
  Commit each tested step without asking. Ask before pushing, merging, or
  changing the protocol incompatibly, the keypress path or the UX.

## Where the code is

- **Firmware**
  - Module ID: `input/module_id.[ch]` (pure); reading it,
    `boards/.../board.c` (`module_id_chan_ready` configures one ADC channel
    per read, never on an armed key pin).
  - Pins: `board_pins.h` (`BOARD_HE3_*`, `BOARD_MX3_KEY3_GPIO`).
  - MX keys: `input/keypad.c` (`KEYPAD_KEY_COUNT` 3, `keypad_set_key3_gpio`),
    `board_key3_*` in `board.c`; set up in `app_main.c`.
  - ADS7028: `input/ads7028*.{c,h}` (driver), `input/he_spi_source.c` (GPTimer
    and sampler task), `input/he_spi_frame.c` (pure decode).
  - HE backend: `input/he_backend.c`, with `process_frame` shared by V1 and V3.
    Key 3 through the HE stack: `HE_KEYS_MAX = 3` in `he_params.h`, plus
    `he_cal*`, `he_health`, `he_cal_samples.h`, `he_cal_ui.c`.
  - Board test: `diag/board_test.c`, `diag/he_adc_test.c`.
  - Game keys on the screen: `input/game_keys.h`, `ui/ui_port.c`.
  - HID: `usb/usb_hid.c`. Config: `config/device_config.c`. Counters:
    `counters/counters.c` (`key3_cnt`).
  - Decode diagnostics: `protocol_decode_host_message` in
    `protocol/protocol.c`, `DIAG_DECODE_*` in `diag/diag.h`.
- **Protocol:** `protocol/osupad.proto`. Fields added on this branch include
  `HelloAck.key_count` and `input_module`, `DeviceStatus.lifetime_key3` and
  `map_key3`, `HeCalibrationStatus.key3_*`, `ConfigPayload.key3_hid_usage`,
  `key3_he` and `game_keys`, `HE_CAL_STAGE_KEY3`, `HeLiveSample.key3_*`,
  `BoardTestResult.cable_reversed` and `he_adc`, and `InputModule` values 4
  and 5.

  Regenerate with `scripts/gen_proto.sh`, using a venv with `protobuf`,
  `grpcio-tools` and `nanopb==0.4.9.1` on PATH (the IDF python env lacks
  protobuf): `env -u IDF_PATH PATH=<venv>/bin:$PATH ./scripts/gen_proto.sh`.
- **Desktop**
  - Board test model and checks: `opad-model/src/board_test.rs`.
  - `DeviceConfig` key 3 and game keys, `Key3Counters`, `DeviceInfo`
    (`input_module`, `key_pins_configurable`): `opad-model/src/lib.rs`.
  - Diagnostic event text: `opad-model/src/diag.rs`.
  - Mapping: `opad-device/src/lib.rs`.
  - Daemon: `daemon/src/runtime.rs` (`key3` state, `pad_presses` slot mapping).
  - Storage: `opad-storage` (v17).
  - GUI: `gui/src/keys.rs`, `pages.rs` (Settings key 3, game keys, key pins,
    calibration), `diagnostics.rs` (PCB Test, magnet test, Key 3 tester).
  - CLI: `cli/src/main.rs` (board-test groups and `--magnet`, status).

## Checks

- `make check` passes at the branch head: fmt, clippy `-D warnings`, workspace
  tests, the firmware host tests.
- Firmware build: `cd firmware && idf.py build` (ESP-IDF v5.5.2, already set to
  esp32s3 in the worktree).
- A GUI closed to the tray keeps running the old binary after
  `make install-user`: quit it and start it again.
