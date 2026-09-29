//! Stats page: the player's tap rate in PPM (presses per minute), for the
//! attempt being played and over the history period. PPM is how fast the
//! keys are pressed, not the beatmap's BPM.

use crate::theme::{self, caption, heading, muted};
use crate::{ipc, App, Message};
use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Alignment, Element, Length, Task};
use opad_ipc::{IpcRequest, IpcResponse, TapStatsSnapshot};
use opad_model::tap_rate::{
    history_period_label, AttemptStats, BeatmapRef, TapChannel, TapHistory, HISTORY_PERIOD_CHOICES,
};
use std::time::{Duration, Instant};

/// The longest wait between retries once GetTapStats keeps failing
const MAX_BACKOFF: Duration = Duration::from_secs(30);
/// A request unanswered this long no longer holds the next one back
const REQUEST_STALE: Duration = Duration::from_secs(10);

#[derive(Debug, Default)]
pub struct StatsState {
    pub snapshot: Option<TapStatsSnapshot>,
    /// Why the last GetTapStats failed; the snapshot, if any, is older
    pub error: Option<String>,
    /// Why the last period change failed
    pub period_error: Option<String>,
    /// A period change the daemon has not answered yet. Polls answered
    /// meanwhile may still carry the old period, so they are dropped.
    pub pending_period: Option<u32>,
    /// When the unanswered GetTapStats was sent
    in_flight: Option<Instant>,
    failures: u32,
    retry_at: Option<Instant>,
}

impl StatsState {
    /// GetTapStats, unless one is on its way or a failing daemon is being
    /// left alone for a while (an older daemon drops the connection on it)
    pub fn poll(&mut self) -> Option<Task<Message>> {
        let now = Instant::now();
        if self.in_flight.is_some_and(|t| now < t + REQUEST_STALE)
            || self.retry_at.is_some_and(|t| now < t)
        {
            return None;
        }
        self.in_flight = Some(now);
        Some(Task::perform(
            ipc::request(IpcRequest::GetTapStats),
            Message::TapStats,
        ))
    }

    /// Opening the page tries again at once
    pub fn reset_backoff(&mut self) {
        self.retry_at = None;
    }

    pub fn received(&mut self, result: Result<IpcResponse, String>) {
        self.in_flight = None;
        match result {
            Ok(IpcResponse::TapStats(snapshot)) => {
                self.failures = 0;
                self.retry_at = None;
                self.error = None;
                if self.pending_period.is_none() {
                    self.snapshot = Some(snapshot);
                }
            }
            Ok(IpcResponse::Error(e)) => self.failed(e),
            Ok(other) => self.failed(format!("unexpected reply {other:?}")),
            Err(e) => self.failed(e),
        }
    }

    fn failed(&mut self, error: String) {
        self.failures = self.failures.saturating_add(1);
        self.retry_at = Some(Instant::now() + backoff(self.failures));
        self.error = Some(error);
    }

    pub fn set_period(&mut self, days: u32) -> Task<Message> {
        self.pending_period = Some(days);
        self.period_error = None;
        Task::perform(
            ipc::request(IpcRequest::SetTapHistoryPeriod { days }),
            Message::TapPeriodSet,
        )
    }

    pub fn period_set(&mut self, result: Result<IpcResponse, String>) {
        self.pending_period = None;
        match result {
            Ok(IpcResponse::TapStats(snapshot)) => {
                self.error = None;
                self.snapshot = Some(snapshot);
            }
            Ok(IpcResponse::Error(e)) => self.period_error = Some(e),
            Ok(other) => self.period_error = Some(format!("unexpected reply {other:?}")),
            Err(e) => self.period_error = Some(e),
        }
    }
}

/// 1 s after the first failure, doubling up to [`MAX_BACKOFF`]
fn backoff(failures: u32) -> Duration {
    Duration::from_secs(1u64 << failures.saturating_sub(1).min(5)).min(MAX_BACKOFF)
}

/// A PPM as shown: whole numbers, "-" when there is none
pub(crate) fn fmt_ppm(ppm: Option<f64>) -> String {
    match ppm.filter(|p| p.is_finite()) {
        Some(p) => format!("{:.0}", p),
        None => "-".into(),
    }
}

/// "Artist - Title [Difficulty] +HDDT", or None when tosu gave no title
pub(crate) fn map_line(map: &BeatmapRef) -> Option<String> {
    if map.title.is_empty() {
        return None;
    }
    let mut line = if map.artist.is_empty() {
        map.title.clone()
    } else {
        format!("{} - {}", map.artist, map.title)
    };
    if !map.difficulty.is_empty() {
        line.push_str(&format!(" [{}]", map.difficulty));
    }
    if let Some(mods) = map.mods.as_deref().filter(|m| !m.is_empty()) {
        line.push_str(&format!(" +{mods}"));
    }
    Some(line)
}

fn card<'a>(content: impl Into<Element<'a, Message>>) -> container::Container<'a, Message> {
    container(content)
        .padding(20)
        .width(Length::Fill)
        .style(theme::card)
}

fn stat<'a>(label: &'a str, value: String) -> Element<'a, Message> {
    column![caption(label), text(value).size(20).font(theme::FONT_BOLD)]
        .spacing(4)
        .into()
}

/// One table row: a name column, then equal value columns
fn table_row<'a>(cells: Vec<String>, header: bool, bold: bool) -> Element<'a, Message> {
    let mut r = row![].spacing(8);
    for (i, cell) in cells.into_iter().enumerate() {
        let mut t = text(cell).size(if header { 12 } else { 15 });
        if header {
            t = t.color(theme::MUTED);
        } else if bold || i == 0 {
            t = t.font(theme::FONT_BOLD);
        }
        r = r.push(t.width(Length::FillPortion(if i == 0 { 3 } else { 2 })));
    }
    r.into()
}

fn attempt_card<'a>(attempt: &AttemptStats, live: bool) -> Element<'a, Message> {
    let (title, badge, badge_color) = if live {
        ("Current attempt", "LIVE".to_string(), theme::GREEN)
    } else {
        let ended = attempt
            .ended_at
            .map(|t| format!("ENDED {}", t.with_timezone(&chrono::Local).format("%H:%M")))
            .unwrap_or_else(|| "ENDED".into());
        ("Last attempt", ended, theme::MUTED)
    };
    let mut header = vec!["".into()];
    if live {
        header.push("Current".into());
    }
    header.extend(["Average".into(), "Peak".into(), "Presses".into()]);
    let mut rows = column![table_row(header, true, false)].spacing(8);
    for channel in TapChannel::ALL {
        let c = attempt.channel(channel);
        let mut cells = vec![channel.label().to_string()];
        if live {
            cells.push(fmt_ppm(c.current_ppm));
        }
        cells.extend([
            fmt_ppm(c.average_ppm),
            fmt_ppm(c.peak_ppm),
            crate::pages::grouped(u64::from(c.presses)),
        ]);
        rows = rows.push(table_row(cells, false, channel == TapChannel::Combined));
    }

    column![
        row![
            text(title).size(16).font(theme::FONT_BOLD),
            Space::new().width(Length::Fill),
            text(badge)
                .size(12)
                .font(theme::FONT_BOLD)
                .color(badge_color),
        ]
        .align_y(Alignment::Center),
        text(map_line(&attempt.beatmap).unwrap_or_else(|| "Unknown map".into()))
            .size(14)
            .color(theme::CYAN),
        container(rows).padding([4, 0]),
        caption("PPM = presses per minute. Current is the rate over your last few presses."),
    ]
    .spacing(10)
    .into()
}

fn history_body<'a>(history: &TapHistory, unsaved: usize) -> Element<'a, Message> {
    let all = &history.combined;
    let mut body = column![
        row![
            stat("ATTEMPTS", crate::pages::grouped(history.attempts)),
            stat("AVERAGE PPM", fmt_ppm(all.average_ppm)),
            stat("AVERAGE PEAK", fmt_ppm(all.average_peak_ppm)),
            stat("BEST PEAK", fmt_ppm(all.best_peak_ppm)),
            stat("PRESSES", crate::pages::grouped(all.total_presses)),
        ]
        .spacing(36),
        table_row(
            vec![
                "".into(),
                "Average".into(),
                "Avg peak".into(),
                "Best peak".into(),
                "Presses".into(),
            ],
            true,
            false,
        ),
    ]
    .spacing(12);
    for channel in [TapChannel::K1, TapChannel::K2] {
        let c = history.channel(channel);
        body = body.push(table_row(
            vec![
                channel.label().to_string(),
                fmt_ppm(c.average_ppm),
                fmt_ppm(c.average_peak_ppm),
                fmt_ppm(c.best_peak_ppm),
                crate::pages::grouped(c.total_presses),
            ],
            false,
            false,
        ));
    }
    if unsaved > 0 {
        body = body.push(unsaved_note(unsaved));
    }
    body.into()
}

fn unsaved_note<'a>(unsaved: usize) -> Element<'a, Message> {
    caption(if unsaved == 1 {
        "1 attempt will be saved to the history when play stops.".to_string()
    } else {
        format!("{unsaved} attempts will be saved to the history when play stops.")
    })
    .into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let s = &app.stats;
    let mut page = column![row![
        heading("Stats"),
        Space::new().width(Length::Fill),
        caption("TAP RATE · PPM"),
    ]
    .align_y(Alignment::Center)]
    .spacing(14);

    if let Some(e) = &s.error {
        let msg = if !app.daemon_online {
            "The daemon is not running. PPM statistics appear once it is.".to_string()
        } else if s.snapshot.is_none() {
            format!(
                "PPM statistics are not available from this daemon (it may be an older \
                 version). {e}"
            )
        } else {
            format!("Could not refresh PPM statistics: {e}")
        };
        page = page.push(
            container(text(msg).size(13).color(theme::YELLOW))
                .padding([10, 14])
                .width(Length::Fill)
                .style(theme::banner),
        );
    }

    let Some(snapshot) = &s.snapshot else {
        if s.error.is_none() {
            page = page.push(card(muted("Loading PPM statistics...")));
        }
        return scrollable(page).into();
    };

    let attempt: Element<'_, Message> = match (&snapshot.current, &snapshot.last) {
        (Some(current), _) => attempt_card(current, true),
        (None, Some(last)) => attempt_card(last, false),
        (None, None) => column![
            text("Current attempt").size(16).font(theme::FONT_BOLD),
            muted(
                "Play a map with tosu running and your tap rate (PPM) shows up here, \
                 per key and combined."
            ),
        ]
        .spacing(10)
        .into(),
    };
    page = page.push(card(attempt));

    // The configured period, or the one just asked for
    let selected = s.pending_period.unwrap_or(snapshot.period_days);
    let periods = row(HISTORY_PERIOD_CHOICES.iter().map(|&days| {
        let label = match days {
            0 => "All time".to_string(),
            d => format!("{d} days"),
        };
        button(text(label).size(13))
            .padding([6, 12])
            .style(theme::tab_button(days == selected))
            .on_press_maybe(
                (s.pending_period.is_none() && days != snapshot.period_days)
                    .then_some(Message::SetTapPeriod(days)),
            )
            .into()
    }))
    .spacing(6);

    let mut history = column![row![
        column![
            text("History").size(16).font(theme::FONT_BOLD),
            muted(history_period_label(selected)).size(13),
        ]
        .spacing(2),
        Space::new().width(Length::Fill),
        periods,
    ]
    .align_y(Alignment::Center)]
    .spacing(14);
    if let Some(e) = &s.period_error {
        history = history.push(
            text(format!("Could not change the period: {e}"))
                .size(13)
                .color(theme::RED),
        );
    }
    history = history.push(match &snapshot.history {
        Some(h) if h.attempts > 0 => history_body(h, snapshot.unsaved_attempts),
        Some(_) => empty_history(
            "No attempts in this period yet. Finished attempts are saved here.",
            snapshot.unsaved_attempts,
        ),
        None => empty_history(
            "The history is not available right now (storage not ready).",
            snapshot.unsaved_attempts,
        ),
    });
    page = page.push(card(history));

    scrollable(page).into()
}

fn empty_history<'a>(msg: &'a str, unsaved: usize) -> Element<'a, Message> {
    let mut c = column![muted(msg)].spacing(8);
    if unsaved > 0 {
        c = c.push(unsaved_note(unsaved));
    }
    c.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ppm_is_a_whole_number_or_a_dash() {
        assert_eq!(fmt_ppm(Some(199.6)), "200");
        assert_eq!(fmt_ppm(Some(0.0)), "0");
        assert_eq!(fmt_ppm(None), "-");
        assert_eq!(fmt_ppm(Some(f64::NAN)), "-");
        assert_eq!(fmt_ppm(Some(f64::INFINITY)), "-");
    }

    #[test]
    fn map_line_names_what_is_known() {
        assert_eq!(map_line(&BeatmapRef::default()), None);
        let map = BeatmapRef {
            title: "Blue Zenith".into(),
            artist: "xi".into(),
            difficulty: "FOUR DIMENSIONS".into(),
            mods: Some("HDDT".into()),
            ..Default::default()
        };
        assert_eq!(
            map_line(&map).unwrap(),
            "xi - Blue Zenith [FOUR DIMENSIONS] +HDDT"
        );
        let bare = BeatmapRef {
            title: "Blue Zenith".into(),
            mods: Some(String::new()),
            ..Default::default()
        };
        assert_eq!(map_line(&bare).unwrap(), "Blue Zenith");
    }

    #[test]
    fn a_failing_daemon_is_polled_less_often() {
        assert_eq!(backoff(1), Duration::from_secs(1));
        assert_eq!(backoff(2), Duration::from_secs(2));
        assert_eq!(backoff(4), Duration::from_secs(8));
        assert_eq!(backoff(40), MAX_BACKOFF);

        let mut s = StatsState {
            in_flight: Some(Instant::now()),
            ..Default::default()
        };
        s.received(Err("connection closed".into()));
        assert!(s.in_flight.is_none());
        assert!(s.error.is_some());
        // Waiting out the backoff: no request
        assert!(s.poll().is_none());
        s.reset_backoff();
        s.received(Ok(IpcResponse::TapStats(TapStatsSnapshot::default())));
        assert!(s.error.is_none());
        assert_eq!(s.failures, 0);
        assert!(s.snapshot.is_some());
    }

    #[test]
    fn polls_do_not_undo_a_period_change() {
        let mut s = StatsState {
            pending_period: Some(7),
            ..Default::default()
        };
        let old = TapStatsSnapshot {
            period_days: 30,
            ..Default::default()
        };
        s.received(Ok(IpcResponse::TapStats(old)));
        assert!(s.snapshot.is_none());
        s.period_set(Ok(IpcResponse::TapStats(TapStatsSnapshot {
            period_days: 7,
            ..Default::default()
        })));
        assert_eq!(s.pending_period, None);
        assert_eq!(s.snapshot.unwrap().period_days, 7);
        let mut s = StatsState {
            pending_period: Some(7),
            ..Default::default()
        };
        s.period_set(Ok(IpcResponse::Error("storage busy".into())));
        assert_eq!(s.period_error.as_deref(), Some("storage busy"));
    }
}
