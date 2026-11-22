---
title: Customizing the Screen Layout
description: How to design, preview, apply, export, and import custom Idle and Playing screen layouts in opad-gui, with every available data source.
sidebar:
  order: 3
---

OPad's `320x240` LCD has two independent layouts: the **Idle screen** (shown outside a map) and the **Playing screen** (shown while a map is running). You edit both in the **Designer** page of `opad-gui` using a 2x pixel-exact preview rendered by the firmware's own LVGL layout engine (`opad-ui-preview`). <!-- src: /desktop/gui/src/designer/mod.rs, /firmware/main/ui/core/ui_core.h, /firmware/main/ui/ui_port.c#pad_timer_cb -->

## What you need

- `opad-gui` and `opad-daemon` running. You can design, export, and import layouts even while the pad is disconnected; pressing **Apply to pad** saves the layout in the daemon and sends it automatically the next time the pad connects. <!-- src: /desktop/gui/src/designer/mod.rs#update, /desktop/daemon/src/ipc_handlers.rs#IpcRequest::SetLayout -->
- Your OPad connected over USB to see changes live on the screen. If the pad belongs to another installation, claim ownership first (see [Recovery, Reflashing & Unbinding](../recovery.md)). <!-- src: /desktop/daemon/src/ipc_handlers.rs#IpcRequest::SetLayout -->
- `tosu` running if you want to preview **Live data from osu!** (otherwise the preview uses built-in sample values). <!-- src: /desktop/gui/src/designer/mod.rs#Message::LiveData -->

## Building a layout

1. Open `opad-gui` and select **Designer** in the left navigation bar (or launch `opad-gui --page designer`). <!-- src: /desktop/gui/src/main.rs#Page::ALL, /desktop/gui/src/main.rs#TOKENS -->
2. Pick the screen to edit at the top of the preview: **Idle screen** or **Playing screen**. Each screen is saved independently. <!-- src: /desktop/gui/src/designer/mod.rs#view, /desktop/crates/opad-layout/src/lib.rs#Screen -->
3. Under **Add widget**, click a widget type to add it to the screen (up to `32` widgets per screen): <!-- src: /desktop/gui/src/designer/mod.rs#palette, /desktop/crates/opad-layout/src/lib.rs#MAX_WIDGETS -->
   - **Text**: Renders `Label` + bound value + `Suffix`. With `Data` set to `None (static text)`, it shows static text. Tap-rate sources can switch between `KPM (keys per minute)` and `KPS (keys per second)`. <!-- src: /desktop/gui/src/designer/mod.rs#kind_help, /desktop/crates/opad-layout/src/lib.rs#WidgetKind -->
   - **Progress bar**: Horizontal bar where `Background` is the track and `Accent` is the fill. Bind it to a `0..1` source such as `map.progress` or `play.health`. <!-- src: /desktop/gui/src/designer/mod.rs#kind_help, /firmware/main/ui/core/ui_screen.c#build_progress -->
   - **Key card**: Counter card for K1 or K2 (`pad.k1_map`, `pad.k1_lifetime`, `pad.k1_label`, or the K2 equivalents). `Label` is the card title; `Background` and `Accent` swap while the key is held. Enable **Show tap rate (key's live KPM or KPS)** to show that key's live rate in the corner. <!-- src: /desktop/gui/src/designer/mod.rs#kind_help, /firmware/main/ui/core/ui_screen.c#build_keycard -->
   - **Status dot**: Circle (`Accent` when active, `Background` when inactive) followed by `Label` in `Text / fg` color. Bind it to a `0/1` source such as `status.pc`, `status.tosu`, or `status.osu`. <!-- src: /desktop/gui/src/designer/mod.rs#kind_help, /firmware/main/ui/core/ui_screen.c#build_status_dot -->
   - **Rectangle**: Filled rounded rectangle (`Background` with `Radius`), with an optional 1 px **Border** in `Text / fg` color. <!-- src: /desktop/gui/src/designer/mod.rs#kind_help, /firmware/main/ui/core/ui_screen.c#build_rect -->
   - **Grade**: Renders the rank letter (`play.grade`) in osu!'s grade colors (`SS`/`X`, `S`, `A`, `B`, `C`, `D`). <!-- src: /desktop/gui/src/designer/mod.rs#kind_help, /firmware/main/ui/core/ui_screen.c#build_grade -->
4. Position and layer widgets: <!-- src: /desktop/gui/src/designer/mod.rs#pointer_down, /desktop/gui/src/designer/overlay.rs -->
   - Drag any widget on the preview to move it, or drag its bottom-right pink handle to resize it. <!-- src: /desktop/gui/src/designer/mod.rs#pointer_move, /desktop/gui/src/designer/overlay.rs -->
   - Use the arrow keys to nudge the selected widget by `1 px` (`Shift` + arrow keys for `10 px`), and `Delete` to remove it. <!-- src: /desktop/gui/src/designer/mod.rs#subscription -->
   - In the **Widgets (`n`/32)** list (`top of list = drawn on top`), use **Raise**, **Lower**, **Duplicate** (offsets the copy by `+8, +8 px`), and **Delete** to manage draw order. <!-- src: /desktop/gui/src/designer/mod.rs#widget_list -->
5. Edit the selected widget under **Properties**: <!-- src: /desktop/gui/src/designer/mod.rs#properties -->
   - **Type**, **Data**, **Font** (`12 px`, `14 px`, `16 px`, `20 px`, `24 px`, `32 px`, `48 px`), and **Align** (`Left`, `Center`, `Right`). <!-- src: /desktop/crates/opad-layout/src/lib.rs#Font, /desktop/crates/opad-layout/src/lib.rs#Align -->
   - **X** (`-320` to `640`), **Y** (`-240` to `480`), **Width** (`1` to `640`), **Height** (`1` to `480`), **Radius** (`0` to `120`), and **Decimals** (`Source default` or `0`–`4`). <!-- src: /desktop/gui/src/designer/mod.rs#properties -->
   - **Label** (up to `31` bytes) and **Suffix** (up to `11` bytes). <!-- src: /desktop/crates/opad-layout/src/lib.rs#LABEL_MAX_BYTES, /desktop/crates/opad-layout/src/lib.rs#SUFFIX_MAX_BYTES -->
   - **Text / fg**, **Background**, **Accent**, and **Screen bg** (click the color swatch or type a `#RRGGBB` hex value). <!-- src: /desktop/gui/src/designer/mod.rs#color_row -->
   - Optional flags: **Fill background (text)**, **Border**, **Hide when there is no data**, **Show tap rate (key's live KPM or KPS)** (Key card only), and **Unit** (`KPM (keys per minute)` or `KPS (keys per second)`, which also updates whole-word `KPM`/`PPM`/`KPS` labels and suffixes). <!-- src: /desktop/gui/src/designer/mod.rs#properties, /desktop/crates/opad-layout/src/lib.rs#set_tap_rate_unit -->
6. Use the preview checkboxes below the action bar to test your layout before sending it: <!-- src: /desktop/gui/src/designer/mod.rs#view -->
   - **Live data from osu!**: Polls `opad-daemon` every second to render real `tosu` and tap-rate values. <!-- src: /desktop/gui/src/designer/mod.rs#subscription -->
   - **Show keys pressed**: Simulates both keys held down so you can check Key card highlight colors. <!-- src: /desktop/gui/src/designer/mod.rs#Message::SimulatePress -->
7. Click **Apply to pad** (labeled **Apply to pad •** while edits are unapplied). <!-- src: /desktop/gui/src/designer/mod.rs#view -->
   - Outside a map (`Idle`), the daemon saves the layout to SQLite and sends it to the pad, which stores it in NVS (`opad_ui`). <!-- src: /desktop/daemon/src/ipc_handlers.rs#IpcRequest::SetLayout, /firmware/main/ui/ui_store.c -->
   - During a map or cooldown (`Playing` / `Cooldown`), the layout is applied to the pad's RAM immediately and persisted to disk and NVS once the pad returns to `Idle`. <!-- src: /desktop/daemon/src/ipc_handlers.rs#IpcRequest::SetLayout, /firmware/main/ui/ui_store.c#ui_store_save -->

To undo unapplied changes, click **Revert**. To restore the firmware's built-in layout for the active screen, click **Reset to default** and then **Apply to pad**. To discard local edits and re-fetch saved layouts from the daemon, click **Reload**. <!-- src: /desktop/gui/src/designer/mod.rs#update -->

## Exporting and importing layouts

- **Export**: Select either **Idle screen** or **Playing screen** and click **Export**. `opad-gui` opens a save dialog defaulting to `opad-idle.json` or `opad-playing.json` and writes the layout as formatted JSON. <!-- src: /desktop/gui/src/designer/mod.rs#export_layout -->
- **Import**: Select the target screen (**Idle screen** or **Playing screen**), click **Import**, and pick a `.json` layout file. `opad-gui` parses and validates it against the same rules as the firmware (`ui_layout_validate`), loads it into the editor, and prompts you to click **Apply to pad** to save and send it. <!-- src: /desktop/gui/src/designer/mod.rs#import_layout, /desktop/crates/opad-layout/src/lib.rs#validate -->

## Selectable data sources (`ui_source_t`)

Every widget can bind to one data source ID defined in `ui_source_t` (`firmware/main/ui/core/ui_ids.h`). The **Data** dropdown lists every source below except IDs `72` and `73` (`pad.k1_down`, `pad.k2_down`), which are internal press-state flags bound automatically by **Key card** widgets. ID `42` is reserved. <!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#all_sources -->

### Static text

| ID | Identifier | Designer label | Format | Description |
|---|---|---|---|---|
| `0` | `none` | `None (static text)` | Text | Static `Label` + `Suffix` with no live data. |
<!-- src: /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#STATIC_SOURCE -->

### Map (from `tosu`)

| ID | Identifier | Designer label | Format (default decimals) | Description |
|---|---|---|---|---|
| `1` | `map.title` | `Map: Title` | Text | Beatmap title. |
| `2` | `map.artist` | `Map: Artist` | Text | Beatmap artist. |
| `3` | `map.mapper` | `Map: Mapper` | Text | Beatmap creator. |
| `4` | `map.difficulty` | `Map: Difficulty` | Text | Difficulty name. |
| `5` | `map.status` | `Map: Status` | Text | Ranked status (e.g., `ranked`, `loved`). |
| `6` | `map.stars` | `Map: Stars` | Fixed (`2`) | Full beatmap star rating (with active mods). |
| `7` | `map.stars_live` | `Map: Stars live` | Fixed (`2`) | Live star rating at current map position. |
| `8` | `map.ar` | `Map: AR` | Fixed (`1`) | Approach Rate. |
| `9` | `map.cs` | `Map: CS` | Fixed (`1`) | Circle Size. |
| `10` | `map.od` | `Map: OD` | Fixed (`1`) | Overall Difficulty. |
| `11` | `map.hp` | `Map: HP` | Fixed (`1`) | HP Drain Rate. |
| `12` | `map.bpm` | `Map: BPM` | Integer | Beatmap BPM (not your tapping speed). |
| `13` | `map.objects` | `Map: Objects` | Grouped integer | Total hit objects in the beatmap. |
| `14` | `map.max_combo` | `Map: Max combo` | Grouped integer | Maximum possible beatmap combo. |
| `15` | `map.length` | `Map: Length` | Duration (`m:ss`) | Total beatmap length (ms). |
| `16` | `map.time_elapsed` | `Map: Time elapsed` | Duration (`m:ss`) | Elapsed beatmap time (ms). |
| `17` | `map.time_remaining` | `Map: Time remaining` | Duration (`m:ss`) | Remaining beatmap time (ms). |
| `18` | `map.progress` | `Map: Progress` | Percent (`0`, raw `0..1`) | Beatmap progress (usable in Progress bar). |
| `19` | `map.kiai` | `Map: Kiai` | Integer (`0`/`1`) | `1` during kiai time, `0` otherwise. |
<!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#source_info -->

### Live play (from `tosu` and daemon tap-rate)

| ID | Identifier | Designer label | Format (default decimals) | Description |
|---|---|---|---|---|
| `20` | `play.pp` | `Live play: PP` | Fixed (`0`) | Current live PP. |
| `21` | `play.pp_fc` | `Live play: PP fc` | Fixed (`0`) | Estimated PP if full combo. |
| `22` | `play.pp_max` | `Live play: PP max` | Fixed (`0`) | Maximum achievable PP on this play. |
| `23` | `play.accuracy` | `Live play: Accuracy` | Fixed (`2`) | Current accuracy percentage. |
| `24` | `play.score` | `Live play: Score` | Grouped integer | Current score. |
| `25` | `play.combo` | `Live play: Combo` | Grouped integer | Current combo. |
| `26` | `play.max_combo` | `Live play: Max combo` | Grouped integer | Highest combo achieved in this attempt. |
| `27` | `play.grade` | `Live play: Grade` | Text | Current rank letter (`SS`, `S`, `A`, `B`, `C`, `D`). |
| `28` | `play.hits_300` | `Live play: Hits 300` | Grouped integer | Count of 300 hits. |
| `29` | `play.hits_100` | `Live play: Hits 100` | Grouped integer | Count of 100 hits. |
| `30` | `play.hits_50` | `Live play: Hits 50` | Grouped integer | Count of 50 hits. |
| `31` | `play.hits_miss` | `Live play: Hits miss` | Grouped integer | Miss count. |
| `32` | `play.slider_breaks` | `Live play: Slider breaks` | Grouped integer | Slider break count. |
| `33` | `play.ur` | `Live play: UR` | Fixed (`1`) | Unstable Rate. |
| `34` | `play.health` | `Live play: Health` | Percent (`0`, raw `0..1`) | Current HP bar (usable in Progress bar). |
| `35` | `play.mods` | `Live play: Mods` | Text | Active mods string (e.g., `HDDT`). |
| `36` | `play.player` | `Live play: Player` | Text | Player name on the active play. |
| `37` | `play.failed` | `Live play: Failed` | Integer (`0`/`1`) | `1` when the play has failed, `0` otherwise. |
| `51` | `play.ppm` | `Live play: Live KPM` | Integer (or `1` dec. in KPS) | Current combined tapping speed (`0` while not tapping). |
| `52` | `play.ppm_avg` | `Live play: Tapping avg KPM` | Integer (or `1` dec. in KPS) | Attempt average while tapping (breaks excluded). |
| `53` | `play.ppm_peak` | `Live play: PEAK KPM` | Integer (or `1` dec. in KPS) | Fastest 7 consecutive presses in this attempt. |
| `54` | `play.k1_ppm` | `Live play: K1 live KPM` | Integer (or `1` dec. in KPS) | Current live tapping speed for Key 1. |
| `55` | `play.k1_ppm_avg` | `Live play: K1 tapping avg KPM` | Integer (or `1` dec. in KPS) | Key 1 average while tapping in this attempt. |
| `56` | `play.k1_ppm_peak` | `Live play: K1 peak KPM` | Integer (or `1` dec. in KPS) | Key 1 peak burst speed in this attempt. |
| `57` | `play.k2_ppm` | `Live play: K2 live KPM` | Integer (or `1` dec. in KPS) | Current live tapping speed for Key 2. |
| `58` | `play.k2_ppm_avg` | `Live play: K2 tapping avg KPM` | Integer (or `1` dec. in KPS) | Key 2 average while tapping in this attempt. |
| `59` | `play.k2_ppm_peak` | `Live play: K2 peak KPM` | Integer (or `1` dec. in KPS) | Key 2 peak burst speed in this attempt. |
| `84` | `play.ppm_song` | `Live play: AVG KPM` | Integer (or `1` dec. in KPS) | Song-wide tap rate (first to last note, breaks included). |
| `86` | `play.ppm_song_peak` | `Live play: Best 10 s KPM` | Integer (or `1` dec. in KPS) | Fastest 10-second song window in this attempt. |
<!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#tap_rate_label -->

### Profile & session (from `tosu`)

| ID | Identifier | Designer label | Format (default decimals) | Description |
|---|---|---|---|---|
| `40` | `profile.name` | `Profile & session: Name` | Text | Logged-in osu! username. |
| `41` | `profile.rank` | `Profile & session: Rank` | Grouped integer | Global rank. |
| `43` | `profile.pp` | `Profile & session: PP` | Fixed (`0`) | Profile total PP. |
| `44` | `profile.accuracy` | `Profile & session: Accuracy` | Fixed (`2`) | Profile accuracy percentage. |
| `45` | `profile.playcount` | `Profile & session: Playcount` | Grouped integer | Profile total playcount. |
| `46` | `profile.level` | `Profile & session: Level` | Fixed (`0`) | Profile level. |
| `47` | `profile.country` | `Profile & session: Country` | Text | Country code. |
| `48` | `session.playtime` | `Profile & session: Playtime` | Duration (`m:ss` / `h:mm:ss`) | Active session playtime (ms). |
| `49` | `session.playcount` | `Profile & session: Playcount` | Grouped integer | Maps played in this session. |
| `50` | `game.state` | `Profile & session: State` | Text | Current osu! state (e.g., `Playing`, `Song select`). |
<!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#source_info -->

### Pad (device-local, works without a host)

| ID | Identifier | Designer label | Format | Description |
|---|---|---|---|---|
| `60` | `pad.k1_map` | `Pad: K1 map` | Grouped integer | First game key presses in the current map. |
| `61` | `pad.k2_map` | `Pad: K2 map` | Grouped integer | Second game key presses in the current map. |
| `62` | `pad.total_map` | `Pad: Total map` | Grouped integer | Combined game key presses in the current map. |
| `63` | `pad.k1_lifetime` | `Pad: K1 lifetime` | Grouped integer | First game key lifetime press counter. |
| `64` | `pad.k2_lifetime` | `Pad: K2 lifetime` | Grouped integer | Second game key lifetime press counter. |
| `65` | `pad.total_lifetime` | `Pad: Total lifetime` | Grouped integer | Combined lifetime presses for active game keys. |
| `66` | `pad.kps` | `Pad: KPS` | Integer | Pad-local rolling 1-second keys per second. |
| `67` | `pad.k1_label` | `Pad: K1 label` | Text | Bound key character for the first game key (e.g., `Z`). |
| `68` | `pad.k2_label` | `Pad: K2 label` | Text | Bound key character for the second game key (e.g., `X`). |
| `69` | `pad.clock` | `Pad: Clock` | Text | Local clock (`HH:MM`, synced from host). |
| `70` | `pad.date` | `Pad: Date` | Text | Local date (e.g., `Sat 12 Sep`). |
| `71` | `pad.uptime` | `Pad: Uptime` | Duration (`m:ss` / `h:mm:ss`) | Time since pad boot (ms). |
| `72` | `pad.k1_down` | *(internal — Key card only)* | Integer (`0`/`1`) | Key 1 pressed highlight state (hidden from dropdown). |
| `73` | `pad.k2_down` | *(internal — Key card only)* | Integer (`0`/`1`) | Key 2 pressed highlight state (hidden from dropdown). |
<!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#all_sources -->

### History (from daemon SQLite, independent of `tosu`)

| ID | Identifier | Designer label | Format (default decimals) | Description |
|---|---|---|---|---|
| `74` | `history.ppm_avg` | `History: Tapping avg KPM` | Integer (or `1` dec. in KPS) | Historical tapping average over the selected period. |
| `75` | `history.ppm_peak` | `History: Best PEAK KPM` | Integer (or `1` dec. in KPS) | Best 7-press burst peak over the selected period. |
| `76` | `history.k1_ppm_avg` | `History: K1 tapping avg KPM` | Integer (or `1` dec. in KPS) | Historical Key 1 tapping average. |
| `77` | `history.k1_ppm_peak` | `History: K1 best peak KPM` | Integer (or `1` dec. in KPS) | Historical Key 1 best peak. |
| `78` | `history.k2_ppm_avg` | `History: K2 tapping avg KPM` | Integer (or `1` dec. in KPS) | Historical Key 2 tapping average. |
| `79` | `history.k2_ppm_peak` | `History: K2 best peak KPM` | Integer (or `1` dec. in KPS) | Historical Key 2 best peak. |
| `83` | `history.period` | `History: Period` | Text | Active history window label (e.g., `LAST 30 DAYS`, `ALL TIME`). |
| `85` | `history.ppm_song` | `History: AVG KPM` | Integer (or `1` dec. in KPS) | Historical average song-wide tap rate. |
| `87` | `history.ppm_song_peak` | `History: Best 10 s KPM` | Integer (or `1` dec. in KPS) | Best 10-second song tap rate over the selected period. |
<!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#tap_rate_label -->

### Connection (status indicators, `0`/`1`)

| ID | Identifier | Designer label | Format | Description |
|---|---|---|---|---|
| `80` | `status.pc` | `Connection: PC` | Integer (`0`/`1`) | `1` when `opad-daemon` is connected over USB CDC. |
| `81` | `status.tosu` | `Connection: Tosu` | Integer (`0`/`1`) | `1` when `opad-daemon` is connected to `tosu`. |
| `82` | `status.osu` | `Connection: Osu` | Integer (`0`/`1`) | `1` when `tosu` sees a running `osu!` process. |
<!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#source_info -->

## How to check it worked

- After clicking **Apply to pad**, the status line below the preview changes to `Applied to the pad` (or `Layout applied to pad RAM; will be saved after gameplay` during a map) and the `•` marker disappears from the button. <!-- src: /desktop/gui/src/designer/mod.rs#Message::Applied -->
- The pad's LCD immediately reflects the new **Idle screen** layout (and switches to your **Playing screen** layout when you start a map in osu!). <!-- src: /firmware/main/ui/ui_port.c#pad_timer_cb -->
- Unplug and replug the pad: the custom layouts reload from NVS (`opad_ui`) automatically. <!-- src: /firmware/main/ui/ui_port.c#ui_init, /firmware/main/ui/ui_store.c -->

## If it did not work

- **`This pad's firmware shows tap rates in KPM only; update the pad's firmware to show them in KPS`**: A widget uses **Unit** = `KPS (keys per second)` (`FLAG_PER_SECOND`), which older firmware ignores. Either switch the widget back to `KPM (keys per minute)` or follow [Updating Firmware](update-firmware.md). <!-- src: /desktop/daemon/src/ipc_handlers.rs#IpcRequest::SetLayout -->
- **`Daemon unavailable (...)` or `No live data yet (is tosu running?)`**: Check that `opad-daemon` and `tosu` are running; see [Troubleshooting](../troubleshooting.md) and [Using with tosu](use-with-tosu.md). <!-- src: /desktop/gui/src/designer/mod.rs#update -->
- **Layout rejected on import or apply**: Check that the layout has at most `32` widgets, valid widget dimensions (`Width` 1–640, `Height` 1–480), `Label` ≤ 31 bytes, `Suffix` ≤ 11 bytes, and valid `ui_source_t` IDs. <!-- src: /desktop/crates/opad-layout/src/lib.rs#validate -->
- **Pad is locked on the red calibration screen**: An uncalibrated Hall Effect pad hides normal screens until calibration finishes; see [Calibrating Hall Effect Switches](calibrate-hall-effect.md). <!-- src: /firmware/main/ui/ui_port.c#pad_timer_cb -->
