//! Board test: what the pad measures on the carrier and input module PCBs
//! (`BoardTestResult` in `protocol/osupad.proto`), and the pass/fail checks
//! the CLI and GUI show for it.
//!
//! The pad only measures; every judgement is made here so it can change
//! without a firmware update. Pinout and part names: `hardware/pcb/V1/README.md`.

use serde::{Deserialize, Serialize};

/// Where the carrier routes an MX module's keys (connector pins 3 and 4)
pub const CARRIER_KEY1_GPIO: u32 = 10;
pub const CARRIER_KEY2_GPIO: u32 = 7;

/// The spare module lines, in connector order: (GPIO, connector pin)
const SPARE_LINES: [(u32, u32); 3] = [(6, 6), (4, 7), (2, 8)];

/// A reversed cable lifts ID to about 1.5 V in the reverse probe; a correct one
/// leaves it near 0 V
const REVERSE_PROBE_MIN_MV: u32 = 700;
/// What the firmware calls "something is on ID" (board_module_from_id)
const ID_PRESENT_MIN_MV: u32 = 120;
const ID_TOO_HIGH_MV: u32 = 1600;
/// MX divider: 3.3 V × 10k / 110k = 0.30 V
const MX_ID_RANGE_MV: (u32, u32) = (200, 450);

/// Input module on the carrier's connector, from its ID voltage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputModule {
    #[default]
    Unknown,
    None,
    Mx,
    HallEffect,
}

impl InputModule {
    pub fn label(self) -> &'static str {
        match self {
            InputModule::Unknown => "unknown",
            InputModule::None => "no module",
            InputModule::Mx => "MX module",
            InputModule::HallEffect => "Hall Effect module",
        }
    }
}

/// The pad's measurements. GPIO masks have bit n for GPIOn.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardTestReport {
    /// False when the pad refused (a map is being played); see `message`
    pub ran: bool,
    pub message: String,
    pub boot_module: InputModule,
    pub module: InputModule,
    /// ID voltage with the pad's pull-down off; `None` = not measured
    pub id_mv: Option<u32>,
    /// ID voltage with the ~45k internal pull-down on
    pub id_loaded_mv: Option<u32>,
    /// ID voltage with GPIO2 pulled up (reversed cable probe); `None` = not run
    pub reverse_probe_mv: Option<u32>,
    pub key1_gpio: u32,
    pub key2_gpio: u32,
    /// False while key input is off (Hall Effect module on v1 firmware)
    pub keys_enabled: bool,
    pub tested_gpios: u64,
    pub high_with_pullup: u64,
    pub high_with_pulldown: u64,
    pub bridged_gpios: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Skip,
    Pass,
    Warn,
    Fail,
}

impl CheckStatus {
    pub fn label(self) -> &'static str {
        match self {
            CheckStatus::Skip => "skip",
            CheckStatus::Pass => "pass",
            CheckStatus::Warn => "warn",
            CheckStatus::Fail => "FAIL",
        }
    }
}

/// Something the app can fix itself
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardFix {
    /// Set Key 1 = GPIO10 and Key 2 = GPIO7
    UseCarrierKeyPins,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BoardCheck {
    pub name: &'static str,
    pub status: CheckStatus,
    pub detail: String,
    pub fix: Option<BoardFix>,
}

impl BoardCheck {
    fn new(name: &'static str, status: CheckStatus, detail: impl Into<String>) -> Self {
        BoardCheck {
            name,
            status,
            detail: detail.into(),
            fix: None,
        }
    }
}

/// The worst status among the checks (Skip when there are none)
pub fn overall(checks: &[BoardCheck]) -> CheckStatus {
    checks
        .iter()
        .map(|c| c.status)
        .max()
        .unwrap_or(CheckStatus::Skip)
}

fn volts(mv: u32) -> String {
    format!("{:.2} V", mv as f64 / 1000.0)
}

fn bit(gpio: u32) -> u64 {
    1u64.checked_shl(gpio).unwrap_or(0)
}

impl BoardTestReport {
    fn tested(&self, gpio: u32) -> bool {
        self.tested_gpios & bit(gpio) != 0
    }
    fn up(&self, gpio: u32) -> bool {
        self.high_with_pullup & bit(gpio) != 0
    }
    fn down(&self, gpio: u32) -> bool {
        self.high_with_pulldown & bit(gpio) != 0
    }
    fn bridged(&self, gpio: u32) -> bool {
        self.bridged_gpios & bit(gpio) != 0
    }
    fn on_carrier_pins(&self) -> bool {
        self.key1_gpio == CARRIER_KEY1_GPIO && self.key2_gpio == CARRIER_KEY2_GPIO
    }
}

/// Turn the pad's measurements into checks, in the order to show them
pub fn evaluate(r: &BoardTestReport) -> Vec<BoardCheck> {
    if !r.ran {
        let why = if r.message.is_empty() {
            "the pad did not run the test".to_string()
        } else {
            format!("The pad did not run the test: {}.", r.message)
        };
        return vec![BoardCheck::new("Board test", CheckStatus::Skip, why)];
    }

    let mut checks = vec![module_check(r)];

    if r.boot_module != r.module
        && r.boot_module != InputModule::Unknown
        && r.module != InputModule::Unknown
    {
        checks.push(BoardCheck::new(
            "Since boot",
            CheckStatus::Warn,
            format!(
                "The pad started with {} and now sees {}. Key pins and the Hall Effect \
                 lockout are chosen at boot, so unplug the pad and plug it back in. \
                 Don't swap the module cable while the pad is powered.",
                r.boot_module.label(),
                r.module.label()
            ),
        ));
    }

    let mx = r.module == InputModule::Mx || r.boot_module == InputModule::Mx;
    if mx {
        checks.push(key_pins_check(r));
    }

    if !r.keys_enabled {
        checks.push(BoardCheck::new(
            "Key lines",
            CheckStatus::Skip,
            "Not tested: key input is off because a Hall Effect module is connected.",
        ));
    } else {
        checks.push(key_line_check(r, 1, r.key1_gpio, mx));
        checks.push(key_line_check(r, 2, r.key2_gpio, mx));
    }

    if let Some(c) = spare_lines_check(r) {
        checks.push(c);
    }
    checks
}

fn module_check(r: &BoardTestReport) -> BoardCheck {
    let (Some(id), Some(loaded)) = (r.id_mv, r.id_loaded_mv) else {
        return BoardCheck::new(
            "Input module",
            CheckStatus::Fail,
            "The pad could not read the ID voltage (ADC error). Restart the pad and run \
             the test again.",
        );
    };
    match r.module {
        InputModule::Mx if (MX_ID_RANGE_MV.0..=MX_ID_RANGE_MV.1).contains(&id) => BoardCheck::new(
            "Input module",
            CheckStatus::Pass,
            format!("MX module found: ID reads {} (expected 0.30 V).", volts(id)),
        ),
        InputModule::Mx => BoardCheck::new(
            "Input module",
            CheckStatus::Warn,
            format!(
                "Read as an MX module, but ID is {} instead of 0.30 V. Check R3 (100 kΩ) \
                 and R4 (10 kΩ) on the module, and connector pin 5 for a solder bridge.",
                volts(id)
            ),
        ),
        InputModule::HallEffect => BoardCheck::new(
            "Input module",
            CheckStatus::Warn,
            format!(
                "Hall Effect module found (ID {}). It needs firmware v2, so the keys are \
                 off. If an MX module is plugged in, its ID line is wrong: check R3/R4 \
                 and connector pins 4 and 5 for a bridge.",
                volts(id)
            ),
        ),
        InputModule::None | InputModule::Unknown => {
            if r.reverse_probe_mv.unwrap_or(0) >= REVERSE_PROBE_MIN_MV {
                BoardCheck::new(
                    "Cable",
                    CheckStatus::Fail,
                    "The module is on a reversed cable (pin 1 ↔ pin 8), so it gets no \
                     power and the keys can't work. Use a same-direction JST-SH cable, \
                     pin 1 to pin 1 (check with a multimeter).",
                )
            } else if loaded >= ID_PRESENT_MIN_MV && id >= ID_TOO_HIGH_MV {
                BoardCheck::new(
                    "Input module",
                    CheckStatus::Fail,
                    format!(
                        "ID reads {}, higher than any module. Connector pin 5 (ID) is \
                         probably shorted to 3V3 (pin 1) or to a key line (pin 4).",
                        volts(id)
                    ),
                )
            } else {
                // Keys already on the carrier's pins: someone means to use the carrier
                let status = if r.on_carrier_pins() {
                    CheckStatus::Fail
                } else {
                    CheckStatus::Skip
                };
                BoardCheck::new(
                    "Input module",
                    status,
                    format!(
                        "No input module answers (ID {}). That's normal for a hand-wired \
                         pad. With the carrier: check the cable is seated at both ends, \
                         and that the carrier sockets P2-1 (3V3), P2-2 (GND) and P1-8 \
                         (ID) are soldered.",
                        volts(id)
                    ),
                )
            }
        }
    }
}

fn key_pins_check(r: &BoardTestReport) -> BoardCheck {
    if r.on_carrier_pins() {
        return BoardCheck::new(
            "Key pins",
            CheckStatus::Pass,
            "Keys are set to GPIO10 and GPIO7, where the carrier routes them.",
        );
    }
    BoardCheck {
        fix: Some(BoardFix::UseCarrierKeyPins),
        ..BoardCheck::new(
            "Key pins",
            CheckStatus::Fail,
            format!(
                "Keys are set to GPIO{} / GPIO{}, but the carrier routes the MX module to \
                 GPIO10 (Key 1) and GPIO7 (Key 2). Set Key 1 = GPIO10 and Key 2 = GPIO7.",
                r.key1_gpio, r.key2_gpio
            ),
        )
    }
}

fn key_line_check(r: &BoardTestReport, key: u8, gpio: u32, mx: bool) -> BoardCheck {
    let name = if key == 1 { "Key 1 line" } else { "Key 2 line" };
    // What sits on that key's line between the pad and the switch
    let (conn_pin, socket, resistor) = if key == 1 {
        (3, "P1-10", "R1")
    } else {
        (4, "P1-9", "R2")
    };
    let carrier_pin = if key == 1 {
        CARRIER_KEY1_GPIO
    } else {
        CARRIER_KEY2_GPIO
    };

    if !r.tested(gpio) {
        return BoardCheck::new(name, CheckStatus::Skip, format!("GPIO{gpio} not tested."));
    }
    if !r.up(gpio) {
        return BoardCheck::new(
            name,
            CheckStatus::Fail,
            format!(
                "GPIO{gpio} reads low: the switch was held during the test, or the line is \
                 shorted to GND (a solder bridge at the hot-swap socket, connector pin \
                 {conn_pin}, or the carrier socket). Release both keys and run the test \
                 again."
            ),
        );
    }
    let pulled_up = r.down(gpio);
    match (mx, gpio == carrier_pin, pulled_up) {
        (true, true, true) => BoardCheck::new(
            name,
            CheckStatus::Pass,
            format!(
                "The module's pull-up reaches GPIO{gpio}: the cable, connector pin \
                 {conn_pin} and the carrier socket {socket} are connected."
            ),
        ),
        (true, true, false) => BoardCheck::new(
            name,
            CheckStatus::Fail,
            format!(
                "The MX module's 10 kΩ pull-up does not reach GPIO{gpio}: Key {key}'s line \
                 is open. Check the cable at connector pin {conn_pin}, the carrier socket \
                 {socket}, and {resistor} on the module."
            ),
        ),
        (true, false, _) => BoardCheck::new(
            name,
            CheckStatus::Skip,
            format!("GPIO{gpio} is not a carrier key pin; fix the key pins first."),
        ),
        (false, _, true) => BoardCheck::new(
            name,
            CheckStatus::Pass,
            format!("GPIO{gpio} idles high, with an external pull-up."),
        ),
        (false, _, false) => BoardCheck::new(
            name,
            CheckStatus::Pass,
            format!(
                "GPIO{gpio} idles high on the pad's pull-up (no external pull-up, normal \
                 for a hand-wired switch)."
            ),
        ),
    }
}

fn spare_lines_check(r: &BoardTestReport) -> Option<BoardCheck> {
    let lines: Vec<(u32, u32)> = SPARE_LINES
        .iter()
        .copied()
        .filter(|&(gpio, _)| r.tested(gpio))
        .collect();
    if lines.is_empty() {
        return None;
    }

    let mut issues = Vec::new();
    for &(gpio, pin) in &lines {
        if !r.up(gpio) {
            let neighbour = if pin == 6 { " or to ID (pin 5)" } else { "" };
            issues.push(format!(
                "connector pin {pin} (GPIO{gpio}) is shorted to GND{neighbour}"
            ));
        } else if r.down(gpio) {
            issues.push(format!(
                "connector pin {pin} (GPIO{gpio}) is shorted to 3V3"
            ));
        }
    }
    for pair in lines.windows(2) {
        let ((ga, pa), (gb, pb)) = (pair[0], pair[1]);
        if r.bridged(ga) && r.bridged(gb) {
            issues.push(format!(
                "connector pins {pa} and {pb} (GPIO{ga} / GPIO{gb}) are bridged"
            ));
        }
    }

    let pins: Vec<String> = lines.iter().map(|(_, p)| p.to_string()).collect();
    Some(if issues.is_empty() {
        BoardCheck::new(
            "Spare lines",
            CheckStatus::Pass,
            format!("Connector pins {} are free of shorts.", pins.join(", ")),
        )
    } else {
        BoardCheck::new(
            "Spare lines",
            CheckStatus::Warn,
            format!(
                "{}. The MX module doesn't use these pins, but a solder bridge here \
                 usually has company: inspect J_MOD on the carrier and J1 on the module.",
                capitalise(&issues.join("; "))
            ),
        )
    })
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPARES: u64 = (1 << 6) | (1 << 4) | (1 << 2);
    const KEYS: u64 = (1 << 10) | (1 << 7);

    /// A healthy MX module on the carrier
    fn healthy_mx() -> BoardTestReport {
        BoardTestReport {
            ran: true,
            message: String::new(),
            boot_module: InputModule::Mx,
            module: InputModule::Mx,
            id_mv: Some(300),
            id_loaded_mv: Some(250),
            reverse_probe_mv: None,
            key1_gpio: 10,
            key2_gpio: 7,
            keys_enabled: true,
            tested_gpios: KEYS | SPARES,
            high_with_pullup: KEYS | SPARES,
            high_with_pulldown: KEYS,
            bridged_gpios: 0,
        }
    }

    fn find<'a>(checks: &'a [BoardCheck], name: &str) -> &'a BoardCheck {
        checks
            .iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("no {name} check in {checks:#?}"))
    }

    #[test]
    fn healthy_mx_passes_everything() {
        let checks = evaluate(&healthy_mx());
        assert_eq!(overall(&checks), CheckStatus::Pass, "{checks:#?}");
        assert_eq!(checks.len(), 5); // module, key pins, two key lines, spares
    }

    #[test]
    fn refused_test_is_one_skip() {
        let r = BoardTestReport {
            ran: false,
            message: "not while a map is played".into(),
            ..Default::default()
        };
        let checks = evaluate(&r);
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].status, CheckStatus::Skip);
        assert!(checks[0].detail.contains("map"));
    }

    #[test]
    fn reversed_cable_is_named() {
        let r = BoardTestReport {
            boot_module: InputModule::None,
            module: InputModule::None,
            id_mv: Some(40),
            id_loaded_mv: Some(0),
            reverse_probe_mv: Some(1500),
            tested_gpios: KEYS,
            high_with_pullup: KEYS,
            high_with_pulldown: 0,
            ..healthy_mx()
        };
        let checks = evaluate(&r);
        let cable = find(&checks, "Cable");
        assert_eq!(cable.status, CheckStatus::Fail);
        assert!(cable.detail.contains("reversed"));
    }

    #[test]
    fn hand_wired_pad_without_module_is_fine() {
        let r = BoardTestReport {
            boot_module: InputModule::None,
            module: InputModule::None,
            id_mv: Some(20),
            id_loaded_mv: Some(0),
            reverse_probe_mv: Some(5),
            key1_gpio: 14,
            key2_gpio: 9,
            tested_gpios: (1 << 14) | (1 << 9),
            high_with_pullup: (1 << 14) | (1 << 9),
            high_with_pulldown: 0,
            ..healthy_mx()
        };
        let checks = evaluate(&r);
        assert_eq!(overall(&checks), CheckStatus::Pass, "{checks:#?}");
        assert_eq!(find(&checks, "Input module").status, CheckStatus::Skip);
    }

    #[test]
    fn missing_module_on_carrier_pins_fails() {
        let r = BoardTestReport {
            boot_module: InputModule::None,
            module: InputModule::None,
            id_mv: Some(20),
            id_loaded_mv: Some(0),
            reverse_probe_mv: Some(5),
            tested_gpios: KEYS,
            high_with_pulldown: 0,
            ..healthy_mx()
        };
        let checks = evaluate(&r);
        let m = find(&checks, "Input module");
        assert_eq!(m.status, CheckStatus::Fail);
        assert!(m.detail.contains("P2-1"));
    }

    #[test]
    fn wrong_key_pins_offer_the_fix() {
        let r = BoardTestReport {
            key1_gpio: 14,
            key2_gpio: 9,
            tested_gpios: (1 << 14) | (1 << 9) | SPARES,
            high_with_pullup: (1 << 14) | (1 << 9) | SPARES,
            high_with_pulldown: 0,
            ..healthy_mx()
        };
        let checks = evaluate(&r);
        let pins = find(&checks, "Key pins");
        assert_eq!(pins.status, CheckStatus::Fail);
        assert_eq!(pins.fix, Some(BoardFix::UseCarrierKeyPins));
        // The key lines say nothing about pins the module isn't on
        assert_eq!(find(&checks, "Key 1 line").status, CheckStatus::Skip);
    }

    #[test]
    fn open_key_line_points_at_its_parts() {
        let r = BoardTestReport {
            high_with_pulldown: 1 << 10, // key 2's pull-up is missing
            ..healthy_mx()
        };
        let checks = evaluate(&r);
        assert_eq!(find(&checks, "Key 1 line").status, CheckStatus::Pass);
        let k2 = find(&checks, "Key 2 line");
        assert_eq!(k2.status, CheckStatus::Fail);
        assert!(
            k2.detail.contains("P1-9") && k2.detail.contains("R2"),
            "{}",
            k2.detail
        );
    }

    #[test]
    fn low_key_line_is_held_or_shorted() {
        let r = BoardTestReport {
            high_with_pullup: (1 << 7) | SPARES,
            high_with_pulldown: 1 << 7,
            ..healthy_mx()
        };
        let k1 = evaluate(&r)
            .into_iter()
            .find(|c| c.name == "Key 1 line")
            .unwrap();
        assert_eq!(k1.status, CheckStatus::Fail);
        assert!(k1.detail.contains("held"));
    }

    #[test]
    fn spare_shorts_and_bridges_are_listed() {
        let r = BoardTestReport {
            high_with_pullup: KEYS | (1 << 4) | (1 << 2), // pin 6 low with pull-up
            bridged_gpios: (1 << 4) | (1 << 2),
            ..healthy_mx()
        };
        let checks = evaluate(&r);
        let s = find(&checks, "Spare lines");
        assert_eq!(s.status, CheckStatus::Warn);
        assert!(s.detail.contains("pin 6"), "{}", s.detail);
        assert!(s.detail.contains("ID (pin 5)"), "{}", s.detail);
        assert!(s.detail.contains("pins 7 and 8"), "{}", s.detail);
    }

    #[test]
    fn hall_effect_skips_key_lines() {
        let r = BoardTestReport {
            boot_module: InputModule::HallEffect,
            module: InputModule::HallEffect,
            id_mv: Some(1060),
            id_loaded_mv: Some(600),
            keys_enabled: false,
            tested_gpios: SPARES,
            high_with_pullup: SPARES,
            high_with_pulldown: 0,
            ..healthy_mx()
        };
        let checks = evaluate(&r);
        assert_eq!(find(&checks, "Input module").status, CheckStatus::Warn);
        assert_eq!(find(&checks, "Key lines").status, CheckStatus::Skip);
        assert!(checks.iter().all(|c| c.name != "Key pins"));
    }

    #[test]
    fn module_change_since_boot_warns() {
        let r = BoardTestReport {
            boot_module: InputModule::None,
            ..healthy_mx()
        };
        assert_eq!(find(&evaluate(&r), "Since boot").status, CheckStatus::Warn);
    }

    #[test]
    fn unreadable_id_fails() {
        let r = BoardTestReport {
            id_mv: None,
            ..healthy_mx()
        };
        assert_eq!(
            find(&evaluate(&r), "Input module").status,
            CheckStatus::Fail
        );
    }
}
