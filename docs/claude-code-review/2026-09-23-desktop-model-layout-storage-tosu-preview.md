# Code review: `desktop/crates/opad-model`, `opad-layout`, `opad-storage`, `opad-tosu`, `opad-ui-preview`

- Date: 2026-09-23
- Command: `/code-review high desktop/crates/opad-model desktop/crates/opad-layout desktop/crates/opad-storage desktop/crates/opad-tosu desktop/crates/opad-ui-preview`
- Base: `main` @ `b0e0a37`
- Scope: no diff touched these crates, so the review covers their current code. `opad-layout` and `opad-storage` produced no findings (`KEY_PINS` matches the schematic pinout exactly).
- Verification: finding 3 was verified with rustc; the rest were not re-verified.
- Status legend: `open` / `fixed` / `wontfix` / `invalid`

Findings are ranked most severe first.

---

## 1. `TosuSupervisor::pause()` never observes the child exit — `fixed`

**File:** `desktop/crates/opad-tosu/src/lib.rs:435`
**Category:** correctness

`pause()` only sleeps 500 ms; it never observes the child exit, so the doc-comment guarantee "returns once the child is actually gone" is false and the updater swaps the binary while tosu may still be running.

**Failure scenario:** `updater.rs:473` calls `supervisor.pause().await` then `tosu::install()`. The supervisor drops the `Child` (`kill_on_drop` sends SIGKILL/TerminateProcess without waiting), or is mid-`launch_tosu()` having already passed the paused check. On Windows the image file is still locked → install fails; on Linux the old inode keeps running and the update is hidden until restart.

**Suggested fix:** Replace `drop(child)` with `child.kill().await` and signal completion via a `tokio::sync::Notify`/`watch` that `pause()` awaits, instead of a fixed sleep.

## 2. `JsonBackup::validate` accepts HID usages the firmware rejects — `fixed`

**File:** `desktop/crates/opad-model/src/lib.rs:451`
**Category:** correctness

`JsonBackup::validate` accepts any hex key mapping via `char_to_hid_usage` (`"0x00"`, `"0xFFFF"`), while `DeviceConfig::validate` and the firmware only accept `0x04..=0xE7`.

**Failure scenario:** A backup with `config.key1 = "0x00"` passes `validate()`; `ipc_handlers.rs:897-923` builds `DeviceConfig { key1_hid_usage: 0 }` and calls `save_config()` with no `DeviceConfig::validate()`, so an invalid usage is persisted and pushed to the pad, which answers `CONFIG_REJECTED`.

**Suggested fix:** Have `JsonBackup::validate` construct the `DeviceConfig` and delegate to `DeviceConfig::validate()` (which also removes the duplicated debounce/brightness/sleep/hz range checks at 428-450).

## 3. Log columns never align: `Display` impls use `write!`, which ignores width — `fixed` (verified)

**File:** `desktop/crates/opad-model/src/log.rs:75`
**Category:** correctness

`format_line` pads with `{:<7}`/`{:<5}` but the `LogSource`/`LogLevel` `Display` impls use `write!()`, which ignores width, so log columns never align.

**Failure scenario:** Verified with rustc: `format!("[{:<7}]", X)` where `Display` does `write!(f, "TOSU")` prints `[TOSU]` not `[TOSU   ]`. Every consumer (`cli main.rs:475`, `gui pages.rs:1237/1250`, `gui main.rs:2618`) shows ragged columns.

**Suggested fix:** Use `f.pad("TOSU")` in the `Display` impls.

## 4. A `pause()` kill is treated as a crash and doubles the relaunch backoff — `fixed`

**File:** `desktop/crates/opad-tosu/src/lib.rs:541`
**Category:** correctness

After a `pause()` kill, the supervisor treats the stop as a crash and doubles the relaunch backoff (up to 60 s), so tosu stays down long after `resume()`.

**Failure scenario:** Daemon starts, launches tosu, updater pauses within 60 s → `drop(child)`, `break` → backoff = 10 s (then 20, 40, 60 on later updates within the hour) → `sleep(backoff)` before the loop re-checks `paused`. Telemetry and the pad's tosu status dot stay dark for that long after every update.

**Suggested fix:** Skip the backoff computation when the break was caused by `paused`, or reset backoff on `resume`.

## 5. Missing `$OPAD_TOSU_PATH` target gives no fallback and a misleading warning — `fixed`

**File:** `desktop/crates/opad-tosu/src/lib.rs:268`
**Category:** correctness

An explicit `$OPAD_TOSU_PATH` pointing at a missing file makes `find_tosu_binary` return `None` with no fallback, and the supervisor then logs "no $OPAD_TOSU_PATH", which is the opposite of the situation.

**Failure scenario:** User sets `OPAD_TOSU_PATH=/opt/tosu/tosu`, later uninstalls it. Supervisor warns "tosu binary not found: no $OPAD_TOSU_PATH, none on $PATH, and no bundled copy" even though the var is set and a bundled copy exists; `ptrace_access()` also reports Blocked with the scope fix rather than the real cause.

**Suggested fix:** Either fall through to the other candidates or emit a warning naming the bad override path.

## 6. Stub-mode `build.rs` does not rerun on `ui_ids.h` changes — `fixed`

**File:** `desktop/crates/opad-ui-preview/build.rs:54`
**Category:** correctness

Stub mode emits `rerun-if-changed` only for `managed_components` and `preview_stub.c`, not for `firmware/main/ui/core`, so edits to `ui_ids.h` do not rebuild the stub.

**Failure scenario:** On a checkout without LVGL (release CI / clean dev box), add a source to `ui_ids.h` and to `opad_model::ui_source`; `cargo test -p opad-ui-preview` reuses the stale stub object, `source_ids_match_firmware` compares against the old table and fails (or passes when it should fail if only `ui_ids.h` changed).

**Suggested fix:** Move the `ui_core` `rerun-if-changed` (line 97) above the stub early-return.

## 7. `strip_ansi` ends escape sequences at the first ASCII letter — `fixed`

**File:** `desktop/crates/opad-tosu/src/lib.rs:460`
**Category:** correctness

`strip_ansi` ends an escape sequence at the first ASCII letter, which is wrong for OSC (`ESC ] ... BEL`) and other non-CSI sequences, leaking the tail of the sequence into log lines.

**Failure scenario:** tosu (Node, chalk/ora) prints `ESC ] 0 ; title BEL`: the parser exits escape mode at `t`, so the callback receives `itle\x07` prepended to the real line.

**Suggested fix:** Track `[` after ESC and consume until a final byte `0x40..0x7E` for CSI; for `]` consume until BEL or `ESC \`.

## 8. `opad-ui-preview` re-declares wire constants and hardcodes source ids — `open`

**File:** `desktop/crates/opad-ui-preview/src/lib.rs:8`
**Category:** reuse

`SCREEN_W`/`SCREEN_H`/`MAX_WIDGETS`/`LABEL_MAX`/`SUFFIX_MAX` re-declare constants `opad_layout` already exports, and `sample_values()` hardcodes raw source ids instead of `opad_model::ui_source` names.

**Failure scenario:** Two independent copies of the wire constants (`opad_layout::SCREEN_W = 320 i16`, `LABEL_MAX_BYTES = 31` vs `LABEL_MAX = 32` here) must be kept in step by hand; `sample_values()` entries like `(43, ...)` / `(41, ...)` silently map to the wrong widget if an id is ever renumbered, because `opad-model` is only a dev-dependency here.

**Suggested fix:** Depend on `opad-model` normally and use `ui_source::PROFILE_PP` etc., and derive the sizes from `opad_layout`.

## 9. `install_lib_dir()` duplicates `install_prefix()` — `fixed`

**File:** `desktop/crates/opad-model/src/paths.rs:97`
**Category:** reuse

`install_lib_dir()` duplicates `install_prefix()` (`current_exe → parent → parent`, Windows special case) instead of composing it.

**Failure scenario:** Two copies of the prefix-resolution rule; a future change (e.g. handling `/usr/local/bin` or an AppImage mount) must be made twice or the two paths silently diverge, breaking `the_install_lib_dir_sits_under_the_prefix`.

**Suggested fix:** `let prefix = install_prefix()?; Ok(if cfg!(windows) { prefix } else { prefix.join("lib").join("opad") })`.

## 10. `launch_tosu` spawns two byte-identical reader tasks — `open`

**File:** `desktop/crates/opad-tosu/src/lib.rs:620`
**Category:** simplification

`launch_tosu` spawns two byte-identical reader tasks for stdout and stderr (copy-paste), each capturing the `Arc<Mutex<Option<File>>>` and callback.

**Failure scenario:** Any fix to the line handling (e.g. writing the ANSI-stripped `clean` to the file, or the `strip_ansi` fix above) has to be applied in two places.

**Suggested fix:** A single generic `fn pump(reader: impl AsyncRead, cb, file)` called twice.
