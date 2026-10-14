//! Result animations (issue #3): what the pad plays when osu! shows the
//! results screen, picked per grade on the PC.
//!
//! Ids are `PlayAnimation.animation` on the wire and match
//! `firmware/main/ui/core/ui_anim.h`. A built-in is never renumbered: the
//! choice is stored by id. [`CUSTOM_BASE`] + [`Trigger::index`] is reserved
//! for an uploaded animation, one slot per trigger that the next upload to
//! it overwrites.

use serde::{Deserialize, Serialize};

pub const NONE: u32 = 0;
pub const POP: u32 = 1;
pub const DROP: u32 = 2;
pub const BURST: u32 = 3;
pub const CUSTOM_BASE: u32 = 128;

/// The built-ins as the app lists them, in id order
pub const BUILTINS: &[(u32, &str)] = &[(POP, "Pop"), (DROP, "Drop"), (BURST, "Burst")];

/// Name of an animation id for the app ("None", "Pop", ...)
pub fn name(id: u32) -> &'static str {
    if id == NONE {
        return "None";
    }
    BUILTINS
        .iter()
        .find(|(b, _)| *b == id)
        .map_or("Custom", |(_, n)| n)
}

/// What starts an animation. Grades only for now; full combo, PBs and PP
/// records come after them, so a trigger's index never changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Trigger {
    SS,
    S,
    A,
    B,
    C,
    D,
}

impl Trigger {
    pub const ALL: [Trigger; 6] = [
        Trigger::SS,
        Trigger::S,
        Trigger::A,
        Trigger::B,
        Trigger::C,
        Trigger::D,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Trigger::SS => "SS",
            Trigger::S => "S",
            Trigger::A => "A",
            Trigger::B => "B",
            Trigger::C => "C",
            Trigger::D => "D",
        }
    }

    /// The trigger for tosu's rank. Silver ranks count as their gold one
    /// (stable says X/XH for SS, lazer SS/SSH); F and anything unknown has
    /// none.
    pub fn from_grade(rank: &str) -> Option<Self> {
        match rank.trim().to_ascii_uppercase().as_str() {
            "X" | "XH" | "SS" | "SSH" => Some(Trigger::SS),
            "S" | "SH" => Some(Trigger::S),
            "A" => Some(Trigger::A),
            "B" => Some(Trigger::B),
            "C" => Some(Trigger::C),
            "D" => Some(Trigger::D),
            _ => None,
        }
    }
}

/// The animation picked for each [`Trigger`], by [`Trigger::index`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResultAnimations([u32; 6]);

impl Default for ResultAnimations {
    fn default() -> Self {
        Self([BURST, BURST, POP, POP, DROP, DROP])
    }
}

impl ResultAnimations {
    pub fn get(&self, trigger: Trigger) -> u32 {
        self.0[trigger.index()]
    }

    pub fn set(&mut self, trigger: Trigger, id: u32) {
        self.0[trigger.index()] = id;
    }

    /// As stored in the PC's database: "3,3,1,1,2,2"
    pub fn to_storage(&self) -> String {
        self.0
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Read back [`ResultAnimations::to_storage`]. A missing or unreadable
    /// entry keeps its default, so a newer app's longer list still loads.
    pub fn from_storage(s: &str) -> Self {
        let mut out = Self::default();
        for (slot, part) in out.0.iter_mut().zip(s.split(',')) {
            if let Ok(id) = part.trim().parse() {
                *slot = id;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grades_map_to_triggers() {
        assert_eq!(Trigger::from_grade("XH"), Some(Trigger::SS));
        assert_eq!(Trigger::from_grade("SSH"), Some(Trigger::SS));
        assert_eq!(Trigger::from_grade("sh"), Some(Trigger::S));
        assert_eq!(Trigger::from_grade("D"), Some(Trigger::D));
        assert_eq!(Trigger::from_grade("F"), None);
        assert_eq!(Trigger::from_grade(""), None);
    }

    #[test]
    fn storage_round_trips_and_tolerates_junk() {
        let mut anims = ResultAnimations::default();
        anims.set(Trigger::A, NONE);
        anims.set(Trigger::D, CUSTOM_BASE + 5);
        assert_eq!(ResultAnimations::from_storage(&anims.to_storage()), anims);

        let partial = ResultAnimations::from_storage("2,x");
        assert_eq!(partial.get(Trigger::SS), DROP);
        assert_eq!(partial.get(Trigger::S), BURST);
        assert_eq!(partial.get(Trigger::D), DROP);
    }

    #[test]
    fn names() {
        assert_eq!(name(NONE), "None");
        assert_eq!(name(BURST), "Burst");
        assert_eq!(name(CUSTOM_BASE), "Custom");
    }
}
