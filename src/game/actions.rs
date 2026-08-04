use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GameAction {
    Help,
    Look,
    Move { destination: String },
    Unlock { target: String },
    Barricade { target: String },
    Inspect { target: String },
    Take { item_name: String },
    Equip { item_name: String },
    Use { item_name: String },
    Attack,
    Wait,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EncounterKind {
    Enemy,
    Boss,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MovementHazardKind {
    Barricade,
    LockedGate,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ItemUseEffect {
    Healing { amount: i32 },
    RevealedLocations { count: usize },
    NoEffect,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GameEvent {
    HelpShown,
    ActionRejected {
        reason: String,
    },
    SelectedPackFailedValidation {
        folder_name: String,
        reason: String,
    },
    LocationLooked {
        location_id: String,
    },
    Moved {
        from_location_id: String,
        to_location_id: String,
    },
    MovementBlocked {
        attempted_destination: String,
    },
    LocationUnlocked {
        location_id: String,
        item_id: String,
    },
    LocationBarricaded {
        location_id: String,
        item_id: String,
    },
    Inspected {
        target: String,
    },
    ItemTaken {
        item_id: String,
    },
    ItemEquipped {
        item_id: String,
    },
    ItemUsed {
        item_id: String,
        effect: ItemUseEffect,
    },
    AttackResolved {
        target_id: String,
        target_kind: EncounterKind,
        damage: i32,
        defeated: bool,
    },
    DamageTaken {
        amount: i32,
        remaining_hp: i32,
    },
    NoiseSpawnedEnemy {
        enemy_id: String,
        template_id: String,
        location_id: String,
    },
    NoiseAttractorShifted {
        location_id: String,
        enemy_ids: Vec<String>,
    },
    SightAttractorAcquired {
        enemy_id: String,
        subject_id: String,
        location_id: String,
    },
    SightAttractorMissed {
        enemy_id: String,
        subject_id: String,
        location_id: String,
        detect_chance_percent: u8,
        roll_percent: u8,
    },
    SightAttractorLost {
        enemy_id: String,
        subject_id: String,
    },
    SpawnedEnemyMoved {
        enemy_id: String,
        from_location_id: String,
        to_location_id: String,
        target_location_id: String,
    },
    SpawnedEnemyWaited {
        enemy_id: String,
        location_id: String,
        reason: String,
    },
    SpawnedEnemyAttackedHazard {
        enemy_id: String,
        hazard_kind: MovementHazardKind,
        location_id: String,
        break_chance_percent: u8,
        roll_percent: u8,
        broken: bool,
    },
    AttackWhiff,
    Waited {
        location_id: String,
    },
    ObjectiveCompleted {
        objective_id: String,
    },
    RunWon,
    RunLost,
}

#[derive(Clone, Debug)]
pub struct ActionOutcome {
    pub events: Vec<GameEvent>,
    pub lines: Vec<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct ParsedCommand {
    pub raw_input: String,
    pub normalized_input: String,
    pub action: GameAction,
}

#[allow(dead_code)]
pub fn parse_action(input: &str) -> Result<GameAction, String> {
    parse_command(input).map(|command| command.action)
}

pub fn parse_command(input: &str) -> Result<ParsedCommand, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Type a command first.".to_owned());
    }

    let lower = trimmed.to_ascii_lowercase();

    if lower == "help" {
        return parsed(trimmed, &lower, GameAction::Help);
    }
    if lower == "look" || lower == "inspect room" {
        return parsed(trimmed, &lower, GameAction::Look);
    }
    if lower == "attack" || lower == "hit" {
        return parsed(trimmed, &lower, GameAction::Attack);
    }
    if lower == "wait" {
        return parsed(trimmed, &lower, GameAction::Wait);
    }
    if let Some(verb) = targetless_verb(&lower) {
        return Err(format!("Tell me what to {}.", verb));
    }
    if let Some(rest) = lower
        .strip_prefix("go ")
        .or_else(|| lower.strip_prefix("move "))
        .or_else(|| lower.strip_prefix("walk "))
    {
        return parsed_target(trimmed, &lower, rest, "go", |destination| {
            GameAction::Move { destination }
        });
    }
    if let Some(rest) = lower
        .strip_prefix("unlock ")
        .or_else(|| lower.strip_prefix("open "))
    {
        return parsed_target(trimmed, &lower, rest, "unlock", |target| {
            GameAction::Unlock { target }
        });
    }
    if let Some(rest) = lower
        .strip_prefix("barricade ")
        .or_else(|| lower.strip_prefix("fortify "))
        .or_else(|| lower.strip_prefix("secure "))
    {
        return parsed_target(trimmed, &lower, rest, "barricade", |target| {
            GameAction::Barricade { target }
        });
    }
    if let Some(rest) = lower.strip_prefix("inspect ") {
        return parsed_target(trimmed, &lower, rest, "inspect", |target| {
            GameAction::Inspect { target }
        });
    }
    if let Some(rest) = lower.strip_prefix("take ") {
        return parsed_target(trimmed, &lower, rest, "take", |item_name| {
            GameAction::Take { item_name }
        });
    }
    if let Some(rest) = lower.strip_prefix("equip ") {
        return parsed_target(trimmed, &lower, rest, "equip", |item_name| {
            GameAction::Equip { item_name }
        });
    }
    if let Some(rest) = lower.strip_prefix("use ") {
        return parsed_target(trimmed, &lower, rest, "use", |item_name| GameAction::Use {
            item_name,
        });
    }

    Err("I only understand a narrow set of commands right now. Try: help, look, go ..., unlock ..., barricade ..., inspect ..., take ..., equip ..., use ..., attack, wait.".to_owned())
}

fn targetless_verb(input: &str) -> Option<&'static str> {
    match input {
        "go" | "move" | "walk" => Some("go"),
        "unlock" | "open" => Some("unlock"),
        "barricade" | "fortify" | "secure" => Some("barricade"),
        "inspect" => Some("inspect"),
        "take" => Some("take"),
        "equip" => Some("equip"),
        "use" => Some("use"),
        _ => None,
    }
}

fn parsed(
    raw_input: &str,
    normalized_input: &str,
    action: GameAction,
) -> Result<ParsedCommand, String> {
    Ok(ParsedCommand {
        raw_input: raw_input.to_owned(),
        normalized_input: normalized_input.to_owned(),
        action,
    })
}

fn parsed_target(
    raw_input: &str,
    normalized_input: &str,
    rest: &str,
    verb: &str,
    make_action: impl FnOnce(String) -> GameAction,
) -> Result<ParsedCommand, String> {
    let target = rest.trim();
    if target.is_empty() {
        return Err(format!("Tell me what to {}.", verb));
    }

    parsed(raw_input, normalized_input, make_action(target.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::{GameAction, parse_action, parse_command};

    #[test]
    fn parse_action_supports_unlock_aliases() {
        assert!(matches!(
            parse_action("unlock garage"),
            Ok(GameAction::Unlock { target }) if target == "garage"
        ));
        assert!(matches!(
            parse_action("open garage"),
            Ok(GameAction::Unlock { target }) if target == "garage"
        ));
    }

    #[test]
    fn parse_action_supports_barricade_aliases() {
        assert!(matches!(
            parse_action("barricade front verandah"),
            Ok(GameAction::Barricade { target }) if target == "front verandah"
        ));
        assert!(matches!(
            parse_action("fortify front verandah"),
            Ok(GameAction::Barricade { target }) if target == "front verandah"
        ));
    }

    #[test]
    fn parse_command_preserves_raw_input_and_resolves_to_structured_action() {
        let parsed = parse_command("  Go Garage  ").expect("expected command to parse");

        assert_eq!(parsed.raw_input, "Go Garage");
        assert_eq!(parsed.normalized_input, "go garage");
        assert!(matches!(
            parsed.action,
            GameAction::Move { destination } if destination == "garage"
        ));
    }

    #[test]
    fn parse_action_rejects_target_verbs_without_targets() {
        assert!(matches!(parse_action("go "), Err(error) if error == "Tell me what to go."));
        assert!(
            matches!(parse_action("inspect "), Err(error) if error == "Tell me what to inspect.")
        );
        assert!(matches!(parse_action("use "), Err(error) if error == "Tell me what to use."));
    }
}
