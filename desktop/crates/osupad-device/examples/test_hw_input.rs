use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn GetAsyncKeyState(vKey: i32) -> i16;
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn SetConsoleCtrlHandler(
        handler: Option<unsafe extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
    fn Beep(dwFreq: u32, dwDuration: u32) -> i32;
}

static RUNNING: AtomicBool = AtomicBool::new(true);

#[cfg(windows)]
unsafe extern "system" fn ctrl_handler(_ctrl_type: u32) -> i32 {
    RUNNING.store(false, Ordering::SeqCst);
    1 // Handled (prevents abrupt process termination so summary prints)
}

#[cfg(windows)]
fn is_key_down(vk: i32) -> bool {
    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

#[cfg(not(windows))]
fn is_key_down(_vk: i32) -> bool {
    false
}

fn play_chatter_alert() {
    #[cfg(windows)]
    {
        // Fire audio beep in background thread so the 5000 Hz polling loop is never delayed
        std::thread::spawn(|| {
            unsafe {
                // 1760 Hz (A6 tone), 40 ms duration - clear, crisp audible cue
                Beep(1760, 40);
            }
        });
    }
}

struct KeyTracker {
    name: String,
    vk: i32,
    is_down: bool,
    press_count: u32,
    last_pressed: Option<Instant>,
    last_released: Option<Instant>,
    last_hold_duration_ms: Option<f64>,
    definite_chatter_count: u32, // < 15ms (physical contact bounce)
    fast_flutter_count: u32,     // 15ms - 35ms (borderline rapid twitch / wobble)
    shortest_repress_ms: Option<f64>,
}

impl KeyTracker {
    fn new(name: String, vk: i32) -> Self {
        Self {
            name,
            vk,
            is_down: false,
            press_count: 0,
            last_pressed: None,
            last_released: None,
            last_hold_duration_ms: None,
            definite_chatter_count: 0,
            fast_flutter_count: 0,
            shortest_repress_ms: None,
        }
    }
}

fn parse_key_arg(arg: &str) -> Option<(char, i32)> {
    let s = arg.trim().to_uppercase();
    if s.len() == 1 {
        let c = s.chars().next()?;
        if c.is_ascii_alphanumeric() {
            return Some((c, c as i32));
        }
    }
    None
}

fn print_help() {
    println!("OPad In-Game Hardware Input & Chatter Diagnostic Tool");
    println!();
    println!("Usage:");
    println!("  test_hw_input.exe [KEY1] [KEY2] [OPTIONS]");
    println!();
    println!("Arguments:");
    println!("  KEY1        First osu! key to monitor (default: Z)");
    println!("  KEY2        Second osu! key to monitor (default: X)");
    println!();
    println!("Options:");
    println!("  --no-beep   Disable audio beep alert when chatter is detected");
    println!("  -h, --help  Show this help information");
    println!();
    println!("In-Game Controls:");
    println!("  F12         Finish session and view report (global hotkey, works while in osu!)");
    println!("  Ctrl+C      Finish session and view report (from terminal)");
    println!("  Escape      IGNORED - you can freely pause, retry, and navigate menus in osu!");
    println!();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return;
    }

    let sound_enabled = !args.iter().any(|a| a == "--no-beep" || a == "--silent");

    let positional: Vec<&String> = args
        .iter()
        .filter(|a| !a.starts_with('-'))
        .collect();

    let (k1_char, k1_vk) = if positional.len() > 0 {
        parse_key_arg(positional[0]).unwrap_or(('Z', 0x5A))
    } else {
        ('Z', 0x5A)
    };

    let (k2_char, k2_vk) = if positional.len() > 1 {
        parse_key_arg(positional[1]).unwrap_or(('X', 0x58))
    } else {
        ('X', 0x58)
    };

    #[cfg(windows)]
    unsafe {
        SetConsoleCtrlHandler(Some(ctrl_handler), 1);
    }

    println!("==================================================================");
    println!("        OPad In-Game Hardware Input & Switch Chatter Tester       ");
    println!("==================================================================");
    println!("Monitoring Keys: '{}' (0x{:02X}) and '{}' (0x{:02X}) globally", k1_char, k1_vk, k2_char, k2_vk);
    println!("Audio Alert:     {}", if sound_enabled { "ENABLED (short beep on switch chatter)" } else { "DISABLED" });
    println!("In-Game Exit:    Press [F12] or [Ctrl+C] at any time to finish & see report.");
    println!("                 (Escape is safe to use in-game - it will NOT close this tool)\n");
    println!("Launch osu!, play your song/map normally, and return here when done!\n");

    let mut k1 = KeyTracker::new(format!("Key 1 ('{}')", k1_char), k1_vk);
    let mut k2 = KeyTracker::new(format!("Key 2 ('{}')", k2_char), k2_vk);

    let start_time = Instant::now();
    let mut simultaneous_passes: u32 = 0;
    let mut last_activity = Instant::now();
    let mut last_status_print = Instant::now();

    // 0x7B is VK_F12
    const VK_F12: i32 = 0x7B;

    while RUNNING.load(Ordering::SeqCst) {
        if is_key_down(VK_F12) {
            println!("\n[F12 detected] Finishing session...");
            break;
        }

        let now = Instant::now();
        let k1_down = is_key_down(k1.vk);
        let k2_down = is_key_down(k2.vk);

        // --- Process Key 1 ---
        if k1_down && !k1.is_down {
            k1.is_down = true;
            k1.press_count += 1;
            last_activity = now;

            if let Some(rel) = k1.last_released {
                let delta_ms = (now - rel).as_secs_f64() * 1000.0;
                match k1.shortest_repress_ms {
                    Some(curr) if delta_ms < curr => k1.shortest_repress_ms = Some(delta_ms),
                    None => k1.shortest_repress_ms = Some(delta_ms),
                    _ => {}
                }

                if delta_ms < 15.0 {
                    k1.definite_chatter_count += 1;
                    println!(
                        "\n  🔴 [CHATTER DETECTED] {} re-pressed after only {:.2} ms! (Switch bounce)",
                        k1.name, delta_ms
                    );
                    if sound_enabled {
                        play_chatter_alert();
                    }
                } else if delta_ms <= 35.0 {
                    k1.fast_flutter_count += 1;
                    println!(
                        "\n  🟡 [RAPID FLUTTER]    {} re-pressed in {:.2} ms (Borderline rapid twitch / wobble)",
                        k1.name, delta_ms
                    );
                }
            }
            k1.last_pressed = Some(now);

            // Check simultaneous press with K2 (HW-03 ghosting test)
            if k2.is_down {
                if let Some(k2_press) = k2.last_pressed {
                    let delta_ms = (now - k2_press).as_secs_f64() * 1000.0;
                    if delta_ms <= 10.0 {
                        simultaneous_passes += 1;
                    }
                }
            }
        } else if !k1_down && k1.is_down {
            k1.is_down = false;
            if let Some(press) = k1.last_pressed {
                k1.last_hold_duration_ms = Some((now - press).as_secs_f64() * 1000.0);
            }
            k1.last_released = Some(now);
        }

        // --- Process Key 2 ---
        if k2_down && !k2.is_down {
            k2.is_down = true;
            k2.press_count += 1;
            last_activity = now;

            if let Some(rel) = k2.last_released {
                let delta_ms = (now - rel).as_secs_f64() * 1000.0;
                match k2.shortest_repress_ms {
                    Some(curr) if delta_ms < curr => k2.shortest_repress_ms = Some(delta_ms),
                    None => k2.shortest_repress_ms = Some(delta_ms),
                    _ => {}
                }

                if delta_ms < 15.0 {
                    k2.definite_chatter_count += 1;
                    println!(
                        "\n  🔴 [CHATTER DETECTED] {} re-pressed after only {:.2} ms! (Switch bounce)",
                        k2.name, delta_ms
                    );
                    if sound_enabled {
                        play_chatter_alert();
                    }
                } else if delta_ms <= 35.0 {
                    k2.fast_flutter_count += 1;
                    println!(
                        "\n  🟡 [RAPID FLUTTER]    {} re-pressed in {:.2} ms (Borderline rapid twitch / wobble)",
                        k2.name, delta_ms
                    );
                }
            }
            k2.last_pressed = Some(now);

            // Check simultaneous press with K1 (HW-03 ghosting test)
            if k1.is_down {
                if let Some(k1_press) = k1.last_pressed {
                    let delta_ms = (now - k1_press).as_secs_f64() * 1000.0;
                    if delta_ms <= 10.0 {
                        simultaneous_passes += 1;
                    }
                }
            }
        } else if !k2_down && k2.is_down {
            k2.is_down = false;
            if let Some(press) = k2.last_pressed {
                k2.last_hold_duration_ms = Some((now - press).as_secs_f64() * 1000.0);
            }
            k2.last_released = Some(now);
        }

        // Periodic live status display (every 500 ms while active)
        if now.duration_since(last_status_print) >= Duration::from_millis(500) {
            let total_taps = k1.press_count + k2.press_count;
            let elapsed_sec = now.duration_since(start_time).as_secs_f64();
            let total_chatter = k1.definite_chatter_count + k2.definite_chatter_count;

            if total_taps > 0 && now.duration_since(last_activity) < Duration::from_secs(4) {
                let rate = total_taps as f64 / elapsed_sec;
                let bpm = (rate * 60.0) / 4.0; // 1/4 stream equivalent
                print!(
                    "\r[IN-GAME] Taps: {:4} ({}: {:3}, {}: {:3}) | Chatter: {:2} | Speed: {:.1} p/s ({:.0} BPM)    ",
                    total_taps, k1_char, k1.press_count, k2_char, k2.press_count, total_chatter, rate, bpm
                );
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
            last_status_print = now;
        }

        // 0.2 ms polling interval (5000 samples/second) for sub-millisecond hardware accuracy
        std::thread::sleep(Duration::from_micros(200));
    }

    // =========================================================================
    //                            POST-GAME SUMMARY
    // =========================================================================
    let total_taps = k1.press_count + k2.press_count;
    let total_chatter = k1.definite_chatter_count + k2.definite_chatter_count;
    let total_flutter = k1.fast_flutter_count + k2.fast_flutter_count;
    let elapsed = start_time.elapsed();
    let mins = elapsed.as_secs() / 60;
    let secs = elapsed.as_secs() % 60;

    println!("\n\n========================= PLAY SESSION SUMMARY =========================");
    println!("Session Duration:      {}m {:02}s ({:.1} total seconds)", mins, secs, elapsed.as_secs_f64());
    println!("Total Registered Taps: {}", total_taps);
    println!("  - {}: {:4} taps", k1.name, k1.press_count);
    println!("  - {}: {:4} taps", k2.name, k2.press_count);
    println!();
    println!("Timing Analysis:");
    println!(
        "  - Definite Chatter (< 15ms):   {} events ({:.2}%)",
        total_chatter,
        if total_taps > 0 { (total_chatter as f64 / total_taps as f64) * 100.0 } else { 0.0 }
    );
    println!(
        "  - Rapid Flutter (15ms - 35ms): {} events ({:.2}%)",
        total_flutter,
        if total_taps > 0 { (total_flutter as f64 / total_taps as f64) * 100.0 } else { 0.0 }
    );
    println!(
        "  - Clean Alternation (> 35ms):  {} events ({:.2}%)",
        total_taps.saturating_sub(total_chatter + total_flutter),
        if total_taps > 0 {
            (total_taps.saturating_sub(total_chatter + total_flutter) as f64 / total_taps as f64) * 100.0
        } else {
            0.0
        }
    );
    println!();
    println!("Fastest Single-Key Re-press Recorded:");
    if let Some(k1_min) = k1.shortest_repress_ms {
        println!("  - {}: {:.2} ms", k1.name, k1_min);
    } else {
        println!("  - {}: None recorded", k1.name);
    }
    if let Some(k2_min) = k2.shortest_repress_ms {
        println!("  - {}: {:.2} ms", k2.name, k2_min);
    } else {
        println!("  - {}: None recorded", k2.name);
    }

    if simultaneous_passes > 0 {
        println!();
        println!("Anti-Ghosting (HW-03):");
        println!("  - Simultaneous Presses: {} clean dual-key events registered without drop", simultaneous_passes);
    }

    println!("\n========================== DIAGNOSTIC VERDICT ==========================");
    if total_chatter > 0 {
        let min_delta = match (k1.shortest_repress_ms, k2.shortest_repress_ms) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (None, None) => 0.0,
        };
        println!("  ⚠️  HARDWARE SWITCH CHATTER CONFIRMED!");
        println!("  The tool recorded {} switch bounces that occurred in under 15 ms (fastest: {:.2} ms).", total_chatter, min_delta);
        println!("  Human fingers CANNOT physically press and re-press a mechanical switch in < 15 ms.");
        println!("  This proves that your double-presses are genuine hardware contact chatter, NOT your fault.");
        println!();
        let recommended_debounce = ((min_delta + 3.0).ceil() as u32 * 1000).clamp(5000, 15000);
        println!("  Recommended Fix:");
        println!("  - Increase the firmware debounce lockout to {} µs ({:.1} ms).", recommended_debounce, recommended_debounce as f64 / 1000.0);
        println!("  - Because OPad uses eager debounce, increasing this setting adds ZERO input");
        println!("    latency to your initial keypress, while cleanly filtering out post-release bounce!");
    } else if total_flutter > 0 && total_taps >= 50 {
        println!("  🔍 BORDERLINE RE-PRESSES DETECTED (15ms - 35ms):");
        println!("  Zero definite chatter (< 15 ms) was observed, but {} rapid flutters occurred.", total_flutter);
        println!("  This usually happens when a finger twitches, vibrates near the actuation point, or");
        println!("  slides off the keycap. If you experience unexpected double-hits during fast streams,");
        println!("  raising debounce lockout slightly to 6000-8000 µs can help stabilize the key feel.");
    } else if total_taps >= 50 {
        println!("  ✅ CLEAN HARDWARE PASS - NO SWITCH CHATTER DETECTED!");
        println!("  Across all {} taps, every single re-press was clean and well above 15 ms.", total_taps);
        println!("  Your mechanical switches and contact leaves are physically rock-solid.");
        println!("  Any missed notes or accidental double-taps experienced in-game are caused by");
        println!("  finger stamina/finger twitch/key release timing rather than hardware bounce.");
    } else {
        println!("  ℹ️  INSUFFICIENT SAMPLE SIZE (Tapped fewer than 50 times).");
        println!("  Play a full beatmap and tap vigorously to get a comprehensive diagnostic.");
    }
    println!("========================================================================\n");
}
