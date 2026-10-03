//! The tap rate (PPM) attempt lifecycle in the runtime controller (issue #2):
//! what counts, when an attempt ends, and when it may be saved.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use opad_daemon::runtime::{RuntimeAction, RuntimeController, RuntimeEvent, COOLDOWN_DURATION};
use opad_model::tap_rate::{AttemptStats, BeatmapRef, ChannelHistory, TapHistory};
use opad_model::ui_source::{self as src, SourceValue};
use opad_model::{CounterState, DeviceConfig, KeyCounts, RuntimeMode};

fn controller(now: Instant) -> RuntimeController {
    RuntimeController::new(
        DeviceConfig::default(),
        None,
        CounterState::default(),
        HashMap::new(),
        None,
        Vec::new(),
        now,
    )
}

fn frame(is_playing: bool, live_time_ms: f64, failed: bool) -> RuntimeEvent {
    RuntimeEvent::TosuTelemetry {
        is_playing,
        live_time_ms,
        title: "Map".into(),
        values: Vec::new(),
        beatmap: BeatmapRef {
            title: "Map".into(),
            checksum: Some("md5".into()),
            ..Default::default()
        },
        failed,
        clock: None,
        other_player: false,
    }
}

/// `DataSync` rate-limits flushes to the pad on the real clock, not on event
/// times, so a test that expects a data update has to let that much pass
fn let_a_flush_interval_pass() {
    std::thread::sleep(opad_daemon::telemetry::FLUSH_INTERVAL_IDLE + Duration::from_millis(10));
}

fn keys(k1: u32, k2: u32, at: Instant) -> RuntimeEvent {
    RuntimeEvent::TosuKeys(KeyCounts { k1, k2, at })
}

/// Alternates K1/K2 every 300 ms (200 PPM combined) for `presses` presses,
/// starting from the given counts and time; returns the counts and time after
fn tap(
    c: &mut RuntimeController,
    mut counts: (u32, u32),
    mut at: Instant,
    presses: u32,
) -> ((u32, u32), Instant) {
    for i in 0..presses {
        if i % 2 == 0 {
            counts.0 += 1;
        } else {
            counts.1 += 1;
        }
        at += Duration::from_millis(300);
        c.on_event(keys(counts.0, counts.1, at), at);
    }
    (counts, at)
}

/// Every attempt a `SaveTapStats` action carries
fn saved(actions: &[RuntimeAction]) -> Vec<AttemptStats> {
    actions
        .iter()
        .flat_map(|a| match a {
            RuntimeAction::SaveTapStats { attempts, .. } => attempts.clone(),
            _ => Vec::new(),
        })
        .collect()
}

/// Runs the play session's end through COOLDOWN and SYNC into IDLE, returning
/// the actions of the first IDLE tick
fn settle(c: &mut RuntimeController, ended: Instant) -> Vec<RuntimeAction> {
    let after_cooldown = ended + COOLDOWN_DURATION + Duration::from_millis(10);
    let actions = c.on_event(RuntimeEvent::Tick(after_cooldown), after_cooldown);
    assert_eq!(c.state.mode, RuntimeMode::Sync);
    assert!(saved(&actions).is_empty(), "saved during SYNC");
    c.on_event(
        RuntimeEvent::SyncCompleted {
            success: true,
            counters: CounterState::default(),
            time_str: None,
            error: None,
        },
        after_cooldown,
    );
    assert_eq!(c.state.mode, RuntimeMode::Idle);
    let t = after_cooldown + Duration::from_millis(50);
    c.on_event(RuntimeEvent::Tick(t), t)
}

#[test]
fn an_attempt_is_measured_and_saved_only_once_play_has_stopped() {
    let t0 = Instant::now();
    let mut c = controller(t0);
    c.on_event(keys(0, 0, t0), t0);
    c.on_event(frame(true, 0.0, false), t0);
    let (_, end) = tap(&mut c, (0, 0), t0, 20);

    let live = c.state.tap.snapshot.current.clone().expect("live attempt");
    let ppm = live.combined.current_ppm.unwrap();
    assert!((ppm - 200.0).abs() < 1.0, "{ppm}");
    assert_eq!(live.combined.presses, 20);

    // Nothing may be written while playing, however long it lasts
    let t = end + Duration::from_secs(1);
    assert!(saved(&c.on_event(RuntimeEvent::Tick(t), t)).is_empty());

    // The map ends: the attempt is final, kept as `last` and queued
    let actions = c.on_event(frame(false, 0.0, false), end);
    assert!(saved(&actions).is_empty(), "saved in COOLDOWN");
    assert!(c.state.tap.snapshot.current.is_none());
    let last = c.state.tap.snapshot.last.clone().expect("last attempt");
    assert_eq!(last.combined.current_ppm, Some(0.0));
    assert!(last.ended_at.is_some());
    assert_eq!(c.state.tap.snapshot.unsaved_attempts, 1);

    let attempts = saved(&settle(&mut c, end));
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].beatmap.checksum.as_deref(), Some("md5"));
    assert_eq!(c.state.tap.snapshot.unsaved_attempts, 0);
}

#[test]
fn a_retry_starts_a_fresh_attempt_and_queues_the_old_one() {
    let t0 = Instant::now();
    let mut c = controller(t0);
    c.on_event(keys(0, 0, t0), t0);
    c.on_event(frame(true, 30_000.0, false), t0);
    let (_, t) = tap(&mut c, (0, 0), t0, 20);

    // Retry: the song position jumps back, and osu! restarts its counters
    c.on_event(frame(true, 0.0, false), t);
    assert_eq!(
        c.state
            .tap
            .snapshot
            .current
            .as_ref()
            .unwrap()
            .combined
            .presses,
        0
    );
    assert_eq!(
        c.state.tap.snapshot.last.as_ref().unwrap().combined.presses,
        20
    );
    assert_eq!(c.state.tap.snapshot.unsaved_attempts, 1);

    let (_, t) = tap(&mut c, (0, 0), t, 12);
    let current = c.state.tap.snapshot.current.clone().unwrap();
    assert_eq!(current.combined.presses, 12);

    c.on_event(frame(false, 0.0, false), t);
    assert_eq!(saved(&settle(&mut c, t)).len(), 2);
}

#[test]
fn a_fail_ends_the_attempt_and_later_presses_do_not_count() {
    let t0 = Instant::now();
    let mut c = controller(t0);
    c.on_event(keys(0, 0, t0), t0);
    c.on_event(frame(true, 0.0, false), t0);
    let (counts, t) = tap(&mut c, (0, 0), t0, 16);

    c.on_event(frame(true, 5000.0, true), t);
    assert!(c.state.tap.snapshot.current.is_none());
    assert_eq!(
        c.state.tap.snapshot.last.as_ref().unwrap().combined.presses,
        16
    );

    // Mashing on the fail screen
    let (_, t) = tap(&mut c, counts, t, 10);
    assert!(c.state.tap.snapshot.current.is_none());
    assert_eq!(
        c.state.tap.snapshot.last.as_ref().unwrap().combined.presses,
        16
    );

    // A retry after the fail is a new attempt, even while tosu's fail flag
    // is still up on its first frame
    c.on_event(frame(true, 0.0, true), t);
    assert!(c.state.tap.snapshot.current.is_some());
    c.on_event(frame(true, 100.0, false), t);
    assert!(c.state.tap.snapshot.current.is_some());
}

#[test]
fn menu_presses_are_never_recorded() {
    let t0 = Instant::now();
    let mut c = controller(t0);
    c.on_event(keys(0, 0, t0), t0);
    c.on_event(frame(false, 0.0, false), t0);
    tap(&mut c, (0, 0), t0, 30);
    assert!(c.state.tap.snapshot.current.is_none());
    assert!(c.state.tap.snapshot.last.is_none());
    let t = t0 + Duration::from_secs(20);
    assert!(saved(&c.on_event(RuntimeEvent::Tick(t), t)).is_empty());
}

#[test]
fn a_short_attempt_is_shown_but_not_recorded() {
    let t0 = Instant::now();
    let mut c = controller(t0);
    c.on_event(keys(0, 0, t0), t0);
    c.on_event(frame(true, 0.0, false), t0);
    let (_, t) = tap(&mut c, (0, 0), t0, 4);
    c.on_event(frame(false, 0.0, false), t);
    assert!(c.state.tap.snapshot.last.is_some());
    assert!(saved(&settle(&mut c, t)).is_empty());
}

#[test]
fn the_first_press_of_a_play_counts_when_the_counters_restart() {
    let t0 = Instant::now();
    let mut c = controller(t0);
    // The previous play left the counters at 120/130
    c.on_event(keys(120, 130, t0), t0);
    c.on_event(frame(true, 0.0, false), t0);
    // osu! restarts at zero, and the first frame tosu reports already has
    // the first press in it
    let t = t0 + Duration::from_millis(300);
    c.on_event(keys(1, 0, t), t);
    assert_eq!(c.state.tap.snapshot.current.as_ref().unwrap().k1.presses, 1);
}

#[test]
fn the_pad_gets_the_live_rate_and_the_history() {
    let t0 = Instant::now();
    let mut c = controller(t0);
    c.state.device_connected = true;
    c.on_event(keys(0, 0, t0), t0);
    c.on_event(frame(true, 0.0, false), t0);
    let (_, t) = tap(&mut c, (0, 0), t0, 20);

    let t = t + Duration::from_millis(200);
    let_a_flush_interval_pass();
    let actions = c.on_event(RuntimeEvent::Tick(t), t);
    let update = actions
        .iter()
        .find_map(|a| match a {
            RuntimeAction::SendDataUpdate(v) => Some(v.clone()),
            _ => None,
        })
        .expect("data update");
    assert!(update.contains(&(src::PLAY_PPM, SourceValue::Number(200.0))));
    assert!(update.contains(&(src::PLAY_K1_PPM, SourceValue::Number(100.0))));

    // Tapping stops: the live rate drops to 0 (still shown), the attempt's
    // average stays
    let t = t + Duration::from_secs(2);
    let_a_flush_interval_pass();
    let actions = c.on_event(RuntimeEvent::Tick(t), t);
    let update: Vec<_> = actions
        .iter()
        .flat_map(|a| match a {
            RuntimeAction::SendDataUpdate(v) => v.clone(),
            _ => Vec::new(),
        })
        .collect();
    assert!(update.contains(&(src::PLAY_PPM, SourceValue::Number(0.0))));
    assert!(!update.iter().any(|(s, _)| *s == src::PLAY_PPM_AVG));

    // A new history (written by IPC or after a save) reaches the pad
    c.state.tap.snapshot.history = Some(TapHistory {
        period_days: 7,
        attempts: 3,
        combined: ChannelHistory {
            average_ppm: Some(190.4),
            best_peak_ppm: Some(250.0),
            ..Default::default()
        },
        ..Default::default()
    });
    let t = t + Duration::from_secs(1);
    let_a_flush_interval_pass();
    let actions = c.on_event(RuntimeEvent::Tick(t), t);
    let update: Vec<_> = actions
        .iter()
        .flat_map(|a| match a {
            RuntimeAction::SendDataUpdate(v) => v.clone(),
            _ => Vec::new(),
        })
        .collect();
    assert!(update.contains(&(src::HISTORY_PPM_AVG, SourceValue::Number(190.0))));
    assert!(update.contains(&(src::HISTORY_PPM_PEAK, SourceValue::Number(250.0))));
    assert!(update.contains(&(src::HISTORY_PERIOD, SourceValue::Text("LAST 7 DAYS".into()))));
}

#[test]
fn losing_tosu_mid_play_ends_the_attempt_but_keeps_the_history() {
    let t0 = Instant::now();
    let mut c = controller(t0);
    c.state.device_connected = true;
    c.state.tap.snapshot.history = Some(TapHistory {
        period_days: 30,
        attempts: 1,
        ..Default::default()
    });
    c.on_event(RuntimeEvent::TosuConnectionChanged(true), t0);
    c.on_event(keys(0, 0, t0), t0);
    c.on_event(frame(true, 0.0, false), t0);
    let (_, t) = tap(&mut c, (0, 0), t0, 20);

    c.on_event(RuntimeEvent::TosuConnectionChanged(false), t);
    assert!(c.state.tap.snapshot.current.is_none());
    assert_eq!(c.state.tap.snapshot.unsaved_attempts, 1);

    let t = t + Duration::from_secs(1);
    let_a_flush_interval_pass();
    let actions = c.on_event(RuntimeEvent::Tick(t), t);
    let update: Vec<_> = actions
        .iter()
        .flat_map(|a| match a {
            RuntimeAction::SendDataUpdate(v) => v.clone(),
            _ => Vec::new(),
        })
        .collect();
    assert!(update.contains(&(src::PLAY_PPM_AVG, SourceValue::Clear)));
    assert!(update.contains(&(
        src::HISTORY_PERIOD,
        SourceValue::Text("LAST 30 DAYS".into())
    )));
}

#[test]
fn unsaved_attempts_survive_a_failed_save() {
    let t0 = Instant::now();
    let mut c = controller(t0);
    c.on_event(keys(0, 0, t0), t0);
    c.on_event(frame(true, 0.0, false), t0);
    let (_, t) = tap(&mut c, (0, 0), t0, 20);
    c.on_event(frame(false, 0.0, false), t);
    let attempts = saved(&settle(&mut c, t));
    assert_eq!(attempts.len(), 1);

    // The write failed: they are offered again on a later IDLE tick
    c.requeue_tap_attempts(attempts);
    let t = t + Duration::from_secs(60);
    assert_eq!(saved(&c.on_event(RuntimeEvent::Tick(t), t)).len(), 1);
}

#[test]
fn a_period_changed_during_play_is_saved_in_idle() {
    let t0 = Instant::now();
    let mut c = controller(t0);
    c.on_event(frame(true, 0.0, false), t0);
    // What the IPC handler does when the write is blocked
    c.state.tap.snapshot.period_days = 7;
    c.state.tap.period_unsaved = true;
    let t = t0 + Duration::from_secs(1);
    assert!(!c
        .on_event(RuntimeEvent::Tick(t), t)
        .iter()
        .any(|a| matches!(a, RuntimeAction::SaveTapStats { .. })));

    c.on_event(frame(false, 0.0, false), t);
    let actions = settle(&mut c, t);
    assert!(actions.contains(&RuntimeAction::SaveTapStats {
        attempts: Vec::new(),
        period_days: Some(7),
    }));
    assert!(!c.state.tap.period_unsaved);
}

// ---- Pad-timed presses ---------------------------------------------------------------

/// A controller with a connected pad that times its own presses, playing an
/// attempt that started at `t0`
fn pad_controller(t0: Instant) -> RuntimeController {
    let mut c = controller(t0);
    c.on_event(RuntimeEvent::PadPressTimes(true), t0);
    c.state.device_connected = true;
    c.on_event(keys(0, 0, t0), t0);
    c.on_event(frame(true, 0.0, false), t0);
    c
}

/// One batch of pad presses, delivered at `received` with the pad's clock at
/// `pad_now_us`
fn batch(presses: Vec<(u8, u64)>, pad_now_us: u64, received: Instant) -> RuntimeEvent {
    RuntimeEvent::PadPresses {
        now_us: pad_now_us,
        presses,
        dropped: 0,
        received,
    }
}

#[test]
fn pad_times_are_exact_even_when_batches_arrive_bunched() {
    let t0 = Instant::now();
    let mut c = pad_controller(t0);
    // A 400 BPM 1/4 burst: 1600 PPM, a press every 37.5 ms, alternating keys.
    // The pad's clock reads 1 s at t0. All of it arrives in one late batch,
    // the way a stalled delivery bunches tosu's messages.
    let base_us = 1_000_000u64;
    let presses: Vec<(u8, u64)> = (0..14u64)
        .map(|i| (1 + (i % 2) as u8, base_us + 100_000 + i * 37_500))
        .collect();
    let last_us = presses.last().unwrap().1;
    // The pad's clock zero is t0 - 1 s, learnt from a quick earlier batch
    c.on_event(batch(Vec::new(), base_us, t0), t0);
    let late = t0 + Duration::from_millis(800);
    c.on_event(batch(presses, last_us + 2_000, late), late);

    let s = c.state.tap.snapshot.current.clone().unwrap();
    assert_eq!(s.combined.presses, 14);
    let peak = s.combined.peak_ppm.unwrap();
    assert!((peak - 1600.0).abs() < 1.0, "peak {peak}");
    // Each key alone: 7 presses, every 75 ms
    assert!((s.k1.peak_ppm.unwrap() - 800.0).abs() < 1.0);
}

#[test]
fn once_the_pad_sends_presses_tosu_counts_are_ignored() {
    let t0 = Instant::now();
    let mut c = pad_controller(t0);
    let t = t0 + Duration::from_millis(300);
    c.on_event(batch(vec![(1, 300_000)], 300_000, t), t);
    // tosu reports the same press, and later ones the pad also sends
    c.on_event(keys(5, 0, t), t);
    let s = c.state.tap.snapshot.current.clone().unwrap();
    assert_eq!(s.combined.presses, 1);
}

#[test]
fn presses_the_pad_does_not_send_are_shown_but_not_saved() {
    // Your own replay: tosu's counters run from the replay, the pad is idle
    let t0 = Instant::now();
    let mut c = pad_controller(t0);
    let (_, t) = tap(&mut c, (0, 0), t0, 30);
    let live = c.state.tap.snapshot.current.clone().unwrap();
    assert!(live.replay);
    assert_eq!(live.combined.presses, 30, "shown, from tosu");
    c.on_event(frame(false, 0.0, false), t);
    assert!(c.state.tap.snapshot.last.as_ref().unwrap().replay);
    assert!(saved(&settle(&mut c, t)).is_empty(), "never saved");
}

#[test]
fn another_players_replay_is_shown_but_not_saved() {
    let t0 = Instant::now();
    let mut c = pad_controller(t0);
    let mut replay = frame(true, 0.0, false);
    if let RuntimeEvent::TosuTelemetry { other_player, .. } = &mut replay {
        *other_player = true;
    }
    c.on_event(replay, t0);
    // Shown from tosu at once, without waiting for the pad
    let (_, t) = tap(&mut c, (0, 0), t0, 3);
    let live = c.state.tap.snapshot.current.clone().unwrap();
    assert!(live.replay);
    assert_eq!(live.combined.presses, 3);
    c.on_event(frame(false, 0.0, false), t);
    assert!(saved(&settle(&mut c, t)).is_empty());
}

#[test]
fn a_replay_at_nightcore_speed_cannot_make_an_impossible_peak() {
    let t0 = Instant::now();
    let mut c = pad_controller(t0);
    let mut replay = frame(true, 0.0, false);
    if let RuntimeEvent::TosuTelemetry { other_player, .. } = &mut replay {
        *other_player = true;
    }
    c.on_event(replay, t0);
    // tosu delivers 600 PPM taps in bunches of 4, 2 ms apart
    let mut count = 0u32;
    for bunch in 0..30u64 {
        let at = t0 + Duration::from_millis(400 * bunch);
        for i in 0..4u64 {
            count += 1;
            let at = at + Duration::from_millis(2 * i);
            c.on_event(keys(count, 0, at), at);
        }
    }
    let peak = c
        .state
        .tap
        .snapshot
        .current
        .clone()
        .unwrap()
        .combined
        .peak_ppm
        .unwrap();
    assert!(peak < 1000.0, "peak {peak}");
}

#[test]
fn a_pad_without_press_times_uses_tosu_at_once() {
    let t0 = Instant::now();
    let mut c = controller(t0);
    c.state.device_connected = true;
    c.on_event(RuntimeEvent::PadPressTimes(false), t0);
    c.on_event(keys(0, 0, t0), t0);
    c.on_event(frame(true, 0.0, false), t0);
    tap(&mut c, (0, 0), t0, 2);
    assert_eq!(
        c.state
            .tap
            .snapshot
            .current
            .clone()
            .unwrap()
            .combined
            .presses,
        2
    );
}

#[test]
fn presses_from_before_a_retry_stay_out_of_the_new_attempt() {
    let t0 = Instant::now();
    let mut c = pad_controller(t0);
    c.on_event(batch(Vec::new(), 0, t0), t0);
    // Retry 2 s in; the batch still holds two presses from before it
    let retry = t0 + Duration::from_secs(2);
    c.on_event(frame(true, 30_000.0, false), retry);
    c.on_event(frame(true, 0.0, false), retry);
    let t = retry + Duration::from_millis(100);
    c.on_event(
        batch(
            vec![(1, 1_900_000), (2, 1_950_000), (1, 2_050_000)],
            2_100_000,
            t,
        ),
        t,
    );
    assert_eq!(
        c.state
            .tap
            .snapshot
            .current
            .clone()
            .unwrap()
            .combined
            .presses,
        1
    );
}
