//! Host-owned vocabulary for mechanical entity capabilities.
//!
//! Datapacks may *select* these names and provide their documented parameters, but cannot add
//! a private capability with private reducer semantics.  The reducer remains the sole state
//! authority.  Keeping the vocabulary here makes authored templates and runtime proposals use
//! the same contract.

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum EngineCapability {
    Health,
    Combat,
    Hearing,
    Sight,
    Movement,
    Inventory,
    NoiseSpawn,
    NoiseTrack,
    SightTrack,
}

/// A field whose applicability is owned by a capability rather than an entity noun. Future
/// capability families add their dependent fields here; templates, proposal forms, UI, and
/// mutation validation all ask this registry whether a field is applicable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityDependentField {
    MaxHp,
    ActorDamage,
}

impl CapabilityDependentField {
    pub const fn proposal_field(self) -> &'static str {
        match self {
            Self::MaxHp => "MAX_HP",
            Self::ActorDamage => "ACTOR_DAMAGE",
        }
    }

    pub const fn ui_label(self) -> &'static str {
        match self {
            Self::MaxHp => "Max HP",
            Self::ActorDamage => "Actor damage",
        }
    }
}

impl EngineCapability {
    pub const ALL: [Self; 9] = [
        Self::Health,
        Self::Combat,
        Self::Hearing,
        Self::Sight,
        Self::Movement,
        Self::Inventory,
        Self::NoiseSpawn,
        Self::NoiseTrack,
        Self::SightTrack,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Health => "health",
            Self::Combat => "combat",
            Self::Hearing => "hearing",
            Self::Sight => "sight",
            Self::Movement => "movement",
            Self::Inventory => "inventory",
            Self::NoiseSpawn => "noise_spawn",
            Self::NoiseTrack => "noise_track",
            Self::SightTrack => "sight_track",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            // Accepted only for legacy packs; new content uses `movement`.
            "mobile" => Some(Self::Movement),
            value => Self::ALL
                .into_iter()
                .find(|capability| capability.id() == value),
        }
    }

    /// The questionnaire/UI root controlled by this capability. The root is a real schema
    /// transition: only when it is enabled do its dependent fields become applicable.
    pub const fn proposal_root_field(self) -> &'static str {
        match self {
            Self::Health => "HAS_HEALTH",
            Self::Combat => "CAN_ENGAGE",
            Self::Hearing => "CAN_HEAR",
            Self::Sight => "CAN_SEE",
            Self::Movement => "CAN_MOVE",
            Self::Inventory => "CAN_CARRY_INVENTORY",
            Self::NoiseSpawn => "CAN_SPAWN_FROM_NOISE",
            Self::NoiseTrack => "CAN_TRACK_NOISE",
            Self::SightTrack => "CAN_TRACK_SIGHT",
        }
    }

    pub const fn ui_label(self) -> &'static str {
        match self {
            Self::Health => "Has health",
            Self::Combat => "Can engage enemies",
            Self::Hearing => "Can hear",
            Self::Sight => "Can see",
            Self::Movement => "Can move",
            Self::Inventory => "Can carry inventory",
            Self::NoiseSpawn => "Can spawn from noise",
            Self::NoiseTrack => "Can track noise",
            Self::SightTrack => "Can track sight",
        }
    }

    pub const fn prerequisites(self) -> &'static [Self] {
        match self {
            Self::Combat => &[Self::Health],
            Self::NoiseSpawn => &[Self::Hearing],
            Self::NoiseTrack => &[Self::Hearing, Self::Movement],
            Self::SightTrack => &[Self::Sight, Self::Movement],
            _ => &[],
        }
    }

    pub const fn dependent_fields(self) -> &'static [CapabilityDependentField] {
        match self {
            Self::Health => &[CapabilityDependentField::MaxHp],
            Self::Combat => &[CapabilityDependentField::ActorDamage],
            _ => &[],
        }
    }
}

/// The generic, capability-gated section of an item/NPC runtime proposal. Entity-specific
/// identity fields stay with their template; this list is exclusively owned by host capability
/// schemas.
pub fn actor_proposal_fields() -> Vec<&'static str> {
    EngineCapability::ALL
        .into_iter()
        .flat_map(|capability| {
            std::iter::once(capability.proposal_root_field()).chain(
                capability
                    .dependent_fields()
                    .iter()
                    .map(|field| field.proposal_field()),
            )
        })
        .collect()
}

impl fmt::Display for EngineCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id())
    }
}

/// Validates a capability selection and its engine-owned prerequisites.  This is intentionally
/// shared by every template family: type names never grant mechanics.
#[allow(clippy::too_many_arguments)]
pub fn validate_selection(
    template_kind: &str,
    id: &str,
    capabilities: &[String],
    has_actor_state: bool,
    has_health: bool,
    has_damage: bool,
    can_hear: bool,
    can_see: bool,
    can_move: bool,
    errors: &mut Vec<String>,
) {
    for capability in capabilities {
        if EngineCapability::parse(capability).is_none() {
            errors.push(format!(
                "{template_kind} '{id}' selects unknown host capability '{capability}'."
            ));
        }
    }
    // Keep validation and runtime lookup on the same vocabulary. In particular, `mobile` is a
    // read-only legacy spelling for `movement`; accepting it in `parse` but not here made an old
    // pack validate a different contract from the one the actor runtime observed.
    let has = |capability: EngineCapability| contains(capabilities, capability);
    if !has_actor_state && !capabilities.is_empty() {
        errors.push(format!(
            "{template_kind} '{id}' selects host capabilities but has no actor configuration."
        ));
    }
    if has(EngineCapability::Health) && !has_health {
        errors.push(format!(
            "{template_kind} '{id}' selects health but does not define actor.hp."
        ));
    }
    if has(EngineCapability::Combat) && (!has_health || !has_damage) {
        errors.push(format!(
            "{template_kind} '{id}' selects combat but requires actor.hp and actor.damage."
        ));
    }
    for capability in EngineCapability::ALL {
        if !has(capability) {
            continue;
        }
        let missing = capability
            .prerequisites()
            .iter()
            .filter_map(|prerequisite| {
                let present = match prerequisite {
                    // A raw legacy field can satisfy an old implicit contract only when there
                    // is no selected envelope.  Inside an explicit capability tree the parent
                    // root itself must be selected as well, or a child branch would remain
                    // executable after its parent checkbox was turned off.
                    EngineCapability::Health => has(EngineCapability::Health) && has_health,
                    EngineCapability::Hearing => has(EngineCapability::Hearing) && can_hear,
                    EngineCapability::Sight => has(EngineCapability::Sight) && can_see,
                    EngineCapability::Movement => has(EngineCapability::Movement) && can_move,
                    EngineCapability::Inventory => has(EngineCapability::Inventory),
                    EngineCapability::Combat => has(EngineCapability::Combat),
                    EngineCapability::NoiseSpawn => has(EngineCapability::NoiseSpawn),
                    EngineCapability::NoiseTrack => has(EngineCapability::NoiseTrack),
                    EngineCapability::SightTrack => has(EngineCapability::SightTrack),
                };
                (!present).then_some(prerequisite.id())
            })
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            errors.push(format!(
                "{template_kind} '{id}' selects {} but requires {}.",
                capability.id(),
                missing.join(" and ")
            ));
        }
    }
}

pub fn contains(capabilities: &[String], capability: EngineCapability) -> bool {
    capabilities
        .iter()
        .any(|candidate| EngineCapability::parse(candidate) == Some(capability))
}

#[cfg(test)]
mod tests {
    use super::{EngineCapability, actor_proposal_fields, contains, validate_selection};

    #[test]
    fn registry_owns_capability_roots_prerequisites_and_dependent_fields() {
        assert_eq!(EngineCapability::Combat.proposal_root_field(), "CAN_ENGAGE");
        assert_eq!(
            EngineCapability::Combat.prerequisites(),
            &[EngineCapability::Health]
        );
        assert_eq!(
            EngineCapability::Combat.dependent_fields()[0].proposal_field(),
            "ACTOR_DAMAGE"
        );
        let fields = actor_proposal_fields();
        assert!(fields.contains(&"HAS_HEALTH"));
        assert!(fields.contains(&"MAX_HP"));
        assert!(fields.contains(&"CAN_ENGAGE"));
        assert!(fields.contains(&"ACTOR_DAMAGE"));
    }

    #[test]
    fn registry_accepts_only_host_capabilities_and_normalizes_legacy_movement() {
        assert_eq!(
            EngineCapability::parse("mobile"),
            Some(EngineCapability::Movement)
        );
        assert!(contains(
            &["movement".to_owned()],
            EngineCapability::Movement
        ));
        assert_eq!(EngineCapability::parse("pack_private_magic"), None);
    }

    #[test]
    fn legacy_mobile_uses_the_same_validation_path_as_movement() {
        let mut errors = Vec::new();
        validate_selection(
            "Item",
            "legacy_runner",
            &["mobile".to_owned()],
            true,
            false,
            false,
            false,
            false,
            true,
            &mut errors,
        );
        assert!(
            errors.is_empty(),
            "legacy movement should be canonicalized: {errors:?}"
        );
    }

    #[test]
    fn capability_prerequisites_are_host_validated() {
        let mut errors = Vec::new();
        validate_selection(
            "Item",
            "restless_case",
            &["noise_track".to_owned()],
            true,
            true,
            true,
            false,
            false,
            false,
            &mut errors,
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("requires hearing and movement"))
        );
    }

    #[test]
    fn child_capabilities_require_their_selected_parent_roots_not_only_payload_values() {
        let mut errors = Vec::new();
        validate_selection(
            "NPC",
            "stale_contract",
            &["combat".to_owned(), "noise_track".to_owned()],
            true,
            true,
            true,
            true,
            false,
            true,
            &mut errors,
        );

        assert!(
            errors
                .iter()
                .any(|error| error.contains("selects combat but requires health")),
            "combat cannot use stale HP without the health root: {errors:?}"
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("selects noise_track but requires hearing")),
            "tracking cannot use stale hearing without the hearing root: {errors:?}"
        );
    }
}
