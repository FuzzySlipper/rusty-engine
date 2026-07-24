use std::collections::{BTreeMap, BTreeSet, VecDeque};

use render_model::{RenderAssetError, RenderAssetKind};
use serde::{Deserialize, Serialize};

use crate::{verify_asset, PresentationAssetError, PresentationAssetLookup};

pub const ANIMATION_CATALOG_SCHEMA_VERSION: u32 = 1;
pub const BLEND_WEIGHT_SCALE: i32 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationCatalog {
    pub schema_version: u32,
    pub catalog_id: String,
    pub assets: Vec<AnimationClipAsset>,
    pub graphs: Vec<AnimationGraphDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationClipAsset {
    pub asset_id: String,
    pub content_hash: String,
    pub clips: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationGraphDefinition {
    pub graph_id: String,
    pub version: u32,
    pub asset_id: String,
    pub initial_state_id: String,
    pub parameters: Vec<AnimationParameterDefinition>,
    pub states: Vec<AnimationStateDefinition>,
    pub transitions: Vec<AnimationTransitionDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationParameterDefinition {
    pub parameter_id: String,
    pub kind: AnimationParameterKind,
    pub default_value: AnimationParameterValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnimationParameterKind {
    Float,
    Bool,
    Trigger,
}

/// Float parameters use signed thousandths so graph selection is identical in
/// Rust and browser hosts.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum AnimationParameterValue {
    Float(i32),
    Bool(bool),
    Trigger(bool),
}

impl AnimationParameterValue {
    pub const fn kind(&self) -> AnimationParameterKind {
        match self {
            Self::Float(_) => AnimationParameterKind::Float,
            Self::Bool(_) => AnimationParameterKind::Bool,
            Self::Trigger(_) => AnimationParameterKind::Trigger,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationStateDefinition {
    pub state_id: String,
    pub motion: AnimationMotionDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AnimationMotionDefinition {
    Clip {
        clip_id: String,
        speed_milli: i32,
    },
    LinearBlend {
        parameter_id: String,
        low_clip_id: String,
        high_clip_id: String,
        minimum_milli: i32,
        maximum_milli: i32,
        speed_milli: i32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationTransitionDefinition {
    pub transition_id: String,
    pub from_state_id: String,
    pub to_state_id: String,
    /// Lower values win. Priorities must be unique per source state.
    pub priority: u16,
    pub duration_ticks: u32,
    pub conditions: Vec<AnimationCondition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AnimationCondition {
    FloatGreaterThan {
        parameter_id: String,
        threshold_milli: i32,
    },
    FloatLessThanOrEqual {
        parameter_id: String,
        threshold_milli: i32,
    },
    BoolEquals {
        parameter_id: String,
        value: bool,
    },
    TriggerSet {
        parameter_id: String,
    },
}

impl AnimationCondition {
    pub(crate) fn parameter_id(&self) -> &str {
        match self {
            Self::FloatGreaterThan { parameter_id, .. }
            | Self::FloatLessThanOrEqual { parameter_id, .. }
            | Self::BoolEquals { parameter_id, .. }
            | Self::TriggerSet { parameter_id } => parameter_id,
        }
    }

    const fn expected_kind(&self) -> AnimationParameterKind {
        match self {
            Self::FloatGreaterThan { .. } | Self::FloatLessThanOrEqual { .. } => {
                AnimationParameterKind::Float
            }
            Self::BoolEquals { .. } => AnimationParameterKind::Bool,
            Self::TriggerSet { .. } => AnimationParameterKind::Trigger,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnimationCatalogDiagnosticCode {
    UnsupportedSchema,
    InvalidId,
    DuplicateId,
    MissingAsset,
    AssetKindMismatch,
    ContentHashMismatch,
    MissingClip,
    MissingState,
    MissingParameter,
    ParameterTypeMismatch,
    InvalidPlaybackSpeed,
    InvalidBlendRange,
    AmbiguousTransition,
    UnreachableState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationCatalogDiagnostic {
    pub code: AnimationCatalogDiagnosticCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationCatalogValidationError {
    pub diagnostics: Vec<AnimationCatalogDiagnostic>,
}

impl core::fmt::Display for AnimationCatalogValidationError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            formatter,
            "animation catalog rejected with {} diagnostic(s)",
            self.diagnostics.len()
        )
    }
}

impl std::error::Error for AnimationCatalogValidationError {}

#[derive(Debug, Clone)]
pub struct ValidatedAnimationCatalog {
    source: AnimationCatalog,
    pub(crate) graphs: BTreeMap<String, ValidatedGraph>,
    assets: BTreeMap<String, AnimationClipAsset>,
}

impl ValidatedAnimationCatalog {
    pub fn source(&self) -> &AnimationCatalog {
        &self.source
    }

    pub fn graph_definition(&self, graph_id: &str) -> Option<&AnimationGraphDefinition> {
        self.graphs.get(graph_id).map(|graph| &graph.definition)
    }

    pub fn clip_asset(&self, asset_id: &str) -> Option<&AnimationClipAsset> {
        self.assets.get(asset_id)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ValidatedGraph {
    pub definition: AnimationGraphDefinition,
    pub states: BTreeMap<String, AnimationStateDefinition>,
    pub parameters: BTreeMap<String, AnimationParameterDefinition>,
    pub transitions: BTreeMap<String, Vec<AnimationTransitionDefinition>>,
}

pub fn validate_animation_catalog(
    catalog: AnimationCatalog,
    assets: &impl PresentationAssetLookup,
) -> Result<ValidatedAnimationCatalog, AnimationCatalogValidationError> {
    let mut diagnostics = Vec::new();
    if catalog.schema_version != ANIMATION_CATALOG_SCHEMA_VERSION {
        diagnostic(
            &mut diagnostics,
            AnimationCatalogDiagnosticCode::UnsupportedSchema,
            "schemaVersion",
            "only animation catalog schema version 1 is supported",
        );
    }
    validate_id(&catalog.catalog_id, "catalogId", &mut diagnostics);

    let mut clip_assets = BTreeMap::<String, AnimationClipAsset>::new();
    for (asset_index, asset) in catalog.assets.iter().enumerate() {
        let path = format!("assets[{asset_index}]");
        if asset.content_hash.is_empty() {
            diagnostic(
                &mut diagnostics,
                AnimationCatalogDiagnosticCode::ContentHashMismatch,
                format!("{path}.contentHash"),
                "animated mesh content hash must be non-empty",
            );
        }
        match verify_asset(
            assets,
            &asset.asset_id,
            RenderAssetKind::AnimatedMesh,
            Some(&asset.content_hash),
        ) {
            Ok(()) => {}
            Err(error) => {
                let (code, message) = asset_diagnostic(error);
                diagnostic(&mut diagnostics, code, format!("{path}.assetId"), message);
            }
        }
        let mut clips = BTreeSet::new();
        for (clip_index, clip) in asset.clips.iter().enumerate() {
            validate_id(
                clip,
                &format!("{path}.clips[{clip_index}]"),
                &mut diagnostics,
            );
            if !clips.insert(clip.clone()) {
                diagnostic(
                    &mut diagnostics,
                    AnimationCatalogDiagnosticCode::DuplicateId,
                    format!("{path}.clips[{clip_index}]"),
                    "clip id is duplicated in the asset",
                );
            }
        }
        if clip_assets
            .insert(asset.asset_id.clone(), asset.clone())
            .is_some()
        {
            diagnostic(
                &mut diagnostics,
                AnimationCatalogDiagnosticCode::DuplicateId,
                format!("{path}.assetId"),
                "animated mesh asset is duplicated",
            );
        }
    }

    let mut graph_ids = BTreeSet::new();
    let mut validated_graphs = BTreeMap::new();
    for (graph_index, graph) in catalog.graphs.iter().enumerate() {
        let path = format!("graphs[{graph_index}]");
        validate_id(
            &graph.graph_id,
            &format!("{path}.graphId"),
            &mut diagnostics,
        );
        if !graph_ids.insert(graph.graph_id.clone()) {
            diagnostic(
                &mut diagnostics,
                AnimationCatalogDiagnosticCode::DuplicateId,
                format!("{path}.graphId"),
                "graph id is duplicated",
            );
        }
        if graph.version == 0 {
            diagnostic(
                &mut diagnostics,
                AnimationCatalogDiagnosticCode::InvalidId,
                format!("{path}.version"),
                "graph version must be non-zero",
            );
        }
        let Some(asset) = clip_assets.get(&graph.asset_id) else {
            diagnostic(
                &mut diagnostics,
                AnimationCatalogDiagnosticCode::MissingAsset,
                format!("{path}.assetId"),
                "graph references an unknown animated mesh",
            );
            continue;
        };
        let clips = asset.clips.iter().map(String::as_str).collect();
        if let Some(validated) = validate_graph(graph, &clips, &path, &mut diagnostics) {
            validated_graphs.insert(graph.graph_id.clone(), validated);
        }
    }

    if diagnostics.is_empty() {
        Ok(ValidatedAnimationCatalog {
            source: catalog,
            graphs: validated_graphs,
            assets: clip_assets,
        })
    } else {
        Err(AnimationCatalogValidationError { diagnostics })
    }
}

fn validate_graph(
    graph: &AnimationGraphDefinition,
    asset_clips: &BTreeSet<&str>,
    path: &str,
    diagnostics: &mut Vec<AnimationCatalogDiagnostic>,
) -> Option<ValidatedGraph> {
    let mut parameters = BTreeMap::new();
    for (index, parameter) in graph.parameters.iter().enumerate() {
        let parameter_path = format!("{path}.parameters[{index}]");
        validate_id(
            &parameter.parameter_id,
            &format!("{parameter_path}.parameterId"),
            diagnostics,
        );
        if parameter.kind != parameter.default_value.kind() {
            diagnostic(
                diagnostics,
                AnimationCatalogDiagnosticCode::ParameterTypeMismatch,
                format!("{parameter_path}.defaultValue"),
                "default value does not match the declared parameter kind",
            );
        }
        if parameters
            .insert(parameter.parameter_id.clone(), parameter.clone())
            .is_some()
        {
            diagnostic(
                diagnostics,
                AnimationCatalogDiagnosticCode::DuplicateId,
                format!("{parameter_path}.parameterId"),
                "parameter id is duplicated",
            );
        }
    }

    let mut states = BTreeMap::new();
    for (index, state) in graph.states.iter().enumerate() {
        let state_path = format!("{path}.states[{index}]");
        validate_id(
            &state.state_id,
            &format!("{state_path}.stateId"),
            diagnostics,
        );
        validate_motion(
            &state.motion,
            asset_clips,
            &parameters,
            &format!("{state_path}.motion"),
            diagnostics,
        );
        if states
            .insert(state.state_id.clone(), state.clone())
            .is_some()
        {
            diagnostic(
                diagnostics,
                AnimationCatalogDiagnosticCode::DuplicateId,
                format!("{state_path}.stateId"),
                "state id is duplicated",
            );
        }
    }
    if !states.contains_key(&graph.initial_state_id) {
        diagnostic(
            diagnostics,
            AnimationCatalogDiagnosticCode::MissingState,
            format!("{path}.initialStateId"),
            "initial state is not declared by the graph",
        );
    }

    let mut transition_ids = BTreeSet::new();
    let mut priorities = BTreeSet::new();
    let mut transitions = BTreeMap::<String, Vec<AnimationTransitionDefinition>>::new();
    for (index, transition) in graph.transitions.iter().enumerate() {
        let transition_path = format!("{path}.transitions[{index}]");
        validate_id(
            &transition.transition_id,
            &format!("{transition_path}.transitionId"),
            diagnostics,
        );
        if !transition_ids.insert(transition.transition_id.clone()) {
            diagnostic(
                diagnostics,
                AnimationCatalogDiagnosticCode::DuplicateId,
                format!("{transition_path}.transitionId"),
                "transition id is duplicated",
            );
        }
        if !states.contains_key(&transition.from_state_id) {
            diagnostic(
                diagnostics,
                AnimationCatalogDiagnosticCode::MissingState,
                format!("{transition_path}.fromStateId"),
                "transition source state is not declared",
            );
        }
        if !states.contains_key(&transition.to_state_id) {
            diagnostic(
                diagnostics,
                AnimationCatalogDiagnosticCode::MissingState,
                format!("{transition_path}.toStateId"),
                "transition target state is not declared",
            );
        }
        if !priorities.insert((transition.from_state_id.clone(), transition.priority)) {
            diagnostic(
                diagnostics,
                AnimationCatalogDiagnosticCode::AmbiguousTransition,
                format!("{transition_path}.priority"),
                "two transitions from the same state have equal priority",
            );
        }
        for (condition_index, condition) in transition.conditions.iter().enumerate() {
            let condition_path = format!("{transition_path}.conditions[{condition_index}]");
            match parameters.get(condition.parameter_id()) {
                None => diagnostic(
                    diagnostics,
                    AnimationCatalogDiagnosticCode::MissingParameter,
                    format!("{condition_path}.parameterId"),
                    "condition references an undeclared parameter",
                ),
                Some(parameter) if parameter.kind != condition.expected_kind() => diagnostic(
                    diagnostics,
                    AnimationCatalogDiagnosticCode::ParameterTypeMismatch,
                    format!("{condition_path}.parameterId"),
                    "condition kind does not match the referenced parameter",
                ),
                Some(_) => {}
            }
        }
        transitions
            .entry(transition.from_state_id.clone())
            .or_default()
            .push(transition.clone());
    }
    for candidates in transitions.values_mut() {
        candidates.sort_by(|left, right| {
            left.priority
                .cmp(&right.priority)
                .then_with(|| left.transition_id.cmp(&right.transition_id))
        });
    }

    validate_reachability(graph, &states, diagnostics, path);
    Some(ValidatedGraph {
        definition: graph.clone(),
        states,
        parameters,
        transitions,
    })
}

fn validate_motion(
    motion: &AnimationMotionDefinition,
    asset_clips: &BTreeSet<&str>,
    parameters: &BTreeMap<String, AnimationParameterDefinition>,
    path: &str,
    diagnostics: &mut Vec<AnimationCatalogDiagnostic>,
) {
    match motion {
        AnimationMotionDefinition::Clip {
            clip_id,
            speed_milli,
        } => {
            if *speed_milli <= 0 {
                diagnostic(
                    diagnostics,
                    AnimationCatalogDiagnosticCode::InvalidPlaybackSpeed,
                    format!("{path}.speedMilli"),
                    "clip playback speed must be positive",
                );
            }
            if !asset_clips.contains(clip_id.as_str()) {
                diagnostic(
                    diagnostics,
                    AnimationCatalogDiagnosticCode::MissingClip,
                    format!("{path}.clipId"),
                    "motion references a clip absent from the animated mesh",
                );
            }
        }
        AnimationMotionDefinition::LinearBlend {
            parameter_id,
            low_clip_id,
            high_clip_id,
            minimum_milli,
            maximum_milli,
            speed_milli,
        } => {
            if *speed_milli <= 0 {
                diagnostic(
                    diagnostics,
                    AnimationCatalogDiagnosticCode::InvalidPlaybackSpeed,
                    format!("{path}.speedMilli"),
                    "linear blend playback speed must be positive",
                );
            }
            for (field, clip) in [("lowClipId", low_clip_id), ("highClipId", high_clip_id)] {
                if !asset_clips.contains(clip.as_str()) {
                    diagnostic(
                        diagnostics,
                        AnimationCatalogDiagnosticCode::MissingClip,
                        format!("{path}.{field}"),
                        "linear blend references a clip absent from the animated mesh",
                    );
                }
            }
            match parameters.get(parameter_id) {
                None => diagnostic(
                    diagnostics,
                    AnimationCatalogDiagnosticCode::MissingParameter,
                    format!("{path}.parameterId"),
                    "linear blend references an undeclared parameter",
                ),
                Some(parameter) if parameter.kind != AnimationParameterKind::Float => diagnostic(
                    diagnostics,
                    AnimationCatalogDiagnosticCode::ParameterTypeMismatch,
                    format!("{path}.parameterId"),
                    "linear blend parameter must be float",
                ),
                Some(_) => {}
            }
            if minimum_milli >= maximum_milli {
                diagnostic(
                    diagnostics,
                    AnimationCatalogDiagnosticCode::InvalidBlendRange,
                    format!("{path}.minimumMilli"),
                    "linear blend minimum must be less than maximum",
                );
            }
        }
    }
}

fn validate_reachability(
    graph: &AnimationGraphDefinition,
    states: &BTreeMap<String, AnimationStateDefinition>,
    diagnostics: &mut Vec<AnimationCatalogDiagnostic>,
    path: &str,
) {
    if !states.contains_key(&graph.initial_state_id) {
        return;
    }
    let mut reached = BTreeSet::from([graph.initial_state_id.as_str()]);
    let mut queue = VecDeque::from([graph.initial_state_id.as_str()]);
    while let Some(current) = queue.pop_front() {
        for transition in graph
            .transitions
            .iter()
            .filter(|transition| transition.from_state_id == current)
        {
            if states.contains_key(&transition.to_state_id)
                && reached.insert(transition.to_state_id.as_str())
            {
                queue.push_back(transition.to_state_id.as_str());
            }
        }
    }
    for (state_index, state) in graph.states.iter().enumerate() {
        if !reached.contains(state.state_id.as_str()) {
            diagnostic(
                diagnostics,
                AnimationCatalogDiagnosticCode::UnreachableState,
                format!("{path}.states[{state_index}].stateId"),
                "state is unreachable from the initial state",
            );
        }
    }
}

fn validate_id(value: &str, path: &str, diagnostics: &mut Vec<AnimationCatalogDiagnostic>) {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'/' | b'_' | b'-')
        });
    if !valid {
        diagnostic(
            diagnostics,
            AnimationCatalogDiagnosticCode::InvalidId,
            path,
            "id must use 1-128 lowercase stable-id characters",
        );
    }
}

fn asset_diagnostic(
    error: PresentationAssetError,
) -> (AnimationCatalogDiagnosticCode, &'static str) {
    match error {
        PresentationAssetError::Missing(_) => (
            AnimationCatalogDiagnosticCode::MissingAsset,
            "animated mesh is unavailable",
        ),
        PresentationAssetError::Invalid(
            RenderAssetError::ContentHashMismatch { .. } | RenderAssetError::EmptyContentHash,
        ) => (
            AnimationCatalogDiagnosticCode::ContentHashMismatch,
            "animated mesh content hash does not match",
        ),
        PresentationAssetError::Invalid(_) => (
            AnimationCatalogDiagnosticCode::AssetKindMismatch,
            "animation asset has the wrong resource kind",
        ),
    }
}

fn diagnostic(
    diagnostics: &mut Vec<AnimationCatalogDiagnostic>,
    code: AnimationCatalogDiagnosticCode,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    diagnostics.push(AnimationCatalogDiagnostic {
        code,
        path: path.into(),
        message: message.into(),
    });
}
