//! What every page is built from: section cards, tab bars, the note shown in
//! place of a pad setting while no pad is connected, and the save bar that
//! Keys and Screen share.

use crate::theme::{self, muted};
use crate::{App, Message};
use iced::widget::{button, column, container, row, text, Space};
use iced::{Alignment, Element, Length};

pub fn card<'a>(content: impl Into<Element<'a, Message>>) -> container::Container<'a, Message> {
    container(content)
        .padding(20)
        .width(Length::Fill)
        .style(theme::card)
}

/// A titled part of a page
pub fn section<'a>(title: &'a str, body: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    card(column![text(title).size(18).font(theme::FONT_BOLD), body.into()].spacing(16)).into()
}

/// A pad setting's section: its controls with a pad, and one sentence in
/// their place without one. The same on every page that edits the pad.
pub fn pad_section<'a>(
    app: &App,
    title: &'a str,
    body: impl FnOnce() -> Element<'a, Message>,
) -> Element<'a, Message> {
    if app.device_connected {
        section(title, body())
    } else {
        section(title, muted("Connect your pad to change this.").size(14))
    }
}

/// A row of tabs; `selected` is the one showing
pub fn tab_bar<'a, T: Copy + PartialEq + 'a>(
    tabs: &[(T, &'a str)],
    selected: T,
    on_select: impl Fn(T) -> Message,
) -> Element<'a, Message> {
    row(tabs.iter().map(|(tab, label)| {
        button(text(*label).size(14))
            .padding([8, 16])
            .style(theme::tab_button(*tab == selected))
            .on_press(on_select(*tab))
            .into()
    }))
    .spacing(8)
    .into()
}

/// A label and its value on one line
pub fn line<'a>(label: &'a str, value: impl Into<String>) -> Element<'a, Message> {
    row![
        muted(label).size(14).width(180),
        text(value.into()).size(14)
    ]
    .spacing(8)
    .into()
}

/// One sentence of help under a control
pub fn help<'a>(sentence: &'a str) -> Element<'a, Message> {
    muted(sentence).size(13).into()
}

/// Pinned under Keys and Screen: whether something on either page is not on
/// the pad yet, with the two things to do about it. Both pages edit the same
/// settings object, so one Save sends everything.
pub fn save_bar(app: &App) -> Element<'_, Message> {
    let dirty = app.pad_dirty();
    let saving = app.pad_sent.is_some();
    let detecting = app.detecting_pin.is_some();
    let state = if !app.device_connected {
        text("Connect your pad to change its settings.").color(theme::MUTED)
    } else if saving {
        text("Saving to the pad…").color(theme::MUTED)
    } else if dirty {
        text("Unsaved changes").color(theme::YELLOW)
    } else {
        text("Everything is saved on the pad.").color(theme::MUTED)
    };
    let ready = dirty && !saving && !detecting;
    container(
        row![
            state.size(14),
            Space::new().width(Length::Fill),
            button(text("Revert").size(14))
                .padding([9, 18])
                .style(theme::secondary)
                .on_press_maybe((dirty && !saving).then_some(Message::RevertPad)),
            button(text("Save to pad").size(14))
                .padding([9, 18])
                .style(if ready {
                    theme::primary
                } else {
                    theme::secondary
                })
                .on_press_maybe(ready.then_some(Message::SavePad)),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding([12, 18])
    .width(Length::Fill)
    .style(theme::card)
    .into()
}

/// A page's content, scrolling when it is taller than the window. The right
/// padding keeps the scrollbar off the cards.
pub fn scrolling<'a>(
    content: impl Into<Element<'a, Message>>,
) -> iced::widget::Scrollable<'a, Message> {
    iced::widget::scrollable(container(content).padding(iced::Padding {
        right: 16.0,
        ..iced::Padding::ZERO
    }))
}

/// A page that edits the pad: its sections scroll, the save bar stays put
pub fn with_save_bar<'a>(app: &'a App, content: Element<'a, Message>) -> Element<'a, Message> {
    column![scrolling(content).height(Length::Fill), save_bar(app)]
        .spacing(12)
        .into()
}
