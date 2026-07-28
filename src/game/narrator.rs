use crate::data::datapacks::DatapackBundle;
use crate::game::actions::EncounterKind;
use crate::game::derived::run_phase_label;

use super::{ActionOutcome, GameAction, GameEvent, RunState};

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct NarratorContext {
    pub action: Option<GameAction>,
    pub reducer_lines: Vec<String>,
    pub event_facts: Vec<String>,
    pub run_phase: String,
    pub current_location_id: String,
    pub current_location_name: String,
    pub current_location_description: String,
    pub current_location_brief: Option<String>,
    pub focus_brief: Option<String>,
    pub visible_exits: Vec<NarratorExitContext>,
    pub recent_summary: Vec<String>,
    pub inventory_items: Vec<String>,
    pub equipped_item: Option<String>,
    pub hp: i32,
    pub max_hp: i32,
    pub noise_level: i32,
    pub noise_label: String,
    pub objective_name: String,
    pub objective_complete: bool,
    pub objective_facts: Vec<String>,
    pub dm_style: Option<String>,
    pub world_tone: Option<String>,
    pub safety_rules: Vec<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct NarratorExitContext {
    pub location_id: String,
    pub name: String,
    pub locked: bool,
    pub barricaded: bool,
}

pub trait Narrator {
    fn narrate_run_start(
        &self,
        bundle: &DatapackBundle,
        state: &RunState,
        seed_lines: &[String],
    ) -> Vec<String>;

    fn narrate_action(
        &self,
        bundle: &DatapackBundle,
        action: &GameAction,
        outcome: &ActionOutcome,
        state: &RunState,
    ) -> Vec<String>;
}

pub fn build_narrator_context(
    bundle: &DatapackBundle,
    state: &RunState,
    action: Option<&GameAction>,
    outcome: Option<&ActionOutcome>,
) -> NarratorContext {
    let current_location = bundle
        .locations
        .iter()
        .find(|location| location.id == state.current_location_id);
    let current_location_name = current_location
        .map(|location| location.name.clone())
        .unwrap_or_else(|| state.current_location_id.clone());
    let current_location_description = current_location
        .map(|location| {
            if state.active_objective.completed {
                location
                    .epilogue_description
                    .clone()
                    .unwrap_or_else(|| location.description.clone())
            } else {
                location.description.clone()
            }
        })
        .unwrap_or_else(|| "Current location could not be resolved.".to_owned());
    let current_location_brief =
        current_location.and_then(|location| location.narrator_brief.clone());

    NarratorContext {
        action: action.cloned(),
        reducer_lines: outcome
            .map(|outcome| outcome.lines.clone())
            .unwrap_or_default(),
        event_facts: outcome
            .map(|outcome| outcome.events.iter().map(event_fact).collect())
            .unwrap_or_default(),
        run_phase: run_phase_label(state).to_owned(),
        current_location_id: state.current_location_id.clone(),
        current_location_name,
        current_location_description,
        current_location_brief,
        focus_brief: outcome
            .and_then(|outcome| {
                outcome
                    .events
                    .iter()
                    .rev()
                    .find_map(|event| brief_for_event(bundle, event))
            })
            .or_else(|| current_location.and_then(|location| location.narrator_brief.clone())),
        visible_exits: current_location
            .map(|location| {
                location
                    .connections
                    .iter()
                    .filter_map(|location_id| {
                        bundle
                            .locations
                            .iter()
                            .find(|candidate| candidate.id == *location_id)
                            .map(|exit| NarratorExitContext {
                                location_id: exit.id.clone(),
                                name: exit.name.clone(),
                                locked: state.locked_locations.contains(&exit.id),
                                barricaded: state.barricaded_locations.contains(&exit.id),
                            })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        recent_summary: state.rolling_summary.clone(),
        inventory_items: state
            .inventory
            .iter()
            .map(|item| item.name.clone())
            .collect(),
        equipped_item: state.equipped_item_id.as_ref().and_then(|equipped_id| {
            state
                .inventory
                .iter()
                .find(|item| item.id == *equipped_id)
                .map(|item| item.name.clone())
        }),
        hp: state.hp,
        max_hp: state.max_hp,
        noise_level: state.noise_level,
        noise_label: noise_label(state.noise_level).to_owned(),
        objective_name: state.active_objective.name.clone(),
        objective_complete: state.active_objective.completed,
        objective_facts: objective_facts(state),
        dm_style: bundle.dm_style.clone(),
        world_tone: bundle.world_tone.clone(),
        safety_rules: narrator_safety_rules(),
    }
}

fn event_fact(event: &GameEvent) -> String {
    match event {
        GameEvent::HelpShown => "Help was shown.".to_owned(),
        GameEvent::ActionRejected { reason } => format!("Action rejected: {}.", reason),
        GameEvent::LocationLooked { location_id } => {
            format!("Location looked: {}.", location_id)
        }
        GameEvent::Moved {
            from_location_id,
            to_location_id,
        } => format!("Moved from {} to {}.", from_location_id, to_location_id),
        GameEvent::MovementBlocked {
            attempted_destination,
        } => format!("Movement blocked toward {}.", attempted_destination),
        GameEvent::LocationUnlocked {
            location_id,
            item_id,
        } => format!("Unlocked {} with {}.", location_id, item_id),
        GameEvent::LocationBarricaded {
            location_id,
            item_id,
        } => format!("Barricaded {} with {}.", location_id, item_id),
        GameEvent::Inspected { target } => format!("Inspected {}.", target),
        GameEvent::ItemTaken { item_id } => format!("Took item {}.", item_id),
        GameEvent::ItemEquipped { item_id } => format!("Equipped item {}.", item_id),
        GameEvent::ItemUsed { item_id, effect } => {
            format!("Used item {} with effect {:?}.", item_id, effect)
        }
        GameEvent::AttackResolved {
            target_id,
            target_kind,
            damage,
            defeated,
        } => format!(
            "Attack resolved against {:?} {} for {} damage; defeated: {}.",
            target_kind, target_id, damage, defeated
        ),
        GameEvent::DamageTaken {
            amount,
            remaining_hp,
        } => format!(
            "Player took {} damage; remaining HP: {}.",
            amount, remaining_hp
        ),
        GameEvent::AttackWhiff => "Attack found no valid target.".to_owned(),
        GameEvent::Waited { location_id } => format!("Waited at {}.", location_id),
        GameEvent::ObjectiveCompleted { objective_id } => {
            format!("Objective completed: {}.", objective_id)
        }
        GameEvent::RunWon => "Run won.".to_owned(),
        GameEvent::RunLost => "Run lost.".to_owned(),
    }
}

fn brief_for_event(bundle: &DatapackBundle, event: &GameEvent) -> Option<String> {
    match event {
        GameEvent::Moved { to_location_id, .. }
        | GameEvent::LocationLooked {
            location_id: to_location_id,
        }
        | GameEvent::Waited {
            location_id: to_location_id,
        } => location_brief(bundle, to_location_id),
        GameEvent::Inspected { target }
        | GameEvent::ItemTaken { item_id: target }
        | GameEvent::ItemEquipped { item_id: target }
        | GameEvent::ItemUsed {
            item_id: target, ..
        } => item_brief(bundle, target)
            .or_else(|| enemy_brief(bundle, target))
            .or_else(|| boss_brief(bundle, target)),
        GameEvent::AttackResolved {
            target_id,
            target_kind,
            ..
        } => match target_kind {
            EncounterKind::Enemy => enemy_brief(bundle, target_id),
            EncounterKind::Boss => boss_brief(bundle, target_id),
        },
        GameEvent::HelpShown
        | GameEvent::ActionRejected { .. }
        | GameEvent::MovementBlocked { .. }
        | GameEvent::LocationUnlocked { .. }
        | GameEvent::LocationBarricaded { .. }
        | GameEvent::DamageTaken { .. }
        | GameEvent::AttackWhiff
        | GameEvent::ObjectiveCompleted { .. }
        | GameEvent::RunWon
        | GameEvent::RunLost => None,
    }
}

fn location_brief(bundle: &DatapackBundle, location_id: &str) -> Option<String> {
    bundle
        .locations
        .iter()
        .find(|location| location.id == location_id)
        .and_then(|location| location.narrator_brief.clone())
}

fn item_brief(bundle: &DatapackBundle, item_id: &str) -> Option<String> {
    bundle
        .items
        .iter()
        .find(|item| item.id == item_id)
        .and_then(|item| item.narrator_brief.clone())
}

fn enemy_brief(bundle: &DatapackBundle, enemy_id: &str) -> Option<String> {
    bundle
        .enemies
        .iter()
        .find(|enemy| enemy.id == enemy_id)
        .and_then(|enemy| enemy.narrator_brief.clone())
}

fn boss_brief(bundle: &DatapackBundle, boss_id: &str) -> Option<String> {
    bundle
        .bosses
        .iter()
        .find(|boss| boss.id == boss_id)
        .and_then(|boss| boss.narrator_brief.clone())
}

fn objective_facts(state: &RunState) -> Vec<String> {
    let mut facts = Vec::new();

    if let Some(required_item_id) = state.active_objective.required_item_id.as_deref() {
        let held = state
            .inventory
            .iter()
            .any(|item| item.id == required_item_id);
        facts.push(format!(
            "Required item {} held: {}.",
            required_item_id, held
        ));
    }

    if let Some(required_location_id) = state.active_objective.required_location_id.as_deref() {
        facts.push(format!(
            "Required location {} reached now: {}.",
            required_location_id,
            state.current_location_id == required_location_id
        ));
    }

    if let Some(target_boss_id) = state.active_objective.target_boss_id.as_deref() {
        facts.push(format!(
            "Target boss {} defeated: {}.",
            target_boss_id,
            state.bosses_defeated.contains(target_boss_id)
        ));
    }

    facts
}

fn narrator_safety_rules() -> Vec<String> {
    [
        "Do not invent permanent items.",
        "Do not invent canonical locations.",
        "Do not alter HP, inventory, equipment, lock state, barricade state, objective state, or player location.",
        "Do not claim a reducer action succeeded if the event facts say it was rejected or blocked.",
        "Treat reducer lines and event facts as authoritative.",
        "Use tone and atmosphere only as presentation over confirmed game truth.",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn noise_label(level: i32) -> &'static str {
    match level {
        0 => "Quiet",
        1 => "Stirred",
        2 => "Loud",
        _ => "Swarming",
    }
}

#[derive(Clone, Debug)]
pub struct MockNarrator {
    dm_style: String,
    world_tone: Option<String>,
    chaos_mode: f32,
}

impl MockNarrator {
    pub fn new(bundle: &DatapackBundle, chaos_mode: f32) -> Self {
        Self {
            dm_style: bundle
                .dm_style
                .clone()
                .unwrap_or_else(|| "Dry rules referee".to_owned()),
            world_tone: bundle.world_tone.clone(),
            chaos_mode,
        }
    }

    fn style_label(&self) -> &'static str {
        let lower = self.dm_style.to_ascii_lowercase();
        if lower.contains("hostile") || lower.contains("meatgrinder") {
            "DM"
        } else if lower.contains("cozy") {
            "Storyteller"
        } else if lower.contains("slapstick") {
            "Goblin DM"
        } else {
            "Narrator"
        }
    }

    fn mood_line(&self) -> Option<String> {
        self.world_tone
            .as_ref()
            .map(|tone| format!("{} notes the tone: {}.", self.style_label(), tone))
    }

    fn focus_flavor_line_from_context(&self, context: &NarratorContext) -> Option<String> {
        let brief = context.focus_brief.as_deref()?;

        Some(format!(
            "{}: {}",
            self.style_label(),
            self.flavor_wrapper(context.action.as_ref(), brief)
        ))
    }

    fn flavor_wrapper(&self, action: Option<&GameAction>, brief: &str) -> String {
        let lower = self.dm_style.to_ascii_lowercase();

        if lower.contains("hostile") || lower.contains("meatgrinder") {
            return match action {
                None => format!("Here is the shape of the misery: {}", brief),
                Some(GameAction::Move { .. }) | Some(GameAction::Look) | Some(GameAction::Wait) => {
                    format!("The place makes its case quickly: {}", brief)
                }
                Some(GameAction::Unlock { .. }) | Some(GameAction::Barricade { .. }) => {
                    format!("The mechanism gives up its little truth: {}", brief)
                }
                Some(GameAction::Inspect { .. }) => {
                    format!("A closer look only improves the bad news: {}", brief)
                }
                Some(GameAction::Take { .. })
                | Some(GameAction::Equip { .. })
                | Some(GameAction::Use { .. }) => {
                    format!("The object has opinions too: {}", brief)
                }
                Some(GameAction::Attack) => {
                    format!("The threat deserves this much honesty: {}", brief)
                }
                Some(GameAction::Help) => format!("The rules remain ugly and simple: {}", brief),
            };
        }

        if lower.contains("slapstick") {
            return match action {
                None => format!("The scene waddles in like this: {}", brief),
                Some(GameAction::Move { .. }) | Some(GameAction::Look) | Some(GameAction::Wait) => {
                    format!(
                        "The place presents itself with terrible confidence: {}",
                        brief
                    )
                }
                Some(GameAction::Unlock { .. }) | Some(GameAction::Barricade { .. }) => {
                    format!("Even the hardware joins the bit: {}", brief)
                }
                Some(GameAction::Inspect { .. }) => {
                    format!("Closer inspection somehow makes it weirder: {}", brief)
                }
                Some(GameAction::Take { .. })
                | Some(GameAction::Equip { .. })
                | Some(GameAction::Use { .. }) => {
                    format!("The prop department insists on this detail: {}", brief)
                }
                Some(GameAction::Attack) => {
                    format!("The violence arrives with flavour text attached: {}", brief)
                }
                Some(GameAction::Help) => format!("Even the rules sound like a bit: {}", brief),
            };
        }

        if lower.contains("cozy") {
            return match action {
                None => format!("The scene opens gently: {}", brief),
                Some(GameAction::Move { .. }) | Some(GameAction::Look) | Some(GameAction::Wait) => {
                    format!("The place settles around you like this: {}", brief)
                }
                Some(GameAction::Unlock { .. }) | Some(GameAction::Barricade { .. }) => {
                    format!("The small mechanism yields in a readable way: {}", brief)
                }
                Some(GameAction::Inspect { .. }) => {
                    format!("A closer look reveals the little truth of it: {}", brief)
                }
                Some(GameAction::Take { .. })
                | Some(GameAction::Equip { .. })
                | Some(GameAction::Use { .. }) => {
                    format!("The item carries its own quiet story: {}", brief)
                }
                Some(GameAction::Attack) => {
                    format!("Even the danger arrives with a clear feeling: {}", brief)
                }
                Some(GameAction::Help) => {
                    format!("The shape of the world stays understandable: {}", brief)
                }
            };
        }

        match action {
            None => format!("The scene sets itself like this: {}", brief),
            Some(GameAction::Move { .. }) | Some(GameAction::Look) | Some(GameAction::Wait) => {
                format!("The place reads like this: {}", brief)
            }
            Some(GameAction::Unlock { .. }) | Some(GameAction::Barricade { .. }) => {
                format!("The gate gives up its state cleanly: {}", brief)
            }
            Some(GameAction::Inspect { .. }) => {
                format!("A closer look gives the right texture: {}", brief)
            }
            Some(GameAction::Take { .. })
            | Some(GameAction::Equip { .. })
            | Some(GameAction::Use { .. }) => {
                format!("The object lands in the hand like this: {}", brief)
            }
            Some(GameAction::Attack) => format!("The threat comes into focus: {}", brief),
            Some(GameAction::Help) => format!("The situation stays legible: {}", brief),
        }
    }

    fn embellish_line_from_context(&self, context: &NarratorContext, line: &str) -> String {
        let prefix = format!("{}: ", self.style_label());
        let lower = self.dm_style.to_ascii_lowercase();
        let action = context.action.as_ref();

        if matches!(line, "You win." | "You lose.") || line.starts_with("Objective complete:") {
            return format!("{}{}", prefix, line);
        }

        if lower.contains("hostile") {
            if matches!(action, Some(GameAction::Wait)) {
                return format!("{}{} Pathetic, but technically sensible.", prefix, line);
            }
            if matches!(action, Some(GameAction::Attack)) && context.hp < context.max_hp {
                return format!("{}{} You look less immortal already.", prefix, line);
            }
            if line.contains("You move to") {
                return format!("{}{} Try not to die in this room too.", prefix, line);
            }
        }

        if lower.contains("slapstick") && self.chaos_mode > 0.0 {
            if matches!(action, Some(GameAction::Look)) {
                return format!(
                    "{}{} The property somehow disapproves of your face.",
                    prefix, line
                );
            }
            if line.contains("You take the") {
                return format!(
                    "{}{} A triumph for little grabby hands everywhere.",
                    prefix, line
                );
            }
        }

        if lower.contains("cozy") {
            return format!("{}{}", prefix, line);
        }

        format!("{}{}", prefix, line)
    }

    fn event_preface_from_context(&self, context: &NarratorContext) -> Option<String> {
        if context.event_facts.iter().any(|fact| fact == "Run won.") {
            return Some(format!(
                "{}: The ledger agrees. You won.",
                self.style_label()
            ));
        }

        if context.event_facts.iter().any(|fact| fact == "Run lost.") {
            return Some(format!(
                "{}: The ledger agrees. You lost.",
                self.style_label()
            ));
        }

        None
    }
}

impl Narrator for MockNarrator {
    fn narrate_run_start(
        &self,
        bundle: &DatapackBundle,
        state: &RunState,
        seed_lines: &[String],
    ) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(format!("{} enters the scene.", self.style_label()));
        lines.extend(
            seed_lines
                .iter()
                .map(|line| format!("{}: {}", self.style_label(), line)),
        );
        let context = build_narrator_context(
            bundle,
            state,
            None,
            Some(&ActionOutcome {
                events: vec![GameEvent::LocationLooked {
                    location_id: state.current_location_id.clone(),
                }],
                lines: Vec::new(),
            }),
        );
        if let Some(brief) = self.focus_flavor_line_from_context(&context) {
            lines.push(brief);
        }
        if let Some(mood) = self.mood_line() {
            lines.push(mood);
        }
        lines
    }

    fn narrate_action(
        &self,
        bundle: &DatapackBundle,
        action: &GameAction,
        outcome: &ActionOutcome,
        state: &RunState,
    ) -> Vec<String> {
        let context = build_narrator_context(bundle, state, Some(action), Some(outcome));
        let mut lines = Vec::new();
        if let Some(preface) = self.event_preface_from_context(&context) {
            lines.push(preface);
        }
        if let Some(brief) = self.focus_flavor_line_from_context(&context) {
            lines.push(brief);
        }
        lines.extend(
            context
                .reducer_lines
                .iter()
                .map(|line| self.embellish_line_from_context(&context, line)),
        );
        lines
    }
}

#[cfg(test)]
mod tests {
    use crate::data::datapacks::load_datapack_bundle_by_folder;
    use crate::game::actions::{EncounterKind, GameAction, GameEvent};
    use crate::game::generation::generate_new_run;

    use super::{MockNarrator, Narrator, build_narrator_context};

    #[test]
    fn narrator_run_start_and_actions_reflect_existing_state() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let state = generate_new_run(&bundle).state;
        let narrator = MockNarrator::new(&bundle, 0.10);

        let start_lines = narrator.narrate_run_start(
            &bundle,
            &state,
            &["Scenario loaded: Property Siege Classic.".to_owned()],
        );
        assert!(
            start_lines
                .iter()
                .any(|line| line.contains("DM enters the scene."))
        );
        assert!(
            start_lines
                .iter()
                .any(|line| line.contains("Suburban siege horror"))
        );

        let outcome = crate::game::ActionOutcome {
            events: vec![GameEvent::ItemTaken {
                item_id: "medkit".to_owned(),
            }],
            lines: vec!["You take the Medkit.".to_owned()],
        };
        let narrated = narrator.narrate_action(
            &bundle,
            &GameAction::Take {
                item_name: "medkit".to_owned(),
            },
            &outcome,
            &state,
        );

        assert!(
            narrated
                .iter()
                .any(|line| line.contains("You take the Medkit."))
        );
        assert!(narrated.iter().any(|line| line.contains("tiny miracle")));
        assert!(
            !narrated
                .iter()
                .any(|line| line.contains("new canonical location"))
        );
    }

    #[test]
    fn narrator_surfaces_win_without_owning_state() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let state = generate_new_run(&bundle).state;
        let narrator = MockNarrator::new(&bundle, 0.10);
        let outcome = crate::game::ActionOutcome {
            events: vec![
                GameEvent::ObjectiveCompleted {
                    objective_id: "secure_property".to_owned(),
                },
                GameEvent::RunWon,
            ],
            lines: vec![
                "Objective complete: Secure The Property.".to_owned(),
                "You win.".to_owned(),
            ],
        };

        let narrated = narrator.narrate_action(&bundle, &GameAction::Attack, &outcome, &state);

        assert!(
            narrated
                .iter()
                .any(|line| line.contains("The ledger agrees. You won."))
        );
        assert!(narrated.iter().any(|line| line.contains("You win.")));
    }

    #[test]
    fn narrator_context_exposes_bounded_truth_for_future_adapters() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "garage".to_owned();
        state.locked_locations.remove("garage");
        state.noise_level = 3;
        state.active_objective.completed = true;
        let outcome = crate::game::ActionOutcome {
            events: vec![
                GameEvent::AttackResolved {
                    target_id: "brute_in_garage".to_owned(),
                    target_kind: EncounterKind::Boss,
                    damage: 3,
                    defeated: true,
                },
                GameEvent::RunWon,
            ],
            lines: vec!["You win.".to_owned()],
        };

        let context =
            build_narrator_context(&bundle, &state, Some(&GameAction::Attack), Some(&outcome));

        assert_eq!(context.run_phase, "Epilogue");
        assert_eq!(context.current_location_id, "garage");
        assert_eq!(context.noise_label, "Swarming");
        assert_eq!(context.objective_name, "Secure The Property");
        assert!(context.objective_complete);
        assert!(
            context
                .recent_summary
                .iter()
                .any(|line| { line.contains("Run started for scenario 'Property Siege Classic'") })
        );
        assert!(context.event_facts.iter().any(|fact| fact == "Run won."));
        assert!(
            context
                .objective_facts
                .iter()
                .any(|fact| fact.contains("Target boss brute_in_garage defeated"))
        );
        assert!(
            context
                .current_location_description
                .contains("gives up being an arena")
        );
        assert!(
            context
                .safety_rules
                .iter()
                .any(|rule| rule.contains("Do not invent canonical locations"))
        );
    }

    #[test]
    fn narrator_context_surfaces_visible_exits_without_unlocking_them() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let state = generate_new_run(&bundle).state;

        let context = build_narrator_context(&bundle, &state, None, None);

        assert!(
            context
                .visible_exits
                .iter()
                .any(|exit| exit.location_id == "garage" && exit.locked)
        );
        assert!(
            context
                .visible_exits
                .iter()
                .any(|exit| exit.location_id == "back_garden" && exit.locked)
        );
        assert!(!context.objective_complete);
        assert_eq!(state.current_location_id, "front_verandah");
    }
}
