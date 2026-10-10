//! Small, host-owned social substrate shared by every actor-shaped entity.
//!
//! Personality is deliberately data (tags), not a property of an NPC noun.  Consumers such as
//! gifts, future trade, and quest negotiation all ask this module for the same thresholds.

use serde::{Deserialize, Serialize};

use super::state::ActorDisposition;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SocialEventKind {
    Gift,
    Help,
    Insult,
    Threat,
    Betrayal,
    Theft,
    ReturnItem,
}

impl SocialEventKind {
    pub fn relationship_delta(self) -> i32 {
        match self {
            Self::Gift | Self::Help | Self::ReturnItem => 1,
            Self::Insult => -1,
            Self::Threat | Self::Theft => -2,
            Self::Betrayal => -3,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SocialState {
    /// A signed, reversible relationship measure toward the controlled actor.
    #[serde(default)]
    pub relationship_score: i32,
    /// Old saves have no relationship history.  On first contact we seed the score from the
    /// entity's authored disposition, preserving an authored hostile/friendly starting stance.
    #[serde(default)]
    pub initialized: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SocialThresholds {
    pub positive: i32,
    pub negative: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PersonalityArchetype {
    id: &'static str,
    positive_modifier: i32,
    negative_modifier: i32,
}

const PERSONALITY_ARCHETYPES: &[PersonalityArchetype] = &[
    PersonalityArchetype {
        id: "cheerful",
        positive_modifier: -1,
        negative_modifier: -1,
    },
    PersonalityArchetype {
        id: "dour",
        positive_modifier: 1,
        negative_modifier: 0,
    },
    PersonalityArchetype {
        id: "trusting",
        positive_modifier: -1,
        negative_modifier: -1,
    },
    PersonalityArchetype {
        id: "suspicious",
        positive_modifier: 1,
        negative_modifier: 1,
    },
    PersonalityArchetype {
        id: "stubborn",
        positive_modifier: 2,
        negative_modifier: -2,
    },
    PersonalityArchetype {
        id: "volatile",
        positive_modifier: -1,
        negative_modifier: 1,
    },
];

/// Returns the subset of ordinary descriptive tags which the host currently interprets as
/// personality data. UI and future mechanics call this registry-backed helper rather than owning
/// their own personality vocabulary.
pub fn personality_tags(tags: &[String]) -> Vec<String> {
    tags.iter()
        .filter(|tag| {
            PERSONALITY_ARCHETYPES
                .iter()
                .any(|archetype| archetype.id == tag.trim().to_ascii_lowercase())
        })
        .cloned()
        .collect()
}

/// The one personality allowance calculation.  Tags are intentionally read at application time:
/// changing a tag changes future social mechanics without changing the entity's Rust type.
pub fn thresholds_for_tags(tags: &[String]) -> SocialThresholds {
    let mut positive = 3;
    let mut negative = -3;
    for tag in tags {
        if let Some(archetype) = PERSONALITY_ARCHETYPES
            .iter()
            .find(|archetype| archetype.id == tag.trim().to_ascii_lowercase())
        {
            positive += archetype.positive_modifier;
            negative += archetype.negative_modifier;
        }
    }
    SocialThresholds {
        positive: positive.clamp(1, 8),
        negative: negative.clamp(-8, -1),
    }
}

pub fn initial_score(disposition: &ActorDisposition, thresholds: SocialThresholds) -> i32 {
    match disposition {
        ActorDisposition::Friendly => thresholds.positive,
        ActorDisposition::Neutral => 0,
        ActorDisposition::Hostile => thresholds.negative,
    }
}

pub fn disposition_for_score(score: i32, thresholds: SocialThresholds) -> ActorDisposition {
    if score >= thresholds.positive {
        ActorDisposition::Friendly
    } else if score <= thresholds.negative {
        ActorDisposition::Hostile
    } else {
        ActorDisposition::Neutral
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SocialOutcome {
    pub kind: SocialEventKind,
    pub delta: i32,
    pub relationship_score: i32,
    pub disposition: ActorDisposition,
}

pub fn apply_event(
    state: &mut SocialState,
    initial_disposition: &ActorDisposition,
    tags: &[String],
    kind: SocialEventKind,
) -> SocialOutcome {
    let thresholds = thresholds_for_tags(tags);
    if !state.initialized {
        state.relationship_score = initial_score(initial_disposition, thresholds);
        state.initialized = true;
    }
    let delta = kind.relationship_delta();
    state.relationship_score = (state.relationship_score + delta).clamp(-100, 100);
    SocialOutcome {
        kind,
        delta,
        relationship_score: state.relationship_score,
        disposition: disposition_for_score(state.relationship_score, thresholds),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personality_tags_change_the_shared_thresholds() {
        assert!(
            thresholds_for_tags(&["trusting".to_owned()]).positive
                < thresholds_for_tags(&["suspicious".to_owned()]).positive
        );
    }

    #[test]
    fn relationship_can_move_in_both_directions() {
        let mut state = SocialState::default();
        let tags = Vec::new();
        for _ in 0..3 {
            apply_event(
                &mut state,
                &ActorDisposition::Neutral,
                &tags,
                SocialEventKind::Gift,
            );
        }
        assert_eq!(state.relationship_score, 3);
        for _ in 0..4 {
            apply_event(
                &mut state,
                &ActorDisposition::Neutral,
                &tags,
                SocialEventKind::Insult,
            );
        }
        assert_eq!(state.relationship_score, -1);
    }
}
