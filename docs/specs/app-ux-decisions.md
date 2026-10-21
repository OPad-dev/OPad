# Desktop app UX: decision record

**Decided:** 2026-10-22, from the UX review of 2026-10-21 (branch
`feat/he-night`). Item numbers match that review. Status: not implemented yet.

## Goal and principles

An osu! player with no computer experience can install the app, connect the pad
and configure everything without reading a manual.

Every change follows these rules:

- **Plain words first.** Technical detail lives behind a Details control, a
  tooltip or the Device page, never on the main path.
- **Every state and error says what happened and what to do.** No internal
  values, enum names or debug output in the interface; the raw detail goes to
  Logs.
- **One way to save, everywhere.** The same save bar and the same warning before
  leaving unsaved changes on every page that edits settings.
- **Notifications behave like a phone's.** They slide in, say one thing, and go
  away on their own.
- **Scope:** the desktop app and the words on the pad's screen. User manuals
  live on the website's documentation; the README stays developer-facing.

## Critical

### 1. First run and "no pad" state

A first-time user gets a short welcome flow, and a missing pad is explained
instead of shown as zeros.

- **Welcome flow,** shown until a pad has connected once:
    1. Plug in your pad. The step watches for it and moves on by itself when it
       appears.
    2. Test your keys. Each key lights up when pressed.
    3. Pick your keys (Z and X by default).
    4. HE pads only: calibrate, using the existing calibration page.
- **No pad connected later:** the Dashboard shows "No pad found. Plug it in with
  its USB cable." in place of the counters.
- Where: `dashboard()` in `desktop/gui/src/pages.rs`; first-run state kept by
  the GUI.

### 2. Choose a key by pressing it, letters only

The Key 1 and Key 2 bindings are set by pressing the key, and only letters A to
Z are accepted.

- A "Press a key" button listens for the next key press and shows the chosen
  letter large.
- Any other key is refused with "Pick a letter key (A to Z)."
- Replaces the one-character text box (`desktop/gui/src/pages.rs:272`).
  `char_to_hid_usage` in `desktop/crates/opad-model/src/lib.rs` also accepts
  digits and hex codes today; the GUI no longer offers them.

### 4. Pairing prompts in plain words

The pairing and pad-replacement prompts ask one question with one recommended
answer.

- **Another computer:** "This pad was set up on another computer. Use it here?"
  Primary button: **Use it here** (keeps the pad's counts). Secondary:
  **Not now** (the pad still types; the app leaves it alone).
- **More options** reveals "Use this computer's counts instead".
- **New pad replacing an old one:** "This looks like a new pad. Bring over the
  counts from your old pad?" Yes / Start from zero.
- No file paths in the interface: `docs/recovery.md`
  (`desktop/gui/src/main.rs:2329`) becomes a link to the website's help page.

## High

### 5. No debug text or raw errors

States show as words and every error says what to do; raw detail moves to Logs.

- Dashboard mode label (`desktop/gui/src/pages.rs:243`, today the enum name in
  capitals): Idle → **Ready**, Playing → **In a map**, Cooldown → **Map just
  ended**, Sync → **Syncing** (`RuntimeMode` in
  `desktop/crates/opad-model/src/lib.rs`).
- Errors: `Unexpected response: {…}` and `Error: …`
  (`desktop/gui/src/main.rs:1104`, `1530`–`1595`, `1855`) become one sentence of
  what happened and one of what to do, e.g. "The pad didn't answer. Check the
  USB cable and try again." The raw text is written to Logs.
- The Device page is not part of this change (see Kept as is).

### 7. Plain names for technical terms

On main screens the technical term is replaced; it may stay in a tooltip.

| Today | Becomes | Where |
| --- | --- | --- |
| Debounce lockout | Switch bounce filter, moved under Advanced | Keys page |
| Gameplay refresh rate | Screen updates during a map | Settings, Display |
| GPIO14 (P2 pin 11) | Header P2, pin 11 (GPIO in the tooltip) | Keys page, wiring |
| Key latency p50 / p99 / p99.9 / max in µs | "Key speed: 0.03 ms" (typical), percentiles behind Details | Dashboard |
| tosu integration, Binary source | osu! connection (tosu) | Settings, About |
| Running slot, "flash the app partition" | Hidden in the firmware card; "The pad stops typing for about 30 seconds while it updates." | Settings, Updates |

### 8. One Keys page

Everything about the keys lives on the sidebar's **Keys** page, in four
sections: Bindings · Key feel (HE pads: actuation and rapid trigger) · Wiring
(pins, Auto-Detect) · Advanced (switch bounce filter, noise-floor option). The
Settings tab "Keypad" keeps only the touchscreen and is renamed **Touchscreen**.

### 9. One save model

Every page that edits settings works like the Keys page today
(`desktop/gui/src/keys.rs`).

- A bar pinned at the bottom: "Unsaved changes · Revert · Save to pad", greyed
  out when nothing changed.
- Leaving the page with unsaved changes asks, inside the app: **Save** /
  **Discard** / **Stay**. It is a panel drawn by the app, the same on Linux and
  Windows.
- Instant actions stay instant: Play an animation, Sync, Auto-Detect,
  Calibrate.

### 10. Notifications like a phone

Results of actions appear as toasts and go away on their own; banners stay only
for states that need the user.

- Toast in the top-right corner, sliding in. Success stays 4 s, warning or
  error 8 s. Hovering pauses the timer, and a close button dismisses it. At most
  3 stacked, newest on top.
- Persistent banners only for: daemon offline, device permissions missing,
  pairing question, restart after an update.
- Replaces today's banner that stays until Dismiss (`DismissBanner` in
  `desktop/gui/src/main.rs`).

## Medium

### 11. GNOME without a tray

When the desktop has no tray, OPad starts only in the background at login and
never opens a window by itself.

- Today a `--tray` start with no tray falls back to a window at every login,
  with "No system tray found; the app will quit when closed."
  (`NO_TRAY_BANNER`, `desktop/gui/src/main.rs:310`).
- New: the login start runs only the background service (counting, the pad's
  screen, tosu). The setting reads "Start OPad in the background at login".
- One line in Settings: "No tray icon on GNOME? Install the AppIndicator
  extension", linked to the website's help page.

### 12. Short help text

Each control gets at most one sentence of help; exceptions go behind a "Learn
more" link to the website. First targets: the touchscreen swipes paragraph,
Result animations, and the Diagnostics card in `desktop/gui/src/pages.rs`.

### 13. Readable text size

Text people read is at least 13 px; small labels at least 12 px. Today much of
the help text is `.size(11)` or `.size(12)` in grey.

### 14. Consistent wording

- Sentence case for every button, heading, tab and label ("Restore pad from
  PC", not "Restore PAD from PC"; "Recalibrate", not "Recalibrate Again").
- No emoji in buttons or tabs (today "⌨ Keypad", "💾 Save Diagnostic File").
- One name per thing: "pad", "Key 1" and "Key 2", "osu!".

### 15. Links you can click

Release notes, the tosu repository and website help open in the browser. Today
they are plain text ("What's new: https://…", `desktop/gui/src/pages.rs:741`).

## Low

### 19a. PPM becomes KPM

The tap rate is called **KPM (keys per minute)** everywhere a person reads it.

- App: Stats page (`desktop/gui/src/stats.rs`, "TAP RATE · PPM") and the
  Designer's data names (`desktop/gui/src/designer/mod.rs`).
- Command line: `opadctl stats` output (`desktop/cli/src/main.rs`).
- The pad's screen: the default layout's "AVG PPM" label
  (`firmware/main/ui/core/ui_defaults.c:123`).
- Internal names in code and protocol can stay; only text people see changes.

### 19b. tosu explained in About

The About section gets a short paragraph: "tosu is a free program that reads
what osu! is showing (the map, your accuracy, pp) so OPad can show it on the
pad's screen. OPad installs it and starts it for you." The licence and
repository lines stay below it.

### 20. Accessibility basics

- Every control can be reached with Tab and shows where the focus is.
- Colour is never the only signal: the yellow counter mismatch also gets a word
  or icon.
- Text keeps a contrast of at least 4.5:1 against its background.

## Kept as is

| Review item | Decision |
| --- | --- |
| 6. Device page | Stays technical. It is the place for device ID, firmware slot, counter generation and the PC versus pad counter table. |
| 16. Minimum window 1100 × 700 | Stays. Buyers are not expected to use 1366 × 768 screens. |
| 17. Developer-oriented README | Stays. Manuals for players live in the website's documentation. |

## Needs development (not in this pass)

Listed so they are not forgotten; they need their own work, outside these
design changes.

- [ ] **3. Update feed.** The daemon checks
  `https://opad-dev.github.io/OPad/updates/stable.json`
  (`desktop/crates/opad-update/src/client.rs:21`), which returned 404 on
  2026-10-21, so every user sees "Last check failed". The release pipeline has
  to publish the feed.
- [ ] **18. Windows code signing.** The installer is unsigned, so Windows shows
  "Windows protected your PC". Planned through SignPath Foundation once the
  repository is public (README, Windows section).

## Order and checks

The work goes from low risk to structural, so each step can ship on its own.

1. **Wording pass** (5, 7, 12, 14, 15, 19a, 19b). Done when no screen shows an
   enum name, raw error, emoji button or ALL-CAPS label, and every link opens
   the browser.
2. **Notifications** (10). Done when a save, a sync and a failed action each
   show a toast that leaves on its own, and the four persistent banners still
   stay.
3. **One save model** (9). Done when every settings page has the pinned bar, and
   leaving with unsaved changes always asks Save, Discard or Stay.
4. **One Keys page and key capture** (8, 2). Done when bindings, key feel,
   wiring and advanced live on Keys, the Settings tab is called Touchscreen, and
   only A to Z can be bound.
5. **First run and pairing** (1, 4). Done when a fresh install shows the welcome
   flow, a missing pad shows "No pad found", and both pairing prompts have one
   recommended button.
6. **Platform and polish** (11, 13, 20). Done when GNOME without a tray starts
   only in the background, no read text is under 13 px, and every control is
   reachable with Tab.

For every step: `make check` passes, the Linux and Windows builds pass in CI,
AGENTS.md rules hold (nothing on the keypress path changes), and each changed
screen is looked at on a real desktop before the step is called done.
