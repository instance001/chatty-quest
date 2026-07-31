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

fn default_spawned_enemy_movement_policy() -> String {
    "random".to_owned()
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
    #[serde(default = "default_spawned_enemy_movement_policy")]
    pub spawned_enemy_movement_policy: String,
    #[serde(default)]
    pub starter_hint_line: Option<String>,
    #[serde(default)]
    pub finale_target_location_id: Option<String>,
    #[serde(default)]
    pub finale_boss_id: Option<String>,
    #[serde(default)]
    pub finale_secured_location_ids: Vec<String>,
    #[serde(default)]
    pub finale_secured_retaliation_reduction: i32,
    #[serde(default)]
    pub finale_security_secured_line: Option<String>,
    #[serde(default)]
    pub finale_security_unsecured_line: Option<String>,
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
    pub route_note: Option<String>,
    #[serde(default)]
    pub threat_forecast_locked: Option<String>,
    #[serde(default)]
    pub threat_forecast_cleared: Option<String>,
    #[serde(default)]
    pub threat_forecast_barricaded: Option<String>,
    #[serde(default)]
    pub threat_forecast_open: Option<String>,
    #[serde(default)]
    pub threat_forecast_boss_live: Option<String>,
    #[serde(default)]
    pub threat_forecast_boss_secured: Option<String>,
    #[serde(default)]
    pub movement_context_lines: Vec<String>,
    #[serde(default)]
    pub movement_context_secured_line: Option<String>,
    #[serde(default)]
    pub boss_defeated_objective_line: Option<String>,
    #[serde(default)]
    pub boss_retaliation_context_line: Option<String>,
    #[serde(default)]
    pub situation_enemy_cleared_line: Option<String>,
    #[serde(default)]
    pub situation_barricaded_line: Option<String>,
    #[serde(default)]
    pub situation_high_noise_line: Option<String>,
    #[serde(default)]
    pub situation_boss_wounded_secured_line: Option<String>,
    #[serde(default)]
    pub situation_boss_wounded_line: Option<String>,
    #[serde(default)]
    pub situation_boss_secured_line: Option<String>,
    #[serde(default)]
    pub situation_boss_partially_secured_line: Option<String>,
    #[serde(default)]
    pub passive_pressure_enemy_id: Option<String>,
    #[serde(default)]
    pub passive_pressure_blocked_line: Option<String>,
    #[serde(default)]
    pub passive_pressure_damage_line: Option<String>,
    #[serde(default)]
    pub passive_pressure_high_noise_line: Option<String>,
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
    #[serde(default)]
    pub inspect_lines: Vec<String>,
    #[serde(default)]
    pub objective_required_line: Option<String>,
    #[serde(default)]
    pub objective_not_required_line: Option<String>,
    #[serde(default)]
    pub pickup_line: Option<String>,
    #[serde(default)]
    pub utility_success_line: Option<String>,
    #[serde(default)]
    pub utility_empty_line: Option<String>,
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
    #[serde(default)]
    pub retaliation_line: Option<String>,
    #[serde(default)]
    pub defeat_line: Option<String>,
    #[serde(default)]
    pub inspect_alive_line: Option<String>,
    #[serde(default)]
    pub inspect_defeated_line: Option<String>,
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
    #[serde(default)]
    pub wounded_phase_hp_threshold: Option<i32>,
    #[serde(default)]
    pub wounded_phase_damage_bonus: i32,
    #[serde(default)]
    pub defeat_line: Option<String>,
    #[serde(default)]
    pub retaliation_line: Option<String>,
    #[serde(default)]
    pub finale_security_retaliation_line: Option<String>,
    #[serde(default)]
    pub wounded_phase_combat_line: Option<String>,
    #[serde(default)]
    pub wounded_phase_retaliation_line: Option<String>,
    #[serde(default)]
    pub wounded_phase_inspect_active: Option<String>,
    #[serde(default)]
    pub wounded_phase_inspect_pending: Option<String>,
    #[serde(default)]
    pub wounded_phase_inspect_defeated: Option<String>,
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
        match rules.spawned_enemy_movement_policy.trim() {
            "random" | "path_to_attractor" => {}
            "" => errors
                .push("rules.toml spawned_enemy_movement_policy must not be blank.".to_owned()),
            other => errors.push(format!(
                "rules.toml spawned_enemy_movement_policy '{}' is not supported.",
                other
            )),
        }

        for (field_name, value) in [
            ("starter_hint_line", rules.starter_hint_line.as_deref()),
            (
                "finale_target_location_id",
                rules.finale_target_location_id.as_deref(),
            ),
            ("finale_boss_id", rules.finale_boss_id.as_deref()),
            (
                "finale_security_secured_line",
                rules.finale_security_secured_line.as_deref(),
            ),
            (
                "finale_security_unsecured_line",
                rules.finale_security_unsecured_line.as_deref(),
            ),
        ] {
            if let Some(value) = value
                && value.trim().is_empty()
            {
                errors.push(format!(
                    "rules.toml {} must not be blank when present.",
                    field_name
                ));
            }
        }

        for location_id in &rules.finale_secured_location_ids {
            if location_id.trim().is_empty() {
                errors.push(
                    "rules.toml finale_secured_location_ids must not contain blank ids.".to_owned(),
                );
            }
        }

        if rules.finale_secured_retaliation_reduction < 0 {
            errors.push(
                "rules.toml finale_secured_retaliation_reduction must not be negative.".to_owned(),
            );
        }
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
            for line in &location.movement_context_lines {
                if line.trim().is_empty() {
                    errors.push(format!(
                        "Location '{}' must not define blank movement_context_lines entries.",
                        location.id
                    ));
                }
            }
            for (field_name, value) in [
                ("route_note", location.route_note.as_deref()),
                (
                    "threat_forecast_locked",
                    location.threat_forecast_locked.as_deref(),
                ),
                (
                    "threat_forecast_cleared",
                    location.threat_forecast_cleared.as_deref(),
                ),
                (
                    "threat_forecast_barricaded",
                    location.threat_forecast_barricaded.as_deref(),
                ),
                (
                    "threat_forecast_open",
                    location.threat_forecast_open.as_deref(),
                ),
                (
                    "threat_forecast_boss_live",
                    location.threat_forecast_boss_live.as_deref(),
                ),
                (
                    "threat_forecast_boss_secured",
                    location.threat_forecast_boss_secured.as_deref(),
                ),
                (
                    "movement_context_secured_line",
                    location.movement_context_secured_line.as_deref(),
                ),
                (
                    "boss_defeated_objective_line",
                    location.boss_defeated_objective_line.as_deref(),
                ),
                (
                    "boss_retaliation_context_line",
                    location.boss_retaliation_context_line.as_deref(),
                ),
                (
                    "situation_enemy_cleared_line",
                    location.situation_enemy_cleared_line.as_deref(),
                ),
                (
                    "situation_barricaded_line",
                    location.situation_barricaded_line.as_deref(),
                ),
                (
                    "situation_high_noise_line",
                    location.situation_high_noise_line.as_deref(),
                ),
                (
                    "situation_boss_wounded_secured_line",
                    location.situation_boss_wounded_secured_line.as_deref(),
                ),
                (
                    "situation_boss_wounded_line",
                    location.situation_boss_wounded_line.as_deref(),
                ),
                (
                    "situation_boss_secured_line",
                    location.situation_boss_secured_line.as_deref(),
                ),
                (
                    "situation_boss_partially_secured_line",
                    location.situation_boss_partially_secured_line.as_deref(),
                ),
                (
                    "passive_pressure_enemy_id",
                    location.passive_pressure_enemy_id.as_deref(),
                ),
                (
                    "passive_pressure_blocked_line",
                    location.passive_pressure_blocked_line.as_deref(),
                ),
                (
                    "passive_pressure_damage_line",
                    location.passive_pressure_damage_line.as_deref(),
                ),
                (
                    "passive_pressure_high_noise_line",
                    location.passive_pressure_high_noise_line.as_deref(),
                ),
            ] {
                if let Some(value) = value
                    && value.trim().is_empty()
                {
                    errors.push(format!(
                        "Location '{}' must not define a blank {}.",
                        location.id, field_name
                    ));
                }
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
            for line in &item.inspect_lines {
                if line.trim().is_empty() {
                    errors.push(format!(
                        "Item '{}' must not define blank inspect_lines entries.",
                        item.id
                    ));
                }
            }
            for (field_name, value) in [
                (
                    "objective_required_line",
                    item.objective_required_line.as_deref(),
                ),
                (
                    "objective_not_required_line",
                    item.objective_not_required_line.as_deref(),
                ),
                ("pickup_line", item.pickup_line.as_deref()),
                ("utility_success_line", item.utility_success_line.as_deref()),
                ("utility_empty_line", item.utility_empty_line.as_deref()),
            ] {
                if let Some(value) = value
                    && value.trim().is_empty()
                {
                    errors.push(format!(
                        "Item '{}' must not define a blank {}.",
                        item.id, field_name
                    ));
                }
            }
        }
    }
    if let Some(enemies) = &enemies {
        validate_unique_ids("enemies", &enemies.enemies, &mut errors);
        validate_non_blank_names("enemies", &enemies.enemies, &mut errors);
        for enemy in &enemies.enemies {
            for (field_name, value) in [
                ("retaliation_line", enemy.retaliation_line.as_deref()),
                ("defeat_line", enemy.defeat_line.as_deref()),
                ("inspect_alive_line", enemy.inspect_alive_line.as_deref()),
                (
                    "inspect_defeated_line",
                    enemy.inspect_defeated_line.as_deref(),
                ),
            ] {
                if let Some(value) = value
                    && value.trim().is_empty()
                {
                    errors.push(format!(
                        "Enemy '{}' must not define a blank {}.",
                        enemy.id, field_name
                    ));
                }
            }
        }
    }
    if let Some(bosses) = &bosses {
        validate_unique_ids("bosses", &bosses.bosses, &mut errors);
        validate_non_blank_names("bosses", &bosses.bosses, &mut errors);
        for boss in &bosses.bosses {
            if let Some(threshold) = boss.wounded_phase_hp_threshold {
                if threshold <= 0 {
                    errors.push(format!(
                        "Boss '{}' wounded_phase_hp_threshold must be greater than 0.",
                        boss.id
                    ));
                }
                if threshold >= boss.hp {
                    errors.push(format!(
                        "Boss '{}' wounded_phase_hp_threshold must be lower than boss hp.",
                        boss.id
                    ));
                }
            }
            if boss.wounded_phase_damage_bonus < 0 {
                errors.push(format!(
                    "Boss '{}' wounded_phase_damage_bonus must not be negative.",
                    boss.id
                ));
            }
            for (field_name, value) in [
                ("defeat_line", boss.defeat_line.as_deref()),
                ("retaliation_line", boss.retaliation_line.as_deref()),
                (
                    "finale_security_retaliation_line",
                    boss.finale_security_retaliation_line.as_deref(),
                ),
                (
                    "wounded_phase_combat_line",
                    boss.wounded_phase_combat_line.as_deref(),
                ),
                (
                    "wounded_phase_retaliation_line",
                    boss.wounded_phase_retaliation_line.as_deref(),
                ),
                (
                    "wounded_phase_inspect_active",
                    boss.wounded_phase_inspect_active.as_deref(),
                ),
                (
                    "wounded_phase_inspect_pending",
                    boss.wounded_phase_inspect_pending.as_deref(),
                ),
                (
                    "wounded_phase_inspect_defeated",
                    boss.wounded_phase_inspect_defeated.as_deref(),
                ),
            ] {
                if let Some(value) = value
                    && value.trim().is_empty()
                {
                    errors.push(format!(
                        "Boss '{}' must not define a blank {}.",
                        boss.id, field_name
                    ));
                }
            }
        }
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

        if let Some(location_id) = rules
            .finale_target_location_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            && !known_locations.contains(location_id)
        {
            errors.push(format!(
                "rules.toml finale_target_location_id '{}' was not found in templates/locations.toml.",
                location_id
            ));
        }

        for location_id in &rules.finale_secured_location_ids {
            let location_id = location_id.trim();
            if !location_id.is_empty() && !known_locations.contains(location_id) {
                errors.push(format!(
                    "rules.toml finale_secured_location_ids references unknown location '{}'.",
                    location_id
                ));
            }
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
            if let Some(enemy_id) = location
                .passive_pressure_enemy_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                && !known_enemies.contains(enemy_id)
            {
                errors.push(format!(
                    "Location '{}' references unknown passive_pressure_enemy_id '{}'.",
                    location.id, enemy_id
                ));
            }
        }
    }

    if let (Some(bosses), Some(locations)) = (&bosses, &locations) {
        let known_bosses: HashSet<&str> = bosses
            .bosses
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        if let Some(rules) = &rules
            && let Some(boss_id) = rules
                .finale_boss_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            && !known_bosses.contains(boss_id)
        {
            errors.push(format!(
                "rules.toml finale_boss_id '{}' was not found in templates/bosses.toml.",
                boss_id
            ));
        }

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
    fn station_smoke_test_is_discoverable_and_loads_with_distinct_ids() {
        let catalog = discover_datapacks();
        let record = catalog
            .valid
            .iter()
            .find(|record| record.folder_name == "station_smoke_test")
            .expect("expected station_smoke_test datapack to be discoverable");

        assert_eq!(record.summary.display_name, "Station Smoke Test");
        assert_eq!(record.summary.location_count, 2);
        assert_eq!(record.summary.item_count, 1);
        assert_eq!(record.summary.enemy_count, 1);
        assert_eq!(record.summary.boss_count, 1);
        assert_eq!(record.summary.objective_count, 1);
        assert_eq!(record.summary.media_reference_count, 0);

        let bundle = load_datapack_bundle_by_folder("station_smoke_test")
            .expect("expected station_smoke_test bundle to load");
        assert_eq!(bundle.pack.id, "station_smoke_test");
        assert_eq!(bundle.rules.starting_location, "station_platform");
        assert_eq!(bundle.rules.spawned_enemy_movement_policy, "random");
        assert_eq!(
            bundle.objectives[0].required_location_id.as_deref(),
            Some("signal_box")
        );
        assert!(bundle.items.iter().any(|item| item.id == "brass_token"));
        assert!(
            bundle
                .enemies
                .iter()
                .any(|enemy| enemy.id == "static_guard" && !enemy.can_hear)
        );
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
        assert_eq!(
            bundle.rules.spawned_enemy_movement_policy,
            "path_to_attractor"
        );
        assert!(
            bundle
                .rules
                .starter_hint_line
                .as_deref()
                .is_some_and(|line| line.contains("go garage"))
        );
        assert_eq!(
            bundle.rules.finale_target_location_id.as_deref(),
            Some("garage")
        );
        assert_eq!(
            bundle.rules.finale_boss_id.as_deref(),
            Some("brute_in_garage")
        );
        assert_eq!(
            bundle.rules.finale_secured_location_ids,
            vec!["front_verandah".to_owned(), "back_garden".to_owned()]
        );
        assert_eq!(bundle.rules.finale_secured_retaliation_reduction, 1);
        assert!(
            bundle
                .rules
                .finale_security_secured_line
                .as_deref()
                .is_some_and(|line| line.contains("garage retaliation"))
        );
        assert!(
            bundle
                .rules
                .finale_security_unsecured_line
                .as_deref()
                .is_some_and(|line| line.contains("{required_locations}"))
        );
        assert!(
            bundle
                .locations
                .iter()
                .any(|location| location.id == "garage")
        );
        assert!(bundle.items.iter().any(|item| item.id == "cricket_bat"));
        assert!(bundle.items.iter().any(|item| item.id == "barricade_kit"));
        let house_keys = bundle
            .items
            .iter()
            .find(|item| item.id == "house_keys")
            .expect("expected house keys template");
        assert!(
            house_keys
                .inspect_lines
                .iter()
                .any(|line| line.contains("opens the garage"))
        );
        assert!(
            house_keys
                .objective_required_line
                .as_deref()
                .is_some_and(|line| line.contains("still be holding them"))
        );
        assert!(
            house_keys
                .pickup_line
                .as_deref()
                .is_some_and(|line| line.contains("much heavier"))
        );
        let shambler = bundle
            .enemies
            .iter()
            .find(|enemy| enemy.id == "shambler_front_gate")
            .expect("expected shambler template");
        assert!(shambler.can_hear);
        assert!(shambler.can_see);
        assert!(
            shambler
                .retaliation_line
                .as_deref()
                .is_some_and(|line| line.contains("full dead weight"))
        );
        assert!(
            shambler
                .defeat_line
                .as_deref()
                .is_some_and(|line| line.contains("front step"))
        );
        assert!(
            shambler
                .inspect_alive_line
                .as_deref()
                .is_some_and(|line| line.contains("direct-pressure lane"))
        );
        assert!(
            shambler
                .inspect_defeated_line
                .as_deref()
                .is_some_and(|line| line.contains("mechanically calmer"))
        );
        assert_eq!(
            bundle
                .items
                .iter()
                .find(|item| item.id == "torch")
                .and_then(|item| item.utility_effect.as_deref()),
            Some("reveal_connections")
        );
        let torch = bundle
            .items
            .iter()
            .find(|item| item.id == "torch")
            .expect("expected torch template");
        assert!(
            torch
                .utility_success_line
                .as_deref()
                .is_some_and(|line| line.contains("sweep the torch"))
        );
        assert!(
            torch
                .utility_empty_line
                .as_deref()
                .is_some_and(|line| line.contains("does not reveal anything new"))
        );
        let garage_brute = bundle
            .bosses
            .iter()
            .find(|boss| boss.id == "brute_in_garage")
            .expect("expected garage brute template");
        assert!(garage_brute.can_hear);
        assert!(garage_brute.can_see);
        assert_eq!(garage_brute.wounded_phase_hp_threshold, Some(4));
        assert_eq!(garage_brute.wounded_phase_damage_bonus, 1);
        assert!(
            garage_brute
                .wounded_phase_combat_line
                .as_deref()
                .is_some_and(|line| line.contains("more dangerous"))
        );
        assert!(
            garage_brute
                .defeat_line
                .as_deref()
                .is_some_and(|line| line.contains("{boss_name} collapses"))
        );
        assert!(
            garage_brute
                .retaliation_line
                .as_deref()
                .is_some_and(|line| line.contains("{damage} damage"))
        );
        assert!(
            garage_brute
                .finale_security_retaliation_line
                .as_deref()
                .is_some_and(|line| line.contains("{reduction}"))
        );
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
        assert_eq!(
            garage.threat_forecast_boss_secured.as_deref(),
            Some("finale is live, but both siege lanes are secured; brute retaliation is reduced")
        );
        assert!(
            garage
                .movement_context_lines
                .iter()
                .any(|line| line.contains("house keys got you this far"))
        );
        assert!(
            garage
                .boss_defeated_objective_line
                .as_deref()
                .is_some_and(|line| line.contains("sounds like a room"))
        );
        assert!(
            garage
                .boss_retaliation_context_line
                .as_deref()
                .is_some_and(|line| line.contains("make mistakes"))
        );
        assert!(
            garage
                .situation_boss_wounded_secured_line
                .as_deref()
                .is_some_and(|line| line.contains("both exposed approaches"))
        );
        assert!(
            garage
                .situation_boss_partially_secured_line
                .as_deref()
                .is_some_and(|line| line.contains("front barricade"))
        );
        let front_verandah = bundle
            .locations
            .iter()
            .find(|location| location.id == "front_verandah")
            .expect("expected front verandah template");
        assert!(
            front_verandah
                .tags
                .iter()
                .any(|tag| tag == "noise_pressure")
        );
        assert!(front_verandah.barricadable);
        assert_eq!(
            front_verandah.route_note.as_deref(),
            Some("threshold defense against front-gate pressure")
        );
        assert!(
            front_verandah
                .threat_forecast_open
                .as_deref()
                .is_some_and(|line| line.contains("{pressure_damage} HP"))
        );
        assert_eq!(
            front_verandah.passive_pressure_enemy_id.as_deref(),
            Some("shambler_front_gate")
        );
        assert!(
            front_verandah
                .passive_pressure_damage_line
                .as_deref()
                .is_some_and(|line| line.contains("threshold"))
        );
        assert!(
            front_verandah
                .situation_enemy_cleared_line
                .as_deref()
                .is_some_and(|line| line.contains("belongs to you"))
        );
        assert!(
            front_verandah
                .situation_high_noise_line
                .as_deref()
                .is_some_and(|line| line.contains("front step"))
        );
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
        assert!(back_garden.tags.iter().any(|tag| tag == "noise_pressure"));
        assert!(
            back_garden
                .route_note
                .as_deref()
                .is_some_and(|line| line.contains("{barricade_heal} HP"))
        );
        assert!(back_garden.locked);
        assert_eq!(back_garden.unlock_item_id.as_deref(), Some("house_keys"));
        assert!(back_garden.barricadable);
        assert_eq!(
            back_garden.passive_pressure_enemy_id.as_deref(),
            Some("crawler_in_weeds")
        );
        assert!(
            back_garden
                .passive_pressure_blocked_line
                .as_deref()
                .is_some_and(|line| line.contains("back barricade"))
        );
        assert!(
            back_garden
                .situation_barricaded_line
                .as_deref()
                .is_some_and(|line| line.contains("{barricade_heal} HP"))
        );
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
    fn rules_validation_rejects_unknown_finale_references() {
        let dir = TempDatapackDir::new("rules_unknown_finale_refs");
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
            "rules.toml",
            r#"scenario_id = "test_pack"
starting_location = "start"
boundary_mode = "scenario_blocked"
objective_mode = "single_frozen_objective"
finale_target_location_id = "missing_room"
finale_boss_id = "missing_boss"
finale_secured_location_ids = ["start", "missing_lane"]
finale_secured_retaliation_reduction = 1
"#,
        );

        let errors = load_datapack_bundle_from_path(dir.path(), "temp_test_pack")
            .expect_err("expected datapack validation to fail");

        assert!(errors.iter().any(|error| {
            error.contains("finale_target_location_id 'missing_room' was not found")
        }));
        assert!(errors.iter().any(|error| {
            error.contains("finale_secured_location_ids references unknown location 'missing_lane'")
        }));
        assert!(
            errors
                .iter()
                .any(|error| error.contains("finale_boss_id 'missing_boss' was not found"))
        );
    }

    #[test]
    fn rules_validation_rejects_negative_finale_reduction() {
        let dir = TempDatapackDir::new("rules_negative_finale_reduction");
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
            "rules.toml",
            r#"scenario_id = "test_pack"
starting_location = "start"
boundary_mode = "scenario_blocked"
objective_mode = "single_frozen_objective"
finale_target_location_id = "start"
finale_boss_id = "test_boss"
finale_secured_location_ids = ["start"]
finale_secured_retaliation_reduction = -1
finale_security_secured_line = ""
finale_security_unsecured_line = ""
"#,
        );

        let errors = load_datapack_bundle_from_path(dir.path(), "temp_test_pack")
            .expect_err("expected datapack validation to fail");

        assert!(errors.iter().any(|error| {
            error.contains("finale_secured_retaliation_reduction must not be negative")
        }));
        assert!(
            errors
                .iter()
                .any(|error| error.contains("finale_security_secured_line must not be blank"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("finale_security_unsecured_line must not be blank"))
        );
    }

    #[test]
    fn rules_validation_rejects_invalid_spawned_enemy_movement_policy() {
        let dir = TempDatapackDir::new("rules_invalid_spawned_enemy_movement_policy");
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
            "rules.toml",
            r#"scenario_id = "test_pack"
starting_location = "start"
boundary_mode = "scenario_blocked"
objective_mode = "single_frozen_objective"
spawned_enemy_movement_policy = "teleport"
starter_hint_line = ""
"#,
        );

        let errors = load_datapack_bundle_from_path(dir.path(), "temp_test_pack")
            .expect_err("expected datapack validation to fail");

        assert!(errors.iter().any(|error| {
            error.contains("spawned_enemy_movement_policy 'teleport' is not supported")
        }));
        assert!(
            errors
                .iter()
                .any(|error| error.contains("starter_hint_line must not be blank"))
        );
    }

    #[test]
    fn boss_validation_rejects_invalid_wounded_phase_fields() {
        let dir = TempDatapackDir::new("boss_invalid_wounded_phase");
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
            "templates/bosses.toml",
            r#"[[bosses]]
id = "test_boss"
name = "Test Boss"
description = "A boss."
tags = ["boss"]
hp = 4
damage = 1
defeat_line = ""
retaliation_line = ""
finale_security_retaliation_line = ""
wounded_phase_hp_threshold = 4
wounded_phase_damage_bonus = -1
wounded_phase_combat_line = ""
"#,
        );

        let errors = load_datapack_bundle_from_path(dir.path(), "temp_test_pack")
            .expect_err("expected datapack validation to fail");

        assert!(errors.iter().any(|error| {
            error.contains("wounded_phase_hp_threshold must be lower than boss hp")
        }));
        assert!(
            errors
                .iter()
                .any(|error| { error.contains("wounded_phase_damage_bonus must not be negative") })
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("blank wounded_phase_combat_line"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("blank defeat_line"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("blank retaliation_line"))
        );
        assert!(
            errors
                .iter()
                .any(|error| { error.contains("blank finale_security_retaliation_line") })
        );
    }

    #[test]
    fn enemy_validation_rejects_blank_flavor_hook_fields() {
        let dir = TempDatapackDir::new("enemy_blank_flavor_hooks");
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
            "templates/enemies.toml",
            r#"[[enemies]]
id = "test_enemy"
name = "Test Enemy"
description = "An enemy."
tags = ["melee"]
hp = 1
damage = 1
retaliation_line = ""
"#,
        );

        let errors = load_datapack_bundle_from_path(dir.path(), "temp_test_pack")
            .expect_err("expected datapack validation to fail");

        assert!(
            errors
                .iter()
                .any(|error| error.contains("blank retaliation_line"))
        );
    }

    #[test]
    fn item_validation_rejects_blank_flavor_hook_fields() {
        let dir = TempDatapackDir::new("item_blank_flavor_hooks");
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
            "templates/items.toml",
            r#"[[items]]
id = "test_item"
name = "Test Item"
description = "An item."
tags = ["utility"]
inspect_lines = [""]
objective_required_line = ""
pickup_line = ""
utility_success_line = ""
utility_empty_line = ""
"#,
        );

        let errors = load_datapack_bundle_from_path(dir.path(), "temp_test_pack")
            .expect_err("expected datapack validation to fail");

        assert!(
            errors
                .iter()
                .any(|error| { error.contains("must not define blank inspect_lines entries") })
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("blank objective_required_line"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("blank pickup_line"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("blank utility_success_line"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("blank utility_empty_line"))
        );
    }

    #[test]
    fn location_validation_rejects_blank_context_hook_fields() {
        let dir = TempDatapackDir::new("location_blank_context_hooks");
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
route_note = ""
threat_forecast_open = ""
movement_context_lines = [""]
boss_defeated_objective_line = ""
boss_retaliation_context_line = ""
situation_enemy_cleared_line = ""
situation_barricaded_line = ""
situation_high_noise_line = ""
situation_boss_wounded_secured_line = ""
situation_boss_wounded_line = ""
situation_boss_secured_line = ""
situation_boss_partially_secured_line = ""
passive_pressure_enemy_id = ""
passive_pressure_blocked_line = ""
passive_pressure_damage_line = ""
passive_pressure_high_noise_line = ""
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
                .any(|error| error.contains("blank route_note"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("blank threat_forecast_open"))
        );
        assert!(errors.iter().any(|error| {
            error.contains("must not define blank movement_context_lines entries")
        }));
        assert!(
            errors
                .iter()
                .any(|error| error.contains("blank boss_defeated_objective_line"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("blank boss_retaliation_context_line"))
        );
        for field_name in [
            "situation_enemy_cleared_line",
            "situation_barricaded_line",
            "situation_high_noise_line",
            "situation_boss_wounded_secured_line",
            "situation_boss_wounded_line",
            "situation_boss_secured_line",
            "situation_boss_partially_secured_line",
        ] {
            assert!(
                errors
                    .iter()
                    .any(|error| error.contains(&format!("blank {}", field_name))),
                "expected blank validation error for {field_name}"
            );
        }
        assert!(
            errors
                .iter()
                .any(|error| error.contains("blank passive_pressure_enemy_id"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("blank passive_pressure_blocked_line"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("blank passive_pressure_damage_line"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("blank passive_pressure_high_noise_line"))
        );
    }

    #[test]
    fn location_validation_rejects_unknown_passive_pressure_enemy() {
        let dir = TempDatapackDir::new("location_unknown_passive_pressure_enemy");
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
passive_pressure_enemy_id = "missing_enemy"
items = []
enemies = []
bosses = ["test_boss"]
"#,
        );

        let errors = load_datapack_bundle_from_path(dir.path(), "temp_test_pack")
            .expect_err("expected datapack validation to fail");

        assert!(errors.iter().any(|error| {
            error.contains("references unknown passive_pressure_enemy_id 'missing_enemy'")
        }));
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
