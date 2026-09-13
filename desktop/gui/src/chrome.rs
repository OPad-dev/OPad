//! Window chrome replacing the system decorations: title bar (drag, double-click to
//! maximize, minimize/maximize/close) and invisible edges for resizing.

use crate::theme;
use crate::Message;
use iced::widget::{button, column, container, mouse_area, row, stack, text, Space};
use iced::window::Direction;
use iced::{mouse, Alignment, Border, Color, Element, Length, Theme};

pub const TITLE_BAR_HEIGHT: f32 = 40.0;
const EDGE: f32 = 6.0;
const CORNER: f32 = 12.0;

#[derive(Debug, Clone, Copy)]
pub enum WindowAction {
    Drag,
    ToggleMaximize,
    Minimize,
    Close,
    Resize(Direction),
}

fn caption_button_style(close: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: hovered
                .then(|| (if close { theme::RED } else { theme::CARD_HOVER }).into()),
            text_color: if hovered && close {
                theme::WHITE
            } else {
                theme::MUTED
            },
            border: Border::default(),
            shadow: Default::default(),
            snap: true,
        }
    }
}

fn caption_button<'a>(
    icon: Element<'a, Message>,
    action: WindowAction,
    close: bool,
) -> Element<'a, Message> {
    button(container(icon).center(Length::Fill))
        .width(46)
        .height(TITLE_BAR_HEIGHT)
        .padding(0)
        .style(caption_button_style(close))
        .on_press(Message::Window(action))
        .into()
}

/// A glyph drawn from boxes so it does not depend on icon fonts
fn line_icon<'a>(width: f32, height: f32, filled: bool) -> Element<'a, Message> {
    container(Space::new().width(width).height(height))
        .style(move |_: &Theme| container::Style {
            background: filled.then(|| theme::MUTED.into()),
            border: Border {
                color: theme::MUTED,
                width: if filled { 0.0 } else { 1.5 },
                radius: 1.5.into(),
            },
            ..Default::default()
        })
        .into()
}

pub fn title_bar<'a>(maximized: bool) -> Element<'a, Message> {
    let drag_area = mouse_area(
        container(
            row![
                text("osu!")
                    .size(15)
                    .font(theme::FONT_BOLD)
                    .color(theme::PINK),
                text("pad").size(15).font(theme::FONT_BOLD),
            ]
            .align_y(Alignment::Center),
        )
        .padding([0, 16])
        .height(Length::Fill)
        .width(Length::Fill)
        .align_y(Alignment::Center),
    )
    .on_press(Message::Window(WindowAction::Drag))
    .on_double_click(Message::Window(WindowAction::ToggleMaximize));

    let maximize_icon = if maximized {
        // Restore: two overlapping squares
        stack![
            container(line_icon(8.0, 8.0, false)).padding(iced::Padding {
                top: 0.0,
                right: 0.0,
                bottom: 3.0,
                left: 3.0
            }),
            container(line_icon(8.0, 8.0, false)).padding(iced::Padding {
                top: 3.0,
                right: 3.0,
                bottom: 0.0,
                left: 0.0
            }),
        ]
        .into()
    } else {
        line_icon(10.0, 10.0, false)
    };

    container(
        row![
            drag_area,
            caption_button(line_icon(10.0, 1.5, true), WindowAction::Minimize, false),
            caption_button(maximize_icon, WindowAction::ToggleMaximize, false),
            caption_button(
                text("×").size(22).line_height(1.0).into(),
                WindowAction::Close,
                true
            ),
        ]
        .height(TITLE_BAR_HEIGHT),
    )
    .style(|_: &Theme| container::Style {
        background: Some(theme::SURFACE.into()),
        border: Border {
            color: theme::BORDER,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn edge<'a>(
    direction: Direction,
    width: Length,
    height: Length,
    cursor: mouse::Interaction,
) -> Element<'a, Message> {
    mouse_area(Space::new().width(width).height(height))
        .interaction(cursor)
        .on_press(Message::Window(WindowAction::Resize(direction)))
        .into()
}

/// Invisible resize handles along the window border, layered over the content
pub fn resize_edges<'a>() -> Element<'a, Message> {
    use mouse::Interaction::{
        ResizingDiagonallyDown as Diag, ResizingDiagonallyUp as AntiDiag,
        ResizingHorizontally as H, ResizingVertically as V,
    };
    let fill = Length::Fill;
    column![
        row![
            edge(Direction::NorthWest, CORNER.into(), EDGE.into(), Diag),
            edge(Direction::North, fill, EDGE.into(), V),
            edge(Direction::NorthEast, CORNER.into(), EDGE.into(), AntiDiag),
        ],
        row![
            edge(Direction::West, EDGE.into(), fill, H),
            Space::new().width(fill).height(fill),
            edge(Direction::East, EDGE.into(), fill, H),
        ]
        .height(fill),
        row![
            edge(Direction::SouthWest, CORNER.into(), EDGE.into(), AntiDiag),
            edge(Direction::South, fill, EDGE.into(), V),
            edge(Direction::SouthEast, CORNER.into(), EDGE.into(), Diag),
        ],
    ]
    .into()
}

/// Thin frame so the undecorated window stands out from what is behind it
pub fn frame(_: &Theme) -> container::Style {
    container::Style {
        background: Some(theme::BG.into()),
        border: Border {
            color: Color {
                a: 0.35,
                ..theme::PINK
            },
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}
