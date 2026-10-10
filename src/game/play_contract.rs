use serde::{Deserialize, Serialize};

/// The run's explicit agreement about how much latitude the command resolver and narrator have.
/// It is derived from the persisted preset name so old saves keep their original behavior.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlayContract {
    pub preset: PlayPreset,
    pub command_tolerance: CommandTolerance,
    pub accounting_strict: bool,
    pub failure_severity: FailureSeverity,
    pub narrator_tone_freedom: ToneFreedom,
    pub allow_generated_flavor: bool,
    pub allow_generated_persistent_content: bool,
    pub allow_generated_mechanical_effects: bool,
    pub protected_invariants: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayPreset {
    Simulation,
    RulesFirst,
    Adventure,
    Chaos,
    Unleashed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandTolerance {
    Exact,
    Helpful,
    Permissive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureSeverity {
    Low,
    Standard,
    High,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToneFreedom {
    Narrow,
    Guided,
    Wide,
}

impl PlayContract {
    pub fn from_preset_name(name: &str) -> Self {
        let preset = match name {
            "simulation" => PlayPreset::Simulation,
            "adventure" => PlayPreset::Adventure,
            "chaos" => PlayPreset::Chaos,
            "unleashed" => PlayPreset::Unleashed,
            _ => PlayPreset::RulesFirst,
        };
        let protected_invariants = vec![
            "Reducers own canonical mutations.".to_owned(),
            "Narrator proposals require player confirmation.".to_owned(),
            "Authored datapacks are never modified.".to_owned(),
        ];
        match preset {
            PlayPreset::Simulation => Self {
                preset,
                command_tolerance: CommandTolerance::Exact,
                accounting_strict: true,
                failure_severity: FailureSeverity::High,
                narrator_tone_freedom: ToneFreedom::Narrow,
                allow_generated_flavor: false,
                allow_generated_persistent_content: false,
                allow_generated_mechanical_effects: false,
                protected_invariants,
            },
            PlayPreset::RulesFirst => Self {
                preset,
                command_tolerance: CommandTolerance::Helpful,
                accounting_strict: true,
                failure_severity: FailureSeverity::Standard,
                narrator_tone_freedom: ToneFreedom::Guided,
                allow_generated_flavor: true,
                allow_generated_persistent_content: false,
                allow_generated_mechanical_effects: false,
                protected_invariants,
            },
            PlayPreset::Adventure => Self {
                preset,
                command_tolerance: CommandTolerance::Helpful,
                accounting_strict: false,
                failure_severity: FailureSeverity::Standard,
                narrator_tone_freedom: ToneFreedom::Guided,
                allow_generated_flavor: true,
                allow_generated_persistent_content: true,
                allow_generated_mechanical_effects: false,
                protected_invariants,
            },
            PlayPreset::Chaos => Self {
                preset,
                command_tolerance: CommandTolerance::Permissive,
                accounting_strict: false,
                failure_severity: FailureSeverity::Low,
                narrator_tone_freedom: ToneFreedom::Wide,
                allow_generated_flavor: true,
                allow_generated_persistent_content: true,
                allow_generated_mechanical_effects: true,
                protected_invariants,
            },
            PlayPreset::Unleashed => Self {
                preset,
                command_tolerance: CommandTolerance::Permissive,
                accounting_strict: false,
                failure_severity: FailureSeverity::Low,
                narrator_tone_freedom: ToneFreedom::Wide,
                allow_generated_flavor: true,
                allow_generated_persistent_content: true,
                allow_generated_mechanical_effects: true,
                protected_invariants,
            },
        }
    }

    pub fn allows_persistent_content(&self) -> bool {
        self.allow_generated_persistent_content
    }

    #[allow(dead_code)]
    pub fn allows_intent_admission(&self) -> bool {
        matches!(self.command_tolerance, CommandTolerance::Permissive)
    }

    pub fn control_summary(&self) -> Vec<String> {
        vec![
            format!("Commands: {:?}", self.command_tolerance),
            format!(
                "Accounting: {}",
                if self.accounting_strict {
                    "strict"
                } else {
                    "guided"
                }
            ),
            format!("Failure: {:?}", self.failure_severity),
            format!("Narrator tone: {:?}", self.narrator_tone_freedom),
            format!(
                "Generated mechanics: {}",
                if self.allow_generated_mechanical_effects {
                    "allowed with confirmation"
                } else {
                    "off"
                }
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandTolerance, PlayContract};

    #[test]
    fn strict_presets_refuse_persistent_content_and_unleashed_is_permissive() {
        assert!(!PlayContract::from_preset_name("simulation").allows_persistent_content());
        assert!(!PlayContract::from_preset_name("rules_first").allows_persistent_content());
        let unleashed = PlayContract::from_preset_name("unleashed");
        assert!(unleashed.allows_persistent_content());
        assert_eq!(unleashed.command_tolerance, CommandTolerance::Permissive);
    }
}
