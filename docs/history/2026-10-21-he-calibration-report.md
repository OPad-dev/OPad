<!-- SPDX-FileCopyrightText: 2026 GFerreiroS <info@gferreiro.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Hall Effect calibration review — 2026-10-21

Scope: `feat/he-night`, changes after `0fbcb6d`, including the ten original
calibration commits and the follow-up commits below (`git log 0fbcb6d..HEAD`).
No pad was connected. This is a software review and host/build verification,
not hardware sign-off. Owner-approved touch-start and stale-reload fixes are
recorded in Q4/Q7. The strict HE reporting cutoff remains a separate decision
in Q8; this report does not claim complete D7 output isolation.

## 1. Commits

Original commits, oldest first (IDs from `git log`, one-line summaries):

- `00f4502` — document the calibration plan and align capture parameters with code.
- `7fee807` — add pure-C button hit detection and the multi-stage runner.
- `a2364db` — add lock, screen-mode and start-refusal decision tests.
- `b5100ab` — add calibration commands and progress messages.
- `276bcc3` — add the firmware lock screen and calibration flow.
- `1c08dd2` — add the desktop page and daemon auto-launch.
- `9364e62` — return failed first-use calibration to the lock screen.
- `f63d0e6` — gate desktop calibration on the HelloAck capability.
- `45a8ac4` — correct names and behavior in the plan.
- `b4611a8` — report raw 0 before the first sample.

Review commits (review finding IDs CAL#1 onward):

- `21ca183` — retain the ordered frame stream during calibration (CAL#1).
- `f574658` — retain touch coordinates and clear held touch keys (CAL#2).
- `ccd1714` — preserve old calibration until saving succeeds; serialize lifecycle/progress (CAL#3).
- `71dda42` — guard desktop requests and reject stale calibration replies (CAL#4).
- `9947446` — exercise concurrent complete NVS transactions (CAL#5).
- `4fdd9bb` — model immediate NVS writes and late errors (CAL#6).
- `3ec4800` — show snapshot sampling can save a wrong full value (CAL#7).
- `c6617ce` — preserve the required fresh release across parameter reloads (CAL#8).
- `3beb8c1` — refuse calibration until the current touch cleanup finishes (CAL#9).
- The commit containing this report — correct documentation and record remaining decisions.

No existing commit was rewritten. New commits use the configured maintainer
identity, `Compatibility:` and `AI: generated` trailers (`git log --format=full`).

## 2. Findings and changes

### Sampling (CAL#1)

The old LVGL timer read one newest sample pair every 20 ms. Its sequence
counter detected freshness, but did not recover the intervening 250 µs frames.
That loses peaks and lengthens count-based rest/settle windows. The replacement
accumulates each frame's newest conversion pair, in order, and per-key extrema
in two bounded buffers. The reader resets its detached spare outside `s_lock`,
then swaps ownership inside the lock. Capture runs outside the ISR on the
detached data (`firmware/main/input/he_backend.c`, `he_cal_samples.h`,
`he_cal_runner.c`, `firmware/main/ui/he_cal_ui.c`).

A single min/max pair per display tick is insufficient: rest → press → rest
and the return of one press followed by another press can have the same
extrema. Replaying extrema in a fixed order can invent or merge strokes.
Ordered samples retain both the `press_arm_span` and `release_arm_span`
crossings and the actual rest distribution. Each buffer holds 512 frame pairs
(128 ms at 4 kHz); overflow or an invalid pair fails the attempt without
publishing its results. This retains the median of five and the existing
capture thresholds (`firmware/main/input/he_cal_samples.h`, `he_cal_runner.c`,
`he_cal_capture.c`). These are frame samples, not every ADC conversion.

The runner now discards 64 frame samples for settling (16 ms), then captures
64 rest samples (16 ms). The old 16-sample comment promised 16 ms without a
matching input rate. Restart also clears the previous attempt's live readings
(`firmware/main/input/he_cal_runner.c`).

### Spec conformance and rule 6 (CAL#2/CAL#3)

Lock/sampling suppress both HE keys with `HE_PARAMS_NEVER`; start installs
both keys' suppression parameters, faults any held states, and enables
sampling in one existing `s_lock` section. Cancel/finish restore parameters
through the release-arming behavior of `he_rt_set_params`; they never call a
key-down API. The playing screen is gated separately (`firmware/main/input/he_backend.c`,
`he_rt.c`, `firmware/main/ui/ui_port.c`).
Parameter reloads now preserve an outstanding `need_release`, even when the
backend supplies a stale pre-fault rest reading. Only a subsequent sensor step
can satisfy it; the owner approved this safety fix (Q7, `he_rt.c`, `he_rt.h`).

First-use lock modes retain the capable-app/no-button choice, the approximately
5 s no-app grace period, download URL text and on-pad Calibrate button. A
stale button click cannot start the pad flow after a capable host arrives.
Failure/disconnect returns to the lock screen after the existing 2 s reason
display; a previously calibrated pad offers no on-pad recalibration. No
calibration timer is created on MX (`firmware/main/ui/he_cal_ui.c`,
`firmware/main/input/he_cal_runner.c`). Cancel/timeout/design choices remain
open in §8; no new cancellation gesture, timeout or redesign was added.

The original success path ignored `he_backend_finish_calibration`'s return,
and `he_cal_set` exposed new RAM values before persistence. Interactive saves
now use `he_cal_save`: commit the other A/B slot before publishing either new
key. An error leaves the old active calibration and old NVS slot intact. A
save attempted outside IDLE fails instead of claiming durable success. The
existing deferred `he_cal_set`/`he_cal_flush` API remains available and is
serialized with interactive saves (`firmware/main/input/he_cal.c`, `he_cal.h`,
`he_backend.c`, `firmware/main/ui/he_cal_ui.c`).

This is RAM publication/error handling, not a promise of reboot rollback
after every reported API error. In the installed IDF v5.5.2,
`NVSHandleSimple::set_blob` writes the record immediately and `nvs_commit`
is currently a no-op for a valid handle. A candidate already persisted before
a hypothetical later commit error can be selected on reboot. The final host
test models this explicitly; its original buffered-until-commit double was
corrected during the claims pass. SDK sources read: `components/nvs_flash/src/nvs_api.cpp`
and `components/nvs_flash/src/nvs_handle_simple.cpp`, relative to the installed
SDK path recorded in §6. The old slot is retained; no new active-slot wire or
storage format was invented to make a stronger claim.

The normal Alt swipe cleanup sends F24. Calling it when calibration became
active could therefore make a host start command cause a new keyboard press.
The calibration interruption branch now clears the touch keyboard state
directly. The approved touch interlock also refuses start during a contact or
its complete release cleanup. It reserves an idle touch before publishing
backend suppression; a contact racing after reservation latches its gate
even before that backend flag is visible. This closes the delayed touch
sequence race without a new per-poll wait (Q4, `touch_retry.c`,
`he_cal_touch_guard.h`, `firmware/main/ui/he_cal_ui.c`). It does not withdraw
previously submitted USB transfers or fence the HE publisher (Q8).

### Concurrency (CAL#3)

New/shared calibration state and its protection:

| State | Writers/readers | Protection or ownership |
|---|---|---|
| `s_calibrating`, ISR-owned `s_cal_write`, buffer contents | ISR, LVGL, backend control | Existing `s_lock`; flag readers also take it. One LVGL reader owns the detached buffer until the next swap. |
| `s_cal_read`, runner, results, timers, overlay objects, button tracker, touch eligibility, system error | LVGL timer, touch task, protocol task | Recursive LVGL lock for every public lifecycle/touch entry. Checks and state transitions occur under the same lock. Init happens before use. |
| `s_app_capable`, `s_host_disconnected`, `s_btn_active` | Protocol/touch/LVGL | Atomic flags; lifecycle and button transitions also use the LVGL lock. Pre-UI capability notification uses the atomic store alone. |
| `s_key_settings`, threshold reload/start/cancel/finish transactions | Config/protocol and LVGL | Task-only recursive `s_params_mutex`; engine/state copies still use `s_lock`. Neither ISR nor `he_task` takes this mutex. |
| `s_active_cal`, slot/sequence/dirty metadata | UI save, runtime flush, status readers | `s_cal_lock` for short snapshots/publication; `s_write_mutex` serializes complete NVS transactions outside the spinlock. |
| Pending protocol progress | UI/protocol producer, CDC consumer | `s_progress_lock`; latest update coalesces. CDC alone frames/transmits and advances `s_out_sequence`. Disconnect clears pending progress. |
| Touch gate, pending retry, held retry/Alt, gesture tracker, retained coordinates | Touch task | Task-local; the gate remains latched through physical release. |
| Touch/start reservation | Touch task and protocol/UI start | Atomic bitmask; contact owns BUSY through cleanup, start compare-exchanges idle to START, and finishing start preserves a concurrent BUSY bit. |
| Desktop progress and launch-once state | Runtime events/IPC | Shared `DaemonState` mutex and `apply_event`; controller owns launch-once state. GUI request epochs reject replies from earlier starts/cancels/connections. |

Sources: `firmware/main/input/he_backend.c`, `he_cal.c`, `touch_retry.c`,
`firmware/main/ui/he_cal_ui.c`, `firmware/main/protocol/protocol.c`,
`desktop/daemon/src/runtime.rs`, `ipc_handlers.rs`, `desktop/gui/src/main.rs`.

Existing backend state was also traced: `s_keys`, `s_down`, `s_edge_us`, frame
statistics and shared health readings use `s_lock`; ADC/retry/backoff control
belongs to `he_task`; the module probe uses its semaphore handshake. Boot
publishes `s_task`/`s_running` before runtime use. One existing exception is
`s_stats.isr_max_cycles`: its ISR-only writer is outside the reader's lock,
so the reader lock alone is not a C-level synchronization guarantee. It was
not changed in this calibration review because that changes the ISR beyond
the authorized sampling work. `s_touch_pressed` is also an existing volatile
observation flag, not a fence for delayed touch report sequences. Neither is
presented here as proof of race freedom (`firmware/main/input/he_backend.c`,
`he_health.c`, `touch_retry.c`).

NVS still runs synchronously on Core 1 from the LVGL callback, with the LVGL
lock held. It can pause drawing and delay commands acquiring that lock. It
is gated to IDLE and occurs while calibration keys are suppressed; no NVS
operation is added to the frame ISR or reporting task. Physical flash/cache
stall behavior was not measured. The transaction mutex does not wait for
LVGL, so runtime flush does not introduce an inverse UI-lock dependency
(`firmware/main/ui/he_cal_ui.c`, `firmware/main/input/he_cal.c`, `he_backend.c`,
`firmware/main/runtime/runtime.c`).

### Touch walkthrough (CAL#2/CAL#9)

| Sequence | Code path and outcome |
|---|---|
| Contact begins locked | Latch `cal_gated`; only feed calibration hit testing. Use last valid contact coordinates on release, because CST816 up reads have no position. |
| Calibration gate rises while Quick Retry is held | Clear retry/pending retry and touch keyboard state, stop gesture tracking, and suppress until physical release. This applies on up reads too. |
| Lock lifts during that contact | The per-contact gate stays set. Releasing it cannot generate the normal pending retry tap. A contact already down before a button appears cannot click it. |
| App start arrives during a swipe/contact/release cleanup | Refuse with FAILED/NONE and “Release the touchscreen first”; no automatic retry. A contact arriving after an accepted start reservation is gated until release. |

Sources: `firmware/main/input/touch_retry.c`, `he_cal_button.c`,
`firmware/main/ui/he_cal_ui.c`. Button excursion now includes a final movement
reported with release (`firmware/test/host/test_he_cal_button.c`). No real
touch controller or USB key report was exercised.

### Desktop, protocol and test hygiene (CAL#4 and claims audit)

- `cal_gui_launched` resets on disconnect and is set before the launch action,
  so repeated status cannot launch in a loop. A failed spawn logs a warning
  and is not retried on that connection. The daemon tries its sibling
  `opad-gui.exe` on Windows or `opad-gui` otherwise, then the `opad-gui`
  command fallback. Missing installations need the warning acted on; this
  session did not exercise Windows packaging (`desktop/daemon/src/main.rs`,
  `runtime.rs`, `desktop/daemon/tests/daemon_tests.rs`).
  If both executable locations fail, the daemon still advertises its
  calibration capability, so the pad retains app-only guidance until that
  CDC connection closes. Stopping the daemon permits the on-pad fallback;
  there is no automatic capability downgrade after spawn failure
  (`desktop/crates/opad-device/src/lib.rs`, `firmware/main/ui/he_cal_ui.c`).
- `--page calibrate` uses the shared page-token table and the single-instance
  show request. A Linux socket test reaches `Page::Calibration` in a running
  instance; the Windows pipe test case includes this token but was not run
  here (`desktop/gui/src/main.rs`, `single_instance.rs`).
- Start/cancel check connection, capability and ownership. Start additionally
  requires IDLE. A fast progress reply is no longer erased after enqueueing
  start; late disconnected/unsupported progress is ignored. Direct GUI page
  entry on an unsupported pad hides the controls. GUI polling/start/cancel
  completions carry epochs (`desktop/daemon/src/ipc_handlers.rs`, `runtime.rs`,
  `desktop/gui/src/main.rs`, `pages.rs`).
- No new “0 = keep current” setting exists. `raw_value = 0` means unavailable;
  valid frame samples exclude rails. It is the last consumed frame reading,
  not a peak or every ADC conversion. Non-key stages use key 1. Progress is
  periodically refreshed and can coalesce before transmission. All nine
  shared error values 0..8 have compile-time assertions; timeout 9 is reserved
  and unused (`protocol/osupad.proto`, `firmware/main/ui/he_cal_ui.c`,
  `firmware/main/input/he_frame.c`).
- Renamed the boolean-decision test from `test_no_key_during_lock_or_calibration`
  to `test_input_suppression_decision`: it tests a predicate, not emitted HID
  reports. The existing apply-to-saved test exercises a pure helper; the new
  storage test additionally runs real storage publication/error paths
  (`firmware/test/host/test_he_cal_runner.c`, `test_he_cal_storage.c`).

## 3. Compatibility

These are decoder/control-path checks and host tests, not tests on frozen old
firmware/app binaries or on a pad. No wire tag, enum value or NVS record layout
was changed by the review fixes (`protocol/osupad.proto`,
`firmware/main/input/he_cal_record.h`).

| Pair | Decode/capability path | Result and evidence |
|---|---|---|
| New app + old firmware | Missing/false HelloAck capability; optional old calibration status is discarded by the daemon. New Hello tag 3 is additive. | No calibration controls or auto-launch. `pre_calibration_firmware_status_does_not_launch_or_offer_calibration`; old/new Hello projection test in `desktop/crates/opad-protocol/src/lib.rs`; device handshake in `desktop/crates/opad-device/src/lib.rs`. |
| Old app + new firmware | Hello without tag 3 initializes capability false (`osupad_Hello_init_zero` and nanopb decode); protocol dispatch passes false to the UI. | No capable app: on-pad fallback after grace/disconnect; routine progress is capability-gated (an explicit calibration command can still receive a refusal). Existing framing is negotiated from the received frame. `firmware/main/protocol/protocol.c`, `osupad.pb.h`, `firmware/main/ui/he_cal_ui.c`; Hello default and lock-mode host tests. |
| New app + new firmware + MX | `he_input` describes the build; `he_calibration` follows the running HE backend, which is false on MX. Status calibration is absent. | No lock/calibration timer/page/commands. `protocol_send_hello_ack`, `protocol_send_status`, `he_cal_ui_init`; `calibrated_he_and_mx_pads_do_not_launch_gui_calibration` and IPC capability tests. |
| New app + new firmware + HE | Capability true; status includes lock/calibration flags; start/cancel/progress use additive variants and the host's framing. | One GUI launch per connection, guarded start/cancel, success requests fresh status. Firmware start is refused outside IDLE or while busy. Protocol round trips, daemon reconnect/IPC tests, runner start-refusal tests; `firmware/main/protocol/protocol.c`, `desktop/daemon/src/runtime.rs`. |

NVS A/B schema remains version 1; `he_cal_record.c/.h` were not changed. The
new app's serialization additions retain defaults for optional status fields
(`desktop/crates/opad-model/src/lib.rs`, `opad-ipc/src/lib.rs`). This session
did not certify arbitrary mixed versions of the GUI and daemon; the product
compatibility requirement here is app versus firmware.

## 4. Latency evidence

ESP-IDF v5.5.2 HE ELF, `xtensa-esp32s3-elf-nm -S`:

```text
40387814 000000fa T he_rt_step
40377114 00000040 T key_event_report
403772a8 000001a1 t on_frame
4037725c 0000004a t publish
3fca16bc 00001028 b s_cal_buffers
40377710 000000ac T usb_hid_handle_key_event
```

The five functions remain in IRAM; buffers occupy 0x1028 (4136) bytes of
internal BSS. `xtensa-esp32s3-elf-objdump -d --disassemble=on_frame` shows the
inactive calibration branch jumping over the inline accumulator:

```text
40377318: l32r a11, ... (s_calibrating)
4037731f: l8ui a11, a11, 0
4037732c: beqz a11, 40377385 <on_frame+0xdd>
```

The seven call targets are `esp_timer_get_time`, `he_frame_parse`,
`xPortEnterCriticalTimeout`, `he_rt_step`, `key_event_press_accepted`,
`vPortExitCritical`, `vTaskGenericNotifyGiveFromISR`, all at IRAM addresses
in the inspected disassembly. The accumulator adds no call, allocation,
logging or additional lock. Task transaction mutexes are absent from this
call path (`firmware/main/input/he_backend.c`, `he_cal_samples.h`; inspected
artifact `firmware/build-he/opad-firmware.elf`).

A compile-time assertion also ties the backend conversion rate/frame length
to the 4 kHz rate used by the calibration windows
(`firmware/main/input/he_backend.c`, `he_cal_samples.h`).

The owner-approved rearming change is confined to task-side
`he_rt_set_params`; `he_rt_step`'s before/after disassemblies compare equal,
including instruction bytes and addresses. Touch reservation runs only at
contact/start boundaries on Core 1, not in the frame ISR or ordinary switch
reporting path (`he_rt.c`, `touch_retry.c`, `he_cal_touch_guard.h`).

This establishes placement and control flow, not timing. ISR cost, jitter,
USB-visible latency, flash/cache interference and short-tap delivery were
**not measured on a pad**. No claim of identical latency follows from equal
symbol addresses. Touch polling and calibration-only task work also need
real-pad checks (`docs/testing-checklist.md`).

## 5. Tests and mutation evidence

`make check` passed after the changes: formatting, clippy with warnings denied,
workspace tests and all 21 numbered firmware host suites, plus version checks
(`Makefile`, `firmware/test/host/run_tests.sh`). Relevant Rust suite counts in
the final implementation check were 72 daemon integration tests, 20 GUI tests
and 15 protocol tests. The GUI state test constructs the app and drives its
real update handler without executing returned window/IPC tasks
(`desktop/gui/src/main.rs`).

The 4 kHz stream test uses rest near 2086, synthetic noise with approximately
5.6-count standard deviation, and 20–60 ms triangular presses with +450..510
count spans. Both the real accumulator/batch feed and the old 20 ms snapshot
control see the same stream. Recorded output:

```text
key 1: true median 2566, batched full 2566, snapshot full 0 (stage 5)
key 2: true median 2571, batched full 2571, snapshot full 0 (stage 5)
key 1: true median 2568, batched full 2568, snapshot full 2403 (stage 4)
key 2: true median 2569, batched full 2569, snapshot full 2408 (stage 4)
```

The true median is computed from all generated frame peaks. The test also
includes a fixed-phase 60 ms case where the old feed completes successfully
but saves substantially low full values, rather than merely failing peak
consistency. It also checks rest accuracy, an intervening moving-rest sample,
overflow and missing
samples (`firmware/test/host/test_he_cal_sampling.c`). Physical conversion
noise and actual presses were not sampled in this session.

The storage test links the real `he_cal.c` and record implementation against
explicit NVS/RTOS test doubles. It checks first-use and recalibration failures
at open/set/commit, reboot selection, IDLE gating, deferred flush and two
concurrent writers. The concurrent phase checks twenty completed saves with
consecutive A/B sequences and that reboot loads the last published values.
These doubles do not replace either ESP-IDF firmware build or prove physical
power-loss behavior (`firmware/test/host/test_he_cal_storage.c`, `cal_stubs/`).

Each mutation below was actually applied, the command returned nonzero, then
the edit was reverted and the diff inspected. Line numbers identify the
recorded mutation run, not a promise about future source line numbers.

| Mutation | Regression test | Recorded failing output |
|---|---|---|
| Feed only each batch's last sample | `test_frame_batches_preserve_real_peaks` | `test_he_cal_sampling.c:91: s.runner.stage == HE_RUNNER_SUCCESS`; exit 134 |
| Accept excursion equal to 30 px | `test_excursion_exceeded` | `test_he_cal_button.c:56: !he_cal_button_feed_touch(...false, 130, 190)`; exit 134 |
| Allow apply-to-saved before success | `test_failures_and_cancel_keep_old_calibration` | `test_he_cal_runner.c:264: !he_cal_runner_apply_to_saved(&runner, saved)`; exit 134 |
| Publish calibration on failed persistence | Storage `expect_active` | `test_he_cal_storage.c:71: got[k].rest == expected[k].rest`; exit 134 |
| Bypass HelloAck capability gate | `pre_calibration_firmware_status_does_not_launch_or_offer_calibration` | `daemon_tests.rs:4032: !actions.contains(&RuntimeAction::LaunchGuiCalibration)`; exit 101 |
| Decode every show target as logs | `calibration_launch_reaches_the_running_gui` | `single_instance.rs:398: left Some(Logs), right Some(Calibration)`; exit 101 |
| Encode Start as Cancel | `he_calibration_messages_round_trip_and_old_firmware_defaults_false` | `opad-protocol/src/lib.rs:475: left CancelHeCalibration(true), right StartHeCalibration(true)`; exit 101 |
| Accept old GUI poll epoch | `calibration_ignores_replies_from_before_start_or_cancel` | `gui/src/main.rs:3254: app.cal_progress.is_none()`; exit 101 |
| Reserve calibration even when touch is busy | Touch interlock contact-first case | `test_he_cal_touch_guard.c:12: !he_cal_touch_reserve_start(&g)`; exit 134 |
| Drop the outstanding release requirement on reload | `test_param_reload_preserves_release_requirement` | `test_he_rt.c:344: k.need_release`; exit 134 |

The original C mutations were caught through `firmware/test/host/run_tests.sh`;
the two owner-approved fixes also used direct GCC builds of their host tests.
Rust mutations used filtered `cargo test` commands. No mutation remains in the
branch. Detailed local command transcripts were kept in `/tmp/he-cal-*.log`;
the essential failures are transcribed above so this report does not depend
on those temporary files.

## 6. Three verification passes and claims audit

All passes covered the batch from `0fbcb6d` and the follow-up edits; each
looked at code as well as the existing commit messages. Tests were rerun as
changes warranted; their passing did not replace the behavioral review.

1. **Pass A — builds and tests.** Baseline `make check`, protobuf generation,
   MX/HE builds passed, confirming that the sampling and persistence defects
   were not detected by those checks. After fixes the checks passed again;
   the new stream/storage tests exercise those defects. Builds used the
   installed v5.5.2 environment and `firmware/build` / `firmware/build-he`.
   An initial `idf.py -C firmware -B build-he` invocation resolved its output
   directory at the repository root; that generated directory was moved to
   `/tmp/opad-calibration-build-RC2NKD/build-he`, and the required HE build was
   rerun from `firmware/`. No such output was committed.
2. **Pass B — behavioral review.** Traced lock/start/cancel/finish, NVS errors,
   ISR/task/UI sharing, the four touch sequences, old/new capabilities,
   desktop launch and single-instance forwarding. This found the released
   coordinate loss, final-excursion gap, F24 interruption, lifecycle checks
   outside the UI lock, cross-task CDC framing/TX access, premature save
   success, stale progress and inconsistent desktop guards. Fixes and their
   limits are in §2. The final follow-up closes the delayed touch sequence
   after owner approval (Q4); the HE publisher cutoff remains Q8.
3. **Pass C — claims audit.** Compared identifiers and numeric claims with
   capture/runner/backend/proto/IPC/UI source and command output. Corrected
   1 kHz/64 ms claims, the obsolete “uncalibrated pad types” comment,
   `GetCalibrationProgress` (the actual request is `GetHeCalibrationProgress`),
   a stale “entry point not built” status, the false “engine not called”
   statement, nonexistent timeout checklist coverage and the meaning of peak
   spread. Corrected disconnect/cancel checklist expectations, documented
   stable partial-rest limits, narrowed the decision-test name and pointed
   OPEN comments to this report's real §8. Checking the installed NVS source
   also corrected the test double's commit semantics and removed any implied
   across-reboot rollback promise after a late API error. A host diagnostic
   reproduced the stale-rest reload case in Q7; the owner approved preserving
   release arming, and its regression test plus mutation now cover that fix.

Historical commit claims need qualification: `7fee807`'s “Fully tested” is
host state-machine coverage, not pad/UI/USB verification. `a2364db`'s
“atomic saved-calibration updates” describes a success-only pure helper,
not a storage transaction or concurrency proof; the storage path was fixed
in CAL#3. Old nm addresses and equal-address claims are historical snapshots,
not latency measurements; §4 contains the newly inspected final artifact.
Existing commit messages were preserved as requested. Plan/checklist prose
that implied guaranteed rejection of every held key, generic app-disconnect
cancellation or identical MX performance has been corrected or qualified
against the cited code.

Protobuf was regenerated with the IDF environment, never hand-edited; repeat
generation produced no additional diff (`scripts/gen_proto.sh`). REUSE
returned exit 1 for exactly the pre-existing missing licensing information
on `docs/release-readiness-guide-2026-09-22.md`; no new finding was reported
(`scripts/check_reuse.py`). That existing problem was left outside this task.

The installed SDK reports `ESP-IDF v5.5.2-dirty`; its HEAD is exactly tag
`v5.5.2`. Read-only inspection found broad file-mode changes, example script
type changes and a dirty NimBLE submodule, with no added/deleted text lines
in the top-level SDK diff. The SDK was not modified or cleaned by this work.
These builds used that installed environment, not a newly checked-out clean
SDK (`idf.py --version`, SDK `git describe --tags --exact-match HEAD` and
`git diff`).

## 7. Not done / not tested

- No hardware was flashed, connected or exercised. Run HE-CAL-01..HE-CAL-11
  in `docs/testing-checklist.md` on Linux and Windows, including the approved Q4 policy;
  the checklist retains NOT RUN. In particular verify fallback/button
  coordinates, quick 20–60 ms presses, no report leakage or stuck touch key,
  disconnect/reconnect, MX operation, held-key release arming and unlock
  without reboot (`firmware/main/input/touch_retry.c`, `he_backend.c`,
  `firmware/main/ui/he_cal_ui.c`).
- Actual display layout/text, Windows builds/packaging/pipes, physical NVS
  failures/power interruption, and mixed-version binaries were not tested.
  Neither host test doubles nor the Linux socket test establish those results
  (`firmware/test/host/cal_stubs/`, `desktop/gui/src/single_instance.rs`).
- Repeat firmware compilation in the clean pinned CI image before release;
  the local SDK checkout caveat is recorded in §6.
- The finite 128 ms buffer must be stressed against real LVGL/display load;
  the safe response to an overrun is failure, not a silently lower peak.
  Sensor rest bounds and movement defaults remain provisional; no threshold,
  pinned dependency or protocol policy was changed to manufacture passing
  hardware claims (`firmware/main/input/he_cal_samples.h`, `he_cal_capture.c`,
  `he_backend.c`).
- The pre-existing ISR maximum-statistic synchronization caveat in §2 needs
  separate key-path authorization/review. The calibration sampling change
  did not alter its placement or treat its value as a measurement from a pad
  (`firmware/main/input/he_backend.c`).

## 8. Questions for the owner

### Q1. Cancel the on-pad flow

Options: retain no on-pad cancellation; add a visible Cancel button; or define
a long-hold gesture. Recommendation: keep the current behavior until a visible
button is approved. A gesture needs a separate conflict review with touch
semantics. Current code only supports host cancellation and app-started
CDC-disconnect cancellation (`firmware/main/ui/he_cal_ui.c`).

### Q2. Walk-away timeout

Options: none (current); 60 s without a captured press; or 120 s without a
captured press. Recommendation: 60 s of inactivity, returning to the lock
screen and preserving old calibration, once approved. Nothing currently
sends `HE_CAL_ERROR_TIMEOUT`; no timer was invented here
(`protocol/osupad.proto`, `firmware/main/ui/he_cal_ui.c`).

### Q3. Wording and design

Options: approve the current provisional screen/error text; approve revised
copy and layout before implementation; or supply a specific visual reference.
Recommendation: review the actual 320×240 layout and friendly error text on
hardware before approving revised copy. Existing wording/design was retained
except where an error would falsely claim success or offer forbidden on-pad
recalibration (`firmware/main/ui/he_cal_ui.c`,
`firmware/main/input/he_cal_runner.c`, `desktop/gui/src/pages.rs`).

### Q4. Calibration start during an in-flight touch sequence — decision implemented

`tap_keyboard_key`, `step_volume` and `release_alt` can wait/delay between
reports. Calibration starts from a different task. A once-per-poll gate cannot
withdraw a sequence already past its check; even the ordinary release cleanup
can still be running after `s_touch_pressed` becomes false
(`firmware/main/input/touch_retry.c`, `firmware/main/ui/he_cal_ui.c`).

The owner selected: **refuse app calibration until the current touch is fully
released**, the smallest-change policy. Implemented as an atomic contact/start
interlock, with contact ownership extending through tap/release/Alt cleanup.
No delayed touch sequence can continue submitting new reports after an
accepted start. If start reserves first, a racing contact latches suppression.
The on-pad button ends contact ownership before its release callback starts
calibration (`he_cal_touch_guard.h`, `touch_retry.c`, `he_cal_ui.c`).

The host interleaving test covers the interlock, not real HID scheduling or
controller behavior. A report already submitted before acceptance may still
reach USB afterward; no USB-transfer drain was added. The distinct HE
publisher race is Q8 (`firmware/test/host/test_he_cal_touch_guard.c`,
`firmware/main/usb/usb_hid.c`).

### Q5. A steady partial press during rest

The algorithm can reject motion and a mean outside 1500..2500; it cannot know
that an in-range steady value is a partially held key. `HE_CAL_ERR_REST_PRESSED`
has a name/mapping but is not emitted by capture. No automatic invalidation or
new rest heuristic was introduced (`firmware/main/input/he_cal_capture.c`).

Options: retain the algorithm and explicitly require fingers off; measure
hardware to justify narrower rest bounds; or design a separate release
confirmation step. Recommendation: retain the algorithm, document the limit,
and measure hardware before changing bounds or UX. The old HE-CAL-09 promise
that any held key fails rest was not supported by code
(`docs/testing-checklist.md`).

### Q6. Confirm the governing touch exception before integration

The worktree's `AGENTS.md` §1.4 explicitly excepts an uncalibrated HE pad from
Quick Retry/swipes, citing D7. The AGENTS text supplied with the initiating
session omitted that sentence. Existing lock behavior was retained and neither
rulebook was edited. Options: confirm the worktree's HE exception as intended;
or reject that exception and revisit D7 before integration. Recommendation:
confirm the existing D7 exception; changing it would conflict with the
requested first-use lock (`AGENTS.md`, `docs/specs/v2-hall-effect-decisions.md`).

### Q7. Stale parameter-reload reading — safety fix approved and implemented

Before CAL#8, `he_backend_set_params` passed the last nonnegative `s_stats.raw`
to `he_rt_set_params`, without checking whether it predated a fault. A reload
could therefore clear `need_release` using a pre-fault rest sample. When valid
frames resumed with the key held, the engine could press without a post-fault
release. The new cancel/finish paths also call this existing reload function
(`firmware/main/input/he_backend.c`, `he_rt.c`, `he_health.c`).

A host-only diagnostic linked the real `he_rt.c`, using actuation 2300,
release 2200 and a remembered rest of 2086: initialize, observe rest, fault,
reload parameters with that stale 2086, then feed a held sample of 2400.
Actual output was:

```text
after fault + reload using stale rest: need_release=0
first post-fault sample held: event=1 (DOWN=1)
```

Passing `INT32_MAX` instead kept the held sample disarmed. This demonstrates
the engine's reaction to the stale argument; it is not an ADC/HID hardware
test. The diagnostic source/output was kept in `/tmp/he-cal-stale-raw-repro.c`
and `/tmp/he-cal-stale-raw-repro`.

The owner authorized the guard and permanent regression test. The fix
preserves an outstanding engine `need_release` across parameter reloads,
regardless of the supplied remembered reading or health timestamp resync.
Only a subsequent `he_rt_step` observation can satisfy it. Tests cover boot,
faulted up/down states, repeated stale reloads and all three plain/rapid
trigger modes. Ordinary stepping remains byte-for-byte unchanged in the HE
artifact; hardware release behavior remains NOT RUN (`he_rt.c`, `he_rt.h`,
`firmware/test/host/test_he_rt.c`, `docs/specs/v2-hall-effect-decisions.md` D4).

### Q8. HE reporting cutoff at calibration start — awaiting direction

The HE publisher snapshots `s_down` under `s_lock` and calls
`key_event_report` after releasing it. A start can cross that read/report
interval, so the threshold lock alone is not a reporting barrier. An
already-submitted USB report also cannot be retroactively withdrawn
(`firmware/main/input/he_backend.c`, `firmware/main/usb/usb_hid.c`).

Options: authorize a key-task start barrier, allowing previously submitted
USB transfers to finish; or defer it to a separate key-path review and
withhold strict D7 output-isolation sign-off. Recommendation: add the
task-side barrier, with new tests and latency review. The owner was asked
separately because the touch refusal and stale-sample approvals do not
authorize this additional reporting-path change (AGENTS.md §8).
