use std::fs;

use serde::{Deserialize, Serialize};

use crate::app_paths;
use crate::game::{ActorDisposition, ActorState};

const COLLECTION_VERSION: u32 = 1;
const COLLECTION_PATH: &str = "runtime/config/personal_narrator_collection.json";

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PlayerTemplateKind {
    #[default]
    Item,
    Weapon,
    Status,
    Effect,
    Npc,
    Enemy,
    Boss,
    Objective,
    Location,
    TemporaryExit,
}

impl PlayerTemplateKind {
    pub fn is_item_like(&self) -> bool {
        matches!(self, Self::Item | Self::Weapon)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct PlayerTemplateAdmission {
    #[serde(default = "default_admission_source")]
    pub source: String,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub proposal_id: Option<String>,
}

fn default_admission_source() -> String {
    "narrator_approved".to_owned()
}

fn default_admission() -> PlayerTemplateAdmission {
    PlayerTemplateAdmission {
        source: default_admission_source(),
        ..PlayerTemplateAdmission::default()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct PlayerTemplateMediaReferences {
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub motion: Option<String>,
    #[serde(default)]
    pub audio: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(tag = "family", rename_all = "snake_case")]
pub enum PlayerTemplateMechanics {
    Item {
        damage: i32,
        condition: Option<String>,
    },
    Status {
        trigger: String,
        duration: String,
        #[serde(default)]
        additive_damage: i32,
        #[serde(default)]
        additive_healing: i32,
        #[serde(default)]
        required_target_tags: Vec<String>,
        #[serde(default)]
        required_context_tags: Vec<String>,
    },
    Npc {
        faction: String,
    },
    Enemy {
        max_hp: i32,
        damage: i32,
    },
    Boss {
        max_hp: i32,
        damage: i32,
    },
    Objective {
        completion_key: String,
    },
    Location {
        #[serde(default)]
        location_id: String,
        initially_known: bool,
    },
    TemporaryExit {
        from_location_id: String,
        to_location_id: String,
        locked: bool,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerTemplate {
    pub id: String,
    #[serde(default)]
    pub kind: PlayerTemplateKind,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub damage: i32,
    #[serde(default)]
    pub condition: Option<String>,
    #[serde(default)]
    pub mechanics: Option<PlayerTemplateMechanics>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Optional shared host actor component. It uses the same state and capability vocabulary as
    /// authored templates and runtime proposal admission.
    #[serde(default)]
    pub actor: Option<ActorState>,
    #[serde(default)]
    pub disposition: ActorDisposition,
    pub source_datapack_id: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_provenance")]
    pub provenance: String,
    #[serde(default = "default_admission")]
    pub admission: PlayerTemplateAdmission,
    #[serde(default)]
    pub media_references: PlayerTemplateMediaReferences,
}

pub type CollectionItem = PlayerTemplate;

impl PlayerTemplate {
    pub fn effective_mechanics(&self) -> PlayerTemplateMechanics {
        self.mechanics
            .clone()
            .unwrap_or(PlayerTemplateMechanics::Item {
                damage: self.damage,
                condition: self.condition.clone(),
            })
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty()
            || self.name.trim().is_empty()
            || self.description.trim().is_empty()
        {
            return Err("Player templates require non-empty id, name, and description.".to_owned());
        }
        if self.source_datapack_id.trim().is_empty() {
            return Err("Player templates require a compatible datapack id.".to_owned());
        }
        let mechanics = self.effective_mechanics();
        let kind_matches = matches!(
            (&self.kind, &mechanics),
            (
                PlayerTemplateKind::Item | PlayerTemplateKind::Weapon,
                PlayerTemplateMechanics::Item { .. }
            ) | (
                PlayerTemplateKind::Status | PlayerTemplateKind::Effect,
                PlayerTemplateMechanics::Status { .. }
            ) | (PlayerTemplateKind::Npc, PlayerTemplateMechanics::Npc { .. })
                | (
                    PlayerTemplateKind::Enemy,
                    PlayerTemplateMechanics::Enemy { .. }
                )
                | (
                    PlayerTemplateKind::Boss,
                    PlayerTemplateMechanics::Boss { .. }
                )
                | (
                    PlayerTemplateKind::Objective,
                    PlayerTemplateMechanics::Objective { .. }
                )
                | (
                    PlayerTemplateKind::Location,
                    PlayerTemplateMechanics::Location { .. }
                )
                | (
                    PlayerTemplateKind::TemporaryExit,
                    PlayerTemplateMechanics::TemporaryExit { .. }
                )
        );
        if !kind_matches {
            return Err(
                "Player template kind does not match its explicit mechanics family.".to_owned(),
            );
        }
        if let Some(actor) = &self.actor {
            let mut errors = Vec::new();
            crate::game::capabilities::validate_selection(
                "Player template",
                &self.id,
                &actor.capabilities,
                true,
                actor.max_hp.is_some_and(|hp| hp > 0),
                actor.damage.is_some_and(|damage| damage >= 0),
                actor.can_hear,
                actor.can_see,
                actor.can_move,
                &mut errors,
            );
            if !errors.is_empty() {
                return Err(errors.join(" "));
            }
        }
        match mechanics {
            PlayerTemplateMechanics::Enemy { max_hp, damage }
            | PlayerTemplateMechanics::Boss { max_hp, damage }
                if max_hp <= 0 || damage < 0 =>
            {
                Err(
                    "Encounter templates require positive max HP and non-negative damage."
                        .to_owned(),
                )
            }
            PlayerTemplateMechanics::TemporaryExit {
                from_location_id,
                to_location_id,
                ..
            } if from_location_id.trim().is_empty() || to_location_id.trim().is_empty() => {
                Err("Temporary exits require both endpoint location ids.".to_owned())
            }
            PlayerTemplateMechanics::Location { location_id, .. }
                if location_id.trim().is_empty() =>
            {
                Err("Location templates require a target location id.".to_owned())
            }
            _ => Ok(()),
        }
    }
}

fn default_enabled() -> bool {
    true
}

fn default_provenance() -> String {
    "player_library".to_owned()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersonalCollection {
    version: u32,
    #[serde(default, alias = "items")]
    templates: Vec<PlayerTemplate>,
}

pub fn save_collection_item(mut item: CollectionItem) -> Result<String, String> {
    item.tags = normalize_tags(item.tags);
    item.validate()?;
    let path = app_paths::path(COLLECTION_PATH);
    let mut collection = match fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str::<PersonalCollection>(&json)
            .map_err(|err| format!("Could not parse personal narrator collection: {}.", err))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => PersonalCollection {
            version: COLLECTION_VERSION,
            templates: Vec::new(),
        },
        Err(error) => {
            return Err(format!(
                "Could not read personal narrator collection: {}.",
                error
            ));
        }
    };

    if collection.version != COLLECTION_VERSION {
        return Err(format!(
            "Unsupported personal narrator collection version {}.",
            collection.version
        ));
    }

    if let Some(existing) = collection
        .templates
        .iter_mut()
        .find(|existing| existing.id == item.id)
    {
        *existing = item;
    } else {
        collection.templates.push(item);
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Could not create collection folder: {}.", err))?;
    }
    let json = serde_json::to_string_pretty(&collection)
        .map_err(|err| format!("Could not serialize personal narrator collection: {}.", err))?;
    fs::write(&path, json)
        .map_err(|err| format!("Could not save personal narrator collection: {}.", err))?;
    Ok(path.display().to_string())
}

pub fn save_player_template(template: PlayerTemplate) -> Result<String, String> {
    save_collection_item(template)
}

pub fn load_collection_items_for_datapack(
    datapack_id: &str,
) -> Result<Vec<CollectionItem>, String> {
    let path = app_paths::path(COLLECTION_PATH);
    let json = match fs::read_to_string(&path) {
        Ok(json) => json,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "Could not read personal narrator collection: {}.",
                error
            ));
        }
    };
    let collection = serde_json::from_str::<PersonalCollection>(&json)
        .map_err(|err| format!("Could not parse personal narrator collection: {}.", err))?;
    if collection.version != COLLECTION_VERSION {
        return Err(format!(
            "Unsupported personal narrator collection version {}.",
            collection.version
        ));
    }
    let templates = collection
        .templates
        .into_iter()
        .filter(|item| item.source_datapack_id == datapack_id)
        .collect::<Vec<_>>();
    for template in &templates {
        template.validate().map_err(|error| {
            format!(
                "Personal narrator collection contains invalid template '{}': {error}",
                template.id
            )
        })?;
    }
    Ok(templates)
}

pub fn load_player_templates_for_datapack(
    datapack_id: &str,
) -> Result<Vec<PlayerTemplate>, String> {
    load_collection_items_for_datapack(datapack_id)
}

pub fn set_collection_item_enabled(id: &str, enabled: bool) -> Result<(), String> {
    set_collection_item_enabled_at(&app_paths::path(COLLECTION_PATH), id, enabled)
}

fn set_collection_item_enabled_at(
    path: &std::path::Path,
    id: &str,
    enabled: bool,
) -> Result<(), String> {
    mutate_collection_at(path, |items| {
        let item = items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| format!("Unknown collection item '{}'.", id))?;
        item.enabled = enabled;
        Ok(())
    })
}
pub fn remove_collection_item(id: &str) -> Result<(), String> {
    remove_collection_item_at(&app_paths::path(COLLECTION_PATH), id)
}

fn remove_collection_item_at(path: &std::path::Path, id: &str) -> Result<(), String> {
    mutate_collection_at(path, |items| {
        let before = items.len();
        items.retain(|item| item.id != id);
        if items.len() == before {
            return Err(format!("Unknown collection item '{}'.", id));
        }
        Ok(())
    })
}

pub fn set_collection_item_tags(id: &str, tags: Vec<String>) -> Result<(), String> {
    mutate_collection(|items| {
        let item = items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| format!("Unknown collection item '{}'.", id))?;
        item.tags = normalize_tags(tags);
        Ok(())
    })
}

/// Exact player-owned presentation references are optional and never affect a template's
/// mechanics. When present, the media resolver prefers them over tagged pool selection.
pub fn set_collection_item_media_references(
    id: &str,
    media_references: PlayerTemplateMediaReferences,
) -> Result<(), String> {
    set_collection_item_media_references_at(&app_paths::path(COLLECTION_PATH), id, media_references)
}

fn set_collection_item_media_references_at(
    path: &std::path::Path,
    id: &str,
    media_references: PlayerTemplateMediaReferences,
) -> Result<(), String> {
    mutate_collection_at(path, |items| {
        let item = items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| format!("Unknown collection item '{}'.", id))?;
        item.media_references = PlayerTemplateMediaReferences {
            image: normalize_optional_media_path(media_references.image),
            motion: normalize_optional_media_path(media_references.motion),
            audio: normalize_optional_media_path(media_references.audio),
        };
        Ok(())
    })
}

fn normalize_optional_media_path(value: Option<String>) -> Option<String> {
    value.and_then(|path| {
        let path = path.trim();
        (!path.is_empty()).then(|| path.to_owned())
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
fn mutate_collection(
    change: impl FnOnce(&mut Vec<CollectionItem>) -> Result<(), String>,
) -> Result<(), String> {
    let path = app_paths::path(COLLECTION_PATH);
    mutate_collection_at(&path, change)
}

fn mutate_collection_at(
    path: &std::path::Path,
    change: impl FnOnce(&mut Vec<CollectionItem>) -> Result<(), String>,
) -> Result<(), String> {
    let mut collection = match fs::read_to_string(path) {
        Ok(json) => serde_json::from_str::<PersonalCollection>(&json).map_err(|e| e.to_string())?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => PersonalCollection {
            version: COLLECTION_VERSION,
            templates: Vec::new(),
        },
        Err(e) => return Err(e.to_string()),
    };
    change(&mut collection.templates)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(
        path,
        serde_json::to_string_pretty(&collection).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

pub fn collection_item_id(datapack_id: &str, name: &str) -> String {
    let normalized_name = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("{}.{}", datapack_id, normalized_name.trim_matches('_'))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::game::ActorDisposition;

    use super::{
        PersonalCollection, PlayerTemplate, PlayerTemplateKind, PlayerTemplateMechanics,
        PlayerTemplateMediaReferences, collection_item_id, mutate_collection_at, normalize_tags,
        remove_collection_item_at, set_collection_item_enabled_at,
        set_collection_item_media_references_at,
    };

    fn item_template(id: &str) -> PlayerTemplate {
        PlayerTemplate {
            id: id.to_owned(),
            kind: PlayerTemplateKind::Item,
            name: "Test item".to_owned(),
            description: "A persistence fixture.".to_owned(),
            damage: 0,
            condition: None,
            mechanics: Some(PlayerTemplateMechanics::Item {
                damage: 0,
                condition: None,
            }),
            tags: Vec::new(),
            actor: None,
            disposition: ActorDisposition::Neutral,
            source_datapack_id: "test".to_owned(),
            enabled: true,
            provenance: "test".to_owned(),
            admission: Default::default(),
            media_references: Default::default(),
        }
    }

    #[test]
    fn collection_item_id_is_stable_and_scoped_to_the_source_pack() {
        assert_eq!(
            collection_item_id("property_siege_classic", "Test Item"),
            "property_siege_classic.test_item"
        );
    }

    #[test]
    fn old_collection_json_defaults_entries_to_enabled() {
        let collection: PersonalCollection = serde_json::from_str(
            r#"{"version":1,"items":[{"id":"test.item","name":"Test","description":"Test","damage":1,"condition":null,"source_datapack_id":"test"}]}"#,
        )
        .expect("old collection JSON should remain readable");
        assert!(collection.templates[0].enabled);
        assert_eq!(collection.templates[0].kind, PlayerTemplateKind::Item);
        assert_eq!(collection.templates[0].provenance, "player_library");
        assert_eq!(
            collection.templates[0].admission.source,
            "narrator_approved"
        );
    }

    #[test]
    fn player_template_tags_are_normalized_for_deterministic_matching() {
        assert_eq!(
            normalize_tags(vec![
                " Blade ".to_owned(),
                "blade".to_owned(),
                "".to_owned()
            ]),
            vec!["blade".to_owned()]
        );
    }

    #[test]
    fn template_kind_must_match_its_mechanics_family() {
        let template = PlayerTemplate {
            id: "test.enemy".to_owned(),
            kind: PlayerTemplateKind::Enemy,
            name: "Test enemy".to_owned(),
            description: "A validation fixture.".to_owned(),
            damage: 0,
            condition: None,
            mechanics: Some(PlayerTemplateMechanics::Item {
                damage: 1,
                condition: None,
            }),
            tags: Vec::new(),
            actor: None,
            disposition: ActorDisposition::Neutral,
            source_datapack_id: "test".to_owned(),
            enabled: true,
            provenance: "test".to_owned(),
            admission: Default::default(),
            media_references: Default::default(),
        };
        assert!(template.validate().is_err());
    }

    #[test]
    fn legacy_status_mechanics_default_new_evaluator_fields() {
        let template: PlayerTemplate = serde_json::from_str(
            r#"{"id":"test.status","kind":"status","name":"Focus","description":"A test effect.","source_datapack_id":"test","mechanics":{"family":"status","trigger":"combat","duration":"run"}}"#,
        )
        .expect("legacy status template should remain readable");
        match template.effective_mechanics() {
            PlayerTemplateMechanics::Status {
                additive_damage,
                additive_healing,
                required_target_tags,
                required_context_tags,
                ..
            } => {
                assert_eq!(additive_damage, 0);
                assert_eq!(additive_healing, 0);
                assert!(required_target_tags.is_empty());
                assert!(required_context_tags.is_empty());
            }
            _ => panic!("expected status mechanics"),
        }
    }

    #[test]
    fn typed_exit_and_objective_templates_validate_with_explicit_mechanics() {
        let mut exit = PlayerTemplate {
            id: "test.exit".to_owned(),
            kind: PlayerTemplateKind::TemporaryExit,
            name: "Back shortcut".to_owned(),
            description: "A test route.".to_owned(),
            damage: 0,
            condition: None,
            mechanics: Some(PlayerTemplateMechanics::TemporaryExit {
                from_location_id: "front_verandah".to_owned(),
                to_location_id: "laundry".to_owned(),
                locked: false,
            }),
            tags: Vec::new(),
            actor: None,
            disposition: ActorDisposition::Neutral,
            source_datapack_id: "test".to_owned(),
            enabled: true,
            provenance: "test".to_owned(),
            admission: Default::default(),
            media_references: Default::default(),
        };
        assert!(exit.validate().is_ok());
        exit.kind = PlayerTemplateKind::Objective;
        exit.mechanics = Some(PlayerTemplateMechanics::Objective {
            completion_key: "laundry".to_owned(),
        });
        assert!(exit.validate().is_ok());
        exit.kind = PlayerTemplateKind::Location;
        exit.mechanics = Some(PlayerTemplateMechanics::Location {
            location_id: "laundry".to_owned(),
            initially_known: true,
        });
        assert!(exit.validate().is_ok());
    }

    #[test]
    fn boss_templates_require_positive_hp_and_non_negative_damage() {
        let template = PlayerTemplate {
            id: "test.boss".to_owned(),
            kind: PlayerTemplateKind::Boss,
            name: "Test boss".to_owned(),
            description: "A validation fixture.".to_owned(),
            damage: 0,
            condition: None,
            mechanics: Some(PlayerTemplateMechanics::Boss {
                max_hp: 12,
                damage: 3,
            }),
            tags: vec!["boss".to_owned()],
            actor: None,
            disposition: ActorDisposition::Hostile,
            source_datapack_id: "test".to_owned(),
            enabled: true,
            provenance: "test".to_owned(),
            admission: Default::default(),
            media_references: Default::default(),
        };
        assert!(template.validate().is_ok());
        let mut invalid = template;
        invalid.mechanics = Some(PlayerTemplateMechanics::Boss {
            max_hp: 0,
            damage: 3,
        });
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn template_enable_and_remove_changes_persist_to_disk() {
        let path = std::env::temp_dir().join(format!(
            "chatty_quest_collection_toggle_{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos()
        ));
        mutate_collection_at(&path, |templates| {
            templates.push(item_template("test.item"));
            Ok(())
        })
        .expect("fixture collection writes");
        set_collection_item_enabled_at(&path, "test.item", false)
            .expect("disable writes through the collection store");

        let stored: PersonalCollection = serde_json::from_str(
            &std::fs::read_to_string(&path).expect("collection can be reread"),
        )
        .expect("collection remains valid JSON");
        assert!(!stored.templates[0].enabled);

        remove_collection_item_at(&path, "test.item")
            .expect("remove writes through the collection store");
        let stored: PersonalCollection = serde_json::from_str(
            &std::fs::read_to_string(&path).expect("collection can be reread"),
        )
        .expect("collection remains valid JSON");
        assert!(stored.templates.is_empty());
        std::fs::remove_file(path).expect("fixture collection cleanup");
    }

    #[test]
    fn exact_template_media_references_persist_and_normalize_blank_values() {
        let path = std::env::temp_dir().join(format!(
            "chatty_quest_collection_media_{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos()
        ));
        mutate_collection_at(&path, |templates| {
            templates.push(item_template("test.item"));
            Ok(())
        })
        .expect("fixture collection writes");

        set_collection_item_media_references_at(
            &path,
            "test.item",
            PlayerTemplateMediaReferences {
                image: Some(" C:/media/item.png ".to_owned()),
                motion: Some("   ".to_owned()),
                audio: Some("C:/media/item.ogg".to_owned()),
            },
        )
        .expect("media references write through the collection store");

        let stored: PersonalCollection = serde_json::from_str(
            &std::fs::read_to_string(&path).expect("collection can be reread"),
        )
        .expect("collection remains valid JSON");
        assert_eq!(
            stored.templates[0].media_references.image.as_deref(),
            Some("C:/media/item.png")
        );
        assert!(stored.templates[0].media_references.motion.is_none());
        assert_eq!(
            stored.templates[0].media_references.audio.as_deref(),
            Some("C:/media/item.ogg")
        );
        std::fs::remove_file(path).expect("fixture collection cleanup");
    }
}
