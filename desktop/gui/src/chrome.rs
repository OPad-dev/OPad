//! Window chrome replacing the system decorations: a rounded frame, the logo
//! over the sidebar and a strip over the page (both drag the window, a
//! double-click maximizes it), minimize/maximize/close, and invisible edges
//! for resizing.

use crate::theme;
use crate::Message;
use iced::widget::{button, column, container, mouse_area, row, stack, text, Space};
use iced::window::Direction;
use iced::{mouse, Alignment, Border, Color, Element, Length, Theme};

pub const TITLE_BAR_HEIGHT: f32 = 40.0;
const EDGE: f32 = 6.0;
const CORNER: f32 = 12.0;
/// How round the window's corners are. The window itself is transparent
/// (`open_window`), so what is drawn decides its shape.
const RADIUS: f32 = 12.0;
const FRAME: f32 = 1.0;

/// The radius of what sits in a corner of the window, just inside the frame.
/// A maximized window fills the screen and has square corners.
fn inner_radius(maximized: bool) -> f32 {
    if maximized {
        0.0
    } else {
        RADIUS - FRAME
    }
}

#[derive(Debug, Clone, Copy)]
pub enum WindowAction {
    Drag,
    ToggleMaximize,
    Minimize,
    Close,
    Resize(Direction),
}

/// `corner` rounds the button's top right corner: the close button sits in
/// the window's, and its highlight must not stick out of it
fn caption_button_style(
    close: bool,
    corner: f32,
) -> impl Fn(&Theme, button::Status) -> button::Style {
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
            border: Border {
                radius: iced::border::top_right(corner),
                ..Border::default()
            },
            shadow: Default::default(),
            snap: true,
        }
    }
}

fn caption_button<'a>(
    icon: Element<'a, Message>,
    action: WindowAction,
    close: bool,
    corner: f32,
) -> Element<'a, Message> {
    button(container(icon).center(Length::Fill))
        .width(46)
        .height(TITLE_BAR_HEIGHT)
        .padding(0)
        .style(caption_button_style(close, corner))
        .on_press(Message::Window(action))
        .into()
}

/// Dragging `content` moves the window, a double-click maximizes it
fn drags_window<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    mouse_area(content)
        .on_press(Message::Window(WindowAction::Drag))
        .on_double_click(Message::Window(WindowAction::ToggleMaximize))
        .into()
}

/// The logo at the top of the sidebar, lined up with the page names under it
pub fn logo<'a>() -> Element<'a, Message> {
    drags_window(
        container(
            row![
                text("O").size(18).font(theme::FONT_BOLD).color(theme::PINK),
                text("Pad").size(18).font(theme::FONT_BOLD),
            ]
            .align_y(Alignment::Center),
        )
        .padding(iced::Padding {
            left: 32.0,
            top: 6.0,
            ..iced::Padding::ZERO
        })
        .height(TITLE_BAR_HEIGHT + 12.0)
        .width(Length::Fill)
        .align_y(Alignment::Center),
    )
}

/// The sidebar: one surface from the top of the window to the bottom
pub fn sidebar(maximized: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(theme::SURFACE.into()),
        border: Border {
            radius: iced::border::left(inner_radius(maximized)),
            ..Border::default()
        },
        ..Default::default()
    }
}

/// The line between the sidebar and the page
pub fn divider<'a>() -> Element<'a, Message> {
    container(Space::new().width(1).height(Length::Fill))
        .style(|_: &Theme| container::Style {
            background: Some(theme::BORDER.into()),
            ..Default::default()
        })
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

/// The strip over the page: it has the page's own background, so the window
/// buttons sit on the page rather than on a bar of another colour
pub fn title_bar<'a>(maximized: bool) -> Element<'a, Message> {
    let drag_area = drags_window(Space::new().width(Length::Fill).height(Length::Fill));
    let corner = inner_radius(maximized);

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

    row![
        drag_area,
        caption_button(
            line_icon(10.0, 1.5, true),
            WindowAction::Minimize,
            false,
            0.0
        ),
        caption_button(maximize_icon, WindowAction::ToggleMaximize, false, 0.0),
        caption_button(
            text("×").size(22).line_height(1.0).into(),
            WindowAction::Close,
            true,
            corner
        ),
    ]
    .height(TITLE_BAR_HEIGHT)
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

/// The window's own shape: rounded corners and a thin line, so that it
/// stands out from what is behind it. What is inside is inset by
/// `frame_width`, or it would be drawn over the line.
pub fn frame(maximized: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(theme::BG.into()),
        border: Border {
            color: theme::BORDER,
            width: frame_width(maximized),
            radius: if maximized { 0.0 } else { RADIUS }.into(),
        },
        ..Default::default()
    }
}

pub fn frame_width(maximized: bool) -> f32 {
    if maximized {
        0.0
    } else {
        FRAME
    }
}

/// The layer that dims the window under a dialog, in the window's shape
pub fn dim(maximized: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(
            Color {
                a: 0.75,
                ..theme::BG
            }
            .into(),
        ),
        border: Border {
            radius: if maximized { 0.0 } else { RADIUS }.into(),
            ..Border::default()
        },
        ..Default::default()
    }
}
