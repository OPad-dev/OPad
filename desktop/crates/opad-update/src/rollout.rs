//! Gradual rollout (docs/specs/release-and-updates.md §9.2).
//!
//! A stable feed entry may carry `rollout.percent`. Each install decides on
//! its own whether it is in: its random install ID, the component and the
//! version are hashed into a bucket from 0 to 99, and the version is offered
//! when the bucket is below the percentage. Nothing is sent anywhere, the feed
//! is the same file for everyone, and an install keeps its bucket for a given
//! version, so raising the percentage only ever adds installs.
//!
//! The beta channel ignores rollout: people who chose beta asked for early.

use crate::manifest::{Component, ReleaseManifest, BETA};
use sha2::{Digest, Sha256};

/// This install's bucket for one version of one component, 0..=99
pub fn bucket(install_id: &str, component: &str, version: &str) -> u8 {
    let digest = Sha256::digest(format!("{install_id}\n{component}\n{version}").as_bytes());
    let n = u64::from_be_bytes(digest[..8].try_into().expect("8 bytes"));
    (n % 100) as u8
}

/// Whether this install is offered `component` at the version the feed names.
///
/// Without an install ID (the database is unavailable) a partial rollout is
/// not offered: the safe side of a gradual release is later, not sooner.
pub fn is_offered(
    feed_channel: &str,
    name: &str,
    component: &Component,
    install_id: Option<&str>,
) -> bool {
    if feed_channel == BETA {
        return true;
    }
    let Some(rollout) = component.rollout else {
        return true;
    };
    if rollout.percent >= 100 {
        return true;
    }
    match install_id {
        Some(id) => bucket(id, name, &component.version) < rollout.percent,
        None => false,
    }
}

/// The feed as this install sees it: components still being rolled out to
/// others are left out, which every planner reads as "nothing new".
pub fn for_install(feed: &ReleaseManifest, install_id: Option<&str>) -> ReleaseManifest {
    let mut out = feed.clone();
    out.components
        .retain(|name, c| is_offered(&feed.channel, name, c, install_id));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Rollout, APP, STABLE};

    fn component(percent: Option<u8>) -> Component {
        Component {
            version: "1.2.0".to_string(),
            rollout: percent.map(|percent| Rollout { percent }),
            ..Default::default()
        }
    }

    #[test]
    fn the_bucket_is_stable_and_in_range() {
        let b = bucket("3f1c-install", APP, "1.2.0");
        assert!(b < 100);
        assert_eq!(b, bucket("3f1c-install", APP, "1.2.0"));
    }

    #[test]
    fn buckets_spread_across_installs() {
        // 1000 installs at 10 %: roughly 100 are in, never none or all
        let ids: Vec<String> = (0..1000).map(|i| format!("install-{i}")).collect();
        let c = component(Some(10));
        let offered = ids
            .iter()
            .filter(|id| is_offered(STABLE, APP, &c, Some(id)))
            .count();
        assert!(
            (50..150).contains(&offered),
            "{offered} of 1000 offered at 10 %"
        );
    }

    #[test]
    fn raising_the_percentage_only_adds_installs() {
        let ids: Vec<String> = (0..200).map(|i| format!("install-{i}")).collect();
        for id in &ids {
            if is_offered(STABLE, APP, &component(Some(10)), Some(id)) {
                assert!(is_offered(STABLE, APP, &component(Some(50)), Some(id)));
            }
        }
    }

    #[test]
    fn no_rollout_or_full_rollout_is_offered_to_everyone() {
        assert!(is_offered(STABLE, APP, &component(None), None));
        assert!(is_offered(STABLE, APP, &component(Some(100)), None));
    }

    #[test]
    fn zero_percent_pauses_it_for_everyone() {
        for i in 0..100 {
            assert!(!is_offered(
                STABLE,
                APP,
                &component(Some(0)),
                Some(&format!("i{i}"))
            ));
        }
    }

    #[test]
    fn beta_ignores_rollout() {
        assert!(is_offered(BETA, APP, &component(Some(0)), None));
    }

    #[test]
    fn without_an_install_id_a_partial_rollout_waits() {
        assert!(!is_offered(STABLE, APP, &component(Some(99)), None));
    }

    #[test]
    fn for_install_drops_what_is_not_offered() {
        let mut feed = ReleaseManifest {
            schema: 2,
            channel: STABLE.to_string(),
            ..Default::default()
        };
        feed.components.insert(APP.to_string(), component(Some(0)));
        feed.components.insert("tosu".to_string(), component(None));
        let seen = for_install(&feed, Some("x"));
        assert!(!seen.components.contains_key(APP));
        assert!(seen.components.contains_key("tosu"));
    }
}
