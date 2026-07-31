use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::app_paths;

const DATAPACKS_ROOT: &str = "assets/datapacks";
const DATAPACK_SCHEMA_VERSION: &str = "v0.1-toml-templates-2";

fn default_true() -> bool {
    true
}

fn default_sight_acquire_chance_percent() -> u8 {
    70
}

fn default_sight_chase_delay_chance_percent() -> u8 {
    35
}

fn default_spawned_hazard_break_chance_percent() -> u8 {
    35
}

#[derive(Clone, Debug)]
pub struct DatapackSummary {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub primary_scenario: String,
    pub boundary_response: Option<String>,
    pub location_count: usize,
    pub item_count: usize,
    pub enemy_count: usize,
    pub boss_count: usize,
    pub objective_count: usize,
    pub narrator_brief_count: usize,
    pub media_reference_count: usize,
    pub sensory_template_count: usize,
    pub dm_style_preview: Option<String>,
    pub world_tone_preview: Option<String>,
}

#[derive(Clone, Debug)]
pub struct DatapackRecord {
    pub folder_name: String,
    pub summary: DatapackSummary,
}

#[derive(Clone, Debug)]
pub struct DatapackCatalog {
    pub valid: Vec<DatapackRecord>,
    pub invalid: Vec<InvalidDatapack>,
}

#[derive(Clone, Debug)]
pub struct InvalidDatapack {
    pub folder_name: String,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct DatapackBundle {
    pub folder_name: String,
    pub pack: PackToml,
    pub rules: RulesToml,
    pub locations: Vec<LocationTemplate>,
    pub items: Vec<ItemTemplate>,
    pub enemies: Vec<EnemyTemplate>,
    pub bosses: Vec<BossTemplate>,
    pub objectives: Vec<ObjectiveTemplate>,
    pub dm_style: Option<String>,
    pub world_tone: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MediaReferences {
    pub image: Option<String>,
    pub gif: Option<String>,
    pub video: Option<String>,
    pub audio: Option<String>,
    pub display_role: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ResolvedMediaAssetReference {
    pub field_name: String,
    pub relative_path: String,
    pub resolved_path: String,
    pub present: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PackToml {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub primary_scenario: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RulesToml {
    pub scenario_id: String,
    pub starting_location: String,
    pub boundary_mode: String,
    pub boundary_response: Option<String>,
    pub objective_mode: String,
    pub chaos_mode_note: Option<String>,
    #[serde(default = "default_sight_acquire_chance_percent")]
    pub sight_acquire_chance_percent: u8,
    #[serde(default = "default_sight_chase_delay_chance_percent")]
    pub sight_chase_delay_chance_percent: u8,
    #[serde(default = "default_spawned_hazard_break_chance_percent")]
    pub spawned_hazard_break_chance_percent: u8,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LocationTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub epilogue_description: Option<String>,
    #[serde(default)]
    pub epilogue_hook: Option<String>,
    pub narrator_brief: Option<String>,
    pub tags: Vec<String>,
    #[serde(default)]
    pub media: MediaReferences,
    #[serde(default)]
    pub connections: Vec<String>,
    #[serde(default)]
    pub barricadable: bool,
    #[serde(default)]
    pub barricade_item_id: Option<String>,
    #[serde(default)]
    pub barricade_response: Option<String>,
    #[serde(default)]
    pub already_barricaded_response: Option<String>,
    #[serde(default)]
    pub barricade_heal: i32,
    #[serde(default)]
    pub barricade_blocks_retaliation: bool,
    #[serde(default)]
    pub barricade_attack_bonus: i32,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub unlock_item_id: Option<String>,
    #[serde(default)]
    pub locked_response: Option<String>,
    #[serde(default)]
    pub items: Vec<String>,
    #[serde(default)]
    pub enemies: Vec<String>,
    #[serde(default)]
    pub bosses: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ItemTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub narrator_brief: Option<String>,
    pub tags: Vec<String>,
    #[serde(default)]
    pub media: MediaReferences,
    #[serde(default)]
    pub damage: i32,
    #[serde(default)]
    pub utility_effect: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EnemyTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub narrator_brief: Option<String>,
    pub tags: Vec<String>,
    #[serde(default = "default_true")]
    pub can_hear: bool,
    #[serde(default = "default_true")]
    pub can_see: bool,
    #[serde(default)]
    pub media: MediaReferences,
    pub hp: i32,
    pub damage: i32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BossTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub narrator_brief: Option<String>,
    pub tags: Vec<String>,
    #[serde(default = "default_true")]
    pub can_hear: bool,
    #[serde(default = "default_true")]
    pub can_see: bool,
    #[serde(default)]
    pub media: MediaReferences,
    pub hp: i32,
    pub damage: i32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ObjectiveTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub target_boss_id: Option<String>,
    pub required_item_id: Option<String>,
    pub required_location_id: Option<String>,
}

#[derive(Deserialize)]
struct LocationFile {
    locations: Vec<LocationTemplate>,
}

#[derive(Deserialize)]
struct ItemFile {
    items: Vec<ItemTemplate>,
}

#[derive(Deserialize)]
struct EnemyFile {
    enemies: Vec<EnemyTemplate>,
}

#[derive(Deserialize)]
struct BossFile {
    bosses: Vec<BossTemplate>,
}

#[derive(Deserialize)]
struct ObjectiveFile {
    objectives: Vec<ObjectiveTemplate>,
}

pub fn discover_datapacks() -> DatapackCatalog {
    let mut valid = Vec::new();
    let mut invalid = Vec::new();

    let root = app_paths::datapacks_root();
    let Ok(entries) = fs::read_dir(root) else {
        invalid.push(InvalidDatapack {
            folder_name: DATAPACKS_ROOT.to_owned(),
            errors: vec!["Datapack root folder could not be read.".to_owned()],
        });

        return DatapackCatalog { valid, invalid };
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let folder_name = entry.file_name().to_string_lossy().to_string();
        match load_datapack_bundle_from_path(&path, &folder_name) {
            Ok(bundle) => valid.push(DatapackRecord {
                folder_name,
                summary: bundle_to_summary(&bundle),
            }),
            Err(errors) => invalid.push(InvalidDatapack {
                folder_name,
                errors,
            }),
        }
    }

    valid.sort_by(|a, b| a.summary.display_name.cmp(&b.summary.display_name));
    invalid.sort_by(|a, b| a.folder_name.cmp(&b.folder_name));

    DatapackCatalog { valid, invalid }
}

pub fn load_datapack_bundle_by_folder(folder_name: &str) -> Result<DatapackBundle, Vec<String>> {
    let path = app_paths::datapacks_root().join(folder_name);
    load_datapack_bundle_from_path(&path, folder_name)
}

pub fn datapack_schema_version() -> &'static str {
    DATAPACK_SCHEMA_VERSION
}

pub fn resolve_media_path(bundle: &DatapackBundle, relative_path: &str) -> Option<String> {
    let trimmed = relative_path.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(
        app_paths::datapacks_root()
            .join(&bundle.folder_name)
            .join(trimmed)
            .display()
            .to_string(),
    )
}

pub fn resolve_media_assets(
    bundle: &DatapackBundle,
    media: &MediaReferences,
) -> Vec<ResolvedMediaAssetReference> {
    let mut assets = Vec::new();

    for (field_name, relative_path) in [
        ("image", media.image.as_deref()),
        ("gif", media.gif.as_deref()),
        ("video", media.video.as_deref()),
        ("audio", media.audio.as_deref()),
    ] {
        let Some(relative_path) = relative_path
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };

        if let Some(resolved_path) = resolve_media_path(bundle, relative_path) {
            assets.push(ResolvedMediaAssetReference {
                field_name: field_name.to_owned(),
                relative_path: relative_path.to_owned(),
                present: Path::new(&resolved_path).exists(),
                resolved_path,
            });
        }
    }

    assets
}

fn load_datapack_bundle_from_path(
    path: &Path,
    folder_name: &str,
) -> Result<DatapackBundle, Vec<String>> {
    let mut errors = Vec::new();

    let pack_path = path.join("pack.toml");
    let rules_path = path.join("rules.toml");
    let templates_dir = path.join("templates");
    let capsules_dir = path.join("capsules");

    let pack = parse_toml_file::<PackToml>(&pack_path, "pack.toml", &mut errors);
    let rules = parse_toml_file::<RulesToml>(&rules_path, "rules.toml", &mut errors);

    let locations = parse_toml_file::<LocationFile>(
        &templates_dir.join("locations.toml"),
        "templates/locations.toml",
        &mut errors,
    );
    let items = parse_toml_file::<ItemFile>(
        &templates_dir.join("items.toml"),
        "templates/items.toml",
        &mut errors,
    );
    let enemies = parse_toml_file::<EnemyFile>(
        &templates_dir.join("enemies.toml"),
        "templates/enemies.toml",
        &mut errors,
    );
    let bosses = parse_toml_file::<BossFile>(
        &templates_dir.join("bosses.toml"),
        "templates/bosses.toml",
        &mut errors,
    );
    let objectives = parse_toml_file::<ObjectiveFile>(
        &templates_dir.join("objectives.toml"),
        "templates/objectives.toml",
        &mut errors,
    );

    if let (Some(pack), Some(rules)) = (&pack, &rules) {
        if pack.id != rules.scenario_id {
            errors.push(format!(
                "Scenario mismatch: pack id '{}' does not match rules scenario_id '{}'.",
                pack.id, rules.scenario_id
            ));
        }

        if pack.primary_scenario != rules.scenario_id {
            errors.push(format!(
                "Primary scenario mismatch: pack primary_scenario '{}' does not match rules scenario_id '{}'.",
                pack.primary_scenario, rules.scenario_id
            ));
        }

        if rules.boundary_mode.trim().is_empty() {
            errors.push("rules.toml boundary_mode must not be empty.".to_owned());
        }

        if rules.objective_mode.trim().is_empty() {
            errors.push("rules.toml objective_mode must not be empty.".to_owned());
        }

        if let Some(note) = &rules.chaos_mode_note
            && note.trim().is_empty()
        {
            errors.push("rules.toml chaos_mode_note must not be blank when present.".to_owned());
        }

        validate_percent(
            "rules.toml sight_acquire_chance_percent",
            rules.sight_acquire_chance_percent,
            &mut errors,
        );
        validate_percent(
            "rules.toml sight_chase_delay_chance_percent",
            rules.sight_chase_delay_chance_percent,
            &mut errors,
        );
        validate_percent(
            "rules.toml spawned_hazard_break_chance_percent",
            rules.spawned_hazard_break_chance_percent,
            &mut errors,
        );
    }

    if let Some(locations) = &locations {
        validate_unique_ids("locations", &locations.locations, &mut errors);
        validate_non_blank_names("locations", &locations.locations, &mut errors);
        for location in &locations.locations {
            if let Some(epilogue_description) = &location.epilogue_description
                && epilogue_description.trim().is_empty()
            {
                errors.push(format!(
                    "Location '{}' must not define a blank epilogue_description.",
                    location.id
                ));
            }
            if let Some(epilogue_hook) = &location.epilogue_hook
                && epilogue_hook.trim().is_empty()
            {
                errors.push(format!(
                    "Location '{}' must not define a blank epilogue_hook.",
                    location.id
                ));
            }
        }
    }
    if let Some(items) = &items {
        validate_unique_ids("items", &items.items, &mut errors);
        validate_non_blank_names("items", &items.items, &mut errors);
        for item in &items.items {
            if let Some(utility_effect) = item.utility_effect.as_deref() {
                match utility_effect.trim() {
                    "reveal_connections" | "barricade" => {}
                    "" => errors.push(format!(
                        "Item '{}' must not define a blank utility_effect.",
                        item.id
                    )),
                    other => errors.push(format!(
                        "Item '{}' defines unknown utility_effect '{}'.",
                        item.id, other
                    )),
                }
            }
        }
    }
    if let Some(enemies) = &enemies {
        validate_unique_ids("enemies", &enemies.enemies, &mut errors);
        validate_non_blank_names("enemies", &enemies.enemies, &mut errors);
    }
    if let Some(bosses) = &bosses {
        validate_unique_ids("bosses", &bosses.bosses, &mut errors);
        validate_non_blank_names("bosses", &bosses.bosses, &mut errors);
    }
    if let Some(objectives) = &objectives {
        validate_unique_ids("objectives", &objectives.objectives, &mut errors);
        validate_non_blank_names("objectives", &objectives.objectives, &mut errors);
    }

    if let (Some(rules), Some(locations)) = (&rules, &locations) {
        let known_locations: HashSet<&str> = locations
            .locations
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        if !known_locations.contains(rules.starting_location.as_str()) {
            errors.push(format!(
                "rules.toml starting_location '{}' was not found in templates/locations.toml.",
                rules.starting_location
            ));
        }

        for location in &locations.locations {
            for connection in &location.connections {
                if !known_locations.contains(connection.as_str()) {
                    errors.push(format!(
                        "Location '{}' references unknown connection '{}'.",
                        location.id, connection
                    ));
                }
            }
        }
    }

    if let (Some(items), Some(locations)) = (&items, &locations) {
        let known_items: HashSet<&str> =
            items.items.iter().map(|entry| entry.id.as_str()).collect();
        for location in &locations.locations {
            if location.locked {
                let Some(unlock_item_id) = location
                    .unlock_item_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                else {
                    errors.push(format!(
                        "Locked location '{}' must define unlock_item_id.",
                        location.id
                    ));
                    continue;
                };
                if !known_items.contains(unlock_item_id) {
                    errors.push(format!(
                        "Location '{}' references unknown unlock_item_id '{}'.",
                        location.id, unlock_item_id
                    ));
                }
            }
            if location.barricadable {
                let Some(barricade_item_id) = location
                    .barricade_item_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                else {
                    errors.push(format!(
                        "Barricadable location '{}' must define barricade_item_id.",
                        location.id
                    ));
                    continue;
                };
                if !known_items.contains(barricade_item_id) {
                    errors.push(format!(
                        "Location '{}' references unknown barricade_item_id '{}'.",
                        location.id, barricade_item_id
                    ));
                }
            } else {
                for (field_name, value) in [
                    ("barricade_item_id", location.barricade_item_id.as_deref()),
                    ("barricade_response", location.barricade_response.as_deref()),
                    (
                        "already_barricaded_response",
                        location.already_barricaded_response.as_deref(),
                    ),
                ] {
                    if value.is_some() {
                        errors.push(format!(
                            "Location '{}' defines {} but is not barricadable.",
                            location.id, field_name
                        ));
                    }
                }
            }
            if let Some(response) = &location.barricade_response
                && response.trim().is_empty()
            {
                errors.push(format!(
                    "Location '{}' must not define a blank barricade_response.",
                    location.id
                ));
            }
            if let Some(response) = &location.already_barricaded_response
                && response.trim().is_empty()
            {
                errors.push(format!(
                    "Location '{}' must not define a blank already_barricaded_response.",
                    location.id
                ));
            }
            if location.barricade_heal < 0 {
                errors.push(format!(
                    "Location '{}' must not define a negative barricade_heal.",
                    location.id
                ));
            }
            if location.barricade_attack_bonus < 0 {
                errors.push(format!(
                    "Location '{}' must not define a negative barricade_attack_bonus.",
                    location.id
                ));
            }
            for item_id in &location.items {
                if !known_items.contains(item_id.as_str()) {
                    errors.push(format!(
                        "Location '{}' references unknown item '{}'.",
                        location.id, item_id
                    ));
                }
            }
        }
    }

    if let (Some(enemies), Some(locations)) = (&enemies, &locations) {
        let known_enemies: HashSet<&str> = enemies
            .enemies
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        for location in &locations.locations {
            for enemy_id in &location.enemies {
                if !known_enemies.contains(enemy_id.as_str()) {
                    errors.push(format!(
                        "Location '{}' references unknown enemy '{}'.",
                        location.id, enemy_id
                    ));
                }
            }
        }
    }

    if let (Some(bosses), Some(locations)) = (&bosses, &locations) {
        let known_bosses: HashSet<&str> = bosses
            .bosses
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        for location in &locations.locations {
            for boss_id in &location.bosses {
                if !known_bosses.contains(boss_id.as_str()) {
                    errors.push(format!(
                        "Location '{}' references unknown boss '{}'.",
                        location.id, boss_id
                    ));
                }
            }
        }
    }

    if let (Some(objectives), Some(items), Some(bosses), Some(locations)) =
        (&objectives, &items, &bosses, &locations)
    {
        let known_items: HashSet<&str> =
            items.items.iter().map(|entry| entry.id.as_str()).collect();
        let known_bosses: HashSet<&str> = bosses
            .bosses
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        let known_locations: HashSet<&str> = locations
            .locations
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        for objective in &objectives.objectives {
            let target_boss_id = objective
                .target_boss_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let required_item_id = objective
                .required_item_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let required_location_id = objective
                .required_location_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty());

            if target_boss_id.is_none() && objective.target_boss_id.is_some() {
                errors.push(format!(
                    "Objective '{}' must not define a blank target_boss_id.",
                    objective.id
                ));
            }
            if required_item_id.is_none() && objective.required_item_id.is_some() {
                errors.push(format!(
                    "Objective '{}' must not define a blank required_item_id.",
                    objective.id
                ));
            }
            if required_location_id.is_none() && objective.required_location_id.is_some() {
                errors.push(format!(
                    "Objective '{}' must not define a blank required_location_id.",
                    objective.id
                ));
            }
            if target_boss_id.is_none()
                && required_item_id.is_none()
                && required_location_id.is_none()
            {
                errors.push(format!(
                    "Objective '{}' must define at least one completion condition field.",
                    objective.id
                ));
            }
            if let Some(target_boss_id) = target_boss_id
                && !known_bosses.contains(target_boss_id)
            {
                errors.push(format!(
                    "Objective '{}' references unknown target boss '{}'.",
                    objective.id, target_boss_id
                ));
            }
            if let Some(required_item_id) = required_item_id
                && !known_items.contains(required_item_id)
            {
                errors.push(format!(
                    "Objective '{}' references unknown required item '{}'.",
                    objective.id, required_item_id
                ));
            }
            if let Some(required_location_id) = required_location_id
                && !known_locations.contains(required_location_id)
            {
                errors.push(format!(
                    "Objective '{}' references unknown required location '{}'.",
                    objective.id, required_location_id
                ));
            }
        }
    }

    if let Some(objectives) = &objectives
        && objectives.objectives.is_empty()
    {
        errors.push("templates/objectives.toml must define at least one objective.".to_owned());
    }

    if let Some(locations) = &locations
        && locations.locations.is_empty()
    {
        errors.push("templates/locations.toml must define at least one location.".to_owned());
    }

    if let Some(items) = &items
        && items.items.is_empty()
    {
        errors.push("templates/items.toml must define at least one item.".to_owned());
    }

    if let Some(enemies) = &enemies
        && enemies.enemies.is_empty()
    {
        errors.push("templates/enemies.toml must define at least one enemy.".to_owned());
    }

    if let Some(bosses) = &bosses
        && bosses.bosses.is_empty()
    {
        errors.push("templates/bosses.toml must define at least one boss.".to_owned());
    }

    let dm_style = read_optional_text_preview(&capsules_dir.join("dm_style.txt"), &mut errors);
    let world_tone = read_optional_text_preview(&capsules_dir.join("world_tone.txt"), &mut errors);

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(DatapackBundle {
        folder_name: folder_name.to_owned(),
        pack: pack.expect("pack validated"),
        rules: rules.expect("rules validated"),
        locations: locations.expect("locations validated").locations,
        items: items.expect("items validated").items,
        enemies: enemies.expect("enemies validated").enemies,
        bosses: bosses.expect("bosses validated").bosses,
        objectives: objectives.expect("objectives validated").objectives,
        dm_style,
        world_tone,
    })
}

fn bundle_to_summary(bundle: &DatapackBundle) -> DatapackSummary {
    DatapackSummary {
        id: bundle.pack.id.clone(),
        display_name: bundle.pack.display_name.clone(),
        version: bundle.pack.version.clone(),
        author: bundle.pack.author.clone(),
        description: bundle.pack.description.clone(),
        primary_scenario: bundle.pack.primary_scenario.clone(),
        boundary_response: bundle.rules.boundary_response.clone(),
        location_count: bundle.locations.len(),
        item_count: bundle.items.len(),
        enemy_count: bundle.enemies.len(),
        boss_count: bundle.bosses.len(),
        objective_count: bundle.objectives.len(),
        narrator_brief_count: bundle
            .locations
            .iter()
            .filter(|entry| entry.narrator_brief.is_some())
            .count()
            + bundle
                .items
                .iter()
                .filter(|entry| entry.narrator_brief.is_some())
                .count()
            + bundle
                .enemies
                .iter()
                .filter(|entry| entry.narrator_brief.is_some())
                .count()
            + bundle
                .bosses
                .iter()
                .filter(|entry| entry.narrator_brief.is_some())
                .count(),
        media_reference_count: bundle
            .locations
            .iter()
            .map(|entry| count_media_references(&entry.media))
            .sum::<usize>()
            + bundle
                .items
                .iter()
                .map(|entry| count_media_references(&entry.media))
                .sum::<usize>()
            + bundle
                .enemies
                .iter()
                .map(|entry| count_media_references(&entry.media))
                .sum::<usize>()
            + bundle
                .bosses
                .iter()
                .map(|entry| count_media_references(&entry.media))
                .sum::<usize>(),
        sensory_template_count: bundle
            .enemies
            .iter()
            .filter(|entry| entry.can_hear || entry.can_see)
            .count()
            + bundle
                .bosses
                .iter()
                .filter(|entry| entry.can_hear || entry.can_see)
                .count(),
        dm_style_preview: bundle.dm_style.clone(),
        world_tone_preview: bundle.world_tone.clone(),
    }
}

fn count_media_references(media: &MediaReferences) -> usize {
    [
        media.image.as_deref(),
        media.gif.as_deref(),
        media.video.as_deref(),
        media.audio.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter(|value| !value.trim().is_empty())
    .count()
}

fn parse_toml_file<T>(path: &PathBuf, label: &str, errors: &mut Vec<String>) -> Option<T>
where
    T: for<'de> Deserialize<'de>,
{
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => {
            errors.push(format!("Missing or unreadable required file: {}.", label));
            return None;
        }
    };

    match toml::from_str::<T>(&content) {
        Ok(parsed) => Some(parsed),
        Err(err) => {
            errors.push(format!("Failed to parse {}: {}.", label, err));
            None
        }
    }
}

trait HasTemplateIdentity {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
}

impl HasTemplateIdentity for LocationTemplate {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl HasTemplateIdentity for ItemTemplate {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl HasTemplateIdentity for EnemyTemplate {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl HasTemplateIdentity for BossTemplate {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl HasTemplateIdentity for ObjectiveTemplate {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }
}

fn validate_unique_ids<T: HasTemplateIdentity>(
    kind: &str,
    entries: &[T],
    errors: &mut Vec<String>,
) {
    let mut seen = HashSet::new();
    for entry in entries {
        if entry.id().trim().is_empty() {
            errors.push(format!("{} template id must not be blank.", kind));
        } else if !seen.insert(entry.id()) {
            errors.push(format!("Duplicate {} template id '{}'.", kind, entry.id()));
        }
    }
}

fn validate_non_blank_names<T: HasTemplateIdentity>(
    kind: &str,
    entries: &[T],
    errors: &mut Vec<String>,
) {
    for entry in entries {
        if entry.name().trim().is_empty() {
            errors.push(format!(
                "{} template '{}' must not have a blank name.",
                kind,
                entry.id()
            ));
        }
    }
}

fn validate_percent(label: &str, value: u8, errors: &mut Vec<String>) {
    if value > 100 {
        errors.push(format!("{} must be between 0 and 100.", label));
    }
}

fn read_optional_text_preview(path: &Path, errors: &mut Vec<String>) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(content) => {
            let trimmed = content.trim();
            if trimmed.is_empty() {
                errors.push(format!(
                    "Optional capsule file '{}' exists but is blank.",
                    path.display()
                ));
                None
            } else {
                Some(trimmed.to_owned())
            }
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        discover_datapacks, load_datapack_bundle_by_folder, load_datapack_bundle_from_path,
    };

    struct TempDatapackDir {
        path: PathBuf,
    }

    impl TempDatapackDir {
        fn new(name: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("chatty_quest_{}_{}", name, unique));
            fs::create_dir_all(path.join("templates")).expect("expected templates dir");
            Self { path }
        }

        fn write_file(&self, relative_path: &str, content: &str) {
            let path = self.path.join(relative_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("expected parent dir");
            }
            fs::write(path, content).expect("expected file write");
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDatapackDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn write_minimal_test_datapack(dir: &TempDatapackDir, objectives_toml: &str) {
        dir.write_file(
            "pack.toml",
            r#"id = "test_pack"
display_name = "Test Pack"
version = "0.0.1"
author = "tests"
description = "temp pack"
primary_scenario = "test_pack"
"#,
        );
        dir.write_file(
            "rules.toml",
            r#"scenario_id = "test_pack"
starting_location = "start"
boundary_mode = "hard"
boundary_response = "no"
objective_mode = "single"
"#,
        );
        dir.write_file(
            "templates/locations.toml",
            r#"[[locations]]
id = "start"
name = "Start"
description = "Start room."
tags = ["start"]
connections = []
items = []
enemies = []
bosses = ["test_boss"]
"#,
        );
        dir.write_file(
            "templates/items.toml",
            r#"[[items]]
id = "real_key"
name = "Real Key"
description = "A real key."
tags = ["utility"]
"#,
        );
        dir.write_file(
            "templates/enemies.toml",
            r#"[[enemies]]
id = "test_enemy"
name = "Test Enemy"
description = "An enemy."
tags = ["melee"]
hp = 1
damage = 1
"#,
        );
        dir.write_file(
            "templates/bosses.toml",
            r#"[[bosses]]
id = "test_boss"
name = "Test Boss"
description = "A boss."
tags = ["boss"]
hp = 1
damage = 1
"#,
        );
        dir.write_file("templates/objectives.toml", objectives_toml);
    }

    #[test]
    fn property_siege_classic_is_discoverable_from_assets() {
        let catalog = discover_datapacks();
        let record = catalog
            .valid
            .iter()
            .find(|record| record.folder_name == "property_siege_classic")
            .expect("expected property_siege_classic datapack to be discoverable");

        assert_eq!(record.summary.display_name, "Property Siege Classic");
        assert_eq!(record.summary.location_count, 5);
        assert_eq!(record.summary.item_count, 5);
        assert_eq!(record.summary.enemy_count, 2);
        assert_eq!(record.summary.boss_count, 1);
        assert_eq!(record.summary.objective_count, 1);
        assert_eq!(record.summary.sensory_template_count, 3);
    }

    #[test]
    fn property_siege_classic_bundle_loads_with_expected_templates() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");

        assert_eq!(bundle.pack.id, "property_siege_classic");
        assert_eq!(bundle.rules.starting_location, "front_verandah");
        assert_eq!(bundle.rules.sight_acquire_chance_percent, 70);
        assert_eq!(bundle.rules.sight_chase_delay_chance_percent, 35);
        assert_eq!(bundle.rules.spawned_hazard_break_chance_percent, 35);
        assert!(
            bundle
                .locations
                .iter()
                .any(|location| location.id == "garage")
        );
        assert!(bundle.items.iter().any(|item| item.id == "cricket_bat"));
        assert!(bundle.items.iter().any(|item| item.id == "barricade_kit"));
        let shambler = bundle
            .enemies
            .iter()
            .find(|enemy| enemy.id == "shambler_front_gate")
            .expect("expected shambler template");
        assert!(shambler.can_hear);
        assert!(shambler.can_see);
        assert_eq!(
            bundle
                .items
                .iter()
                .find(|item| item.id == "torch")
                .and_then(|item| item.utility_effect.as_deref()),
            Some("reveal_connections")
        );
        let garage_brute = bundle
            .bosses
            .iter()
            .find(|boss| boss.id == "brute_in_garage")
            .expect("expected garage brute template");
        assert!(garage_brute.can_hear);
        assert!(garage_brute.can_see);
        assert_eq!(
            bundle.objectives[0].target_boss_id.as_deref(),
            Some("brute_in_garage")
        );
        assert_eq!(
            bundle.objectives[0].required_item_id.as_deref(),
            Some("house_keys")
        );
        assert_eq!(
            bundle.objectives[0].required_location_id.as_deref(),
            Some("garage")
        );
        let garage = bundle
            .locations
            .iter()
            .find(|location| location.id == "garage")
            .expect("expected garage template");
        assert!(
            garage
                .epilogue_description
                .as_deref()
                .is_some_and(|description| description.contains("gives up being an arena"))
        );
        assert!(
            garage
                .epilogue_hook
                .as_deref()
                .is_some_and(|hook| hook.contains("end-card media"))
        );
        assert!(garage.locked);
        assert_eq!(garage.unlock_item_id.as_deref(), Some("house_keys"));
        let front_verandah = bundle
            .locations
            .iter()
            .find(|location| location.id == "front_verandah")
            .expect("expected front verandah template");
        assert!(front_verandah.barricadable);
        assert_eq!(
            front_verandah.barricade_item_id.as_deref(),
            Some("barricade_kit")
        );
        assert!(front_verandah.barricade_blocks_retaliation);
        assert_eq!(front_verandah.barricade_attack_bonus, 1);
        let back_garden = bundle
            .locations
            .iter()
            .find(|location| location.id == "back_garden")
            .expect("expected back garden template");
        assert!(back_garden.locked);
        assert_eq!(back_garden.unlock_item_id.as_deref(), Some("house_keys"));
        assert!(back_garden.barricadable);
        assert_eq!(
            back_garden.barricade_item_id.as_deref(),
            Some("barricade_kit")
        );
        assert_eq!(back_garden.barricade_heal, 2);
        assert!(!back_garden.barricade_blocks_retaliation);
        assert_eq!(back_garden.barricade_attack_bonus, 0);
        assert_eq!(back_garden.items, vec!["barricade_kit".to_owned()]);
    }

    #[test]
    fn objective_validation_rejects_missing_completion_fields() {
        let dir = TempDatapackDir::new("objective_missing_conditions");
        write_minimal_test_datapack(
            &dir,
            r#"[[objectives]]
id = "test_objective"
name = "Test Objective"
description = "No conditions."
tags = ["test"]
"#,
        );

        let errors = load_datapack_bundle_from_path(dir.path(), "temp_test_pack")
            .expect_err("expected datapack validation to fail");

        assert!(errors.iter().any(|error| {
            error.contains("must define at least one completion condition field")
        }));
    }

    #[test]
    fn objective_validation_rejects_unknown_required_item_id() {
        let dir = TempDatapackDir::new("objective_unknown_item");
        write_minimal_test_datapack(
            &dir,
            r#"[[objectives]]
id = "test_objective"
name = "Test Objective"
description = "Bad required item."
tags = ["test"]
required_item_id = "missing_key"
target_boss_id = "test_boss"
"#,
        );

        let errors = load_datapack_bundle_from_path(dir.path(), "temp_test_pack")
            .expect_err("expected datapack validation to fail");

        assert!(
            errors
                .iter()
                .any(|error| { error.contains("references unknown required item 'missing_key'") })
        );
    }

    #[test]
    fn objective_validation_rejects_unknown_required_location_id() {
        let dir = TempDatapackDir::new("objective_unknown_location");
        write_minimal_test_datapack(
            &dir,
            r#"[[objectives]]
id = "test_objective"
name = "Test Objective"
description = "Bad required location."
tags = ["test"]
required_location_id = "missing_room"
target_boss_id = "test_boss"
"#,
        );

        let errors = load_datapack_bundle_from_path(dir.path(), "temp_test_pack")
            .expect_err("expected datapack validation to fail");

        assert!(errors.iter().any(|error| {
            error.contains("references unknown required location 'missing_room'")
        }));
    }

    #[test]
    fn item_validation_rejects_unknown_utility_effect() {
        let dir = TempDatapackDir::new("item_unknown_utility");
        write_minimal_test_datapack(
            &dir,
            r#"[[objectives]]
id = "test_objective"
name = "Test Objective"
description = "Valid objective."
tags = ["test"]
target_boss_id = "test_boss"
"#,
        );
        dir.write_file(
            "templates/items.toml",
            r#"[[items]]
id = "real_key"
name = "Real Key"
description = "A real key."
tags = ["utility"]
utility_effect = "unknown_effect"
"#,
        );

        let errors = load_datapack_bundle_from_path(dir.path(), "temp_test_pack")
            .expect_err("expected datapack validation to fail");

        assert!(
            errors
                .iter()
                .any(|error| { error.contains("defines unknown utility_effect 'unknown_effect'") })
        );
    }

    #[test]
    fn location_validation_rejects_unknown_barricade_item_id() {
        let dir = TempDatapackDir::new("location_unknown_barricade_item");
        write_minimal_test_datapack(
            &dir,
            r#"[[objectives]]
id = "test_objective"
name = "Valid Objective"
description = "Valid objective."
tags = ["test"]
target_boss_id = "test_boss"
"#,
        );
        dir.write_file(
            "templates/locations.toml",
            r#"[[locations]]
id = "start"
name = "Start"
description = "Start room."
tags = ["start"]
connections = []
barricadable = true
barricade_item_id = "missing_kit"
items = []
enemies = []
bosses = ["test_boss"]
"#,
        );

        let errors = load_datapack_bundle_from_path(dir.path(), "temp_test_pack")
            .expect_err("expected datapack validation to fail");

        assert!(
            errors.iter().any(|error| {
                error.contains("references unknown barricade_item_id 'missing_kit'")
            })
        );
    }

    #[test]
    fn location_validation_rejects_blank_epilogue_description() {
        let dir = TempDatapackDir::new("location_blank_epilogue");
        write_minimal_test_datapack(
            &dir,
            r#"[[objectives]]
id = "test_objective"
name = "Valid Objective"
description = "Valid objective."
tags = ["test"]
target_boss_id = "test_boss"
"#,
        );
        dir.write_file(
            "templates/locations.toml",
            r#"[[locations]]
id = "start"
name = "Start"
description = "Start room."
epilogue_description = "   "
tags = ["start"]
connections = []
items = []
enemies = []
bosses = ["test_boss"]
"#,
        );

        let errors = load_datapack_bundle_from_path(dir.path(), "temp_test_pack")
            .expect_err("expected datapack validation to fail");

        assert!(
            errors
                .iter()
                .any(|error| { error.contains("must not define a blank epilogue_description") })
        );
    }

    #[test]
    fn location_validation_rejects_blank_epilogue_hook() {
        let dir = TempDatapackDir::new("location_blank_epilogue_hook");
        write_minimal_test_datapack(
            &dir,
            r#"[[objectives]]
id = "test_objective"
name = "Valid Objective"
description = "Valid objective."
tags = ["test"]
target_boss_id = "test_boss"
"#,
        );
        dir.write_file(
            "templates/locations.toml",
            r#"[[locations]]
id = "start"
name = "Start"
description = "Start room."
epilogue_hook = "   "
tags = ["start"]
connections = []
items = []
enemies = []
bosses = ["test_boss"]
"#,
        );

        let errors = load_datapack_bundle_from_path(dir.path(), "temp_test_pack")
            .expect_err("expected datapack validation to fail");

        assert!(
            errors
                .iter()
                .any(|error| { error.contains("must not define a blank epilogue_hook") })
        );
    }
}
