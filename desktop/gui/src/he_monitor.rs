//! HE phantom and double-press monitor for the Diagnostics page. Follows the
//! HE debug firmware's log (`<data>/he-debug.log`, which the daemon writes,
//! desktop/daemon/src/he_debug_log.rs) and judges every press from the pad's
//! own readings: the 12 frames (250 us each) before it, its time and the
//! key's previous edges, all on the pad's clock. Normal firmware sends no
//! `he.*` lines, so the monitor then shows that it has nothing to follow.
//!
//! The host's key events cannot do this: a phantom is often a 1 ms press,
//! shorter than the switch tester's 4 ms polling. The rules are the ones used
//! in docs/history/2026-10-22-he-phantom-presses.md.

use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::time::Duration;

/// A press whose last frame jumped more than this from a flat reading
const PHANTOM_JUMP: i32 = 60;
/// "Flat": the frames before the jump span less than this, at rest or held.
/// A finger moves the reading steadily over several frames instead
const FLAT_SPREAD: i32 = 50;
/// A press released this soon is not a finger
const SHORT_MS: u32 = 8;
/// A key pressed again this soon after its release
const REPRESS_MS: u32 = 25;

#[derive(Debug, Clone, PartialEq)]
pub enum HeFinding {
    /// `jump`: counts in the last frame, `frames`: the 12 logged readings
    Phantom { jump: i32, frames: String },
    /// Held only `ms`
    Short { ms: u32 },
    /// Pressed again `ms` after its release
    Repress { ms: u32 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum HeMonitorEvent {
    /// The pad's once-a-second line: the monitor is following live data
    Alive,
    /// The pad pressed key `key` (1 or 2)
    Press { key: u8 },
    /// `time`: the log's wall-clock time of the press
    Found {
        key: u8,
        time: String,
        finding: HeFinding,
    },
}

/// Turns log lines into events; one per log file followed
#[derive(Default)]
pub struct Judge {
    last_down: [Option<u32>; 2],
    last_up: [Option<u32>; 2],
    /// A press waiting for its he.pre line: key index and time
    pending: Option<(usize, String)>,
}

/// `2026-10-22 23:48:04.328 INFO  he.edge  [   146097 ms] K2 DOWN ...` ->
/// (time, tag, pad ms, the rest)
fn split(line: &str) -> Option<(&str, &str, u32, &str)> {
    let mut it = line.split_whitespace();
    let _date = it.next()?;
    let time = it.next()?;
    let _level = it.next()?;
    let tag = it.next()?;
    let open = line.find('[')?;
    let close = open + line[open..].find("ms]")?;
    let ms = line[open + 1..close].trim().parse().ok()?;
    Some((time, tag, ms, line[close + 3..].trim()))
}

fn key_index(word: &str) -> Option<usize> {
    match word {
        "K1" => Some(0),
        "K2" => Some(1),
        _ => None,
    }
}

impl Judge {
    pub fn line(&mut self, line: &str) -> Vec<HeMonitorEvent> {
        let mut out = Vec::new();
        let Some((time, tag, ms, rest)) = split(line) else {
            return out;
        };
        let mut words = rest.split_whitespace();
        match tag {
            "he.sec" => out.push(HeMonitorEvent::Alive),
            "he.edge" => {
                let (Some(k), Some(ev)) = (words.next().and_then(key_index), words.next()) else {
                    return out;
                };
                let key = k as u8 + 1;
                let time = time.get(..8).unwrap_or(time).to_string();
                if ev == "DOWN" {
                    out.push(HeMonitorEvent::Press { key });
                    if let Some(up) = self.last_up[k] {
                        let gap = ms.wrapping_sub(up);
                        if gap < REPRESS_MS {
                            out.push(HeMonitorEvent::Found {
                                key,
                                time: time.clone(),
                                finding: HeFinding::Repress { ms: gap },
                            });
                        }
                    }
                    self.last_down[k] = Some(ms);
                    self.pending = Some((k, time));
                } else if ev == "UP" {
                    if let Some(down) = self.last_down[k] {
                        let held = ms.wrapping_sub(down);
                        if held < SHORT_MS {
                            out.push(HeMonitorEvent::Found {
                                key,
                                time,
                                finding: HeFinding::Short { ms: held },
                            });
                        }
                    }
                    self.last_up[k] = Some(ms);
                }
            }
            "he.pre" => {
                let k = words.next().and_then(key_index);
                let Some((pk, time)) = self.pending.take() else {
                    return out;
                };
                if k != Some(pk) {
                    return out;
                }
                // "x" (no reading in that frame) ends the judgement
                let v: Option<Vec<i32>> = words.clone().map(|w| w.parse().ok()).collect();
                let Some(v) = v.filter(|v| v.len() >= 3) else {
                    return out;
                };
                let (before, last) = (&v[..v.len() - 1], v[v.len() - 1]);
                let jump = last - before[before.len() - 1];
                let spread = before.iter().max().unwrap_or(&0) - before.iter().min().unwrap_or(&0);
                if jump > PHANTOM_JUMP && spread < FLAT_SPREAD {
                    out.push(HeMonitorEvent::Found {
                        key: pk as u8 + 1,
                        time,
                        finding: HeFinding::Phantom {
                            jump,
                            frames: words.collect::<Vec<_>>().join(" "),
                        },
                    });
                }
            }
            _ => {}
        }
        out
    }
}

/// Follows the daemon's he-debug.log from its end, on a thread of its own,
/// and yields what [`Judge`] finds. Rotation (the daemon renames the file at
/// 50 MB) is followed by reopening when the file shrinks. The thread stops
/// once the subscription is dropped.
pub fn stream() -> impl futures_util::Stream<Item = HeMonitorEvent> {
    iced::stream::channel(
        64,
        |mut output: iced::futures::channel::mpsc::Sender<HeMonitorEvent>| async move {
            use iced::futures::SinkExt;
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            std::thread::spawn(move || {
                let Ok(path) = opad_model::paths::data_dir().map(|d| d.join("he-debug.log")) else {
                    return;
                };
                let mut reader: Option<BufReader<std::fs::File>> = None;
                let mut pos = 0u64;
                let mut partial = String::new();
                let mut judge = Judge::default();
                while !tx.is_closed() {
                    let len = std::fs::metadata(&path).map(|m| m.len()).ok();
                    match len {
                        None => reader = None,
                        Some(len) if reader.is_none() => {
                            // First open: from the end. After a rotation: the
                            // new file from its start
                            if let Ok(mut f) = std::fs::File::open(&path) {
                                pos = if pos == 0 { len } else { 0 };
                                if f.seek(SeekFrom::Start(pos)).is_ok() {
                                    reader = Some(BufReader::new(f));
                                    judge = Judge::default();
                                }
                            }
                        }
                        Some(len) if len < pos => {
                            reader = None;
                            pos = 1; // reopen from the start
                            continue;
                        }
                        Some(_) => {
                            let r = reader.as_mut().expect("opened above");
                            while let Ok(n @ 1..) = r.read_line(&mut partial) {
                                pos += n as u64;
                                if partial.ends_with('\n') {
                                    for e in judge.line(&partial) {
                                        let _ = tx.send(e);
                                    }
                                    partial.clear();
                                }
                            }
                        }
                    }
                    std::thread::sleep(Duration::from_millis(200));
                }
            });
            while let Some(event) = rx.recv().await {
                if output.send(event).await.is_err() {
                    break;
                }
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(j: &mut Judge, lines: &[&str]) -> Vec<HeMonitorEvent> {
        lines.iter().flat_map(|l| j.line(l)).collect()
    }

    const T: &str = "2026-10-22 23:09:09.100 INFO  ";

    #[test]
    fn a_jump_from_rest_is_a_phantom_and_a_1_ms_press() {
        let mut j = Judge::default();
        let e = feed(
            &mut j,
            &[
                &format!("{T}he.edge  [   381514 ms] K2 DOWN A>P raw+168 act+140"),
                &format!(
                    "{T}he.pre   [   381514 ms] K2 +15 +4 +16 +1 +16 +2 -1 +1 +14 -5 +15 +168"
                ),
                &format!("{T}he.edge  [   381515 ms] K2 UP P>R raw+20 act+140"),
            ],
        );
        assert_eq!(e[0], HeMonitorEvent::Press { key: 2 });
        assert!(matches!(
            e[1],
            HeMonitorEvent::Found {
                key: 2,
                finding: HeFinding::Phantom { jump: 153, .. },
                ..
            }
        ));
        assert_eq!(
            e[2],
            HeMonitorEvent::Found {
                key: 2,
                time: "23:09:09".into(),
                finding: HeFinding::Short { ms: 1 }
            }
        );
    }

    #[test]
    fn a_fast_real_press_is_not_a_phantom() {
        let mut j = Judge::default();
        let e = feed(
            &mut j,
            &[
                &format!("{T}he.edge  [   1000 ms] K1 DOWN A>P raw+140"),
                &format!(
                    "{T}he.pre   [   1000 ms] K1 +12 +26 +41 +64 +66 +64 +71 +78 +94 +106 +78 +140"
                ),
            ],
        );
        assert_eq!(e, vec![HeMonitorEvent::Press { key: 1 }]);
    }

    #[test]
    fn a_spike_while_held_and_a_quick_re_press_are_found() {
        let mut j = Judge::default();
        let e = feed(
            &mut j,
            &[
                &format!("{T}he.edge  [   2000 ms] K2 UP P>R raw+600"),
                &format!("{T}he.edge  [   2015 ms] K2 DOWN R>P raw+999"),
                &format!("{T}he.pre   [   2015 ms] K2 +639 +654 +651 +655 +659 +635 +642 +648 +650 +655 +635 +999"),
            ],
        );
        assert!(e.contains(&HeMonitorEvent::Found {
            key: 2,
            time: "23:09:09".into(),
            finding: HeFinding::Repress { ms: 15 }
        }));
        assert!(e.iter().any(|x| matches!(
            x,
            HeMonitorEvent::Found {
                finding: HeFinding::Phantom { jump: 364, .. },
                ..
            }
        )));
    }

    #[test]
    fn other_lines_and_ordinary_presses_find_nothing() {
        let mut j = Judge::default();
        let e = feed(
            &mut j,
            &[
                &format!("{T}he.sec   [   3000 ms] K1 -33..+30 avg-2 K2 -29..+34 avg-1"),
                &format!("{T}he.edge  [   3100 ms] K1 DOWN A>P raw+130"),
                &format!("{T}he.pre   [   3100 ms] K1 +60 +70 +80 +90 +95 +100 +105 +110 +115 +120 +125 +130"),
                &format!("{T}he.edge  [   3180 ms] K1 UP P>R raw+300"),
                &format!("{T}he.edge  [   3300 ms] K1 DOWN R>P raw+400"),
                "garbage",
            ],
        );
        assert_eq!(
            e,
            vec![
                HeMonitorEvent::Alive,
                HeMonitorEvent::Press { key: 1 },
                HeMonitorEvent::Press { key: 1 }
            ]
        );
    }
}
