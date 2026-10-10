use super::actions::{ActionOutcome, GameEvent, event_projection};
use super::state::RunState;

pub(super) const ROLLING_SUMMARY_LIMIT: usize = 24;

pub(super) fn append_rolling_summary(state: &mut RunState, outcome: &ActionOutcome) {
    let summary_lines = rolling_summary_lines(&outcome.events, &outcome.lines);
    state.rolling_summary.extend(summary_lines);
    trim_rolling_summary(&mut state.rolling_summary);
}

fn rolling_summary_lines(events: &[GameEvent], fallback_lines: &[String]) -> Vec<String> {
    let lines = events.iter().map(summarize_event).collect::<Vec<_>>();
    if lines.is_empty() {
        fallback_lines.to_vec()
    } else {
        lines
    }
}

fn trim_rolling_summary(summary: &mut Vec<String>) {
    if summary.len() > ROLLING_SUMMARY_LIMIT {
        let remove_count = summary.len() - ROLLING_SUMMARY_LIMIT;
        summary.drain(0..remove_count);
    }
}

/// A rolling summary is a direct rendering of the host-owned event projection. It must not grow
/// into a second event interpreter with its own causal semantics.
fn summarize_event(event: &GameEvent) -> String {
    let projection = event_projection(event);
    format!(
        "Action [{}] {} -> {}: {}",
        projection.action_type, projection.origin, projection.impact, projection.outcome
    )
}
