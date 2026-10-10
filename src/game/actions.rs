use serde::{Deserialize, Serialize};

/// Why a shared ownership transfer was requested. It never changes the ownership primitive;
/// it only selects the host policy and any reducer-confirmed social consequence.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferIntent {
    #[default]
    Ordinary,
    Theft,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GameAction {
    Help,
    Look,
    Move {
        destination: String,
    },
    Unlock {
        target: String,
    },
    Barricade {
        target: String,
    },
    Inspect {
        target: String,
    },
    Take {
        item_name: String,
    },
    Equip {
        item_name: String,
    },
    Use {
        item_name: String,
    },
    Attack,
    /// A controlled-player attack with an explicit hostile query. The reducer resolves the
    /// current controlled actor at commit time, so command parsing never hard-codes `player`.
    AttackTarget {
        target: String,
    },
    /// A host-resolved request for any runtime actor to use the shared combat mechanic.
    /// `origin_id` is an instance id, never a narrator-owned display name.
    ActorAttack {
        origin_id: String,
        target: String,
    },
    /// A host-resolved request for any active runtime actor to use the shared movement mechanic.
    /// As with actor combat, this carries an instance id rather than a display name.
    ActorMove {
        origin_id: String,
        destination: String,
    },
    TransferItem {
        item_id: String,
        origin: crate::game::state::ItemOwner,
        destination: crate::game::state::ItemOwner,
        #[serde(default)]
        intent: TransferIntent,
    },
    /// A bounded semantic social intent after host resolution. The reducer owns the arithmetic.
    ApplySocialEvent {
        origin_id: String,
        subject_id: String,
        kind: crate::game::social::SocialEventKind,
    },
    /// Uses one mutable grant against its immutable template. The reducer validates possession,
    /// tags, charges and expiry before applying the host-supported effect.
    UseAbility {
        origin_id: String,
        grant_id: String,
        target: String,
    },
    UnlockAbility {
        owner: crate::game::state::ItemOwner,
        template_id: String,
    },
    /// Materializes one mutable grant from an immutable ability template. This is the single
    /// host action used by training, rewards, temporary effects, and item-bound sources; callers
    /// choose the owner and concrete lifetime without inventing another ability mechanic.
    GrantAbility {
        owner: crate::game::state::ItemOwner,
        template_id: String,
        remaining_uses: Option<u32>,
        duration_turns: Option<u64>,
        source: crate::game::state::AbilityGrantSource,
    },
    /// Converts a concrete item-held ability grant into permanent knowledge. The reducer checks
    /// the learner's permanent tag-matched literacy grant before consuming the source item.
    LearnAbilityFromItem {
        learner_id: String,
        item_id: String,
        grant_id: String,
    },
    /// Awards positive progress into one authored pool. This is intentionally separate from an
    /// ability grant: a cast, trainer, reward, or future quest can use the same progression path.
    AwardProgress {
        recipient_id: String,
        pool: String,
        amount: i32,
    },
    Wait,
    AdmitNarratorItem {
        name: String,
        description: String,
        damage: i32,
        condition: Option<String>,
        tags: Vec<String>,
        actor: Option<crate::game::ActorState>,
        disposition: crate::game::ActorDisposition,
    },
    AdmitNarratorStatus {
        name: String,
        trigger: String,
        duration: String,
        additive_damage: i32,
        additive_healing: i32,
        required_target_tags: Vec<String>,
        required_context_tags: Vec<String>,
    },
    AdmitNarratorNpc {
        name: String,
        description: String,
        faction: String,
        tags: Vec<String>,
        actor: crate::game::ActorState,
        disposition: crate::game::ActorDisposition,
    },
    AdmitNarratorEnemy {
        name: String,
        description: String,
        max_hp: i32,
        damage: i32,
        tags: Vec<String>,
    },
    AdmitNarratorBoss {
        name: String,
        description: String,
        max_hp: i32,
        damage: i32,
        tags: Vec<String>,
    },
    AdmitNarratorObjective {
        name: String,
        description: String,
        completion_key: String,
    },
    AdmitNarratorTemporaryExit {
        name: String,
        description: String,
        from_location_id: String,
        to_location_id: String,
        locked: bool,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EncounterKind {
    Enemy,
    Boss,
    Npc,
    /// A runtime actor-capable item or future entity that is neither an encounter template nor an
    /// NPC. The reducer still applies the same health/combat contract.
    Actor,
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
        origin_id: String,
        location_id: String,
    },
    Moved {
        origin_id: String,
        from_location_id: String,
        to_location_id: String,
    },
    MovementBlocked {
        origin_id: String,
        attempted_destination: String,
    },
    LocationUnlocked {
        origin_id: String,
        location_id: String,
        item_id: String,
    },
    LocationBarricaded {
        origin_id: String,
        location_id: String,
        item_id: String,
    },
    Inspected {
        origin_id: String,
        target: String,
    },
    ItemTaken {
        origin_id: String,
        item_id: String,
    },
    ItemEquipped {
        origin_id: String,
        item_id: String,
    },
    ItemTransferred {
        item_id: String,
        origin: crate::game::state::ItemOwner,
        destination: crate::game::state::ItemOwner,
    },
    SocialEventApplied {
        origin_id: String,
        subject_id: String,
        kind: crate::game::social::SocialEventKind,
        relationship_delta: i32,
        relationship_score: i32,
        disposition: crate::game::state::ActorDisposition,
    },
    AbilityUsed {
        origin_id: String,
        grant_id: String,
        template_id: String,
        target_id: String,
        effect: String,
        remaining_uses: Option<u32>,
    },
    AbilityCooldownSet {
        origin_id: String,
        grant_id: String,
        template_id: String,
        ready_at_turn: u64,
    },
    StatusEffectApplied {
        origin_id: String,
        instance_id: String,
        trigger: crate::game::state::EffectTrigger,
        duration: crate::game::state::EffectDuration,
    },
    ConnectionsRevealed {
        origin_id: String,
        location_id: String,
        count: usize,
    },
    ProgressAwarded {
        origin_id: String,
        pool: String,
        amount: i32,
        total: i32,
    },
    SkillLevelGained {
        origin_id: String,
        pool: String,
        level: u32,
    },
    MaxHpIncreased {
        origin_id: String,
        amount: i32,
        max_hp: i32,
    },
    AbilityGranted {
        owner_id: String,
        grant_id: String,
        template_id: String,
        source: crate::game::state::AbilityGrantSource,
    },
    AbilityGrantExpired {
        owner_id: String,
        grant_id: String,
        template_id: String,
    },
    ItemUsed {
        origin_id: String,
        item_id: String,
        effect: ItemUseEffect,
    },
    ItemConsumed {
        origin_id: String,
        item_id: String,
        template_id: String,
        reason: String,
    },
    NarratorItemAdmitted {
        item_id: String,
    },
    NarratorStatusAdmitted {
        instance_id: String,
    },
    NarratorNpcAdmitted {
        instance_id: String,
    },
    NarratorEnemyAdmitted {
        instance_id: String,
    },
    NarratorBossAdmitted {
        instance_id: String,
    },
    NarratorTemporaryExitAdmitted {
        instance_id: String,
    },
    StatusEffectExpired {
        instance_id: String,
    },
    AttackResolved {
        origin_id: String,
        target_id: String,
        target_kind: EncounterKind,
        damage: i32,
        defeated: bool,
    },
    DamageTaken {
        origin_id: String,
        target_id: String,
        amount: i32,
        remaining_hp: i32,
    },
    HealthRestored {
        origin_id: String,
        target_id: String,
        amount: i32,
        remaining_hp: i32,
    },
    ActorSpawnedFromNoise {
        origin_id: String,
        template_id: String,
        location_id: String,
    },
    NoiseAttractorShifted {
        location_id: String,
        actor_ids: Vec<String>,
    },
    SightAttractorAcquired {
        origin_id: String,
        subject_id: String,
        location_id: String,
    },
    SightAttractorMissed {
        origin_id: String,
        subject_id: String,
        location_id: String,
        detect_chance_percent: u8,
        roll_percent: u8,
    },
    SightAttractorLost {
        origin_id: String,
        subject_id: String,
    },
    ActorMoved {
        origin_id: String,
        from_location_id: String,
        to_location_id: String,
        target_location_id: String,
    },
    ActorWaited {
        origin_id: String,
        location_id: String,
        reason: String,
    },
    ActorAttackedHazard {
        origin_id: String,
        hazard_kind: MovementHazardKind,
        location_id: String,
        break_chance_percent: u8,
        roll_percent: u8,
        broken: bool,
    },
    AttackWhiff {
        origin_id: String,
    },
    Waited {
        origin_id: String,
        location_id: String,
    },
    ObjectiveCompleted {
        objective_id: String,
    },
    RunWon,
    RunLost {
        actor_id: String,
    },
}

#[derive(Clone, Debug)]
pub struct ActionOutcome {
    pub events: Vec<GameEvent>,
    pub lines: Vec<String>,
}

/// A presentation-safe account of a reducer event.  It contains no new authority: it merely
/// makes the existing event stream legible to the chat, narrator, diagnostics, and future UI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionTrace {
    pub action_type: String,
    pub origin: String,
    pub impact: String,
    pub outcome: String,
}

/// The one host-owned, presentation-neutral reading of a reducer fact. Consumers that only need
/// causality (log, narrator, diagnostics, summary) should use this instead of reinterpreting
/// individual event variants.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventProjection {
    pub action_type: String,
    pub origin: String,
    pub impact: String,
    pub outcome: String,
}

pub fn event_projection(event: &GameEvent) -> EventProjection {
    match event {
        GameEvent::HelpShown => EventProjection {
            action_type: "interface".to_owned(),
            origin: "player".to_owned(),
            impact: "command help".to_owned(),
            outcome: "shown".to_owned(),
        },
        GameEvent::ActionRejected { reason } => EventProjection {
            action_type: "rejected".to_owned(),
            origin: "host validator".to_owned(),
            impact: "requested action".to_owned(),
            outcome: reason.clone(),
        },
        GameEvent::SelectedPackFailedValidation {
            folder_name,
            reason,
        } => EventProjection {
            action_type: "validation".to_owned(),
            origin: "host validator".to_owned(),
            impact: folder_name.clone(),
            outcome: reason.clone(),
        },
        GameEvent::LocationLooked {
            origin_id,
            location_id,
        } => EventProjection {
            action_type: "inspection".to_owned(),
            origin: origin_id.clone(),
            impact: location_id.clone(),
            outcome: "observed".to_owned(),
        },
        GameEvent::Moved {
            origin_id,
            from_location_id,
            to_location_id,
        } => EventProjection {
            action_type: "movement".to_owned(),
            origin: origin_id.clone(),
            impact: to_location_id.clone(),
            outcome: format!("moved from {from_location_id}"),
        },
        GameEvent::MovementBlocked {
            origin_id,
            attempted_destination,
        } => EventProjection {
            action_type: "movement".to_owned(),
            origin: origin_id.clone(),
            impact: attempted_destination.clone(),
            outcome: "blocked".to_owned(),
        },
        GameEvent::LocationUnlocked {
            origin_id,
            location_id,
            item_id,
        } => EventProjection {
            action_type: "unlock".to_owned(),
            origin: origin_id.clone(),
            impact: location_id.clone(),
            outcome: format!("unlocked with {item_id}"),
        },
        GameEvent::LocationBarricaded {
            origin_id,
            location_id,
            item_id,
        } => EventProjection {
            action_type: "barricade".to_owned(),
            origin: origin_id.clone(),
            impact: location_id.clone(),
            outcome: format!("secured with {item_id}"),
        },
        GameEvent::Inspected { origin_id, target } => EventProjection {
            action_type: "inspection".to_owned(),
            origin: origin_id.clone(),
            impact: target.clone(),
            outcome: "inspected".to_owned(),
        },
        GameEvent::AttackResolved {
            origin_id,
            target_id,
            target_kind,
            damage,
            defeated,
        } => EventProjection {
            action_type: "combat".to_owned(),
            origin: origin_id.clone(),
            impact: format!("{:?} {target_id}", target_kind).to_ascii_lowercase(),
            outcome: format!(
                "{damage} damage{}",
                if *defeated { "; defeated" } else { "" }
            ),
        },
        GameEvent::ItemTaken { origin_id, item_id } => EventProjection {
            action_type: "inventory".to_owned(),
            origin: origin_id.clone(),
            impact: item_id.clone(),
            outcome: "taken".to_owned(),
        },
        GameEvent::ItemTransferred {
            item_id,
            origin,
            destination,
        } => EventProjection {
            action_type: "transfer".to_owned(),
            origin: origin.to_string(),
            impact: destination.to_string(),
            outcome: format!("{item_id} transferred"),
        },
        GameEvent::SocialEventApplied {
            origin_id,
            subject_id,
            kind,
            relationship_delta,
            relationship_score,
            disposition,
        } => EventProjection {
            action_type: "social".to_owned(),
            origin: origin_id.clone(),
            impact: subject_id.clone(),
            outcome: format!(
                "{:?}: {relationship_delta:+}; relationship {relationship_score}, {:?}",
                kind, disposition
            )
            .to_ascii_lowercase(),
        },
        GameEvent::AbilityUsed {
            origin_id,
            template_id,
            target_id,
            effect,
            remaining_uses,
            ..
        } => EventProjection {
            action_type: "ability".to_owned(),
            origin: origin_id.clone(),
            impact: target_id.clone(),
            outcome: format!(
                "{template_id} {effect}; {} uses remaining",
                remaining_uses.map_or("infinite".to_owned(), |uses| uses.to_string())
            ),
        },
        GameEvent::AbilityCooldownSet {
            origin_id,
            template_id,
            ready_at_turn,
            ..
        } => EventProjection {
            action_type: "cooldown".to_owned(),
            origin: origin_id.clone(),
            impact: template_id.clone(),
            outcome: format!("ready at world turn {ready_at_turn}"),
        },
        GameEvent::StatusEffectApplied {
            origin_id,
            instance_id,
            trigger,
            duration,
        } => EventProjection {
            action_type: "status".to_owned(),
            origin: origin_id.clone(),
            impact: instance_id.clone(),
            outcome: format!("active for {trigger:?} until {duration:?}").to_ascii_lowercase(),
        },
        GameEvent::ConnectionsRevealed {
            origin_id,
            location_id,
            count,
        } => EventProjection {
            action_type: "discovery".to_owned(),
            origin: origin_id.clone(),
            impact: location_id.clone(),
            outcome: format!("{count} connections revealed"),
        },
        GameEvent::ProgressAwarded {
            origin_id,
            pool,
            amount,
            total,
        } => EventProjection {
            action_type: "progress".to_owned(),
            origin: origin_id.clone(),
            impact: pool.clone(),
            outcome: format!("{amount} XP awarded; total {total}"),
        },
        GameEvent::SkillLevelGained {
            origin_id,
            pool,
            level,
        } => EventProjection {
            action_type: "skill_level".to_owned(),
            origin: origin_id.clone(),
            impact: pool.clone(),
            outcome: format!("reached level {level}"),
        },
        GameEvent::MaxHpIncreased {
            origin_id,
            amount,
            max_hp,
        } => EventProjection {
            action_type: "health".to_owned(),
            origin: origin_id.clone(),
            impact: "maximum hp".to_owned(),
            outcome: format!("increased by {amount}; maximum {max_hp}"),
        },
        GameEvent::AbilityGranted {
            owner_id,
            template_id,
            source,
            ..
        } => EventProjection {
            action_type: "ability_grant".to_owned(),
            origin: owner_id.clone(),
            impact: template_id.clone(),
            outcome: format!("grant added from {source:?}").to_ascii_lowercase(),
        },
        GameEvent::AbilityGrantExpired {
            owner_id,
            template_id,
            ..
        } => EventProjection {
            action_type: "ability_grant".to_owned(),
            origin: owner_id.clone(),
            impact: template_id.clone(),
            outcome: "temporary grant expired".to_owned(),
        },
        GameEvent::ItemEquipped { origin_id, item_id } => EventProjection {
            action_type: "equipment".to_owned(),
            origin: origin_id.clone(),
            impact: item_id.clone(),
            outcome: "equipped".to_owned(),
        },
        GameEvent::ItemUsed {
            origin_id, item_id, ..
        } => EventProjection {
            action_type: "item_use".to_owned(),
            origin: origin_id.clone(),
            impact: item_id.clone(),
            outcome: "applied".to_owned(),
        },
        GameEvent::ItemConsumed {
            origin_id,
            item_id,
            reason,
            ..
        } => EventProjection {
            action_type: "consumption".to_owned(),
            origin: origin_id.clone(),
            impact: item_id.clone(),
            outcome: reason.clone(),
        },
        GameEvent::NarratorItemAdmitted { item_id } => EventProjection {
            action_type: "admission".to_owned(),
            origin: "player approval".to_owned(),
            impact: item_id.clone(),
            outcome: "item materialized".to_owned(),
        },
        GameEvent::NarratorStatusAdmitted { instance_id } => EventProjection {
            action_type: "admission".to_owned(),
            origin: "player approval".to_owned(),
            impact: instance_id.clone(),
            outcome: "status materialized".to_owned(),
        },
        GameEvent::NarratorNpcAdmitted { instance_id } => EventProjection {
            action_type: "admission".to_owned(),
            origin: "player approval".to_owned(),
            impact: instance_id.clone(),
            outcome: "NPC materialized".to_owned(),
        },
        GameEvent::NarratorEnemyAdmitted { instance_id } => EventProjection {
            action_type: "admission".to_owned(),
            origin: "player approval".to_owned(),
            impact: instance_id.clone(),
            outcome: "enemy materialized".to_owned(),
        },
        GameEvent::NarratorBossAdmitted { instance_id } => EventProjection {
            action_type: "admission".to_owned(),
            origin: "player approval".to_owned(),
            impact: instance_id.clone(),
            outcome: "boss materialized".to_owned(),
        },
        GameEvent::NarratorTemporaryExitAdmitted { instance_id } => EventProjection {
            action_type: "admission".to_owned(),
            origin: "player approval".to_owned(),
            impact: instance_id.clone(),
            outcome: "temporary exit materialized".to_owned(),
        },
        GameEvent::StatusEffectExpired { instance_id } => EventProjection {
            action_type: "status".to_owned(),
            origin: "host lifecycle".to_owned(),
            impact: instance_id.clone(),
            outcome: "expired".to_owned(),
        },
        GameEvent::DamageTaken {
            origin_id,
            target_id,
            amount,
            remaining_hp,
        } => EventProjection {
            action_type: "damage".to_owned(),
            origin: origin_id.clone(),
            impact: target_id.clone(),
            outcome: format!("{amount} damage; {remaining_hp} HP remaining"),
        },
        GameEvent::HealthRestored {
            origin_id,
            target_id,
            amount,
            remaining_hp,
        } => EventProjection {
            action_type: "healing".to_owned(),
            origin: origin_id.clone(),
            impact: target_id.clone(),
            outcome: format!("{amount} HP restored; {remaining_hp} HP remaining"),
        },
        GameEvent::ActorSpawnedFromNoise {
            origin_id,
            template_id,
            location_id,
        } => EventProjection {
            action_type: "noise_spawn".to_owned(),
            origin: "host noise system".to_owned(),
            impact: location_id.clone(),
            outcome: format!("spawned {origin_id} from {template_id}"),
        },
        GameEvent::NoiseAttractorShifted {
            location_id,
            actor_ids,
        } => EventProjection {
            action_type: "noise_targeting".to_owned(),
            origin: "host noise system".to_owned(),
            impact: actor_ids.join(", "),
            outcome: format!("attracted toward {location_id}"),
        },
        GameEvent::SightAttractorAcquired {
            origin_id,
            subject_id,
            location_id,
        } => EventProjection {
            action_type: "sight_targeting".to_owned(),
            origin: origin_id.clone(),
            impact: subject_id.clone(),
            outcome: format!("sighted at {location_id}"),
        },
        GameEvent::SightAttractorMissed {
            origin_id,
            subject_id,
            location_id,
            detect_chance_percent,
            roll_percent,
        } => EventProjection {
            action_type: "sight_targeting".to_owned(),
            origin: origin_id.clone(),
            impact: subject_id.clone(),
            outcome: format!(
                "missed at {location_id}; {detect_chance_percent}% chance, rolled {roll_percent}"
            ),
        },
        GameEvent::SightAttractorLost {
            origin_id,
            subject_id,
        } => EventProjection {
            action_type: "sight_targeting".to_owned(),
            origin: origin_id.clone(),
            impact: subject_id.clone(),
            outcome: "lost sight".to_owned(),
        },
        GameEvent::ActorMoved {
            origin_id,
            to_location_id,
            ..
        } => EventProjection {
            action_type: "movement".to_owned(),
            origin: origin_id.clone(),
            impact: to_location_id.clone(),
            outcome: "moved".to_owned(),
        },
        GameEvent::ActorWaited {
            origin_id,
            location_id,
            reason,
        } => EventProjection {
            // The actor's controller is not mechanics. An autonomous actor and a future
            // commanded pet both performed the same host wait action; `origin` carries who.
            action_type: "wait".to_owned(),
            origin: origin_id.clone(),
            impact: location_id.clone(),
            outcome: reason.clone(),
        },
        GameEvent::ActorAttackedHazard {
            origin_id,
            hazard_kind,
            location_id,
            broken,
            ..
        } => EventProjection {
            action_type: "hazard_attack".to_owned(),
            origin: origin_id.clone(),
            impact: format!("{:?} at {location_id}", hazard_kind).to_ascii_lowercase(),
            outcome: if *broken {
                "broken".to_owned()
            } else {
                "held".to_owned()
            },
        },
        GameEvent::AttackWhiff { origin_id } => EventProjection {
            action_type: "combat".to_owned(),
            origin: origin_id.clone(),
            impact: "no valid target".to_owned(),
            outcome: "missed".to_owned(),
        },
        GameEvent::Waited {
            origin_id,
            location_id,
        } => EventProjection {
            action_type: "wait".to_owned(),
            origin: origin_id.clone(),
            impact: location_id.clone(),
            outcome: "turn advanced".to_owned(),
        },
        GameEvent::ObjectiveCompleted { objective_id } => EventProjection {
            action_type: "objective".to_owned(),
            origin: "host objective system".to_owned(),
            impact: objective_id.clone(),
            outcome: "completed".to_owned(),
        },
        GameEvent::RunWon => EventProjection {
            action_type: "run_state".to_owned(),
            origin: "host objective system".to_owned(),
            impact: "run".to_owned(),
            outcome: "won".to_owned(),
        },
        GameEvent::RunLost { actor_id } => EventProjection {
            action_type: "run_state".to_owned(),
            origin: "host health system".to_owned(),
            impact: actor_id.clone(),
            outcome: "defeated; run lost".to_owned(),
        },
    }
}

impl ActionTrace {
    pub fn chat_line(&self) -> String {
        format!(
            "Action [{}] {} -> {}: {}",
            self.action_type, self.origin, self.impact, self.outcome
        )
    }
}

/// Converts reducer-owned events into a generic origin → impact → outcome trace. Entity kinds
/// remain data, rather than each UI or narrator inventing its own interpretation of a combat
/// result, movement, spawn, or state change.
pub fn action_traces(_: &GameAction, outcome: &ActionOutcome) -> Vec<ActionTrace> {
    outcome
        .events
        .iter()
        .map(event_projection)
        .map(|projection| ActionTrace {
            action_type: projection.action_type,
            origin: projection.origin,
            impact: projection.impact,
            outcome: projection.outcome,
        })
        .collect()
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct ParsedCommand {
    pub raw_input: String,
    pub normalized_input: String,
    pub action: GameAction,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum IntentResolution {
    Confirmed(ParsedCommand),
    NeedsAdmission { player_text: String, reason: String },
}

#[allow(dead_code)]
pub fn resolve_player_intent(
    input: &str,
    allow_generated_action_proposal: bool,
) -> Result<IntentResolution, String> {
    match parse_command(input) {
        Ok(command) => Ok(IntentResolution::Confirmed(command)),
        Err(error) if allow_generated_action_proposal && !input.trim().is_empty() => {
            Ok(IntentResolution::NeedsAdmission {
                player_text: input.trim().to_owned(),
                reason: error,
            })
        }
        Err(error) => Err(error),
    }
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
    if let Some(rest) = lower
        .strip_prefix("attack ")
        .or_else(|| lower.strip_prefix("hit "))
    {
        return parsed_target(trimmed, &lower, rest, "attack", |target| {
            // The player is an ordinary controlled actor. The reducer supplies whichever entity
            // is currently controlled, then validates the hostile target and capability contract.
            GameAction::AttackTarget { target }
        });
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

    Err("I only understand a narrow set of commands right now. Try: help, look, go ..., unlock ..., barricade ..., inspect ..., take ..., equip ..., use ..., attack [target], wait. A named combat-capable actor can also attack a target.".to_owned())
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
    use super::{
        ActionOutcome, EncounterKind, GameAction, GameEvent, IntentResolution, action_traces,
        event_projection, parse_action, parse_command, resolve_player_intent,
    };

    #[test]
    fn action_traces_expose_origin_impact_type_and_outcome() {
        let outcome = ActionOutcome {
            events: vec![
                GameEvent::AttackResolved {
                    origin_id: "lira_voss_1".to_owned(),
                    target_id: "zombie_1".to_owned(),
                    target_kind: EncounterKind::Enemy,
                    damage: 3,
                    defeated: false,
                },
                GameEvent::DamageTaken {
                    origin_id: "zombie_1".to_owned(),
                    target_id: "player".to_owned(),
                    amount: 1,
                    remaining_hp: 9,
                },
            ],
            lines: Vec::new(),
        };
        let traces = action_traces(
            &GameAction::ActorAttack {
                origin_id: "lira_voss_1".to_owned(),
                target: "zombie_1".to_owned(),
            },
            &outcome,
        );
        assert_eq!(traces[0].origin, "lira_voss_1");
        assert_eq!(traces[0].impact, "enemy zombie_1");
        assert_eq!(traces[0].action_type, "combat");
        assert_eq!(traces[1].origin, "zombie_1");
        assert_eq!(traces[1].impact, "player");
        assert!(traces[1].outcome.contains("9 HP"));
    }

    #[test]
    fn autonomous_actor_events_project_without_an_enemy_specific_interpreter() {
        let projection = event_projection(&GameEvent::ActorMoved {
            origin_id: "runtime_actor.backpack".to_owned(),
            from_location_id: "kitchen".to_owned(),
            to_location_id: "laundry".to_owned(),
            target_location_id: "front_verandah".to_owned(),
        });

        assert_eq!(projection.action_type, "movement");
        assert_eq!(projection.origin, "runtime_actor.backpack");
        assert_eq!(projection.impact, "laundry");
        assert_eq!(projection.outcome, "moved");
    }

    #[test]
    fn actor_wait_projects_as_a_generic_wait_not_an_ai_action() {
        let projection = event_projection(&GameEvent::ActorWaited {
            origin_id: "friendly_npc_1".to_owned(),
            location_id: "kitchen".to_owned(),
            reason: "holding position".to_owned(),
        });

        assert_eq!(projection.action_type, "wait");
        assert_eq!(projection.origin, "friendly_npc_1");
        assert_eq!(projection.impact, "kitchen");
        assert_eq!(projection.outcome, "holding position");
    }

    #[test]
    fn controller_issued_legacy_actions_keep_the_actual_actor_in_the_projection() {
        let look = event_projection(&GameEvent::LocationLooked {
            origin_id: "marnie_1".to_owned(),
            location_id: "front_verandah".to_owned(),
        });
        let inspect = event_projection(&GameEvent::Inspected {
            origin_id: "marnie_1".to_owned(),
            target: "rusty_toolbox".to_owned(),
        });
        let wait = event_projection(&GameEvent::Waited {
            origin_id: "marnie_1".to_owned(),
            location_id: "front_verandah".to_owned(),
        });
        let whiff = event_projection(&GameEvent::AttackWhiff {
            origin_id: "marnie_1".to_owned(),
        });

        for projection in [look, inspect, wait, whiff] {
            assert_eq!(projection.origin, "marnie_1");
        }
    }

    #[test]
    fn damage_projection_keeps_the_actual_impacted_actor() {
        let projection = event_projection(&GameEvent::DamageTaken {
            origin_id: "crawler".to_owned(),
            target_id: "marnie".to_owned(),
            amount: 2,
            remaining_hp: 3,
        });

        assert_eq!(projection.action_type, "damage");
        assert_eq!(projection.origin, "crawler");
        assert_eq!(projection.impact, "marnie");
        assert_eq!(projection.outcome, "2 damage; 3 HP remaining");
    }

    #[test]
    fn progress_awards_project_through_the_generic_action_trace() {
        let projection = event_projection(&GameEvent::ProgressAwarded {
            origin_id: "player".to_owned(),
            pool: "necromancy".to_owned(),
            amount: 3,
            total: 10,
        });

        assert_eq!(projection.action_type, "progress");
        assert_eq!(projection.origin, "player");
        assert_eq!(projection.impact, "necromancy");
        assert_eq!(projection.outcome, "3 XP awarded; total 10");
    }

    #[test]
    fn cooldown_and_level_events_share_the_canonical_projection_path() {
        let cooldown = event_projection(&GameEvent::AbilityCooldownSet {
            origin_id: "player".to_owned(),
            grant_id: "fireball_scroll_1".to_owned(),
            template_id: "fireball".to_owned(),
            ready_at_turn: 12,
        });
        assert_eq!(cooldown.action_type, "cooldown");
        assert_eq!(cooldown.origin, "player");
        assert_eq!(cooldown.impact, "fireball");
        assert_eq!(cooldown.outcome, "ready at world turn 12");

        let health = event_projection(&GameEvent::MaxHpIncreased {
            origin_id: "player".to_owned(),
            amount: 2,
            max_hp: 12,
        });
        assert_eq!(health.action_type, "health");
        assert_eq!(health.origin, "player");
        assert_eq!(health.impact, "maximum hp");
        assert_eq!(health.outcome, "increased by 2; maximum 12");
    }

    #[test]
    fn source_consumption_projects_exact_item_authority_without_losing_its_reason() {
        let projection = event_projection(&GameEvent::ItemConsumed {
            origin_id: "player".to_owned(),
            item_id: "authored_item_grave_scroll".to_owned(),
            template_id: "grave_scroll".to_owned(),
            reason: "studied into spectral_spark".to_owned(),
        });

        assert_eq!(projection.action_type, "consumption");
        assert_eq!(projection.origin, "player");
        assert_eq!(projection.impact, "authored_item_grave_scroll");
        assert_eq!(projection.outcome, "studied into spectral_spark");
    }

    #[test]
    fn permissive_intent_resolution_requests_admission_instead_of_inventing_an_action() {
        assert!(matches!(
            resolve_player_intent("ask the moon for a sword", true),
            Ok(IntentResolution::NeedsAdmission { .. })
        ));
    }

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
    fn targeted_player_attack_enters_the_generic_actor_dispatch() {
        assert!(matches!(
            parse_action("attack hostile toolbox"),
            Ok(GameAction::AttackTarget { target }) if target == "hostile toolbox"
        ));
        assert!(matches!(
            parse_action("hit hostile toolbox"),
            Ok(GameAction::AttackTarget { target }) if target == "hostile toolbox"
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
