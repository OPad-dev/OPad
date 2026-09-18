use std::time::{Duration, Instant};

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn GetAsyncKeyState(vKey: i32) -> i16;
}

#[cfg(windows)]
fn is_key_down(vk: i32) -> bool {
    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

#[cfg(not(windows))]
fn is_key_down(_vk: i32) -> bool {
    false
}

const VK_Z: i32 = 0x5A; // Key 1 default
const VK_X: i32 = 0x58; // Key 2 default

struct KeyTracker {
    name: &'static str,
    vk: i32,
    is_down: bool,
    press_count: u32,
    last_pressed: Option<Instant>,
    last_released: Option<Instant>,
    chatter_count: u32,
}

impl KeyTracker {
    fn new(name: &'static str, vk: i32) -> Self {
        Self {
            name,
            vk,
            is_down: false,
            press_count: 0,
            last_pressed: None,
            last_released: None,
            chatter_count: 0,
        }
    }
}

fn main() {
    println!("============================================================");
    println!("        osu!pad Hardware Input & Chatter Verification       ");
    println!("          Tests HW-03 (Ghosting) & HW-04 (Streaming)        ");
    println!("============================================================");
    println!("Monitoring Key 1 ('Z') and Key 2 ('X') globally...");
    println!("Press Ctrl+C or Escape at any time to finish and view results.\n");

    let mut k1 = KeyTracker::new("Key 1 (Z)", VK_Z);
    let mut k2 = KeyTracker::new("Key 2 (X)", VK_X);

    let start_time = Instant::now();
    let mut simultaneous_passes: u32 = 0;
    let mut last_activity = Instant::now();
    let mut last_status_print = Instant::now();

    // Chatter threshold in milliseconds (mechanical switch bounce usually < 15ms)
    let chatter_threshold_ms = 15.0;

    loop {
        if is_key_down(0x1B) { // Escape key to exit
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
                if delta_ms < chatter_threshold_ms {
                    k1.chatter_count += 1;
                    println!(
                        "  ⚠️  [CHATTER] {} re-pressed after only {:.2} ms! (Switch bounce / double-tap)",
                        k1.name, delta_ms
                    );
                }
            }
            k1.last_pressed = Some(now);

            // Check simultaneous press with K2
            if k2.is_down {
                if let Some(k2_press) = k2.last_pressed {
                    let delta_ms = (now - k2_press).as_secs_f64() * 1000.0;
                    if delta_ms <= 10.0 {
                        simultaneous_passes += 1;
                        println!(
                            "  ✓ [HW-03 PASS] Simultaneous press: K1 and K2 down within {:.2} ms (No ghosting)",
                            delta_ms
                        );
                    }
                }
            }
        } else if !k1_down && k1.is_down {
            k1.is_down = false;
            k1.last_released = Some(now);
        }

        // --- Process Key 2 ---
        if k2_down && !k2.is_down {
            k2.is_down = true;
            k2.press_count += 1;
            last_activity = now;

            if let Some(rel) = k2.last_released {
                let delta_ms = (now - rel).as_secs_f64() * 1000.0;
                if delta_ms < chatter_threshold_ms {
                    k2.chatter_count += 1;
                    println!(
                        "  ⚠️  [CHATTER] {} re-pressed after only {:.2} ms! (Switch bounce / double-tap)",
                        k2.name, delta_ms
                    );
                }
            }
            k2.last_pressed = Some(now);

            // Check simultaneous press with K1
            if k1.is_down {
                if let Some(k1_press) = k1.last_pressed {
                    let delta_ms = (now - k1_press).as_secs_f64() * 1000.0;
                    if delta_ms <= 10.0 {
                        simultaneous_passes += 1;
                        println!(
                            "  ✓ [HW-03 PASS] Simultaneous press: K2 and K1 down within {:.2} ms (No ghosting)",
                            delta_ms
                        );
                    }
                }
            }
        } else if !k2_down && k2.is_down {
            k2.is_down = false;
            k2.last_released = Some(now);
        }

        // Periodic live status display (every 1 second if keys are being tapped)
        if now.duration_since(last_status_print) >= Duration::from_millis(1000) {
            let total_taps = k1.press_count + k2.press_count;
            let elapsed_sec = now.duration_since(start_time).as_secs_f64();
            if total_taps > 0 && now.duration_since(last_activity) < Duration::from_secs(3) {
                let rate = total_taps as f64 / elapsed_sec;
                let bpm = (rate * 60.0) / 4.0; // 1/4 stream BPM equivalent
                print!(
                    "\r[STREAM MONITOR] Taps: {} (K1: {}, K2: {}) | Speed: {:.1} presses/sec ({:.0} BPM stream)    ",
                    total_taps, k1.press_count, k2.press_count, rate, bpm
                );
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
            last_status_print = now;
        }

        // 0.2 ms polling resolution (5000 samples/second) for sub-millisecond precision
        std::thread::sleep(Duration::from_micros(200));
    }

    println!("\n\n==================== TEST SUMMARY ====================");
    println!("Elapsed Time:       {:.1} seconds", start_time.elapsed().as_secs_f64());
    println!("Total Presses:      {}", k1.press_count + k2.press_count);
    println!("  - Key 1 (Z):      {} presses", k1.press_count);
    println!("  - Key 2 (X):      {} presses", k2.press_count);
    println!();
    println!("HW-03 (Ghosting):   {} simultaneous press events registered cleanly", simultaneous_passes);
    if simultaneous_passes > 0 {
        println!("  -> HW-03 RESULT:  PASS (Both keys report down concurrently with 0 ghosting)");
    } else {
        println!("  -> HW-03 RESULT:  NOT TRIGGERED (Press K1+K2 together to test)");
    }
    println!();
    let total = k1.press_count + k2.press_count;
    let total_chatter = k1.chatter_count + k2.chatter_count;
    let chatter_pct = if total > 0 { (total_chatter as f64 / total as f64) * 100.0 } else { 0.0 };
    println!("Switch Chatter:     {} double-tap bounce events detected ({:.2}%)", total_chatter, chatter_pct);
    println!("  - Key 1 Chatter:  {}", k1.chatter_count);
    println!("  - Key 2 Chatter:  {}", k2.chatter_count);
    if total_chatter > 0 {
        println!("  -> DIAGNOSTIC:    Mechanical contact bounce was observed < 15ms after release.");
        println!("                    To eliminate this, increase Debounce Lockout to 5000 µs (5ms).");
    } else if total >= 20 {
        println!("  -> HW-04 RESULT:  PASS (Zero switch chatter or double-taps observed)");
    }
    println!("======================================================");
}
