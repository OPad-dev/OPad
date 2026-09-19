//! Screen designer: edit the pad's idle and playing layouts on a pixel-exact preview
//! (the firmware's own LVGL UI, compiled into `opad-ui-preview`) and push them to the pad.

mod overlay;

use crate::theme;
use iced::widget::{
    button, canvas, checkbox, column, container, image, pick_list, row, scrollable, stack, text,
    text_input, Space,
};
use iced::{keyboard, Alignment, Color, Element, Length, Subscription, Task};
use iced_aw::helpers::{color_picker, number_input};
use opad_ipc::{IpcRequest, IpcResponse};
use opad_layout::{
    all_sources, source_info, Align, Font, Layout, Screen, SourceInfo, Widget, WidgetKind,
    DECIMALS_DEFAULT, FLAG_BG_FILL, FLAG_BORDER, FLAG_HIDE_WHEN_EMPTY, MAX_WIDGETS,
};
use opad_model::ui_source::{self, SourceValue};
use opad_ui_preview as preview;
use std::time::Duration;

/// Preview zoom (pad pixels -> screen pixels)
pub const SCALE: f32 = 2.0;
/// Resize handle size in pad pixels
pub const HANDLE: i16 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorField {
    Fg,
    Bg,
    Accent,
    Background,
}

/// Decimal places choice for the property panel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decimals(u8);

impl std::fmt::Display for Decimals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == DECIMALS_DEFAULT {
            f.write_str("Source default")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

const DECIMAL_CHOICES: [Decimals; 6] = [
    Decimals(DECIMALS_DEFAULT),
    Decimals(0),
    Decimals(1),
    Decimals(2),
    Decimals(3),
    Decimals(4),
];

#[derive(Debug, Clone)]
pub enum Message {
    Loaded(Result<(Option<Layout>, Option<Layout>), String>),
    Reload,
    SelectScreen(Screen),
    Select(usize),
    PointerDown { x: f32, y: f32 },
    PointerMove { x: f32, y: f32 },
    PointerUp,
    Nudge(i16, i16),
    Add(WidgetKind),
    Delete,
    Duplicate,
    Raise,
    Lower,
    Kind(WidgetKind),
    Source(SourceInfo),
    Font(Font),
    Align(Align),
    X(i16),
    Y(i16),
    W(i16),
    H(i16),
    Radius(u8),
    Decimals(Decimals),
    Flag(u8, bool),
    Label(String),
    Suffix(String),
    OpenColor(ColorField),
    CancelColor,
    SubmitColor(Color),
    Hex(ColorField, String),
    LiveData(bool),
    PollLive,
    LiveValues(Result<Vec<(u8, SourceValue)>, String>),
    SimulatePress(bool),
    Apply,
    Applied(Result<String, String>),
    Revert,
    ResetDefault,
    Export,
    Exported(Result<String, String>),
    Import,
    Imported(Result<Layout, String>),
}

#[derive(Debug, Clone, Copy)]
enum Drag {
    Move { index: usize, dx: f32, dy: f32 },
    Resize { index: usize },
}

pub struct Designer {
    screen: Screen,
    working: [Layout; 2],
    /// What the pad shows (saved custom layout, or the built-in default)
    applied: [Layout; 2],
    defaults: [Layout; 2],
    selected: Option<usize>,
    drag: Option<Drag>,
    picker: Option<ColorField>,
    hex: [String; 4],
    preview: image::Handle,
    live: bool,
    press: bool,
    busy: bool,
    pub status: String,
}

fn idx(screen: Screen) -> usize {
    screen.to_wire() as usize
}

fn color(rgb: u32) -> Color {
    Color::from_rgb8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

fn rgb(color: Color) -> u32 {
    let [r, g, b, _] = color.into_rgba8();
    (r as u32) << 16 | (g as u32) << 8 | b as u32
}

fn hex(rgb: u32) -> String {
    format!("#{:06X}", rgb & 0xFF_FFFF)
}

fn parse_hex(s: &str) -> Option<u32> {
    let h = s.trim().trim_start_matches('#');
    (h.len() == 6)
        .then(|| u32::from_str_radix(h, 16).ok())
        .flatten()
}

impl Designer {
    pub fn new() -> (Self, Task<Message>) {
        let defaults = [
            preview::default_model(Screen::Idle),
            preview::default_model(Screen::Playing),
        ];
        preview::set_values(preview::sample_values().iter().map(|(s, v)| (*s, v)));
        let mut designer = Designer {
            screen: Screen::Playing,
            working: defaults.clone(),
            applied: defaults.clone(),
            defaults,
            selected: None,
            drag: None,
            picker: None,
            hex: Default::default(),
            preview: image::Handle::from_rgba(1, 1, vec![0; 4]),
            live: false,
            press: false,
            busy: true,
            status: "Loading layouts from the daemon...".into(),
        };
        designer.refresh();
        (designer, Task::perform(load_layouts(), Message::Loaded))
    }

    pub fn reload(&mut self) -> Task<Message> {
        self.busy = true;
        self.status = "Loading layouts from the daemon...".into();
        Task::perform(load_layouts(), Message::Loaded)
    }

    fn layout(&self) -> &Layout {
        &self.working[idx(self.screen)]
    }

    fn layout_mut(&mut self) -> &mut Layout {
        &mut self.working[idx(self.screen)]
    }

    fn widget_mut(&mut self) -> Option<&mut Widget> {
        let i = self.selected?;
        self.layout_mut().widgets.get_mut(i)
    }

    fn dirty(&self) -> bool {
        self.working[idx(self.screen)] != self.applied[idx(self.screen)]
    }

    /// Re-render the preview and resync color text fields after any change
    fn refresh(&mut self) {
        let layout = self.layout().clone();
        if let Some(rgba) = preview::render_model(&layout) {
            self.preview =
                image::Handle::from_rgba(preview::SCREEN_W as u32, preview::SCREEN_H as u32, rgba);
        }
        if let Some(w) = self.selected.and_then(|i| layout.widgets.get(i)) {
            self.hex[0] = hex(w.fg);
            self.hex[1] = hex(w.bg);
            self.hex[2] = hex(w.accent);
        }
        self.hex[3] = hex(layout.background);
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Reload => {
                return self.reload();
            }
            Message::Loaded(result) => {
                self.busy = false;
                match result {
                    Ok((idle, playing)) => {
                        for (screen, saved) in [(Screen::Idle, idle), (Screen::Playing, playing)] {
                            let layout =
                                saved.unwrap_or_else(|| self.defaults[idx(screen)].clone());
                            self.applied[idx(screen)] = layout.clone();
                            self.working[idx(screen)] = layout;
                        }
                        self.status =
                            "Drag widgets to move them, drag the corner handle to resize.".into();
                    }
                    Err(e) => {
                        self.status = format!("Daemon unavailable ({}); editing the defaults", e)
                    }
                }
                self.selected = None;
            }
            Message::SelectScreen(screen) => {
                self.screen = screen;
                self.selected = None;
            }
            Message::Select(i) => self.selected = Some(i),
            Message::PointerDown { x, y } => self.pointer_down(x, y),
            Message::PointerMove { x, y } => self.pointer_move(x, y),
            Message::PointerUp => self.drag = None,
            Message::Nudge(dx, dy) => {
                if let Some(w) = self.widget_mut() {
                    w.x = w.x.saturating_add(dx);
                    w.y = w.y.saturating_add(dy);
                }
            }
            Message::Add(kind) => {
                if self.layout().widgets.len() >= MAX_WIDGETS {
                    self.status = format!("A screen holds at most {} widgets", MAX_WIDGETS);
                } else {
                    self.layout_mut().widgets.push(Widget::new(kind));
                    self.selected = Some(self.layout().widgets.len() - 1);
                }
            }
            Message::Delete => {
                if let Some(i) = self.selected.take() {
                    self.layout_mut().widgets.remove(i);
                }
            }
            Message::Duplicate => {
                if let Some(i) = self.selected {
                    if self.layout().widgets.len() < MAX_WIDGETS {
                        let mut copy = self.layout().widgets[i].clone();
                        copy.x += 8;
                        copy.y += 8;
                        self.layout_mut().widgets.push(copy);
                        self.selected = Some(self.layout().widgets.len() - 1);
                    }
                }
            }
            Message::Raise => {
                if let Some(i) = self
                    .selected
                    .filter(|i| i + 1 < self.layout().widgets.len())
                {
                    self.layout_mut().widgets.swap(i, i + 1);
                    self.selected = Some(i + 1);
                }
            }
            Message::Lower => {
                if let Some(i) = self.selected.filter(|i| *i > 0) {
                    self.layout_mut().widgets.swap(i, i - 1);
                    self.selected = Some(i - 1);
                }
            }
            Message::Kind(kind) => self.edit(|w| w.kind = kind),
            Message::Source(source) => self.edit(|w| w.source = source.id),
            Message::Font(font) => self.edit(|w| w.font = font),
            Message::Align(align) => self.edit(|w| w.align = align),
            Message::X(v) => self.edit(|w| w.x = v),
            Message::Y(v) => self.edit(|w| w.y = v),
            Message::W(v) => self.edit(|w| w.w = v.max(1)),
            Message::H(v) => self.edit(|w| w.h = v.max(1)),
            Message::Radius(v) => self.edit(|w| w.radius = v),
            Message::Decimals(d) => self.edit(|w| w.decimals = d.0),
            Message::Flag(flag, on) => self.edit(|w| w.set_flag(flag, on)),
            Message::Label(s) => {
                let s = truncate(&s, opad_layout::LABEL_MAX_BYTES);
                self.edit(|w| w.label = s);
            }
            Message::Suffix(s) => {
                let s = truncate(&s, opad_layout::SUFFIX_MAX_BYTES);
                self.edit(|w| w.suffix = s);
            }
            Message::OpenColor(field) => self.picker = Some(field),
            Message::CancelColor => self.picker = None,
            Message::SubmitColor(c) => {
                if let Some(field) = self.picker.take() {
                    self.set_color(field, rgb(c));
                }
            }
            Message::Hex(field, s) => {
                if let Some(value) = parse_hex(&s) {
                    self.set_color(field, value);
                }
                self.hex[field as usize] = s;
                return Task::none();
            }
            Message::LiveData(on) => {
                self.live = on;
                if on {
                    return Task::perform(fetch_values(), Message::LiveValues);
                }
                preview::set_values(preview::sample_values().iter().map(|(s, v)| (*s, v)));
            }
            Message::PollLive => {
                if self.live {
                    return Task::perform(fetch_values(), Message::LiveValues);
                }
                return Task::none();
            }
            Message::LiveValues(result) => match result {
                Ok(values) if !values.is_empty() => {
                    let converted: Vec<(u8, preview::Value)> = values
                        .into_iter()
                        .map(|(s, v)| {
                            (
                                s,
                                match v {
                                    SourceValue::Number(n) => preview::Value::Number(n),
                                    SourceValue::Text(t) => preview::Value::Text(t),
                                    SourceValue::Clear => preview::Value::Empty,
                                },
                            )
                        })
                        .collect();
                    preview::set_values(converted.iter().map(|(s, v)| (*s, v)));
                }
                Ok(_) => {
                    self.status =
                        "No live data yet (is tosu running?); showing sample values".into()
                }
                Err(e) => self.status = format!("Live data unavailable: {}", e),
            },
            Message::SimulatePress(on) => {
                self.press = on;
                let v = preview::Value::Number(if on { 1.0 } else { 0.0 });
                preview::set_values([(ui_source::PAD_K1_DOWN, &v), (ui_source::PAD_K2_DOWN, &v)]);
            }
            Message::Apply => {
                if let Err(e) = self.layout().validate() {
                    self.status = format!("Can't apply: {}", e);
                } else {
                    self.busy = true;
                    self.status = "Applying to the pad...".into();
                    let (screen, layout) = (self.screen, self.layout().clone());
                    return Task::perform(apply_layout(screen, layout), Message::Applied);
                }
            }
            Message::Applied(result) => {
                self.busy = false;
                match result {
                    Ok(note) => {
                        self.applied[idx(self.screen)] = self.layout().clone();
                        self.status = if note.is_empty() {
                            "Applied to the pad".into()
                        } else {
                            format!("Applied: {}", note)
                        };
                    }
                    Err(e) => self.status = e,
                }
            }
            Message::Revert => {
                self.working[idx(self.screen)] = self.applied[idx(self.screen)].clone();
                self.selected = None;
                self.status = "Reverted to what the pad shows".into();
            }
            Message::ResetDefault => {
                self.working[idx(self.screen)] = self.defaults[idx(self.screen)].clone();
                self.selected = None;
                self.status =
                    "Loaded the built-in default; press Apply to use it on the pad".into();
            }
            Message::Export => {
                let (screen, layout) = (self.screen, self.layout().clone());
                return Task::perform(export_layout(screen, layout), Message::Exported);
            }
            Message::Exported(result) => match result {
                Ok(path) => self.status = format!("Exported to {}", path),
                Err(e) => self.status = e,
            },
            Message::Import => return Task::perform(import_layout(), Message::Imported),
            Message::Imported(result) => match result {
                Ok(layout) => {
                    *self.layout_mut() = layout;
                    self.selected = None;
                    self.status = "Imported; press Apply to use it on the pad".into();
                }
                Err(e) if e.is_empty() => {}
                Err(e) => self.status = e,
            },
        }
        self.refresh();
        Task::none()
    }

    fn edit(&mut self, f: impl FnOnce(&mut Widget)) {
        if let Some(w) = self.widget_mut() {
            f(w);
        }
    }

    fn set_color(&mut self, field: ColorField, value: u32) {
        match field {
            ColorField::Background => self.layout_mut().background = value,
            ColorField::Fg => self.edit(|w| w.fg = value),
            ColorField::Bg => self.edit(|w| w.bg = value),
            ColorField::Accent => self.edit(|w| w.accent = value),
        }
        self.refresh();
    }

    fn pointer_down(&mut self, x: f32, y: f32) {
        let widgets = &self.layout().widgets;
        if let Some(i) = self.selected.filter(|i| *i < widgets.len()) {
            let w = &widgets[i];
            let (hx, hy) = ((w.x + w.w) as f32, (w.y + w.h) as f32);
            if (x - hx).abs() <= HANDLE as f32 && (y - hy).abs() <= HANDLE as f32 {
                self.drag = Some(Drag::Resize { index: i });
                return;
            }
        }
        // Topmost widget under the cursor
        let hit = widgets
            .iter()
            .enumerate()
            .rev()
            .find(|(_, w)| w.contains(x as i16, y as i16))
            .map(|(i, w)| (i, x - w.x as f32, y - w.y as f32));
        self.selected = hit.map(|(i, _, _)| i);
        self.drag = hit.map(|(index, dx, dy)| Drag::Move { index, dx, dy });
    }

    fn pointer_move(&mut self, x: f32, y: f32) {
        let Some(drag) = self.drag else { return };
        let clamp =
            |v: f32, lo: i16, hi: i16| (v.round() as i32).clamp(lo as i32, hi as i32) as i16;
        match drag {
            Drag::Move { index, dx, dy } => {
                if let Some(w) = self.layout_mut().widgets.get_mut(index) {
                    w.x = clamp(x - dx, -w.w + 4, opad_layout::SCREEN_W - 4);
                    w.y = clamp(y - dy, -w.h + 4, opad_layout::SCREEN_H - 4);
                }
            }
            Drag::Resize { index } => {
                if let Some(w) = self.layout_mut().widgets.get_mut(index) {
                    w.w = clamp(x - w.x as f32, 2, 2 * opad_layout::SCREEN_W);
                    w.h = clamp(y - w.y as f32, 2, 2 * opad_layout::SCREEN_H);
                }
            }
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let keys = keyboard::listen().filter_map(|event| {
            let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
                return None;
            };
            let step = if modifiers.shift() { 10 } else { 1 };
            match key {
                keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                    Some(Message::Nudge(-step, 0))
                }
                keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                    Some(Message::Nudge(step, 0))
                }
                keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                    Some(Message::Nudge(0, -step))
                }
                keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                    Some(Message::Nudge(0, step))
                }
                keyboard::Key::Named(keyboard::key::Named::Delete) => Some(Message::Delete),
                _ => None,
            }
        });
        if self.live {
            Subscription::batch([
                keys,
                iced::time::every(Duration::from_secs(1)).map(|_| Message::PollLive),
            ])
        } else {
            keys
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let layout = self.layout();

        let screens = row(Screen::ALL.iter().map(|s| {
            button(text(format!("{} screen", s)))
                .style(if *s == self.screen {
                    theme::primary
                } else {
                    theme::secondary
                })
                .on_press(Message::SelectScreen(*s))
                .into()
        }))
        .spacing(8);

        let (w, h) = (
            preview::SCREEN_W as f32 * SCALE,
            preview::SCREEN_H as f32 * SCALE,
        );
        let overlay = overlay::Overlay {
            boxes: layout
                .widgets
                .iter()
                .map(|w| (w.x, w.y, w.w, w.h))
                .collect(),
            selected: self.selected,
        };
        let canvas_area = stack![
            image(self.preview.clone())
                .width(w)
                .height(h)
                .filter_method(image::FilterMethod::Nearest),
            canvas(overlay).width(w).height(h),
        ];

        let actions = row![
            button(text(if self.dirty() {
                "Apply to pad •"
            } else {
                "Apply to pad"
            }))
            .style(theme::primary)
            .on_press_maybe((!self.busy).then_some(Message::Apply)),
            button("Revert")
                .style(theme::secondary)
                .on_press(Message::Revert),
            button("Reset to default")
                .style(theme::secondary)
                .on_press(Message::ResetDefault),
            button("Export")
                .style(theme::secondary)
                .on_press(Message::Export),
            button("Import")
                .style(theme::secondary)
                .on_press(Message::Import),
            button("Reload")
                .style(theme::secondary)
                .on_press(Message::Reload),
        ]
        .spacing(8);

        let toggles = row![
            checkbox(self.live)
                .label("Live data from osu!")
                .on_toggle(Message::LiveData),
            checkbox(self.press)
                .label("Show keys pressed")
                .on_toggle(Message::SimulatePress),
        ]
        .spacing(20);

        let left = column![
            row![theme::heading("Designer"), Space::new().width(20), screens]
                .spacing(12)
                .align_y(Alignment::Center),
            container(canvas_area).style(theme::card).padding(6),
            theme::muted(&self.status).size(13),
            actions,
            toggles,
            theme::caption("Arrow keys nudge the selection (Shift = 10 px), Delete removes it."),
        ]
        .spacing(10)
        .width(Length::Shrink);

        let right = column![
            container(self.palette())
                .padding(16)
                .width(Length::Fill)
                .style(theme::card),
            container(self.widget_list())
                .padding(16)
                .width(Length::Fill)
                .style(theme::card),
            container(self.properties())
                .padding(16)
                .width(Length::Fill)
                .style(theme::card),
        ]
        .spacing(12)
        .width(Length::Fill);

        row![left, scrollable(right).height(Length::Fill)]
            .spacing(16)
            .into()
    }

    fn palette(&self) -> Element<'_, Message> {
        column![
            text("Add widget").size(16).font(theme::FONT_BOLD),
            row(WidgetKind::ALL
                .iter()
                .map(|k| button(text(k.label()).size(12))
                    .padding([5, 10])
                    .style(theme::secondary)
                    .on_press(Message::Add(*k))
                    .into()))
            .spacing(6)
            .wrap(),
        ]
        .spacing(6)
        .into()
    }

    fn widget_list(&self) -> Element<'_, Message> {
        let layout = self.layout();
        let items = column(layout.widgets.iter().enumerate().rev().map(|(i, w)| {
            let source = source_info(w.source).map_or("?".to_string(), |s| s.label());
            let what = if w.source == 0 {
                format!("\"{}\"", w.label)
            } else {
                source
            };
            button(text(format!("{}  ·  {}", w.kind.label(), what)).size(12))
                .width(Length::Fill)
                .padding([3, 8])
                .style(theme::list_item(Some(i) == self.selected))
                .on_press(Message::Select(i))
                .into()
        }))
        .spacing(2);

        column![
            row![
                text(format!(
                    "Widgets ({}/{})",
                    layout.widgets.len(),
                    MAX_WIDGETS
                ))
                .size(16)
                .font(theme::FONT_BOLD),
                Space::new().width(Length::Fill),
                theme::caption("top of list = drawn on top").size(11),
            ]
            .align_y(Alignment::Center),
            scrollable(items).height(170),
            row![
                button(text("Raise").size(12))
                    .style(theme::secondary)
                    .on_press_maybe(self.selected.map(|_| Message::Raise)),
                button(text("Lower").size(12))
                    .style(theme::secondary)
                    .on_press_maybe(self.selected.map(|_| Message::Lower)),
                button(text("Duplicate").size(12))
                    .style(theme::secondary)
                    .on_press_maybe(self.selected.map(|_| Message::Duplicate)),
                button(text("Delete").size(12))
                    .style(theme::danger)
                    .on_press_maybe(self.selected.map(|_| Message::Delete)),
            ]
            .spacing(6),
        ]
        .spacing(6)
        .into()
    }

    fn color_row(
        &self,
        label: &'static str,
        field: ColorField,
        value: u32,
    ) -> Element<'_, Message> {
        let swatch = button(Space::new().width(28).height(18))
            .style(move |_theme, _status| button::Style {
                background: Some(color(value).into()),
                border: iced::Border {
                    color: Color::WHITE,
                    width: 1.0,
                    radius: 3.0.into(),
                },
                ..Default::default()
            })
            .on_press(Message::OpenColor(field));
        row![
            theme::muted(label).size(13).width(90),
            color_picker(
                self.picker == Some(field),
                color(value),
                swatch,
                Message::CancelColor,
                Message::SubmitColor
            ),
            text_input("#RRGGBB", &self.hex[field as usize])
                .on_input(move |s| Message::Hex(field, s))
                .width(100),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    }

    fn properties(&self) -> Element<'_, Message> {
        let layout = self.layout();
        let background = self.color_row("Screen bg", ColorField::Background, layout.background);

        let Some(w) = self.selected.and_then(|i| layout.widgets.get(i)) else {
            return column![
                text("Properties").size(16).font(theme::FONT_BOLD),
                background,
                theme::muted("Select a widget to edit it.").size(13)
            ]
            .spacing(8)
            .into();
        };

        let field =
            |label: &'static str, input: Element<'static, Message>| -> Element<'static, Message> {
                row![theme::muted(label).size(13).width(90), input]
                    .spacing(8)
                    .align_y(Alignment::Center)
                    .into()
            };
        let sources = all_sources();
        let current_source = source_info(w.source);

        column![
            text("Properties").size(16).font(theme::FONT_BOLD),
            field(
                "Type",
                pick_list(WidgetKind::ALL, Some(w.kind), Message::Kind).into()
            ),
            field(
                "Data",
                pick_list(sources, current_source, Message::Source)
                    .width(Length::Fill)
                    .into()
            ),
            field(
                "Font",
                pick_list(Font::ALL, Some(w.font), Message::Font).into()
            ),
            field(
                "Align",
                pick_list(Align::ALL, Some(w.align), Message::Align).into()
            ),
            row![
                field(
                    "X",
                    number_input(&w.x, -320..=640, Message::X).width(90).into()
                ),
                field(
                    "Y",
                    number_input(&w.y, -240..=480, Message::Y).width(90).into()
                ),
            ]
            .spacing(8),
            row![
                field(
                    "Width",
                    number_input(&w.w, 1..=640, Message::W).width(90).into()
                ),
                field(
                    "Height",
                    number_input(&w.h, 1..=480, Message::H).width(90).into()
                ),
            ]
            .spacing(8),
            field(
                "Radius",
                number_input(&w.radius, 0..=120, Message::Radius)
                    .width(90)
                    .into()
            ),
            field(
                "Decimals",
                pick_list(
                    DECIMAL_CHOICES,
                    Some(Decimals(w.decimals)),
                    Message::Decimals
                )
                .into()
            ),
            field(
                "Label",
                text_input("prefix / static text", &w.label)
                    .on_input(Message::Label)
                    .into()
            ),
            field(
                "Suffix",
                text_input("e.g. pp, %, x", &w.suffix)
                    .on_input(Message::Suffix)
                    .into()
            ),
            self.color_row("Text / fg", ColorField::Fg, w.fg),
            self.color_row("Background", ColorField::Bg, w.bg),
            self.color_row("Accent", ColorField::Accent, w.accent),
            checkbox(w.has_flag(FLAG_BG_FILL))
                .label("Fill background (text)")
                .on_toggle(|on| Message::Flag(FLAG_BG_FILL, on)),
            checkbox(w.has_flag(FLAG_BORDER))
                .label("Border")
                .on_toggle(|on| Message::Flag(FLAG_BORDER, on)),
            checkbox(w.has_flag(FLAG_HIDE_WHEN_EMPTY))
                .label("Hide when there is no data")
                .on_toggle(|on| Message::Flag(FLAG_HIDE_WHEN_EMPTY, on)),
            iced::widget::rule::horizontal(1),
            background,
            theme::muted(kind_help(w.kind)).size(12),
        ]
        .spacing(8)
        .into()
    }
}

fn kind_help(kind: WidgetKind) -> &'static str {
    match kind {
        WidgetKind::Text => "Shows Label + value + Suffix. With no data source it is static text.",
        WidgetKind::Progress => {
            "Background = track, Accent = fill. Use a 0-1 source such as map progress or health."
        }
        WidgetKind::KeyCard => {
            "Label is the title. Background at rest, Accent while the key is held (they swap)."
        }
        WidgetKind::StatusDot => {
            "Accent = connected, Background = disconnected, Text = label color."
        }
        WidgetKind::Rect => {
            "Background fill with Radius; Border draws a 1 px outline in Text color."
        }
        WidgetKind::Grade => "Shows the rank letter in osu!'s grade colors.",
    }
}

fn truncate(s: &str, max: usize) -> String {
    let mut end = s.len().min(max);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

// ---- daemon & file I/O ------------------------------------------------------------------

use crate::ipc::request as ipc;

async fn load_layouts() -> Result<(Option<Layout>, Option<Layout>), String> {
    match ipc(IpcRequest::GetLayouts).await? {
        IpcResponse::Layouts { idle, playing } => Ok((idle, playing)),
        other => Err(format!("unexpected response: {:?}", other)),
    }
}

async fn fetch_values() -> Result<Vec<(u8, SourceValue)>, String> {
    match ipc(IpcRequest::GetUiValues).await? {
        IpcResponse::UiValues(values) => Ok(values),
        other => Err(format!("unexpected response: {:?}", other)),
    }
}

async fn apply_layout(screen: Screen, layout: Layout) -> Result<String, String> {
    match ipc(IpcRequest::SetLayout { screen, layout }).await? {
        IpcResponse::LayoutApplied { message, .. } => Ok(message),
        IpcResponse::OperationRejected { reason } => Err(reason),
        IpcResponse::Error(e) => Err(e),
        other => Err(format!("unexpected response: {:?}", other)),
    }
}

async fn export_layout(screen: Screen, layout: Layout) -> Result<String, String> {
    let file = rfd::AsyncFileDialog::new()
        .set_file_name(format!("osupad-{}.json", screen.label().to_lowercase()))
        .add_filter("Layout", &["json"])
        .save_file()
        .await
        .ok_or_else(|| "Export cancelled".to_string())?;
    std::fs::write(file.path(), layout.to_json()).map_err(|e| format!("Export failed: {}", e))?;
    Ok(file.path().display().to_string())
}

async fn import_layout() -> Result<Layout, String> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .add_filter("Layout", &["json"])
        .pick_file()
        .await
    else {
        return Err(String::new());
    };
    let json = std::fs::read_to_string(file.path()).map_err(|e| format!("Import failed: {}", e))?;
    let layout = Layout::from_json(&json).map_err(|e| format!("Not a layout file: {}", e))?;
    layout
        .validate()
        .map_err(|e| format!("Invalid layout: {}", e))?;
    Ok(layout)
}
