//! What the touchscreen swipes can be set to, as the Settings page offers it:
//! the pad's own actions, osu!'s keyboard shortcuts by name, and keyboard keys
//! that osu! leaves free.
//!
//! On the wire every osu! shortcut is just [`SwipeAction::Key`] with a key and
//! modifiers; the name is found again from those when the config is read back.

use crate::{hid_usage_to_char, DeviceConfig, SwipeAction};
use std::fmt;

/// HID keyboard modifier bits, as the pad sends them
pub const MOD_CTRL: u32 = 0x01;
pub const MOD_SHIFT: u32 = 0x02;
pub const MOD_ALT: u32 = 0x04;
pub const MOD_GUI: u32 = 0x08;

/// HID keyboard usages of the keys named below
mod usage {
    pub const ENTER: u32 = 0x28;
    pub const ESC: u32 = 0x29;
    pub const BACKSPACE: u32 = 0x2A;
    pub const F1: u32 = 0x3A;
    pub const F12: u32 = 0x45;
    pub const INSERT: u32 = 0x49;
    pub const HOME: u32 = 0x4A;
    pub const RIGHT: u32 = 0x4F;
    pub const LEFT: u32 = 0x50;
    pub const DOWN: u32 = 0x51;
    pub const UP: u32 = 0x52;
    pub const F13: u32 = 0x68;
    pub const F24: u32 = 0x73;
    pub const B: u32 = 0x05;
    pub const E: u32 = 0x08;
    pub const N: u32 = 0x11;
    pub const O: u32 = 0x12;
    pub const P: u32 = 0x13;
    pub const R: u32 = 0x15;
    pub const S: u32 = 0x16;
    pub const T: u32 = 0x17;

    pub const fn f(n: u32) -> u32 {
        if n <= 12 {
            F1 + n - 1
        } else {
            F13 + n - 13
        }
    }
}

/// A key's name, e.g. "F2", "Esc", "←"
pub fn key_name(usage: u32) -> String {
    match usage {
        usage::F1..=usage::F12 => format!("F{}", usage - usage::F1 + 1),
        usage::F13..=usage::F24 => format!("F{}", usage - usage::F13 + 13),
        usage::ENTER => "Enter".into(),
        usage::ESC => "Esc".into(),
        usage::BACKSPACE => "Backspace".into(),
        0x2B => "Tab".into(),
        0x2C => "Space".into(),
        usage::INSERT => "Insert".into(),
        usage::HOME => "Home".into(),
        0x4B => "Page Up".into(),
        0x4C => "Delete".into(),
        0x4D => "End".into(),
        0x4E => "Page Down".into(),
        usage::RIGHT => "→".into(),
        usage::LEFT => "←".into(),
        usage::DOWN => "↓".into(),
        usage::UP => "↑".into(),
        other => hid_usage_to_char(other),
    }
}

/// "Ctrl+Shift+R"
pub fn combo_name(usage: u32, modifiers: u32) -> String {
    let mut name = String::new();
    for (bit, label) in [
        (MOD_CTRL, "Ctrl+"),
        (MOD_SHIFT, "Shift+"),
        (MOD_ALT, "Alt+"),
        (MOD_GUI, "Win+"),
    ] {
        if modifiers & bit != 0 {
            name.push_str(label);
        }
    }
    name.push_str(&key_name(usage));
    name
}

/// Which osu! a shortcut belongs to
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsuClient {
    Both,
    Lazer,
    /// Windows only: Linux supports osu!lazer alone
    Stable,
}

/// One of osu!'s default keyboard shortcuts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OsuShortcut {
    pub action: &'static str,
    pub usage: u32,
    pub modifiers: u32,
    pub client: OsuClient,
}

impl OsuShortcut {
    /// Offered on this platform: osu! stable runs on Windows only here
    pub fn available(&self) -> bool {
        self.client != OsuClient::Stable || cfg!(windows)
    }
}

impl fmt::Display for OsuShortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let game = match self.client {
            OsuClient::Both => "osu!",
            OsuClient::Lazer => "osu!lazer",
            OsuClient::Stable => "osu! stable",
        };
        write!(
            f,
            "{}: {} ({})",
            game,
            self.action,
            combo_name(self.usage, self.modifiers)
        )
    }
}

const fn shortcut(
    action: &'static str,
    usage: u32,
    modifiers: u32,
    client: OsuClient,
) -> OsuShortcut {
    OsuShortcut {
        action,
        usage,
        modifiers,
        client,
    }
}

/// osu!'s default shortcuts that do something outside a map (swipes never
/// send keyboard keys during one). From osu!lazer's GlobalActionContainer
/// and osu-framework's FrameworkActionContainer, and the osu! wiki's
/// "Shortcut key reference" for stable. Same key in both games, same entry.
pub const OSU_SHORTCUTS: &[OsuShortcut] = {
    use usage::*;
    use OsuClient::*;
    &[
        // Song select
        shortcut("Random beatmap", f(2), 0, Both),
        shortcut("Previous random beatmap", f(2), MOD_SHIFT, Both),
        shortcut("Mod select", f(1), 0, Both),
        shortcut("Beatmap options", f(3), 0, Both),
        shortcut("Previous beatmap", LEFT, 0, Both),
        shortcut("Next beatmap", RIGHT, 0, Both),
        shortcut("Previous difficulty", UP, 0, Both),
        shortcut("Next difficulty", DOWN, 0, Both),
        shortcut("Previous group", LEFT, MOD_SHIFT, Both),
        shortcut("Next group", RIGHT, MOD_SHIFT, Both),
        shortcut("Deselect all mods", BACKSPACE, 0, Lazer),
        // Anywhere
        shortcut("Play / select", ENTER, 0, Both),
        shortcut("Back", ESC, 0, Both),
        shortcut("Chat", f(8), 0, Both),
        shortcut("Social / extended chat", f(9), 0, Both),
        shortcut("Settings", O, MOD_CTRL, Both),
        shortcut("Screenshot", f(12), 0, Both),
        shortcut("Toggle mouse buttons", f(10), 0, Both),
        shortcut("Toggle fullscreen", ENTER, MOD_ALT, Both),
        shortcut("Now playing", f(6), 0, Lazer),
        shortcut("Toggle toolbar", T, MOD_CTRL, Lazer),
        shortcut("Beatmap listing", B, MOD_CTRL, Lazer),
        shortcut("Notifications", N, MOD_CTRL, Lazer),
        shortcut("Profile", P, MOD_CTRL, Lazer),
        shortcut("Main menu", HOME, MOD_ALT, Lazer),
        shortcut("Random skin", R, MOD_CTRL | MOD_SHIFT, Lazer),
        shortcut("Previous skin", E, MOD_CTRL | MOD_SHIFT, Lazer),
        shortcut("Next skin", T, MOD_CTRL | MOD_SHIFT, Lazer),
        shortcut("Boss key (hide osu!)", INSERT, 0, Stable),
        shortcut("Reload skin", S, MOD_CTRL | MOD_ALT | MOD_SHIFT, Stable),
    ]
};

/// The shortcut a key and modifiers trigger, if osu! has one (and it is
/// offered on this platform)
pub fn osu_shortcut(usage: u32, modifiers: u32) -> Option<OsuShortcut> {
    OSU_SHORTCUTS
        .iter()
        .copied()
        .find(|s| s.available() && s.usage == usage && s.modifiers == modifiers)
}

/// A keyboard key that osu! leaves free
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreeKey(pub u32);

impl fmt::Display for FreeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&key_name(self.0))
    }
}

/// Keys osu! binds nothing to, in any screen of either game: F13 to F24,
/// which no keyboard has, so binding one in osu!, OBS, Discord... clashes
/// with nothing. F24 is left out everywhere: the pad taps it to stop Windows
/// opening the window menu after Alt+wheel. On Linux the keyboard layout
/// gives F13 (Settings on GNOME), F20 (mic mute) and F21-F23 (touchpad)
/// system meanings, so only F14-F19 are offered there.
pub fn free_keys() -> Vec<FreeKey> {
    let range = if cfg!(target_os = "linux") {
        14..=19
    } else {
        13..=23
    };
    range.map(|n| FreeKey(usage::f(n))).collect()
}

/// One entry in a swipe's dropdown
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeChoice {
    Action(SwipeAction),
    Osu(OsuShortcut),
    /// A free keyboard key, picked in a second list
    OtherKey,
}

impl fmt::Display for SwipeChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SwipeChoice::Action(a) => a.fmt(f),
            SwipeChoice::Osu(s) => s.fmt(f),
            SwipeChoice::OtherKey => f.write_str("Other key (F13-F24)…"),
        }
    }
}

/// Every choice for one swipe, in the order the dropdown lists them
pub fn swipe_choices() -> Vec<SwipeChoice> {
    let mut choices: Vec<SwipeChoice> = SwipeAction::ALL
        .into_iter()
        .filter(|a| *a != SwipeAction::Key)
        .map(SwipeChoice::Action)
        .collect();
    choices.extend(
        OSU_SHORTCUTS
            .iter()
            .filter(|s| s.available())
            .map(|s| SwipeChoice::Osu(*s)),
    );
    choices.push(SwipeChoice::OtherKey);
    choices
}

/// The four swipe directions, in the pad's order
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SwipeDir {
    Up,
    Down,
    Left,
    Right,
}

impl SwipeDir {
    pub const ALL: [SwipeDir; 4] = [
        SwipeDir::Up,
        SwipeDir::Down,
        SwipeDir::Left,
        SwipeDir::Right,
    ];

    /// Position in [`SwipeDir::ALL`]
    pub fn index(self) -> usize {
        self as usize
    }
}

/// What one swipe direction is set to
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwipeSetting {
    pub action: SwipeAction,
    /// HID usage for [`SwipeAction::Key`], 0 = none
    pub key: u32,
    /// HID modifier bits held with `key`
    pub modifiers: u32,
}

impl SwipeSetting {
    /// How the dropdown shows it: a key that is an osu! shortcut shows as one
    pub fn choice(&self) -> SwipeChoice {
        match self.action {
            SwipeAction::Key => osu_shortcut(self.key, self.modifiers)
                .map_or(SwipeChoice::OtherKey, SwipeChoice::Osu),
            other => SwipeChoice::Action(other),
        }
    }

    /// The setting a dropdown choice gives. Picking "Other key" keeps a free
    /// key already chosen, so the second list does not reset.
    pub fn chosen(&self, choice: SwipeChoice) -> SwipeSetting {
        match choice {
            SwipeChoice::Action(action) => SwipeSetting { action, ..*self },
            SwipeChoice::Osu(s) => SwipeSetting {
                action: SwipeAction::Key,
                key: s.usage,
                modifiers: s.modifiers,
            },
            SwipeChoice::OtherKey => {
                let keep = self.modifiers == 0 && free_keys().contains(&FreeKey(self.key));
                SwipeSetting {
                    action: SwipeAction::Key,
                    key: if keep { self.key } else { 0 },
                    modifiers: 0,
                }
            }
        }
    }

    /// "Volume up", "osu!: Random beatmap (F2)", "key F15"
    pub fn describe(&self) -> String {
        match self.choice() {
            SwipeChoice::OtherKey if self.key == 0 => "key (none chosen)".into(),
            SwipeChoice::OtherKey => format!("key {}", combo_name(self.key, self.modifiers)),
            choice => choice.to_string(),
        }
    }
}

impl DeviceConfig {
    pub fn swipe(&self, dir: SwipeDir) -> SwipeSetting {
        let (action, key, modifiers) = match dir {
            SwipeDir::Up => (
                self.swipe_up_action,
                self.swipe_up_key,
                self.swipe_up_modifiers,
            ),
            SwipeDir::Down => (
                self.swipe_down_action,
                self.swipe_down_key,
                self.swipe_down_modifiers,
            ),
            SwipeDir::Left => (
                self.swipe_left_action,
                self.swipe_left_key,
                self.swipe_left_modifiers,
            ),
            SwipeDir::Right => (
                self.swipe_right_action,
                self.swipe_right_key,
                self.swipe_right_modifiers,
            ),
        };
        SwipeSetting {
            action,
            key,
            modifiers,
        }
    }

    pub fn set_swipe(&mut self, dir: SwipeDir, s: SwipeSetting) {
        let (action, key, modifiers) = match dir {
            SwipeDir::Up => (
                &mut self.swipe_up_action,
                &mut self.swipe_up_key,
                &mut self.swipe_up_modifiers,
            ),
            SwipeDir::Down => (
                &mut self.swipe_down_action,
                &mut self.swipe_down_key,
                &mut self.swipe_down_modifiers,
            ),
            SwipeDir::Left => (
                &mut self.swipe_left_action,
                &mut self.swipe_left_key,
                &mut self.swipe_left_modifiers,
            ),
            SwipeDir::Right => (
                &mut self.swipe_right_action,
                &mut self.swipe_right_key,
                &mut self.swipe_right_modifiers,
            ),
        };
        *action = s.action;
        *key = s.key;
        *modifiers = s.modifiers;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_are_named_with_their_keys() {
        let random = osu_shortcut(usage::f(2), 0).unwrap();
        assert_eq!(random.to_string(), "osu!: Random beatmap (F2)");
        let skin = osu_shortcut(usage::R, MOD_CTRL | MOD_SHIFT).unwrap();
        assert_eq!(skin.to_string(), "osu!lazer: Random skin (Ctrl+Shift+R)");
        assert_eq!(combo_name(usage::ENTER, MOD_ALT), "Alt+Enter");
        // Same key, other modifiers: another shortcut, or none
        assert_eq!(
            osu_shortcut(usage::f(2), MOD_SHIFT).unwrap().action,
            "Previous random beatmap"
        );
        assert!(osu_shortcut(usage::f(2), MOD_CTRL).is_none());
    }

    #[test]
    fn no_two_shortcuts_share_a_key() {
        for (i, a) in OSU_SHORTCUTS.iter().enumerate() {
            for b in &OSU_SHORTCUTS[i + 1..] {
                assert!(
                    (a.usage, a.modifiers) != (b.usage, b.modifiers),
                    "{} and {} share a key",
                    a.action,
                    b.action
                );
            }
        }
    }

    #[test]
    fn free_keys_are_no_osu_shortcut_and_never_f24() {
        let free = free_keys();
        assert!(!free.is_empty());
        for k in &free {
            assert!(osu_shortcut(k.0, 0).is_none());
            assert_ne!(k.0, usage::F24, "the pad's Alt mask key");
            assert!((usage::F13..=usage::F24).contains(&k.0));
        }
        if cfg!(target_os = "linux") {
            // F13 opens Settings on GNOME, F20 mutes the mic
            assert!(!free.contains(&FreeKey(usage::f(13))));
            assert!(!free.contains(&FreeKey(usage::f(20))));
        }
    }

    #[test]
    fn a_setting_round_trips_through_its_choice() {
        let none = SwipeSetting {
            action: SwipeAction::VolumeUp,
            key: 0,
            modifiers: 0,
        };
        assert_eq!(none.choice(), SwipeChoice::Action(SwipeAction::VolumeUp));

        let random = none.chosen(swipe_choices()[7]);
        assert!(matches!(random.choice(), SwipeChoice::Osu(_)));
        assert_eq!(random.describe(), "osu!: Random beatmap (F2)");

        // "Other key" drops an osu! key, keeps a free one
        let other = random.chosen(SwipeChoice::OtherKey);
        assert_eq!((other.action, other.key), (SwipeAction::Key, 0));
        let free = free_keys()[0];
        let picked = SwipeSetting {
            key: free.0,
            ..other
        };
        assert_eq!(picked.choice(), SwipeChoice::OtherKey);
        assert_eq!(picked.chosen(SwipeChoice::OtherKey), picked);
        assert_eq!(picked.describe(), format!("key {}", free));

        let mut cfg = DeviceConfig::default();
        cfg.set_swipe(SwipeDir::Left, random);
        assert_eq!(cfg.swipe(SwipeDir::Left), random);
        assert_eq!(cfg.swipe_left_key, usage::f(2));
    }

    #[test]
    fn choices_list_actions_then_osu_then_other_key() {
        let choices = swipe_choices();
        assert_eq!(choices[0], SwipeChoice::Action(SwipeAction::None));
        assert!(!choices.contains(&SwipeChoice::Action(SwipeAction::Key)));
        assert_eq!(choices.last(), Some(&SwipeChoice::OtherKey));
        let stable = OSU_SHORTCUTS
            .iter()
            .any(|s| s.client == OsuClient::Stable && choices.contains(&SwipeChoice::Osu(*s)));
        assert_eq!(stable, cfg!(windows));
    }
}
