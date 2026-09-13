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

pub fn reset_reason_to_str(reason: u32) -> &'static str {
    match reason {
        1 => "POWERON_RESET",
        3 => "SW_RESET",
        4 => "OWDT_RESET",
        5 => "DEEPSLEEP_RESET",
        6 => "SDIO_RESET",
        7 => "TG0WDT_SYS_RESET",
        8 => "TG1WDT_SYS_RESET",
        9 => "RTCWDT_SYS_RESET",
        10 => "INTRUSION_RESET",
        11 => "TGWDT_CPU_RESET",
        12 => "SW_CPU_RESET",
        13 => "RTCWDT_CPU_RESET",
        14 => "RTCWDT_BROWN_OUT_RESET",
        15 => "RTCWDT_RTC_RESET",
        _ => "UNKNOWN",
    }
}

pub fn format_diag_event(event_id: u32, arg0: u32, arg1: u32) -> String {
    match event_id {
        DIAG_EVENT_BOOT => format!("Device boot (reset reason: {} [{}])", reset_reason_to_str(arg0), arg0),
        DIAG_EVENT_HID_MOUNTED => "USB HID mounted".to_string(),
        DIAG_EVENT_HID_UNMOUNTED => "USB HID unmounted".to_string(),
        DIAG_EVENT_HID_SUSPENDED => "USB HID suspended".to_string(),
        DIAG_EVENT_CDC_OPENED => "CDC serial port opened".to_string(),
        DIAG_EVENT_CDC_CLOSED => "CDC serial port closed".to_string(),
        DIAG_EVENT_NVS_INIT_FAILED => format!("NVS initialization failed (esp_err: 0x{:04x})", arg0),
        DIAG_EVENT_NVS_ERASED => "NVS flash partition erased/recovered".to_string(),
        DIAG_EVENT_LCD_INIT_FAILED => format!("LCD initialization failed (esp_err: 0x{:04x})", arg0),
        DIAG_EVENT_DECODE_FAILED => format!("Protobuf frame decode failed (code: {})", arg0),
        DIAG_EVENT_FRAME_TOO_LARGE => format!("CDC RX frame exceeds max buffer (size: {} bytes)", arg0),
        DIAG_EVENT_CONFIG_REJECTED => format!("Device configuration rejected (reason code: {})", arg0),
        DIAG_EVENT_COUNTER_SYNC_REJECTED => format!("Counter sync rejected (reason code: {})", arg0),
        DIAG_EVENT_LAYOUT_REJECTED => format!("Screen layout rejected (screen_id: {})", arg0),
        DIAG_EVENT_DEFERRED_WRITE_FLUSHED => format!("Flushed {} deferred flash writes to NVS", arg0),
        DIAG_EVENT_DISPLAY_SLEEP => "Display entered sleep mode".to_string(),
        DIAG_EVENT_DISPLAY_WAKE => "Display woke up".to_string(),
        DIAG_EVENT_LATENCY_OUTLIER => format!("Key latency outlier detected: {} µs (count: {})", arg0, arg1),
        DIAG_EVENT_CDC_WRITE_DROPPED => format!("CDC write buffer dropped bytes (tried {}, dropped {})", arg0, arg1),
        DIAG_EVENT_BUFFER_OVERFLOW => format!("Diagnostic buffer overflow: {} events dropped", arg0),
        DIAG_EVENT_UNKNOWN_HOST_MSG => format!("Unhandled host message payload tag: {}", arg0),
        _ => format!("Unknown diagnostic event (id: {}, arg0: {}, arg1: {})", event_id, arg0, arg1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_diag_event() {
        let msg = format_diag_event(DIAG_EVENT_BOOT, 1, 0);
        assert!(msg.contains("POWERON_RESET"));

        let msg = format_diag_event(DIAG_EVENT_HID_MOUNTED, 0, 0);
        assert_eq!(msg, "USB HID mounted");

        let msg = format_diag_event(DIAG_EVENT_BUFFER_OVERFLOW, 5, 0);
        assert_eq!(msg, "Diagnostic buffer overflow: 5 events dropped");

        let msg = format_diag_event(DIAG_EVENT_LATENCY_OUTLIER, 1250, 2);
        assert_eq!(msg, "Key latency outlier detected: 1250 µs (count: 2)");
    }
}
