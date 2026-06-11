use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GameAction {
    Help,
    Look,
    Move { destination: String },
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

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ItemUseEffect {
    Healing { amount: i32 },
    NoEffect,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GameEvent {
    HelpShown,
    ActionRejected {
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

pub fn parse_action(input: &str) -> Result<GameAction, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Type a command first.".to_owned());
    }

    let lower = trimmed.to_ascii_lowercase();

    if lower == "help" {
        return Ok(GameAction::Help);
    }
    if lower == "look" || lower == "inspect room" {
        return Ok(GameAction::Look);
    }
    if lower == "attack" || lower == "hit" {
        return Ok(GameAction::Attack);
    }
    if lower == "wait" {
        return Ok(GameAction::Wait);
    }
    if let Some(rest) = lower
        .strip_prefix("go ")
        .or_else(|| lower.strip_prefix("move "))
        .or_else(|| lower.strip_prefix("walk "))
    {
        return Ok(GameAction::Move {
            destination: rest.trim().to_owned(),
        });
    }
    if let Some(rest) = lower.strip_prefix("inspect ") {
        return Ok(GameAction::Inspect {
            target: rest.trim().to_owned(),
        });
    }
    if let Some(rest) = lower.strip_prefix("take ") {
        return Ok(GameAction::Take {
            item_name: rest.trim().to_owned(),
        });
    }
    if let Some(rest) = lower.strip_prefix("equip ") {
        return Ok(GameAction::Equip {
            item_name: rest.trim().to_owned(),
        });
    }
    if let Some(rest) = lower.strip_prefix("use ") {
        return Ok(GameAction::Use {
            item_name: rest.trim().to_owned(),
        });
    }

    Err("I only understand a narrow set of commands right now. Try: help, look, go ..., inspect ..., take ..., equip ..., use ..., attack, wait.".to_owned())
}
