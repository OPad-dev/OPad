//! Diagnostic event IDs and human-readable event mapping (§24.2, P2-1)

pub const DIAG_EVENT_NONE: u32 = 0;
pub const DIAG_EVENT_BOOT: u32 = 1;
pub const DIAG_EVENT_HID_MOUNTED: u32 = 2;
pub const DIAG_EVENT_HID_UNMOUNTED: u32 = 3;
pub const DIAG_EVENT_HID_SUSPENDED: u32 = 4;
pub const DIAG_EVENT_CDC_OPENED: u32 = 5;
pub const DIAG_EVENT_CDC_CLOSED: u32 = 6;
pub const DIAG_EVENT_NVS_INIT_FAILED: u32 = 7;
pub const DIAG_EVENT_NVS_ERASED: u32 = 8;
pub const DIAG_EVENT_LCD_INIT_FAILED: u32 = 9;
pub const DIAG_EVENT_DECODE_FAILED: u32 = 10;
pub const DIAG_EVENT_FRAME_TOO_LARGE: u32 = 11;
pub const DIAG_EVENT_CONFIG_REJECTED: u32 = 12;
pub const DIAG_EVENT_COUNTER_SYNC_REJECTED: u32 = 13;
pub const DIAG_EVENT_LAYOUT_REJECTED: u32 = 14;
pub const DIAG_EVENT_DEFERRED_WRITE_FLUSHED: u32 = 15;
pub const DIAG_EVENT_DISPLAY_SLEEP: u32 = 16;
pub const DIAG_EVENT_DISPLAY_WAKE: u32 = 17;
pub const DIAG_EVENT_LATENCY_OUTLIER: u32 = 18;
pub const DIAG_EVENT_CDC_WRITE_DROPPED: u32 = 19;
pub const DIAG_EVENT_BUFFER_OVERFLOW: u32 = 20;
pub const DIAG_EVENT_UNKNOWN_HOST_MSG: u32 = 21;
pub const DIAG_EVENT_DECODE_BYTES: u32 = 22;
pub const DIAG_EVENT_MEMORY: u32 = 23;
pub const DIAG_EVENT_UI_PERF: u32 = 24;

/// nanopb's error message for a DIAG_EVENT_DECODE_FAILED reason code
/// (`DIAG_DECODE_*` in `firmware/main/diag/diag.h`)
fn decode_reason_to_str(code: u32) -> &'static str {
    match code {
        1 => "string overflow",
        2 => "array overflow",
        3 => "bytes overflow",
        4 => "end-of-stream",
        5 => "wrong wire type",
        6 => "invalid wire_type",
        7 => "varint overflow",
        8 => "size too large",
        9 => "parent stream too short",
        10 => "invalid utf8",
        11 => "zero tag",
        12 => "io error",
        13 => "invalid union tag",
        _ => "other error",
    }
}

/// The HostToDevice payload a field number carries (`protocol/opad.proto`)
fn host_payload_name(field: u32) -> &'static str {
    match field {
        2 => "Hello",
        3 => "SetConfig",
        4 => "TimeSync",
        5 => "GameplayDisplayState",
        6 => "CounterSyncRequest",
        7 => "request_status",
        8 => "HostStatus",
        9 => "reset_latency_stats",
        10 => "DataUpdate",
        11 => "SetLayout",
        12 => "reset_layout",
        13 => "request_logs",
        14 => "ClaimOwnership",
        15 => "DetectPinRequest",
        16 => "run_board_test",
        17 => "PlayAnimation",
        18 => "restart",
        19 => "start_he_calibration",
        20 => "cancel_he_calibration",
        21 => "he_live_view",
        _ => "unknown payload",
    }
}

fn decode_failed_message(len: u32, detail: u32) -> String {
    if detail == 0 {
        // Firmware that recorded only the length
        return format!("Protobuf frame decode failed ({len} bytes)");
    }
    let reason = detail >> 24;
    let field = (detail >> 16) & 0xFF;
    let offset = detail & 0xFFFF;
    let payload = if field == 0 {
        "no payload field".to_string()
    } else {
        format!("{} (field {field})", host_payload_name(field))
    };
    format!(
        "Protobuf frame decode failed ({len} bytes): {}, in {payload}, at byte {offset}",
        decode_reason_to_str(reason)
    )
}

/// The pad's boot reason: ESP-IDF's `esp_reset_reason_t`, which the firmware
/// sends in DIAG_EVENT_BOOT (`esp_reset_reason()` in `firmware/main/app_main.c`)
pub fn reset_reason_to_str(reason: u32) -> &'static str {
    match reason {
        1 => "POWERON_RESET",
        2 => "EXT_RESET",
        3 => "SW_RESET",
        4 => "PANIC",
        5 => "INT_WDT",
        6 => "TASK_WDT",
        7 => "WDT",
        8 => "DEEPSLEEP",
        9 => "BROWNOUT",
        10 => "SDIO",
        11 => "USB",
        12 => "JTAG",
        13 => "EFUSE",
        14 => "PWR_GLITCH",
        15 => "CPU_LOCKUP",
        _ => "UNKNOWN",
    }
}

pub fn format_diag_event(event_id: u32, arg0: u32, arg1: u32) -> String {
    match event_id {
        DIAG_EVENT_BOOT => format!(
            "Device boot (reset reason: {} [{}])",
            reset_reason_to_str(arg0),
            arg0
        ),
        DIAG_EVENT_HID_MOUNTED => "USB HID mounted".to_string(),
        DIAG_EVENT_HID_UNMOUNTED => "USB HID unmounted".to_string(),
        DIAG_EVENT_HID_SUSPENDED => "USB HID suspended".to_string(),
        DIAG_EVENT_CDC_OPENED => "CDC serial port opened".to_string(),
        DIAG_EVENT_CDC_CLOSED => "CDC serial port closed".to_string(),
        DIAG_EVENT_NVS_INIT_FAILED => {
            format!("NVS initialization failed (esp_err: 0x{:04x})", arg0)
        }
        DIAG_EVENT_NVS_ERASED => "NVS flash partition erased/recovered".to_string(),
        DIAG_EVENT_LCD_INIT_FAILED => {
            format!("LCD initialization failed (esp_err: 0x{:04x})", arg0)
        }
        DIAG_EVENT_DECODE_FAILED => decode_failed_message(arg0, arg1),
        DIAG_EVENT_DECODE_BYTES => {
            let bytes: Vec<String> = arg0
                .to_le_bytes()
                .iter()
                .chain(arg1.to_le_bytes().iter())
                .map(|b| format!("{b:02x}"))
                .collect();
            format!("Undecodable frame starts {}", bytes.join(" "))
        }
        DIAG_EVENT_FRAME_TOO_LARGE => match arg1 {
            0 => format!("Skipped {} bytes looking for a frame start", arg0),
            1 => format!(
                "Dropped a partial frame the host stopped sending ({} bytes)",
                arg0
            ),
            2 => format!("Dropped {} frame(s) with a bad CRC", arg0),
            // Firmware predating the reason
            _ => format!("CDC RX frame exceeds max buffer (size: {} bytes)", arg0),
        },
        DIAG_EVENT_CONFIG_REJECTED => {
            format!("Device configuration rejected (reason code: {})", arg0)
        }
        DIAG_EVENT_COUNTER_SYNC_REJECTED => {
            format!("Counter sync rejected (reason code: {})", arg0)
        }
        DIAG_EVENT_LAYOUT_REJECTED => format!("Screen layout rejected (screen_id: {})", arg0),
        DIAG_EVENT_DEFERRED_WRITE_FLUSHED => {
            format!("Flushed {} deferred flash writes to NVS", arg0)
        }
        DIAG_EVENT_DISPLAY_SLEEP => "Display entered sleep mode".to_string(),
        DIAG_EVENT_DISPLAY_WAKE => "Display woke up".to_string(),
        DIAG_EVENT_LATENCY_OUTLIER => format!(
            "Key latency outlier detected: {} µs (count: {})",
            arg0, arg1
        ),
        DIAG_EVENT_CDC_WRITE_DROPPED => format!(
            "CDC write buffer dropped bytes (tried {}, dropped {})",
            arg0, arg1
        ),
        DIAG_EVENT_BUFFER_OVERFLOW => {
            format!("Diagnostic buffer overflow: {} events dropped", arg0)
        }
        DIAG_EVENT_UNKNOWN_HOST_MSG => format!("Unhandled host message payload tag: {}", arg0),
        DIAG_EVENT_MEMORY => format!(
            "Internal RAM free: {:.1} KB, largest DMA block {:.1} KB",
            arg0 as f64 / 1024.0,
            arg1 as f64 / 1024.0
        ),
        DIAG_EVENT_UI_PERF => format!(
            "UI perf ({}): {} frames, average {:.1} ms, longest {:.1} ms",
            match arg0 >> 16 {
                0 => "idle",
                1 => "song",
                2 => "result animation",
                _ => "?",
            },
            arg0 & 0xFFFF,
            (arg1 & 0xFFFF) as f64 / 10.0,
            (arg1 >> 16) as f64 / 10.0
        ),
        _ => format!(
            "Unknown diagnostic event (id: {}, arg0: {}, arg1: {})",
            event_id, arg0, arg1
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_failure_names_reason_payload_and_bytes() {
        let msg = format_diag_event(DIAG_EVENT_DECODE_FAILED, 231, (1 << 24) | (10 << 16) | 97);
        assert_eq!(
            msg,
            "Protobuf frame decode failed (231 bytes): string overflow, in DataUpdate \
             (field 10), at byte 97"
        );
        // Older firmware: arg1 is 0
        assert_eq!(
            format_diag_event(DIAG_EVENT_DECODE_FAILED, 175, 0),
            "Protobuf frame decode failed (175 bytes)"
        );
        let bytes = format_diag_event(DIAG_EVENT_DECODE_BYTES, 0x5201_2a08, 0x0000_0a08);
        assert_eq!(bytes, "Undecodable frame starts 08 2a 01 52 08 0a 00 00");
    }

    #[test]
    fn memory_and_ui_perf_events_read_as_numbers() {
        assert_eq!(
            format_diag_event(DIAG_EVENT_MEMORY, 200 * 1024, 150 * 1024 + 512),
            "Internal RAM free: 200.0 KB, largest DMA block 150.5 KB"
        );
        assert_eq!(
            format_diag_event(DIAG_EVENT_UI_PERF, (1 << 16) | 250, (183 << 16) | 72),
            "UI perf (song): 250 frames, average 7.2 ms, longest 18.3 ms"
        );
    }

    #[test]
    fn test_format_diag_event() {
        let msg = format_diag_event(DIAG_EVENT_BOOT, 1, 0);
        assert!(msg.contains("POWERON_RESET"));
        // esp_reset_reason_t 4 is a panic, not a watchdog
        assert!(format_diag_event(DIAG_EVENT_BOOT, 4, 0).contains("PANIC"));

        let msg = format_diag_event(DIAG_EVENT_HID_MOUNTED, 0, 0);
        assert_eq!(msg, "USB HID mounted");

        let msg = format_diag_event(DIAG_EVENT_BUFFER_OVERFLOW, 5, 0);
        assert_eq!(msg, "Diagnostic buffer overflow: 5 events dropped");

        let msg = format_diag_event(DIAG_EVENT_LATENCY_OUTLIER, 1250, 2);
        assert_eq!(msg, "Key latency outlier detected: 1250 µs (count: 2)");
    }
}
