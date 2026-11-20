//! Board test: what the pad measures on the carrier and input module PCBs
//! (`BoardTestResult` in `protocol/osupad.proto`), and the pass/fail checks
//! the CLI and GUI show for it.
//!
//! The pad only measures; every judgement is made here so it can change
//! without a firmware update. Pinout and part names: `hardware/pcb/V1/README.md`,
//! and for V3 `hardware/pcb/V3/README.md` and `hardware/verify/golden/he_v3.toml`
//! (branch `hw/redesign`). The ADS7028 notes: `docs/hardware/components/ads7028.md`.

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
/// Above the highest module class (MX V3, 2.24 V): ID is shorted to 3V3
const ID_TOO_HIGH_MV: u32 = 2600;
/// MX divider: 3.3 V × 10k / 110k = 0.30 V
const MX_ID_RANGE_MV: (u32, u32) = (200, 450);
/// HE V3 divider: 3.3 V × 100k / 200k = 1.65 V
const HE_V3_ID_RANGE_MV: (u32, u32) = (1500, 1800);
/// MX V3 divider: 3.3 V × 100k / 147k = 2.24 V
const MX_V3_ID_RANGE_MV: (u32, u32) = (2050, 2450);

/// Input module on the carrier's connector, from its ID voltage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputModule {
    #[default]
    Unknown,
    None,
    Mx,
    HallEffect,
    /// Three Hall Effect keys read by an ADS7028 on the module, over SPI
    HallEffectV3,
    /// Three MX keys on the V3 FFC
    MxV3,
}

impl InputModule {
    pub fn label(self) -> &'static str {
        match self {
            InputModule::Unknown => "unknown",
            InputModule::None => "no module",
            InputModule::Mx => "MX module",
            InputModule::HallEffect => "Hall Effect module",
            InputModule::HallEffectV3 => "Hall Effect V3 module",
            InputModule::MxV3 => "MX V3 module",
        }
    }

    /// On the V3 carrier's 14-way FFC
    pub fn is_v3(self) -> bool {
        matches!(self, InputModule::HallEffectV3 | InputModule::MxV3)
    }
}

/// One SCLK step of the ADC's fixed-pattern test
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeAdcSpeed {
    pub sclk_hz: u32,
    /// 0: the pad's pins cannot run this fast, so it was not tried
    pub frames: u32,
    /// Frames that did not read the pattern
    pub errors: u32,
}

/// One sensor, a burst of 12-bit conversions (AVDD full scale)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeAdcChannel {
    pub ain: u32,
    pub samples: u32,
    pub min: u32,
    pub max: u32,
    pub mean_x16: u32,
    pub noise_x100: u32,
}

/// The ADC's reference: AVDD, the module's 3.3 V LDO (U5)
pub const HE_V3_AVDD_V: f64 = 3.3;
pub const HE_V3_FULL_SCALE: f64 = 4096.0;

impl HeAdcChannel {
    pub fn mean(&self) -> f64 {
        self.mean_x16 as f64 / 16.0
    }
    /// Standard deviation, in codes
    pub fn noise(&self) -> f64 {
        self.noise_x100 as f64 / 100.0
    }
    pub fn volts(&self) -> f64 {
        self.mean() * HE_V3_AVDD_V / HE_V3_FULL_SCALE
    }
}

/// What the pad measured on a Hall Effect V3 module's ADC (`HeAdcTest` on the
/// wire, `firmware/main/diag/he_adc_test.h`)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeAdcReport {
    pub ran: bool,
    pub message: String,
    pub sclk_hz: u32,
    /// SYSTEM_STATUS after a software reset: 0x81. `None` = not read
    pub status_after_reset: Option<u8>,
    /// After clearing BOR: 0x80
    pub status_after_clear: Option<u8>,
    pub walk_writes: u32,
    pub walk_mismatches: u32,
    pub walk_wrong_bits: u8,
    pub cal_done: bool,
    /// When done: BOR set again means AVDD dipped during the test
    pub status_end: Option<u8>,
    pub speeds: Vec<HeAdcSpeed>,
    /// Key 1, 2, 3
    pub channels: Vec<HeAdcChannel>,
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
    /// A V3 module's ID came on GPIO7: its FFC is turned over
    #[serde(default)]
    pub cable_reversed: bool,
    /// The ADC test of a Hall Effect V3 module; `None` with other modules and
    /// from firmware predating it
    #[serde(default)]
    pub he_adc: Option<HeAdcReport>,
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

/// The part of the board a check is about, in the order the app shows them
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckGroup {
    /// Which module, and the cable to it
    Module,
    /// Switch key lines
    Keys,
    /// Connector lines no key uses
    Connector,
    /// A Hall Effect V3 module's ADC and its SPI link
    Adc,
    /// A Hall Effect V3 module's sensors
    Sensors,
}

impl CheckGroup {
    pub fn label(self) -> &'static str {
        match self {
            CheckGroup::Module => "Module & cable",
            CheckGroup::Keys => "Key lines",
            CheckGroup::Connector => "Connector",
            CheckGroup::Adc => "ADC & SPI link",
            CheckGroup::Sensors => "Hall sensors",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BoardCheck {
    pub name: &'static str,
    pub group: CheckGroup,
    pub status: CheckStatus,
    pub detail: String,
    pub fix: Option<BoardFix>,
}

impl BoardCheck {
    fn new(name: &'static str, status: CheckStatus, detail: impl Into<String>) -> Self {
        BoardCheck {
            name,
            group: CheckGroup::Module,
            status,
            detail: detail.into(),
            fix: None,
        }
    }

    fn in_group(mut self, group: CheckGroup) -> Self {
        self.group = group;
        self
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

    if r.module.is_v3() {
        checks.push(v3_cable_check(r));
    }

    // MX V3 on a straight cable has keys 1 and 2 where MX V1 has them
    let is_mx =
        |m: InputModule| m == InputModule::Mx || (m == InputModule::MxV3 && !r.cable_reversed);
    let mx = is_mx(r.module) || is_mx(r.boot_module);
    if mx {
        checks.push(key_pins_check(r).in_group(CheckGroup::Keys));
    }

    if !r.keys_enabled {
        let why = match r.module {
            InputModule::HallEffectV3 => {
                "Not tested: the module's lines are its ADC's SPI bus, tested below."
            }
            InputModule::MxV3 => {
                "Not tested: this firmware reads no keys from an MX V3 module on a turned-over \
                 cable."
            }
            _ => "Not tested: key input is off because a Hall Effect module is connected.",
        };
        checks
            .push(BoardCheck::new("Key lines", CheckStatus::Skip, why).in_group(CheckGroup::Keys));
    } else {
        checks.push(key_line_check(r, 1, r.key1_gpio, mx).in_group(CheckGroup::Keys));
        checks.push(key_line_check(r, 2, r.key2_gpio, mx).in_group(CheckGroup::Keys));
    }

    if let Some(c) = spare_lines_check(r) {
        checks.push(c.in_group(CheckGroup::Connector));
    }

    if let Some(adc) = &r.he_adc {
        checks.extend(he_adc_checks(adc));
    }
    checks
}

fn v3_cable_check(r: &BoardTestReport) -> BoardCheck {
    if r.cable_reversed {
        BoardCheck::new(
            "Cable",
            CheckStatus::Pass,
            "The module's FFC is turned over (contact 1 meets contact 14): its ID came on \
             GPIO7. That is wired to work: the pad uses the swapped pins.",
        )
    } else {
        BoardCheck::new(
            "Cable",
            CheckStatus::Pass,
            "The module's ID came on GPIO8: the FFC runs contact 1 to contact 1.",
        )
    }
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
        InputModule::HallEffectV3 | InputModule::MxV3 => v3_module_check(r.module, id),
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
            } else if let Some(v3) = v3_class_from_id(id).filter(|_| loaded >= ID_PRESENT_MIN_MV) {
                BoardCheck::new(
                    "Input module",
                    CheckStatus::Warn,
                    format!(
                        "ID reads {}: that is the {}, which the pad's firmware predates. \
                         Update the firmware to use and test it.",
                        volts(id),
                        v3.label()
                    ),
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

/// A V3 module named by its ID voltage alone (firmware that predates V3 sees none)
fn v3_class_from_id(id: u32) -> Option<InputModule> {
    if (HE_V3_ID_RANGE_MV.0..=HE_V3_ID_RANGE_MV.1).contains(&id) {
        Some(InputModule::HallEffectV3)
    } else if (MX_V3_ID_RANGE_MV.0..=MX_V3_ID_RANGE_MV.1).contains(&id) {
        Some(InputModule::MxV3)
    } else {
        None
    }
}

fn v3_module_check(module: InputModule, id: u32) -> BoardCheck {
    let (range, nominal, parts) = if module == InputModule::HallEffectV3 {
        (HE_V3_ID_RANGE_MV, "1.65 V", "R1 and R2 (100 kΩ each)")
    } else {
        (
            MX_V3_ID_RANGE_MV,
            "2.24 V",
            "the ID divider (47 kΩ / 100 kΩ)",
        )
    };
    if (range.0..=range.1).contains(&id) {
        BoardCheck::new(
            "Input module",
            CheckStatus::Pass,
            format!(
                "{} found: ID reads {} (expected {nominal}).",
                capitalise(module.label()),
                volts(id)
            ),
        )
    } else {
        BoardCheck::new(
            "Input module",
            CheckStatus::Warn,
            format!(
                "Read as the {}, but ID is {} instead of {nominal}. Check {parts} on the \
                 module and FFC contact 4 for a solder bridge.",
                module.label(),
                volts(id)
            ),
        )
    }
}

/// SYSTEM_STATUS bits (ADS7028 datasheet SBAS978A 8.5.1)
const STATUS_RSVD: u8 = 0x80;
const STATUS_CRC_ERR_FUSE: u8 = 0x04;
const STATUS_CRC_ERR_IN: u8 = 0x02;
const STATUS_BOR: u8 = 0x01;

/// What each key's sensor path is made of on the HE V3 module (he_v3.toml)
const HE_V3_KEY_PARTS: [(&str, &str, &str); 3] =
    [("U1", "R5", "C9"), ("U2", "R6", "C10"), ("U3", "R7", "C11")];

/// The SPI lines on the module's FFC contacts
const SPI_LINES: &str = "CS (contact 10), SCLK (7), MOSI (8) and MISO (5, through R3)";

fn he_adc_checks(a: &HeAdcReport) -> Vec<BoardCheck> {
    let adc = |name, status, detail: String| {
        BoardCheck::new(name, status, detail).in_group(CheckGroup::Adc)
    };
    let mut checks = Vec::new();

    if !a.ran {
        let status = if a.message.contains("key input") {
            CheckStatus::Warn
        } else {
            CheckStatus::Fail
        };
        checks.push(adc(
            "ADC test",
            status,
            format!(
                "The ADC test did not run: {}. The pad picks its input at boot: restart it \
                 with the module plugged in.",
                if a.message.is_empty() {
                    "no reason given"
                } else {
                    &a.message
                }
            ),
        ));
        return checks;
    }

    // 1. Does it answer at all?
    let answers = match a.status_after_reset {
        None => {
            checks.push(adc(
                "ADC answers",
                CheckStatus::Fail,
                format!(
                    "The pad's SPI failed before the ADC could answer: {}.",
                    a.message
                ),
            ));
            false
        }
        Some(0x00) => {
            checks.push(adc(
                "ADC answers",
                CheckStatus::Fail,
                format!(
                    "MISO reads all zeros: the ADS7028 (U4) does not answer. Check the \
                     module's 3V3 (DVDD, contacts 3 and 12), the {SPI_LINES} and U4's \
                     soldering, especially its exposed pad."
                ),
            ));
            false
        }
        Some(0xFF) => {
            checks.push(adc(
                "ADC answers",
                CheckStatus::Fail,
                format!(
                    "MISO reads all ones: nothing drives it. U4 may be unpowered or not \
                     selected: check DVDD (3V3), CS and its 100 kΩ pull-up R4, and the {SPI_LINES}."
                ),
            ));
            false
        }
        Some(s) if s & STATUS_CRC_ERR_FUSE != 0 => {
            checks.push(adc(
                "ADC answers",
                CheckStatus::Fail,
                format!(
                    "The ADS7028 answers (status 0x{s:02X}) but did not load its factory \
                     configuration (CRC_ERR_FUSE). Power-cycle the pad; if it stays, U4 is faulty."
                ),
            ));
            false
        }
        Some(0x81) => {
            checks.push(adc(
                "ADC answers",
                CheckStatus::Pass,
                "The ADS7028 answers over SPI and resets on command (status 0x81).".to_string(),
            ));
            true
        }
        Some(s) => {
            checks.push(adc(
                "ADC answers",
                CheckStatus::Fail,
                format!(
                    "The ADC's status reads 0x{s:02X} after a reset, not 0x81: bits arrive \
                     shifted or corrupted. A slow or loaded SCLK or MISO line does this: check \
                     the {SPI_LINES} for a bad joint or a bridge to a neighbour."
                ),
            ));
            s & STATUS_RSVD != 0
        }
    };
    if !answers {
        return checks;
    }

    // 2. Do writes land and read back?
    let cleared = a.status_after_clear == Some(0x80);
    if a.walk_writes > 0 && a.walk_mismatches == 0 && cleared {
        checks.push(adc(
            "Register writes",
            CheckStatus::Pass,
            format!(
                "{} register writes read back exactly: MOSI, MISO and SCLK carry every bit \
                 both ways.",
                a.walk_writes
            ),
        ));
    } else if a.walk_mismatches == a.walk_writes && a.walk_writes > 0 {
        checks.push(adc(
            "Register writes",
            CheckStatus::Fail,
            "The ADC answers, but no register write takes: MOSI does not reach it. Check \
             FFC contact 8 (MOSI) and its carrier socket P1-2 (GPIO4)."
                .to_string(),
        ));
    } else {
        checks.push(adc(
            "Register writes",
            CheckStatus::Fail,
            format!(
                "{} of {} register writes read back wrong (bits 0x{:02X} affected){}. A \
                 marginal MOSI or MISO line: look for a cold joint at U4 or the FFC connectors.",
                a.walk_mismatches,
                a.walk_writes,
                a.walk_wrong_bits,
                if cleared {
                    ""
                } else {
                    ", and BOR would not clear"
                }
            ),
        ));
    }

    // 3. Offset calibration
    checks.push(if a.cal_done {
        adc(
            "Offset calibration",
            CheckStatus::Pass,
            "The ADC calibrated its offset.".to_string(),
        )
    } else {
        adc(
            "Offset calibration",
            CheckStatus::Warn,
            "The ADC's offset calibration did not finish within 10 ms.".to_string(),
        )
    });

    // 4. Signal margin: the fixed pattern at rising SCLK
    checks.push(speed_check(a));

    // 5. Did the supply hold?
    checks.push(match a.status_end {
        None => adc(
            "Supply",
            CheckStatus::Warn,
            "The ADC's status could not be read at the end.".to_string(),
        ),
        Some(s) if s & STATUS_BOR != 0 => adc(
            "Supply",
            CheckStatus::Fail,
            format!(
                "The ADC reset itself during the test (BOR, status 0x{s:02X}): its supply \
                 dipped. Check the LDO U5, its 5 V (contacts 2 and 13, ferrite FB1) and C1-C3."
            ),
        ),
        Some(s) if s & STATUS_CRC_ERR_IN != 0 => adc(
            "Supply",
            CheckStatus::Warn,
            format!(
                "The ADC flagged a corrupted frame (CRC_ERR_IN, status 0x{s:02X}) although \
                 the pad does not use CRC yet: noise on the SPI lines."
            ),
        ),
        Some(_) => adc(
            "Supply",
            CheckStatus::Pass,
            "The ADC stayed powered through the test (no brown-out).".to_string(),
        ),
    });

    checks.extend(sensor_checks(a));
    checks
}

fn mhz(hz: u32) -> String {
    let m = hz as f64 / 1e6;
    if m.fract() == 0.0 {
        format!("{m:.0} MHz")
    } else {
        format!("{m:.1} MHz")
    }
}

fn speed_check(a: &HeAdcReport) -> BoardCheck {
    let tried: Vec<&HeAdcSpeed> = a.speeds.iter().filter(|s| s.frames > 0).collect();
    let table = tried
        .iter()
        .map(|s| format!("{} {}/{}", mhz(s.sclk_hz), s.errors, s.frames))
        .collect::<Vec<_>>()
        .join(", ");
    let first_bad = tried.iter().find(|s| s.errors > 0);
    let best_clean = tried
        .iter()
        .take_while(|s| s.errors == 0)
        .last()
        .map(|s| s.sclk_hz);
    let (status, verdict) = match (first_bad, best_clean) {
        (None, Some(best)) => (
            CheckStatus::Pass,
            format!("Every pattern read clean up to {}.", mhz(best)),
        ),
        (None, None) => (CheckStatus::Skip, "Not measured.".to_string()),
        (Some(bad), _) if bad.sclk_hz <= a.sclk_hz.max(4_000_000) => (
            CheckStatus::Fail,
            format!(
                "Patterns read wrong at {}, a speed the pad relies on: the SPI link is \
                 faulty, not just slow. Check the {SPI_LINES}.",
                mhz(bad.sclk_hz)
            ),
        ),
        (Some(bad), Some(best)) if bad.sclk_hz <= 10_000_000 => (
            CheckStatus::Warn,
            format!(
                "Clean up to {}, but errors from {}: little margin. Reseat both ends of the \
                 FFC and check its contacts.",
                mhz(best),
                mhz(bad.sclk_hz)
            ),
        ),
        (Some(bad), Some(best)) => (
            CheckStatus::Pass,
            format!(
                "Clean up to {}; errors from {} are the long cable's limit, far above what \
                 the pad uses.",
                mhz(best),
                mhz(bad.sclk_hz)
            ),
        ),
        (Some(bad), None) => (
            CheckStatus::Fail,
            format!("Patterns read wrong from {}.", mhz(bad.sclk_hz)),
        ),
    };
    BoardCheck::new(
        "Signal margin",
        status,
        format!("{verdict} Fixed pattern 0xA5A, errors per frames: {table}."),
    )
    .in_group(CheckGroup::Adc)
}

/// A DRV5055 rests at half its supply with no magnet near: 2048 of 4096
const SENSOR_REST_RANGE: (f64, f64) = (1700.0, 2400.0);
/// Near the rails: no output, or shorted
const SENSOR_RAIL_MARGIN: f64 = 100.0;
/// Standard deviation in codes (1 code = 0.8 mV)
const SENSOR_NOISE_PASS: f64 = 3.0;
const SENSOR_NOISE_WARN: f64 = 8.0;

fn sensor_checks(a: &HeAdcReport) -> Vec<BoardCheck> {
    const NAMES: [&str; 3] = ["Key 1 sensor", "Key 2 sensor", "Key 3 sensor"];
    let sensor = |i: usize, status, detail: String| {
        BoardCheck::new(NAMES[i], status, detail).in_group(CheckGroup::Sensors)
    };
    if a.channels.is_empty() {
        return vec![
            BoardCheck::new("Sensors", CheckStatus::Skip, "The sensors were not read.")
                .in_group(CheckGroup::Sensors),
        ];
    }
    if a.channels.len() == 3 && a.channels.iter().all(|c| c.mean() < SENSOR_RAIL_MARGIN) {
        return vec![BoardCheck::new(
            "Sensor supply",
            CheckStatus::Fail,
            "All three sensors read 0 V: their supply is off. Check the LDO U5 (AVDD) \
             and the 5 V reaching it: FFC contacts 2 and 13, ferrite FB1.",
        )
        .in_group(CheckGroup::Sensors)];
    }

    let mut checks = Vec::new();
    for (i, c) in a.channels.iter().enumerate().take(3) {
        let (u, r, cap) = HE_V3_KEY_PARTS[i];
        let m = c.mean();
        let v = c.volts();
        let noise = c.noise();
        let pp = c.max.saturating_sub(c.min);
        let stats = format!(
            "{v:.3} V ({m:.0} codes, AIN{}), noise {noise:.1} codes rms, {pp} peak-to-peak.",
            c.ain
        );
        let (status, verdict) = if m < SENSOR_RAIL_MARGIN {
            (
                CheckStatus::Fail,
                format!(
                    "Reads 0 V: no output from {u}, or AIN{} shorted to GND. Check {u}, {r} \
                     (220 Ω) and {cap} (10 nF).",
                    c.ain
                ),
            )
        } else if m > HE_V3_FULL_SCALE - SENSOR_RAIL_MARGIN {
            (
                CheckStatus::Fail,
                format!(
                    "Reads full scale: AIN{} is shorted to AVDD, or {u}'s output is. Check \
                     {u} and {r}.",
                    c.ain
                ),
            )
        } else if noise > SENSOR_NOISE_WARN {
            (
                CheckStatus::Fail,
                format!("Far too noisy for a still sensor: check {u}'s 100 nF and {cap}."),
            )
        } else if !(SENSOR_REST_RANGE.0..=SENSOR_REST_RANGE.1).contains(&m) {
            (
                CheckStatus::Warn,
                "Rests away from half supply (1.65 V). Normal with a key fitted, whose \
                 magnet shifts it; otherwise a magnet or steel is near the sensor."
                    .to_string(),
            )
        } else if noise > SENSOR_NOISE_PASS {
            (
                CheckStatus::Warn,
                "Rests where it should, but noisier than expected.".to_string(),
            )
        } else {
            (
                CheckStatus::Pass,
                "Rests at half supply, quiet.".to_string(),
            )
        };
        checks.push(sensor(i, status, format!("{verdict} {stats}")));
    }
    checks
}

/// The interactive half of the sensor test: the person moves a magnet over
/// each key in turn, the app runs the board test over and over and feeds
/// every report here. A sensor passes once its reading moves well away from
/// where it rested; another sensor following it closely means their ADC
/// inputs are bridged.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MagnetTest {
    baseline: Option<Vec<f64>>,
    /// Largest movement seen per sensor, in codes
    peak: Vec<f64>,
    /// Largest share another sensor moved while this one moved most
    crosstalk: Vec<f64>,
    pub reports: u32,
}

/// Movement that proves a sensor sees a magnet: ~0.2 V
pub const MAGNET_MOVE_CODES: f64 = 250.0;
/// Another sensor moving this share as much: bridged inputs, or sensors that
/// see each other's magnet
const MAGNET_CROSSTALK_SHARE: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MagnetKey {
    Waiting { moved: f64 },
    Seen { moved: f64 },
    Crosstalk { moved: f64, share: f64 },
}

impl MagnetTest {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one board test report. Reports without sensor readings are ignored.
    pub fn update(&mut self, report: &BoardTestReport) {
        let Some(adc) = report
            .he_adc
            .as_ref()
            .filter(|a| a.ran && !a.channels.is_empty())
        else {
            return;
        };
        let now: Vec<f64> = adc.channels.iter().map(HeAdcChannel::mean).collect();
        self.reports += 1;
        let Some(base) = &self.baseline else {
            self.peak = vec![0.0; now.len()];
            self.crosstalk = vec![0.0; now.len()];
            self.baseline = Some(now);
            return;
        };
        let delta: Vec<f64> = now.iter().zip(base).map(|(n, b)| (n - b).abs()).collect();
        let Some((lead, &lead_delta)) = delta.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))
        else {
            return;
        };
        for (i, d) in delta.iter().enumerate() {
            if i < self.peak.len() && *d > self.peak[i] {
                self.peak[i] = *d;
            }
        }
        if lead_delta >= MAGNET_MOVE_CODES {
            let second = delta
                .iter()
                .enumerate()
                .filter(|&(i, _)| i != lead)
                .map(|(_, d)| *d)
                .fold(0.0, f64::max);
            let share = second / lead_delta;
            if share > self.crosstalk[lead] {
                self.crosstalk[lead] = share;
            }
        }
    }

    /// Readings at the start, in codes, once the first report came
    pub fn baseline(&self) -> Option<&[f64]> {
        self.baseline.as_deref()
    }

    pub fn key(&self, i: usize) -> MagnetKey {
        let moved = self.peak.get(i).copied().unwrap_or(0.0);
        let share = self.crosstalk.get(i).copied().unwrap_or(0.0);
        if moved < MAGNET_MOVE_CODES {
            MagnetKey::Waiting { moved }
        } else if share >= MAGNET_CROSSTALK_SHARE {
            MagnetKey::Crosstalk { moved, share }
        } else {
            MagnetKey::Seen { moved }
        }
    }

    pub fn keys(&self) -> usize {
        self.peak.len()
    }

    /// Every sensor has seen the magnet
    pub fn done(&self) -> bool {
        self.keys() > 0
            && (0..self.keys()).all(|i| !matches!(self.key(i), MagnetKey::Waiting { .. }))
    }

    /// The checks to show for it so far
    pub fn checks(&self) -> Vec<BoardCheck> {
        const NAMES: [&str; 3] = ["Key 1 magnet", "Key 2 magnet", "Key 3 magnet"];
        (0..self.keys().min(3))
            .map(|i| {
                let (status, detail) = match self.key(i) {
                    MagnetKey::Waiting { moved } => (
                        CheckStatus::Skip,
                        format!(
                            "Not seen yet: moved {moved:.0} codes, needs {MAGNET_MOVE_CODES:.0}."
                        ),
                    ),
                    MagnetKey::Seen { moved } => (
                        CheckStatus::Pass,
                        format!("Sees the magnet ({moved:.0} codes), on its own."),
                    ),
                    MagnetKey::Crosstalk { moved, share } => (
                        CheckStatus::Warn,
                        format!(
                            "Sees the magnet ({moved:.0} codes), but another sensor moved {:.0} % \
                             as much. Hold the magnet over this key only; if it stays, look for \
                             a bridge between U4's AIN pins.",
                            share * 100.0
                        ),
                    ),
                };
                BoardCheck::new(NAMES[i], status, detail).in_group(CheckGroup::Sensors)
            })
            .collect()
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
            cable_reversed: false,
            he_adc: None,
        }
    }

    /// The HE V3 module as measured on the bench (2026-11-20), healthy
    fn healthy_he_v3() -> BoardTestReport {
        let speed = |mhz: u32| HeAdcSpeed {
            sclk_hz: mhz * 1_000_000,
            frames: 500,
            errors: 0,
        };
        let channel = |ain: u32, mean: u32| HeAdcChannel {
            ain,
            samples: 256,
            min: mean - 3,
            max: mean + 3,
            mean_x16: mean * 16,
            noise_x100: 120,
        };
        BoardTestReport {
            ran: true,
            boot_module: InputModule::HallEffectV3,
            module: InputModule::HallEffectV3,
            id_mv: Some(1655),
            id_loaded_mv: Some(440),
            reverse_probe_mv: None,
            key1_gpio: 10,
            key2_gpio: 7,
            keys_enabled: false,
            he_adc: Some(HeAdcReport {
                ran: true,
                sclk_hz: 4_000_000,
                status_after_reset: Some(0x81),
                status_after_clear: Some(0x80),
                walk_writes: 20,
                cal_done: true,
                status_end: Some(0x80),
                speeds: vec![speed(1), speed(4), speed(10), speed(20)],
                channels: vec![channel(5, 2050), channel(4, 2040), channel(3, 2061)],
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn adc(r: &mut BoardTestReport) -> &mut HeAdcReport {
        r.he_adc.as_mut().unwrap()
    }

    #[test]
    fn healthy_he_v3_passes() {
        let checks = evaluate(&healthy_he_v3());
        assert_eq!(overall(&checks), CheckStatus::Pass, "{checks:#?}");
        assert!(checks.iter().any(|c| c.group == CheckGroup::Adc));
        assert_eq!(
            checks
                .iter()
                .filter(|c| c.group == CheckGroup::Sensors)
                .count(),
            3
        );
        assert!(checks.iter().all(|c| c.name != "Key pins"));
    }

    #[test]
    fn he_v3_reversed_cable_is_fine_and_said() {
        let r = BoardTestReport {
            cable_reversed: true,
            ..healthy_he_v3()
        };
        let cable = find(&evaluate(&r), "Cable").clone();
        assert_eq!(cable.status, CheckStatus::Pass);
        assert!(cable.detail.contains("turned over"));
    }

    #[test]
    fn silent_adc_names_miso() {
        let mut r = healthy_he_v3();
        adc(&mut r).status_after_reset = Some(0x00);
        let checks = evaluate(&r);
        let c = find(&checks, "ADC answers");
        assert_eq!(c.status, CheckStatus::Fail);
        assert!(c.detail.contains("MISO"));
        // Nothing after a silent ADC is judged
        assert!(checks.iter().all(|c| c.group != CheckGroup::Sensors));
    }

    #[test]
    fn writes_that_never_land_blame_mosi() {
        let mut r = healthy_he_v3();
        let a = adc(&mut r);
        a.walk_mismatches = 20;
        a.walk_wrong_bits = 0xFF;
        a.status_after_clear = Some(0x81);
        let c = find(&evaluate(&r), "Register writes").clone();
        assert_eq!(c.status, CheckStatus::Fail);
        assert!(c.detail.contains("MOSI"));
    }

    #[test]
    fn errors_at_10_mhz_warn() {
        let mut r = healthy_he_v3();
        adc(&mut r).speeds[2].errors = 7;
        let c = find(&evaluate(&r), "Signal margin").clone();
        assert_eq!(c.status, CheckStatus::Warn, "{}", c.detail);
        assert!(c.detail.contains("4 MHz"));
    }

    #[test]
    fn errors_at_working_speed_fail() {
        let mut r = healthy_he_v3();
        adc(&mut r).speeds[1].errors = 1;
        assert_eq!(
            find(&evaluate(&r), "Signal margin").status,
            CheckStatus::Fail
        );
    }

    #[test]
    fn brown_out_during_test_fails_supply() {
        let mut r = healthy_he_v3();
        adc(&mut r).status_end = Some(0x81);
        let c = find(&evaluate(&r), "Supply").clone();
        assert_eq!(c.status, CheckStatus::Fail);
        assert!(c.detail.contains("U5"));
    }

    #[test]
    fn dead_and_shorted_sensors_name_their_parts() {
        let mut r = healthy_he_v3();
        let a = adc(&mut r);
        a.channels[1].mean_x16 = 3 * 16;
        a.channels[2].mean_x16 = 4090 * 16;
        let checks = evaluate(&r);
        let k2 = find(&checks, "Key 2 sensor");
        assert_eq!(k2.status, CheckStatus::Fail);
        assert!(
            k2.detail.contains("U2") && k2.detail.contains("R6"),
            "{}",
            k2.detail
        );
        assert_eq!(find(&checks, "Key 3 sensor").status, CheckStatus::Fail);
        assert_eq!(find(&checks, "Key 1 sensor").status, CheckStatus::Pass);
    }

    #[test]
    fn all_sensors_at_zero_is_the_ldo() {
        let mut r = healthy_he_v3();
        for c in &mut adc(&mut r).channels {
            c.mean_x16 = 16;
        }
        let checks = evaluate(&r);
        let c = find(&checks, "Sensor supply");
        assert_eq!(c.status, CheckStatus::Fail);
        assert!(c.detail.contains("U5"));
    }

    #[test]
    fn fitted_key_shifts_rest_and_warns() {
        let mut r = healthy_he_v3();
        adc(&mut r).channels[0].mean_x16 = 2900 * 16;
        assert_eq!(
            find(&evaluate(&r), "Key 1 sensor").status,
            CheckStatus::Warn
        );
    }

    #[test]
    fn adc_test_refused_with_keys_on() {
        let mut r = healthy_he_v3();
        *adc(&mut r) = HeAdcReport {
            message: "key input is on: restart the pad".into(),
            ..Default::default()
        };
        let checks = evaluate(&r);
        let c = find(&checks, "ADC test");
        assert_eq!(c.status, CheckStatus::Warn);
    }

    #[test]
    fn old_firmware_with_he_v3_says_update() {
        let r = BoardTestReport {
            boot_module: InputModule::None,
            module: InputModule::None,
            id_mv: Some(1655),
            id_loaded_mv: Some(440),
            reverse_probe_mv: Some(439),
            keys_enabled: false,
            ..healthy_mx()
        };
        let m = find(&evaluate(&r), "Input module").clone();
        assert_eq!(m.status, CheckStatus::Warn);
        assert!(m.detail.contains("Hall Effect V3") && m.detail.contains("firmware"));
    }

    fn with_means(means: [u32; 3]) -> BoardTestReport {
        let mut r = healthy_he_v3();
        for (c, m) in adc(&mut r).channels.iter_mut().zip(means) {
            c.mean_x16 = m * 16;
        }
        r
    }

    #[test]
    fn magnet_test_sees_each_key_on_its_own() {
        let mut t = MagnetTest::new();
        t.update(&with_means([2048, 2048, 2048]));
        assert!(!t.done());
        t.update(&with_means([2600, 2100, 2048]));
        assert!(matches!(t.key(0), MagnetKey::Seen { .. }));
        assert!(matches!(t.key(1), MagnetKey::Waiting { .. }));
        t.update(&with_means([2048, 1500, 2048]));
        t.update(&with_means([2048, 2048, 2500]));
        assert!(t.done(), "{t:?}");
        assert!(t.checks().iter().all(|c| c.status == CheckStatus::Pass));
    }

    #[test]
    fn magnet_test_flags_bridged_inputs() {
        let mut t = MagnetTest::new();
        t.update(&with_means([2048, 2048, 2048]));
        t.update(&with_means([2600, 2580, 2048]));
        assert!(matches!(t.key(0), MagnetKey::Crosstalk { .. }));
        assert_eq!(t.checks()[0].status, CheckStatus::Warn);
    }

    #[test]
    fn magnet_test_ignores_reports_without_sensors() {
        let mut t = MagnetTest::new();
        t.update(&healthy_mx());
        assert_eq!(t.reports, 0);
        assert_eq!(t.keys(), 0);
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
