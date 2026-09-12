//! UI data source ids, mirroring `firmware/main/ui/core/ui_ids.h` (`ui_source_t`).
//! `osupad-ui-preview` tests that ids and names match the firmware table.

use serde::{Deserialize, Serialize};

/// A value for a UI data source, as sent to the device in `DataUpdate`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SourceValue {
    Number(f64),
    Text(String),
    /// No value (widgets with "hide when empty" disappear)
    Clear,
}

macro_rules! sources {
    ($($name:ident = $id:literal, $key:literal;)*) => {
        $(pub const $name: u8 = $id;)*
        /// (id, stable name) for every defined source
        pub const ALL: &[(u8, &str)] = &[$(($id, $key)),*];
    };
}

sources! {
    MAP_TITLE = 1, "map.title";
    MAP_ARTIST = 2, "map.artist";
    MAP_MAPPER = 3, "map.mapper";
    MAP_DIFFICULTY = 4, "map.difficulty";
    MAP_STATUS = 5, "map.status";
    MAP_STARS = 6, "map.stars";
    MAP_STARS_LIVE = 7, "map.stars_live";
    MAP_AR = 8, "map.ar";
    MAP_CS = 9, "map.cs";
    MAP_OD = 10, "map.od";
    MAP_HP = 11, "map.hp";
    MAP_BPM = 12, "map.bpm";
    MAP_OBJECTS = 13, "map.objects";
    MAP_MAX_COMBO = 14, "map.max_combo";
    MAP_LENGTH = 15, "map.length";
    MAP_TIME_ELAPSED = 16, "map.time_elapsed";
    MAP_TIME_REMAINING = 17, "map.time_remaining";
    MAP_PROGRESS = 18, "map.progress";
    MAP_KIAI = 19, "map.kiai";

    PLAY_PP = 20, "play.pp";
    PLAY_PP_FC = 21, "play.pp_fc";
    PLAY_PP_MAX = 22, "play.pp_max";
    PLAY_ACCURACY = 23, "play.accuracy";
    PLAY_SCORE = 24, "play.score";
    PLAY_COMBO = 25, "play.combo";
    PLAY_MAX_COMBO = 26, "play.max_combo";
    PLAY_GRADE = 27, "play.grade";
    PLAY_HITS_300 = 28, "play.hits_300";
    PLAY_HITS_100 = 29, "play.hits_100";
    PLAY_HITS_50 = 30, "play.hits_50";
    PLAY_HITS_MISS = 31, "play.hits_miss";
    PLAY_SLIDER_BREAKS = 32, "play.slider_breaks";
    PLAY_UR = 33, "play.ur";
    PLAY_HEALTH = 34, "play.health";
    PLAY_MODS = 35, "play.mods";
    PLAY_PLAYER = 36, "play.player";
    PLAY_FAILED = 37, "play.failed";

    PROFILE_NAME = 40, "profile.name";
    PROFILE_RANK = 41, "profile.rank";
    PROFILE_PP = 43, "profile.pp";
    PROFILE_ACCURACY = 44, "profile.accuracy";
    PROFILE_PLAYCOUNT = 45, "profile.playcount";
    PROFILE_LEVEL = 46, "profile.level";
    PROFILE_COUNTRY = 47, "profile.country";
    SESSION_PLAYTIME = 48, "session.playtime";
    SESSION_PLAYCOUNT = 49, "session.playcount";
    GAME_STATE = 50, "game.state";

    PAD_K1_MAP = 60, "pad.k1_map";
    PAD_K2_MAP = 61, "pad.k2_map";
    PAD_TOTAL_MAP = 62, "pad.total_map";
    PAD_K1_LIFETIME = 63, "pad.k1_lifetime";
    PAD_K2_LIFETIME = 64, "pad.k2_lifetime";
    PAD_TOTAL_LIFETIME = 65, "pad.total_lifetime";
    PAD_KPS = 66, "pad.kps";
    PAD_K1_LABEL = 67, "pad.k1_label";
    PAD_K2_LABEL = 68, "pad.k2_label";
    PAD_CLOCK = 69, "pad.clock";
    PAD_DATE = 70, "pad.date";
    PAD_UPTIME = 71, "pad.uptime";
    PAD_K1_DOWN = 72, "pad.k1_down";
    PAD_K2_DOWN = 73, "pad.k2_down";

    STATUS_PC = 80, "status.pc";
    STATUS_TOSU = 81, "status.tosu";
    STATUS_OSU = 82, "status.osu";
}
