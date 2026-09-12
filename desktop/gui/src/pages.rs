//! Dashboard, settings, device and monitor pages.

use crate::theme::{self, caption, heading, muted};
use crate::{App, Message};
use iced::widget::{button, column, container, row, scrollable, slider, text, text_input, Space};
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

    column![
        heading("Settings"),
        row![keys, display].spacing(14),
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
            line("Counter generation", app.counters.counter_generation.to_string()),
            line("Last sync", app.last_sync_time.clone().unwrap_or_else(|| "Never".into())),
        ]
        .spacing(10),
    );

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
                button(text("Reset lifetime counters").size(14)).padding([10, 18]).style(theme::danger).on_press(Message::ResetCounters),
                muted("Sets K1 and K2 back to zero. This cannot be undone.").size(13),
            ]
            .spacing(14)
            .align_y(Alignment::Center),
        ]
        .spacing(14),
    );

    let backup = card(
        column![
            text("Backup").size(18).font(theme::FONT_BOLD),
            muted("Export or restore settings and counters from a terminal:"),
            text("osupadctl export backup.json").size(14).color(theme::CYAN),
            text("osupadctl import backup.json").size(14).color(theme::CYAN),
        ]
        .spacing(8),
    );

    scrollable(column![heading("Device"), details, actions, backup].spacing(14)).into()
}

// ---- monitor --------------------------------------------------------------------------------

pub fn monitor(app: &App) -> Element<'_, Message> {
    let lines = column(app.logs.iter().rev().map(|line| text(line).size(13).into())).spacing(4);
    column![
        heading("Monitor"),
        muted("Daemon and pad events, newest first."),
        card(scrollable(lines).height(Length::Fill)).height(Length::Fill),
    ]
    .spacing(14)
    .into()
}
