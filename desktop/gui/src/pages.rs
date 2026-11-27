//! The Dashboard and Pad pages, the log list Troubleshooting shows, and the
//! Hall Effect calibration that opens inside Keys.

use crate::theme::{self, caption, heading, muted};
use crate::widgets::{card, help, line, scrolling, section};
use crate::{App, Message, Target};
use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Alignment, Color, Element, Length};
use opad_model::ui_source::{self as src, SourceValue};
use opad_model::RuntimeMode;

pub(crate) fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
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

/// What the pad is doing, in the words a player would use
pub fn mode_words(mode: RuntimeMode) -> &'static str {
    match mode {
        RuntimeMode::Idle => "Ready",
        RuntimeMode::Playing => "In a map",
        RuntimeMode::Cooldown => "Map just ended",
        RuntimeMode::Sync => "Syncing",
    }
}

/// Presses on the keys the player plays with: what "total" means everywhere
/// it is shown (the Dashboard, the tray)
pub fn total_presses(app: &App) -> u64 {
    let c = &app.counters;
    [
        c.lifetime_key1,
        c.lifetime_key2,
        app.key3.map_or(0, |k| k.lifetime),
    ]
    .iter()
    .enumerate()
    .filter(|(k, _)| app.config.is_game_key(*k as u8 + 1))
    .map(|(_, v)| v)
    .sum()
}

/// The typical key speed in the unit a player reads: "0.03 ms"
pub fn key_speed(us: u32) -> String {
    format!("{:.2} ms", us as f64 / 1000.0)
}

// ---- dashboard ------------------------------------------------------------------------------

pub fn dashboard(app: &App) -> Element<'_, Message> {
    let title = row![
        heading("Dashboard"),
        Space::new().width(Length::Fill),
        if app.device_connected {
            muted(mode_words(app.mode)).size(14)
        } else {
            muted("")
        },
    ]
    .align_y(Alignment::Center);

    // No pad: say so, where the counters would be
    if !app.device_connected {
        return scrolling(
            column![
                title,
                card(
                    column![
                        text("No pad found").size(24).font(theme::FONT_BOLD),
                        muted("Plug it in with its USB cable.").size(15),
                    ]
                    .spacing(8)
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
                )
                .padding(40),
                osu_cards(app),
            ]
            .spacing(14),
        )
        .into();
    }

    let c = &app.counters;
    let dark_70 = Color {
        a: 0.7,
        ..theme::BG
    };
    let total = card(
        column![
            caption("TOTAL PRESSES"),
            text(grouped(total_presses(app)))
                .size(64)
                .font(theme::FONT_BOLD),
        ]
        .spacing(2)
        .align_x(Alignment::Center)
        .width(Length::Fill),
    );

    let k1 = container(
        column![
            text(format!("Key 1 · {}", app.config.key1_char()))
                .size(14)
                .color(dark_70),
            text(grouped(c.lifetime_key1))
                .size(40)
                .font(theme::FONT_BOLD)
                .color(theme::BG),
            text(format!("{} this map", grouped(c.map_key1 as u64)))
                .size(13)
                .color(dark_70),
        ]
        .spacing(4)
        .align_x(Alignment::Center)
        .width(Length::Fill),
    )
    .padding(18)
    .width(Length::Fill)
    .style(theme::pink_card);

    let plain_key = |label: String, lifetime: u64, map: u64| {
        container(
            column![
                caption(label).size(14),
                text(grouped(lifetime)).size(40).font(theme::FONT_BOLD),
                muted(format!("{} this map", grouped(map))).size(13),
            ]
            .spacing(4)
            .align_x(Alignment::Center)
            .width(Length::Fill),
        )
        .padding(18)
        .width(Length::Fill)
        .style(theme::outlined_card)
    };
    let mut key_cards = row![
        k1,
        plain_key(
            format!("Key 2 · {}", app.config.key2_char()),
            c.lifetime_key2,
            c.map_key2 as u64
        )
    ]
    .spacing(14);
    if let Some(k3) = app.key3 {
        let letter = if app.config.key3_hid_usage == 0 {
            "sends nothing".to_string()
        } else {
            app.config.key3_char()
        };
        key_cards = key_cards.push(plain_key(
            format!("Key 3 · {letter}"),
            k3.lifetime,
            k3.map as u64,
        ));
    }

    // One figure a player can read; the percentiles are on Stats
    let speed = match app.latency {
        Some(l) if l.samples > 0 => row![
            text("Key speed").size(16).font(theme::FONT_BOLD),
            text(key_speed(l.p50_us))
                .size(22)
                .font(theme::FONT_BOLD)
                .color(theme::PINK),
            muted("from the switch to the computer, typically").size(14),
            Space::new().width(Length::Fill),
            button(text("Details").size(13))
                .style(theme::secondary)
                .on_press(Message::Navigate(crate::Page::Stats)),
        ],
        _ => row![
            text("Key speed").size(16).font(theme::FONT_BOLD),
            muted("Press a key to measure it.").size(14),
        ],
    }
    .spacing(12)
    .align_y(Alignment::Center);

    scrolling(column![title, total, key_cards, osu_cards(app), card(speed)].spacing(14)).into()
}

/// The osu! profile and what is playing, from tosu
fn osu_cards(app: &App) -> Element<'_, Message> {
    let profile: Element<'_, Message> = match text_value(app, src::PROFILE_NAME) {
        Some(name) => column![
            caption("OSU! PROFILE"),
            text(name).size(24).font(theme::FONT_BOLD),
            row![
                text(
                    number_value(app, src::PROFILE_RANK)
                        .map_or("#-".into(), |r| format!("#{}", grouped(r as u64)))
                )
                .size(15)
                .font(theme::FONT_BOLD)
                .color(theme::MUTED),
                text("·").size(15).color(theme::MUTED),
                text(
                    number_value(app, src::PROFILE_PP)
                        .map_or("-".into(), |p| format!("{:.0}pp", p))
                )
                .size(15)
                .font(theme::FONT_BOLD)
                .color(theme::PINK),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            muted(format!(
                "{} accuracy · {} plays",
                number_value(app, src::PROFILE_ACCURACY)
                    .map_or("-".into(), |a| format!("{:.2}%", a)),
                number_value(app, src::PROFILE_PLAYCOUNT).map_or("-".into(), |p| grouped(p as u64)),
            ))
            .size(13),
        ]
        .spacing(6)
        .into(),
        None => column![
            caption("OSU! PROFILE"),
            if app.tosu_connected {
                Element::from(muted("Open osu! to see your profile.").size(14))
            } else {
                row![
                    muted("osu! is not connected.").size(14),
                    button(text("osu! connection").size(13))
                        .style(theme::secondary)
                        .on_press(Message::Open(Target::OSU)),
                ]
                .spacing(10)
                .align_y(Alignment::Center)
                .into()
            },
        ]
        .spacing(6)
        .into(),
    };

    let now: Element<'_, Message> = match text_value(app, src::MAP_TITLE) {
        Some(title) => {
            let mut info = column![
                caption("NOW PLAYING"),
                text(title).size(20).font(theme::FONT_BOLD),
                muted(text_value(app, src::MAP_ARTIST).unwrap_or("")).size(14),
                row![
                    text(
                        text_value(app, src::MAP_DIFFICULTY)
                            .map_or(String::new(), |d| format!("[{}]", d))
                    )
                    .size(14)
                    .color(theme::CYAN),
                    text(
                        number_value(app, src::MAP_STARS)
                            .map_or(String::new(), |s| format!("{:.2} stars", s))
                    )
                    .size(14)
                    .color(theme::YELLOW),
                ]
                .spacing(10),
            ]
            .spacing(6);
            if app.mode == RuntimeMode::Playing {
                if let Some(pp) = number_value(app, src::PLAY_PP) {
                    info = info.push(
                        text(format!("{:.0}pp", pp))
                            .size(32)
                            .font(theme::FONT_BOLD)
                            .color(theme::PINK),
                    );
                }
            }
            info.into()
        }
        None => column![
            caption("NOW PLAYING"),
            muted("Nothing selected in osu!.").size(14)
        ]
        .spacing(6)
        .into(),
    };
    row![card(profile), card(now)].spacing(14).into()
}

// ---- pad ------------------------------------------------------------------------------------

/// A timestamp from the daemon (RFC 3339) as a person reads it
fn when(stamp: Option<&str>) -> String {
    let Some(stamp) = stamp else {
        return "Never".to_string();
    };
    chrono::DateTime::parse_from_rfc3339(stamp)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%-d %b %Y, %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| stamp.to_string())
}

pub fn pad(app: &App) -> Element<'_, Message> {
    let info = app.device_info.as_ref();
    let known = |value: Option<&str>| value.filter(|s| !s.is_empty()).unwrap_or("-").to_string();
    let playing = matches!(app.mode, RuntimeMode::Playing | RuntimeMode::Cooldown);

    // ---- this pad ----
    let firmware = row![
        muted("Firmware").size(14).width(180),
        text(known(info.map(|i| i.firmware_version.as_str()))).size(14),
        match app
            .firmware_offer
            .as_ref()
            .and_then(|o| o.available.as_ref())
        {
            Some(v) if app.device_connected => Element::from(
                button(text(format!("Update to {v}")).size(13))
                    .padding([3, 10])
                    .style(theme::primary)
                    .on_press(Message::OpenUpdates)
            ),
            _ => button(text("Updates").size(13))
                .padding([3, 10])
                .style(theme::secondary)
                .on_press(Message::OpenUpdates)
                .into(),
        },
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let mut this_pad = column![
        line("Pad ID", known(info.map(|i| i.device_id.as_str()))),
        line("Board", known(info.map(|i| i.board_profile.as_str()))),
        firmware,
        line(
            "Doing now",
            if app.device_connected {
                mode_words(app.mode)
            } else {
                "Not connected"
            }
        ),
        line("Last sync", when(app.last_sync_time.as_deref())),
    ]
    .spacing(10);
    if let Some(error) = &app.last_sync_error {
        this_pad = this_pad.push(
            text(format!("The last sync did not work: {error}"))
                .size(13)
                .color(theme::YELLOW),
        );
    }
    this_pad = this_pad.push(
        button(
            text(if app.pad_details {
                "Hide details"
            } else {
                "Details"
            })
            .size(13),
        )
        .style(theme::secondary)
        .on_press(Message::TogglePadDetails),
    );
    if app.pad_details {
        this_pad = this_pad
            .push(line(
                "Running slot",
                known(
                    info.and_then(|i| i.running_partition.as_deref())
                        .or_else(|| {
                            app.firmware_offer
                                .as_ref()
                                .and_then(|o| o.running_partition.as_deref())
                        }),
                ),
            ))
            .push(line(
                "Counters source",
                format!("{:?}", app.counters_source),
            ))
            .push(line(
                "Counter generation",
                app.counters.counter_generation.to_string(),
            ));
    }

    // ---- counters ----
    let pc = app.pc_counters.as_ref();
    let on_pad = app.esp_counters.as_ref().unwrap_or(&app.counters);
    let pc_gen = pc.map_or(0, |c| c.counter_generation);
    // Key 3 is counted by the pad and reported as one figure
    let key3 = app.key3.map(|k| k.lifetime);
    let rows: Vec<(&str, u64, u64)> = [
        Some((
            "Key 1",
            pc.map_or(0, |c| c.lifetime_key1),
            on_pad.lifetime_key1,
        )),
        Some((
            "Key 2",
            pc.map_or(0, |c| c.lifetime_key2),
            on_pad.lifetime_key2,
        )),
        key3.map(|k| ("Key 3", k, k)),
    ]
    .into_iter()
    .flatten()
    .collect();
    let differ = app.device_connected
        && (rows.iter().any(|(_, a, b)| a != b) || pc_gen != on_pad.counter_generation);

    let cell = |value: String, warn: bool| {
        text(value)
            .size(14)
            .color(if warn { theme::YELLOW } else { theme::WHITE })
            .width(Length::FillPortion(3))
    };
    let mut table = column![row![
        muted("").size(13).width(Length::FillPortion(2)),
        muted("On this PC").size(13).width(Length::FillPortion(3)),
        muted("On the pad").size(13).width(Length::FillPortion(3)),
    ]]
    .spacing(8);
    for (label, on_pc, on_pad_value) in &rows {
        let warn = app.device_connected && on_pc != on_pad_value;
        table = table.push(row![
            text(*label).size(14).width(Length::FillPortion(2)),
            cell(grouped(*on_pc), warn),
            cell(
                if app.device_connected {
                    grouped(*on_pad_value)
                } else {
                    "-".into()
                },
                warn
            ),
        ]);
    }

    let state = if !app.device_connected {
        text("No pad connected").size(13).color(theme::MUTED)
    } else if differ {
        text("The PC and the pad differ")
            .size(13)
            .font(theme::FONT_BOLD)
            .color(theme::YELLOW)
    } else {
        text("The PC and the pad agree")
            .size(13)
            .color(theme::GREEN)
    };
    let usable = app.device_connected && !playing;
    let mut counters = column![
        row![
            help("Every press is counted on the pad and kept on this PC too."),
            Space::new().width(Length::Fill),
            state
        ]
        .align_y(Alignment::Center),
        container(table).padding(14).style(theme::outlined_card),
        row![
            button(text("Sync now").size(14))
                .padding([9, 18])
                .style(theme::secondary)
                .on_press_maybe(usable.then_some(Message::Sync)),
            button(text("Reset lifetime counters").size(14))
                .padding([9, 18])
                .style(theme::danger)
                .on_press_maybe(usable.then_some(Message::PromptResetCounters)),
        ]
        .spacing(10),
    ]
    .spacing(12);
    if differ && !playing {
        counters = counters.push(
            row![
                button(text("Restore pad from PC").size(14))
                    .padding([9, 18])
                    .style(theme::secondary)
                    .on_press(Message::PromptRestoreDeviceFromPc),
                button(text("Import PC from pad").size(14))
                    .padding([9, 18])
                    .style(theme::secondary)
                    .on_press(Message::PromptImportPcFromDevice),
            ]
            .spacing(10),
        );
    }
    if app.device_connected && playing {
        counters = counters.push(help("Not during a map: these wait until it has ended."));
    } else if !app.device_connected {
        counters = counters.push(help("Connect your pad to sync or reset its counters."));
    }

    // ---- backup ----
    let backup = column![
        help("Your lifetime counters and the pad's settings, in a file."),
        row![
            button(text("Export backup").size(14))
                .padding([9, 18])
                .style(theme::secondary)
                .on_press_maybe((!playing).then_some(Message::ExportBackup)),
            button(text("Import backup").size(14))
                .padding([9, 18])
                .style(theme::secondary)
                .on_press_maybe((!playing).then_some(Message::StartImportBackup)),
        ]
        .spacing(10),
        line("Last automatic backup", when(app.last_backup.as_deref())),
    ]
    .spacing(12);

    scrolling(
        column![
            heading("Pad"),
            section("This pad", this_pad),
            section("Counters", counters),
            section("Backup", backup),
        ]
        .spacing(14),
    )
    .into()
}

// ---- logs -----------------------------------------------------------------------------------

fn filter_btn<'a>(label: &'a str, selected: bool, msg: Message) -> Element<'a, Message> {
    button(text(label).size(13))
        .padding([5, 10])
        .style(if selected {
            theme::primary
        } else {
            theme::secondary
        })
        .on_press(msg)
        .into()
}

/// Troubleshooting's Logs tab: what the service, this app, tosu and the pad wrote
pub fn logs(app: &App) -> Element<'_, Message> {
    use opad_model::{LogLevel, LogSource};

    let severity_filters = row![
        caption("LEVEL"),
        filter_btn(
            "All",
            app.log_filter_level.is_none(),
            Message::FilterLogLevel(None)
        ),
        filter_btn(
            "Debug",
            app.log_filter_level == Some(LogLevel::Debug),
            Message::FilterLogLevel(Some(LogLevel::Debug))
        ),
        filter_btn(
            "Info",
            app.log_filter_level == Some(LogLevel::Info),
            Message::FilterLogLevel(Some(LogLevel::Info))
        ),
        filter_btn(
            "Warning",
            app.log_filter_level == Some(LogLevel::Warn),
            Message::FilterLogLevel(Some(LogLevel::Warn))
        ),
        filter_btn(
            "Error",
            app.log_filter_level == Some(LogLevel::Error),
            Message::FilterLogLevel(Some(LogLevel::Error))
        ),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let source_filters = row![
        caption("SOURCE"),
        filter_btn(
            "All",
            app.log_filter_source.is_none(),
            Message::FilterLogSource(None)
        ),
        filter_btn(
            "Service",
            app.log_filter_source == Some(LogSource::Host),
            Message::FilterLogSource(Some(LogSource::Host))
        ),
        filter_btn(
            "App",
            app.log_filter_source == Some(LogSource::Program),
            Message::FilterLogSource(Some(LogSource::Program))
        ),
        filter_btn(
            "tosu",
            app.log_filter_source == Some(LogSource::Tosu),
            Message::FilterLogSource(Some(LogSource::Tosu))
        ),
        filter_btn(
            "Pad",
            app.log_filter_source == Some(LogSource::Esp),
            Message::FilterLogSource(Some(LogSource::Esp))
        ),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let filter_row = row![severity_filters, Space::new().width(16), source_filters].spacing(10);

    let visible_entries: Vec<_> = app
        .logs
        .iter()
        .filter(|e| app.log_visible_after_clear(e))
        .filter(|e| app.log_filter_level.is_none_or(|l| e.level >= l))
        .filter(|e| app.log_filter_source.is_none_or(|s| e.source == s))
        .collect();

    let action_row = row![
        button(text("Clear").size(13))
            .padding([5, 12])
            .style(theme::secondary)
            .on_press(Message::ClearLogs),
        button(text("Copy").size(13))
            .padding([5, 12])
            .style(theme::secondary)
            .on_press(Message::CopyLogs),
        button(text("Save log").size(13))
            .padding([5, 12])
            .style(theme::secondary)
            .on_press(Message::SaveLogs),
        button(
            text(if app.log_auto_scroll {
                "Newest first"
            } else {
                "Oldest first"
            })
            .size(13)
        )
        .padding([5, 12])
        .style(theme::secondary)
        .on_press(Message::ToggleAutoScroll),
        Space::new().width(Length::Fill),
        caption(format!(
            "{} entries, kept for 24 hours",
            visible_entries.len()
        )),
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
        filter_row,
        action_row,
        card(scrollable(lines).height(Length::Fixed(460.0))),
    ]
    .spacing(12)
    .into()
}

pub fn calibration(app: &App) -> Element<'_, Message> {
    let mut content = column![
        row![
            heading("Calibrate the keys"),
            Space::new().width(Length::Fill),
            button(text("Back to Keys").size(13))
                .style(theme::secondary)
                .on_press(Message::CloseCalibration),
        ]
        .align_y(Alignment::Center),
        muted("Measures where each Hall Effect key rests and how far it travels.").size(14),
    ]
    .spacing(14);

    if !app.device_connected {
        content = content.push(card(
            column![
                text("No pad connected").size(16).font(theme::FONT_BOLD),
                muted("Connect your pad to calibrate its keys.").size(14),
            ]
            .spacing(14),
        ));
        return scrolling(content).into();
    }

    if !app.device_info.as_ref().is_some_and(|i| i.he_calibration) {
        return scrolling(content.push(card(text(
            "This pad's keys are not Hall Effect: there is nothing to calibrate.",
        ))))
        .into();
    }

    let progress = app.cal_progress.as_ref();
    let stage = progress
        .map(|p| p.stage)
        .unwrap_or(opad_model::HeCalStage::None);

    let progress_boxes = |presses_done: u32| {
        row((1..=5).map(|i| {
            let filled = i <= presses_done;
            container(
                text(format!("{i}"))
                    .size(15)
                    .font(theme::FONT_BOLD)
                    .color(if filled {
                        Color::WHITE
                    } else {
                        Color::from_rgb8(150, 150, 150)
                    }),
            )
            .width(44)
            .height(44)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(move |_| container::Style {
                background: Some(if filled {
                    Color::from_rgb8(230, 70, 130).into()
                } else {
                    Color::from_rgb8(45, 45, 55).into()
                }),
                border: iced::border::rounded(6),
                ..Default::default()
            })
            .into()
        }))
        .spacing(10)
    };

    let body = match stage {
        opad_model::HeCalStage::None => {
            let mut col = column![
                text("Three short steps").size(16).font(theme::FONT_BOLD),
                muted(
                    "The pad measures each key at rest and fully pressed, and sets where the \
                     keys press from that.",
                )
                .size(14),
                {
                    let three = app.device_info.as_ref().is_some_and(|i| i.key_count >= 3);
                    let mut steps = column![
                        text(if three {
                            "1. Take your fingers off all three keys."
                        } else {
                            "1. Take your fingers off both keys."
                        })
                        .size(14),
                        text("2. Press Key 1 all the way down 5 times, slowly.").size(14),
                        text("3. Press Key 2 all the way down 5 times, slowly.").size(14),
                    ]
                    .spacing(6);
                    if three {
                        steps = steps.push(
                            text("4. Press Key 3 all the way down 5 times, slowly.").size(14),
                        );
                    }
                    steps
                },
            ]
            .spacing(14);

            if let Some(msg) = &app.cal_status_msg {
                col = col.push(text(msg).size(13).color(Color::from_rgb8(230, 180, 50)));
            }

            let start_btn = button(
                text(if app.cal_starting {
                    "Starting…"
                } else {
                    "Start"
                })
                .size(14)
                .font(theme::FONT_BOLD),
            )
            .padding([8, 20])
            .style(theme::primary);

            let start_btn = if app.cal_starting {
                start_btn
            } else {
                start_btn.on_press(Message::StartCalibration)
            };

            col = col.push(
                row![
                    start_btn,
                    Space::new().width(12),
                    button(text("Cancel").size(13))
                        .style(theme::secondary)
                        .on_press(Message::CloseCalibration),
                ]
                .align_y(Alignment::Center),
            );

            card(col)
        }
        opad_model::HeCalStage::Rest => card(
            column![
                text("Step 1: keys at rest").size(16).font(theme::FONT_BOLD),
                muted("Take your fingers off the keys and do not touch the pad.").size(14),
                text("Measuring…").size(14),
                if let Some(p) = progress {
                    if p.raw_value > 0 {
                        text(format!("Sensor reading: {}", p.raw_value))
                            .size(12)
                            .color(theme::MUTED)
                    } else {
                        text("")
                    }
                } else {
                    text("")
                },
                row![button(text("Cancel").size(13))
                    .style(theme::danger)
                    .on_press(Message::CancelCalibration),],
            ]
            .spacing(14),
        ),
        opad_model::HeCalStage::Key1 => card(
            column![
                text("Step 2: press Key 1 5 times")
                    .size(16)
                    .font(theme::FONT_BOLD),
                muted("Press Key 1 all the way down and let go, slowly, 5 times.").size(14),
                progress_boxes(progress.map(|p| p.presses_done).unwrap_or(0)),
                text(format!(
                    "{} of 5",
                    progress.map(|p| p.presses_done).unwrap_or(0)
                ))
                .size(14),
                if let Some(p) = progress {
                    if p.raw_value > 0 {
                        text(format!("Sensor reading: {}", p.raw_value))
                            .size(12)
                            .color(theme::MUTED)
                    } else {
                        text("")
                    }
                } else {
                    text("")
                },
                row![button(text("Cancel").size(13))
                    .style(theme::danger)
                    .on_press(Message::CancelCalibration),],
            ]
            .spacing(14),
        ),
        opad_model::HeCalStage::Key2 => card(
            column![
                text("Step 3: press Key 2 5 times")
                    .size(16)
                    .font(theme::FONT_BOLD),
                muted("Press Key 2 all the way down and let go, slowly, 5 times.").size(14),
                progress_boxes(progress.map(|p| p.presses_done).unwrap_or(0)),
                text(format!(
                    "{} of 5",
                    progress.map(|p| p.presses_done).unwrap_or(0)
                ))
                .size(14),
                if let Some(p) = progress {
                    if p.raw_value > 0 {
                        text(format!("Sensor reading: {}", p.raw_value))
                            .size(12)
                            .color(theme::MUTED)
                    } else {
                        text("")
                    }
                } else {
                    text("")
                },
                row![button(text("Cancel").size(13))
                    .style(theme::danger)
                    .on_press(Message::CancelCalibration),],
            ]
            .spacing(14),
        ),
        opad_model::HeCalStage::Key3 => card(
            column![
                text("Step 4: press Key 3 5 times")
                    .size(16)
                    .font(theme::FONT_BOLD),
                muted("Press Key 3 all the way down and let go, slowly, 5 times.").size(14),
                progress_boxes(progress.map(|p| p.presses_done).unwrap_or(0)),
                text(format!(
                    "{} of 5",
                    progress.map(|p| p.presses_done).unwrap_or(0)
                ))
                .size(14),
                if let Some(p) = progress {
                    if p.raw_value > 0 {
                        text(format!("Sensor reading: {}", p.raw_value))
                            .size(12)
                            .color(theme::MUTED)
                    } else {
                        text("")
                    }
                } else {
                    text("")
                },
                row![button(text("Cancel").size(13))
                    .style(theme::danger)
                    .on_press(Message::CancelCalibration),],
            ]
            .spacing(14),
        ),
        opad_model::HeCalStage::Success => card(
            column![
                text("Calibration done")
                    .size(18)
                    .font(theme::FONT_BOLD)
                    .color(theme::GREEN),
                muted("Every key is calibrated and saved on the pad. The keys work now.").size(14),
                if let Some(p) = progress {
                    if p.key1_full > 0 && p.key2_full > 0 {
                        let key3 = if p.key3_full > 0 {
                            format!("   ·   Key 3: rest {} → full {}", p.key3_rest, p.key3_full)
                        } else {
                            String::new()
                        };
                        text(format!(
                            "Key 1: rest {} → full {}   ·   Key 2: rest {} → full {}{}",
                            p.key1_rest, p.key1_full, p.key2_rest, p.key2_full, key3
                        ))
                        .size(13)
                    } else {
                        text("")
                    }
                } else {
                    text("")
                },
                row![
                    button(text("Back to Keys").size(14).font(theme::FONT_BOLD))
                        .padding([8, 18])
                        .style(theme::primary)
                        .on_press(Message::CloseCalibration),
                    Space::new().width(12),
                    button(text("Calibrate again").size(13))
                        .style(theme::secondary)
                        .on_press(Message::StartCalibration),
                ]
                .align_y(Alignment::Center),
            ]
            .spacing(14),
        ),
        opad_model::HeCalStage::Failed => {
            let err_text = if let Some(p) = progress {
                match p.error {
                    opad_model::HeCalError::RestNotStill => {
                        "The keys moved while they were measured at rest. Keep your fingers off them and try again."
                    }
                    opad_model::HeCalError::RestOutOfRange => {
                        "A key reads far from where a key at rest should. Check that its switch is seated and its magnet is the right way round."
                    }
                    opad_model::HeCalError::RestPressed => {
                        "A key was pressed while the keys were measured at rest. Let go of them and try again."
                    }
                    opad_model::HeCalError::SignalNotRising => {
                        "A key did not read as going down when pressed. Check that its switch is seated and its magnet is the right way round."
                    }
                    opad_model::HeCalError::SpanTooSmall => {
                        "A key did not travel far enough. Press it all the way down each time."
                    }
                    opad_model::HeCalError::PeaksInconsistent => {
                        "The 5 presses did not go equally deep. Press the key all the way down each time."
                    }
                    opad_model::HeCalError::TooFewPeaks => "Fewer than 5 presses were seen. Try again, a little slower.",
                    opad_model::HeCalError::TooManyPeaks => "More than 5 presses were seen. Try again, one clean press at a time.",
                    opad_model::HeCalError::Timeout => "The pad waited too long for a press. Try again.",
                    _ => {
                        if !p.error_message.is_empty() {
                            &p.error_message
                        } else {
                            "The pad did not say why. Try again."
                        }
                    }
                }
            } else {
                "The pad did not say why. Try again."
            };

            card(
                column![
                    text("Calibration did not finish")
                        .size(18)
                        .font(theme::FONT_BOLD)
                        .color(theme::RED),
                    muted(err_text).size(14),
                    row![
                        button(text("Try again").size(14).font(theme::FONT_BOLD))
                            .padding([8, 18])
                            .style(theme::primary)
                            .on_press(Message::StartCalibration),
                        Space::new().width(12),
                        button(text("Back to Keys").size(13))
                            .style(theme::secondary)
                            .on_press(Message::CloseCalibration),
                    ]
                    .align_y(Alignment::Center),
                ]
                .spacing(14),
            )
        }
    };

    content = content.push(body);
    scrolling(content).into()
}
