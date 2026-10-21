//! Keys tab: Hall Effect actuation and rapid trigger settings, with a live
//! cut-away of each switch. Shown only for a pad whose HE backend is running
//! (HelloAck.he_calibration). Travel is a share of the key's calibrated
//! magnetic span, never millimetres (decision record D3), so every value here
//! is a percentage.

use crate::theme::{self, caption, heading, muted};
use crate::{ipc, App, Message};
use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::widget::{
    button, canvas as canvas_widget, column, container, row, slider, text, toggler,
};
use iced::widget::{scrollable, Space};
use iced::{
    Alignment, Color, Element, Length, Pixels, Point, Rectangle, Renderer, Size, Task, Theme,
};
use opad_ipc::{IpcRequest, IpcResponse};
use opad_model::{DeviceConfig, HeCalibrationStatus, HeKeyConfig, HeLiveSample, HeRtMode};

/// One setting of [`HeKeyConfig`], in the pad's 0.1 % units
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Actuation,
    /// Simple mode: press and release sensitivity together
    Sensitivity,
    RtPress,
    RtRelease,
    ReleaseHyst,
    TopReset,
    BottomDeadZone,
}

#[derive(Debug, Clone)]
pub enum KeysMessage {
    /// `true`: Key 1 and Key 2 get their own settings
    Separate(bool),
    ToggleAdvanced,
    Set(usize, Field, u32),
    RapidTrigger(usize, bool),
    Continuous(usize, bool),
    Save,
    Revert,
    PollLive,
    Live(Box<Result<IpcResponse, String>>),
    /// The window is about to redraw: move the drawn stems on
    Frame(iced::time::Instant),
}

#[derive(Debug, Default)]
pub struct KeysState {
    separate: bool,
    advanced: bool,
    /// Edited values, not yet saved. `None` until the pad's are adopted.
    edit: Option<[HeKeyConfig; 2]>,
    dirty: bool,
    live: Option<HeLiveSample>,
    /// What the live view draws: each key's reading through a One Euro
    /// filter. Display only; the pad decides presses from raw readings.
    smooth: [OneEuro; 2],
    /// Where the stems are drawn: glides towards `smooth` every frame, so
    /// the motion has no steps between the ~50 readings a second
    drawn: [Option<f32>; 2],
    /// The filtered reading with jitter suppression: moves only once the
    /// reading leaves a dead zone around it, so a still key stays still
    held: [Option<f32>; 2],
    last_frame: Option<iced::time::Instant>,
    live_supported: bool,
    live_inflight: bool,
}

fn pad_settings(config: &DeviceConfig) -> [HeKeyConfig; 2] {
    [
        config.key1_he.unwrap_or_default(),
        config.key2_he.unwrap_or_default(),
    ]
}

impl KeysState {
    /// Follow the pad's saved settings unless there are unsaved edits
    pub fn adopt(&mut self, config: &DeviceConfig) {
        if self.dirty {
            return;
        }
        let pad = pad_settings(config);
        if self.edit.is_none() {
            self.separate = pad[0] != pad[1];
        }
        self.edit = Some(pad);
    }

    /// Leaving the page or losing the pad: the next live view starts fresh
    pub fn stop_live(&mut self) {
        self.live = None;
        self.live_inflight = false;
        self.smooth = Default::default();
        self.drawn = [None; 2];
        self.held = [None; 2];
        self.last_frame = None;
    }

    /// Ease each drawn stem towards its filtered reading, by real time
    fn advance(&mut self, now: iced::time::Instant) {
        let dt = self
            .last_frame
            .map_or(0.0, |t| now.saturating_duration_since(t).as_secs_f32());
        self.last_frame = Some(now);
        // Most of the way in ~3 time constants: fast enough to keep up with
        // a press, slow enough to hide the 20 ms steps between readings
        let follow = 1.0 - (-dt / DRAW_TAU_S).exp();
        for k in 0..2 {
            self.held[k] = match (self.held[k], self.smooth[k].value) {
                (Some(h), Some(v)) => Some(hold(h, v)),
                (_, v) => v,
            };
            self.drawn[k] = match (self.drawn[k], self.held[k]) {
                (Some(d), Some(t)) => Some(d + (t - d) * follow),
                (_, t) => t,
            };
        }
    }

    fn take_sample(&mut self, sample: Option<HeLiveSample>, now: std::time::Instant) {
        self.live = sample;
        match sample {
            Some(s) => {
                for (k, raw) in [s.key1_raw, s.key2_raw].into_iter().enumerate() {
                    if raw > 0 {
                        self.smooth[k].filter(raw as f32, now);
                    } else {
                        self.smooth[k] = OneEuro::default();
                    }
                }
            }
            // Paused (a map) or gone: start fresh rather than glide from a stale value
            None => self.smooth = Default::default(),
        }
    }
}

/// One Euro filter (Casiez, Roussel & Vogel, CHI 2012), as pen tablets use:
/// the cutoff rises with speed, so a still key loses its sensor noise while
/// a fast press is followed with almost no lag. Tuned in raw counts: rest
/// noise is ~5-6 counts sd (decision record, V1.1 bench), a fast press moves
/// ~450 counts in a few tens of ms.
/// Time constant of the drawn stem's glide towards the filtered reading
const DRAW_TAU_S: f32 = 0.025;

/// Jitter suppression, in raw counts (~1.3 % of a 450-count stroke): what
/// the One Euro filter lets through at rest stays inside it, so a still key
/// is drawn perfectly still. Smaller movements are not drawn; the pad still
/// sees them.
const HOLD_COUNTS: f32 = 6.0;

/// Move the held value only as far as needed to keep the reading within
/// HOLD_COUNTS of it: no steps, and nothing at all inside the dead zone
fn hold(held: f32, value: f32) -> f32 {
    if value > held + HOLD_COUNTS {
        value - HOLD_COUNTS
    } else if value < held - HOLD_COUNTS {
        value + HOLD_COUNTS
    } else {
        held
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct OneEuro {
    value: Option<f32>,
    speed: f32,
    at: Option<std::time::Instant>,
}

impl OneEuro {
    /// Cutoff for a still key, Hz: low, so the noise is averaged away
    const MIN_CUTOFF: f32 = 0.5;
    /// Extra cutoff per count/s of movement
    const BETA: f32 = 0.004;
    /// Cutoff for the speed estimate itself, Hz: low, so noise does not
    /// read as movement and open the filter up
    const SPEED_CUTOFF: f32 = 0.7;

    fn alpha(cutoff: f32, dt: f32) -> f32 {
        let tau = 1.0 / (2.0 * std::f32::consts::PI * cutoff);
        1.0 / (1.0 + tau / dt)
    }

    fn filter(&mut self, x: f32, now: std::time::Instant) -> f32 {
        let (Some(prev), Some(at)) = (self.value, self.at) else {
            self.value = Some(x);
            self.at = Some(now);
            return x;
        };
        let dt = now.duration_since(at).as_secs_f32().max(1e-3);
        let speed = (x - prev) / dt;
        self.speed += Self::alpha(Self::SPEED_CUTOFF, dt) * (speed - self.speed);
        let cutoff = Self::MIN_CUTOFF + Self::BETA * self.speed.abs();
        let v = prev + Self::alpha(cutoff, dt) * (x - prev);
        self.value = Some(v);
        self.at = Some(now);
        v
    }
}

pub fn update(app: &mut App, msg: KeysMessage) -> Task<Message> {
    let pad = pad_settings(&app.config);
    match msg {
        KeysMessage::Save => {
            if !app.device_connected {
                app.banner = Some("Cannot save: the pad is not connected.".into());
                return Task::none();
            }
            let [k1, k2] = app.keys.edit.unwrap_or(pad);
            let config = DeviceConfig {
                key1_he: Some(k1),
                key2_he: Some(k2),
                ..app.config.clone()
            };
            app.keys.dirty = false;
            app.banner = Some("Saving key settings...".into());
            Task::perform(
                ipc::request(IpcRequest::UpdateConfig(config)),
                Message::ActionDone,
            )
        }
        KeysMessage::PollLive => {
            if app.keys.live_inflight || !app.device_connected {
                return Task::none();
            }
            app.keys.live_inflight = true;
            Task::perform(ipc::request(IpcRequest::GetHeLive), |r| {
                Message::Keys(KeysMessage::Live(Box::new(r)))
            })
        }
        KeysMessage::Live(result) => {
            let state = &mut app.keys;
            state.live_inflight = false;
            match *result {
                Ok(IpcResponse::HeLive { supported, sample }) => {
                    state.live_supported = supported;
                    state.take_sample(sample, std::time::Instant::now());
                }
                // An older daemon closes the connection on GetHeLive
                _ => {
                    state.live_supported = false;
                    state.take_sample(None, std::time::Instant::now());
                }
            }
            Task::none()
        }
        KeysMessage::Frame(now) => {
            app.keys.advance(now);
            Task::none()
        }
        edit_msg => {
            app.keys.edit_settings(edit_msg, pad);
            Task::none()
        }
    }
}

impl KeysState {
    fn edit_settings(&mut self, msg: KeysMessage, pad: [HeKeyConfig; 2]) {
        let separate = self.separate;
        let edit = self.edit.get_or_insert(pad);
        // Linked: every change goes to both keys
        let apply = |edit: &mut [HeKeyConfig; 2], key: usize, f: &dyn Fn(&mut HeKeyConfig)| {
            if separate {
                f(&mut edit[key]);
            } else {
                edit.iter_mut().for_each(f);
            }
        };
        match msg {
            KeysMessage::Separate(on) => {
                self.separate = on;
                if !on && edit[0] != edit[1] {
                    edit[1] = edit[0];
                    self.dirty = true;
                }
            }
            KeysMessage::ToggleAdvanced => self.advanced = !self.advanced,
            KeysMessage::Set(key, field, v) => {
                apply(edit, key, &|c| match field {
                    Field::Actuation => c.actuation = v,
                    Field::Sensitivity => {
                        c.rt_press = v;
                        c.rt_release = v;
                    }
                    Field::RtPress => c.rt_press = v,
                    Field::RtRelease => c.rt_release = v,
                    Field::ReleaseHyst => c.release_hyst = v,
                    Field::TopReset => c.top_reset = v,
                    Field::BottomDeadZone => c.bottom_dead_zone = v,
                });
                self.dirty = true;
            }
            KeysMessage::RapidTrigger(key, on) => {
                apply(edit, key, &|c| {
                    c.rt_mode = match (on, c.rt_mode) {
                        (false, _) => HeRtMode::Off,
                        (true, HeRtMode::Off) => HeRtMode::Normal,
                        (true, mode) => mode,
                    }
                });
                self.dirty = true;
            }
            KeysMessage::Continuous(key, on) => {
                apply(edit, key, &|c| {
                    if c.rt_mode != HeRtMode::Off {
                        c.rt_mode = if on {
                            HeRtMode::Continuous
                        } else {
                            HeRtMode::Normal
                        };
                    }
                });
                self.dirty = true;
            }
            KeysMessage::Revert => {
                self.dirty = false;
                self.edit = Some(pad);
                self.separate = pad[0] != pad[1];
            }
            KeysMessage::Save
            | KeysMessage::PollLive
            | KeysMessage::Live(_)
            | KeysMessage::Frame(_) => {}
        }
    }
}

fn pct(v: u32) -> String {
    format!("{:.1} %", v as f32 / 10.0)
}

fn setting<'a>(
    label: &'a str,
    hint: &'a str,
    value: u32,
    range: std::ops::RangeInclusive<u32>,
    on_change: impl Fn(u32) -> Message + 'a,
) -> Element<'a, Message> {
    column![
        row![
            caption(label),
            Space::new().width(Length::Fill),
            text(pct(value)).size(14)
        ],
        slider(range, value, on_change).step(5u32),
        muted(hint).size(12),
    ]
    .spacing(6)
    .into()
}

fn key_controls<'a>(app: &'a App, key: usize, c: HeKeyConfig) -> Element<'a, Message> {
    let set = move |field: Field| move |v| Message::Keys(KeysMessage::Set(key, field, v));
    let rt_on = c.rt_mode != HeRtMode::Off;
    let mut col = column![setting(
        "ACTUATION POINT",
        "How deep the key goes before it presses.",
        c.actuation,
        10..=950,
        set(Field::Actuation),
    )]
    .spacing(16);

    col = col.push(
        row![
            column![
                text("Rapid trigger").size(15),
                muted("Presses again as soon as the key moves back down, and releases as soon as it moves up.")
                    .size(12),
            ]
            .spacing(2)
            .width(Length::Fill),
            toggler(rt_on).on_toggle(move |on| Message::Keys(KeysMessage::RapidTrigger(key, on))),
        ]
        .align_y(Alignment::Center),
    );

    if rt_on {
        if !app.keys.advanced {
            col = col.push(setting(
                "SENSITIVITY",
                "How far the key has to move to press or release again.",
                c.rt_press,
                10..=500,
                set(Field::Sensitivity),
            ));
        }
        col = col.push(
            row![
                column![
                    text("Continuous").size(15),
                    muted("Rapid trigger keeps working all the way up to the top reset point.")
                        .size(12),
                ]
                .spacing(2)
                .width(Length::Fill),
                toggler(c.rt_mode == HeRtMode::Continuous)
                    .on_toggle(move |on| Message::Keys(KeysMessage::Continuous(key, on))),
            ]
            .align_y(Alignment::Center),
        );
    }

    if app.keys.advanced {
        if rt_on {
            col = col.push(setting(
                "PRESS SENSITIVITY",
                "Movement down that presses again.",
                c.rt_press,
                10..=500,
                set(Field::RtPress),
            ));
            col = col.push(setting(
                "RELEASE SENSITIVITY",
                "Movement up that releases.",
                c.rt_release,
                10..=500,
                set(Field::RtRelease),
            ));
        }
        col = col.push(setting(
            "RELEASE POINT",
            "Without rapid trigger, the key releases this far above the actuation point.",
            c.release_hyst,
            10..=500,
            set(Field::ReleaseHyst),
        ));
        if c.rt_mode == HeRtMode::Continuous {
            col = col.push(setting(
                "TOP RESET",
                "Continuous rapid trigger ends when the key comes up above this depth.",
                c.top_reset,
                10..=500,
                set(Field::TopReset),
            ));
        }
        col = col.push(setting(
            "BOTTOM DEAD ZONE",
            "Movement this close to a full press is ignored, so pressing harder into the bottom never counts. 0 = off.",
            c.bottom_dead_zone,
            0..=300,
            set(Field::BottomDeadZone),
        ));
    }
    col.into()
}

fn cal_span(cal: &HeCalibrationStatus, key: usize) -> Option<(u32, u32)> {
    let (ok, rest, full) = if key == 0 {
        (cal.key1_calibrated, cal.key1_rest, cal.key1_full)
    } else {
        (cal.key2_calibrated, cal.key2_rest, cal.key2_full)
    };
    (ok && full > rest).then_some((rest, full))
}

fn live_view<'a>(app: &'a App, key: usize, c: HeKeyConfig) -> Element<'a, Message> {
    let sample = app.keys.live;
    let span = app
        .he_calibration
        .as_ref()
        .and_then(|cal| cal_span(cal, key));
    // Pressed comes from the pad as is; only the drawn position is smoothed
    let down = match sample {
        Some(s) if key == 0 => s.key1_down,
        Some(s) => s.key2_down,
        None => false,
    };
    let shown = app.keys.drawn[key];
    let depth = match (span, shown) {
        (Some((rest, full)), Some(r)) => Some((r - rest as f32) / (full as f32 - rest as f32)),
        _ => None,
    };
    let status = if !app.keys.live_supported {
        "Live view needs a newer daemon and pad firmware"
    } else if sample.is_none() {
        "Waiting for the pad (paused during a map)"
    } else if depth.is_none() {
        "No reading"
    } else if down {
        "Pressed"
    } else {
        "Released"
    };
    column![
        canvas_widget(SwitchView {
            depth,
            down,
            settings: c,
        })
        .width(Length::Fixed(SWITCH_W * SWITCH_SCALE))
        .height(Length::Fixed(SWITCH_H * SWITCH_SCALE)),
        text(match depth {
            Some(d) => format!("{:.0} %", (d * 100.0).clamp(-99.0, 199.0)),
            None => "-".to_string(),
        })
        .size(16)
        .font(theme::FONT_BOLD),
        muted(status).size(12),
    ]
    .spacing(4)
    .align_x(Alignment::Center)
    .into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let mut content = column![
        heading("Keys"),
        muted("Where the keys press and release, and rapid trigger. Depths are a share of each key's calibrated travel. The moving switches are smoothed for display; the pad presses on raw readings."),
    ]
    .spacing(14);

    if !app.device_connected {
        return scrollable(content.push(card(text("Connect your OPad to change its keys."))))
            .into();
    }
    let calibrated = app
        .he_calibration
        .as_ref()
        .is_some_and(|c| c.key1_calibrated && c.key2_calibrated);
    if !calibrated {
        return scrollable(content.push(card(
            column![
                text("The keys are not calibrated").size(16).font(theme::FONT_BOLD),
                muted("Calibrate them first: the settings and the live view use each key's measured travel.")
                    .size(13),
                row![button(text("Calibrate").size(13))
                    .style(theme::primary)
                    .on_press(Message::Navigate(crate::Page::Calibration))],
            ]
            .spacing(12),
        )))
        .into();
    }

    let edit = app.keys.edit.unwrap_or_else(|| pad_settings(&app.config));
    let separate = app.keys.separate;

    let top = row![
        toggler(separate)
            .label("Set keys separately")
            .on_toggle(|on| Message::Keys(KeysMessage::Separate(on))),
        Space::new().width(Length::Fill),
        button(
            text(if app.keys.advanced {
                "Hide advanced"
            } else {
                "Advanced"
            })
            .size(12)
        )
        .style(theme::secondary)
        .on_press(Message::Keys(KeysMessage::ToggleAdvanced)),
        button(text("Recalibrate").size(12))
            .style(theme::secondary)
            .on_press(Message::Navigate(crate::Page::Calibration)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    content = content.push(top);

    // Separate keys: one column each, its switch on top and controls below
    let key_column = |key: usize, title: &'static str| {
        card(
            column![
                text(title).size(16).font(theme::FONT_BOLD),
                container(live_view(app, key, edit[key])).center_x(Length::Fill),
                key_controls(app, key, edit[key]),
            ]
            .spacing(12),
        )
    };
    if separate {
        content = content.push(
            row![key_column(0, "Key 1"), key_column(1, "Key 2")]
                .spacing(14)
                .width(Length::Fill),
        );
    } else {
        // Linked: one set of controls, both keys' live views
        content = content.push(card(
            column![
                text("Both keys").size(16).font(theme::FONT_BOLD),
                row![
                    container(key_controls(app, 0, edit[0])).width(Length::Fill),
                    live_view(app, 0, edit[0]),
                    live_view(app, 1, edit[1]),
                ]
                .spacing(20),
            ]
            .spacing(12),
        ));
    }

    // Pinned under the scrolling part, so Save is never scrolled away;
    // greyed out until something changes
    let dirty = app.keys.dirty;
    let actions = row![
        muted(
            "Values are provisional until tested on the pad: the noise floor that limits the smallest sensitivity is not measured yet, and the pad raises any setting below it."
        )
        .size(12)
        .width(Length::Fill),
        button(text("Revert").size(13))
            .style(theme::secondary)
            .on_press_maybe(dirty.then_some(Message::Keys(KeysMessage::Revert))),
        button(text("Save to pad").size(13))
            .style(theme::primary)
            .on_press_maybe(dirty.then_some(Message::Keys(KeysMessage::Save))),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    column![scrollable(content).height(Length::Fill), actions]
        .spacing(12)
        .into()
}

fn card<'a>(content: impl Into<Element<'a, Message>>) -> container::Container<'a, Message> {
    container(content)
        .padding(18)
        .width(Length::Fill)
        .style(theme::card)
}

/// A magnetic switch cut in half, front view: keycap shell on the cross
/// mount, translucent top housing, stem with the magnet in its foot, spring
/// on the centre pole, bottom housing with its pegs, and the Hall sensor on
/// the PCB under the magnet. The stem sits at the key's (smoothed) depth;
/// the ticks on the right mark where the settings press, release and reset.
struct SwitchView {
    /// 0 = rest, 1 = calibrated full press; `None` without a reading
    depth: Option<f32>,
    down: bool,
    settings: HeKeyConfig,
}

/// Canvas size the drawing is laid out for
const SWITCH_W: f32 = 260.0;
const SWITCH_H: f32 = 250.0;
/// Drawn at this size so both keys' columns fit the default window
/// (1360x860) without scrolling; labels keep their on-screen size
const SWITCH_SCALE: f32 = 0.72;
/// Keycap travel in pixels from rest to a calibrated full press
const TRAVEL_PX: f32 = 40.0;

fn rgba(hex: u32, a: f32) -> Color {
    Color::from_rgba8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, a)
}

fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Path {
    Path::rectangle(Point::new(x0, y0), Size::new(x1 - x0, y1 - y0))
}

fn rounded(x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> Path {
    Path::new(|b| {
        b.rounded_rectangle(
            Point::new(x0, y0),
            Size::new(x1 - x0, y1 - y0),
            iced::border::Radius::from(r),
        )
    })
}

fn polygon(points: &[(f32, f32)]) -> Path {
    Path::new(|b| {
        b.move_to(Point::new(points[0].0, points[0].1));
        for &(x, y) in &points[1..] {
            b.line_to(Point::new(x, y));
        }
        b.close();
    })
}

fn vertical(y0: f32, y1: f32, top: Color, bottom: Color) -> canvas::gradient::Linear {
    canvas::gradient::Linear::new(Point::new(0.0, y0), Point::new(0.0, y1))
        .add_stop(0.0, top)
        .add_stop(1.0, bottom)
}

impl canvas::Program<Message> for SwitchView {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        frame.scale(SWITCH_SCALE);
        // On-screen text size, undoing the scale
        let px = |size: f32| Pixels(size / SWITCH_SCALE);
        let line = |c: Color, width: f32| Stroke::default().with_color(c).with_width(width);
        let cx = 85.0;
        let depth = self.depth.unwrap_or(0.0).clamp(-0.1, 1.15);
        let off = depth * TRAVEL_PX;

        // PCB with the Hall sensor right under the magnet
        frame.fill(
            &rect(12.0, 222.0, 158.0, 230.0),
            vertical(222.0, 230.0, rgba(0x1F7A4A, 1.0), rgba(0x125233, 1.0)),
        );
        for x in [cx - 34.0, cx, cx + 34.0] {
            frame.fill(&rect(x - 7.0, 222.0, x + 7.0, 224.0), rgba(0xC9A24A, 1.0));
        }
        if self.down {
            frame.fill(
                &rounded(cx - 13.0, 209.0, cx + 13.0, 224.0, 4.0),
                rgba(0xFF66AA, 0.35),
            );
        }
        frame.fill(
            &rounded(cx - 8.0, 213.0, cx + 8.0, 221.0, 1.5),
            rgba(0x15151C, 1.0),
        );
        for x in [cx - 6.0, cx, cx + 6.0] {
            frame.fill(&rect(x - 1.0, 221.0, x + 1.0, 223.0), rgba(0xB8B8C4, 1.0));
        }
        frame.stroke(
            &rounded(cx - 8.0, 213.0, cx + 8.0, 221.0, 1.5),
            line(
                if self.down {
                    theme::PINK
                } else {
                    rgba(0x44445A, 1.0)
                },
                1.0,
            ),
        );

        // Bottom housing (section): walls, floor, centre pole, pegs into the PCB
        let housing = rgba(0x2E2E40, 1.0);
        frame.fill(&rect(cx - 54.0, 138.0, cx - 48.0, 202.0), housing);
        frame.fill(&rect(cx + 48.0, 138.0, cx + 54.0, 202.0), housing);
        frame.fill(&rect(cx - 54.0, 194.0, cx + 54.0, 202.0), housing);
        frame.fill(&rect(cx - 6.0, 202.0, cx - 2.0, 228.0), housing);
        frame.fill(&rect(cx + 2.0, 202.0, cx + 6.0, 228.0), housing);
        for x in [cx - 34.0, cx + 34.0] {
            frame.fill(&rect(x - 3.0, 202.0, x + 3.0, 226.0), housing);
        }
        frame.stroke(
            &polygon(&[
                (cx - 54.0, 138.0),
                (cx - 54.0, 202.0),
                (cx + 54.0, 202.0),
                (cx + 54.0, 138.0),
            ]),
            line(rgba(0x45455C, 1.0), 1.0),
        );

        // Spring on the centre pole: back of each coil, pole, front of each coil
        let spring_top = 112.0 + off;
        let spring_bottom = 194.0;
        let turns = 8;
        let pitch = (spring_bottom - spring_top) / turns as f32;
        let coil = |frame: &mut Frame, front: bool| {
            for t in 0..turns {
                let y = spring_top + pitch * t as f32;
                let (from, to) = if front {
                    ((cx - 13.0, y + pitch * 0.5), (cx + 13.0, y + pitch))
                } else {
                    ((cx + 13.0, y), (cx - 13.0, y + pitch * 0.5))
                };
                frame.stroke(
                    &Path::line(Point::new(from.0, from.1), Point::new(to.0, to.1)),
                    line(
                        if front {
                            rgba(0xD2D2DE, 1.0)
                        } else {
                            rgba(0x6A6A7C, 1.0)
                        },
                        2.2,
                    ),
                );
            }
        };
        coil(&mut frame, false);
        frame.fill(
            &rounded(cx - 4.0, 122.0, cx + 4.0, 196.0, 2.0),
            vertical(122.0, 196.0, rgba(0x4A4A60, 1.0), rgba(0x34344A, 1.0)),
        );
        coil(&mut frame, true);

        // Field lines from the magnet to the sensor, stronger as it closes in
        let strength = depth.clamp(0.0, 1.0);
        let field = rgba(0x66CCFF, 0.10 + 0.45 * strength);
        let magnet_bottom = 112.0 + off;
        for bend in [-30.0, -14.0, 0.0, 14.0, 30.0] {
            let curve = Path::new(|b| {
                b.move_to(Point::new(cx + bend * 0.3, magnet_bottom));
                b.quadratic_curve_to(
                    Point::new(cx + bend, (magnet_bottom + 213.0) / 2.0),
                    Point::new(cx + bend * 0.15, 213.0),
                );
            });
            frame.stroke(&curve, line(field, 1.0));
        }

        // Stem: neck through the top housing, body with the magnet in its foot
        let stem = vertical(
            52.0 + off,
            112.0 + off,
            rgba(0xFF8CC0, 1.0),
            rgba(0xE0538F, 1.0),
        );
        frame.fill(&rect(cx - 9.0, 52.0 + off, cx + 9.0, 86.0 + off), stem);
        frame.fill(
            &rounded(cx - 22.0, 84.0 + off, cx + 22.0, 112.0 + off, 3.0),
            stem,
        );
        frame.fill(
            &rect(cx - 22.0, 88.0 + off, cx - 18.0, 108.0 + off),
            rgba(0xFFB3D6, 1.0),
        );
        frame.fill(
            &rect(cx + 18.0, 88.0 + off, cx + 22.0, 108.0 + off),
            rgba(0xFFB3D6, 1.0),
        );
        // Magnet: north half facing the sensor
        frame.fill(
            &rect(cx - 13.0, 100.0 + off, cx + 13.0, 106.0 + off),
            rgba(0x4B7BD8, 1.0),
        );
        frame.fill(
            &rect(cx - 13.0, 106.0 + off, cx + 13.0, 112.0 + off),
            rgba(0xD84B4B, 1.0),
        );
        frame.stroke(
            &rect(cx - 13.0, 100.0 + off, cx + 13.0, 112.0 + off),
            line(rgba(0x1A1A24, 0.6), 1.0),
        );

        // Top housing (section), translucent smoke
        let smoke = rgba(0xBEC3DC, 0.16);
        let edge = line(rgba(0xFFFFFF, 0.30), 1.0);
        let left = polygon(&[
            (cx - 54.0, 142.0),
            (cx - 54.0, 84.0),
            (cx - 45.0, 74.0),
            (cx - 11.0, 74.0),
            (cx - 11.0, 80.0),
            (cx - 47.0, 80.0),
            (cx - 47.0, 142.0),
        ]);
        let right = polygon(&[
            (cx + 54.0, 142.0),
            (cx + 54.0, 84.0),
            (cx + 45.0, 74.0),
            (cx + 11.0, 74.0),
            (cx + 11.0, 80.0),
            (cx + 47.0, 80.0),
            (cx + 47.0, 142.0),
        ]);
        frame.fill(&left, smoke);
        frame.fill(&right, smoke);
        frame.stroke(&left, edge);
        frame.stroke(&right, edge);

        // Cross mount up into the keycap
        frame.fill(
            &rect(cx - 4.0, 31.0 + off, cx + 4.0, 54.0 + off),
            rgba(0xFF8CC0, 1.0),
        );
        frame.fill(
            &rect(cx - 10.0, 40.0 + off, cx + 10.0, 45.0 + off),
            rgba(0xFF8CC0, 1.0),
        );

        // Keycap shell (section)
        let cap = polygon(&[
            (cx - 60.0, 24.0 + off),
            (cx + 60.0, 24.0 + off),
            (cx + 66.0, 68.0 + off),
            (cx + 61.0, 68.0 + off),
            (cx + 55.0, 31.0 + off),
            (cx - 55.0, 31.0 + off),
            (cx - 61.0, 68.0 + off),
            (cx - 66.0, 68.0 + off),
        ]);
        frame.fill(
            &cap,
            vertical(
                24.0 + off,
                68.0 + off,
                rgba(0x4A4A60, 1.0),
                rgba(0x2A2A38, 1.0),
            ),
        );
        frame.stroke(&cap, line(rgba(0x5E5E78, 1.0), 1.0));
        frame.stroke(
            &Path::line(
                Point::new(cx - 58.0, 25.0 + off),
                Point::new(cx + 58.0, 25.0 + off),
            ),
            line(rgba(0xFFFFFF, 0.18), 1.0),
        );

        // Travel gauge beside the switch: the switch is drawn to scale, where
        // settings 10 % apart would be 4 px apart, so they are marked here,
        // magnified, with a pointer at the key's current depth
        let (gx, g0, g1) = (cx + 80.0, 24.0, 204.0);
        let gy = |v: f32| g0 + v.clamp(-0.1, 1.15) * (g1 - g0) / 1.15;
        frame.stroke(
            &Path::line(Point::new(gx, gy(0.0)), Point::new(gx, gy(1.0))),
            line(rgba(0x3A3A4C, 1.0), 2.0),
        );
        let s = &self.settings;
        let mut marks: Vec<(f32, Color, &str)> = vec![
            (0.0, rgba(0x8C8CA6, 1.0), "rest"),
            (s.actuation as f32 / 1000.0, theme::PINK, "press"),
            (1.0, rgba(0x8C8CA6, 1.0), "full"),
        ];
        if s.rt_mode == HeRtMode::Off {
            let release = s.actuation.saturating_sub(s.release_hyst) as f32 / 1000.0;
            marks.push((release, theme::MUTED, "release"));
        }
        if s.rt_mode == HeRtMode::Continuous {
            marks.push((s.top_reset as f32 / 1000.0, theme::CYAN, "reset"));
        }
        if s.bottom_dead_zone > 0 {
            let dz = 1.0 - s.bottom_dead_zone.min(1000) as f32 / 1000.0;
            marks.push((dz, theme::YELLOW, "dead zone"));
        }
        marks.sort_by(|a, b| a.0.total_cmp(&b.0));
        // Labels at least a line apart; the tick itself stays exact
        let mut next_free = f32::MIN;
        for (at, color, label) in marks {
            let y = gy(at);
            frame.stroke(
                &Path::line(Point::new(gx - 5.0, y), Point::new(gx + 5.0, y)),
                line(color, 1.5),
            );
            let label_y = (y - 6.0 / SWITCH_SCALE).max(next_free);
            next_free = label_y + 12.0 / SWITCH_SCALE;
            frame.fill_text(canvas::Text {
                content: label.to_string(),
                position: Point::new(gx + 9.0, label_y),
                color,
                size: px(10.0),
                ..canvas::Text::default()
            });
        }
        if self.depth.is_some() {
            let y = gy(depth);
            frame.fill(
                &polygon(&[(gx - 3.0, y), (gx - 11.0, y - 5.0), (gx - 11.0, y + 5.0)]),
                if self.down { theme::PINK } else { theme::WHITE },
            );
        }
        if self.depth.is_none() {
            frame.fill_text(canvas::Text {
                content: "no reading".to_string(),
                position: Point::new(12.0, 4.0),
                color: theme::MUTED,
                size: px(11.0),
                ..canvas::Text::default()
            });
        }
        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(a: u32, b: u32) -> [HeKeyConfig; 2] {
        [
            HeKeyConfig {
                actuation: a,
                ..HeKeyConfig::default()
            },
            HeKeyConfig {
                actuation: b,
                ..HeKeyConfig::default()
            },
        ]
    }

    fn config(p: [HeKeyConfig; 2]) -> DeviceConfig {
        DeviceConfig {
            key1_he: Some(p[0]),
            key2_he: Some(p[1]),
            ..DeviceConfig::default()
        }
    }

    #[test]
    fn linked_edits_change_both_keys() {
        let mut s = KeysState::default();
        s.adopt(&config(pad(500, 500)));
        assert!(!s.separate, "equal keys start linked");
        s.edit_settings(KeysMessage::Set(0, Field::Actuation, 300), pad(500, 500));
        let e = s.edit.unwrap();
        assert_eq!((e[0].actuation, e[1].actuation), (300, 300));
        assert!(s.dirty);
    }

    #[test]
    fn separate_edits_change_one_key_and_relinking_copies_key_1() {
        let mut s = KeysState::default();
        s.adopt(&config(pad(500, 400)));
        assert!(s.separate, "different keys start separate");
        s.edit_settings(KeysMessage::Set(1, Field::Actuation, 250), pad(500, 400));
        let e = s.edit.unwrap();
        assert_eq!((e[0].actuation, e[1].actuation), (500, 250));
        s.edit_settings(KeysMessage::Separate(false), pad(500, 400));
        let e = s.edit.unwrap();
        assert_eq!((e[0].actuation, e[1].actuation), (500, 500));
    }

    #[test]
    fn unsaved_edits_survive_status_polls_and_revert_restores_the_pad() {
        let mut s = KeysState::default();
        s.adopt(&config(pad(500, 500)));
        s.edit_settings(KeysMessage::Set(0, Field::Actuation, 200), pad(500, 500));
        s.adopt(&config(pad(500, 500)));
        assert_eq!(
            s.edit.unwrap()[0].actuation,
            200,
            "a poll must not undo an edit"
        );
        s.edit_settings(KeysMessage::Revert, pad(500, 500));
        assert!(!s.dirty);
        assert_eq!(s.edit.unwrap()[0].actuation, 500);
    }

    /// Deterministic noise with sd ~5.6 counts, like the V1.1 bench at rest
    fn noise(seed: &mut u32) -> f32 {
        let mut sum = 0i32;
        for _ in 0..6 {
            *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            sum += ((*seed >> 24) & 7) as i32;
        }
        (sum - 21) as f32
    }

    fn sd(v: &[f32]) -> f32 {
        let m = v.iter().sum::<f32>() / v.len() as f32;
        (v.iter().map(|x| (x - m).powi(2)).sum::<f32>() / v.len() as f32).sqrt()
    }

    #[test]
    fn smoothing_removes_rest_noise() {
        let t0 = std::time::Instant::now();
        let step = std::time::Duration::from_millis(20);
        let mut f = OneEuro::default();
        let mut seed = 1;
        let (mut raw, mut out) = (vec![], vec![]);
        for i in 0..300 {
            let x = 2086.0 + noise(&mut seed);
            let y = f.filter(x, t0 + step * i);
            if i >= 30 {
                raw.push(x);
                out.push(y);
            }
        }
        assert!(
            sd(&out) < sd(&raw) * 0.3,
            "raw sd {} shown sd {}",
            sd(&raw),
            sd(&out)
        );
    }

    #[test]
    fn smoothing_follows_a_fast_press_within_a_few_readings() {
        let t0 = std::time::Instant::now();
        let step = std::time::Duration::from_millis(20);
        let mut f = OneEuro::default();
        for i in 0..50 {
            f.filter(2086.0, t0 + step * i);
        }
        // A full press arrives between two readings: a 450-count step
        let shown: Vec<f32> = (50..54)
            .map(|i| f.filter(2536.0, t0 + step * i) - 2086.0)
            .collect();
        assert!(shown[0] > 450.0 * 0.4, "first reading shows {}", shown[0]);
        assert!(shown[2] > 450.0 * 0.85, "third reading shows {}", shown[2]);
    }

    #[test]
    fn a_still_key_is_drawn_perfectly_still() {
        let mut s = KeysState::default();
        let t0 = std::time::Instant::now();
        let f0 = iced::time::Instant::now();
        let reading = std::time::Duration::from_millis(20);
        let frame = std::time::Duration::from_micros(16_667);
        let mut seed = 7;
        let mut drawn = vec![];
        // 10 s at rest: readings at 50/s, frames at 60/s
        for i in 0..500u32 {
            let raw = (2086.0 + noise(&mut seed)) as u32;
            s.take_sample(
                Some(HeLiveSample {
                    key1_raw: raw,
                    ..Default::default()
                }),
                t0 + reading * i,
            );
            s.advance(f0 + frame * (i * 6 / 5));
            if i >= 100 {
                drawn.push(s.drawn[0].unwrap());
            }
        }
        let (lo, hi) = drawn
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), &x| (a.min(x), b.max(x)));
        assert!(hi - lo < 0.5, "still key moved {} counts", hi - lo);
    }

    #[test]
    fn jitter_suppression_still_follows_a_press_closely() {
        let mut h = 2086.0;
        for v in [2090.0, 2083.0, 2091.0] {
            h = hold(h, v);
        }
        assert_eq!(h, 2086.0, "noise inside the dead zone is ignored");
        h = hold(h, 2536.0);
        assert_eq!(
            h,
            2536.0 - HOLD_COUNTS,
            "a press is followed to within the dead zone"
        );
    }

    #[test]
    fn drawn_stem_glides_without_jumping_and_catches_up() {
        let mut s = KeysState::default();
        let t0 = iced::time::Instant::now();
        let frame = std::time::Duration::from_micros(16_667);
        s.smooth[0].value = Some(2086.0);
        s.advance(t0);
        assert_eq!(s.drawn[0], Some(2086.0), "starts where the reading is");
        s.smooth[0].value = Some(2536.0);
        s.advance(t0 + frame);
        let first = s.drawn[0].unwrap() - 2086.0;
        assert!(
            first > 0.0 && first < 450.0 * 0.6,
            "one frame moves part way: {first}"
        );
        for i in 2..=6 {
            s.advance(t0 + frame * i);
        }
        assert!(
            s.drawn[0].unwrap() - 2086.0 > 450.0 * 0.95,
            "caught up within 100 ms"
        );
    }

    #[test]
    fn rapid_trigger_and_continuous_modes() {
        let mut s = KeysState::default();
        let p = pad(500, 500);
        s.adopt(&config(p));
        // Continuous does nothing while rapid trigger is off
        s.edit_settings(KeysMessage::Continuous(0, true), p);
        assert_eq!(s.edit.unwrap()[0].rt_mode, HeRtMode::Off);
        s.edit_settings(KeysMessage::RapidTrigger(0, true), p);
        assert_eq!(s.edit.unwrap()[0].rt_mode, HeRtMode::Normal);
        s.edit_settings(KeysMessage::Continuous(0, true), p);
        assert_eq!(s.edit.unwrap()[1].rt_mode, HeRtMode::Continuous);
        // Switching rapid trigger off and on again keeps its last mode only while on
        s.edit_settings(KeysMessage::RapidTrigger(0, false), p);
        assert_eq!(s.edit.unwrap()[0].rt_mode, HeRtMode::Off);
        // Simple sensitivity sets press and release together
        s.edit_settings(KeysMessage::Set(0, Field::Sensitivity, 120), p);
        let e = s.edit.unwrap()[0];
        assert_eq!((e.rt_press, e.rt_release), (120, 120));
    }
}
