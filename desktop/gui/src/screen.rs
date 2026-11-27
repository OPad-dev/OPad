//! The Screen page: the pad's display, what its touchscreen swipes do, and
//! the animation each grade plays. All of it is the pad's settings, sent by
//! the save bar this page shares with Keys. What is drawn on the screen is
//! the Designer's.

use crate::theme::{caption, heading, muted};
use crate::widgets::{help, pad_section, section, with_save_bar};
use crate::{theme, App, Message, Page};
use iced::widget::{button, column, container, pick_list, row, slider, text, Space};
use iced::{Alignment, Element, Length};
use opad_model::result_anim::{self, Trigger};
use opad_model::swipe::{free_keys, swipe_choices, FreeKey, SwipeChoice, SwipeDir};

/// A result animation as the pickers list it, by id
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimChoice(pub u32);

impl std::fmt::Display for AnimChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(result_anim::name(self.0))
    }
}

fn anim_choices() -> Vec<AnimChoice> {
    std::iter::once(result_anim::NONE)
        .chain(result_anim::BUILTINS.iter().map(|(id, _)| *id))
        .map(AnimChoice)
        .collect()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let content = column![
        heading("Screen"),
        muted("The pad's display, its touchscreen and the result animations.").size(14),
        pad_section(app, "Display", || display(app)),
        pad_section(app, "Touchscreen", || touchscreen(app)),
        pad_section(app, "Result animations", || result_animations(app)),
        section(
            "Layout",
            row![
                muted("What the screen shows, and where, is edited in the Designer.")
                    .size(14)
                    .width(Length::Fill),
                button(text("Edit the layout").size(14))
                    .padding([8, 18])
                    .style(theme::secondary)
                    .on_press(Message::Navigate(Page::Designer)),
            ]
            .align_y(Alignment::Center),
        ),
    ]
    .spacing(14);
    with_save_bar(app, content.into())
}

fn display(app: &App) -> Element<'_, Message> {
    let sleep_label = if app.sleep_seconds.is_multiple_of(60) {
        format!("{} min", app.sleep_seconds / 60)
    } else {
        format!("{} s", app.sleep_seconds)
    };
    column![
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
            help("The screen turns off after this long without a key press or a touch."),
        ]
        .spacing(8),
        column![
            row![
                caption("SCREEN UPDATES DURING A MAP"),
                Space::new().width(Length::Fill),
                text(format!("{} per second", app.gameplay_display_hz)).size(14)
            ],
            slider(1..=30, app.gameplay_display_hz, Message::GameplayDisplayHz),
            help("How often pp, progress and hit counts are refreshed while you play."),
        ]
        .spacing(8),
    ]
    .spacing(18)
    .into()
}

fn touchscreen(app: &App) -> Element<'_, Message> {
    // One direction's action, and the key when that action is "Other key"
    let swipe_select = |label: &'static str, dir: SwipeDir| {
        let setting = app.swipes[dir.index()];
        let choice = setting.choice();
        let mut col = column![
            caption(label),
            pick_list(swipe_choices(), Some(choice), move |c| {
                Message::Swipe(dir, c)
            })
            .width(Length::Fill)
            .padding(10),
        ]
        .spacing(6)
        .width(Length::FillPortion(1));
        if choice == SwipeChoice::OtherKey {
            let current = free_keys().into_iter().find(|k| *k == FreeKey(setting.key));
            col = col.push(
                pick_list(free_keys(), current, move |k| Message::SwipeKey(dir, k))
                    .placeholder("Choose a key")
                    .width(Length::Fill)
                    .padding(10),
            );
        }
        col
    };
    column![
        help(
            "During a map a resting finger is Quick Retry and swipes do nothing. Outside a \
             map each swipe does what you pick here."
        ),
        // Laid out as the swipes go: up on top, down below, left and right
        // either side of the pad
        column![
            row![
                Space::new().width(Length::FillPortion(1)),
                swipe_select("SWIPE UP", SwipeDir::Up),
                Space::new().width(Length::FillPortion(1)),
            ]
            .spacing(16),
            row![
                swipe_select("SWIPE LEFT", SwipeDir::Left),
                container(muted("Pad").size(14)).center_x(Length::FillPortion(1)),
                swipe_select("SWIPE RIGHT", SwipeDir::Right),
            ]
            .spacing(16)
            .align_y(Alignment::Center),
            row![
                Space::new().width(Length::FillPortion(1)),
                swipe_select("SWIPE DOWN", SwipeDir::Down),
                Space::new().width(Length::FillPortion(1)),
            ]
            .spacing(16),
        ]
        .spacing(12),
        help(
            "Volume goes to osu! while osu! is in use and to the system otherwise. While \
             osu! is not connected, every touch is Quick Retry."
        ),
    ]
    .spacing(14)
    .into()
}

fn result_animations(app: &App) -> Element<'_, Message> {
    // One grade's animation, and a button that plays it on the pad now
    let anim_cell = |trigger: Trigger| {
        let id = app.result_animations.get(trigger);
        let play = button(text("Play").size(13))
            .style(theme::secondary)
            .padding(10)
            .on_press_maybe(
                (id != result_anim::NONE).then_some(Message::PreviewAnimation(trigger)),
            );
        column![
            caption(trigger.label()),
            row![
                pick_list(anim_choices(), Some(AnimChoice(id)), move |c| {
                    Message::ResultAnimation(trigger, c)
                })
                .width(Length::Fill)
                .padding(10),
                play,
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(6)
        .width(Length::FillPortion(1))
    };
    column![
        help(
            "Played on the pad when osu! shows the results of your own play, for the grade \
             you got. Play shows one on the pad now."
        ),
        row![
            anim_cell(Trigger::SS),
            anim_cell(Trigger::S),
            anim_cell(Trigger::A)
        ]
        .spacing(16),
        row![
            anim_cell(Trigger::B),
            anim_cell(Trigger::C),
            anim_cell(Trigger::D)
        ]
        .spacing(16),
    ]
    .spacing(14)
    .into()
}
