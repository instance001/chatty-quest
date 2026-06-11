use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const DATAPACKS_ROOT: &str = "assets/datapacks";
const DATAPACK_SCHEMA_VERSION: &str = "v0.1-toml-templates-2";

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
}

#[derive(Clone, Debug, Deserialize)]
pub struct LocationTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub narrator_brief: Option<String>,
    pub tags: Vec<String>,
    #[serde(default)]
    pub media: MediaReferences,
    #[serde(default)]
    pub connections: Vec<String>,
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
}

#[derive(Clone, Debug, Deserialize)]
pub struct EnemyTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub narrator_brief: Option<String>,
    pub tags: Vec<String>,
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
    pub target_boss_id: String,
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

    let root = Path::new(DATAPACKS_ROOT);
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
    let path = Path::new(DATAPACKS_ROOT).join(folder_name);
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
        Path::new(DATAPACKS_ROOT)
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
    }

    if let Some(locations) = &locations {
        validate_unique_ids("locations", &locations.locations, &mut errors);
        validate_non_blank_names("locations", &locations.locations, &mut errors);
    }
    if let Some(items) = &items {
        validate_unique_ids("items", &items.items, &mut errors);
        validate_non_blank_names("items", &items.items, &mut errors);
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

    if let (Some(objectives), Some(bosses)) = (&objectives, &bosses) {
        let known_bosses: HashSet<&str> = bosses
            .bosses
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        for objective in &objectives.objectives {
            if objective.target_boss_id.trim().is_empty() {
                errors.push(format!(
                    "Objective '{}' must define a target_boss_id.",
                    objective.id
                ));
            } else if !known_bosses.contains(objective.target_boss_id.as_str()) {
                errors.push(format!(
                    "Objective '{}' references unknown target boss '{}'.",
                    objective.id, objective.target_boss_id
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
    use super::{discover_datapacks, load_datapack_bundle_by_folder};

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
        assert_eq!(record.summary.item_count, 4);
        assert_eq!(record.summary.enemy_count, 2);
        assert_eq!(record.summary.boss_count, 1);
        assert_eq!(record.summary.objective_count, 1);
    }

    #[test]
    fn property_siege_classic_bundle_loads_with_expected_templates() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");

        assert_eq!(bundle.pack.id, "property_siege_classic");
        assert_eq!(bundle.rules.starting_location, "front_verandah");
        assert!(
            bundle
                .locations
                .iter()
                .any(|location| location.id == "garage")
        );
        assert!(bundle.items.iter().any(|item| item.id == "cricket_bat"));
        assert!(
            bundle
                .bosses
                .iter()
                .any(|boss| boss.id == "brute_in_garage")
        );
        assert_eq!(bundle.objectives[0].target_boss_id, "brute_in_garage");
    }
}
