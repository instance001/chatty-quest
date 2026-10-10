use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::app_paths;

const MEDIA_POOL_PATH: &str = "runtime/config/player_media_pool.json";

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PlayerMediaKind {
    Image,
    Motion,
    Audio,
}

impl PlayerMediaKind {
    pub fn from_path(path: &str) -> Result<Self, String> {
        let extension = Path::new(path)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        match extension.as_str() {
            "png" | "jpg" | "jpeg" | "webp" => Ok(Self::Image),
            "gif" | "mp4" | "webm" | "mov" => Ok(Self::Motion),
            "mp3" | "wav" | "ogg" | "flac" | "m4a" => Ok(Self::Audio),
            _ => Err("Supported media types are images, GIF/video, and audio files.".to_owned()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerMediaAsset {
    pub path: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub kind: PlayerMediaKind,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlayerMediaPool {
    #[serde(default)]
    pub assets: Vec<PlayerMediaAsset>,
}

pub fn load_player_media_pool() -> Result<PlayerMediaPool, String> {
    let path = app_paths::path(MEDIA_POOL_PATH);
    let json = match fs::read_to_string(&path) {
        Ok(json) => json,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PlayerMediaPool::default());
        }
        Err(error) => return Err(format!("Could not read player media pool: {}.", error)),
    };
    serde_json::from_str(&json)
        .map_err(|error| format!("Could not parse player media pool: {}.", error))
}

pub fn set_media_asset_enabled(path: &str, enabled: bool) -> Result<(), String> {
    set_media_asset_enabled_at(&app_paths::path(MEDIA_POOL_PATH), path, enabled)
}

fn set_media_asset_enabled_at(pool_path: &Path, path: &str, enabled: bool) -> Result<(), String> {
    mutate_pool_at(pool_path, |assets| {
        let asset = assets
            .iter_mut()
            .find(|asset| asset.path == path)
            .ok_or_else(|| format!("Unknown media asset '{}'.", path))?;
        asset.enabled = enabled;
        Ok(())
    })
}
pub fn remove_media_asset(path: &str) -> Result<(), String> {
    remove_media_asset_at(&app_paths::path(MEDIA_POOL_PATH), path)
}

fn remove_media_asset_at(pool_path: &Path, path: &str) -> Result<(), String> {
    mutate_pool_at(pool_path, |assets| {
        let before = assets.len();
        assets.retain(|asset| asset.path != path);
        if assets.len() == before {
            return Err(format!("Unknown media asset '{}'.", path));
        }
        Ok(())
    })
}

pub fn add_media_asset(path: String, tags: Vec<String>) -> Result<(), String> {
    let path = path.trim().to_owned();
    if path.is_empty() {
        return Err("Choose a local media file first.".to_owned());
    }
    if !Path::new(&path).is_file() {
        return Err(format!("Media file '{}' does not exist.", path));
    }
    let kind = PlayerMediaKind::from_path(&path)?;
    mutate_pool(|assets| {
        if assets.iter().any(|asset| asset.path == path) {
            return Err(format!(
                "Media file '{}' is already in the player pool.",
                path
            ));
        }
        assets.push(PlayerMediaAsset {
            path,
            tags: normalize_tags(tags),
            kind,
            enabled: true,
        });
        Ok(())
    })
}

pub fn set_media_asset_tags(path: &str, tags: Vec<String>) -> Result<(), String> {
    mutate_pool(|assets| {
        let asset = assets
            .iter_mut()
            .find(|asset| asset.path == path)
            .ok_or_else(|| format!("Unknown media asset '{}'.", path))?;
        asset.tags = normalize_tags(tags);
        Ok(())
    })
}

pub fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut normalized = tags
        .into_iter()
        .map(|tag| tag.trim().to_ascii_lowercase())
        .filter(|tag| !tag.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}
fn mutate_pool(
    change: impl FnOnce(&mut Vec<PlayerMediaAsset>) -> Result<(), String>,
) -> Result<(), String> {
    let path = app_paths::path(MEDIA_POOL_PATH);
    mutate_pool_at(&path, change)
}

fn mutate_pool_at(
    path: &Path,
    change: impl FnOnce(&mut Vec<PlayerMediaAsset>) -> Result<(), String>,
) -> Result<(), String> {
    let mut pool = match fs::read_to_string(path) {
        Ok(json) => serde_json::from_str(&json)
            .map_err(|error| format!("Could not parse player media pool: {}.", error))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => PlayerMediaPool::default(),
        Err(error) => return Err(format!("Could not read player media pool: {}.", error)),
    };
    change(&mut pool.assets)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(
        path,
        serde_json::to_string_pretty(&pool).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

pub fn best_matching_asset(
    pool: &PlayerMediaPool,
    tags: &[String],
    kind: PlayerMediaKind,
) -> Option<PlayerMediaAsset> {
    pool.assets
        .iter()
        .filter(|asset| asset.kind == kind)
        .filter(|asset| asset.enabled)
        .filter_map(|asset| {
            let score = asset
                .tags
                .iter()
                .filter(|tag| {
                    tags.iter()
                        .any(|candidate| candidate.eq_ignore_ascii_case(tag))
                })
                .count();
            (score > 0).then_some((score, asset))
        })
        .max_by(|(left_score, left_asset), (right_score, right_asset)| {
            left_score
                .cmp(right_score)
                .then_with(|| right_asset.path.cmp(&left_asset.path))
        })
        .map(|(_, asset)| asset.clone())
}

/// Host-owned deterministic pool selection. The caller supplies a stable runtime key so two
/// entities can vary while save/load/replay keep selecting the same eligible asset.
pub fn select_pool_asset(
    pool: &PlayerMediaPool,
    tags: &[String],
    kind: PlayerMediaKind,
    stable_key: &str,
) -> Option<PlayerMediaAsset> {
    let mut candidates = pool
        .assets
        .iter()
        .filter(|asset| asset.kind == kind && asset.enabled)
        .filter_map(|asset| {
            let score = asset
                .tags
                .iter()
                .filter(|tag| {
                    tags.iter()
                        .any(|candidate| candidate.eq_ignore_ascii_case(tag))
                })
                .count();
            (score > 0).then_some((score, asset))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| left.path.cmp(&right.path))
    });
    let best_score = candidates.first()?.0;
    let best = candidates
        .into_iter()
        .filter(|(score, _)| *score == best_score)
        .collect::<Vec<_>>();
    let index = stable_key.bytes().fold(0usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as usize)
    }) % best.len();
    Some(best[index].1.clone())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        PlayerMediaAsset, PlayerMediaKind, PlayerMediaPool, best_matching_asset, mutate_pool_at,
        remove_media_asset_at, select_pool_asset, set_media_asset_enabled_at,
    };

    #[test]
    fn tag_match_prefers_the_asset_with_more_matching_tags() {
        let pool = PlayerMediaPool {
            assets: vec![
                PlayerMediaAsset {
                    path: "weapon.png".to_owned(),
                    tags: vec!["weapon".to_owned()],
                    kind: PlayerMediaKind::Image,
                    enabled: true,
                },
                PlayerMediaAsset {
                    path: "blade_absurd.png".to_owned(),
                    tags: vec!["blade".to_owned(), "absurd".to_owned()],
                    kind: PlayerMediaKind::Image,
                    enabled: true,
                },
            ],
        };
        let tags = vec!["weapon".to_owned(), "blade".to_owned(), "absurd".to_owned()];

        assert_eq!(
            best_matching_asset(&pool, &tags, PlayerMediaKind::Image)
                .expect("expected a tagged image")
                .path,
            "blade_absurd.png"
        );
    }

    #[test]
    fn old_media_pool_json_defaults_assets_to_enabled() {
        let pool: PlayerMediaPool = serde_json::from_str(
            r#"{"assets":[{"path":"test.png","tags":["weapon"],"kind":"image"}]}"#,
        )
        .expect("old media pool JSON should remain readable");
        assert!(pool.assets[0].enabled);
    }

    #[test]
    fn disabled_media_asset_does_not_match() {
        let pool = PlayerMediaPool {
            assets: vec![PlayerMediaAsset {
                path: "disabled.png".to_owned(),
                tags: vec!["weapon".to_owned()],
                kind: PlayerMediaKind::Image,
                enabled: false,
            }],
        };
        assert!(
            best_matching_asset(&pool, &["weapon".to_owned()], PlayerMediaKind::Image).is_none()
        );
    }

    #[test]
    fn pool_selection_is_stable_for_the_same_runtime_key() {
        let pool = PlayerMediaPool {
            assets: vec![
                PlayerMediaAsset {
                    path: "first.png".to_owned(),
                    tags: vec!["undead".to_owned()],
                    kind: PlayerMediaKind::Image,
                    enabled: true,
                },
                PlayerMediaAsset {
                    path: "second.png".to_owned(),
                    tags: vec!["undead".to_owned()],
                    kind: PlayerMediaKind::Image,
                    enabled: true,
                },
                PlayerMediaAsset {
                    path: "disabled.png".to_owned(),
                    tags: vec!["undead".to_owned()],
                    kind: PlayerMediaKind::Image,
                    enabled: false,
                },
            ],
        };
        let tags = vec!["undead".to_owned()];
        let first = select_pool_asset(&pool, &tags, PlayerMediaKind::Image, "enemy:one")
            .expect("enabled candidates resolve");
        let repeat = select_pool_asset(&pool, &tags, PlayerMediaKind::Image, "enemy:one")
            .expect("the same runtime key resolves again");

        assert_eq!(first.path, repeat.path);
        assert_ne!(first.path, "disabled.png");
    }

    #[test]
    fn media_enable_and_remove_changes_persist_to_disk() {
        let path = std::env::temp_dir().join(format!(
            "chatty_quest_media_toggle_{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos()
        ));
        mutate_pool_at(&path, |assets| {
            assets.push(PlayerMediaAsset {
                path: "fixture.png".to_owned(),
                tags: vec!["fixture".to_owned()],
                kind: PlayerMediaKind::Image,
                enabled: true,
            });
            Ok(())
        })
        .expect("fixture media pool writes");
        set_media_asset_enabled_at(&path, "fixture.png", false)
            .expect("disable writes through the media store");

        let stored: PlayerMediaPool = serde_json::from_str(
            &std::fs::read_to_string(&path).expect("media pool can be reread"),
        )
        .expect("media pool remains valid JSON");
        assert!(!stored.assets[0].enabled);

        remove_media_asset_at(&path, "fixture.png").expect("remove writes through the media store");
        let stored: PlayerMediaPool = serde_json::from_str(
            &std::fs::read_to_string(&path).expect("media pool can be reread"),
        )
        .expect("media pool remains valid JSON");
        assert!(stored.assets.is_empty());
        std::fs::remove_file(path).expect("fixture media pool cleanup");
    }
}
