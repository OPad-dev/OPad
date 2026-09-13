//! Dashboard, settings, device and monitor pages.

use crate::theme::{self, caption, heading, muted};
use crate::{App, Message};
use iced::widget::{button, checkbox, column, container, row, scrollable, slider, text, text_input, Space};
use iced::{Alignment, Color, Element, Length};
use osupad_model::ui_source::{self as src, SourceValue};

fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn text_value(app: &App, source: u8) -> Option<&str> {
    match app.ui_values.get(&source)? {
        SourceValue::Text(t) if !t.is_empty() => Some(t),
        _ => None,
    }
}

fn number_value(app: &App, source: u8) -> Option<f64> {
    match app.ui_values.get(&source)? {
        SourceValue::Number(n) => Some(*n),
        _ => None,
    }
}

fn card<'a>(content: impl Into<Element<'a, Message>>) -> container::Container<'a, Message> {
    container(content).padding(20).width(Length::Fill).style(theme::card)
}

fn stat<'a>(label: &'a str, value: String) -> Element<'a, Message> {
    column![caption(label), text(value).size(20).font(theme::FONT_BOLD)].spacing(4).into()
}

// ---- dashboard ------------------------------------------------------------------------------

pub fn dashboard(app: &App) -> Element<'_, Message> {
    let c = &app.counters;
    let dark_70 = Color { a: 0.7, ..theme::BG };

    let total = card(
        column![
            caption("TOTAL PRESSES"),
            text(grouped(c.total_lifetime_presses())).size(64).font(theme::FONT_BOLD),
        ]
        .spacing(2)
        .align_x(Alignment::Center)
        .width(Length::Fill),
    );

    let k1 = container(
        column![
            text(format!("K1 · {}", app.config.key1_char())).size(14).color(dark_70),
            text(grouped(c.lifetime_key1)).size(40).font(theme::FONT_BOLD).color(theme::BG),
            text(format!("this map {}", c.map_key1)).size(13).color(dark_70),
        ]
        .spacing(4)
        .align_x(Alignment::Center)
        .width(Length::Fill),
    )
    .padding(18)
    .width(Length::Fill)
    .style(theme::pink_card);

    let k2 = container(
        column![
            caption(format!("K2 · {}", app.config.key2_char())).size(14),
            text(grouped(c.lifetime_key2)).size(40).font(theme::FONT_BOLD),
            muted(format!("this map {}", c.map_key2)).size(13),
        ]
        .spacing(4)
        .align_x(Alignment::Center)
        .width(Length::Fill),
    )
    .padding(18)
    .width(Length::Fill)
    .style(theme::outlined_card);

    let profile: Element<'_, Message> = match text_value(app, src::PROFILE_NAME) {
        Some(name) => column![
            caption("OSU! PROFILE"),
            text(name).size(24).font(theme::FONT_BOLD),
            row![
                muted(number_value(app, src::PROFILE_RANK).map_or("#-".into(), |r| format!("#{}", grouped(r as u64)))),
                text(number_value(app, src::PROFILE_PP).map_or("-".into(), |p| format!("{:.0}pp", p))).color(theme::PINK),
            ]
            .spacing(16),
            muted(format!(
                "{} accuracy · {} plays",
                number_value(app, src::PROFILE_ACCURACY).map_or("-".into(), |a| format!("{:.2}%", a)),
                number_value(app, src::PROFILE_PLAYCOUNT).map_or("-".into(), |p| grouped(p as u64)),
            ))
            .size(13),
        ]
        .spacing(6)
        .into(),
        None => column![caption("OSU! PROFILE"), muted("Open osu! with tosu running to see your profile.")]
            .spacing(6)
            .into(),
    };

    let now: Element<'_, Message> = match text_value(app, src::MAP_TITLE) {
        Some(title) => {
            let mut info = column![
                caption(text_value(app, src::GAME_STATE).unwrap_or("NOW").to_uppercase()),
                text(title).size(20).font(theme::FONT_BOLD),
                muted(text_value(app, src::MAP_ARTIST).unwrap_or("")),
                row![
                    text(text_value(app, src::MAP_DIFFICULTY).map_or(String::new(), |d| format!("[{}]", d)))
                        .size(14)
                        .color(theme::CYAN),
                    text(number_value(app, src::MAP_STARS).map_or(String::new(), |s| format!("{:.2} *", s)))
                        .size(14)
                        .color(theme::YELLOW),
                ]
                .spacing(10),
            ]
            .spacing(6);
            if app.mode == osupad_model::RuntimeMode::Playing {
                if let Some(pp) = number_value(app, src::PLAY_PP) {
                    info = info.push(text(format!("{:.0}pp", pp)).size(32).font(theme::FONT_BOLD).color(theme::PINK));
                }
            }
            info.into()
        }
        None => column![caption("NOW"), muted("Nothing selected in osu!.")].spacing(6).into(),
    };

    let latency: Element<'_, Message> = match app.latency {
        Some(l) if l.samples > 0 => row![
            stat("p50", format!("{} µs", l.p50_us)),
            stat("p99", format!("{} µs", l.p99_us)),
            stat("p99.9", format!("{} µs", l.p999_us)),
            stat("max", format!("{} µs", l.max_us)),
            stat("deferred", l.deferred_reports.to_string()),
            stat("presses", grouped(l.samples as u64)),
        ]
        .spacing(36)
        .into(),
        _ => muted("No key presses measured yet.").into(),
    };

    scrollable(
        column![
            row![
                heading("Dashboard"),
                Space::new().width(Length::Fill),
                caption(format!("{:?}", app.mode).to_uppercase()),
            ]
            .align_y(Alignment::Center),
            total,
            row![k1, k2].spacing(14),
            row![card(profile), card(now)].spacing(14),
            card(
                column![
                    row![
                        text("Key latency").size(16).font(theme::FONT_BOLD),
                        Space::new().width(Length::Fill),
                        button(text("Reset").size(13)).style(theme::secondary).on_press(Message::ResetLatency),
                    ]
                    .align_y(Alignment::Center),
                    muted("Switch press to USB report, measured on the pad. Deferred = waited for the next 1 ms USB poll, then sent.").size(13),
                    latency,
                ]
                .spacing(12),
            ),
        ]
        .spacing(14),
    )
    .into()
}

// ---- settings -----------------------------------------------------------------------------

pub fn settings(app: &App) -> Element<'_, Message> {
    let key_input = |label: &'static str, value: &str, on_input: fn(String) -> Message| {
        column![caption(label), text_input("Z", value).on_input(on_input).size(22).width(80).padding(10)].spacing(6)
    };

    let keys = card(
        column![
            text("Keys").size(18).font(theme::FONT_BOLD),
            row![key_input("KEY 1", &app.k1_input, Message::Key1), key_input("KEY 2", &app.k2_input, Message::Key2)]
                .spacing(24),
            column![
                row![caption("DEBOUNCE LOCKOUT"), Space::new().width(Length::Fill), text(format!("{:.1} ms", app.debounce as f32 / 1000.0)).size(14)],
                slider(500..=20000, app.debounce, Message::Debounce).step(500u32),
                muted("Presses within this window after a switch change are ignored.").size(12),
            ]
            .spacing(8),
        ]
        .spacing(18),
    );

    let sleep_label = if app.sleep_seconds % 60 == 0 {
        format!("{} min", app.sleep_seconds / 60)
    } else {
        format!("{} s", app.sleep_seconds)
    };
    let display = card(
        column![
            text("Display").size(18).font(theme::FONT_BOLD),
            column![
                row![caption("BRIGHTNESS"), Space::new().width(Length::Fill), text(format!("{}%", app.brightness)).size(14)],
                slider(10..=100, app.brightness, Message::Brightness),
            ]
            .spacing(8),
            column![
                row![caption("SLEEP AFTER"), Space::new().width(Length::Fill), text(sleep_label).size(14)],
                slider(60..=3600, app.sleep_seconds, Message::SleepSeconds).step(60u32),
            ]
            .spacing(8),
            muted("Screen layouts and colors are edited in the Designer.").size(12),
        ]
        .spacing(18),
    );

    let advanced = card(
        column![
            text("Advanced").size(18).font(theme::FONT_BOLD),
            column![
                row![
                    caption("GAMEPLAY REFRESH RATE"),
                    Space::new().width(Length::Fill),
                    text(format!("{} Hz", app.gameplay_display_hz)).size(14)
                ],
                slider(1..=30, app.gameplay_display_hz, Message::GameplayDisplayHz),
                muted("Rate at which real-time map statistics (PP, progress, hit counts) are streamed to the pad display during play (1–30 Hz, default: 10 Hz).").size(12),
            ]
            .spacing(8),
            column![
                checkbox(app.autostart_tray).label("Start in tray at login").on_toggle(Message::ToggleAutostartTray),
                muted("Automatically start the applet minimized in the system tray when logging into your desktop session.").size(12),
            ]
            .spacing(6),
        ]
        .spacing(18),
    );

    column![
        heading("Settings"),
        row![keys, display].spacing(14),
        advanced,
        row![button(text("Save settings").size(15)).padding([10, 22]).style(theme::primary).on_press(Message::SaveConfig)],
    ]
    .spacing(14)
    .into()
}

// ---- device ---------------------------------------------------------------------------------

pub fn device(app: &App) -> Element<'_, Message> {
    let info = app.device_info.as_ref();
    let line = |label: &'static str, value: String| {
        row![muted(label).width(170), text(value).size(14)].spacing(8)
    };

    let details = card(
        column![
            text("Pad").size(18).font(theme::FONT_BOLD),
            line("Device ID", info.map_or("-".into(), |i| i.device_id.clone())),
            line("Board", info.map_or("-".into(), |i| i.board_profile.clone())),
            line("Firmware", info.map_or("-".into(), |i| i.firmware_version.clone())),
            line("Mode", format!("{:?}", app.mode)),
            line("Counters source", format!("{:?}", app.counters_source)),
            line("Counter generation", app.counters.counter_generation.to_string()),
            line("Last sync", app.last_sync_time.clone().unwrap_or_else(|| "Never".into())),
        ]
        .spacing(10),
    );

    let playing_or_cooldown = matches!(app.mode, osupad_model::RuntimeMode::Playing | osupad_model::RuntimeMode::Cooldown);

    let pc_k1 = app.pc_counters.as_ref().map(|c| c.lifetime_key1).unwrap_or(0);
    let pc_k2 = app.pc_counters.as_ref().map(|c| c.lifetime_key2).unwrap_or(0);
    let pc_total = pc_k1 + pc_k2;
    let pc_gen = app.pc_counters.as_ref().map(|c| c.counter_generation).unwrap_or(0);

    let esp_k1 = app.esp_counters.as_ref().map(|c| c.lifetime_key1).unwrap_or(app.counters.lifetime_key1);
    let esp_k2 = app.esp_counters.as_ref().map(|c| c.lifetime_key2).unwrap_or(app.counters.lifetime_key2);
    let esp_total = esp_k1 + esp_k2;
    let esp_gen = app.esp_counters.as_ref().map(|c| c.counter_generation).unwrap_or(app.counters.counter_generation);

    let has_mismatch = (pc_k1 != esp_k1 || pc_k2 != esp_k2 || pc_gen != esp_gen) && app.device_connected;

    let k1_color = if app.device_connected && pc_k1 != esp_k1 { theme::YELLOW } else { theme::WHITE };
    let k2_color = if app.device_connected && pc_k2 != esp_k2 { theme::YELLOW } else { theme::WHITE };
    let total_color = if app.device_connected && pc_total != esp_total { theme::YELLOW } else { theme::WHITE };
    let gen_color = if app.device_connected && pc_gen != esp_gen { theme::YELLOW } else { theme::WHITE };

    let mut counter_col = column![
        row![
            text("Counter Reconciliation State").size(18).font(theme::FONT_BOLD),
            Space::new().width(Length::Fill),
            if has_mismatch {
                text("⚠ MISMATCH DETECTED").size(12).font(theme::FONT_BOLD).color(theme::YELLOW)
            } else if app.device_connected {
                text("✓ SYNCHRONIZED").size(12).font(theme::FONT_BOLD).color(theme::GREEN)
            } else {
                text("PAD DISCONNECTED").size(12).color(theme::MUTED)
            }
        ].align_y(Alignment::Center),
        muted("Hardware pad counters compared against host PC SQLite database."),
        container(
            column![
                row![
                    text("Metric").size(12).color(theme::MUTED).width(Length::FillPortion(2)),
                    text("PC Database").size(12).color(theme::MUTED).width(Length::FillPortion(3)),
                    text("PAD Hardware").size(12).color(theme::MUTED).width(Length::FillPortion(3)),
                ],
                row![
                    text("Generation").size(13).width(Length::FillPortion(2)),
                    text(pc_gen.to_string()).size(13).color(gen_color).width(Length::FillPortion(3)),
                    text(if app.device_connected { esp_gen.to_string() } else { "-".into() }).size(13).color(gen_color).width(Length::FillPortion(3)),
                ],
                row![
                    text("Key 1 (K1)").size(13).width(Length::FillPortion(2)),
                    text(format!("{} presses", grouped(pc_k1))).size(13).color(k1_color).width(Length::FillPortion(3)),
                    text(if app.device_connected { format!("{} presses", grouped(esp_k1)) } else { "-".into() }).size(13).color(k1_color).width(Length::FillPortion(3)),
                ],
                row![
                    text("Key 2 (K2)").size(13).width(Length::FillPortion(2)),
                    text(format!("{} presses", grouped(pc_k2))).size(13).color(k2_color).width(Length::FillPortion(3)),
                    text(if app.device_connected { format!("{} presses", grouped(esp_k2)) } else { "-".into() }).size(13).color(k2_color).width(Length::FillPortion(3)),
                ],
                row![
                    text("Total").size(13).font(theme::FONT_BOLD).width(Length::FillPortion(2)),
                    text(format!("{} presses", grouped(pc_total))).size(13).font(theme::FONT_BOLD).color(total_color).width(Length::FillPortion(3)),
                    text(if app.device_connected { format!("{} presses", grouped(esp_total)) } else { "-".into() }).size(13).font(theme::FONT_BOLD).color(total_color).width(Length::FillPortion(3)),
                ],
            ].spacing(8)
        )
        .padding(12)
        .style(theme::card),
    ].spacing(12);

    if has_mismatch && !playing_or_cooldown {
        counter_col = counter_col.push(
            row![
                button(text("Restore PAD from PC").size(13))
                    .padding([8, 14])
                    .style(theme::secondary)
                    .on_press(Message::PromptRestoreDeviceFromPc),
                button(text("Import PC from PAD").size(13))
                    .padding([8, 14])
                    .style(theme::secondary)
                    .on_press(Message::PromptImportPcFromDevice),
            ].spacing(10)
        );
    }

    let counter_table = card(counter_col);

    let actions = card(
        column![
            text("Actions").size(18).font(theme::FONT_BOLD),
            row![
                button(text("Sync now").size(14)).padding([10, 18]).style(theme::secondary).on_press(Message::Sync),
                muted("Reconcile counters with the database and set the pad's clock.").size(13),
            ]
            .spacing(14)
            .align_y(Alignment::Center),
            row![
                button(text("Update firmware").size(14)).padding([10, 18]).style(theme::primary).on_press(Message::PromptUpdateFirmware),
                muted("Select a .bin file and flash the pad automatically via osupadctl.").size(13),
            ]
            .spacing(14)
            .align_y(Alignment::Center),
            row![
                button(text("Reset lifetime counters").size(14)).padding([10, 18]).style(theme::danger).on_press(Message::PromptResetCounters),
                muted("Sets K1 and K2 back to zero. This cannot be undone.").size(13),
            ]
            .spacing(14)
            .align_y(Alignment::Center),
        ]
        .spacing(14),
    );

    let export_btn = button(text("Export backup").size(14))
        .padding([10, 18])
        .style(if playing_or_cooldown { theme::secondary } else { theme::primary });
    let export_btn = if !playing_or_cooldown {
        export_btn.on_press(Message::ExportBackup)
    } else {
        export_btn
    };

    let import_btn = button(text("Import backup").size(14))
        .padding([10, 18])
        .style(theme::secondary);
    let import_btn = if !playing_or_cooldown {
        import_btn.on_press(Message::StartImportBackup)
    } else {
        import_btn
    };

    let mut backup_content = column![
        text("Backup & Restore").size(18).font(theme::FONT_BOLD),
        muted("Save your lifetime counters and device configuration to a file, or restore from a previous backup."),
        row![export_btn, import_btn].spacing(12),
    ]
    .spacing(10);

    if playing_or_cooldown {
        backup_content = backup_content.push(
            text("⚠ Backup operations are disabled during active gameplay and cooldown.")
                .size(13)
                .color(theme::YELLOW),
        );
    }

    let backup = card(backup_content);

    scrollable(column![heading("Device"), details, counter_table, actions, backup].spacing(14)).into()
}

// ---- monitor --------------------------------------------------------------------------------

fn filter_btn<'a>(label: &'a str, selected: bool, msg: Message) -> Element<'a, Message> {
    button(text(label).size(12))
        .padding([4, 10])
        .style(if selected { theme::primary } else { theme::secondary })
        .on_press(msg)
        .into()
}

pub fn monitor(app: &App) -> Element<'_, Message> {
    use osupad_model::{LogLevel, LogSource};

    let severity_filters = row![
        caption("LEVEL:"),
        filter_btn("ALL", app.log_filter_level.is_none(), Message::FilterLogLevel(None)),
        filter_btn("DEBUG", app.log_filter_level == Some(LogLevel::Debug), Message::FilterLogLevel(Some(LogLevel::Debug))),
        filter_btn("INFO", app.log_filter_level == Some(LogLevel::Info), Message::FilterLogLevel(Some(LogLevel::Info))),
        filter_btn("WARN", app.log_filter_level == Some(LogLevel::Warn), Message::FilterLogLevel(Some(LogLevel::Warn))),
        filter_btn("ERROR", app.log_filter_level == Some(LogLevel::Error), Message::FilterLogLevel(Some(LogLevel::Error))),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let source_filters = row![
        caption("SOURCE:"),
        filter_btn("ALL", app.log_filter_source.is_none(), Message::FilterLogSource(None)),
        filter_btn("HOST", app.log_filter_source == Some(LogSource::Host), Message::FilterLogSource(Some(LogSource::Host))),
        filter_btn("ESP", app.log_filter_source == Some(LogSource::Esp), Message::FilterLogSource(Some(LogSource::Esp))),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let filter_row = row![severity_filters, Space::new().width(16), source_filters].spacing(10);

    let visible_entries: Vec<_> = app
        .logs
        .iter()
        .filter(|e| e.seq > app.log_cleared_seq)
        .filter(|e| app.log_filter_level.map_or(true, |l| e.level >= l))
        .filter(|e| app.log_filter_source.map_or(true, |s| e.source == s))
        .collect();

    let action_row = row![
        button(text("Clear").size(12))
            .padding([4, 12])
            .style(theme::secondary)
            .on_press(Message::ClearLogs),
        button(text("Copy").size(12))
            .padding([4, 12])
            .style(theme::secondary)
            .on_press(Message::CopyLogs),
        button(text("Save log").size(12))
            .padding([4, 12])
            .style(theme::secondary)
            .on_press(Message::SaveLogs),
        button(text(if app.log_auto_scroll { "Auto-scroll (Newest First)" } else { "Order (Oldest First)" }).size(12))
            .padding([4, 12])
            .style(if app.log_auto_scroll { theme::secondary } else { theme::secondary })
            .on_press(Message::ToggleAutoScroll),
        Space::new().width(Length::Fill),
        caption(format!("{} entries", visible_entries.len())),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let rendered_items: Vec<Element<'_, Message>> = if app.log_auto_scroll {
        visible_entries
            .into_iter()
            .rev()
            .map(|e| {
                let color = match e.level {
                    LogLevel::Error => theme::RED,
                    LogLevel::Warn => theme::YELLOW,
                    LogLevel::Debug => theme::MUTED,
                    LogLevel::Info => theme::WHITE,
                };
                text(e.format_line()).size(13).color(color).into()
            })
            .collect()
    } else {
        visible_entries
            .into_iter()
            .map(|e| {
                let color = match e.level {
                    LogLevel::Error => theme::RED,
                    LogLevel::Warn => theme::YELLOW,
                    LogLevel::Debug => theme::MUTED,
                    LogLevel::Info => theme::WHITE,
                };
                text(e.format_line()).size(13).color(color).into()
            })
            .collect()
    };

    let lines = column(rendered_items).spacing(4);

    column![
        heading("Monitor"),
        muted("Live daemon and pad diagnostic events with real-time filtering."),
        filter_row,
        action_row,
        card(scrollable(lines).height(Length::Fill)).height(Length::Fill),
    ]
    .spacing(12)
    .into()
}
