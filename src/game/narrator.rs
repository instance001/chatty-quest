use crate::data::datapacks::DatapackBundle;
use crate::game::actions::EncounterKind;

use super::{ActionOutcome, GameAction, GameEvent, RunState};

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

    fn focus_flavor_line(
        &self,
        bundle: &DatapackBundle,
        action: Option<&GameAction>,
        outcome: &ActionOutcome,
        state: &RunState,
    ) -> Option<String> {
        let brief = outcome
            .events
            .iter()
            .rev()
            .find_map(|event| self.brief_for_event(bundle, event))
            .or_else(|| self.location_brief(bundle, &state.current_location_id))?;

        Some(format!(
            "{}: {}",
            self.style_label(),
            self.flavor_wrapper(action, &brief)
        ))
    }

    fn brief_for_event(&self, bundle: &DatapackBundle, event: &GameEvent) -> Option<String> {
        match event {
            GameEvent::Moved { to_location_id, .. }
            | GameEvent::LocationLooked {
                location_id: to_location_id,
            }
            | GameEvent::Waited {
                location_id: to_location_id,
            } => self.location_brief(bundle, to_location_id),
            GameEvent::Inspected { target }
            | GameEvent::ItemTaken { item_id: target }
            | GameEvent::ItemEquipped { item_id: target }
            | GameEvent::ItemUsed {
                item_id: target, ..
            } => self
                .item_brief(bundle, target)
                .or_else(|| self.enemy_brief(bundle, target))
                .or_else(|| self.boss_brief(bundle, target)),
            GameEvent::AttackResolved {
                target_id,
                target_kind,
                ..
            } => match target_kind {
                EncounterKind::Enemy => self.enemy_brief(bundle, target_id),
                EncounterKind::Boss => self.boss_brief(bundle, target_id),
            },
            GameEvent::HelpShown
            | GameEvent::ActionRejected { .. }
            | GameEvent::MovementBlocked { .. }
            | GameEvent::DamageTaken { .. }
            | GameEvent::AttackWhiff
            | GameEvent::ObjectiveCompleted { .. }
            | GameEvent::RunWon
            | GameEvent::RunLost => None,
        }
    }

    fn location_brief(&self, bundle: &DatapackBundle, location_id: &str) -> Option<String> {
        bundle
            .locations
            .iter()
            .find(|location| location.id == location_id)
            .and_then(|location| location.narrator_brief.clone())
    }

    fn item_brief(&self, bundle: &DatapackBundle, item_id: &str) -> Option<String> {
        bundle
            .items
            .iter()
            .find(|item| item.id == item_id)
            .and_then(|item| item.narrator_brief.clone())
    }

    fn enemy_brief(&self, bundle: &DatapackBundle, enemy_id: &str) -> Option<String> {
        bundle
            .enemies
            .iter()
            .find(|enemy| enemy.id == enemy_id)
            .and_then(|enemy| enemy.narrator_brief.clone())
    }

    fn boss_brief(&self, bundle: &DatapackBundle, boss_id: &str) -> Option<String> {
        bundle
            .bosses
            .iter()
            .find(|boss| boss.id == boss_id)
            .and_then(|boss| boss.narrator_brief.clone())
    }

    fn flavor_wrapper(&self, action: Option<&GameAction>, brief: &str) -> String {
        let lower = self.dm_style.to_ascii_lowercase();

        if lower.contains("hostile") || lower.contains("meatgrinder") {
            return match action {
                None => format!("Here is the shape of the misery: {}", brief),
                Some(GameAction::Move { .. }) | Some(GameAction::Look) | Some(GameAction::Wait) => {
                    format!("The place makes its case quickly: {}", brief)
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

    fn embellish_line(&self, action: &GameAction, line: &str, state: &RunState) -> String {
        let prefix = format!("{}: ", self.style_label());
        let lower = self.dm_style.to_ascii_lowercase();

        if matches!(line, "You win." | "You lose.") || line.starts_with("Objective complete:") {
            return format!("{}{}", prefix, line);
        }

        if lower.contains("hostile") {
            if matches!(action, GameAction::Wait) {
                return format!("{}{} Pathetic, but technically sensible.", prefix, line);
            }
            if matches!(action, GameAction::Attack) && state.hp < state.max_hp {
                return format!("{}{} You look less immortal already.", prefix, line);
            }
            if line.contains("You move to") {
                return format!("{}{} Try not to die in this room too.", prefix, line);
            }
        }

        if lower.contains("slapstick") && self.chaos_mode > 0.0 {
            if matches!(action, GameAction::Look) {
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

    fn event_preface(&self, outcome: &ActionOutcome) -> Option<String> {
        if outcome
            .events
            .iter()
            .any(|event| matches!(event, GameEvent::RunWon))
        {
            return Some(format!(
                "{}: The ledger agrees. You won.",
                self.style_label()
            ));
        }

        if outcome
            .events
            .iter()
            .any(|event| matches!(event, GameEvent::RunLost))
        {
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
        if let Some(brief) = self.focus_flavor_line(
            bundle,
            None,
            &ActionOutcome {
                events: vec![GameEvent::LocationLooked {
                    location_id: state.current_location_id.clone(),
                }],
                lines: Vec::new(),
            },
            state,
        ) {
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
        let mut lines = Vec::new();
        if let Some(preface) = self.event_preface(outcome) {
            lines.push(preface);
        }
        if let Some(brief) = self.focus_flavor_line(bundle, Some(action), outcome, state) {
            lines.push(brief);
        }
        lines.extend(
            outcome
                .lines
                .iter()
                .map(|line| self.embellish_line(action, line, state)),
        );
        lines
    }
}

#[cfg(test)]
mod tests {
    use crate::data::datapacks::load_datapack_bundle_by_folder;
    use crate::game::actions::{GameAction, GameEvent};
    use crate::game::generation::generate_new_run;

    use super::{MockNarrator, Narrator};

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
}
