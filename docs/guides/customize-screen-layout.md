---
title: Customizing the Screen Layout
description: How to design, preview, apply, export, and import custom Idle and Playing screen layouts in opad-gui, with every available data source.
sidebar:
  order: 3
---

The pad's 320×240 screen has two layouts: the **Idle screen**, shown outside a map, and the **Playing screen**, shown during one. You edit both on the app's **Designer** page, whose preview (drawn at twice the size) is rendered by the firmware's own UI code: what you see is what the pad shows. <!-- src: /desktop/gui/src/designer/mod.rs, /firmware/main/ui/core/ui_core.h, /firmware/main/ui/ui_port.c#pad_timer_cb -->

## Quick path

You need `opad-gui` and `opad-daemon` running. The pad can be unplugged: **Apply to pad** saves the layout in the daemon and sends it the next time the pad connects. If the app says the pad belongs to another OPad install, settle that first (see [Recovery §7](../recovery.md#7-reflashing-and-unbinding-a-pad)). <!-- src: /desktop/gui/src/designer/mod.rs#update, /desktop/daemon/src/ipc_handlers.rs#IpcRequest::SetLayout -->

1. Open `opad-gui` and select **Designer** in the left navigation bar (or launch `opad-gui --page designer`). <!-- src: /desktop/gui/src/main.rs#Page::GROUPS, /desktop/gui/src/main.rs#Target::TOKENS -->
2. At the top of the preview, pick **Idle screen** or **Playing screen**. Each is saved on its own. <!-- src: /desktop/gui/src/designer/mod.rs#view, /desktop/crates/opad-layout/src/lib.rs#Screen -->
3. Click a type under **Add widget**, drag it into place on the preview, and set its data source, text and colors under **Properties** ([Widgets](#widgets), [Properties](#properties)). <!-- src: /desktop/gui/src/designer/mod.rs#palette, /desktop/gui/src/designer/mod.rs#properties -->
4. Check it with the preview checkboxes: **Live data from osu!** shows the real `tosu` and tap-rate values `opad-daemon` holds, as they change (with `tosu` running; otherwise built-in sample values are shown), and **Show keys pressed** simulates both keys held so you can check Key card highlight colors. <!-- src: /desktop/gui/src/main.rs#watches, /desktop/gui/src/designer/mod.rs#Message::LiveData, /desktop/gui/src/designer/mod.rs#Message::SimulatePress -->
5. Click **Apply to pad** (shown as **Apply to pad •** while there are unapplied edits). To keep a copy as a file, click **Export** ([Export and import a layout](#export-and-import-a-layout)). <!-- src: /desktop/gui/src/designer/mod.rs#view -->

The status line below the preview then says `Applied to the pad` and the `•` disappears. The pad shows your Idle layout at once and switches to your Playing layout when a map starts. The layouts survive a replug: the pad keeps them in NVS (`opad_ui`). <!-- src: /desktop/gui/src/designer/mod.rs#Message::Applied, /firmware/main/ui/ui_port.c#pad_timer_cb, /firmware/main/ui/ui_port.c#ui_init, /firmware/main/ui/ui_store.c -->

## Widgets

A screen holds up to `32` widgets. <!-- src: /desktop/gui/src/designer/mod.rs#palette, /desktop/crates/opad-layout/src/lib.rs#MAX_WIDGETS -->

| Widget | What it draws |
|---|---|
| **Text** | `Label` + bound value + `Suffix`. With `Data` set to `None (static text)`, static text. A tap rate can be shown in `KPM (keys per minute)` or `KPS (keys per second)`; KPS has one decimal by default. <!-- src: /desktop/gui/src/designer/mod.rs#kind_help, /desktop/crates/opad-layout/src/lib.rs#WidgetKind --> |
| **Progress bar** | Horizontal bar: `Background` is the track, `Accent` the fill. Bind it to a `0..1` source such as `map.progress` or `play.health`. <!-- src: /desktop/gui/src/designer/mod.rs#kind_help, /firmware/main/ui/core/ui_screen.c#build_progress --> |
| **Key card** | Counter card for K1 or K2 (`pad.k1_map`, `pad.k1_lifetime`, `pad.k1_label`, or the K2 equivalents). `Label` is the title; `Background` and `Accent` swap while the key is held. **Show tap rate (key's live KPM or KPS)** adds the key's live rate in the corner. <!-- src: /desktop/gui/src/designer/mod.rs#kind_help, /firmware/main/ui/core/ui_screen.c#build_keycard --> |
| **Status dot** | Circle (`Accent` when active, `Background` when inactive) followed by `Label` in `Text / fg`. Bind it to a `0/1` source such as `status.pc`, `status.tosu` or `status.osu`. <!-- src: /desktop/gui/src/designer/mod.rs#kind_help, /firmware/main/ui/core/ui_screen.c#build_status_dot --> |
| **Rectangle** | Filled rounded rectangle (`Background` with `Radius`), with an optional 1 px **Border** in `Text / fg`. <!-- src: /desktop/gui/src/designer/mod.rs#kind_help, /firmware/main/ui/core/ui_screen.c#build_rect --> |
| **Grade** | The rank letter (`play.grade`) in osu!'s grade colors (`SS`/`X`, `S`, `A`, `B`, `C`, `D`). <!-- src: /desktop/gui/src/designer/mod.rs#kind_help, /firmware/main/ui/core/ui_screen.c#build_grade --> |

**Placing and ordering:** <!-- src: /desktop/gui/src/designer/mod.rs#pointer_down, /desktop/gui/src/designer/overlay.rs -->

- Drag a widget on the preview to move it, or drag its bottom-right pink handle to resize it. <!-- src: /desktop/gui/src/designer/mod.rs#pointer_move, /desktop/gui/src/designer/overlay.rs -->
- Arrow keys nudge the selected widget by `1 px` (`Shift` + arrow: `10 px`); `Delete` removes it. <!-- src: /desktop/gui/src/designer/mod.rs#subscription -->
- In the **Widgets (`n`/32)** list (`top of list = drawn on top`), **Raise** and **Lower** change the draw order, **Duplicate** copies the widget offset by `+8, +8 px`, and **Delete** removes it. <!-- src: /desktop/gui/src/designer/mod.rs#widget_list -->

## Properties

The selected widget's settings, under **Properties**: <!-- src: /desktop/gui/src/designer/mod.rs#properties -->

| Property | Values |
|---|---|
| **Type**, **Data** | Widget type; data source ([Data sources](#data-sources-ui_source_t)). |
| **Font** | `12 px`, `14 px`, `16 px`, `20 px`, `24 px`, `32 px`, `48 px` <!-- src: /desktop/crates/opad-layout/src/lib.rs#Font --> |
| **Align** | `Left`, `Center`, `Right` <!-- src: /desktop/crates/opad-layout/src/lib.rs#Align --> |
| **X**, **Y** | `-320` to `640`, `-240` to `480` |
| **Width**, **Height** | `1` to `640`, `1` to `480` |
| **Radius** | `0` to `120` |
| **Decimals** | `Source default` or `0`–`4` |
| **Label**, **Suffix** | Up to `31` and `11` bytes <!-- src: /desktop/crates/opad-layout/src/lib.rs#LABEL_MAX_BYTES, /desktop/crates/opad-layout/src/lib.rs#SUFFIX_MAX_BYTES --> |
| **Text / fg**, **Background**, **Accent**, **Screen bg** | Click the swatch or type `#RRGGBB` <!-- src: /desktop/gui/src/designer/mod.rs#color_row --> |
| Flags | **Fill background (text)**, **Border**, **Hide when there is no data**, **Show tap rate (key's live KPM or KPS)** (Key card only) |
| **Unit** | `KPM (keys per minute)` or `KPS (keys per second)`; also updates whole-word `KPM`/`PPM`/`KPS` in `Label` and `Suffix` <!-- src: /desktop/crates/opad-layout/src/lib.rs#set_tap_rate_unit --> |

## Applying, undoing and resetting

| Button | Effect |
|---|---|
| **Apply to pad** | Outside a map (`Idle`): the daemon saves the layout to SQLite and sends it to the pad, which stores it in NVS (`opad_ui`). During a map or cooldown: it goes to the pad's RAM at once and is saved to disk and NVS when the pad is back in `Idle`. <!-- src: /desktop/daemon/src/ipc_handlers.rs#IpcRequest::SetLayout, /firmware/main/ui/ui_store.c#ui_store_save --> |
| **Apply both** | Sends every screen that has edits, one after the other. A `•` on **Idle screen** or **Playing screen** marks a screen with edits the pad does not show yet. |
| **Undo** (Ctrl+Z) | Steps back through this screen's edits, up to 100. One drag or one typed word is one step. It also brings back what **Revert**, **Import**, **Reload** or **Reset to default** replaced. |
| **Revert** | Goes back to what the pad shows. |
| **Reset to default** | The pad forgets this screen's custom layout and draws its built-in one again, at once. **Undo** brings your layout back as an edit to apply. |
| **Reload** | Fetches the saved layouts from the daemon again, in place of local edits. |
<!-- src: /desktop/gui/src/designer/mod.rs#handle, /desktop/gui/src/designer/mod.rs#update, /desktop/daemon/src/ipc_handlers.rs#IpcRequest::ResetLayout -->

## Export and import a layout

Both work on the screen selected at the top (**Idle screen** or **Playing screen**).

- **Export** opens a save dialog defaulting to `opad-idle.json` or `opad-playing.json` and writes the layout as formatted JSON. <!-- src: /desktop/gui/src/designer/mod.rs#export_layout -->
- **Import** asks for a `.json` layout file, validates it against the same rules as the firmware (`ui_layout_validate`) and loads it into the editor. Click **Apply to pad** to save and send it. <!-- src: /desktop/gui/src/designer/mod.rs#import_layout, /desktop/crates/opad-layout/src/lib.rs#validate -->

## Data sources (`ui_source_t`)

Each widget binds to one source ID from `ui_source_t` (`firmware/main/ui/core/ui_ids.h`). The **Data** list shows each as `<group>: <label>`, for example `Map: Title`, and lists every source below except `72` and `73` (`pad.k1_down`, `pad.k2_down`), the press-state flags that **Key card** widgets bind by themselves. ID `42` is reserved. <!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#all_sources, /desktop/crates/opad-layout/src/lib.rs#SourceInfo -->

In the tables, *tap rate* means an integer in KPM, or one decimal in KPS; numbers in brackets are the default decimals. `0` (`none`, `None (static text)`) is static `Label` + `Suffix` with no live data. <!-- src: /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#STATIC_SOURCE, /desktop/gui/src/designer/mod.rs#kind_help -->

### Map (from `tosu`)

| ID | Identifier | Label | Format | Description |
|---|---|---|---|---|
| `1` | `map.title` | Title | Text | Beatmap title. |
| `2` | `map.artist` | Artist | Text | Beatmap artist. |
| `3` | `map.mapper` | Mapper | Text | Beatmap creator. |
| `4` | `map.difficulty` | Difficulty | Text | Difficulty name. |
| `5` | `map.status` | Status | Text | Ranked status (e.g., `ranked`, `loved`). |
| `6` | `map.stars` | Stars | Fixed (`2`) | Full star rating (with active mods). |
| `7` | `map.stars_live` | Stars live | Fixed (`2`) | Live star rating at the current position. |
| `8` | `map.ar` | AR | Fixed (`1`) | Approach Rate. |
| `9` | `map.cs` | CS | Fixed (`1`) | Circle Size. |
| `10` | `map.od` | OD | Fixed (`1`) | Overall Difficulty. |
| `11` | `map.hp` | HP | Fixed (`1`) | HP Drain Rate. |
| `12` | `map.bpm` | BPM | Integer | Beatmap BPM (not your tapping speed). |
| `13` | `map.objects` | Objects | Grouped integer | Total hit objects. |
| `14` | `map.max_combo` | Max combo | Grouped integer | Maximum possible combo. |
| `15` | `map.length` | Length | Duration (`m:ss`) | Total length (ms). |
| `16` | `map.time_elapsed` | Time elapsed | Duration (`m:ss`) | Elapsed time (ms). |
| `17` | `map.time_remaining` | Time remaining | Duration (`m:ss`) | Remaining time (ms). |
| `18` | `map.progress` | Progress | Percent (`0`, raw `0..1`) | Beatmap progress (usable in Progress bar). |
| `19` | `map.kiai` | Kiai | Integer (`0`/`1`) | `1` during kiai time. |
<!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#source_info -->

### Live play (from `tosu` and daemon tap-rate)

| ID | Identifier | Label | Format | Description |
|---|---|---|---|---|
| `20` | `play.pp` | PP | Fixed (`0`) | Current live PP. |
| `21` | `play.pp_fc` | PP fc | Fixed (`0`) | Estimated PP if full combo. |
| `22` | `play.pp_max` | PP max | Fixed (`0`) | Maximum achievable PP on this play. |
| `23` | `play.accuracy` | Accuracy | Fixed (`2`) | Current accuracy percentage. |
| `24` | `play.score` | Score | Grouped integer | Current score. |
| `25` | `play.combo` | Combo | Grouped integer | Current combo. |
| `26` | `play.max_combo` | Max combo | Grouped integer | Highest combo in this attempt. |
| `27` | `play.grade` | Grade | Text | Current rank letter (`SS`, `S`, `A`, `B`, `C`, `D`). |
| `28` | `play.hits_300` | Hits 300 | Grouped integer | Count of 300 hits. |
| `29` | `play.hits_100` | Hits 100 | Grouped integer | Count of 100 hits. |
| `30` | `play.hits_50` | Hits 50 | Grouped integer | Count of 50 hits. |
| `31` | `play.hits_miss` | Hits miss | Grouped integer | Miss count. |
| `32` | `play.slider_breaks` | Slider breaks | Grouped integer | Slider break count. |
| `33` | `play.ur` | UR | Fixed (`1`) | Unstable Rate. |
| `34` | `play.health` | Health | Percent (`0`, raw `0..1`) | Current HP bar (usable in Progress bar). |
| `35` | `play.mods` | Mods | Text | Active mods (e.g., `HDDT`). |
| `36` | `play.player` | Player | Text | Player name on the active play. |
| `37` | `play.failed` | Failed | Integer (`0`/`1`) | `1` when the play has failed. |
| `51` | `play.ppm` | Live KPM | Tap rate | Current combined tapping speed (`0` while not tapping). |
| `52` | `play.ppm_avg` | Tapping avg KPM | Tap rate | Attempt average while tapping (breaks excluded). |
| `53` | `play.ppm_peak` | PEAK KPM | Tap rate | Fastest 7 consecutive presses in this attempt. |
| `54` | `play.k1_ppm` | K1 live KPM | Tap rate | Key 1 live tapping speed. |
| `55` | `play.k1_ppm_avg` | K1 tapping avg KPM | Tap rate | Key 1 average while tapping in this attempt. |
| `56` | `play.k1_ppm_peak` | K1 peak KPM | Tap rate | Key 1 peak burst in this attempt. |
| `57` | `play.k2_ppm` | K2 live KPM | Tap rate | Key 2 live tapping speed. |
| `58` | `play.k2_ppm_avg` | K2 tapping avg KPM | Tap rate | Key 2 average while tapping in this attempt. |
| `59` | `play.k2_ppm_peak` | K2 peak KPM | Tap rate | Key 2 peak burst in this attempt. |
| `84` | `play.ppm_song` | AVG KPM | Tap rate | Song-wide tap rate (first to last note, breaks included). |
| `86` | `play.ppm_song_peak` | Best 10 s KPM | Tap rate | Fastest 10-second song window in this attempt. |
<!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#tap_rate_label -->

### Profile & session (from `tosu`)

| ID | Identifier | Label | Format | Description |
|---|---|---|---|---|
| `40` | `profile.name` | Name | Text | Logged-in osu! username. |
| `41` | `profile.rank` | Rank | Grouped integer | Global rank. |
| `43` | `profile.pp` | PP | Fixed (`0`) | Profile total PP. |
| `44` | `profile.accuracy` | Accuracy | Fixed (`2`) | Profile accuracy percentage. |
| `45` | `profile.playcount` | Playcount | Grouped integer | Profile total playcount. |
| `46` | `profile.level` | Level | Fixed (`0`) | Profile level. |
| `47` | `profile.country` | Country | Text | Country code. |
| `48` | `session.playtime` | Playtime | Duration (`m:ss` / `h:mm:ss`) | Active session playtime (ms). |
| `49` | `session.playcount` | Playcount | Grouped integer | Maps played in this session. |
| `50` | `game.state` | State | Text | Current osu! state (e.g., `Playing`, `Song select`). |
<!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#source_info -->

### Pad (device-local, works without a host)

| ID | Identifier | Label | Format | Description |
|---|---|---|---|---|
| `60` | `pad.k1_map` | K1 map | Grouped integer | First game key presses in the current map. |
| `61` | `pad.k2_map` | K2 map | Grouped integer | Second game key presses in the current map. |
| `62` | `pad.total_map` | Total map | Grouped integer | Combined game key presses in the current map. |
| `63` | `pad.k1_lifetime` | K1 lifetime | Grouped integer | First game key lifetime presses. |
| `64` | `pad.k2_lifetime` | K2 lifetime | Grouped integer | Second game key lifetime presses. |
| `65` | `pad.total_lifetime` | Total lifetime | Grouped integer | Combined lifetime presses of the active game keys. |
| `66` | `pad.kps` | KPS | Integer | Pad-local rolling 1-second keys per second. |
| `67` | `pad.k1_label` | K1 label | Text | Key character of the first game key (e.g., `Z`). |
| `68` | `pad.k2_label` | K2 label | Text | Key character of the second game key (e.g., `X`). |
| `69` | `pad.clock` | Clock | Text | Local clock (`HH:MM`, synced from host). |
| `70` | `pad.date` | Date | Text | Local date (e.g., `Sat 12 Sep`). |
| `71` | `pad.uptime` | Uptime | Duration (`m:ss` / `h:mm:ss`) | Time since pad boot (ms). |
| `72` | `pad.k1_down` | *(Key card only)* | Integer (`0`/`1`) | Key 1 pressed state (not in the list). |
| `73` | `pad.k2_down` | *(Key card only)* | Integer (`0`/`1`) | Key 2 pressed state (not in the list). |
<!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#all_sources -->

### History (from daemon SQLite, independent of `tosu`)

Over the selected history period.

| ID | Identifier | Label | Format | Description |
|---|---|---|---|---|
| `74` | `history.ppm_avg` | Tapping avg KPM | Tap rate | Tapping average. |
| `75` | `history.ppm_peak` | Best PEAK KPM | Tap rate | Best 7-press burst peak. |
| `76` | `history.k1_ppm_avg` | K1 tapping avg KPM | Tap rate | Key 1 tapping average. |
| `77` | `history.k1_ppm_peak` | K1 best peak KPM | Tap rate | Key 1 best peak. |
| `78` | `history.k2_ppm_avg` | K2 tapping avg KPM | Tap rate | Key 2 tapping average. |
| `79` | `history.k2_ppm_peak` | K2 best peak KPM | Tap rate | Key 2 best peak. |
| `83` | `history.period` | Period | Text | Period label (e.g., `LAST 30 DAYS`, `ALL TIME`). |
| `85` | `history.ppm_song` | AVG KPM | Tap rate | Average song-wide tap rate. |
| `87` | `history.ppm_song_peak` | Best 10 s KPM | Tap rate | Best 10-second song tap rate. |
<!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#tap_rate_label -->

### Connection (status indicators, `0`/`1`)

| ID | Identifier | Label | Format | Description |
|---|---|---|---|---|
| `80` | `status.pc` | PC | Integer (`0`/`1`) | `1` when `opad-daemon` is connected over USB CDC. |
| `81` | `status.tosu` | Tosu | Integer (`0`/`1`) | `1` when `opad-daemon` is connected to `tosu`. |
| `82` | `status.osu` | Osu | Integer (`0`/`1`) | `1` when `tosu` sees a running `osu!` process. |
<!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#source_info -->

## If it did not work

| You see | What to do |
|---|---|
| Status line `Applied: Layout applied to pad RAM; will be saved after gameplay` | Nothing: you applied during a map; it is saved once the map ends. <!-- src: /desktop/gui/src/designer/mod.rs#Message::Applied --> |
| Status line `Applied: Saved; it will be applied when the pad connects` | Nothing: the pad is unplugged; plug it in. <!-- src: /desktop/gui/src/designer/mod.rs#Message::Applied --> |
| `This pad's firmware shows tap rates in KPM only; update the pad's firmware to show them in KPS` | A widget uses **Unit** = `KPS (keys per second)` (`FLAG_PER_SECOND`), which older firmware ignores. Switch it back to `KPM (keys per minute)` or follow [Updating Firmware](update-firmware.md). <!-- src: /desktop/daemon/src/ipc_handlers.rs#IpcRequest::SetLayout --> |
| `Daemon unavailable (...)` or `No live data yet (is tosu running?)` | Check that `opad-daemon` and `tosu` are running; see [Troubleshooting](../troubleshooting.md) and [Using with tosu](use-with-tosu.md). <!-- src: /desktop/gui/src/designer/mod.rs#update --> |
| Layout rejected on import or apply | It needs at most `32` widgets, `Width` 1–640, `Height` 1–480, `Label` ≤ 31 bytes, `Suffix` ≤ 11 bytes, and valid `ui_source_t` IDs. <!-- src: /desktop/crates/opad-layout/src/lib.rs#validate --> |
| The pad stays on the red calibration screen | An uncalibrated Hall Effect pad hides normal screens until calibration finishes; see [Calibrating Hall Effect Switches](calibrate-hall-effect.md). <!-- src: /firmware/main/ui/ui_port.c#pad_timer_cb --> |
