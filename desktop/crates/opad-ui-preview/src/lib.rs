//! Pixel-exact preview of OPad screens.
//!
//! Links the firmware's LVGL UI core (`firmware/main/ui/core`) and LVGL itself, built with the
//! firmware's sdkconfig, and renders layouts into RGBA images on the host.

use std::sync::{Mutex, OnceLock};

pub const SCREEN_W: usize = opad_layout::SCREEN_W as usize;
pub const SCREEN_H: usize = opad_layout::SCREEN_H as usize;
pub const MAX_WIDGETS: usize = opad_layout::MAX_WIDGETS;
/// C field sizes: the text limits plus the NUL (`UI_LABEL_MAX`/`UI_SUFFIX_MAX`)
pub const LABEL_MAX: usize = opad_layout::LABEL_MAX_BYTES + 1;
pub const SUFFIX_MAX: usize = opad_layout::SUFFIX_MAX_BYTES + 1;

pub const SCREEN_IDLE: u8 = 0;
pub const SCREEN_PLAYING: u8 = 1;

/// Mirror of `ui_widget_t` (firmware/main/ui/core/ui_core.h)
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawWidget {
    pub kind: u8,
    pub source: u8,
    pub font: u8,
    pub align: u8,
    pub x: i16,
    pub y: i16,
    pub w: i16,
    pub h: i16,
    pub fg: u32,
    pub bg: u32,
    pub accent: u32,
    pub radius: u8,
    pub decimals: u8,
    pub flags: u8,
    pub reserved: u8,
    pub label: [u8; LABEL_MAX],
    pub suffix: [u8; SUFFIX_MAX],
}

/// Mirror of `ui_layout_t`
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawLayout {
    pub background: u32,
    pub count: u8,
    pub widgets: [RawWidget; MAX_WIDGETS],
}

extern "C" {
    fn preview_init();
    fn preview_render(layout: *const RawLayout, out_rgba: *mut u8) -> i32;
    fn preview_sizeof_widget() -> usize;
    fn preview_sizeof_layout() -> usize;
    fn ui_default_layout(screen: u8) -> *const RawLayout;
    fn ui_data_set_number(source: u8, value: f64);
    fn ui_data_set_string(source: u8, value: *const std::ffi::c_char);
    fn ui_data_clear(source: u8);
    fn ui_source_info(source: u8) -> *const RawSourceInfo;
}

#[repr(C)]
struct RawSourceInfo {
    name: *const std::ffi::c_char,
}

/// Stable name of a data source in the firmware table (e.g. "play.pp"), `None` if undefined
pub fn source_name(source: u8) -> Option<String> {
    let info = unsafe { ui_source_info(source) };
    if info.is_null() {
        return None;
    }
    let name = unsafe { std::ffi::CStr::from_ptr((*info).name) };
    Some(name.to_string_lossy().into_owned())
}

/// LVGL keeps global state and is not thread-safe: every call goes through this lock.
fn lvgl() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let guard = LOCK
        .get_or_init(|| {
            assert_eq!(
                unsafe { preview_sizeof_widget() },
                std::mem::size_of::<RawWidget>(),
                "ui_widget_t layout mismatch"
            );
            assert_eq!(
                unsafe { preview_sizeof_layout() },
                std::mem::size_of::<RawLayout>(),
                "ui_layout_t layout mismatch"
            );
            unsafe { preview_init() };
            Mutex::new(())
        })
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    guard
}

/// A value for a data source
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(f64),
    Text(String),
    Empty,
}

/// Built-in layout for a screen, straight from the firmware defaults
pub fn default_layout(screen: u8) -> Option<RawLayout> {
    let _g = lvgl();
    let ptr = unsafe { ui_default_layout(screen) };
    (!ptr.is_null()).then(|| unsafe { *ptr })
}

/// Set data source values used by subsequent renders
pub fn set_values<'a>(values: impl IntoIterator<Item = (u8, &'a Value)>) {
    let _g = lvgl();
    for (source, value) in values {
        match value {
            Value::Number(n) => unsafe { ui_data_set_number(source, *n) },
            Value::Text(s) => {
                let c = std::ffi::CString::new(s.replace('\0', "")).unwrap();
                unsafe { ui_data_set_string(source, c.as_ptr()) }
            }
            Value::Empty => unsafe { ui_data_clear(source) },
        }
    }
}

/// Render a layout to RGBA8888 (320×240). `None` if the firmware validator rejects it.
pub fn render(layout: &RawLayout) -> Option<Vec<u8>> {
    let _g = lvgl();
    let mut rgba = vec![0u8; SCREEN_W * SCREEN_H * 4];
    let rc = unsafe { preview_render(layout, rgba.as_mut_ptr()) };
    (rc == 0).then_some(rgba)
}

/// Encode RGBA8888 as PNG, optionally upscaled by an integer factor
pub fn encode_png(rgba: &[u8], scale: usize) -> Vec<u8> {
    let scale = scale.max(1);
    let (w, h) = (SCREEN_W * scale, SCREEN_H * scale);
    let mut scaled = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let i = ((y / scale) * SCREEN_W + x / scale) * 4;
            scaled.extend_from_slice(&rgba[i..i + 4]);
        }
    }
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, w as u32, h as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&scaled)
            .unwrap();
    }
    out
}

/// Copy a UTF-8 string into a fixed NUL-terminated C field, truncating on a char boundary
pub fn to_c_field<const N: usize>(s: &str) -> [u8; N] {
    let mut field = [0u8; N];
    let mut end = s.len().min(N - 1);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    field[..end].copy_from_slice(&s.as_bytes()[..end]);
    field
}

/// Plausible values for every source the default layouts show, for previews without live data
pub fn sample_values() -> Vec<(u8, Value)> {
    use opad_model::ui_source as src;
    let text = |s: &str| Value::Text(s.to_string());
    vec![
        (src::MAP_TITLE, text("crystallized")),
        (src::MAP_ARTIST, text("Camellia")),
        (src::MAP_MAPPER, text("-ckopoctb-")),
        (src::MAP_DIFFICULTY, text("My Friends Never Die")),
        (src::MAP_STATUS, text("loved")),
        (src::MAP_STARS, Value::Number(6.39)),
        (src::MAP_STARS_LIVE, Value::Number(4.53)),
        (src::MAP_AR, Value::Number(9.66)),
        (src::MAP_CS, Value::Number(3.81)),
        (src::MAP_OD, Value::Number(9.99)),
        (src::MAP_HP, Value::Number(6.66)),
        (src::MAP_BPM, Value::Number(174.0)),
        (src::MAP_OBJECTS, Value::Number(1064.0)),
        (src::MAP_MAX_COMBO, Value::Number(1787.0)),
        (src::MAP_LENGTH, Value::Number(257_297.0)),
        (src::MAP_TIME_ELAPSED, Value::Number(95_000.0)),
        (src::MAP_TIME_REMAINING, Value::Number(171_304.0)),
        (src::MAP_PROGRESS, Value::Number(0.34)),
        (src::PLAY_PP, Value::Number(286.4)),
        (src::PLAY_PP_FC, Value::Number(423.5)),
        (src::PLAY_PP_MAX, Value::Number(388.72)),
        (src::PLAY_ACCURACY, Value::Number(98.62)),
        (src::PLAY_SCORE, Value::Number(1_234_567.0)),
        (src::PLAY_COMBO, Value::Number(412.0)),
        (src::PLAY_MAX_COMBO, Value::Number(530.0)),
        (src::PLAY_GRADE, text("S")),
        (src::PLAY_HITS_300, Value::Number(612.0)),
        (src::PLAY_HITS_100, Value::Number(24.0)),
        (src::PLAY_HITS_50, Value::Number(2.0)),
        (src::PLAY_HITS_MISS, Value::Number(1.0)),
        (src::PLAY_SLIDER_BREAKS, Value::Number(0.0)),
        (src::PLAY_UR, Value::Number(147.6)),
        (src::PLAY_HEALTH, Value::Number(0.92)),
        (src::PLAY_MODS, text("HD")),
        (src::PLAY_PLAYER, text("osu!player")),
        (src::PROFILE_NAME, text("osu!player")),
        (src::PROFILE_RANK, Value::Number(12_345.0)),
        (src::PROFILE_PP, Value::Number(4321.0)),
        (src::PROFILE_ACCURACY, Value::Number(97.5)),
        (src::PROFILE_PLAYCOUNT, Value::Number(25_000.0)),
        (src::PROFILE_LEVEL, Value::Number(100.0)),
        (src::PROFILE_COUNTRY, text("US")),
        (src::SESSION_PLAYTIME, Value::Number(5_400_000.0)),
        (src::SESSION_PLAYCOUNT, Value::Number(12.0)),
        (src::GAME_STATE, text("Playing")),
        (src::PAD_K1_MAP, Value::Number(312.0)),
        (src::PAD_K2_MAP, Value::Number(287.0)),
        (src::PAD_TOTAL_MAP, Value::Number(599.0)),
        (src::PAD_K1_LIFETIME, Value::Number(1_284_311.0)),
        (src::PAD_K2_LIFETIME, Value::Number(1_190_870.0)),
        (src::PAD_TOTAL_LIFETIME, Value::Number(2_475_181.0)),
        (src::PAD_KPS, Value::Number(9.0)),
        (src::PAD_K1_LABEL, text("Z")),
        (src::PAD_K2_LABEL, text("X")),
        (src::PAD_CLOCK, text("21:07")),
        (src::PAD_DATE, text("Sat 12 Sep")),
        (src::PAD_UPTIME, Value::Number(3_600_000.0)),
        (src::PAD_K1_DOWN, Value::Number(0.0)),
        (src::PAD_K2_DOWN, Value::Number(0.0)),
        (src::STATUS_PC, Value::Number(1.0)),
        (src::STATUS_TOSU, Value::Number(1.0)),
        (src::STATUS_OSU, Value::Number(1.0)),
    ]
}

// ---- opad_layout conversions ----------------------------------------------------------

fn c_str(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

impl From<&opad_layout::Layout> for RawLayout {
    fn from(layout: &opad_layout::Layout) -> Self {
        let empty = RawWidget {
            kind: 0,
            source: 0,
            font: 0,
            align: 0,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            fg: 0,
            bg: 0,
            accent: 0,
            radius: 0,
            decimals: 0,
            flags: 0,
            reserved: 0,
            label: [0; LABEL_MAX],
            suffix: [0; SUFFIX_MAX],
        };
        let mut raw = RawLayout {
            background: layout.background,
            count: 0,
            widgets: [empty; MAX_WIDGETS],
        };
        for (slot, w) in raw.widgets.iter_mut().zip(&layout.widgets) {
            *slot = RawWidget {
                kind: w.kind.to_wire(),
                source: w.source,
                font: w.font.to_wire(),
                align: w.align.to_wire(),
                x: w.x,
                y: w.y,
                w: w.w,
                h: w.h,
                fg: w.fg,
                bg: w.bg,
                accent: w.accent,
                radius: w.radius,
                decimals: w.decimals,
                flags: w.flags,
                reserved: 0,
                label: to_c_field(&w.label),
                suffix: to_c_field(&w.suffix),
            };
        }
        raw.count = layout.widgets.len().min(MAX_WIDGETS) as u8;
        raw
    }
}

impl From<&RawLayout> for opad_layout::Layout {
    fn from(raw: &RawLayout) -> Self {
        use opad_layout::{Align, Font, WidgetKind};
        opad_layout::Layout {
            background: raw.background,
            widgets: raw.widgets[..raw.count as usize]
                .iter()
                .map(|w| opad_layout::Widget {
                    kind: WidgetKind::from_wire(w.kind).unwrap_or(WidgetKind::Text),
                    source: w.source,
                    font: Font::from_wire(w.font).unwrap_or(Font::Px14),
                    align: Align::from_wire(w.align).unwrap_or(Align::Left),
                    x: w.x,
                    y: w.y,
                    w: w.w,
                    h: w.h,
                    fg: w.fg,
                    bg: w.bg,
                    accent: w.accent,
                    radius: w.radius,
                    decimals: w.decimals,
                    flags: w.flags,
                    label: c_str(&w.label),
                    suffix: c_str(&w.suffix),
                })
                .collect(),
        }
    }
}

/// Built-in layout as the shared model
pub fn default_model(screen: opad_layout::Screen) -> opad_layout::Layout {
    let raw = default_layout(screen.to_wire()).expect("firmware defines every screen");
    (&raw).into()
}

/// Render a model layout; `None` if the firmware validator rejects it
pub fn render_model(layout: &opad_layout::Layout) -> Option<Vec<u8>> {
    render(&layout.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_render() {
        for screen in [SCREEN_IDLE, SCREEN_PLAYING] {
            let layout = default_layout(screen).expect("default layout");
            let rgba = render(&layout).expect("valid default layout");
            // Background is not uniform: something was drawn
            assert!(rgba.chunks(4).any(|px| px != &rgba[0..4]));
        }
    }

    #[test]
    fn source_ids_match_firmware() {
        use opad_model::ui_source::ALL;
        for (id, name) in ALL {
            assert_eq!(source_name(*id).as_deref(), Some(*name), "source {}", id);
        }
        let defined = (1..=u8::MAX)
            .filter(|id| source_name(*id).is_some())
            .count();
        assert_eq!(
            defined,
            ALL.len(),
            "firmware defines sources missing from opad_model::ui_source"
        );
    }

    #[test]
    fn model_round_trip_matches_defaults() {
        for screen in opad_layout::Screen::ALL {
            let model = default_model(*screen);
            assert!(
                model.validate().is_ok(),
                "default {:?} passes Rust validation",
                screen
            );
            assert_eq!(
                RawLayout::from(&model),
                default_layout(screen.to_wire()).unwrap()
            );
        }
    }

    #[test]
    fn invalid_layout_is_rejected() {
        let mut layout = default_layout(SCREEN_IDLE).unwrap();
        layout.widgets[0].kind = 200;
        assert!(render(&layout).is_none());
    }
}
