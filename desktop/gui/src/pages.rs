//! Dashboard, settings, device and monitor pages.

use crate::theme::{self, caption, heading, muted};
use crate::{App, Message};
use iced::widget::{
    button, checkbox, column, container, pick_list, row, scrollable, slider, text, text_input,
    Space,
};
use iced::{Alignment, Color, Element, Length};
use opad_model::ui_source::{self as src, SourceValue};
use opad_model::{key_pin, KeyPin, KEY_PINS};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Keypad,
    Display,
    Updates,
    AboutTosu,
    Diagnostics,
}

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

fn card<'a>(content: impl Into<Element<'a, Message>>) -> container::Container<'a, Message> {
    container(content)
        .padding(20)
        .width(Length::Fill)
        .style(theme::card)
}

fn stat<'a>(label: &'a str, value: String) -> Element<'a, Message> {
    column![caption(label), text(value).size(20).font(theme::FONT_BOLD)]
        .spacing(4)
        .into()
}

// ---- dashboard ------------------------------------------------------------------------------

pub fn dashboard(app: &App) -> Element<'_, Message> {
    let c = &app.counters;
    let dark_70 = Color {
        a: 0.7,
        ..theme::BG
    };

    let total = card(
        column![
            caption("TOTAL PRESSES"),
            text(grouped(c.total_lifetime_presses()))
                .size(64)
                .font(theme::FONT_BOLD),
        ]
        .spacing(2)
        .align_x(Alignment::Center)
        .width(Length::Fill),
    );

    let k1 = container(
        column![
            text(format!("K1 · {}", app.config.key1_char()))
                .size(14)
                .color(dark_70),
            text(grouped(c.lifetime_key1))
                .size(40)
                .font(theme::FONT_BOLD)
                .color(theme::BG),
            text(format!("this map {}", c.map_key1))
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

    let k2 = container(
        column![
            caption(format!("K2 · {}", app.config.key2_char())).size(14),
            text(grouped(c.lifetime_key2))
                .size(40)
                .font(theme::FONT_BOLD),
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
            muted("Open osu! with tosu running to see your profile.")
        ]
        .spacing(6)
        .into(),
    };

    let now: Element<'_, Message> = match text_value(app, src::MAP_TITLE) {
        Some(title) => {
            let mut info = column![
                caption(
                    text_value(app, src::GAME_STATE)
                        .unwrap_or("NOW")
                        .to_uppercase()
                ),
                text(title).size(20).font(theme::FONT_BOLD),
                muted(text_value(app, src::MAP_ARTIST).unwrap_or("")),
                row![
                    text(
                        text_value(app, src::MAP_DIFFICULTY)
                            .map_or(String::new(), |d| format!("[{}]", d))
                    )
                    .size(14)
                    .color(theme::CYAN),
                    text(
                        number_value(app, src::MAP_STARS)
                            .map_or(String::new(), |s| format!("{:.2} *", s))
                    )
                    .size(14)
                    .color(theme::YELLOW),
                ]
                .spacing(10),
            ]
            .spacing(6);
            if app.mode == opad_model::RuntimeMode::Playing {
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
        None => column![caption("NOW"), muted("Nothing selected in osu!.")]
            .spacing(6)
            .into(),
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
        let mut input = text_input("Z", value).size(22).width(80).padding(10);
        if app.device_connected {
            input = input.on_input(on_input);
        }
        column![caption(label), input].spacing(6)
    };

    // Only supported header pins, minus the one the other key uses
    let pin_select = |label: &'static str,
                      key_id: u32,
                      gpio: u32,
                      other: u32,
                      on_select: fn(KeyPin) -> Message| {
        let options: Vec<KeyPin> = KEY_PINS
            .iter()
            .copied()
            .filter(|p| p.gpio != other)
            .collect();

        let detect_btn = if app.detecting_pin == Some(key_id) {
            button(text("Detecting... (click to cancel)").size(12))
                .style(theme::primary)
                .on_press(Message::CancelDetectPin)
        } else if app.device_connected {
            button(text("Auto-Detect").size(12))
                .style(theme::secondary)
                .on_press(Message::StartDetectPin(key_id))
        } else {
            button(text("Auto-Detect").size(12)).style(theme::secondary)
        };

        let picker_element: Element<'_, Message> = if app.device_connected {
            pick_list(options, key_pin(gpio), on_select)
                .placeholder(format!("GPIO{} (unsupported)", gpio))
                .width(Length::Fill)
                .padding(10)
                .into()
        } else {
            text_input(
                "",
                &key_pin(gpio).map_or(format!("GPIO{} (disconnected)", gpio), |p| {
                    format!("{} (disconnected)", p)
                }),
            )
            .size(13)
            .width(Length::Fill)
            .padding(10)
            .into()
        };

        column![
            row![caption(label), Space::new().width(Length::Fill), detect_btn,]
                .align_y(Alignment::Center),
            picker_element,
        ]
        .spacing(6)
        .width(Length::FillPortion(1))
    };

    let keys_header = if app.device_connected {
        row![text("Keys").size(18).font(theme::FONT_BOLD)].align_y(Alignment::Center)
    } else {
        row![
            text("Keys").size(18).font(theme::FONT_BOLD),
            Space::new().width(8),
            muted("(device disconnected)").size(12),
        ]
        .align_y(Alignment::Center)
    };

    let keys = card(
        column![
            keys_header,
            row![
                key_input("KEY 1", &app.k1_input, Message::Key1),
                key_input("KEY 2", &app.k2_input, Message::Key2)
            ]
            .spacing(24),
            column![
                row![
                    pin_select("KEY 1 PIN", 1, app.k1_gpio, app.k2_gpio, Message::Key1Pin),
                    pin_select("KEY 2 PIN", 2, app.k2_gpio, app.k1_gpio, Message::Key2Pin),
                ]
                .spacing(24),
                muted("Header pin each switch is wired to (other leg to GND). Press 'Auto-Detect' and hit the switch to identify the pin automatically.").size(12),
            ]
            .spacing(8),
            column![
                row![
                    caption("DEBOUNCE LOCKOUT"),
                    Space::new().width(Length::Fill),
                    text(format!("{:.1} ms", app.debounce as f32 / 1000.0)).size(14)
                ],
                slider(500..=20000, app.debounce, Message::Debounce).step(500u32),
                muted("Presses within this window after a switch change are ignored.").size(12),
            ]
            .spacing(8),
        ]
        .spacing(18),
    );

    let sleep_label = if app.sleep_seconds.is_multiple_of(60) {
        format!("{} min", app.sleep_seconds / 60)
    } else {
        format!("{} s", app.sleep_seconds)
    };

    let display_header = if app.device_connected {
        row![text("Display").size(18).font(theme::FONT_BOLD)].align_y(Alignment::Center)
    } else {
        row![
            text("Display").size(18).font(theme::FONT_BOLD),
            Space::new().width(8),
            muted("(device disconnected)").size(12),
        ]
        .align_y(Alignment::Center)
    };

    let display = card(
        column![
            display_header,
            column![
                row![
                    caption("BRIGHTNESS"),
                    Space::new().width(Length::Fill),
                    text(format!("{}%", app.brightness)).size(14)
                ],
                slider(10..=100, app.brightness, Message::Brightness),
            ]
            .spacing(8),
            column![
                row![
                    caption("SLEEP AFTER"),
                    Space::new().width(Length::Fill),
                    text(sleep_label).size(14)
                ],
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

    let save_btn = if app.device_connected && app.detecting_pin.is_none() {
        button(text("Save settings").size(15))
            .padding([10, 22])
            .style(theme::primary)
            .on_press(Message::SaveConfig)
    } else {
        button(
            text(if !app.device_connected {
                "Save settings (device not connected)"
            } else {
                "Detecting pin..."
            })
            .size(15),
        )
        .padding([10, 22])
        .style(theme::secondary)
    };

    let tab_btn = |tab: SettingsTab, label: &'static str| {
        button(text(label).size(13).font(theme::FONT))
            .padding([8, 16])
            .style(theme::tab_button(app.settings_tab == tab))
            .on_press(Message::SelectSettingsTab(tab))
    };

    let tab_bar = row![
        tab_btn(SettingsTab::Keypad, "⌨ Keypad"),
        tab_btn(SettingsTab::Display, "🖥 Display & Game"),
        tab_btn(SettingsTab::Updates, "🔄 Updates & Firmware"),
        tab_btn(SettingsTab::AboutTosu, "ℹ About & tosu"),
        tab_btn(SettingsTab::Diagnostics, "🛠 Diagnostics"),
    ]
    .spacing(8);

    let disconnected_warning = if !app.device_connected {
        Some(
            card(
                row![text(
                    "⚠ Device not connected. Connect your OPad to adjust hardware settings."
                )
                .size(14)
                .font(theme::FONT_BOLD)
                .color(theme::YELLOW),]
                .padding([6, 10]),
            )
            .width(Length::Fill),
        )
    } else {
        None
    };

    let mut content = column![
        heading("Settings"),
        muted("Configure keypad switches, display brightness, integrations, updates, and testing tools."),
        tab_bar,
    ]
    .spacing(14);

    match app.settings_tab {
        SettingsTab::Keypad => {
            if let Some(w) = disconnected_warning {
                content = content.push(w);
            }
            content = content.push(keys);
            content = content.push(row![save_btn]);
        }
        SettingsTab::Display => {
            if let Some(w) = disconnected_warning {
                content = content.push(w);
            }
            content = content.push(display);
            content = content.push(advanced);
            content = content.push(row![save_btn]);
        }
        SettingsTab::Updates => {
            content = content.push(row![updates(app), firmware_updates(app)].spacing(14));
        }
        SettingsTab::AboutTosu => {
            content = content.push(tosu_settings(app));
            content = content.push(about_section());
        }
        SettingsTab::Diagnostics => {
            let diag_card = card(
                column![
                    text("Interactive Diagnostics & Testing Mode").size(16).font(theme::FONT_BOLD),
                    muted(
                        "Activate the Diagnostics suite to access interactive test tools built for OPad: \
                         the switch chatter / contact bounce tester, COM-02 protocol recovery check, \
                         screen backlight / color verification, and one-click diagnostic report exporter."
                    ).size(13),
                    checkbox(app.diagnostics_enabled)
                        .label("Enable Diagnostics Menu in Navigation")
                        .on_toggle(Message::ToggleDiagnostics),
                    if app.diagnostics_enabled {
                        row![
                            button(text("Open Diagnostics Menu →").size(14).font(theme::FONT_BOLD))
                                .padding([8, 18])
                                .style(theme::primary)
                                .on_press(Message::Navigate(crate::Page::Diagnostics)),
                            Space::new().width(12),
                            muted("The Diagnostics tab is now accessible from the main left sidebar.").size(12),
                        ]
                        .align_y(Alignment::Center)
                    } else {
                        row![
                            muted("Turn on the toggle above to add 'Diagnostics' to your sidebar navigation.").size(12),
                        ]
                    }
                ]
                .spacing(14),
            );
            content = content.push(diag_card);
        }
    }

    scrollable(content).into()
}

/// §U-0.4: every updater is switchable on its own, and shows what is
/// installed, what is available and when it last managed to check.
fn updates(app: &App) -> Element<'_, Message> {
    use opad_ipc::UpdateComponent;

    let Some(u) = &app.updates else {
        return card(
            column![
                caption("SOFTWARE UPDATES"),
                muted("Waiting for the daemon.").size(12),
            ]
            .spacing(8),
        )
        .width(Length::Fill)
        .into();
    };

    let line = |name: &'static str, c: &opad_ipc::ComponentUpdate, component: UpdateComponent| {
        let installed = c.installed.clone().unwrap_or_else(|| "unknown".into());
        let state = match (&c.available, c.notify_only) {
            // §U-2a: a package manager owns these files, so we report only
            (Some(v), true) => format!("{} available — update through your package manager", v),
            (Some(v), false) => format!("{} available", v),
            (None, _) => "up to date".to_string(),
        };
        let mut r = row![
            column![
                text(format!("{} {}", name, installed)).size(13),
                muted(state).size(11),
            ]
            .spacing(2),
            Space::new().width(Length::Fill),
        ]
        .align_y(Alignment::Center);

        // §U-2: never automatic. A person presses this.
        if c.ready_to_install {
            r = r.push(
                button(text("Install now").size(12))
                    .style(theme::primary)
                    .on_press(Message::InstallUpdate(component)),
            );
            r = r.push(Space::new().width(8));
        }
        r.push(
            checkbox(c.enabled)
                .label("Check automatically")
                .on_toggle(move |v| Message::ToggleUpdater(component, v)),
        )
    };

    let checked = u
        .last_check
        .as_deref()
        .map(|t| format!("Last checked {}", t))
        .unwrap_or_else(|| "Not checked yet".to_string());

    let mut content = column![
        caption("SOFTWARE UPDATES"),
        line("OPad", &u.app, UpdateComponent::App),
        line("tosu", &u.tosu, UpdateComponent::Tosu),
        muted(checked).size(11),
    ]
    .spacing(10);

    if let Some(e) = &u.last_error {
        content = content.push(muted(format!("Last check failed: {}", e)).size(11));
    }

    card(content).width(Length::Fill).into()
}

/// §U-3b: Host-driven firmware updates.
///
/// Visually separate from app and tosu updaters because it is not the same kind of
/// thing: it flashes hardware over USB and temporarily stops the pad being a keyboard.
/// Non-negotiable rules:
/// - Explicit consent every time. Never automatic, never silent, not even opt-in.
///   There is NO "always update firmware" switch.
/// - Blockers are rendered verbatim as whole sentences when non-empty, and disable the button.
fn firmware_updates(app: &App) -> Element<'_, Message> {
    let Some(offer) = &app.firmware_offer else {
        return card(
            column![
                caption("FIRMWARE UPDATE"),
                muted("Waiting for the daemon.").size(12),
            ]
            .spacing(8),
        )
        .width(Length::Fill)
        .into();
    };

    let installed_str = offer.installed.as_deref().unwrap_or("unknown (no pad?)");
    let slot_str = offer
        .running_partition
        .as_deref()
        .unwrap_or("unknown (predates OTA layout)");

    let state_str = match &offer.available {
        Some(v) => format!("{} available", v),
        None => "up to date".to_string(),
    };

    let mut content = column![
        caption("FIRMWARE UPDATE"),
        row![
            column![
                text(format!("Firmware {}", installed_str)).size(13),
                muted(format!("Running slot: {}", slot_str)).size(11),
                muted(state_str).size(11),
            ]
            .spacing(2),
            Space::new().width(Length::Fill),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(10);

    if let Some(notes) = &offer.notes {
        content = content.push(muted(format!("Release notes: {}", notes)).size(11));
    }

    if !offer.blockers.is_empty() {
        for blocker in &offer.blockers {
            content = content.push(text(format!("⚠ {}", blocker)).size(12).color(theme::YELLOW));
        }
    }

    let can_update = offer.available.is_some() && offer.blockers.is_empty();
    let mut update_btn = button(text("Update firmware").size(12)).padding([6, 14]);

    if can_update {
        update_btn = update_btn
            .style(theme::primary)
            .on_press(Message::PromptFirmwareConsent);
    } else {
        update_btn = update_btn.style(theme::secondary);
    }

    content = content.push(
        row![
            update_btn,
            Space::new().width(8),
            muted("Takes explicit consent every time").size(11),
        ]
        .align_y(Alignment::Center),
    );

    content = content.push(
        muted(
            "Firmware updates flash the app partition over USB. \
             The pad stops being a keyboard for ~30 seconds during the update.",
        )
        .size(11),
    );

    card(content).width(Length::Fill).into()
}

fn tosu_settings(app: &App) -> Element<'_, Message> {
    let source_str = crate::tosu_source_status(app.tosu_override_path.as_deref());
    let conn_str = if app.tosu_connected {
        "Connected (streaming telemetry)"
    } else {
        "Not running / Disconnected"
    };

    let override_text = app
        .tosu_override_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "Using default / bundled tosu".to_string());

    let mut actions = row![button(text("Browse external tosu...").size(12))
        .padding([6, 12])
        .style(theme::secondary)
        .on_press(Message::PickTosuPath),]
    .spacing(8);

    if app.tosu_override_path.is_some() {
        actions = actions.push(
            button(text("Use bundled").size(12))
                .padding([6, 12])
                .style(theme::secondary)
                .on_press(Message::ResetTosuPath),
        );
    }

    card(
        column![
            caption("TOSU INTEGRATION"),
            row![muted("Binary source:").width(140), text(source_str).size(13)].spacing(8),
            row![muted("Status:").width(140), text(conn_str).size(13)].spacing(8),
            row![muted("Custom path:").width(140), text(override_text).size(13)].spacing(8),
            actions,
            muted("tosu reads game memory and feeds live map telemetry (PP, accuracy, hit counts) to the pad display.").size(12),
        ]
        .spacing(10),
    )
    .into()
}

fn about_section() -> Element<'static, Message> {
    card(
        column![
            caption("ABOUT & THIRD-PARTY SOFTWARE"),
            row![
                muted("Application:").width(140),
                text(format!("OPad v{}", env!("CARGO_PKG_VERSION"))).size(13)
            ]
            .spacing(8),
            row![
                muted("License:").width(140),
                text("MIT License (GFerreiroS)").size(13)
            ]
            .spacing(8),
            Space::new().height(4),
            column![
                text("Bundled Component: tosu").size(13).font(theme::FONT_BOLD),
                muted("Author: Mikhail Babynichev and the tosu contributors").size(12),
                muted("License: GNU Lesser General Public License v3.0 (LGPL-3.0)").size(12),
                muted("Repository: https://github.com/KotRikD/tosu").size(12),
                muted("Under LGPL-3.0, you may replace this bundled component with your own version via the setting above or $OPAD_TOSU_PATH.").size(11),
            ]
            .spacing(4),
        ]
        .spacing(8),
    )
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
            line(
                "Device ID",
                info.map(|i| i.device_id.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("—")
                    .to_string(),
            ),
            line(
                "Board",
                info.map(|i| i.board_profile.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("—")
                    .to_string(),
            ),
            line(
                "Firmware",
                info.map(|i| i.firmware_version.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("—")
                    .to_string(),
            ),
            line(
                "Running slot",
                info.and_then(|i| i.running_partition.as_deref())
                    .or_else(|| {
                        app.firmware_offer
                            .as_ref()
                            .and_then(|o| o.running_partition.as_deref())
                    })
                    .unwrap_or("—")
                    .to_string(),
            ),
            line("Mode", format!("{:?}", app.mode)),
            line("Counters source", format!("{:?}", app.counters_source)),
            line(
                "tosu",
                format!(
                    "{} ({})",
                    if app.tosu_connected {
                        "Connected"
                    } else {
                        "Disconnected"
                    },
                    crate::tosu_source_status(app.tosu_override_path.as_deref())
                ),
            ),
            line(
                "Counter generation",
                app.counters.counter_generation.to_string()
            ),
            line(
                "Last sync",
                app.last_sync_time.clone().unwrap_or_else(|| "Never".into())
            ),
        ]
        .spacing(10),
    );

    let playing_or_cooldown = matches!(
        app.mode,
        opad_model::RuntimeMode::Playing | opad_model::RuntimeMode::Cooldown
    );

    let pc_k1 = app
        .pc_counters
        .as_ref()
        .map(|c| c.lifetime_key1)
        .unwrap_or(0);
    let pc_k2 = app
        .pc_counters
        .as_ref()
        .map(|c| c.lifetime_key2)
        .unwrap_or(0);
    let pc_total = pc_k1 + pc_k2;
    let pc_gen = app
        .pc_counters
        .as_ref()
        .map(|c| c.counter_generation)
        .unwrap_or(0);

    let esp_k1 = app
        .esp_counters
        .as_ref()
        .map(|c| c.lifetime_key1)
        .unwrap_or(app.counters.lifetime_key1);
    let esp_k2 = app
        .esp_counters
        .as_ref()
        .map(|c| c.lifetime_key2)
        .unwrap_or(app.counters.lifetime_key2);
    let esp_total = esp_k1 + esp_k2;
    let esp_gen = app
        .esp_counters
        .as_ref()
        .map(|c| c.counter_generation)
        .unwrap_or(app.counters.counter_generation);

    let has_mismatch =
        (pc_k1 != esp_k1 || pc_k2 != esp_k2 || pc_gen != esp_gen) && app.device_connected;

    let k1_color = if app.device_connected && pc_k1 != esp_k1 {
        theme::YELLOW
    } else {
        theme::WHITE
    };
    let k2_color = if app.device_connected && pc_k2 != esp_k2 {
        theme::YELLOW
    } else {
        theme::WHITE
    };
    let total_color = if app.device_connected && pc_total != esp_total {
        theme::YELLOW
    } else {
        theme::WHITE
    };
    let gen_color = if app.device_connected && pc_gen != esp_gen {
        theme::YELLOW
    } else {
        theme::WHITE
    };

    let mut counter_col = column![
        row![
            text("Counter Reconciliation State")
                .size(18)
                .font(theme::FONT_BOLD),
            Space::new().width(Length::Fill),
            if has_mismatch {
                text("⚠ MISMATCH DETECTED")
                    .size(12)
                    .font(theme::FONT_BOLD)
                    .color(theme::YELLOW)
            } else if app.device_connected {
                text("✓ SYNCHRONIZED")
                    .size(12)
                    .font(theme::FONT_BOLD)
                    .color(theme::GREEN)
            } else {
                text("PAD DISCONNECTED").size(12).color(theme::MUTED)
            }
        ]
        .align_y(Alignment::Center),
        muted("Hardware pad counters compared against host PC SQLite database."),
        container(
            column![
                row![
                    text("Metric")
                        .size(12)
                        .color(theme::MUTED)
                        .width(Length::FillPortion(2)),
                    text("PC Database")
                        .size(12)
                        .color(theme::MUTED)
                        .width(Length::FillPortion(3)),
                    text("PAD Hardware")
                        .size(12)
                        .color(theme::MUTED)
                        .width(Length::FillPortion(3)),
                ],
                row![
                    text("Generation").size(13).width(Length::FillPortion(2)),
                    text(pc_gen.to_string())
                        .size(13)
                        .color(gen_color)
                        .width(Length::FillPortion(3)),
                    text(if app.device_connected {
                        esp_gen.to_string()
                    } else {
                        "-".into()
                    })
                    .size(13)
                    .color(gen_color)
                    .width(Length::FillPortion(3)),
                ],
                row![
                    text("Key 1 (K1)").size(13).width(Length::FillPortion(2)),
                    text(format!("{} presses", grouped(pc_k1)))
                        .size(13)
                        .color(k1_color)
                        .width(Length::FillPortion(3)),
                    text(if app.device_connected {
                        format!("{} presses", grouped(esp_k1))
                    } else {
                        "-".into()
                    })
                    .size(13)
                    .color(k1_color)
                    .width(Length::FillPortion(3)),
                ],
                row![
                    text("Key 2 (K2)").size(13).width(Length::FillPortion(2)),
                    text(format!("{} presses", grouped(pc_k2)))
                        .size(13)
                        .color(k2_color)
                        .width(Length::FillPortion(3)),
                    text(if app.device_connected {
                        format!("{} presses", grouped(esp_k2))
                    } else {
                        "-".into()
                    })
                    .size(13)
                    .color(k2_color)
                    .width(Length::FillPortion(3)),
                ],
                row![
                    text("Total")
                        .size(13)
                        .font(theme::FONT_BOLD)
                        .width(Length::FillPortion(2)),
                    text(format!("{} presses", grouped(pc_total)))
                        .size(13)
                        .font(theme::FONT_BOLD)
                        .color(total_color)
                        .width(Length::FillPortion(3)),
                    text(if app.device_connected {
                        format!("{} presses", grouped(esp_total))
                    } else {
                        "-".into()
                    })
                    .size(13)
                    .font(theme::FONT_BOLD)
                    .color(total_color)
                    .width(Length::FillPortion(3)),
                ],
            ]
            .spacing(8)
        )
        .padding(12)
        .style(theme::card),
    ]
    .spacing(12);

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
            ]
            .spacing(10),
        );
    }

    let counter_table = card(counter_col);

    let actions = card(
        column![
            text("Actions").size(18).font(theme::FONT_BOLD),
            row![
                button(text("Sync now").size(14))
                    .padding([10, 18])
                    .style(theme::secondary)
                    .on_press(Message::Sync),
                muted("Reconcile counters with the database and set the pad's clock.").size(13),
            ]
            .spacing(14)
            .align_y(Alignment::Center),
            row![
                button(text("Update firmware").size(14))
                    .padding([10, 18])
                    .style(theme::secondary)
                    .on_press(Message::Navigate(crate::Page::Settings)),
                muted("Firmware updates are verified and installed on the Settings page.").size(13),
            ]
            .spacing(14)
            .align_y(Alignment::Center),
            row![
                button(text("Reset lifetime counters").size(14))
                    .padding([10, 18])
                    .style(theme::danger)
                    .on_press(Message::PromptResetCounters),
                muted("Sets K1 and K2 back to zero. This cannot be undone.").size(13),
            ]
            .spacing(14)
            .align_y(Alignment::Center),
        ]
        .spacing(14),
    );

    let export_btn = button(text("Export backup").size(14))
        .padding([10, 18])
        .style(if playing_or_cooldown {
            theme::secondary
        } else {
            theme::primary
        });
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

    scrollable(column![heading("Device"), details, counter_table, actions, backup].spacing(14))
        .into()
}

// ---- monitor --------------------------------------------------------------------------------

fn filter_btn<'a>(label: &'a str, selected: bool, msg: Message) -> Element<'a, Message> {
    button(text(label).size(12))
        .padding([4, 10])
        .style(if selected {
            theme::primary
        } else {
            theme::secondary
        })
        .on_press(msg)
        .into()
}

pub fn monitor(app: &App) -> Element<'_, Message> {
    use opad_model::{LogLevel, LogSource};

    let severity_filters = row![
        caption("LEVEL:"),
        filter_btn(
            "ALL",
            app.log_filter_level.is_none(),
            Message::FilterLogLevel(None)
        ),
        filter_btn(
            "DEBUG",
            app.log_filter_level == Some(LogLevel::Debug),
            Message::FilterLogLevel(Some(LogLevel::Debug))
        ),
        filter_btn(
            "INFO",
            app.log_filter_level == Some(LogLevel::Info),
            Message::FilterLogLevel(Some(LogLevel::Info))
        ),
        filter_btn(
            "WARN",
            app.log_filter_level == Some(LogLevel::Warn),
            Message::FilterLogLevel(Some(LogLevel::Warn))
        ),
        filter_btn(
            "ERROR",
            app.log_filter_level == Some(LogLevel::Error),
            Message::FilterLogLevel(Some(LogLevel::Error))
        ),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let source_filters = row![
        caption("SOURCE:"),
        filter_btn(
            "ALL",
            app.log_filter_source.is_none(),
            Message::FilterLogSource(None)
        ),
        filter_btn(
            "DAEMON",
            app.log_filter_source == Some(LogSource::Host),
            Message::FilterLogSource(Some(LogSource::Host))
        ),
        filter_btn(
            "PROGRAM",
            app.log_filter_source == Some(LogSource::Program),
            Message::FilterLogSource(Some(LogSource::Program))
        ),
        filter_btn(
            "TOSU",
            app.log_filter_source == Some(LogSource::Tosu),
            Message::FilterLogSource(Some(LogSource::Tosu))
        ),
        filter_btn(
            "DEVICE",
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
        button(
            text(if app.log_auto_scroll {
                "Auto-scroll (Newest First)"
            } else {
                "Order (Oldest First)"
            })
            .size(12)
        )
        .padding([4, 12])
        .style(theme::secondary)
        .on_press(Message::ToggleAutoScroll),
        Space::new().width(Length::Fill),
        caption(format!("{} entries (24h retention)", visible_entries.len())),
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
        heading("Logs"),
        muted("Live logs from daemon, program, tosu and device (24h retention)."),
        filter_row,
        action_row,
        card(scrollable(lines).height(Length::Fill)).height(Length::Fill),
    ]
    .spacing(12)
    .into()
}

pub use monitor as logs;

pub fn about<'a>(_app: &'a App) -> Element<'a, Message> {
    let title_section = column![
        row![
            text("OPad")
                .size(28)
                .font(theme::FONT_BOLD)
                .color(theme::PINK),
            text(format!("v{}", env!("CARGO_PKG_VERSION")))
                .size(16)
                .font(theme::FONT_BOLD)
                .color(theme::MUTED),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
        muted("Low-latency ESP32-S3 rhythm gaming keypad manager, telemetry HUD, and tray applet for osu!"),
        text("Developed by GFerreiroS <info@gferreiro.com>").size(13).color(theme::MUTED),
        text("Repository: https://github.com/OPad-dev/OPad").size(13).color(theme::CYAN),
    ]
    .spacing(6);

    let system_info = card(
        column![
            text("System & Hardware").size(16).font(theme::FONT_BOLD),
            row![
                text("Hardware Target:")
                    .size(13)
                    .width(Length::FillPortion(2)),
                text("ESP32-S3 (Waveshare Touch LCD 2)")
                    .size(13)
                    .color(theme::WHITE)
                    .width(Length::FillPortion(3)),
            ],
            row![
                text("Desktop Architecture:")
                    .size(13)
                    .width(Length::FillPortion(2)),
                text("Rust, iced, tokio, prost")
                    .size(13)
                    .color(theme::WHITE)
                    .width(Length::FillPortion(3)),
            ],
            row![
                text("Bundled tosu:").size(13).width(Length::FillPortion(2)),
                text("v4.26.2 (reads osu! memory for the HUD)")
                    .size(13)
                    .color(theme::WHITE)
                    .width(Length::FillPortion(3)),
            ],
        ]
        .spacing(8),
    );

    // Each line is a component actually shipped, attributed as its own
    // licence file does; the full texts are in the files named at the bottom
    let credit = |name: &'a str, detail: &'a str| {
        column![text(name).size(14).font(theme::FONT_BOLD), muted(detail)].spacing(2)
    };
    let licenses_section = card(
        column![
            text("Open source licences").size(16).font(theme::FONT_BOLD),
            credit(
                "OPad (app and firmware)",
                "MIT License. Copyright (c) 2026 GFerreiroS."
            ),
            credit(
                "tosu (bundled osu! memory reader)",
                "LGPL-3.0-only. Mikhail Babynichev and the tosu contributors. \
                 Source: https://github.com/KotRikD/tosu (the exact version is \
                 published with each OPad release). You may replace it: Settings → tosu."
            ),
            credit(
                "Node.js 24 (inside the bundled tosu)",
                "MIT License, with the licences of what Node embeds (OpenSSL, ICU, \
                 libuv, V8, ...)."
            ),
            credit(
                "Rust crates (this app)",
                "MIT, Apache-2.0, BSD, ISC, MPL-2.0, Zlib and others; one entry per crate \
                 with its licence text."
            ),
            credit(
                "Montserrat font",
                "SIL Open Font License 1.1. Copyright 2011 The Montserrat Project Authors."
            ),
            credit(
                "Firmware: ESP-IDF, TinyUSB, FreeRTOS, LVGL, Nanopb",
                "Apache-2.0 (ESP-IDF, esp_tinyusb, esp_lvgl_port, Espressif Systems); \
                 MIT (TinyUSB, hathach; FreeRTOS, Amazon; LVGL, LVGL Kft); \
                 Zlib (Nanopb, Petteri Aimonen)."
            ),
            muted(
                "Full texts: THIRD_PARTY_NOTICES.html next to the app, \
                 tosu/THIRD_PARTY_NOTICES.txt, and FIRMWARE_THIRD_PARTY_NOTICES.md \
                 with each release."
            ),
        ]
        .spacing(14),
    );

    scrollable(
        column![
            heading("About & Licenses"),
            title_section,
            system_info,
            licenses_section,
        ]
        .spacing(16),
    )
    .into()
}
