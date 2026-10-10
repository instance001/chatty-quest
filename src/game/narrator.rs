use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Mutex;
use std::time::Duration;

use crate::data::datapacks::DatapackBundle;
use crate::game::PlayContract;
use crate::game::actions::EncounterKind;
use crate::game::capabilities::actor_proposal_fields;
use crate::game::derived::run_phase_label;
use crate::game::queries::{enemy_template_id, find_enemy};

use super::{ActionOutcome, GameAction, GameEvent, RunState};

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize)]
pub struct NarratorContext {
    pub play_contract: PlayContract,
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
    pub session_truth: Vec<String>,
    pub dm_style: Option<String>,
    pub world_tone: Option<String>,
    pub safety_rules: Vec<String>,
    /// Immutable ability ids the host has made available to the bounded grant form.  The
    /// narrator may select one, but can never invent an ability definition.
    pub grantable_ability_ids: Vec<String>,
}

/// Host-owned blank mechanic templates that a narrator may fill for one run. Their output is
/// still only a proposal until the player admits it and the reducer validates it.
pub fn allowed_proposal_kinds(contract: &PlayContract, chaos: f32) -> Vec<String> {
    if !contract.allow_generated_persistent_content {
        return Vec::new();
    }
    let kinds: &[&str] = if matches!(
        contract.preset,
        crate::game::play_contract::PlayPreset::Unleashed
    ) || chaos >= 0.85
    {
        &[
            "item",
            "status",
            "npc",
            "enemy",
            "boss",
            "objective",
            "temporary_exit",
            "ability_grant",
        ]
    } else if contract.allow_generated_mechanical_effects && chaos >= 0.60 {
        &[
            "item",
            "status",
            "npc",
            "enemy",
            "objective",
            "temporary_exit",
            "ability_grant",
        ]
    } else if chaos >= 0.34 {
        &["item", "npc", "objective", "temporary_exit"]
    } else {
        &["item"]
    };
    kinds.iter().map(|kind| (*kind).to_owned()).collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FreeTextProposalDecision {
    pub probability_percent: u8,
    pub eligible: bool,
    pub permitted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmbientProposalDecision {
    pub probability_percent: u8,
    pub cooldown_turns: u64,
    pub permitted: bool,
}

/// A rare, host-owned discovery opportunity. It is independent of player phrasing: only the host
/// evaluates the persisted cooldown and deterministic chance.
pub fn decide_ambient_proposal(
    contract: &PlayContract,
    chaos: f32,
    turn_index: u64,
    last_ambient_proposal_turn: Option<u64>,
) -> AmbientProposalDecision {
    let (probability_percent, cooldown_turns) =
        if !contract.allow_generated_persistent_content || chaos < 0.17 {
            (0, 12)
        } else if chaos < 0.50 {
            (2, 12)
        } else if chaos < 0.84 {
            (4, 8)
        } else {
            (6, 6)
        };
    let since_last = last_ambient_proposal_turn
        .map(|last_turn| turn_index.saturating_sub(last_turn))
        .unwrap_or(turn_index);
    let roll = turn_index.wrapping_mul(1_103_515_245).wrapping_add(12_345) % 100;
    AmbientProposalDecision {
        probability_percent,
        cooldown_turns,
        permitted: since_last >= cooldown_turns && roll < u64::from(probability_percent),
    }
}

/// The host—not the narrator—decides whether a free-text turn is mechanically proposal-worthy.
/// This is intentionally a small, inspectable intent boundary: conversational expression may be
/// narrated, but only constructive/discovery intent can open an editable mechanic form.
pub fn is_proposal_eligible_free_text(player_text: &str) -> bool {
    let normalized = player_text.trim().to_ascii_lowercase();
    let intent = normalized
        .strip_prefix("i ")
        .or_else(|| normalized.strip_prefix("we "))
        .unwrap_or(&normalized);
    [
        "search",
        "rummage",
        "look for",
        "find ",
        "seek ",
        "investigate",
        "scavenge",
        "explore",
        "pray ",
        "request ",
        "build ",
        "repair ",
        "summon ",
        "create ",
        "open ",
    ]
    .iter()
    .any(|prefix| intent.starts_with(prefix))
}

pub fn decide_free_text_proposals(
    contract: &PlayContract,
    chaos: f32,
    turn_index: u64,
    player_text: &str,
) -> FreeTextProposalDecision {
    let eligible = is_proposal_eligible_free_text(player_text);
    let probability_percent = if !contract.allow_generated_persistent_content || chaos < 0.17 {
        0
    } else if chaos < 0.50 {
        25
    } else if chaos < 0.84 {
        60
    } else {
        100
    };
    let roll = player_text
        .bytes()
        .fold(turn_index.wrapping_add(0x811c_9dc5), |hash, byte| {
            hash.wrapping_mul(16_777_619) ^ u64::from(byte)
        })
        % 100;
    FreeTextProposalDecision {
        probability_percent,
        eligible,
        permitted: eligible && roll < u64::from(probability_percent),
    }
}

/// Selects one authorized host template reproducibly. The narrator receives this selected kind
/// only; it never gets to route a vague utterance into the most tempting mechanic type.
pub fn select_host_proposal_kind(
    allowed_kinds: &[String],
    turn_index: u64,
    player_text: &str,
) -> Option<String> {
    (!allowed_kinds.is_empty()).then(|| {
        let hash = player_text
            .bytes()
            .fold(turn_index.wrapping_add(0x9e37_79b9), |value, byte| {
                value.wrapping_mul(1_099_511_628_211) ^ u64::from(byte)
            });
        allowed_kinds[(hash as usize) % allowed_kinds.len()].clone()
    })
}

/// Minimal host-owned blanks presented to the narrator after it selects one authorized kind.
/// Optional internal fields use reducer-safe defaults and are never guessed from narration.
fn proposal_form_fields(kind: &str) -> Option<Vec<&'static str>> {
    let mut fields = match kind {
        "item" => vec!["NAME", "DESCRIPTION", "DAMAGE", "TAGS"],
        // Status effects are a real, bounded mechanical contract. Do not admit a
        // prose-only status: expose the values and applicability gates the reducer evaluates.
        "status" => vec![
            "NAME",
            "TRIGGER",
            "DURATION",
            "ADDITIVE_DAMAGE",
            "ADDITIVE_HEALING",
            "TARGET_TAGS",
            "CONTEXT_TAGS",
        ],
        "npc" => vec!["NAME", "DESCRIPTION", "FACTION", "TAGS"],
        "enemy" | "boss" => vec!["NAME", "DESCRIPTION", "MAX_HP", "DAMAGE", "TAGS"],
        "objective" => vec!["NAME", "DESCRIPTION", "COMPLETION_KEY"],
        "temporary_exit" => vec![
            "NAME",
            "DESCRIPTION",
            "FROM_LOCATION_ID",
            "TO_LOCATION_ID",
            "LOCKED",
        ],
        "ability_grant" => vec!["TEMPLATE_ID", "REMAINING_USES", "DURATION_TURNS"],
        _ => return None,
    };
    if matches!(kind, "item" | "npc") {
        fields.extend(actor_proposal_fields());
        fields.push("DISPOSITION");
    }
    Some(fields)
}

fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn proposal_form_field_line(field: &str) -> String {
    let hint = match field {
        "DAMAGE" | "ACTOR_DAMAGE" | "MAX_HP" | "ADDITIVE_DAMAGE" | "ADDITIVE_HEALING" => {
            "whole number -999 to 999"
        }
        "TRIGGER" => "one of combat, movement, inspect, wait, failure",
        "DURATION" => "one of scene, until_used, run",
        "LOCKED"
        | "HAS_HEALTH"
        | "CAN_ENGAGE"
        | "CAN_HEAR"
        | "CAN_SEE"
        | "CAN_MOVE"
        | "CAN_CARRY_INVENTORY"
        | "CAN_SPAWN_FROM_NOISE"
        | "CAN_TRACK_NOISE"
        | "CAN_TRACK_SIGHT" => "true or false",
        "DISPOSITION" => "friendly, neutral, or hostile",
        "TAGS" | "TARGET_TAGS" | "CONTEXT_TAGS" => "comma-separated short tags, or none",
        "REMAINING_USES" => "a whole number 1 to 999, or infinite",
        "DURATION_TURNS" => "a whole number 1 to 999, or permanent",
        _ => "short plain text",
    };
    format!("{field}: <{hint}>")
}

fn proposal_form_request(
    context: &NarratorContext,
    player_message: Option<&str>,
    kind: &str,
    retry: bool,
) -> String {
    let fields = proposal_form_fields(kind).unwrap_or_default();
    let retry_note = retry.then_some(
        "RETRY: The prior form was invalid. Output every requested label exactly once and nothing else.\n",
    );
    format!(
        "COCKPIT/PROPOSAL-FORM\n{}HOST AUTHORITY: fill values only; the host validates and the player decides.\nKIND: {}\nLOCATION: {}\nLOCATION DESCRIPTION: {}\nOPPORTUNITY: {}\nABILITY IDS (only when KIND is ability_grant): {}\nRESPONSE: output only these field-name/value lines, once each, replacing the angle-bracket hint with a value:\n{}\nMake NAME and DESCRIPTION fields, when present, specific to this location and opportunity. Do not reuse a proper name or description from an unrelated entity. TAGS are short factual descriptors, not prose. For a status, combat may add damage only; movement, inspect, wait, and failure may add healing only; do not submit an all-zero status. An ability grant must use exactly one listed immutable TEMPLATE_ID; it does not define a spell, owner, effect, or new mechanic. Never invent a field: CAN_SPEAK is not valid unless shown, and CAN_ATTACK is only a synonym for CAN_ENGAGE when CAN_ENGAGE is shown. These capability choices are the complete proposed mechanical contract, so choose each deliberately. Do not output LABEL, FIELD, TEMPLATE, KIND, headings, markdown, JSON, or commentary.",
        retry_note.unwrap_or_default(),
        kind,
        one_line(&context.current_location_name),
        one_line(&context.current_location_description),
        player_message.map(one_line).unwrap_or_else(|| {
            "A rare host-selected ambient discovery in the current location.".to_owned()
        }),
        context.grantable_ability_ids.join(", "),
        fields
            .iter()
            .map(|field| proposal_form_field_line(field))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize)]
pub struct NarratorExitContext {
    pub location_id: String,
    pub name: String,
    pub locked: bool,
    pub barricaded: bool,
}

/// Live narrator work may run away from the UI thread. Implementations must therefore be safe to
/// move into the app's single in-flight narration worker and back again after it has produced its
/// presentation-only result.
pub trait Narrator: Send {
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
        host_selected_proposal_kind: Option<&str>,
    ) -> Vec<String>;

    /// Presentation-only player chat. It never receives a reducer action and cannot mutate state.
    fn narrate_player_message(
        &self,
        _: &DatapackBundle,
        _: &RunState,
        _: &str,
        _: Option<&str>,
    ) -> Vec<String> {
        Vec::new()
    }

    /// Returns schema-validated offers emitted by the narrator since the last call.  These are
    /// deliberately display-only until the reducer receives an explicit admission action.
    fn take_item_proposals(&self) -> Vec<ValidatedItemProposal> {
        Vec::new()
    }

    fn take_status_proposals(&self) -> Vec<ValidatedStatusProposal> {
        Vec::new()
    }

    fn take_npc_proposals(&self) -> Vec<ValidatedNpcProposal> {
        Vec::new()
    }

    fn take_enemy_proposals(&self) -> Vec<ValidatedEnemyProposal> {
        Vec::new()
    }
    #[allow(dead_code)]
    fn take_boss_proposals(&self) -> Vec<ValidatedBossProposal> {
        Vec::new()
    }

    fn take_objective_proposals(&self) -> Vec<ValidatedObjectiveProposal> {
        Vec::new()
    }
    fn take_temporary_exit_proposals(&self) -> Vec<ValidatedTemporaryExitProposal> {
        Vec::new()
    }

    fn take_ability_grant_proposals(&self) -> Vec<ValidatedAbilityGrantProposal> {
        Vec::new()
    }

    /// Turn-local diagnostic detail for offers discarded at the narrator boundary.
    fn take_proposal_diagnostics(&self) -> Vec<String> {
        Vec::new()
    }

    fn status(&self) -> NarratorStatus {
        NarratorStatus::ready("Narrator ready")
    }
}

pub struct UnavailableNarrator {
    detail: String,
}

impl UnavailableNarrator {
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }

    fn line(&self) -> Vec<String> {
        vec![format!(
            "Narrator unavailable: {}. Choose Mock or Off to continue without live narration.",
            self.detail
        )]
    }
}

impl Narrator for UnavailableNarrator {
    fn narrate_run_start(&self, _: &DatapackBundle, _: &RunState, _: &[String]) -> Vec<String> {
        self.line()
    }

    fn narrate_action(
        &self,
        _: &DatapackBundle,
        _: &GameAction,
        _: &ActionOutcome,
        _: &RunState,
        _: Option<&str>,
    ) -> Vec<String> {
        self.line()
    }

    fn status(&self) -> NarratorStatus {
        NarratorStatus::unavailable(self.detail.clone())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NarratorStatusKind {
    Ready,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NarratorStatus {
    pub kind: NarratorStatusKind,
    pub label: String,
    pub detail: String,
}

impl NarratorStatus {
    pub fn ready(detail: impl Into<String>) -> Self {
        Self {
            kind: NarratorStatusKind::Ready,
            label: "Narrator: Ready".to_owned(),
            detail: detail.into(),
        }
    }

    pub fn unavailable(detail: impl Into<String>) -> Self {
        Self {
            kind: NarratorStatusKind::Unavailable,
            label: "Narrator: Unavailable".to_owned(),
            detail: detail.into(),
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NarrationHook {
    Routine,
    Notable,
    Major,
    Critical,
    Inspection,
    Summary,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NarrationPreferences {
    pub routine: bool,
    pub notable: bool,
    pub major: bool,
    pub critical: bool,
    pub inspection: bool,
    pub summary: bool,
}

impl Default for NarrationPreferences {
    fn default() -> Self {
        Self {
            routine: true,
            notable: true,
            major: true,
            critical: true,
            inspection: true,
            summary: true,
        }
    }
}

impl NarrationPreferences {
    pub fn allows(&self, hook: NarrationHook) -> bool {
        match hook {
            NarrationHook::Routine => self.routine,
            NarrationHook::Notable => self.notable,
            NarrationHook::Major => self.major,
            NarrationHook::Critical => self.critical,
            NarrationHook::Inspection => self.inspection,
            NarrationHook::Summary => self.summary,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NarrationRoute {
    None,
    Mock,
    SelectedNarrator,
}

pub fn narration_hook_for_action(action: &GameAction, outcome: &ActionOutcome) -> NarrationHook {
    if matches!(action, GameAction::Inspect { .. } | GameAction::Look) {
        return NarrationHook::Inspection;
    }
    if outcome
        .events
        .iter()
        .any(|event| matches!(event, GameEvent::RunWon | GameEvent::RunLost { .. }))
    {
        return NarrationHook::Critical;
    }
    if outcome.events.iter().any(|event| {
        matches!(
            event,
            GameEvent::ObjectiveCompleted { .. } | GameEvent::LocationUnlocked { .. }
        )
    }) {
        return NarrationHook::Major;
    }
    if outcome
        .events
        .iter()
        .any(|event| matches!(event, GameEvent::Moved { .. } | GameEvent::ItemTaken { .. }))
    {
        return NarrationHook::Notable;
    }
    NarrationHook::Routine
}

pub fn narration_route(bundle: &DatapackBundle, hook: NarrationHook) -> NarrationRoute {
    if bundle.rules.narration.usage == "unused" {
        return NarrationRoute::None;
    }
    let route = match hook {
        NarrationHook::Routine => &bundle.rules.narration.routine,
        NarrationHook::Notable => &bundle.rules.narration.notable,
        NarrationHook::Major => &bundle.rules.narration.major,
        NarrationHook::Critical => &bundle.rules.narration.critical,
        NarrationHook::Inspection => &bundle.rules.narration.inspection,
        NarrationHook::Summary => &bundle.rules.narration.summary,
    };
    match route.trim() {
        "none" => NarrationRoute::None,
        "mock" => NarrationRoute::Mock,
        _ => NarrationRoute::SelectedNarrator,
    }
}

pub trait NarratorTransport: Send + Sync {
    fn complete(&self, bounded_request_json: &str) -> Result<String, String>;
}

/// Minimal dependency-free transport for a local HTTP narrator service. The service receives a
/// bounded NarratorContext JSON document and must return a NarratorResponse JSON document.
pub struct LocalHttpNarratorTransport {
    endpoint: String,
    timeout: Duration,
}

/// Adapter for llama.cpp's OpenAI-compatible `/v1/chat/completions` endpoint. It only returns
/// the model's final content; that content must still pass `parse_narrator_response` before the
/// live adapter displays prose or proposals.
pub struct LlamaCppNarratorTransport {
    endpoint: String,
    timeout: Duration,
}

impl LlamaCppNarratorTransport {
    pub fn new(endpoint: impl Into<String>, timeout: Duration) -> Self {
        Self {
            endpoint: endpoint.into(),
            timeout,
        }
    }

    pub fn is_available(endpoint: &str, timeout: Duration) -> bool {
        let Ok((host, port, _)) = parse_http_endpoint(endpoint) else {
            return false;
        };
        let Ok(mut addresses) = format!("{host}:{port}").to_socket_addrs() else {
            return false;
        };
        let Some(address) = addresses.next() else {
            return false;
        };
        TcpStream::connect_timeout(&address, timeout).is_ok()
    }

    /// llama.cpp accepts TCP connections before it is necessarily able to serve a completion.
    /// Its health route reports a non-success response while model slots are still unavailable,
    /// so use it for startup readiness rather than treating an open socket as sufficient.
    pub fn is_ready(endpoint: &str, timeout: Duration) -> bool {
        let Ok((host, port, _)) = parse_http_endpoint(endpoint) else {
            return false;
        };
        let Ok(mut addresses) = format!("{host}:{port}").to_socket_addrs() else {
            return false;
        };
        let Some(address) = addresses.next() else {
            return false;
        };
        let Ok(mut stream) = TcpStream::connect_timeout(&address, timeout) else {
            return false;
        };
        if stream.set_read_timeout(Some(timeout)).is_err()
            || stream.set_write_timeout(Some(timeout)).is_err()
        {
            return false;
        }
        let request = format!("GET /health HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
        if stream.write_all(request.as_bytes()).is_err() {
            return false;
        }
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .is_ok_and(|_| response.starts_with("HTTP/1.1 2") || response.starts_with("HTTP/1.0 2"))
    }
}

impl NarratorTransport for LlamaCppNarratorTransport {
    fn complete(&self, bounded_request_json: &str) -> Result<String, String> {
        let (system, max_tokens) = narrator_prompt(bounded_request_json);
        let user = bounded_request_json;
        let request = serde_json::json!({
            "messages": [{"role":"system","content":system}, {"role":"user","content":user}],
            "temperature": 0.7,
            "max_tokens": max_tokens,
            "stream": false,
        });
        let body = serde_json::to_string(&request).map_err(|error| error.to_string())?;
        let raw = complete_local_http(&self.endpoint, self.timeout, &body)?;
        let response: LlamaCppCompletion = serde_json::from_str(&raw)
            .map_err(|error| format!("llama.cpp returned invalid completion JSON: {error}"))?;
        response
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .filter(|content| !content.trim().is_empty())
            .ok_or_else(|| "llama.cpp completion did not include final content.".to_owned())
    }
}

fn narrator_prompt(request: &str) -> (&'static str, u32) {
    if request.starts_with("COCKPIT/PROPOSAL-FORM\n") {
        (
            "COCKPIT PROTOCOL: The host owns game state and the player approves every mechanic. Fill only the requested field names with values. Every requested line is required. Capability fields must be true or false; DISPOSITION must be friendly, neutral, or hostile. Return no headings and never output LABEL:, FIELD:, TEMPLATE:, or KIND:. Do not use JSON, markdown, braces, lists, narration, explanations, or extra fields.",
            // A full host-owned actor checklist is still flat, but needs enough room for every
            // required line so weak local models are not cut off mid-form.
            192,
        )
    } else {
        (
            "COCKPIT PROTOCOL: The host owns game state. You only present it. Reply naturally as a game narrator. Treat player_message as the player's utterance. Return plain prose only: no JSON, labels, rules explanation, proposal, or claim that state changed. For a reducer turn, use the documented reducer_lines and event_facts to describe the cause and actor of an outcome; never respond with an HP or damage number alone. Never narrate that an NPC attacks, gives, throws, equips, creates, removes, or transfers an item, entity, route, quest, or status unless that exact mechanical action appears in the supplied context. Never name, describe, promise, or imply an item unless it appears in the supplied authoritative inventory. If no owned offer or inventory is supplied, narrate uncertainty, refusal, or that they have nothing to give—never invent a substitute object. Do not repeat context, prompts, schemas, or instructions. Keep it to two sentences.",
            160,
        )
    }
}

/// Authenticated, provider-agnostic cloud narrator transport.  API keys live only in memory;
/// callers pass the provider-specific wire format while the narrator boundary stays identical.
pub struct CloudNarratorTransport {
    provider: String,
    model: String,
    endpoint: String,
    api_key: String,
    client: reqwest::blocking::Client,
}

impl CloudNarratorTransport {
    pub fn new(
        provider: impl Into<String>,
        model: impl Into<String>,
        endpoint: impl Into<String>,
        api_key: impl Into<String>,
        timeout: Duration,
    ) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
            endpoint: endpoint.into(),
            api_key: api_key.into(),
            client: reqwest::blocking::Client::builder()
                .timeout(timeout)
                .build()
                .expect("cloud narrator HTTP client should initialize"),
        }
    }

    fn send_json(
        &self,
        request: reqwest::blocking::RequestBuilder,
    ) -> Result<serde_json::Value, String> {
        let response = request
            .send()
            .map_err(|error| format!("Narrator cloud request failed: {error}"))?;
        let status = response.status();
        let body = response
            .text()
            .map_err(|error| format!("Narrator cloud response could not be read: {error}"))?;
        if !status.is_success() {
            let detail = body.chars().take(400).collect::<String>();
            return Err(format!(
                "Narrator cloud endpoint returned HTTP {status}: {detail}"
            ));
        }
        serde_json::from_str(&body)
            .map_err(|error| format!("Narrator cloud endpoint returned invalid JSON: {error}"))
    }
}

impl NarratorTransport for CloudNarratorTransport {
    fn complete(&self, bounded_request: &str) -> Result<String, String> {
        let (system, max_tokens) = narrator_prompt(bounded_request);
        let provider = self.provider.as_str();
        match provider {
            "Anthropic" => {
                let payload = serde_json::json!({
                    "model": self.model,
                    "max_tokens": max_tokens,
                    "system": system,
                    "messages": [{"role":"user", "content": bounded_request}],
                });
                let response = self.send_json(
                    self.client
                        .post(&self.endpoint)
                        .header("x-api-key", &self.api_key)
                        .header("anthropic-version", "2023-06-01")
                        .json(&payload),
                )?;
                response
                    .pointer("/content/0/text")
                    .and_then(serde_json::Value::as_str)
                    .filter(|text| !text.trim().is_empty())
                    .map(str::to_owned)
                    .ok_or_else(|| "Anthropic response did not include narrator text.".to_owned())
            }
            "Google Gemini" => {
                let endpoint = self.endpoint.replace("{model}", &self.model);
                let payload = serde_json::json!({
                    "systemInstruction": {"parts": [{"text": system}]},
                    "contents": [{"role":"user", "parts":[{"text": bounded_request}]}],
                    "generationConfig": {"maxOutputTokens": max_tokens, "temperature": 0.7},
                });
                let response = self.send_json(
                    self.client
                        .post(endpoint)
                        .header("x-goog-api-key", &self.api_key)
                        .json(&payload),
                )?;
                response
                    .pointer("/candidates/0/content/parts/0/text")
                    .and_then(serde_json::Value::as_str)
                    .filter(|text| !text.trim().is_empty())
                    .map(str::to_owned)
                    .ok_or_else(|| "Gemini response did not include narrator text.".to_owned())
            }
            _ => {
                let payload = serde_json::json!({
                    "model": self.model,
                    "messages": [{"role":"system","content":system}, {"role":"user","content":bounded_request}],
                    "temperature": 0.7,
                    "max_tokens": max_tokens,
                    "stream": false,
                });
                let response = self.send_json(
                    self.client
                        .post(&self.endpoint)
                        .bearer_auth(&self.api_key)
                        .json(&payload),
                )?;
                response
                    .pointer("/choices/0/message/content")
                    .and_then(serde_json::Value::as_str)
                    .filter(|text| !text.trim().is_empty())
                    .map(str::to_owned)
                    .ok_or_else(|| "Cloud response did not include narrator text.".to_owned())
            }
        }
    }
}

#[derive(Deserialize)]
struct LlamaCppCompletion {
    choices: Vec<LlamaCppChoice>,
}
#[derive(Deserialize)]
struct LlamaCppChoice {
    message: LlamaCppMessage,
}
#[derive(Deserialize)]
struct LlamaCppMessage {
    #[serde(default)]
    content: Option<String>,
}

impl LocalHttpNarratorTransport {
    pub fn new(endpoint: impl Into<String>, timeout: Duration) -> Self {
        Self {
            endpoint: endpoint.into(),
            timeout,
        }
    }
}

impl NarratorTransport for LocalHttpNarratorTransport {
    fn complete(&self, bounded_request_json: &str) -> Result<String, String> {
        complete_local_http(&self.endpoint, self.timeout, bounded_request_json)
    }
}

fn complete_local_http(
    endpoint: &str,
    timeout: Duration,
    request_body: &str,
) -> Result<String, String> {
    let (host, port, path) = parse_http_endpoint(endpoint)?;
    let address = format!("{host}:{port}")
        .to_socket_addrs()
        .map_err(|error| format!("Narrator endpoint lookup failed: {error}"))?
        .next()
        .ok_or_else(|| "Narrator endpoint resolved to no address.".to_owned())?;
    let mut stream = TcpStream::connect_timeout(&address, timeout)
        .map_err(|error| format!("Narrator endpoint unavailable: {error}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| error.to_string())?;
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        request_body.len(),
        request_body
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("Narrator request failed: {error}"))?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| format!("Narrator response failed: {error}"))?;
    let (head, response_body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| "Narrator endpoint returned an invalid HTTP response.".to_owned())?;
    if !head.starts_with("HTTP/1.1 2") && !head.starts_with("HTTP/1.0 2") {
        append_narrator_audit(endpoint, request_body.len(), 0, false);
        let detail = response_body.trim();
        let detail = if detail.is_empty() {
            String::new()
        } else {
            format!(" {detail}")
        };
        return Err(format!(
            "Narrator endpoint returned {}.{detail}",
            head.lines().next().unwrap_or("an error"),
        ));
    }
    append_narrator_audit(endpoint, request_body.len(), response_body.len(), true);
    Ok(response_body.to_owned())
}

fn append_narrator_audit(
    endpoint: &str,
    request_bytes: usize,
    response_bytes: usize,
    success: bool,
) {
    let path = crate::app_paths::path("runtime/logs/narrator_adapter.jsonl");
    let Some(parent) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let endpoint = endpoint.replace('"', "");
    let line = format!(
        "{{\"timestamp\":{timestamp},\"endpoint\":\"{endpoint}\",\"request_bytes\":{request_bytes},\"response_bytes\":{response_bytes},\"success\":{success}}}\n"
    );
    let _ = file.write_all(line.as_bytes());
}

fn parse_http_endpoint(endpoint: &str) -> Result<(String, u16, String), String> {
    let value = endpoint.strip_prefix("http://").ok_or_else(|| {
        "Only local http:// narrator endpoints are supported by the built-in transport.".to_owned()
    })?;
    let (authority, path) = value.split_once('/').unwrap_or((value, ""));
    let (host, port) = authority
        .rsplit_once(':')
        .ok_or_else(|| "Narrator endpoint must include an explicit port.".to_owned())?;
    let port = port
        .parse::<u16>()
        .map_err(|_| "Narrator endpoint port is invalid.".to_owned())?;
    if host.is_empty() {
        return Err("Narrator endpoint host is missing.".to_owned());
    }
    Ok((host.to_owned(), port, format!("/{}", path)))
}

pub struct LiveNarratorAdapter<T> {
    transport: T,
    play_contract: PlayContract,
    allowed_proposal_kinds: Vec<String>,
    failure: Mutex<Option<String>>,
    pending_item_proposals: Mutex<Vec<ValidatedItemProposal>>,
    pending_status_proposals: Mutex<Vec<ValidatedStatusProposal>>,
    pending_npc_proposals: Mutex<Vec<ValidatedNpcProposal>>,
    pending_enemy_proposals: Mutex<Vec<ValidatedEnemyProposal>>,
    pending_boss_proposals: Mutex<Vec<ValidatedBossProposal>>,
    pending_objective_proposals: Mutex<Vec<ValidatedObjectiveProposal>>,
    pending_temporary_exit_proposals: Mutex<Vec<ValidatedTemporaryExitProposal>>,
    pending_ability_grant_proposals: Mutex<Vec<ValidatedAbilityGrantProposal>>,
    proposal_diagnostics: Mutex<Vec<String>>,
}

impl<T> LiveNarratorAdapter<T> {
    #[allow(dead_code)]
    pub fn new(transport: T, fallback: MockNarrator) -> Self {
        Self::with_play_contract(
            transport,
            fallback,
            PlayContract::from_preset_name("rules_first"),
        )
    }

    pub fn with_play_contract(
        transport: T,
        _fallback: MockNarrator,
        play_contract: PlayContract,
    ) -> Self {
        // Compatibility constructor for tests and external adapters that predate capability
        // negotiation. Production construction uses `with_play_contract_and_chaos`.
        let mut adapter =
            Self::with_play_contract_and_chaos(transport, _fallback, play_contract, 0.0);
        adapter.allowed_proposal_kinds = [
            "item",
            "status",
            "npc",
            "enemy",
            "boss",
            "objective",
            "temporary_exit",
            "ability_grant",
        ]
        .iter()
        .map(|kind| (*kind).to_owned())
        .collect();
        adapter
    }

    pub fn with_play_contract_and_chaos(
        transport: T,
        _fallback: MockNarrator,
        play_contract: PlayContract,
        chaos: f32,
    ) -> Self {
        let allowed_proposal_kinds = allowed_proposal_kinds(&play_contract, chaos);
        Self {
            transport,
            play_contract,
            allowed_proposal_kinds,
            failure: Mutex::new(None),
            pending_item_proposals: Mutex::new(Vec::new()),
            pending_status_proposals: Mutex::new(Vec::new()),
            pending_npc_proposals: Mutex::new(Vec::new()),
            pending_enemy_proposals: Mutex::new(Vec::new()),
            pending_boss_proposals: Mutex::new(Vec::new()),
            pending_objective_proposals: Mutex::new(Vec::new()),
            pending_temporary_exit_proposals: Mutex::new(Vec::new()),
            pending_ability_grant_proposals: Mutex::new(Vec::new()),
            proposal_diagnostics: Mutex::new(Vec::new()),
        }
    }

    pub fn with_play_contract_chaos_and_allowed_kinds(
        transport: T,
        fallback: MockNarrator,
        play_contract: PlayContract,
        chaos: f32,
        allowed_proposal_kinds: Vec<String>,
    ) -> Self {
        let mut adapter =
            Self::with_play_contract_and_chaos(transport, fallback, play_contract, chaos);
        adapter.allowed_proposal_kinds = allowed_proposal_kinds;
        adapter
    }
}

impl<T: NarratorTransport> LiveNarratorAdapter<T> {
    fn narrate_context(
        &self,
        context: &NarratorContext,
        player_message: Option<&str>,
        host_selected_proposal_kind: Option<&str>,
    ) -> Result<Vec<String>, String> {
        let narration_request = serde_json::to_string(&serde_json::json!({
            "lane": "narration",
            "context": context,
            "player_message": player_message,
        }))
        .map_err(|error| format!("Narrator request could not be encoded: {error}"))?;
        let raw_narration = self.transport.complete(&narration_request)?;
        let narration = recover_plain_text_narration(&raw_narration)
            .unwrap_or_else(|| raw_narration.trim().to_owned());
        if narration.is_empty() {
            return Err("Narrator output omitted narration.".to_owned());
        }
        let proposals = if let Some(kind) = host_selected_proposal_kind {
            self.generate_compact_proposal(context, player_message, kind)
        } else {
            Vec::new()
        };
        let mut items = Vec::new();
        let mut statuses = Vec::new();
        let mut npcs = Vec::new();
        let mut enemies = Vec::new();
        let mut bosses = Vec::new();
        let mut objectives = Vec::new();
        let mut exits = Vec::new();
        let mut ability_grants = Vec::new();
        for proposal in proposals {
            match proposal {
                NarratorTypedProposal::Item { .. } => {
                    items.push(ValidatedItemProposal::from(proposal))
                }
                NarratorTypedProposal::Status { .. } => {
                    statuses.push(ValidatedStatusProposal::from(proposal))
                }
                NarratorTypedProposal::Npc { .. } => {
                    npcs.push(ValidatedNpcProposal::from(proposal))
                }
                NarratorTypedProposal::Enemy { .. } => {
                    enemies.push(ValidatedEnemyProposal::from(proposal))
                }
                NarratorTypedProposal::Boss { .. } => {
                    bosses.push(ValidatedBossProposal::from(proposal))
                }
                NarratorTypedProposal::Objective { .. } => {
                    objectives.push(ValidatedObjectiveProposal::from(proposal))
                }
                NarratorTypedProposal::TemporaryExit { .. } => {
                    exits.push(ValidatedTemporaryExitProposal::from(proposal))
                }
                NarratorTypedProposal::AbilityGrant { .. } => {
                    ability_grants.push(ValidatedAbilityGrantProposal::from(proposal))
                }
            }
        }
        if let Ok(mut pending) = self.pending_item_proposals.lock() {
            pending.extend(items);
        }
        if let Ok(mut pending) = self.pending_status_proposals.lock() {
            pending.extend(statuses);
        }
        if let Ok(mut pending) = self.pending_npc_proposals.lock() {
            pending.extend(npcs);
        }
        if let Ok(mut pending) = self.pending_enemy_proposals.lock() {
            pending.extend(enemies);
        }
        if let Ok(mut pending) = self.pending_boss_proposals.lock() {
            pending.extend(bosses);
        }
        if let Ok(mut pending) = self.pending_objective_proposals.lock() {
            pending.extend(objectives);
        }
        if let Ok(mut pending) = self.pending_temporary_exit_proposals.lock() {
            pending.extend(exits);
        }
        if let Ok(mut pending) = self.pending_ability_grant_proposals.lock() {
            pending.extend(ability_grants);
        }
        if let Ok(mut failure) = self.failure.lock() {
            *failure = None;
        }
        Ok(vec![narration])
    }

    fn record_proposal_diagnostic(&self, message: impl Into<String>) {
        if let Ok(mut diagnostics) = self.proposal_diagnostics.lock() {
            diagnostics.push(message.into());
        }
    }

    fn generate_compact_proposal(
        &self,
        context: &NarratorContext,
        player_message: Option<&str>,
        kind: &str,
    ) -> Vec<NarratorTypedProposal> {
        if !self
            .allowed_proposal_kinds
            .iter()
            .any(|allowed| allowed == kind)
        {
            self.record_proposal_diagnostic(
                "Narrator proposal discarded for this turn: host selected an unauthorized kind.",
            );
            return Vec::new();
        }
        self.record_proposal_diagnostic(format!("Host: proposal kind selected: {kind}."));

        let first_attempt = self
            .transport
            .complete(&proposal_form_request(context, player_message, kind, false))
            .and_then(|raw| parse_compact_proposal(kind, &raw));
        match first_attempt {
            Ok(proposal) => {
                self.record_proposal_diagnostic("Narrator: compact proposal parsed successfully.");
                vec![proposal]
            }
            Err(first_error) => {
                let diagnostic = if first_error.starts_with("missing required field") {
                    format!("Narrator proposal {first_error}")
                } else {
                    format!("Narrator proposal invalid field/value: {first_error}")
                };
                self.record_proposal_diagnostic(diagnostic);
                self.record_proposal_diagnostic("Narrator: proposal retry attempted.");
                match self
                    .transport
                    .complete(&proposal_form_request(context, player_message, kind, true))
                    .and_then(|raw| parse_compact_proposal(kind, &raw))
                {
                    Ok(proposal) => {
                        self.record_proposal_diagnostic(
                            "Narrator: compact proposal parsed successfully after retry.",
                        );
                        vec![proposal]
                    }
                    Err(retry_error) => {
                        self.record_proposal_diagnostic(format!(
                            "Narrator proposal discarded for this turn: {retry_error}"
                        ));
                        Vec::new()
                    }
                }
            }
        }
    }

    fn unavailable_line(&self, error: String) -> Vec<String> {
        if let Ok(mut failure) = self.failure.lock() {
            *failure = Some(error.clone());
        }
        vec![format!(
            "Narrator unavailable: {error}. Choose Mock or Off to continue without live narration."
        )]
    }
}

impl<T: NarratorTransport> Narrator for LiveNarratorAdapter<T> {
    fn narrate_run_start(
        &self,
        bundle: &DatapackBundle,
        state: &RunState,
        _seed_lines: &[String],
    ) -> Vec<String> {
        let context = build_narrator_context_for_contract(
            bundle,
            state,
            None,
            None,
            self.play_contract.clone(),
        );
        // Setup narration is presentation-only. A proposal is only eligible after a player
        // submits non-command free text through the host-owned gate.
        self.narrate_context(&context, None, None)
            .unwrap_or_else(|error| self.unavailable_line(error))
    }

    fn narrate_action(
        &self,
        bundle: &DatapackBundle,
        action: &GameAction,
        outcome: &ActionOutcome,
        state: &RunState,
        host_selected_proposal_kind: Option<&str>,
    ) -> Vec<String> {
        let context = build_narrator_context_for_contract(
            bundle,
            state,
            Some(action),
            Some(outcome),
            self.play_contract.clone(),
        );
        // Reducer-owned actions can be narrated. Only an independently host-gated, cooldown-limited
        // ambient discovery may supply a template kind here.
        self.narrate_context(&context, None, host_selected_proposal_kind)
            .unwrap_or_else(|error| self.unavailable_line(error))
    }

    fn narrate_player_message(
        &self,
        bundle: &DatapackBundle,
        state: &RunState,
        message: &str,
        host_selected_proposal_kind: Option<&str>,
    ) -> Vec<String> {
        let context = build_narrator_context_for_contract(
            bundle,
            state,
            None,
            None,
            self.play_contract.clone(),
        );
        self.narrate_context(&context, Some(message), host_selected_proposal_kind)
            .unwrap_or_else(|error| self.unavailable_line(error))
    }

    fn take_item_proposals(&self) -> Vec<ValidatedItemProposal> {
        self.pending_item_proposals
            .lock()
            .map(|mut pending| std::mem::take(&mut *pending))
            .unwrap_or_default()
    }

    fn take_status_proposals(&self) -> Vec<ValidatedStatusProposal> {
        self.pending_status_proposals
            .lock()
            .map(|mut pending| std::mem::take(&mut *pending))
            .unwrap_or_default()
    }

    fn take_npc_proposals(&self) -> Vec<ValidatedNpcProposal> {
        self.pending_npc_proposals
            .lock()
            .map(|mut pending| std::mem::take(&mut *pending))
            .unwrap_or_default()
    }

    fn take_enemy_proposals(&self) -> Vec<ValidatedEnemyProposal> {
        self.pending_enemy_proposals
            .lock()
            .map(|mut pending| std::mem::take(&mut *pending))
            .unwrap_or_default()
    }
    fn take_boss_proposals(&self) -> Vec<ValidatedBossProposal> {
        self.pending_boss_proposals
            .lock()
            .map(|mut pending| std::mem::take(&mut *pending))
            .unwrap_or_default()
    }

    fn take_objective_proposals(&self) -> Vec<ValidatedObjectiveProposal> {
        self.pending_objective_proposals
            .lock()
            .map(|mut pending| std::mem::take(&mut *pending))
            .unwrap_or_default()
    }
    fn take_temporary_exit_proposals(&self) -> Vec<ValidatedTemporaryExitProposal> {
        self.pending_temporary_exit_proposals
            .lock()
            .map(|mut p| std::mem::take(&mut *p))
            .unwrap_or_default()
    }
    fn take_ability_grant_proposals(&self) -> Vec<ValidatedAbilityGrantProposal> {
        self.pending_ability_grant_proposals
            .lock()
            .map(|mut pending| std::mem::take(&mut *pending))
            .unwrap_or_default()
    }

    fn take_proposal_diagnostics(&self) -> Vec<String> {
        self.proposal_diagnostics
            .lock()
            .map(|mut diagnostics| std::mem::take(&mut *diagnostics))
            .unwrap_or_default()
    }

    fn status(&self) -> NarratorStatus {
        self.failure
            .lock()
            .ok()
            .and_then(|failure| failure.clone())
            .map(NarratorStatus::unavailable)
            .unwrap_or_else(|| NarratorStatus::ready("Live narrator connected"))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct NarratorResponse {
    pub narration: String,
    #[serde(default)]
    pub proposals: Vec<NarratorTypedProposal>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NarratorTypedProposal {
    Item {
        name: String,
        description: String,
        damage: i32,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        actor: NarratorActorConfiguration,
    },
    Status {
        name: String,
        trigger: String,
        duration: String,
        #[serde(default)]
        additive_damage: i32,
        #[serde(default)]
        additive_healing: i32,
        #[serde(default)]
        required_target_tags: Vec<String>,
        #[serde(default)]
        required_context_tags: Vec<String>,
    },
    Npc {
        name: String,
        description: String,
        faction: String,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        actor: NarratorActorConfiguration,
    },
    Enemy {
        name: String,
        description: String,
        max_hp: i32,
        damage: i32,
        #[serde(default)]
        tags: Vec<String>,
    },
    Boss {
        name: String,
        description: String,
        max_hp: i32,
        damage: i32,
        #[serde(default)]
        tags: Vec<String>,
    },
    Objective {
        name: String,
        description: String,
        completion_key: String,
    },
    TemporaryExit {
        name: String,
        description: String,
        from_location_id: String,
        to_location_id: String,
        #[serde(default)]
        locked: bool,
    },
    AbilityGrant {
        template_id: String,
        /// None means a permanent grant; a number belongs to this one grant only.
        remaining_uses: Option<u32>,
        /// None means no turn expiry.
        duration_turns: Option<u64>,
    },
}

/// The compact proposal form's complete actor-capability checklist.  It is still only a
/// suggestion: the host displays it, validates dependencies after edits, and reducers alone
/// materialize it.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct NarratorActorConfiguration {
    pub has_health: bool,
    pub max_hp: i32,
    pub can_engage: bool,
    pub actor_damage: i32,
    pub can_hear: bool,
    pub can_see: bool,
    pub can_move: bool,
    #[serde(default)]
    pub can_carry_inventory: bool,
    pub can_spawn_from_noise: bool,
    pub can_track_noise: bool,
    pub can_track_sight: bool,
    pub disposition: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedObjectiveProposal {
    pub name: String,
    pub description: String,
    pub completion_key: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedTemporaryExitProposal {
    pub name: String,
    pub description: String,
    pub from_location_id: String,
    pub to_location_id: String,
    pub locked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedBossProposal {
    pub name: String,
    pub description: String,
    pub max_hp: i32,
    pub damage: i32,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedEnemyProposal {
    pub name: String,
    pub description: String,
    pub max_hp: i32,
    pub damage: i32,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedNpcProposal {
    pub name: String,
    pub description: String,
    pub faction: String,
    pub tags: Vec<String>,
    pub actor: NarratorActorConfiguration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedStatusProposal {
    pub name: String,
    pub trigger: String,
    pub duration: String,
    pub additive_damage: i32,
    pub additive_healing: i32,
    pub required_target_tags: Vec<String>,
    pub required_context_tags: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedItemProposal {
    pub name: String,
    pub description: String,
    pub damage: i32,
    pub tags: Vec<String>,
    pub actor: NarratorActorConfiguration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedAbilityGrantProposal {
    pub template_id: String,
    pub remaining_uses: Option<u32>,
    pub duration_turns: Option<u64>,
}

impl From<NarratorTypedProposal> for ValidatedAbilityGrantProposal {
    fn from(proposal: NarratorTypedProposal) -> Self {
        match proposal {
            NarratorTypedProposal::AbilityGrant {
                template_id,
                remaining_uses,
                duration_turns,
            } => Self {
                template_id,
                remaining_uses,
                duration_turns,
            },
            _ => unreachable!("non-ability-grant proposal"),
        }
    }
}

impl From<NarratorTypedProposal> for ValidatedStatusProposal {
    fn from(proposal: NarratorTypedProposal) -> Self {
        match proposal {
            NarratorTypedProposal::Status {
                name,
                trigger,
                duration,
                additive_damage,
                additive_healing,
                required_target_tags,
                required_context_tags,
            } => Self {
                name,
                trigger,
                duration,
                additive_damage,
                additive_healing,
                required_target_tags,
                required_context_tags,
            },
            NarratorTypedProposal::Item { .. }
            | NarratorTypedProposal::Npc { .. }
            | NarratorTypedProposal::Enemy { .. }
            | NarratorTypedProposal::Boss { .. }
            | NarratorTypedProposal::Objective { .. }
            | NarratorTypedProposal::TemporaryExit { .. }
            | NarratorTypedProposal::AbilityGrant { .. } => {
                unreachable!("non-status proposal")
            }
        }
    }
}

impl From<NarratorTypedProposal> for ValidatedNpcProposal {
    fn from(proposal: NarratorTypedProposal) -> Self {
        match proposal {
            NarratorTypedProposal::Npc {
                name,
                description,
                faction,
                tags,
                actor,
            } => Self {
                name,
                description,
                faction,
                tags,
                actor,
            },
            NarratorTypedProposal::Item { .. }
            | NarratorTypedProposal::Status { .. }
            | NarratorTypedProposal::Enemy { .. }
            | NarratorTypedProposal::Boss { .. }
            | NarratorTypedProposal::Objective { .. }
            | NarratorTypedProposal::TemporaryExit { .. }
            | NarratorTypedProposal::AbilityGrant { .. } => {
                unreachable!("non-NPC proposal")
            }
        }
    }
}

impl From<NarratorTypedProposal> for ValidatedEnemyProposal {
    fn from(proposal: NarratorTypedProposal) -> Self {
        match proposal {
            NarratorTypedProposal::Enemy {
                name,
                description,
                max_hp,
                damage,
                tags,
            } => Self {
                name,
                description,
                max_hp,
                damage,
                tags,
            },
            NarratorTypedProposal::Item { .. }
            | NarratorTypedProposal::Status { .. }
            | NarratorTypedProposal::Npc { .. }
            | NarratorTypedProposal::Boss { .. }
            | NarratorTypedProposal::Objective { .. }
            | NarratorTypedProposal::TemporaryExit { .. }
            | NarratorTypedProposal::AbilityGrant { .. } => unreachable!("non-enemy proposal"),
        }
    }
}

impl From<NarratorTypedProposal> for ValidatedItemProposal {
    fn from(proposal: NarratorTypedProposal) -> Self {
        match proposal {
            NarratorTypedProposal::Item {
                name,
                description,
                damage,
                tags,
                actor,
            } => Self {
                name,
                description,
                damage,
                tags,
                actor,
            },
            NarratorTypedProposal::Status { .. }
            | NarratorTypedProposal::Npc { .. }
            | NarratorTypedProposal::Enemy { .. }
            | NarratorTypedProposal::Boss { .. }
            | NarratorTypedProposal::Objective { .. }
            | NarratorTypedProposal::TemporaryExit { .. }
            | NarratorTypedProposal::AbilityGrant { .. } => {
                unreachable!("non-item proposal")
            }
        }
    }
}

impl From<NarratorTypedProposal> for ValidatedBossProposal {
    fn from(proposal: NarratorTypedProposal) -> Self {
        match proposal {
            NarratorTypedProposal::Boss {
                name,
                description,
                max_hp,
                damage,
                tags,
            } => Self {
                name,
                description,
                max_hp,
                damage,
                tags,
            },
            _ => unreachable!("non-boss proposal"),
        }
    }
}

impl From<NarratorTypedProposal> for ValidatedObjectiveProposal {
    fn from(proposal: NarratorTypedProposal) -> Self {
        match proposal {
            NarratorTypedProposal::Objective {
                name,
                description,
                completion_key,
            } => Self {
                name,
                description,
                completion_key,
            },
            _ => unreachable!("non-objective proposal"),
        }
    }
}
impl From<NarratorTypedProposal> for ValidatedTemporaryExitProposal {
    fn from(proposal: NarratorTypedProposal) -> Self {
        match proposal {
            NarratorTypedProposal::TemporaryExit {
                name,
                description,
                from_location_id,
                to_location_id,
                locked,
            } => Self {
                name,
                description,
                from_location_id,
                to_location_id,
                locked,
            },
            _ => unreachable!("non-exit proposal"),
        }
    }
}

/// Parses untrusted model output at the narrator boundary. Parsed proposals are display-only;
/// canonical state still changes only through an explicit reducer admission action.
#[allow(dead_code)]
pub fn parse_narrator_response(raw: &str) -> Result<NarratorResponse, String> {
    parse_narrator_response_with_allowed_kinds(
        raw,
        &[
            "item",
            "status",
            "npc",
            "enemy",
            "boss",
            "objective",
            "temporary_exit",
            "ability_grant",
        ]
        .iter()
        .map(|kind| (*kind).to_owned())
        .collect::<Vec<_>>(),
    )
}

fn parse_narrator_response_with_allowed_kinds(
    raw: &str,
    allowed_kinds: &[String],
) -> Result<NarratorResponse, String> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|error| format!("Narrator output was not valid JSON: {error}"))?;
    let narration = value
        .get("narration")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "Narrator output omitted narration.".to_owned())?;
    if narration.trim().is_empty() {
        return Err("Narrator output omitted narration.".to_owned());
    }
    let proposals = parse_proposals_from_value(&value, allowed_kinds);
    Ok(NarratorResponse {
        narration,
        proposals,
    })
}

/// Parses the compact, proposal-only lane. This deliberately has no narration requirement:
/// proposal serialization is independent from presentation and a bad proposal is discarded.
fn parse_compact_proposal(kind: &str, raw: &str) -> Result<NarratorTypedProposal, String> {
    let expected = proposal_form_fields(kind)
        .ok_or_else(|| "proposal kind has no host-owned form.".to_owned())?;
    let mut fields = BTreeMap::new();
    for line in raw.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let (label, value) = line
            .split_once(':')
            .ok_or_else(|| "proposal form contained a malformed line.".to_owned())?;
        let supplied_label = label.trim().to_ascii_uppercase();
        // Syntax-only compatibility for a common generic capability synonym. It is accepted
        // only when the host form actually authorizes the equivalent field. Every other extra
        // line is ignored, never admitted, and cannot add mechanics outside the form.
        let label = if supplied_label == "CAN_ATTACK" && expected.contains(&"CAN_ENGAGE") {
            "CAN_ENGAGE".to_owned()
        } else {
            supplied_label
        };
        if !expected.iter().any(|expected| *expected == label) {
            continue;
        }
        let value = value.trim();
        if value.is_empty() {
            return Err(format!("missing required field {label}."));
        }
        if fields.insert(label.clone(), value.to_owned()).is_some() {
            return Err(format!("duplicate field {label}."));
        }
    }
    for label in expected {
        if !fields.contains_key(label) {
            // Newly registered capability roots default to disabled when an older compact form
            // does not know about them. This is host canonicalisation of an omitted boolean,
            // never inference of a mechanical capability from narration.
            if label == "CAN_CARRY_INVENTORY" {
                fields.insert((*label).to_owned(), "false".to_owned());
            } else if matches!(label, "TAGS" | "TARGET_TAGS" | "CONTEXT_TAGS") {
                // Tags are descriptive metadata. Old compact models may omit this newly-added
                // line, so retain a host-owned type tag instead of discarding an otherwise valid
                // proposal; a supplied line still replaces this default.
                fields.insert((*label).to_owned(), "none".to_owned());
            } else {
                return Err(format!("missing required field {label}."));
            }
        }
    }
    let text = |label: &str| -> Result<String, String> {
        fields
            .get(label)
            .cloned()
            .ok_or_else(|| format!("missing required field {label}."))
    };
    let integer = |label: &str| -> Result<i32, String> {
        text(label)?
            .parse::<i32>()
            .map_err(|_| format!("invalid field/value {label}."))
    };
    let boolean = |label: &str| -> Result<bool, String> {
        match text(label)?.to_ascii_lowercase().as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(format!("invalid field/value {label}.")),
        }
    };
    let optional_count = |label: &str, none_word: &str| -> Result<Option<u32>, String> {
        let value = text(label)?.to_ascii_lowercase();
        if value == none_word {
            Ok(None)
        } else {
            let value = value
                .parse::<u32>()
                .map_err(|_| format!("invalid field/value {label}."))?;
            (value > 0 && value <= 999)
                .then_some(value)
                .map(Some)
                .ok_or_else(|| format!("invalid field/value {label}."))
        }
    };
    let actor = || -> Result<NarratorActorConfiguration, String> {
        let disposition = text("DISPOSITION")?.to_ascii_lowercase();
        if !matches!(disposition.as_str(), "friendly" | "neutral" | "hostile") {
            return Err("invalid field/value DISPOSITION.".to_owned());
        }
        Ok(NarratorActorConfiguration {
            has_health: boolean("HAS_HEALTH")?,
            max_hp: integer("MAX_HP")?,
            can_engage: boolean("CAN_ENGAGE")?,
            actor_damage: integer("ACTOR_DAMAGE")?,
            can_hear: boolean("CAN_HEAR")?,
            can_see: boolean("CAN_SEE")?,
            can_move: boolean("CAN_MOVE")?,
            can_carry_inventory: boolean("CAN_CARRY_INVENTORY")?,
            can_spawn_from_noise: boolean("CAN_SPAWN_FROM_NOISE")?,
            can_track_noise: boolean("CAN_TRACK_NOISE")?,
            can_track_sight: boolean("CAN_TRACK_SIGHT")?,
            disposition,
        })
    };
    let tags = || -> Vec<String> {
        let default_tag = match kind {
            "item" => "item",
            "npc" => "npc",
            "enemy" => "enemy",
            "boss" => "boss",
            _ => return Vec::new(),
        };
        let mut tags = text("TAGS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .filter(|tag| !tag.is_empty() && tag != "none")
            .collect::<Vec<_>>();
        tags.push(default_tag.to_owned());
        tags.sort();
        tags.dedup();
        tags
    };
    let tag_list = |label: &str| -> Vec<String> {
        text(label)
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .filter(|tag| !tag.is_empty() && tag != "none")
            .collect()
    };
    let proposal = match kind {
        "item" => NarratorTypedProposal::Item {
            name: text("NAME")?,
            description: text("DESCRIPTION")?,
            damage: integer("DAMAGE")?,
            tags: tags(),
            actor: actor()?,
        },
        "status" => NarratorTypedProposal::Status {
            name: text("NAME")?,
            trigger: text("TRIGGER")?.to_ascii_lowercase(),
            duration: text("DURATION")?.to_ascii_lowercase(),
            additive_damage: integer("ADDITIVE_DAMAGE")?,
            additive_healing: integer("ADDITIVE_HEALING")?,
            required_target_tags: tag_list("TARGET_TAGS"),
            required_context_tags: tag_list("CONTEXT_TAGS"),
        },
        "npc" => NarratorTypedProposal::Npc {
            name: text("NAME")?,
            description: text("DESCRIPTION")?,
            faction: text("FACTION")?,
            tags: tags(),
            actor: actor()?,
        },
        "enemy" => NarratorTypedProposal::Enemy {
            name: text("NAME")?,
            description: text("DESCRIPTION")?,
            max_hp: integer("MAX_HP")?,
            damage: integer("DAMAGE")?,
            tags: tags(),
        },
        "boss" => NarratorTypedProposal::Boss {
            name: text("NAME")?,
            description: text("DESCRIPTION")?,
            max_hp: integer("MAX_HP")?,
            damage: integer("DAMAGE")?,
            tags: tags(),
        },
        "objective" => NarratorTypedProposal::Objective {
            name: text("NAME")?,
            description: text("DESCRIPTION")?,
            completion_key: text("COMPLETION_KEY")?,
        },
        "temporary_exit" => NarratorTypedProposal::TemporaryExit {
            name: text("NAME")?,
            description: text("DESCRIPTION")?,
            from_location_id: text("FROM_LOCATION_ID")?,
            to_location_id: text("TO_LOCATION_ID")?,
            locked: match text("LOCKED")?.to_ascii_lowercase().as_str() {
                "true" => true,
                "false" => false,
                _ => return Err("invalid field/value LOCKED.".to_owned()),
            },
        },
        "ability_grant" => NarratorTypedProposal::AbilityGrant {
            template_id: text("TEMPLATE_ID")?,
            remaining_uses: optional_count("REMAINING_USES", "infinite")?,
            duration_turns: optional_count("DURATION_TURNS", "permanent")?.map(u64::from),
        },
        _ => return Err("proposal kind was not authorized.".to_owned()),
    };
    validate_narrator_proposal(&proposal)?;
    Ok(proposal)
}

fn parse_proposals_from_value(
    value: &serde_json::Value,
    allowed_kinds: &[String],
) -> Vec<NarratorTypedProposal> {
    value
        .get("proposals")
        .and_then(serde_json::Value::as_array)
        .map(|proposals| parse_proposals_from_slice(proposals, allowed_kinds))
        .unwrap_or_default()
}

fn parse_proposals_from_slice(
    proposals: &[serde_json::Value],
    allowed_kinds: &[String],
) -> Vec<NarratorTypedProposal> {
    proposals
        .iter()
        .filter_map(|proposal| {
            serde_json::from_value::<NarratorTypedProposal>(proposal.clone()).ok()
        })
        .filter(|proposal| {
            allowed_kinds
                .iter()
                .any(|kind| kind == proposal_kind_name(proposal))
        })
        .filter(|proposal| validate_narrator_proposal(proposal).is_ok())
        .collect()
}

/// If a small local model misses the JSON envelope, retain only plain narration.  No malformed
/// proposal is ever reconstructed, so this fallback cannot create or change game state.
fn recover_plain_text_narration(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if let Some((_, after_key)) = trimmed.split_once("\"narration\"") {
        let after_colon = after_key.split_once(':')?.1.trim_start();
        if let Some(after_quote) = after_colon.strip_prefix('\"') {
            let narration = after_quote.split('\"').next()?.trim();
            if !narration.is_empty() {
                return Some(narration.to_owned());
            }
        }
    }
    if !trimmed.is_empty() && !trimmed.starts_with('{') && !trimmed.starts_with('[') {
        Some(trimmed.to_owned())
    } else {
        None
    }
}

fn proposal_kind_name(proposal: &NarratorTypedProposal) -> &'static str {
    match proposal {
        NarratorTypedProposal::Item { .. } => "item",
        NarratorTypedProposal::Status { .. } => "status",
        NarratorTypedProposal::Npc { .. } => "npc",
        NarratorTypedProposal::Enemy { .. } => "enemy",
        NarratorTypedProposal::Boss { .. } => "boss",
        NarratorTypedProposal::Objective { .. } => "objective",
        NarratorTypedProposal::TemporaryExit { .. } => "temporary_exit",
        NarratorTypedProposal::AbilityGrant { .. } => "ability_grant",
    }
}

/// Model proposals are untrusted suggestions.  An unsupported or malformed suggestion is
/// discarded at this boundary, while a valid narration response remains useful to the player.
/// No discarded proposal can reach the reducer.
fn validate_narrator_proposal(proposal: &NarratorTypedProposal) -> Result<(), String> {
    match proposal {
        NarratorTypedProposal::Item {
            name,
            description,
            damage,
            ..
        } if name.trim().is_empty() || description.trim().is_empty() || *damage < 0 => {
            return Err("Narrator item proposal failed schema validation.".to_owned());
        }
        NarratorTypedProposal::Item { .. } => {}
        NarratorTypedProposal::Status {
            name,
            trigger,
            duration,
            additive_damage,
            additive_healing,
            ..
        } if name.trim().is_empty()
            || !matches!(
                trigger.as_str(),
                "combat" | "movement" | "inspect" | "wait" | "failure"
            )
            || !matches!(
                duration.as_str(),
                "scene" | "run" | "until_used" | "until-used"
            )
            // The current host effect substrate has exactly two real outcomes:
            // combat damage and non-combat healing. Reject a decorative no-op.
            || (*additive_damage == 0 && *additive_healing == 0)
            || (trigger == "combat" && *additive_healing != 0)
            || (trigger != "combat" && *additive_damage != 0) =>
        {
            return Err("Narrator status proposal failed schema validation.".to_owned());
        }
        NarratorTypedProposal::Status { .. } => {}
        NarratorTypedProposal::Npc {
            name,
            description,
            faction,
            ..
        } if name.trim().is_empty()
            || description.trim().is_empty()
            || faction.trim().is_empty() =>
        {
            return Err("Narrator NPC proposal failed schema validation.".to_owned());
        }
        NarratorTypedProposal::Npc { .. } => {}
        NarratorTypedProposal::Enemy {
            name,
            description,
            max_hp,
            damage,
            ..
        } if name.trim().is_empty()
            || description.trim().is_empty()
            || *max_hp <= 0
            || *damage < 0 =>
        {
            return Err("Narrator enemy proposal failed schema validation.".to_owned());
        }
        NarratorTypedProposal::Enemy { .. } => {}
        NarratorTypedProposal::Boss {
            name,
            description,
            max_hp,
            damage,
            ..
        } if name.trim().is_empty()
            || description.trim().is_empty()
            || *max_hp <= 0
            || *damage < 0 =>
        {
            return Err("Narrator boss proposal failed schema validation.".to_owned());
        }
        NarratorTypedProposal::Boss { .. } => {}
        NarratorTypedProposal::Objective {
            name,
            description,
            completion_key,
        } if name.trim().is_empty()
            || description.trim().is_empty()
            || completion_key.trim().is_empty() =>
        {
            return Err("Narrator objective proposal failed schema validation.".to_owned());
        }
        NarratorTypedProposal::Objective { .. } => {}
        NarratorTypedProposal::TemporaryExit {
            name,
            description,
            from_location_id,
            to_location_id,
            ..
        } if name.trim().is_empty()
            || description.trim().is_empty()
            || from_location_id.trim().is_empty()
            || to_location_id.trim().is_empty()
            || from_location_id == to_location_id =>
        {
            return Err("Narrator temporary exit proposal failed schema validation.".to_owned());
        }
        NarratorTypedProposal::TemporaryExit { .. } => {}
        NarratorTypedProposal::AbilityGrant {
            template_id,
            remaining_uses,
            duration_turns,
        } if template_id.trim().is_empty()
            || remaining_uses.is_some_and(|uses| uses == 0)
            || duration_turns.is_some_and(|turns| turns == 0) =>
        {
            return Err("Narrator ability grant proposal failed schema validation.".to_owned());
        }
        NarratorTypedProposal::AbilityGrant { .. } => {}
    }
    Ok(())
}

pub fn build_narrator_context(
    bundle: &DatapackBundle,
    state: &RunState,
    action: Option<&GameAction>,
    outcome: Option<&ActionOutcome>,
) -> NarratorContext {
    build_narrator_context_for_contract(
        bundle,
        state,
        action,
        outcome,
        PlayContract::from_preset_name("rules_first"),
    )
}

pub fn build_narrator_context_for_contract(
    bundle: &DatapackBundle,
    state: &RunState,
    action: Option<&GameAction>,
    outcome: Option<&ActionOutcome>,
    play_contract: PlayContract,
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
        play_contract,
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
            .controlled_inventory()
            .map_or_else(Vec::new, |inventory| {
                inventory.iter().map(|item| item.name.clone()).collect()
            }),
        equipped_item: state
            .equipped_item_id_for_owner(&state.controlled_inventory_owner())
            .and_then(|equipped_id| {
                state.controlled_inventory().and_then(|inventory| {
                    inventory
                        .iter()
                        .find(|item| item.id == equipped_id)
                        .map(|item| item.name.clone())
                })
            }),
        hp: state.controlled_actor_health().0,
        max_hp: state.controlled_actor_health().1,
        noise_level: state.noise_level,
        noise_label: noise_label(state.noise_level).to_owned(),
        objective_name: state.active_objective.name.clone(),
        objective_complete: state.active_objective.completed,
        objective_facts: objective_facts(state),
        session_truth: session_truth(state),
        dm_style: bundle.dm_style.clone(),
        world_tone: bundle.world_tone.clone(),
        safety_rules: narrator_safety_rules(),
        grantable_ability_ids: bundle
            .abilities
            .iter()
            .map(|ability| ability.id.clone())
            .collect(),
    }
}

fn session_truth(state: &RunState) -> Vec<String> {
    let mut facts = Vec::new();
    facts.extend(
        state
            .controlled_inventory()
            .into_iter()
            .flatten()
            .filter(|item| matches!(item.origin, super::state::ItemOrigin::NarratorApproved))
            .map(|item| format!("Approved item: {}.", item.name)),
    );
    facts.extend(
        state
            .status_effects
            .iter()
            .filter(|effect| effect.template_id.starts_with("scratch_status_"))
            .map(|effect| format!("Approved status: {}.", effect.template_id)),
    );
    facts.extend(
        state
            .location_npcs
            .get(&state.current_location_id)
            .into_iter()
            .flatten()
            .filter(|npc| npc.lifecycle == super::state::InstanceLifecycle::Active)
            .map(|npc| {
                format!(
                    "NPC here: {} (faction {}, disposition {:?}){}; no transferable inventory or offer is registered unless explicitly listed.",
                    npc.name,
                    npc.faction,
                    npc.disposition,
                    if npc.quest_objective_ids.is_empty() {
                        String::new()
                    } else {
                        format!("; quest offers {}", npc.quest_objective_ids.join(", "))
                    }
                )
            }),
    );
    facts.extend(
        state
            .additional_objectives
            .iter()
            .filter(|objective| objective.lifecycle == super::state::InstanceLifecycle::Active)
            .map(|objective| format!("Active additional objective: {}.", objective.name)),
    );
    facts.extend(
        state
            .temporary_exits
            .iter()
            .filter(|exit| {
                exit.lifecycle == super::state::InstanceLifecycle::Active
                    && exit.from_location_id == state.current_location_id
            })
            .map(|exit| {
                format!(
                    "Approved temporary route: {} -> {}{}.",
                    exit.from_location_id,
                    exit.to_location_id,
                    if exit.locked { " (locked)" } else { "" }
                )
            }),
    );
    facts.truncate(12);
    facts
}

fn event_fact(event: &GameEvent) -> String {
    match event {
        GameEvent::HelpShown => "Help was shown.".to_owned(),
        GameEvent::ActionRejected { reason } => format!("Action rejected: {}.", reason),
        GameEvent::SelectedPackFailedValidation {
            folder_name,
            reason,
        } => format!(
            "Selected pack failed validation: {} ({}).",
            folder_name, reason
        ),
        GameEvent::LocationLooked { location_id, .. } => {
            format!("Location looked: {}.", location_id)
        }
        GameEvent::Moved {
            from_location_id,
            to_location_id,
            ..
        } => format!("Moved from {} to {}.", from_location_id, to_location_id),
        GameEvent::MovementBlocked {
            attempted_destination,
            ..
        } => format!("Movement blocked toward {}.", attempted_destination),
        GameEvent::LocationUnlocked {
            location_id,
            item_id,
            ..
        } => format!("Unlocked {} with {}.", location_id, item_id),
        GameEvent::LocationBarricaded {
            location_id,
            item_id,
            ..
        } => format!("Barricaded {} with {}.", location_id, item_id),
        GameEvent::Inspected { target, .. } => format!("Inspected {}.", target),
        GameEvent::ItemTaken { item_id, .. } => format!("Took item {}.", item_id),
        GameEvent::ItemTransferred {
            item_id,
            origin,
            destination,
        } => format!(
            "Transferred item {} from {} to {}.",
            item_id, origin, destination
        ),
        GameEvent::SocialEventApplied {
            origin_id,
            subject_id,
            kind,
            relationship_delta,
            relationship_score,
            disposition,
        } => format!(
            "Social event {:?} from {} toward {}: {:+}; relationship score {}; disposition {:?}.",
            kind, origin_id, subject_id, relationship_delta, relationship_score, disposition
        ),
        GameEvent::AbilityUsed {
            origin_id,
            template_id,
            target_id,
            effect,
            remaining_uses,
            ..
        } => format!(
            "{} used ability {} on {} for {}; remaining uses {:?}.",
            origin_id, template_id, target_id, effect, remaining_uses
        ),
        GameEvent::AbilityCooldownSet {
            origin_id,
            template_id,
            ready_at_turn,
            ..
        } => format!(
            "{} used {}; it is ready again at world turn {}.",
            origin_id, template_id, ready_at_turn
        ),
        GameEvent::StatusEffectApplied {
            origin_id,
            instance_id,
            trigger,
            duration,
        } => format!(
            "{} applied status {} for {:?} until {:?}.",
            origin_id, instance_id, trigger, duration
        ),
        GameEvent::ConnectionsRevealed {
            origin_id,
            location_id,
            count,
        } => format!(
            "{} revealed {} connections from {}.",
            origin_id, count, location_id
        ),
        GameEvent::ProgressAwarded {
            origin_id,
            pool,
            amount,
            total,
        } => format!(
            "{} gained {} {} XP; total {}.",
            origin_id, amount, pool, total
        ),
        GameEvent::SkillLevelGained {
            origin_id,
            pool,
            level,
        } => format!("{} reached {} skill level {}.", origin_id, pool, level),
        GameEvent::MaxHpIncreased {
            origin_id,
            amount,
            max_hp,
        } => format!(
            "{} gained {} maximum HP; maximum is now {}.",
            origin_id, amount, max_hp
        ),
        GameEvent::AbilityGranted {
            owner_id,
            template_id,
            source,
            ..
        } => {
            format!(
                "{} received ability {} from {:?}.",
                owner_id, template_id, source
            )
        }
        GameEvent::AbilityGrantExpired {
            owner_id,
            template_id,
            ..
        } => format!("{}'s temporary ability {} expired.", owner_id, template_id),
        GameEvent::ItemEquipped { origin_id, item_id } => {
            format!("{} equipped item {}.", origin_id, item_id)
        }
        GameEvent::ItemUsed {
            origin_id,
            item_id,
            effect,
        } => {
            format!(
                "{} used item {} with effect {:?}.",
                origin_id, item_id, effect
            )
        }
        GameEvent::ItemConsumed {
            origin_id,
            item_id,
            reason,
            ..
        } => format!("{} consumed item {}: {}.", origin_id, item_id, reason),
        GameEvent::AttackResolved {
            origin_id,
            target_id,
            target_kind,
            damage,
            defeated,
        } => format!(
            "{} attacked {:?} {} for {} damage; defeated: {}.",
            origin_id, target_kind, target_id, damage, defeated
        ),
        GameEvent::DamageTaken {
            target_id,
            amount,
            remaining_hp,
            ..
        } => format!(
            "{} took {} damage; remaining HP: {}.",
            target_id, amount, remaining_hp
        ),
        GameEvent::HealthRestored {
            origin_id,
            target_id,
            amount,
            remaining_hp,
        } => format!(
            "{} restored {} HP to {}; remaining HP: {}.",
            origin_id, amount, target_id, remaining_hp
        ),
        GameEvent::ActorSpawnedFromNoise {
            origin_id,
            template_id,
            location_id,
        } => format!(
            "Noise spawned pool actor {} from template {} at {}.",
            origin_id, template_id, location_id
        ),
        GameEvent::NoiseAttractorShifted {
            location_id,
            actor_ids,
        } => format!(
            "Noise attractor shifted to {} for spawned actors: {}.",
            location_id,
            actor_ids.join(", ")
        ),
        GameEvent::SightAttractorAcquired {
            origin_id,
            subject_id,
            location_id,
        } => format!(
            "Spawned actor {} sighted {} at {}.",
            origin_id, subject_id, location_id
        ),
        GameEvent::SightAttractorMissed {
            origin_id,
            subject_id,
            location_id,
            detect_chance_percent,
            roll_percent,
        } => format!(
            "Spawned actor {} had a sightline to {} at {} but missed the check: {}% chance, roll {}.",
            origin_id, subject_id, location_id, detect_chance_percent, roll_percent
        ),
        GameEvent::SightAttractorLost {
            origin_id,
            subject_id,
        } => format!("Spawned actor {} lost sight of {}.", origin_id, subject_id),
        GameEvent::ActorMoved {
            origin_id,
            from_location_id,
            to_location_id,
            target_location_id,
        } => format!(
            "Spawned actor {} moved from {} to {} toward {}.",
            origin_id, from_location_id, to_location_id, target_location_id
        ),
        GameEvent::ActorWaited {
            origin_id,
            location_id,
            reason,
        } => format!(
            "Spawned actor {} waited at {}; reason: {}.",
            origin_id, location_id, reason
        ),
        GameEvent::ActorAttackedHazard {
            origin_id,
            hazard_kind,
            location_id,
            break_chance_percent,
            roll_percent,
            broken,
        } => format!(
            "Spawned actor {} attacked {:?} at {}; break chance {}%, roll {}, broken: {}.",
            origin_id, hazard_kind, location_id, break_chance_percent, roll_percent, broken
        ),
        GameEvent::AttackWhiff { .. } => "Attack found no valid target.".to_owned(),
        GameEvent::Waited { location_id, .. } => format!("Waited at {}.", location_id),
        GameEvent::ObjectiveCompleted { objective_id } => {
            format!("Objective completed: {}.", objective_id)
        }
        GameEvent::RunWon => "Run won.".to_owned(),
        GameEvent::RunLost { actor_id } => format!("{} was defeated; run lost.", actor_id),
        GameEvent::NarratorItemAdmitted { item_id } => {
            format!("Narrator item admitted: {}.", item_id)
        }
        GameEvent::NarratorStatusAdmitted { instance_id } => {
            format!("Narrator status admitted: {}.", instance_id)
        }
        GameEvent::NarratorNpcAdmitted { instance_id } => {
            format!("Narrator NPC admitted: {}.", instance_id)
        }
        GameEvent::NarratorEnemyAdmitted { instance_id } => {
            format!("Narrator enemy admitted: {}.", instance_id)
        }
        GameEvent::NarratorBossAdmitted { instance_id } => {
            format!("Narrator boss admitted: {}.", instance_id)
        }
        GameEvent::NarratorTemporaryExitAdmitted { instance_id } => {
            format!("Narrator temporary exit admitted: {}.", instance_id)
        }
        GameEvent::StatusEffectExpired { instance_id } => {
            format!("Status effect expired: {}.", instance_id)
        }
    }
}

fn brief_for_event(bundle: &DatapackBundle, event: &GameEvent) -> Option<String> {
    match event {
        GameEvent::Moved { to_location_id, .. }
        | GameEvent::LocationLooked {
            location_id: to_location_id,
            ..
        }
        | GameEvent::Waited {
            location_id: to_location_id,
            ..
        } => location_brief(bundle, to_location_id),
        GameEvent::Inspected { target, .. }
        | GameEvent::ItemTaken {
            item_id: target, ..
        }
        | GameEvent::ItemTransferred {
            item_id: target, ..
        }
        | GameEvent::ItemEquipped {
            item_id: target, ..
        }
        | GameEvent::ItemUsed {
            item_id: target, ..
        }
        | GameEvent::ItemConsumed {
            template_id: target,
            ..
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
            EncounterKind::Npc => bundle
                .npcs
                .iter()
                .find(|npc| npc.id == *target_id)
                .and_then(|npc| npc.narrator_brief.clone()),
            EncounterKind::Actor => None,
        },
        GameEvent::ActorSpawnedFromNoise { template_id, .. } => {
            pool_actor_brief(bundle, template_id)
        }
        GameEvent::SightAttractorAcquired { origin_id, .. }
        | GameEvent::SightAttractorMissed { origin_id, .. }
        | GameEvent::SightAttractorLost { origin_id, .. } => pool_actor_brief(bundle, origin_id),
        GameEvent::ActorMoved { origin_id, .. }
        | GameEvent::ActorWaited { origin_id, .. }
        | GameEvent::ActorAttackedHazard { origin_id, .. } => pool_actor_brief(bundle, origin_id),
        GameEvent::NoiseAttractorShifted { location_id, .. } => location_brief(bundle, location_id),
        GameEvent::HelpShown
        | GameEvent::ActionRejected { .. }
        | GameEvent::SelectedPackFailedValidation { .. }
        | GameEvent::MovementBlocked { .. }
        | GameEvent::LocationUnlocked { .. }
        | GameEvent::LocationBarricaded { .. }
        | GameEvent::DamageTaken { .. }
        | GameEvent::HealthRestored { .. }
        | GameEvent::AttackWhiff { .. }
        | GameEvent::ObjectiveCompleted { .. }
        | GameEvent::RunWon
        | GameEvent::RunLost { .. }
        | GameEvent::NarratorItemAdmitted { .. }
        | GameEvent::NarratorStatusAdmitted { .. }
        | GameEvent::NarratorNpcAdmitted { .. }
        | GameEvent::NarratorEnemyAdmitted { .. }
        | GameEvent::NarratorBossAdmitted { .. }
        | GameEvent::NarratorTemporaryExitAdmitted { .. }
        | GameEvent::StatusEffectExpired { .. } => None,
        GameEvent::SocialEventApplied { .. }
        | GameEvent::AbilityUsed { .. }
        | GameEvent::AbilityCooldownSet { .. }
        | GameEvent::StatusEffectApplied { .. }
        | GameEvent::ConnectionsRevealed { .. }
        | GameEvent::ProgressAwarded { .. }
        | GameEvent::SkillLevelGained { .. }
        | GameEvent::MaxHpIncreased { .. }
        | GameEvent::AbilityGranted { .. }
        | GameEvent::AbilityGrantExpired { .. } => None,
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
    find_enemy(bundle, enemy_id).and_then(|enemy| enemy.narrator_brief.clone())
}

fn boss_brief(bundle: &DatapackBundle, boss_id: &str) -> Option<String> {
    bundle
        .bosses
        .iter()
        .find(|boss| boss.id == boss_id)
        .and_then(|boss| boss.narrator_brief.clone())
}

fn pool_actor_brief(bundle: &DatapackBundle, actor_id: &str) -> Option<String> {
    let template_id = enemy_template_id(actor_id);
    enemy_brief(bundle, template_id)
        .or_else(|| boss_brief(bundle, template_id))
        .or_else(|| {
            bundle
                .npcs
                .iter()
                .find(|npc| npc.id == template_id)
                .and_then(|npc| npc.narrator_brief.clone())
        })
}

fn objective_facts(state: &RunState) -> Vec<String> {
    let mut facts = Vec::new();

    if let Some(required_item_id) = state.active_objective.required_item_id.as_deref() {
        let held = state
            .controlled_inventory()
            .is_some_and(|inventory| inventory.iter().any(|item| item.id == required_item_id));
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
                Some(GameAction::Move { .. })
                | Some(GameAction::ActorMove { .. })
                | Some(GameAction::Look)
                | Some(GameAction::Wait) => {
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
                | Some(GameAction::Use { .. })
                | Some(GameAction::AdmitNarratorItem { .. })
                | Some(GameAction::AdmitNarratorStatus { .. })
                | Some(GameAction::AdmitNarratorNpc { .. })
                | Some(GameAction::AdmitNarratorEnemy { .. })
                | Some(GameAction::AdmitNarratorBoss { .. })
                | Some(GameAction::AdmitNarratorObjective { .. })
                | Some(GameAction::AdmitNarratorTemporaryExit { .. }) => {
                    format!("The object has opinions too: {}", brief)
                }
                Some(GameAction::Attack)
                | Some(GameAction::AttackTarget { .. })
                | Some(GameAction::ActorAttack { .. }) => {
                    format!("The threat deserves this much honesty: {}", brief)
                }
                Some(GameAction::Help)
                | Some(GameAction::TransferItem { .. })
                | Some(GameAction::ApplySocialEvent { .. })
                | Some(GameAction::UseAbility { .. })
                | Some(GameAction::UnlockAbility { .. })
                | Some(GameAction::GrantAbility { .. })
                | Some(GameAction::LearnAbilityFromItem { .. })
                | Some(GameAction::AwardProgress { .. }) => {
                    format!("The rules remain ugly and simple: {}", brief)
                }
            };
        }

        if lower.contains("slapstick") {
            return match action {
                None => format!("The scene waddles in like this: {}", brief),
                Some(GameAction::Move { .. })
                | Some(GameAction::ActorMove { .. })
                | Some(GameAction::Look)
                | Some(GameAction::Wait) => {
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
                | Some(GameAction::Use { .. })
                | Some(GameAction::AdmitNarratorItem { .. })
                | Some(GameAction::AdmitNarratorStatus { .. })
                | Some(GameAction::AdmitNarratorNpc { .. })
                | Some(GameAction::AdmitNarratorEnemy { .. })
                | Some(GameAction::AdmitNarratorBoss { .. })
                | Some(GameAction::AdmitNarratorObjective { .. })
                | Some(GameAction::AdmitNarratorTemporaryExit { .. }) => {
                    format!("The prop department insists on this detail: {}", brief)
                }
                Some(GameAction::Attack)
                | Some(GameAction::AttackTarget { .. })
                | Some(GameAction::ActorAttack { .. }) => {
                    format!("The violence arrives with flavour text attached: {}", brief)
                }
                Some(GameAction::Help)
                | Some(GameAction::TransferItem { .. })
                | Some(GameAction::ApplySocialEvent { .. })
                | Some(GameAction::UseAbility { .. })
                | Some(GameAction::UnlockAbility { .. })
                | Some(GameAction::GrantAbility { .. })
                | Some(GameAction::LearnAbilityFromItem { .. })
                | Some(GameAction::AwardProgress { .. }) => {
                    format!("Even the rules sound like a bit: {}", brief)
                }
            };
        }

        if lower.contains("cozy") {
            return match action {
                None => format!("The scene opens gently: {}", brief),
                Some(GameAction::Move { .. })
                | Some(GameAction::ActorMove { .. })
                | Some(GameAction::Look)
                | Some(GameAction::Wait) => {
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
                | Some(GameAction::Use { .. })
                | Some(GameAction::AdmitNarratorItem { .. })
                | Some(GameAction::AdmitNarratorStatus { .. })
                | Some(GameAction::AdmitNarratorNpc { .. })
                | Some(GameAction::AdmitNarratorEnemy { .. })
                | Some(GameAction::AdmitNarratorBoss { .. })
                | Some(GameAction::AdmitNarratorObjective { .. })
                | Some(GameAction::AdmitNarratorTemporaryExit { .. }) => {
                    format!("The item carries its own quiet story: {}", brief)
                }
                Some(GameAction::Attack)
                | Some(GameAction::AttackTarget { .. })
                | Some(GameAction::ActorAttack { .. }) => {
                    format!("Even the danger arrives with a clear feeling: {}", brief)
                }
                Some(GameAction::Help)
                | Some(GameAction::TransferItem { .. })
                | Some(GameAction::ApplySocialEvent { .. })
                | Some(GameAction::UseAbility { .. })
                | Some(GameAction::UnlockAbility { .. })
                | Some(GameAction::GrantAbility { .. })
                | Some(GameAction::LearnAbilityFromItem { .. })
                | Some(GameAction::AwardProgress { .. }) => {
                    format!("The shape of the world stays understandable: {}", brief)
                }
            };
        }

        match action {
            None => format!("The scene sets itself like this: {}", brief),
            Some(GameAction::Move { .. })
            | Some(GameAction::ActorMove { .. })
            | Some(GameAction::Look)
            | Some(GameAction::Wait) => {
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
            | Some(GameAction::Use { .. })
            | Some(GameAction::AdmitNarratorItem { .. })
            | Some(GameAction::AdmitNarratorStatus { .. })
            | Some(GameAction::AdmitNarratorNpc { .. })
            | Some(GameAction::AdmitNarratorEnemy { .. })
            | Some(GameAction::AdmitNarratorBoss { .. })
            | Some(GameAction::AdmitNarratorObjective { .. })
            | Some(GameAction::AdmitNarratorTemporaryExit { .. }) => {
                format!("The object lands in the hand like this: {}", brief)
            }
            Some(GameAction::Attack)
            | Some(GameAction::AttackTarget { .. })
            | Some(GameAction::ActorAttack { .. }) => {
                format!("The threat comes into focus: {}", brief)
            }
            Some(GameAction::Help)
            | Some(GameAction::TransferItem { .. })
            | Some(GameAction::ApplySocialEvent { .. })
            | Some(GameAction::UseAbility { .. })
            | Some(GameAction::UnlockAbility { .. })
            | Some(GameAction::GrantAbility { .. })
            | Some(GameAction::LearnAbilityFromItem { .. })
            | Some(GameAction::AwardProgress { .. }) => {
                format!("The situation stays legible: {}", brief)
            }
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
            if matches!(
                action,
                Some(GameAction::Attack) | Some(GameAction::AttackTarget { .. })
            ) && context.hp < context.max_hp
            {
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
                    origin_id: state.player_entity_id.clone(),
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
        _: Option<&str>,
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
    use std::sync::Mutex;

    use crate::data::datapacks::load_datapack_bundle_by_folder;
    use crate::game::ActionOutcome;
    use crate::game::actions::{EncounterKind, GameAction, GameEvent};
    use crate::game::generation::generate_new_run;
    use crate::game::state::{
        InstanceLifecycle, ObjectiveBucket, ObjectiveState, TemporaryExitInstance,
    };

    use super::{
        LiveNarratorAdapter, LlamaCppCompletion, LocalHttpNarratorTransport, MockNarrator,
        NarrationHook, NarrationRoute, Narrator, NarratorStatusKind, NarratorTransport,
        build_narrator_context, narration_hook_for_action, narration_route, parse_compact_proposal,
        parse_http_endpoint, parse_narrator_response,
    };

    struct FixtureTransport {
        response: Result<String, String>,
    }

    impl NarratorTransport for FixtureTransport {
        fn complete(&self, request: &str) -> Result<String, String> {
            let response = self.response.clone()?;
            if request.starts_with("COCKPIT/PROPOSAL-FORM\n") {
                let kind = request
                    .lines()
                    .find_map(|line| line.strip_prefix("KIND: "))
                    .ok_or_else(|| "fixture form omitted kind".to_owned())?;
                let proposal = fixture_proposal(&response, Some(kind))?;
                return fixture_compact_form(&proposal, kind);
            }
            Ok(response)
        }
    }

    fn fixture_proposal(
        response: &str,
        requested_kind: Option<&str>,
    ) -> Result<serde_json::Value, String> {
        let value: serde_json::Value =
            serde_json::from_str(response).map_err(|error| error.to_string())?;
        value
            .get("proposals")
            .and_then(serde_json::Value::as_array)
            .and_then(|proposals| {
                proposals.iter().find(|proposal| {
                    requested_kind.is_none_or(|kind| {
                        proposal.get("kind").and_then(serde_json::Value::as_str) == Some(kind)
                    })
                })
            })
            .cloned()
            .ok_or_else(|| "fixture response omitted a matching proposal".to_owned())
    }

    fn fixture_compact_form(proposal: &serde_json::Value, kind: &str) -> Result<String, String> {
        let fields = super::proposal_form_fields(kind)
            .ok_or_else(|| "fixture kind has no form".to_owned())?;
        fields
            .iter()
            .map(|label| {
                let key = match *label {
                    // Compact-status labels are intentionally player-facing while the typed
                    // response preserves the evaluator's explicit requirement names.
                    "TARGET_TAGS" => "required_target_tags".to_owned(),
                    "CONTEXT_TAGS" => "required_context_tags".to_owned(),
                    _ => label.to_ascii_lowercase(),
                };
                let value = proposal
                    .get(&key)
                    .and_then(|value| {
                        value
                            .as_str()
                            .map(str::to_owned)
                            .or_else(|| value.as_i64().map(|value| value.to_string()))
                            .or_else(|| value.as_bool().map(|value| value.to_string()))
                            .or_else(|| {
                                value.as_array().map(|values| {
                                    values
                                        .iter()
                                        .filter_map(serde_json::Value::as_str)
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                })
                            })
                    })
                    .or_else(|| match *label {
                        "HAS_HEALTH"
                        | "CAN_ENGAGE"
                        | "CAN_HEAR"
                        | "CAN_SEE"
                        | "CAN_MOVE"
                        | "CAN_CARRY_INVENTORY"
                        | "CAN_SPAWN_FROM_NOISE"
                        | "CAN_TRACK_NOISE"
                        | "CAN_TRACK_SIGHT" => Some("false".to_owned()),
                        "MAX_HP" | "ACTOR_DAMAGE" => Some("0".to_owned()),
                        "ADDITIVE_DAMAGE" | "ADDITIVE_HEALING" => Some("0".to_owned()),
                        "TARGET_TAGS" | "CONTEXT_TAGS" => Some("none".to_owned()),
                        "DISPOSITION" => Some("neutral".to_owned()),
                        _ => None,
                    })
                    .ok_or_else(|| format!("fixture proposal omitted {key}"))?;
                Ok(format!("{label}: {value}"))
            })
            .collect::<Result<Vec<_>, String>>()
            .map(|lines| lines.join("\n"))
    }

    struct QueuedTransport {
        responses: Mutex<Vec<Result<String, String>>>,
    }

    impl NarratorTransport for QueuedTransport {
        fn complete(&self, _: &str) -> Result<String, String> {
            self.responses
                .lock()
                .expect("test response queue should be available")
                .remove(0)
        }
    }

    #[test]
    fn live_adapter_uses_valid_response_and_falls_back_on_malformed_output() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let state = generate_new_run(&bundle).state;
        let live = LiveNarratorAdapter::new(
            FixtureTransport {
                response: Ok(
                    r#"{"narration":"Validated live narration.","proposals":[]}"#.to_owned(),
                ),
            },
            MockNarrator::new(&bundle, 0.0),
        );
        assert_eq!(
            live.narrate_player_message(&bundle, &state, "free text", None),
            vec!["Validated live narration."]
        );
        let fallback = LiveNarratorAdapter::new(
            FixtureTransport {
                response: Ok("malformed".to_owned()),
            },
            MockNarrator::new(&bundle, 0.0),
        );
        assert!(!fallback.narrate_run_start(&bundle, &state, &[]).is_empty());
    }

    #[test]
    fn selected_live_failure_is_visible_and_never_impersonates_mock() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let state = generate_new_run(&bundle).state;
        let live = LiveNarratorAdapter::new(
            FixtureTransport {
                response: Err("connection refused".to_owned()),
            },
            MockNarrator::new(&bundle, 0.0),
        );

        let lines = live.narrate_run_start(&bundle, &state, &[]);
        assert!(lines[0].contains("Narrator unavailable: connection refused"));
        assert_eq!(live.status().kind, NarratorStatusKind::Unavailable);
    }

    #[test]
    fn malformed_proposal_lane_keeps_successful_narration_live() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let state = generate_new_run(&bundle).state;
        let live = LiveNarratorAdapter::new(
            QueuedTransport {
                responses: Mutex::new(vec![
                    Ok("The room holds its breath.".to_owned()),
                    Ok(r#"{"proposals":["#.to_owned()),
                    Ok(r#"{"proposals":["#.to_owned()),
                ]),
            },
            MockNarrator::new(&bundle, 0.0),
        );

        assert_eq!(
            live.narrate_player_message(&bundle, &state, "free text", Some("item")),
            vec!["The room holds its breath."]
        );
        assert_eq!(live.status().kind, NarratorStatusKind::Ready);
        assert!(live.take_item_proposals().is_empty());
        assert!(
            live.take_proposal_diagnostics()
                .iter()
                .any(|line| line.contains("discarded for this turn"))
        );
    }

    #[test]
    fn host_forms_expose_only_template_required_fields() {
        assert_eq!(
            super::proposal_form_fields("item"),
            Some(vec![
                "NAME",
                "DESCRIPTION",
                "DAMAGE",
                "TAGS",
                "HAS_HEALTH",
                "MAX_HP",
                "CAN_ENGAGE",
                "ACTOR_DAMAGE",
                "CAN_HEAR",
                "CAN_SEE",
                "CAN_MOVE",
                "CAN_CARRY_INVENTORY",
                "CAN_SPAWN_FROM_NOISE",
                "CAN_TRACK_NOISE",
                "CAN_TRACK_SIGHT",
                "DISPOSITION",
            ])
        );
        assert_eq!(
            super::proposal_form_fields("npc"),
            Some(vec![
                "NAME",
                "DESCRIPTION",
                "FACTION",
                "TAGS",
                "HAS_HEALTH",
                "MAX_HP",
                "CAN_ENGAGE",
                "ACTOR_DAMAGE",
                "CAN_HEAR",
                "CAN_SEE",
                "CAN_MOVE",
                "CAN_CARRY_INVENTORY",
                "CAN_SPAWN_FROM_NOISE",
                "CAN_TRACK_NOISE",
                "CAN_TRACK_SIGHT",
                "DISPOSITION",
            ])
        );
        assert_eq!(
            super::proposal_form_field_line("DURATION"),
            "DURATION: <one of scene, until_used, run>"
        );
    }

    #[test]
    fn cockpit_prompts_keep_narration_and_proposal_lanes_separate() {
        let (narration_system, _) = super::narrator_prompt("ordinary narration request");
        let (form_system, _) = super::narrator_prompt(
            "COCKPIT/PROPOSAL-FORM\nKIND: item\nNAME:\nDESCRIPTION:\nDAMAGE:",
        );
        assert!(narration_system.contains("no JSON, labels, rules explanation, proposal"));
        assert!(form_system.contains("never output LABEL:"));
    }

    #[test]
    fn pack_routes_routine_events_without_invoking_selected_narrator() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let routine = ActionOutcome {
            events: Vec::new(),
            lines: vec!["You wait.".to_owned()],
        };
        assert_eq!(
            narration_hook_for_action(&GameAction::Wait, &routine),
            NarrationHook::Routine
        );
        assert_eq!(
            narration_route(&bundle, NarrationHook::Routine),
            NarrationRoute::Mock
        );
        assert_eq!(
            narration_route(&bundle, NarrationHook::Inspection),
            NarrationRoute::SelectedNarrator
        );

        bundle.rules.narration.usage = "unused".to_owned();
        assert_eq!(
            narration_route(&bundle, NarrationHook::Critical),
            NarrationRoute::None
        );
    }

    #[test]
    fn tight_and_unleashed_expose_different_host_owned_proposal_templates() {
        let tight = super::allowed_proposal_kinds(
            &crate::game::PlayContract::from_preset_name("rules_first"),
            0.0,
        );
        let unleashed = super::allowed_proposal_kinds(
            &crate::game::PlayContract::from_preset_name("unleashed"),
            1.0,
        );
        assert!(tight.is_empty());
        assert_eq!(
            unleashed,
            vec![
                "item",
                "status",
                "npc",
                "enemy",
                "boss",
                "objective",
                "temporary_exit",
                "ability_grant"
            ]
        );
    }

    #[test]
    fn free_text_proposal_gate_has_distinct_tight_guided_loose_and_wild_rates() {
        let contract = crate::game::PlayContract::from_preset_name("unleashed");
        assert_eq!(
            super::decide_free_text_proposals(&contract, 0.0, 1, "free text").probability_percent,
            0
        );
        assert_eq!(
            super::decide_free_text_proposals(&contract, 1.0 / 3.0, 1, "free text")
                .probability_percent,
            25
        );
        assert_eq!(
            super::decide_free_text_proposals(&contract, 2.0 / 3.0, 1, "free text")
                .probability_percent,
            60
        );
        let wild = super::decide_free_text_proposals(&contract, 1.0, 1, "search the room");
        assert_eq!(wild.probability_percent, 100);
        assert!(wild.permitted);
        assert!(!super::is_proposal_eligible_free_text("yell at the sink"));
        assert!(super::is_proposal_eligible_free_text(
            "search the cupboards"
        ));
        assert!(super::is_proposal_eligible_free_text(
            "I pray to the elder gods for help"
        ));
    }

    #[test]
    fn host_selects_an_authorized_proposal_kind_without_model_routing() {
        let allowed = vec!["item".to_owned(), "npc".to_owned(), "objective".to_owned()];
        let first = super::select_host_proposal_kind(&allowed, 17, "search the room")
            .expect("a host-selected kind");
        assert!(allowed.contains(&first));
        assert_eq!(
            Some(first),
            super::select_host_proposal_kind(&allowed, 17, "search the room")
        );
        let selected = (0..32)
            .filter_map(|turn| super::select_host_proposal_kind(&allowed, turn, "search the room"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(
            selected.len() > 1,
            "host selection should vary across deterministic turns rather than locking searches to one kind"
        );
    }

    #[test]
    fn ambient_discoveries_are_host_owned_rare_and_cooldown_limited() {
        let contract = crate::game::PlayContract::from_preset_name("unleashed");
        let first_eligible_turn = (6..200)
            .find(|turn| super::decide_ambient_proposal(&contract, 1.0, *turn, None).permitted)
            .expect("the deterministic rare-event stream should eventually produce an opportunity");
        let decision = super::decide_ambient_proposal(&contract, 1.0, first_eligible_turn, None);
        assert_eq!(decision.probability_percent, 6);
        assert_eq!(decision.cooldown_turns, 6);
        assert!(decision.permitted);
        assert!(
            !super::decide_ambient_proposal(
                &contract,
                1.0,
                first_eligible_turn + 1,
                Some(first_eligible_turn),
            )
            .permitted
        );
        assert!(
            !super::decide_ambient_proposal(&contract, 0.0, first_eligible_turn, None).permitted
        );
    }

    #[test]
    fn reducer_owned_movement_is_narration_only_even_when_wild_is_enabled() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        let action = GameAction::Move {
            destination: "kitchen".to_owned(),
        };
        let outcome = crate::game::apply_action(&mut state, &bundle, action.clone());
        let live = LiveNarratorAdapter::with_play_contract_and_chaos(
            QueuedTransport {
                responses: Mutex::new(vec![Ok("You enter the kitchen.".to_owned())]),
            },
            MockNarrator::new(&bundle, 1.0),
            crate::game::PlayContract::from_preset_name("unleashed"),
            1.0,
        );

        assert_eq!(
            live.narrate_action(&bundle, &action, &outcome, &state, None),
            vec!["You enter the kitchen."]
        );
        assert!(live.take_item_proposals().is_empty());
        assert!(live.take_proposal_diagnostics().is_empty());
    }

    #[test]
    fn live_adapter_queues_only_schema_validated_item_proposals() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let state = generate_new_run(&bundle).state;
        let live = LiveNarratorAdapter::new(
            FixtureTransport {
                response: Ok(
                    r#"{"narration":"An offer arrives.","proposals":[{"kind":"item","name":"Signal Flare","description":"A bright emergency flare.","damage":2,"tags":["tool","light"]}]}"#.to_owned(),
                ),
            },
            MockNarrator::new(&bundle, 0.0),
        );

        live.narrate_player_message(&bundle, &state, "free text", Some("item"));
        let proposals = live.take_item_proposals();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].name, "Signal Flare");
        assert_eq!(proposals[0].tags, vec!["item", "light", "tool"]);
        assert!(live.take_item_proposals().is_empty());
    }

    #[test]
    fn an_unaccepted_live_proposal_never_mutates_canonical_run_state() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let state = generate_new_run(&bundle).state;
        let before = serde_json::to_value(&state).expect("run serializes");
        let live = LiveNarratorAdapter::new(
            FixtureTransport { response: Ok(r#"{"narration":"An offer appears.","proposals":[{"kind":"item","name":"Unclaimed tool","description":"Still only an offer.","damage":3,"tags":["tool"]},{"kind":"temporary_exit","name":"Unclaimed route","description":"Still only an offer.","from_location_id":"front_verandah","to_location_id":"laundry"}]}"#.to_owned()) },
            MockNarrator::new(&bundle, 0.0),
        );

        live.narrate_player_message(&bundle, &state, "free text", Some("item"));

        assert_eq!(
            serde_json::to_value(&state).expect("run serializes"),
            before
        );
        assert_eq!(live.take_item_proposals().len(), 1);
        assert!(live.take_temporary_exit_proposals().is_empty());
    }

    #[test]
    fn live_adapter_queues_schema_validated_status_proposals_separately() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let state = generate_new_run(&bundle).state;
        let live = LiveNarratorAdapter::new(
            FixtureTransport {
                response: Ok(
                    r#"{"narration":"Focus settles in.","proposals":[{"kind":"status","name":"Steady Hands","trigger":"combat","duration":"until_used","additive_damage":3,"required_target_tags":["giant"]}]}"#.to_owned(),
                ),
            },
            MockNarrator::new(&bundle, 0.0),
        );
        live.narrate_player_message(&bundle, &state, "free text", Some("status"));
        let proposals = live.take_status_proposals();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].name, "Steady Hands");
        assert_eq!(proposals[0].additive_damage, 3);
        assert_eq!(proposals[0].required_target_tags, ["giant"]);
    }

    #[test]
    fn compact_status_form_carries_only_reducer_supported_effect_fields() {
        let proposal = parse_compact_proposal(
            "status",
            "NAME: Breaker Focus\nTRIGGER: combat\nDURATION: until_used\nADDITIVE_DAMAGE: 4\nADDITIVE_HEALING: 0\nTARGET_TAGS: boss, armored\nCONTEXT_TAGS: none",
        )
        .expect("a bounded combat status should parse");
        let validated = super::ValidatedStatusProposal::from(proposal);
        assert_eq!(validated.additive_damage, 4);
        assert_eq!(validated.required_target_tags, ["boss", "armored"]);
        assert!(validated.required_context_tags.is_empty());

        assert!(parse_compact_proposal(
            "status",
            "NAME: Decorative Aura\nTRIGGER: wait\nDURATION: run\nADDITIVE_DAMAGE: 0\nADDITIVE_HEALING: 0\nTARGET_TAGS: none\nCONTEXT_TAGS: none",
        )
        .is_err());
    }

    #[test]
    fn compact_ability_grant_form_carries_only_existing_grant_state() {
        let proposal = parse_compact_proposal(
            "ability_grant",
            "TEMPLATE_ID: grave_bolt\nREMAINING_USES: 3\nDURATION_TURNS: permanent",
        )
        .expect("a bounded grant should parse");
        let validated = super::ValidatedAbilityGrantProposal::from(proposal);
        assert_eq!(validated.template_id, "grave_bolt");
        assert_eq!(validated.remaining_uses, Some(3));
        assert_eq!(validated.duration_turns, None);
        assert!(
            parse_compact_proposal(
                "ability_grant",
                "TEMPLATE_ID: grave_bolt\nREMAINING_USES: 0\nDURATION_TURNS: permanent",
            )
            .is_err()
        );
    }

    #[test]
    fn live_adapter_queues_schema_validated_npc_proposals_separately() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let state = generate_new_run(&bundle).state;
        let live = LiveNarratorAdapter::new(
            FixtureTransport { response: Ok(r#"{"narration":"A survivor arrives.","proposals":[{"kind":"npc","name":"Mara","description":"A sharp-eyed survivor.","faction":"survivors","tags":["guide"]}]}"#.to_owned()) },
            MockNarrator::new(&bundle, 0.0),
        );
        live.narrate_player_message(&bundle, &state, "free text", Some("npc"));
        let proposals = live.take_npc_proposals();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].faction, "survivors");
    }

    #[test]
    fn live_adapter_queues_one_schema_validated_proposal_per_turn() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let state = generate_new_run(&bundle).state;
        let live = LiveNarratorAdapter::new(
            FixtureTransport { response: Ok(r#"{"narration":"Two threats arrive.","proposals":[{"kind":"enemy","name":"Fog hound","description":"A fast threat.","max_hp":6,"damage":2,"tags":["fog"]},{"kind":"boss","name":"Fog king","description":"A large threat.","max_hp":18,"damage":4,"tags":["fog","royal"]}]}"#.to_owned()) },
            MockNarrator::new(&bundle, 0.0),
        );
        live.narrate_player_message(&bundle, &state, "free text", Some("enemy"));
        let enemies = live.take_enemy_proposals();
        let bosses = live.take_boss_proposals();
        assert_eq!(enemies[0].max_hp, 6);
        assert!(bosses.is_empty());
    }

    #[test]
    fn live_adapter_queues_schema_validated_objective_proposals_separately() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let state = generate_new_run(&bundle).state;
        let live = LiveNarratorAdapter::new(
            FixtureTransport { response: Ok(r#"{"narration":"A route opens.","proposals":[{"kind":"objective","name":"Reach the laundry","description":"Find the hidden room.","completion_key":"laundry"}]}"#.to_owned()) },
            MockNarrator::new(&bundle, 0.0),
        );
        live.narrate_player_message(&bundle, &state, "free text", Some("objective"));
        let proposals = live.take_objective_proposals();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].completion_key, "laundry");
    }

    #[test]
    fn live_adapter_queues_schema_validated_temporary_exit_proposals_separately() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let state = generate_new_run(&bundle).state;
        let live = LiveNarratorAdapter::new(
            FixtureTransport { response: Ok(r#"{"narration":"A gap opens.","proposals":[{"kind":"temporary_exit","name":"Service hatch","description":"A narrow route.","from_location_id":"front_verandah","to_location_id":"laundry","locked":false}]}"#.to_owned()) },
            MockNarrator::new(&bundle, 0.0),
        );
        live.narrate_player_message(&bundle, &state, "free text", Some("temporary_exit"));
        let proposals = live.take_temporary_exit_proposals();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].to_location_id, "laundry");
    }

    #[test]
    fn malformed_narrator_proposals_are_discarded_before_admission() {
        assert!(parse_narrator_response("not json").is_err());
        assert!(parse_narrator_response(r#"{"narration":"","proposals":[]}"#).is_err());
        for raw in [
            r#"{"narration":"A thing happens.","proposals":[{"kind":"item","name":"","description":"bad","damage":1}]}"#,
            r#"{"narration":"A thing happens.","proposals":[{"kind":"enemy","name":"Bad","description":"bad stats","max_hp":0,"damage":1}]}"#,
            r#"{"narration":"A thing happens.","proposals":[{"kind":"npc","name":"Mara","description":"","faction":"survivors"}]}"#,
            r#"{"narration":"A thing happens.","proposals":[{"kind":"objective","name":"","description":"bad","completion_key":"laundry"}]}"#,
            r#"{"narration":"A thing happens.","proposals":[{"kind":"temporary_exit","name":"loop","description":"bad","from_location_id":"laundry","to_location_id":"laundry"}]}"#,
            r#"{"narration":"A thing happens.","proposals":[{"kind":"location","name":"Invented room"}]}"#,
        ] {
            let response = parse_narrator_response(raw).expect("narration should remain usable");
            assert_eq!(response.narration, "A thing happens.");
            assert!(response.proposals.is_empty());
        }
    }

    #[test]
    fn compact_proposal_form_constructs_host_owned_object() {
        let proposal = parse_compact_proposal(
            "item",
            "NAME: Signal flare\nDESCRIPTION: A bright emergency light.\nDAMAGE: 0\nHAS_HEALTH: false\nMAX_HP: 0\nCAN_ENGAGE: false\nACTOR_DAMAGE: 0\nCAN_HEAR: false\nCAN_SEE: false\nCAN_MOVE: false\nCAN_SPAWN_FROM_NOISE: false\nCAN_TRACK_NOISE: false\nCAN_TRACK_SIGHT: false\nDISPOSITION: neutral",
        )
        .expect("flat form should parse");
        assert_eq!(super::proposal_kind_name(&proposal), "item");
        let validated = super::ValidatedItemProposal::from(proposal);
        assert_eq!(validated.tags, vec!["item"]);
    }

    #[test]
    fn compact_actor_checklist_is_preserved_for_editable_admission() {
        let proposal = parse_compact_proposal(
            "npc",
            "NAME: Witness\nDESCRIPTION: An alert observer.\nFACTION: locals\nHAS_HEALTH: true\nMAX_HP: 8\nCAN_ENGAGE: true\nACTOR_DAMAGE: 2\nCAN_HEAR: true\nCAN_SEE: true\nCAN_MOVE: true\nCAN_SPAWN_FROM_NOISE: false\nCAN_TRACK_NOISE: true\nCAN_TRACK_SIGHT: true\nDISPOSITION: friendly",
        )
        .expect("actor checklist should parse");
        let validated = super::ValidatedNpcProposal::from(proposal);
        assert!(validated.actor.can_engage);
        assert!(validated.actor.can_track_sight);
        assert_eq!(validated.actor.disposition, "friendly");
    }

    #[test]
    fn compact_actor_checklist_preserves_the_inventory_capability_root() {
        let proposal = parse_compact_proposal(
            "npc",
            "NAME: Porter\nDESCRIPTION: A careful carrier.\nFACTION: locals\nHAS_HEALTH: false\nMAX_HP: 0\nCAN_ENGAGE: false\nACTOR_DAMAGE: 0\nCAN_HEAR: false\nCAN_SEE: false\nCAN_MOVE: false\nCAN_CARRY_INVENTORY: true\nCAN_SPAWN_FROM_NOISE: false\nCAN_TRACK_NOISE: false\nCAN_TRACK_SIGHT: false\nDISPOSITION: neutral",
        )
        .expect("inventory capability should remain part of the host-owned form");
        let validated = super::ValidatedNpcProposal::from(proposal);
        assert!(validated.actor.can_carry_inventory);
    }

    #[test]
    fn compact_forms_ignore_unapproved_fields_and_normalize_can_attack_only_when_authorized() {
        let proposal = parse_compact_proposal(
            "item",
            "NAME: Signal flare\nDESCRIPTION: A bright emergency light.\nDAMAGE: 0\nHAS_HEALTH: false\nMAX_HP: 0\nCAN_ATTACK: true\nACTOR_DAMAGE: 1\nCAN_HEAR: false\nCAN_SEE: false\nCAN_MOVE: false\nCAN_SPAWN_FROM_NOISE: false\nCAN_TRACK_NOISE: false\nCAN_TRACK_SIGHT: false\nDISPOSITION: neutral\nCAN_SPEAK: true",
        )
        .expect("unapproved field must not poison an otherwise valid host form");
        let validated = super::ValidatedItemProposal::from(proposal);
        assert!(validated.actor.can_engage);
    }

    #[test]
    fn malformed_compact_proposal_never_becomes_narration_or_mechanics() {
        assert!(parse_compact_proposal("item", "NAME: Just prose.").is_err());
    }

    #[test]
    fn local_http_transport_requires_explicit_local_http_endpoint() {
        let _transport = LocalHttpNarratorTransport::new(
            "http://127.0.0.1:11434/narrate",
            std::time::Duration::from_millis(50),
        );
        assert_eq!(
            parse_http_endpoint("http://127.0.0.1:11434/narrate").expect("endpoint should parse"),
            ("127.0.0.1".to_owned(), 11434, "/narrate".to_owned())
        );
        assert!(parse_http_endpoint("https://example.test/narrate").is_err());
    }

    #[test]
    fn llama_cpp_transport_reads_only_the_final_completion_content() {
        let completion: LlamaCppCompletion = serde_json::from_str(
            r#"{"choices":[{"message":{"content":"{\"narration\":\"The dock creaks.\",\"proposals\":[]}","reasoning_content":"ignored"}}]}"#,
        )
        .expect("fixture completion should deserialize");

        let content = completion
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .expect("fixture completion should include final content");
        assert_eq!(
            content,
            r#"{"narration":"The dock creaks.","proposals":[]}"#
        );
    }

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
                origin_id: "player".to_owned(),
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
            None,
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

        let narrated =
            narrator.narrate_action(&bundle, &GameAction::Attack, &outcome, &state, None);

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
        state.additional_objectives.push(ObjectiveState {
            instance_id: "scratch_objective_1".to_owned(),
            template_id: "scratch_objective_1".to_owned(),
            lifecycle: InstanceLifecycle::Active,
            bucket: ObjectiveBucket::Active,
            id: "scratch_objective_1".to_owned(),
            name: "Reach the laundry".to_owned(),
            description: "A side task.".to_owned(),
            target_boss_id: None,
            required_item_id: None,
            required_location_id: Some("laundry".to_owned()),
            completed: false,
        });
        state.temporary_exits.push(TemporaryExitInstance {
            instance_id: "scratch_exit_1".to_owned(),
            template_id: "scratch_exit_1".to_owned(),
            from_location_id: "garage".to_owned(),
            to_location_id: "laundry".to_owned(),
            locked: false,
            lifecycle: InstanceLifecycle::Active,
            provenance: "narrator_approved".to_owned(),
        });
        state.active_objective.completed = true;
        let outcome = crate::game::ActionOutcome {
            events: vec![
                GameEvent::AttackResolved {
                    origin_id: "player".to_owned(),
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
                .session_truth
                .iter()
                .any(|fact| fact.contains("Reach the laundry"))
        );
        assert!(
            context
                .session_truth
                .iter()
                .any(|fact| fact.contains("garage -> laundry"))
        );
        assert!(context.session_truth.len() <= 12);
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
