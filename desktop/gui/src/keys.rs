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
        .width(Length::Fixed(170.0))
        .height(Length::Fixed(250.0)),
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

    let key_card = |key: usize, title: &'static str| {
        card(
            column![
                text(title).size(16).font(theme::FONT_BOLD),
                row![
                    container(key_controls(app, key, edit[key])).width(Length::Fill),
                    live_view(app, key, edit[key]),
                ]
                .spacing(20),
            ]
            .spacing(12),
        )
    };
    if separate {
        content = content
            .push(key_card(0, "Key 1"))
            .push(key_card(1, "Key 2"));
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

    let mut actions = row![muted(
        "Values are provisional until tested on the pad: the noise floor that limits the smallest sensitivity is not measured yet, and the pad raises any setting below it."
    )
    .size(12)
    .width(Length::Fill)]
    .spacing(10)
    .align_y(Alignment::Center);
    if app.keys.dirty {
        actions = actions
            .push(
                button(text("Revert").size(13))
                    .style(theme::secondary)
                    .on_press(Message::Keys(KeysMessage::Revert)),
            )
            .push(
                button(text("Save to pad").size(13))
                    .style(theme::primary)
                    .on_press(Message::Keys(KeysMessage::Save)),
            );
    }
    content = content.push(actions);
    scrollable(content).into()
}

fn card<'a>(content: impl Into<Element<'a, Message>>) -> container::Container<'a, Message> {
    container(content)
        .padding(18)
        .width(Length::Fill)
        .style(theme::card)
}

/// A switch cut in half: housing, stem, spring and magnet, with the sensor
/// under it. The stem sits at the key's live depth; the lines mark where the
/// settings press, release and reset.
struct SwitchView {
    /// 0 = rest, 1 = calibrated full press; `None` without a reading
    depth: Option<f32>,
    down: bool,
    settings: HeKeyConfig,
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
        let w = bounds.width;
        let line = |c: Color, width: f32| Stroke::default().with_color(c).with_width(width);

        // Travel runs from y0 (rest) to y1 (full press) for the stem's top
        let (x0, x1) = (34.0, w - 34.0);
        let (y0, y1) = (40.0_f32, 130.0_f32);
        let travel = y1 - y0;
        let depth = self.depth.unwrap_or(0.0).clamp(-0.1, 1.15);
        let stem_top = y0 + depth * travel;

        // Keycap stand-in and stem
        let stem_w = 34.0;
        let sx = w / 2.0 - stem_w / 2.0;
        let stem_color = if self.down { theme::PINK } else { theme::MUTED };
        frame.fill(
            &Path::rectangle(
                Point::new(sx - 26.0, stem_top - 22.0),
                Size::new(stem_w + 52.0, 14.0),
            ),
            theme::CARD_HOVER,
        );
        frame.fill(
            &Path::rectangle(Point::new(sx, stem_top - 8.0), Size::new(stem_w, 64.0)),
            stem_color,
        );
        // Magnet at the stem's foot
        frame.fill(
            &Path::rectangle(
                Point::new(sx + 6.0, stem_top + 48.0),
                Size::new(stem_w - 12.0, 8.0),
            ),
            theme::CYAN,
        );

        // Housing: top plate with the stem opening, side walls, floor
        let floor = y1 + 80.0;
        let border = line(theme::BORDER, 3.0);
        frame.stroke(
            &Path::line(Point::new(x0, y0 + 10.0), Point::new(sx - 4.0, y0 + 10.0)),
            border,
        );
        frame.stroke(
            &Path::line(
                Point::new(sx + stem_w + 4.0, y0 + 10.0),
                Point::new(x1, y0 + 10.0),
            ),
            border,
        );
        frame.stroke(
            &Path::line(Point::new(x0, y0 + 10.0), Point::new(x0, floor)),
            border,
        );
        frame.stroke(
            &Path::line(Point::new(x1, y0 + 10.0), Point::new(x1, floor)),
            border,
        );
        frame.stroke(
            &Path::line(Point::new(x0, floor), Point::new(x1, floor)),
            border,
        );

        // Spring between the stem's foot and the floor, compressing as it goes down
        let spring_top = stem_top + 56.0;
        let coils = 6;
        let step = (floor - spring_top) / (coils as f32 * 2.0);
        let spring = Path::new(|p| {
            p.move_to(Point::new(w / 2.0, spring_top));
            for i in 1..=coils * 2 {
                let x = if i % 2 == 0 {
                    w / 2.0 - 12.0
                } else {
                    w / 2.0 + 12.0
                };
                p.line_to(Point::new(x, spring_top + step * i as f32));
            }
        });
        frame.stroke(&spring, line(theme::OFF, 2.0));

        // Hall sensor on the PCB under the switch
        frame.fill(
            &Path::rectangle(
                Point::new(w / 2.0 - 14.0, floor + 6.0),
                Size::new(28.0, 8.0),
            ),
            theme::YELLOW,
        );
        frame.fill(
            &Path::rectangle(
                Point::new(x0 - 10.0, floor + 14.0),
                Size::new(x1 - x0 + 20.0, 4.0),
            ),
            theme::GREEN,
        );

        // Threshold marks on the right wall, at stem-top depth
        let mark = |frame: &mut Frame, at: u32, color: Color, label: &str| {
            let y = y0 + (at as f32 / 1000.0) * travel;
            frame.stroke(
                &Path::line(Point::new(x0 - 6.0, y), Point::new(x1 + 6.0, y)),
                line(color, 1.0),
            );
            frame.fill_text(canvas::Text {
                content: label.to_string(),
                position: Point::new(x1 + 8.0, y - 6.0),
                color,
                size: Pixels(10.0),
                ..canvas::Text::default()
            });
        };
        let s = &self.settings;
        mark(&mut frame, s.actuation, theme::PINK, "act");
        if s.rt_mode == HeRtMode::Off {
            mark(
                &mut frame,
                s.actuation.saturating_sub(s.release_hyst),
                theme::MUTED,
                "rel",
            );
        }
        if s.rt_mode == HeRtMode::Continuous {
            mark(&mut frame, s.top_reset, theme::CYAN, "reset");
        }
        if s.bottom_dead_zone > 0 {
            mark(
                &mut frame,
                1000 - s.bottom_dead_zone.min(1000),
                theme::YELLOW,
                "dz",
            );
        }
        if self.depth.is_none() {
            frame.fill_text(canvas::Text {
                content: "no reading".to_string(),
                position: Point::new(x0, 6.0),
                color: theme::MUTED,
                size: Pixels(11.0),
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
