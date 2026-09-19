//! Screen layout model for the OPad display.
//!
//! Mirrors `ui_layout_t` / `ui_widget_t` in `firmware/main/ui/core/ui_core.h`; the numeric values
//! of every enum are wire format. `validate` applies the same rules as the firmware's
//! `ui_layout_validate`, so the GUI can report problems before anything reaches the pad.

use osupad_model::ui_source;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const SCREEN_W: i16 = 320;
pub const SCREEN_H: i16 = 240;
pub const MAX_WIDGETS: usize = 32;
/// Byte limits without the NUL terminator
pub const LABEL_MAX_BYTES: usize = 31;
pub const SUFFIX_MAX_BYTES: usize = 11;
pub const DECIMALS_DEFAULT: u8 = 0xFF;

pub const FLAG_BG_FILL: u8 = 0x01;
pub const FLAG_HIDE_WHEN_EMPTY: u8 = 0x02;
pub const FLAG_BORDER: u8 = 0x04;

macro_rules! wire_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident = $value:literal, $label:literal;)* }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),* }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),*];

            pub fn to_wire(self) -> u8 {
                match self { $($name::$variant => $value),* }
            }

            pub fn from_wire(value: u8) -> Option<Self> {
                match value { $($value => Some($name::$variant),)* _ => None }
            }

            pub fn label(self) -> &'static str {
                match self { $($name::$variant => $label),* }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.label())
            }
        }
    };
}

wire_enum!(
    /// `ui_screen_id_t`
    Screen {
        Idle = 0, "Idle";
        Playing = 1, "Playing";
    }
);

wire_enum!(
    /// `ui_widget_kind_t`
    WidgetKind {
        Text = 0, "Text";
        Progress = 1, "Progress bar";
        KeyCard = 2, "Key card";
        StatusDot = 3, "Status dot";
        Rect = 4, "Rectangle";
        Grade = 5, "Grade";
    }
);

wire_enum!(
    /// `ui_font_id_t` (LVGL Montserrat sizes)
    Font {
        Px12 = 0, "12 px";
        Px14 = 1, "14 px";
        Px16 = 2, "16 px";
        Px20 = 3, "20 px";
        Px24 = 4, "24 px";
        Px32 = 5, "32 px";
        Px48 = 6, "48 px";
    }
);

wire_enum!(
    /// `ui_align_t`
    Align {
        Left = 0, "Left";
        Center = 1, "Center";
        Right = 2, "Right";
    }
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Widget {
    pub kind: WidgetKind,
    /// `ui_source_t` id (0 = static text), see `osupad_model::ui_source`
    pub source: u8,
    pub font: Font,
    pub align: Align,
    pub x: i16,
    pub y: i16,
    pub w: i16,
    pub h: i16,
    /// Colors as 0xRRGGBB
    pub fg: u32,
    pub bg: u32,
    pub accent: u32,
    pub radius: u8,
    /// `DECIMALS_DEFAULT` uses the source's default precision
    pub decimals: u8,
    pub flags: u8,
    pub label: String,
    pub suffix: String,
}

impl Widget {
    /// A reasonable starting widget of a kind, placed near the screen center
    pub fn new(kind: WidgetKind) -> Self {
        let mut w = Widget {
            kind,
            source: 0,
            font: Font::Px16,
            align: Align::Center,
            x: 110,
            y: 100,
            w: 100,
            h: 24,
            fg: 0xFFFFFF,
            bg: 0x1B1B28,
            accent: 0xFF66AA,
            radius: 0,
            decimals: DECIMALS_DEFAULT,
            flags: 0,
            label: String::new(),
            suffix: String::new(),
        };
        match kind {
            WidgetKind::Text => w.label = "Text".into(),
            WidgetKind::Progress => {
                w.source = ui_source::MAP_PROGRESS;
                w.x = 60;
                w.w = 200;
                w.h = 8;
                w.radius = 4;
            }
            WidgetKind::KeyCard => {
                w.source = ui_source::PAD_K1_MAP;
                w.font = Font::Px24;
                w.w = 144;
                w.h = 52;
                w.radius = 12;
                w.flags = FLAG_BORDER;
                w.label = "K1".into();
            }
            WidgetKind::StatusDot => {
                w.source = ui_source::STATUS_TOSU;
                w.font = Font::Px14;
                w.w = 64;
                w.h = 22;
                w.fg = 0x8C8CA6;
                w.bg = 0x3A3A4C;
                w.accent = 0x44DD88;
                w.label = "tosu".into();
            }
            WidgetKind::Rect => {
                w.w = 120;
                w.h = 40;
                w.radius = 12;
            }
            WidgetKind::Grade => {
                w.source = ui_source::PLAY_GRADE;
                w.font = Font::Px24;
                w.h = 28;
            }
        }
        w
    }

    pub fn has_flag(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }

    pub fn set_flag(&mut self, flag: u8, on: bool) {
        if on {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    pub fn contains(&self, x: i16, y: i16) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    pub background: u32,
    /// Drawn in order: later widgets are on top
    pub widgets: Vec<Widget>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LayoutError {
    #[error("too many widgets ({0}, max {MAX_WIDGETS})")]
    TooManyWidgets(usize),
    #[error("widget {index}: {reason}")]
    Widget { index: usize, reason: String },
}

impl Layout {
    /// Same rules as `ui_layout_validate` in the firmware
    pub fn validate(&self) -> Result<(), LayoutError> {
        if self.widgets.len() > MAX_WIDGETS {
            return Err(LayoutError::TooManyWidgets(self.widgets.len()));
        }
        for (index, w) in self.widgets.iter().enumerate() {
            let fail = |reason: String| Err(LayoutError::Widget { index, reason });
            if w.source != 0 && source_info(w.source).is_none() {
                return fail(format!("unknown data source {}", w.source));
            }
            if w.w <= 0 || w.h <= 0 || w.w > 2 * SCREEN_W || w.h > 2 * SCREEN_H {
                return fail(format!("bad size {}x{}", w.w, w.h));
            }
            if w.x < -SCREEN_W || w.x > 2 * SCREEN_W || w.y < -SCREEN_H || w.y > 2 * SCREEN_H {
                return fail("position out of range".into());
            }
            if w.label.len() > LABEL_MAX_BYTES {
                return fail(format!("label longer than {} bytes", LABEL_MAX_BYTES));
            }
            if w.suffix.len() > SUFFIX_MAX_BYTES {
                return fail(format!("suffix longer than {} bytes", SUFFIX_MAX_BYTES));
            }
        }
        Ok(())
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("layout serializes")
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

// ---- Data source catalog -------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceCategory {
    Static,
    Map,
    Play,
    Profile,
    Pad,
    Status,
}

impl SourceCategory {
    pub fn label(self) -> &'static str {
        match self {
            SourceCategory::Static => "Static text",
            SourceCategory::Map => "Map",
            SourceCategory::Play => "Live play",
            SourceCategory::Profile => "Profile & session",
            SourceCategory::Pad => "Pad",
            SourceCategory::Status => "Connection",
        }
    }
}

/// A data source as shown in the designer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceInfo {
    pub id: u8,
    /// Stable firmware name, e.g. "play.pp"
    pub key: &'static str,
    pub category: SourceCategory,
}

impl SourceInfo {
    /// Human label: "play.pp_fc" -> "PP fc"
    pub fn label(&self) -> String {
        if self.id == 0 {
            return "None (static text)".into();
        }
        let short = self.key.split_once('.').map_or(self.key, |(_, s)| s);
        let mut words: Vec<String> = short.split('_').map(str::to_string).collect();
        for word in &mut words {
            *word = match word.as_str() {
                "pp" | "ur" | "bpm" | "ar" | "cs" | "od" | "hp" | "kps" | "pc" => {
                    word.to_uppercase()
                }
                "k1" | "k2" => word.to_uppercase(),
                _ => word.clone(),
            };
        }
        let mut label = words.join(" ");
        if let Some(first) = label.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        label
    }
}

impl std::fmt::Display for SourceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.id == 0 {
            f.write_str(&self.label())
        } else {
            write!(f, "{}: {}", self.category.label(), self.label())
        }
    }
}

pub const STATIC_SOURCE: SourceInfo = SourceInfo {
    id: 0,
    key: "none",
    category: SourceCategory::Static,
};

pub fn source_info(id: u8) -> Option<SourceInfo> {
    if id == 0 {
        return Some(STATIC_SOURCE);
    }
    ui_source::ALL
        .iter()
        .find(|(sid, _)| *sid == id)
        .map(|(id, key)| SourceInfo {
            id: *id,
            key,
            category: match key.split('.').next() {
                Some("map") => SourceCategory::Map,
                Some("play") => SourceCategory::Play,
                Some("profile" | "session" | "game") => SourceCategory::Profile,
                Some("pad") => SourceCategory::Pad,
                _ => SourceCategory::Status,
            },
        })
}

/// Every source, static text first, in firmware id order
pub fn all_sources() -> Vec<SourceInfo> {
    std::iter::once(STATIC_SOURCE)
        .chain(ui_source::ALL.iter().filter_map(|(id, _)| source_info(*id)))
        // Internal press-state sources drive key cards, they are not useful on their own
        .filter(|s| !matches!(s.id, ui_source::PAD_K1_DOWN | ui_source::PAD_K2_DOWN))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trip() {
        let layout = Layout {
            background: 0x0E0E16,
            widgets: WidgetKind::ALL.iter().map(|k| Widget::new(*k)).collect(),
        };
        assert_eq!(Layout::from_json(&layout.to_json()).unwrap(), layout);
        assert!(layout.validate().is_ok());
    }

    #[test]
    fn validation_matches_firmware_rules() {
        let mut layout = Layout {
            background: 0,
            widgets: vec![Widget::new(WidgetKind::Text)],
        };
        layout.widgets[0].w = 0;
        assert!(layout.validate().is_err());
        layout.widgets[0].w = 10;
        layout.widgets[0].label = "x".repeat(LABEL_MAX_BYTES + 1);
        assert!(layout.validate().is_err());
        layout.widgets[0].label.clear();
        layout.widgets[0].source = 42; // reserved id
        assert!(layout.validate().is_err());
        layout.widgets = vec![Widget::new(WidgetKind::Rect); MAX_WIDGETS + 1];
        assert_eq!(
            layout.validate(),
            Err(LayoutError::TooManyWidgets(MAX_WIDGETS + 1))
        );
    }

    #[test]
    fn source_labels() {
        assert_eq!(source_info(ui_source::PLAY_PP_FC).unwrap().label(), "PP fc");
        assert_eq!(
            source_info(ui_source::MAP_TITLE).unwrap().category,
            SourceCategory::Map
        );
        assert!(all_sources().iter().all(|s| s.id != ui_source::PAD_K1_DOWN));
    }
}
