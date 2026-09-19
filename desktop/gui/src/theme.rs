//! Look of the desktop app, matching the pad's display (firmware/main/ui/core/ui_defaults.c):
//! near-black background, dark cards, osu! pink accent, Montserrat.

use iced::widget::{button, container, text, Text};
use iced::{border, font, Background, Border, Color, Font, Shadow, Theme};

const fn rgb(hex: u32) -> Color {
    Color {
        r: ((hex >> 16) & 0xFF) as f32 / 255.0,
        g: ((hex >> 8) & 0xFF) as f32 / 255.0,
        b: (hex & 0xFF) as f32 / 255.0,
        a: 1.0,
    }
}

pub const BG: Color = rgb(0x0E0E16);
pub const SURFACE: Color = rgb(0x13131E);
pub const CARD: Color = rgb(0x1B1B28);
pub const CARD_HOVER: Color = rgb(0x24243A);
pub const BORDER: Color = rgb(0x2A2A3C);
pub const PINK: Color = rgb(0xFF66AA);
pub const PINK_HOVER: Color = rgb(0xFF85BD);
pub const WHITE: Color = rgb(0xFFFFFF);
pub const MUTED: Color = rgb(0x8C8CA6);
pub const GREEN: Color = rgb(0x44DD88);
pub const RED: Color = rgb(0xFF5566);
pub const OFF: Color = rgb(0x3A3A4C);
pub const CYAN: Color = rgb(0x66CCFF);
pub const YELLOW: Color = rgb(0xFFCC33);

pub const FONT_MEDIUM_BYTES: &[u8] = include_bytes!("../assets/fonts/Montserrat-Medium.ttf");
pub const FONT_BOLD_BYTES: &[u8] = include_bytes!("../assets/fonts/Montserrat-Bold.ttf");

pub const FONT: Font = Font::with_name("Montserrat");
pub const FONT_BOLD: Font = Font {
    weight: font::Weight::Bold,
    ..FONT
};

pub fn theme() -> Theme {
    Theme::custom(
        "osu!pad".to_string(),
        iced::theme::Palette {
            background: BG,
            text: WHITE,
            primary: PINK,
            success: GREEN,
            warning: YELLOW,
            danger: RED,
        },
    )
}

// ---- text ---------------------------------------------------------------------------------

pub fn heading<'a>(content: impl text::IntoFragment<'a>) -> Text<'a> {
    text(content).size(24).font(FONT_BOLD)
}

/// Small uppercase-style caption, like "TOTAL PRESSES" on the pad
pub fn caption<'a>(content: impl text::IntoFragment<'a>) -> Text<'a> {
    text(content).size(12).color(MUTED)
}

pub fn muted<'a>(content: impl text::IntoFragment<'a>) -> Text<'a> {
    text(content).size(14).color(MUTED)
}

// ---- containers -------------------------------------------------------------------------

pub fn card(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(CARD.into()),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 14.0.into(),
        },
        ..Default::default()
    }
}

/// K1-style card: pink fill, dark text
pub fn pink_card(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(PINK.into()),
        text_color: Some(BG),
        border: border::rounded(14),
        ..Default::default()
    }
}

/// K2-style card: dark fill with a half-transparent pink border
pub fn outlined_card(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(CARD.into()),
        border: Border {
            color: Color { a: 0.5, ..PINK },
            width: 1.0,
            radius: 14.0.into(),
        },
        ..Default::default()
    }
}

pub fn sidebar(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(SURFACE.into()),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

pub fn banner(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(CARD.into()),
        border: Border {
            color: Color { a: 0.6, ..PINK },
            width: 1.0,
            radius: 10.0.into(),
        },
        ..Default::default()
    }
}

/// Colored status dot
pub fn dot(on: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some((if on { GREEN } else { OFF }).into()),
        border: border::rounded(5),
        ..Default::default()
    }
}

// ---- buttons ------------------------------------------------------------------------------

fn base(background: Option<Background>, text_color: Color, border: Border) -> button::Style {
    button::Style {
        background,
        text_color,
        border,
        shadow: Shadow::default(),
        snap: true,
    }
}

pub fn primary(_theme: &Theme, status: button::Status) -> button::Style {
    let fill = match status {
        button::Status::Hovered | button::Status::Pressed => PINK_HOVER,
        button::Status::Disabled => Color { a: 0.4, ..PINK },
        button::Status::Active => PINK,
    };
    base(Some(fill.into()), BG, border::rounded(10))
}

pub fn secondary(_theme: &Theme, status: button::Status) -> button::Style {
    let fill = match status {
        button::Status::Hovered | button::Status::Pressed => CARD_HOVER,
        _ => CARD,
    };
    let text_color = if status == button::Status::Disabled {
        MUTED
    } else {
        WHITE
    };
    base(
        Some(fill.into()),
        text_color,
        Border {
            color: BORDER,
            width: 1.0,
            radius: 10.0.into(),
        },
    )
}

pub fn danger(_theme: &Theme, status: button::Status) -> button::Style {
    let fill = match status {
        button::Status::Hovered | button::Status::Pressed => Some(Color { a: 0.15, ..RED }.into()),
        _ => None,
    };
    base(
        fill,
        RED,
        Border {
            color: Color { a: 0.6, ..RED },
            width: 1.0,
            radius: 10.0.into(),
        },
    )
}

/// Sidebar entry
pub fn nav(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let background = if selected {
            Some(CARD.into())
        } else if hovered {
            Some(Color { a: 0.6, ..CARD }.into())
        } else {
            None
        };
        let border = if selected {
            Border {
                color: Color { a: 0.5, ..PINK },
                width: 1.0,
                radius: 10.0.into(),
            }
        } else {
            border::rounded(10)
        };
        base(background, if selected { PINK } else { WHITE }, border)
    }
}

/// List entry in the designer (selected = pink)
pub fn list_item(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        if selected {
            primary(theme, status)
        } else {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            base(
                hovered.then(|| CARD_HOVER.into()),
                WHITE,
                border::rounded(8),
            )
        }
    }
}

/// Clickable status row in sidebar
pub fn sidebar_status(_theme: &Theme, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let background = if hovered {
        Some(Color { a: 0.5, ..CARD }.into())
    } else {
        None
    };
    base(
        background,
        WHITE,
        Border {
            radius: 8.0.into(),
            width: if hovered { 1.0 } else { 0.0 },
            color: if hovered { BORDER } else { Color::TRANSPARENT },
        },
    )
}

