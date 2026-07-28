use crate::data::datapacks::DatapackBundle;

use super::RunState;

#[derive(Clone, Copy)]
pub enum ObjectiveConditionRowStyle {
    Requirement,
    Diagnostic,
}

pub fn run_phase_label(run: &RunState) -> &'static str {
    if run.hp <= 0 {
        "Loss"
    } else if run.active_objective.completed {
        "Epilogue"
    } else {
        "Active"
    }
}

pub fn objective_condition_rows(
    run: &RunState,
    bundle: Option<&DatapackBundle>,
    style: ObjectiveConditionRowStyle,
) -> Vec<String> {
    let mut rows = Vec::new();

    if let Some(required_item_id) = &run.active_objective.required_item_id {
        let held = run
            .inventory
            .iter()
            .any(|item| &item.id == required_item_id);
        rows.push(match style {
            ObjectiveConditionRowStyle::Requirement => format!(
                "Requires item '{}': {}",
                objective_item_label(bundle, required_item_id),
                if held { "held" } else { "missing" }
            ),
            ObjectiveConditionRowStyle::Diagnostic => format!(
                "Objective item '{}': {}",
                objective_item_label(bundle, required_item_id),
                if held { "held" } else { "missing" }
            ),
        });
    }

    if let Some(target_boss_id) = &run.active_objective.target_boss_id {
        let defeated = run.bosses_defeated.contains(target_boss_id);
        rows.push(match style {
            ObjectiveConditionRowStyle::Requirement => format!(
                "Requires boss '{}': {}",
                objective_boss_label(bundle, target_boss_id),
                if defeated { "defeated" } else { "alive" }
            ),
            ObjectiveConditionRowStyle::Diagnostic => format!(
                "Objective boss '{}': {}",
                objective_boss_label(bundle, target_boss_id),
                if defeated { "defeated" } else { "alive" }
            ),
        });
    }

    if let Some(required_location_id) = &run.active_objective.required_location_id {
        let at_location = run.current_location_id == *required_location_id;
        rows.push(match style {
            ObjectiveConditionRowStyle::Requirement => format!(
                "Requires location '{}': {}",
                objective_location_label(bundle, required_location_id),
                if at_location {
                    "reached"
                } else {
                    "not reached"
                }
            ),
            ObjectiveConditionRowStyle::Diagnostic => format!(
                "Objective location '{}': {}",
                objective_location_label(bundle, required_location_id),
                if at_location {
                    "reached"
                } else {
                    "not reached"
                }
            ),
        });
    }

    rows
}

pub fn utility_relevance_rows(run: &RunState, bundle: Option<&DatapackBundle>) -> Vec<String> {
    let Some(bundle) = bundle else {
        return Vec::new();
    };

    let mut rows = Vec::new();
    for inventory_item in &run.inventory {
        let Some(template) = bundle
            .items
            .iter()
            .find(|item| item.id == inventory_item.id)
        else {
            continue;
        };

        match template.utility_effect.as_deref() {
            Some("reveal_connections") => rows.push(format!(
                "Utility: {} ({}) reveals connected exits from the current room.",
                template.name, template.id
            )),
            Some("barricade") => {
                let open_targets = open_barricade_target_labels(run, bundle, &template.id);
                if open_targets.is_empty() {
                    rows.push(format!(
                        "Utility: {} ({}) has no remaining barricade targets.",
                        template.name, template.id
                    ));
                } else {
                    rows.push(format!(
                        "Utility: {} ({}) can secure {}.",
                        template.name,
                        template.id,
                        open_targets.join(", ")
                    ));
                }
            }
            Some(effect) => rows.push(format!(
                "Utility: {} ({}) has utility effect '{}'.",
                template.name, template.id, effect
            )),
            None => {}
        }
    }

    rows
}

pub fn security_summary_rows(run: &RunState, bundle: Option<&DatapackBundle>) -> Vec<String> {
    let Some(bundle) = bundle else {
        return Vec::new();
    };
    let barricadable_locations = bundle
        .locations
        .iter()
        .filter(|location| location.barricadable)
        .collect::<Vec<_>>();
    if barricadable_locations.is_empty() {
        return Vec::new();
    }

    let secured = barricadable_locations
        .iter()
        .filter(|location| run.barricaded_locations.contains(&location.id))
        .map(|location| format!("{} ({})", location.name, location.id))
        .collect::<Vec<_>>();
    let open = barricadable_locations
        .iter()
        .filter(|location| !run.barricaded_locations.contains(&location.id))
        .map(|location| format!("{} ({})", location.name, location.id))
        .collect::<Vec<_>>();

    let mut rows = vec![
        format!(
            "Secured approaches: {}",
            if secured.is_empty() {
                "none".to_owned()
            } else {
                secured.join(", ")
            }
        ),
        format!(
            "Open approaches: {}",
            if open.is_empty() {
                "none".to_owned()
            } else {
                open.join(", ")
            }
        ),
    ];

    if property_siege_lanes_secured(run) {
        rows.push(
            "Finale security: both exposed approaches are secured; garage retaliation is reduced."
                .to_owned(),
        );
    } else if run.active_objective.required_location_id.as_deref() == Some("garage") {
        rows.push(
            "Finale security: secure Front Verandah and Back Garden before the finale to reduce garage retaliation."
                .to_owned(),
        );
    }

    if run.barricaded_locations.is_empty() {
        rows.push("Noise recovery: no barricaded room is available yet.".to_owned());
    } else {
        rows.push("Noise recovery: waiting in a barricaded room can lower noise.".to_owned());
    }

    rows
}

fn objective_item_label(bundle: Option<&DatapackBundle>, item_id: &str) -> String {
    bundle
        .and_then(|bundle| {
            bundle
                .items
                .iter()
                .find(|item| item.id == item_id)
                .map(|item| format!("{} ({})", item.name, item.id))
        })
        .unwrap_or_else(|| item_id.to_owned())
}

fn objective_boss_label(bundle: Option<&DatapackBundle>, boss_id: &str) -> String {
    bundle
        .and_then(|bundle| {
            bundle
                .bosses
                .iter()
                .find(|boss| boss.id == boss_id)
                .map(|boss| format!("{} ({})", boss.name, boss.id))
        })
        .unwrap_or_else(|| boss_id.to_owned())
}

fn objective_location_label(bundle: Option<&DatapackBundle>, location_id: &str) -> String {
    bundle
        .and_then(|bundle| {
            bundle
                .locations
                .iter()
                .find(|location| location.id == location_id)
                .map(|location| format!("{} ({})", location.name, location.id))
        })
        .unwrap_or_else(|| location_id.to_owned())
}

fn open_barricade_target_labels(
    run: &RunState,
    bundle: &DatapackBundle,
    item_id: &str,
) -> Vec<String> {
    bundle
        .locations
        .iter()
        .filter(|location| {
            location.barricadable
                && location.barricade_item_id.as_deref() == Some(item_id)
                && !run.barricaded_locations.contains(&location.id)
        })
        .map(|location| format!("{} ({})", location.name, location.id))
        .collect()
}

fn property_siege_lanes_secured(run: &RunState) -> bool {
    run.barricaded_locations.contains("front_verandah")
        && run.barricaded_locations.contains("back_garden")
}
