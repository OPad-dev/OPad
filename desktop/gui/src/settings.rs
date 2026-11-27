//! The Settings page: the PC app only. Starting at login, updates for the
//! three things OPad keeps current, the osu! connection, and About. Nothing
//! here is a setting of the pad, so nothing waits for a save: every control
//! applies as it is changed.

use crate::theme::{self, heading, muted};
use crate::widgets::{card, help, line, scrolling, section, tab_bar};
use crate::{App, Message};
use iced::widget::{button, checkbox, column, pick_list, row, rule, text, Space};
use iced::{Alignment, Element, Length};
use opad_ipc::UpdateComponent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    General,
    Updates,
    /// The osu! connection (tosu)
    Osu,
    About,
}

const TABS: [(Tab, &str); 4] = [
    (Tab::General, "General"),
    (Tab::Updates, "Updates"),
    (Tab::Osu, "osu! connection"),
    (Tab::About, "About"),
];

const TOSU_REPOSITORY: &str = "https://github.com/KotRikD/tosu";
const OPAD_REPOSITORY: &str = "https://github.com/OPad-dev/OPad";

/// The version of the tosu that ships with OPad, from its folder. The daemon
/// reports the one actually installed (`UpdateView::tosu`); this is what
/// About shows until it has answered.
pub fn installed_tosu_version() -> Option<String> {
    let dir = opad_model::paths::bundled_tosu_dir().ok()?;
    let version = std::fs::read_to_string(dir.join("VERSION")).ok()?;
    let version = version.trim().trim_start_matches('v');
    (!version.is_empty()).then(|| version.to_string())
}

pub fn view(app: &App) -> Element<'_, Message> {
    let tab: Element<'_, Message> = match app.settings_tab {
        Tab::General => general(app),
        Tab::Updates => updates(app),
        Tab::Osu => osu_connection(app),
        Tab::About => about(app),
    };
    scrolling(
        column![
            heading("Settings"),
            muted("The OPad app on this computer. The pad's own settings are on Keys and Screen.")
                .size(14),
            tab_bar(&TABS, app.settings_tab, Message::SelectSettingsTab),
            tab,
        ]
        .spacing(14),
    )
    .into()
}

fn general(app: &App) -> Element<'_, Message> {
    section(
        "Startup",
        column![
            checkbox(app.autostart_tray)
                .label("Start OPad at login")
                .on_toggle(Message::ToggleAutostartTray),
            help("It starts in the tray, without opening this window."),
        ]
        .spacing(8),
    )
}

/// A link that opens in the browser
fn link<'a>(label: &'a str, url: impl Into<String>) -> Element<'a, Message> {
    button(text(label).size(13))
        .style(theme::secondary)
        .padding([3, 10])
        .on_press(Message::OpenUrl(url.into()))
        .into()
}

/// One row of the Updates tab. OPad, tosu and the pad's firmware all read the
/// same: what is installed, what is there to install, whether it installs by
/// itself, and the button that installs it now.
struct UpdateRow<'a> {
    name: String,
    state: String,
    /// The release page of the version on offer
    whats_new: Option<String>,
    /// `None`: this component has no switch of its own
    automatic: Option<(bool, UpdateComponent)>,
    action: Option<(&'a str, Option<Message>)>,
    /// Whole sentences under the row: why not now, or when by itself
    notes: Vec<(String, bool)>,
}

fn update_row(r: UpdateRow<'_>) -> Element<'_, Message> {
    let mut state_line = row![muted(r.state).size(13)]
        .spacing(10)
        .align_y(Alignment::Center);
    if let Some(url) = r.whats_new {
        state_line = state_line.push(link("What's new", url));
    }
    let mut head = row![
        column![text(r.name).size(15).font(theme::FONT_BOLD), state_line].spacing(4),
        Space::new().width(Length::Fill),
    ]
    .spacing(16)
    .align_y(Alignment::Center);
    if let Some((on, component)) = r.automatic {
        head = head.push(
            checkbox(on)
                .label("Update automatically")
                .text_size(13)
                .on_toggle(move |v| Message::ToggleUpdater(component, v)),
        );
    }
    if let Some((label, message)) = r.action {
        let enabled = message.is_some();
        head = head.push(
            button(text(label).size(13))
                .padding([7, 16])
                .style(if enabled {
                    theme::primary
                } else {
                    theme::secondary
                })
                .on_press_maybe(message),
        );
    }
    let mut col = column![head].spacing(6);
    for (note, warning) in r.notes {
        col =
            col.push(
                text(note)
                    .size(13)
                    .color(if warning { theme::YELLOW } else { theme::MUTED }),
            );
    }
    col.into()
}

fn updates(app: &App) -> Element<'_, Message> {
    let Some(u) = &app.updates else {
        return section("Updates", muted("Waiting for the OPad service.").size(14));
    };
    let pending = u.pending_version.as_ref().filter(|_| u.restart_required);

    let component = |name: &str,
                     c: &opad_ipc::ComponentUpdate,
                     component: UpdateComponent,
                     pending: Option<&String>| {
        let installed = c.installed.clone().unwrap_or_else(|| "unknown".into());
        let state = match (pending, &c.available) {
            (Some(v), _) if cfg!(windows) => {
                format!("{v} is downloaded and installs the next time OPad starts.")
            }
            (Some(v), _) => format!("{v} is installed and runs the next time OPad starts."),
            (None, Some(v)) if c.with_app => format!("{v} comes with the next OPad update."),
            (None, Some(v)) => format!("{v} is available."),
            (None, None) if c.with_app => "Up to date. Updated together with OPad.".to_string(),
            (None, None) => "Up to date.".to_string(),
        };
        let mut notes = Vec::new();
        if c.notify_only && c.available.is_some() {
            notes.push((
                "Installed by your package manager: update it there.".to_string(),
                false,
            ));
        }
        UpdateRow {
            name: format!("{name} {installed}"),
            state,
            whats_new: c.available.as_ref().and(c.notes_url.clone()),
            // tosu inside a package is updated with OPad: nothing to switch
            automatic: (!c.with_app).then_some((c.enabled, component)),
            action: (c.ready_to_install && pending.is_none())
                .then_some(("Install now", Some(Message::InstallUpdate(component)))),
            notes,
        }
    };

    let channels = vec!["stable".to_string(), "beta".to_string()];
    let checked = if u.checking {
        "Checking for updates…".to_string()
    } else {
        u.last_check
            .as_deref()
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
            .map(|t| {
                format!(
                    "Last checked {}",
                    t.with_timezone(&chrono::Local).format("%-d %b, %H:%M")
                )
            })
            .unwrap_or_else(|| "Not checked yet".to_string())
    };

    let mut content = column![
        update_row(component("OPad", &u.app, UpdateComponent::App, pending)),
        rule::horizontal(1),
        update_row(component("tosu", &u.tosu, UpdateComponent::Tosu, None)),
        rule::horizontal(1),
        update_row(firmware_row(app)),
        rule::horizontal(1),
        row![
            muted("Channel").size(14),
            pick_list(channels, Some(u.channel.clone()), Message::SetUpdateChannel)
                .text_size(13)
                .padding([5, 10]),
            muted(if u.channel == "beta" {
                "Release candidates as soon as they are out."
            } else {
                "Final releases only."
            })
            .size(13),
            Space::new().width(Length::Fill),
            muted(checked).size(13),
            button(text("Check now").size(13))
                .style(theme::secondary)
                .padding([7, 16])
                .on_press_maybe((!u.checking).then_some(Message::CheckForUpdates)),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    ]
    .spacing(14);

    if let Some(r) = &u.last_result {
        content = content.push(muted(format!("Last update: {r}")).size(13));
    }
    if let Some(e) = &u.last_error {
        content = content.push(
            text(format!("The last check did not work: {e}"))
                .size(13)
                .color(theme::YELLOW),
        );
    }
    section("Updates", content)
}

/// The pad's firmware, flashed over USB. It is the one update that stops the
/// pad being a keyboard while it runs, so by itself it waits until the pad
/// has gone unused (`FirmwareOffer::auto_after_minutes`), and Update now asks
/// first. Blockers are the daemon's own whole sentences.
fn firmware_row(app: &App) -> UpdateRow<'_> {
    let Some(offer) = &app.firmware_offer else {
        return UpdateRow {
            name: "Pad firmware".to_string(),
            state: "Waiting for the OPad service.".to_string(),
            whats_new: None,
            automatic: None,
            action: None,
            notes: Vec::new(),
        };
    };
    let automatic = Some((offer.auto, UpdateComponent::Firmware));
    let Some(installed) = &offer.installed else {
        return UpdateRow {
            name: "Pad firmware".to_string(),
            state: "No pad connected.".to_string(),
            whats_new: None,
            automatic,
            action: None,
            notes: Vec::new(),
        };
    };
    let outage = offer.outage_seconds.max(1);
    let wait = offer.auto_after_minutes.max(1);
    let mut notes: Vec<(String, bool)> = offer.blockers.iter().map(|b| (b.clone(), true)).collect();
    if let Some(v) = &offer.available {
        notes.push((
            format!("The pad stops typing for about {outage} seconds while it updates."),
            false,
        ));
        if offer.auto {
            notes.push((
                match offer.idle_minutes {
                    Some(idle) if idle >= wait => format!(
                        "{v} installs by itself now that the pad has gone {wait} minutes unused."
                    ),
                    Some(idle) => format!(
                        "{v} installs by itself once the pad has gone {wait} minutes unused \
                         ({idle} so far), never during a map."
                    ),
                    None => format!(
                        "{v} installs by itself once the pad has gone {wait} minutes unused, \
                         never during a map."
                    ),
                },
                false,
            ));
        }
    }
    if let Some(app_version) = &offer.needs_app {
        notes.push((
            format!("This firmware needs OPad {app_version} or newer: update OPad first."),
            true,
        ));
    }
    if let Some(notes_text) = &offer.notes {
        notes.push((format!("In this version: {notes_text}"), false));
    }
    let can_update = offer.available.is_some() && offer.blockers.is_empty();
    UpdateRow {
        name: format!("Pad firmware {installed}"),
        state: match &offer.available {
            Some(v) => format!("{v} is available."),
            None => "Up to date.".to_string(),
        },
        whats_new: None,
        automatic,
        action: offer.available.as_ref().map(|_| {
            (
                "Update now",
                can_update.then_some(Message::PromptFirmwareConsent),
            )
        }),
        notes,
    }
}

fn osu_connection(app: &App) -> Element<'_, Message> {
    let status = if app.tosu_connected {
        row![
            text("Connected").size(15).color(theme::GREEN),
            muted("The pad's screen is getting the map, your accuracy and pp.").size(14),
        ]
    } else {
        row![
            text("Not connected").size(15).color(theme::YELLOW),
            muted("Start it, then open osu!.").size(14),
            Space::new().width(Length::Fill),
            button(text("Start").size(14))
                .padding([8, 20])
                .style(theme::primary)
                .on_press_maybe(app.daemon_online.then_some(Message::StartTosu)),
        ]
    }
    .spacing(12)
    .align_y(Alignment::Center);

    let mut program = row![button(text("Choose another tosu…").size(13))
        .padding([7, 14])
        .style(theme::secondary)
        .on_press(Message::PickTosuPath)]
    .spacing(8);
    if app.tosu_override_path.is_some() {
        program = program.push(
            button(text("Use the one that comes with OPad").size(13))
                .padding([7, 14])
                .style(theme::secondary)
                .on_press(Message::ResetTosuPath),
        );
    }

    column![
        section(
            "osu! connection",
            column![
                status,
                help(
                    "OPad reads osu! through tosu, a free program it installs and starts for \
                     you. Without it the pad still types; its screen shows no map data."
                ),
            ]
            .spacing(12),
        ),
        section(
            "tosu program",
            column![
                line(
                    "In use",
                    crate::tosu_source_status(app.tosu_override_path.as_deref())
                ),
                program,
                help(
                    "Only change this if you want to run a tosu you built or downloaded yourself."
                ),
            ]
            .spacing(10),
        ),
    ]
    .spacing(14)
    .into()
}

fn about(app: &App) -> Element<'_, Message> {
    // What the daemon found installed; the bundled folder's version until it answers
    let tosu_version = app
        .updates
        .as_ref()
        .and_then(|u| u.tosu.installed.clone())
        .or_else(|| app.tosu_version.clone());

    let opad = card(
        column![
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
            muted("The app for the OPad keypad: its keys, its screen and your osu! stats.")
                .size(14),
            muted("Made by GFerreiroS. Free software, GNU GPL v3.0 or later.").size(14),
            row![link("Source code", OPAD_REPOSITORY)],
        ]
        .spacing(8),
    );

    let tosu = section(
        "tosu",
        column![
            muted(
                "tosu is a free program that reads what osu! is showing (the map, your \
                 accuracy, pp) so OPad can show it on the pad's screen. OPad installs it and \
                 starts it for you."
            )
            .size(14),
            line(
                "Version installed",
                tosu_version.map_or("Not found".to_string(), |v| format!("v{v}"))
            ),
            line(
                "Made by",
                "Mikhail Babynichev and the tosu contributors (LGPL-3.0)"
            ),
            row![link("tosu's source code", TOSU_REPOSITORY)],
            help("You may replace it with your own build: Settings, osu! connection."),
        ]
        .spacing(8),
    );

    // Each line is a component actually shipped, attributed as its own
    // licence file does; the full texts are in the files named at the bottom
    let credit = |name: &'static str, detail: &'static str| {
        column![
            text(name).size(14).font(theme::FONT_BOLD),
            muted(detail).size(13)
        ]
        .spacing(2)
    };
    let licences = section(
        "Open source licences",
        column![
            credit(
                "OPad (app and firmware)",
                "GNU GPL v3.0 or later. Copyright (c) 2026 GFerreiroS."
            ),
            credit(
                "tosu",
                "LGPL-3.0-only. Mikhail Babynichev and the tosu contributors. The exact \
                 version is published with each OPad release."
            ),
            credit(
                "Node.js 24 (inside tosu)",
                "MIT License, with the licences of what Node embeds (OpenSSL, ICU, libuv, \
                 V8, ...)."
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
                "Apache-2.0 (ESP-IDF, esp_tinyusb, esp_lvgl_port, Espressif Systems); MIT \
                 (TinyUSB, hathach; FreeRTOS, Amazon; LVGL, LVGL Kft); Zlib (Nanopb, Petteri \
                 Aimonen)."
            ),
            muted(
                "Full texts: THIRD_PARTY_NOTICES.html next to the app, \
                 tosu/THIRD_PARTY_NOTICES.txt, and FIRMWARE_THIRD_PARTY_NOTICES.md with each \
                 release."
            )
            .size(13),
        ]
        .spacing(14),
    );

    column![opad, tosu, licences].spacing(14).into()
}
