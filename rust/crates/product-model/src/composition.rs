use std::collections::{BTreeMap, BTreeSet};

use serde::{
    de::{Error as DeError, MapAccess, SeqAccess, Visitor},
    Deserialize, Serialize,
};
use serde_json::Value;

use crate::{diagnostic::failure, manifest::validate_identity, ProductModelError};

const SOURCE: &str = "compiled-composition.json";
pub const MAX_COMPILED_COMPOSITION_BYTES: usize = 1_048_576;
pub const MAX_INPUT_MAP_ENTRIES: usize = 256;
pub const MAX_INTENT_DESCRIPTORS: usize = 256;
pub const MAX_INPUT_CHORD_CONTROLS: usize = 8;
pub const MAX_SCHEDULE_ENTRIES: usize = 512;
pub const MAX_SCHEDULE_ACCESS_DECLARATIONS: usize = 64;
pub const MAX_SCHEDULE_DEPENDENCIES: usize = 64;
pub const SCHEDULE_PHASE_COUNT: usize = 5;
pub const MAX_GAMEPLAY_DEFINITIONS: usize = 512;
pub const MAX_TIMELINES: usize = 256;
pub const MAX_TIMELINE_STEPS: usize = 256;
pub const MAX_CAPABILITY_BINDINGS: usize = 512;
pub const MAX_OPAQUE_JSON_DEPTH: usize = 32;
pub const MAX_OPAQUE_JSON_NODES: usize = 4_096;
pub const MAX_OPAQUE_JSON_STRING_BYTES: usize = 16_384;
pub const MAX_OPAQUE_JSON_ARRAY_ENTRIES: usize = 1_024;
pub const MAX_OPAQUE_JSON_OBJECT_ENTRIES: usize = 1_024;
pub const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;
/// Maximum canonical JSON bytes one direct product-payload intent may carry.
/// This is intentionally smaller than an entire compiled composition and is
/// shared with host adapters through the generated contract descriptor.
pub const MAX_DIRECT_INTENT_PRODUCT_PAYLOAD_BYTES: usize = 65_536;

/// A validated, immutable current Compiled Composition.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledComposition {
    candidate: CompiledCompositionCandidate,
    canonical_bytes: Vec<u8>,
}

impl CompiledComposition {
    pub fn candidate(&self) -> &CompiledCompositionCandidate {
        &self.candidate
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

/// Current schema input. Array order is preserved because each listed section
/// has authored or execution significance; object key ordering is canonicalized
/// only in opaque JSON payloads when encoding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompiledCompositionCandidate {
    pub product: String,
    pub intent_descriptors: Vec<ProductIntentDescriptor>,
    pub input_map: Vec<InputMapEntry>,
    pub schedule: Vec<SchedulePhaseDeclaration>,
    pub gameplay_definitions: Vec<GameplayDefinition>,
    pub timelines: Vec<Timeline>,
    pub capability_bindings: Vec<CapabilityBinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputMapEntry {
    pub id: String,
    pub intent: String,
    pub trigger: InputTrigger,
}

/// One typed product intent. A descriptor may optionally retain one closed
/// capability linkage for legacy Product Kernel execution. Engine-owned VM
/// products use the same typed intent and mapping shape without a capability
/// binding; physical controls and direct UI claims still converge on the one
/// descriptor identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductIntentDescriptor {
    pub id: String,
    pub value_kind: IntentValueKind,
    /// Stable downstream-owned schema identity required only for a direct UI
    /// product-payload intent. It is not a capability target or dispatch key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_contract: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
    pub payload: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IntentValueKind {
    Digital,
    Axis,
    ProductPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InputEdge {
    Held,
    Pressed,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeyboardControl {
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Space,
    Enter,
    Escape,
    ShiftLeft,
    ShiftRight,
    ControlLeft,
    ControlRight,
    AltLeft,
    AltRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InputAxis {
    X,
    Y,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ControllerButton {
    #[serde(rename = "button-0")]
    Button0,
    #[serde(rename = "button-1")]
    Button1,
    #[serde(rename = "button-2")]
    Button2,
    #[serde(rename = "button-3")]
    Button3,
    #[serde(rename = "button-4")]
    Button4,
    #[serde(rename = "button-5")]
    Button5,
    #[serde(rename = "button-6")]
    Button6,
    #[serde(rename = "button-7")]
    Button7,
    #[serde(rename = "button-8")]
    Button8,
    #[serde(rename = "button-9")]
    Button9,
    #[serde(rename = "button-10")]
    Button10,
    #[serde(rename = "button-11")]
    Button11,
    #[serde(rename = "button-12")]
    Button12,
    #[serde(rename = "button-13")]
    Button13,
    #[serde(rename = "button-14")]
    Button14,
    #[serde(rename = "button-15")]
    Button15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ControllerAxis {
    #[serde(rename = "axis-0")]
    Axis0,
    #[serde(rename = "axis-1")]
    Axis1,
    #[serde(rename = "axis-2")]
    Axis2,
    #[serde(rename = "axis-3")]
    Axis3,
}

/// Explicit trigger grammar for host-neutral normalized observations. It
/// intentionally contains no opaque object that could hide newly invented
/// physical-control meaning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum InputTrigger {
    Key {
        code: KeyboardControl,
        edge: InputEdge,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        chord: Vec<KeyboardControl>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context: Option<String>,
    },
    PointerButton {
        button: PointerButton,
        edge: InputEdge,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context: Option<String>,
    },
    PointerAxis {
        axis: InputAxis,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context: Option<String>,
    },
    Wheel {
        axis: InputAxis,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context: Option<String>,
    },
    ControllerButton {
        button: ControllerButton,
        edge: InputEdge,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context: Option<String>,
    },
    ControllerAxis {
        axis: ControllerAxis,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context: Option<String>,
    },
}

impl InputTrigger {
    pub const fn value_kind(&self) -> IntentValueKind {
        match self {
            Self::Key { .. } | Self::PointerButton { .. } | Self::ControllerButton { .. } => {
                IntentValueKind::Digital
            }
            Self::PointerAxis { .. } | Self::Wheel { .. } | Self::ControllerAxis { .. } => {
                IntentValueKind::Axis
            }
        }
    }

    fn validate(&self, path: &str) -> Result<(), ProductModelError> {
        match self {
            Self::Key { chord, context, .. } => {
                validate_chord_and_context(chord, context.as_deref(), path)
            }
            Self::PointerButton { context, .. }
            | Self::PointerAxis { context, .. }
            | Self::Wheel { context, .. }
            | Self::ControllerButton { context, .. }
            | Self::ControllerAxis { context, .. } => validate_context(context.as_deref(), path),
        }
    }
}

/// The five closed Runtime Composition schedule phases. Their order is part of
/// the wire contract and is later mapped to runtime lifecycle tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SchedulePhase {
    Input,
    Simulation,
    Consequences,
    Commit,
    Projection,
}

impl SchedulePhase {
    pub const ALL: [Self; SCHEDULE_PHASE_COUNT] = [
        Self::Input,
        Self::Simulation,
        Self::Consequences,
        Self::Commit,
        Self::Projection,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Simulation => "simulation",
            Self::Consequences => "consequences",
            Self::Commit => "commit",
            Self::Projection => "projection",
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Self::Input => 0,
            Self::Simulation => 1,
            Self::Consequences => 2,
            Self::Commit => 3,
            Self::Projection => 4,
        }
    }
}

/// The explicit operation used to compose a phase around its implicit
/// `Standard.<phase>` anchor. Standard systems are owned by the runtime
/// catalog and are therefore not repeated in the compiled product artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ScheduleComposition {
    Append {
        systems: Vec<ScheduleSystem>,
    },
    Prepend {
        systems: Vec<ScheduleSystem>,
    },
    Extend {
        before: Vec<ScheduleSystem>,
        after: Vec<ScheduleSystem>,
    },
    Replace {
        systems: Vec<ScheduleSystem>,
    },
}

impl ScheduleComposition {
    pub const fn mode(&self) -> ScheduleCompositionMode {
        match self {
            Self::Append { .. } => ScheduleCompositionMode::Append,
            Self::Prepend { .. } => ScheduleCompositionMode::Prepend,
            Self::Extend { .. } => ScheduleCompositionMode::Extend,
            Self::Replace { .. } => ScheduleCompositionMode::Replace,
        }
    }

    pub fn systems(&self) -> impl Iterator<Item = (&ScheduleSystem, SchedulePlacement)> {
        let iter = match self {
            Self::Append { systems } => systems
                .iter()
                .map(|system| (system, SchedulePlacement::Append))
                .collect::<Vec<_>>(),
            Self::Prepend { systems } => systems
                .iter()
                .map(|system| (system, SchedulePlacement::Prepend))
                .collect::<Vec<_>>(),
            Self::Extend { before, after } => before
                .iter()
                .map(|system| (system, SchedulePlacement::ExtendBefore))
                .chain(
                    after
                        .iter()
                        .map(|system| (system, SchedulePlacement::ExtendAfter)),
                )
                .collect::<Vec<_>>(),
            Self::Replace { systems } => systems
                .iter()
                .map(|system| (system, SchedulePlacement::Replace))
                .collect::<Vec<_>>(),
        };
        iter.into_iter()
    }

    pub fn systems_mut(
        &mut self,
    ) -> impl Iterator<Item = (&mut ScheduleSystem, SchedulePlacement)> {
        let iter = match self {
            Self::Append { systems } => systems
                .iter_mut()
                .map(|system| (system, SchedulePlacement::Append))
                .collect::<Vec<_>>(),
            Self::Prepend { systems } => systems
                .iter_mut()
                .map(|system| (system, SchedulePlacement::Prepend))
                .collect::<Vec<_>>(),
            Self::Extend { before, after } => before
                .iter_mut()
                .map(|system| (system, SchedulePlacement::ExtendBefore))
                .chain(
                    after
                        .iter_mut()
                        .map(|system| (system, SchedulePlacement::ExtendAfter)),
                )
                .collect::<Vec<_>>(),
            Self::Replace { systems } => systems
                .iter_mut()
                .map(|system| (system, SchedulePlacement::Replace))
                .collect::<Vec<_>>(),
        };
        iter.into_iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleCompositionMode {
    Append,
    Prepend,
    Extend,
    Replace,
}

impl ScheduleCompositionMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Append => "append",
            Self::Prepend => "prepend",
            Self::Extend => "extend",
            Self::Replace => "replace",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulePlacement {
    Append,
    Prepend,
    ExtendBefore,
    ExtendAfter,
    Replace,
}

impl SchedulePlacement {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Append => "append",
            Self::Prepend => "prepend",
            Self::ExtendBefore => "extend-before",
            Self::ExtendAfter => "extend-after",
            Self::Replace => "replace",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchedulePhaseDeclaration {
    pub phase: SchedulePhase,
    #[serde(flatten)]
    pub composition: ScheduleComposition,
}

impl<'de> Deserialize<'de> for SchedulePhaseDeclaration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let mut object = value
            .as_object()
            .cloned()
            .ok_or_else(|| D::Error::custom("schedule phase must be an object"))?;
        let phase_value = object
            .remove("phase")
            .ok_or_else(|| D::Error::missing_field("phase"))?;
        let phase = serde_json::from_value::<SchedulePhase>(phase_value)
            .map_err(|error| D::Error::custom(error.to_string()))?;
        let mode_value = object
            .remove("mode")
            .ok_or_else(|| D::Error::missing_field("mode"))?;
        let mode = mode_value
            .as_str()
            .ok_or_else(|| D::Error::custom("schedule phase mode must be a string"))?;
        let composition = match mode {
            "append" => ScheduleComposition::Append {
                systems: take_systems(&mut object, "systems").map_err(D::Error::custom)?,
            },
            "prepend" => ScheduleComposition::Prepend {
                systems: take_systems(&mut object, "systems").map_err(D::Error::custom)?,
            },
            "replace" => ScheduleComposition::Replace {
                systems: take_systems(&mut object, "systems").map_err(D::Error::custom)?,
            },
            "extend" => {
                let before = take_systems(&mut object, "before").map_err(D::Error::custom)?;
                let after = take_systems(&mut object, "after").map_err(D::Error::custom)?;
                ScheduleComposition::Extend { before, after }
            }
            _ => {
                return Err(D::Error::custom(format!(
                    "unknown schedule phase mode `{mode}`"
                )))
            }
        };
        if let Some(unknown) = object.keys().next() {
            return Err(D::Error::custom(format!(
                "unknown field `{unknown}` in schedule phase"
            )));
        }
        Ok(Self { phase, composition })
    }
}

fn take_systems(
    object: &mut serde_json::Map<String, Value>,
    field: &str,
) -> Result<Vec<ScheduleSystem>, String> {
    let value = object
        .remove(field)
        .ok_or_else(|| format!("missing field `{field}` in schedule phase"))?;
    serde_json::from_value(value).map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleCadence {
    pub every_steps: u32,
    pub offset_steps: u32,
}

impl ScheduleCadence {
    pub const fn new(every_steps: u32, offset_steps: u32) -> Self {
        Self {
            every_steps,
            offset_steps,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleSystem {
    pub id: String,
    pub capability: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition: Option<String>,
    pub after: Vec<String>,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
    pub cadence: ScheduleCadence,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameplayDefinition {
    pub id: String,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Timeline {
    pub id: String,
    pub steps: Vec<TimelineStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimelineStep {
    pub id: String,
    pub capability: String,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityBinding {
    pub id: String,
    pub target: String,
}

/// Decodes any valid JSON representation and validates the current schema.
pub fn decode_compiled_composition(bytes: &[u8]) -> Result<CompiledComposition, ProductModelError> {
    if bytes.len() > MAX_COMPILED_COMPOSITION_BYTES {
        return Err(failure(
            "COMPOSITION_BYTES_EXCEEDED",
            SOURCE,
            "$",
            format!("compiled composition is limited to {MAX_COMPILED_COMPOSITION_BYTES} bytes"),
        ));
    }
    reject_duplicate_json_keys(bytes)?;
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let candidate: CompiledCompositionCandidate =
        serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
            let path = error.path().to_string();
            failure(
                "COMPOSITION_DECODE",
                SOURCE,
                if path.is_empty() { "$" } else { &path },
                format!(
                    "invalid current Compiled Composition: {}",
                    error.into_inner()
                ),
            )
        })?;
    deserializer.end().map_err(|error| {
        failure(
            "COMPOSITION_TRAILING_DATA",
            SOURCE,
            "$",
            format!("trailing data after Compiled Composition: {error}"),
        )
    })?;
    validate_compiled_composition(candidate)
}

/// Validates direct values. It performs all checks before constructing the
/// immutable value, so failures cannot publish a partial composition.
pub fn validate_compiled_composition(
    candidate: CompiledCompositionCandidate,
) -> Result<CompiledComposition, ProductModelError> {
    validate_identity(&candidate.product, SOURCE, "product")?;
    validate_bounded_count(
        candidate.input_map.len(),
        MAX_INPUT_MAP_ENTRIES,
        "inputMap",
        "COMPOSITION_INPUT_MAP_COUNT",
    )?;
    validate_bounded_count(
        candidate.intent_descriptors.len(),
        MAX_INTENT_DESCRIPTORS,
        "intentDescriptors",
        "COMPOSITION_INTENT_DESCRIPTOR_COUNT",
    )?;
    validate_bounded_count(
        candidate.schedule.len(),
        SCHEDULE_PHASE_COUNT,
        "schedule",
        "COMPOSITION_SCHEDULE_PHASE_COUNT",
    )?;
    validate_bounded_count(
        candidate.gameplay_definitions.len(),
        MAX_GAMEPLAY_DEFINITIONS,
        "gameplayDefinitions",
        "COMPOSITION_DEFINITION_COUNT",
    )?;
    validate_bounded_count(
        candidate.timelines.len(),
        MAX_TIMELINES,
        "timelines",
        "COMPOSITION_TIMELINE_COUNT",
    )?;
    validate_bounded_count(
        candidate.capability_bindings.len(),
        MAX_CAPABILITY_BINDINGS,
        "capabilityBindings",
        "COMPOSITION_CAPABILITY_COUNT",
    )?;

    let capabilities = validate_capability_bindings(&candidate.capability_bindings)?;
    let mut json_nodes = 0usize;
    let intents = validate_intent_descriptors(
        &candidate.intent_descriptors,
        &capabilities,
        &mut json_nodes,
    )?;
    let definitions =
        validate_gameplay_definitions(&candidate.gameplay_definitions, &mut json_nodes)?;
    validate_input_map(&candidate.input_map, &intents)?;
    validate_schedule(
        &candidate.schedule,
        &capabilities,
        &definitions,
        &mut json_nodes,
    )?;
    validate_timelines(&candidate.timelines, &capabilities, &mut json_nodes)?;

    let canonical_candidate = canonicalize_composition(&candidate);
    let mut canonical_bytes = encode_canonical_composition(&canonical_candidate);
    canonical_bytes.push(b'\n');
    if canonical_bytes.len() > MAX_COMPILED_COMPOSITION_BYTES {
        return Err(failure(
            "COMPOSITION_BYTES_EXCEEDED",
            SOURCE,
            "$",
            format!(
                "canonical compiled composition is limited to {MAX_COMPILED_COMPOSITION_BYTES} bytes"
            ),
        ));
    }
    Ok(CompiledComposition {
        candidate,
        canonical_bytes,
    })
}

/// Returns the exact deterministic bytes retained by a validated composition.
pub fn encode_compiled_composition(composition: &CompiledComposition) -> Vec<u8> {
    composition.canonical_bytes.clone()
}

fn validate_capability_bindings(
    bindings: &[CapabilityBinding],
) -> Result<BTreeSet<String>, ProductModelError> {
    let mut ids = BTreeSet::new();
    for (index, binding) in bindings.iter().enumerate() {
        let prefix = format!("capabilityBindings[{index}]");
        validate_identity(&binding.id, SOURCE, &format!("{prefix}.id"))?;
        validate_capability_target(&binding.target, &format!("{prefix}.target"))?;
        if !ids.insert(binding.id.clone()) {
            return Err(failure(
                "COMPOSITION_DUPLICATE_CAPABILITY",
                SOURCE,
                format!("{prefix}.id"),
                format!("capability `{}` is bound more than once", binding.id),
            ));
        }
    }
    Ok(ids)
}

fn validate_gameplay_definitions(
    definitions: &[GameplayDefinition],
    json_nodes: &mut usize,
) -> Result<BTreeSet<String>, ProductModelError> {
    let mut ids = BTreeSet::new();
    for (index, definition) in definitions.iter().enumerate() {
        let prefix = format!("gameplayDefinitions[{index}]");
        validate_identity(&definition.id, SOURCE, &format!("{prefix}.id"))?;
        validate_opaque_json(
            &definition.payload,
            &format!("{prefix}.payload"),
            json_nodes,
        )?;
        if !ids.insert(definition.id.clone()) {
            return Err(failure(
                "COMPOSITION_DUPLICATE_DEFINITION",
                SOURCE,
                format!("{prefix}.id"),
                format!(
                    "gameplay definition `{}` is declared more than once",
                    definition.id
                ),
            ));
        }
    }
    Ok(ids)
}

fn validate_intent_descriptors(
    descriptors: &[ProductIntentDescriptor],
    capabilities: &BTreeSet<String>,
    json_nodes: &mut usize,
) -> Result<std::collections::BTreeMap<String, IntentValueKind>, ProductModelError> {
    let mut intents = std::collections::BTreeMap::new();
    for (index, descriptor) in descriptors.iter().enumerate() {
        let prefix = format!("intentDescriptors[{index}]");
        validate_identity(&descriptor.id, SOURCE, &format!("{prefix}.id"))?;
        match (descriptor.value_kind, &descriptor.payload_contract) {
            (IntentValueKind::ProductPayload, Some(contract)) => {
                validate_identity(contract, SOURCE, &format!("{prefix}.payloadContract"))?;
            }
            (IntentValueKind::ProductPayload, None) => {
                return Err(failure(
                    "COMPOSITION_PRODUCT_PAYLOAD_CONTRACT_REQUIRED",
                    SOURCE,
                    format!("{prefix}.payloadContract"),
                    "product-payload intents require one stable payloadContract identity",
                ));
            }
            (_, Some(_)) => {
                return Err(failure(
                    "COMPOSITION_PRODUCT_PAYLOAD_CONTRACT_UNEXPECTED",
                    SOURCE,
                    format!("{prefix}.payloadContract"),
                    "payloadContract is valid only for product-payload intents",
                ));
            }
            (_, None) => {}
        }
        if let Some(capability) = &descriptor.capability {
            require_capability(capability, capabilities, &format!("{prefix}.capability"))?;
        }
        validate_opaque_json(
            &descriptor.payload,
            &format!("{prefix}.payload"),
            json_nodes,
        )?;
        if intents
            .insert(descriptor.id.clone(), descriptor.value_kind)
            .is_some()
        {
            return Err(failure(
                "COMPOSITION_DUPLICATE_INTENT_DESCRIPTOR",
                SOURCE,
                format!("{prefix}.id"),
                format!(
                    "intent descriptor `{}` is declared more than once",
                    descriptor.id
                ),
            ));
        }
    }
    Ok(intents)
}

fn validate_input_map(
    entries: &[InputMapEntry],
    intents: &std::collections::BTreeMap<String, IntentValueKind>,
) -> Result<(), ProductModelError> {
    let mut ids = BTreeSet::new();
    for (index, entry) in entries.iter().enumerate() {
        let prefix = format!("inputMap[{index}]");
        validate_identity(&entry.id, SOURCE, &format!("{prefix}.id"))?;
        validate_identity(&entry.intent, SOURCE, &format!("{prefix}.intent"))?;
        let value_kind = intents.get(&entry.intent).ok_or_else(|| {
            failure(
                "COMPOSITION_UNKNOWN_INTENT_DESCRIPTOR",
                SOURCE,
                format!("{prefix}.intent"),
                format!(
                    "input mapping references undeclared intent `{}`",
                    entry.intent
                ),
            )
        })?;
        if *value_kind == IntentValueKind::ProductPayload {
            return Err(failure(
                "COMPOSITION_PHYSICAL_INPUT_PRODUCT_PAYLOAD",
                SOURCE,
                format!("{prefix}.intent"),
                format!(
                    "input mapping cannot target direct-UI-only product-payload intent `{}`",
                    entry.intent
                ),
            ));
        }
        entry.trigger.validate(&format!("{prefix}.trigger"))?;
        if entry.trigger.value_kind() != *value_kind {
            return Err(failure(
                "COMPOSITION_INPUT_TRIGGER_VALUE_KIND",
                SOURCE,
                format!("{prefix}.trigger"),
                format!(
                    "input trigger produces {:?}, but intent `{}` requires {:?}",
                    entry.trigger.value_kind(),
                    entry.intent,
                    value_kind
                ),
            ));
        }
        if !ids.insert(entry.id.clone()) {
            return Err(failure(
                "COMPOSITION_DUPLICATE_INPUT",
                SOURCE,
                format!("{prefix}.id"),
                format!("input entry `{}` is declared more than once", entry.id),
            ));
        }
    }
    Ok(())
}

fn validate_chord_and_context(
    chord: &[KeyboardControl],
    context: Option<&str>,
    path: &str,
) -> Result<(), ProductModelError> {
    if chord.len() > MAX_INPUT_CHORD_CONTROLS {
        return Err(failure(
            "COMPOSITION_INPUT_CHORD_COUNT",
            SOURCE,
            format!("{path}.chord"),
            format!("input chords contain at most {MAX_INPUT_CHORD_CONTROLS} controls"),
        ));
    }
    let mut controls = BTreeSet::new();
    for (index, control) in chord.iter().enumerate() {
        if !controls.insert(*control) {
            return Err(failure(
                "COMPOSITION_DUPLICATE_INPUT_CHORD_CONTROL",
                SOURCE,
                format!("{path}.chord[{index}]"),
                "an input chord cannot repeat a control",
            ));
        }
    }
    validate_context(context, path)
}

fn validate_context(context: Option<&str>, path: &str) -> Result<(), ProductModelError> {
    if let Some(context) = context {
        validate_identity(context, SOURCE, &format!("{path}.context"))?;
    }
    Ok(())
}

fn validate_schedule(
    phases: &[SchedulePhaseDeclaration],
    capabilities: &BTreeSet<String>,
    definitions: &BTreeSet<String>,
    json_nodes: &mut usize,
) -> Result<(), ProductModelError> {
    if phases.len() != SCHEDULE_PHASE_COUNT {
        return Err(failure(
            "COMPOSITION_SCHEDULE_PHASE_COUNT",
            SOURCE,
            "schedule",
            format!(
                "schedule must declare exactly {SCHEDULE_PHASE_COUNT} closed phases in canonical order"
            ),
        ));
    }
    for (index, declaration) in phases.iter().enumerate() {
        let expected = SchedulePhase::ALL[index];
        if declaration.phase != expected {
            return Err(failure(
                "COMPOSITION_SCHEDULE_PHASE_ORDER",
                SOURCE,
                format!("schedule[{index}].phase"),
                format!(
                    "schedule phase `{}` is out of order; expected `{}` at position {index}",
                    declaration.phase.as_str(),
                    expected.as_str(),
                ),
            ));
        }
    }

    let total_systems = phases
        .iter()
        .map(|phase| phase.composition.systems().count())
        .sum();
    validate_bounded_count(
        total_systems,
        MAX_SCHEDULE_ENTRIES,
        "schedule",
        "COMPOSITION_SCHEDULE_COUNT",
    )?;

    let mut ids = BTreeSet::new();
    let mut locations = BTreeMap::new();
    for (phase_index, declaration) in phases.iter().enumerate() {
        for (system_index, (system, placement)) in declaration.composition.systems().enumerate() {
            let prefix = format!(
                "schedule[{phase_index}].{}",
                placement_path(placement, system_index)
            );
            validate_schedule_system(system, &prefix, capabilities, definitions, json_nodes)?;
            if !ids.insert(system.id.clone()) {
                return Err(failure(
                    "COMPOSITION_DUPLICATE_SCHEDULE_ENTRY",
                    SOURCE,
                    format!("{prefix}.id"),
                    format!("schedule system `{}` is declared more than once", system.id),
                ));
            }
            locations.insert(system.id.clone(), (phase_index, placement));
        }
    }

    validate_schedule_dependencies(phases, &locations)?;
    validate_schedule_conflicts(phases, &locations)?;
    Ok(())
}

fn validate_schedule_system(
    system: &ScheduleSystem,
    prefix: &str,
    capabilities: &BTreeSet<String>,
    definitions: &BTreeSet<String>,
    json_nodes: &mut usize,
) -> Result<(), ProductModelError> {
    validate_identity(&system.id, SOURCE, &format!("{prefix}.id"))?;
    require_capability(
        &system.capability,
        capabilities,
        &format!("{prefix}.capability"),
    )?;
    if let Some(definition) = &system.definition {
        validate_identity(definition, SOURCE, &format!("{prefix}.definition"))?;
        if !definitions.contains(definition) {
            return Err(failure(
                "COMPOSITION_UNKNOWN_DEFINITION",
                SOURCE,
                format!("{prefix}.definition"),
                format!("schedule system references unknown gameplay definition `{definition}`"),
            ));
        }
    }
    validate_schedule_accesses(
        &system.after,
        &format!("{prefix}.after"),
        MAX_SCHEDULE_DEPENDENCIES,
        "COMPOSITION_SCHEDULE_DEPENDENCY_COUNT",
        "COMPOSITION_DUPLICATE_SCHEDULE_DEPENDENCY",
    )?;
    validate_schedule_accesses(
        &system.reads,
        &format!("{prefix}.reads"),
        MAX_SCHEDULE_ACCESS_DECLARATIONS,
        "COMPOSITION_SCHEDULE_READ_COUNT",
        "COMPOSITION_DUPLICATE_SCHEDULE_READ",
    )?;
    validate_schedule_accesses(
        &system.writes,
        &format!("{prefix}.writes"),
        MAX_SCHEDULE_ACCESS_DECLARATIONS,
        "COMPOSITION_SCHEDULE_WRITE_COUNT",
        "COMPOSITION_DUPLICATE_SCHEDULE_WRITE",
    )?;
    if system.cadence.every_steps == 0 {
        return Err(failure(
            "COMPOSITION_INVALID_SCHEDULE_CADENCE",
            SOURCE,
            format!("{prefix}.cadence.everySteps"),
            "schedule cadence everySteps must be greater than zero",
        ));
    }
    if system.cadence.offset_steps >= system.cadence.every_steps {
        return Err(failure(
            "COMPOSITION_INVALID_SCHEDULE_CADENCE",
            SOURCE,
            format!("{prefix}.cadence.offsetSteps"),
            "schedule cadence offsetSteps must be less than everySteps",
        ));
    }
    validate_opaque_json(&system.payload, &format!("{prefix}.payload"), json_nodes)?;
    Ok(())
}

fn placement_path(placement: SchedulePlacement, index: usize) -> String {
    match placement {
        SchedulePlacement::Append => format!("systems[{index}]"),
        SchedulePlacement::Prepend => format!("systems[{index}]"),
        SchedulePlacement::ExtendBefore => format!("before[{index}]"),
        SchedulePlacement::ExtendAfter => format!("after[{index}]"),
        SchedulePlacement::Replace => format!("systems[{index}]"),
    }
}

fn validate_schedule_dependencies(
    phases: &[SchedulePhaseDeclaration],
    locations: &BTreeMap<String, (usize, SchedulePlacement)>,
) -> Result<(), ProductModelError> {
    for (phase_index, declaration) in phases.iter().enumerate() {
        for (system_index, (system, placement)) in declaration.composition.systems().enumerate() {
            let prefix = format!(
                "schedule[{phase_index}].{}.after",
                placement_path(placement, system_index)
            );
            for (dependency_index, dependency) in system.after.iter().enumerate() {
                if dependency == &system.id {
                    return Err(failure(
                        "COMPOSITION_SCHEDULE_SELF_DEPENDENCY",
                        SOURCE,
                        format!("{prefix}[{dependency_index}]"),
                        format!("schedule system `{}` cannot depend on itself", system.id),
                    ));
                }
                let Some((dependency_phase, dependency_placement)) = locations.get(dependency)
                else {
                    return Err(failure(
                        "COMPOSITION_UNKNOWN_SCHEDULE_DEPENDENCY",
                        SOURCE,
                        format!("{prefix}[{dependency_index}]"),
                        format!(
                            "schedule system `{}` depends on undeclared system `{dependency}`",
                            system.id
                        ),
                    ));
                };
                if *dependency_phase != phase_index {
                    return Err(failure(
                        "COMPOSITION_SCHEDULE_CROSS_PHASE_DEPENDENCY",
                        SOURCE,
                        format!("{prefix}[{dependency_index}]"),
                        format!(
                            "schedule dependency `{dependency}` must remain within phase `{}`",
                            declaration.phase.as_str()
                        ),
                    ));
                }
                if *dependency_placement != placement {
                    return Err(failure(
                        "COMPOSITION_SCHEDULE_PLACEMENT_DEPENDENCY",
                        SOURCE,
                        format!("{prefix}[{dependency_index}]"),
                        format!("schedule dependency `{dependency}` crosses the `{}` composition placement partition", declaration.phase.as_str()),
                    ));
                }
            }
        }
    }

    for (phase_index, declaration) in phases.iter().enumerate() {
        let systems = declaration.composition.systems().collect::<Vec<_>>();
        let ids = systems
            .iter()
            .map(|(system, _)| system.id.as_str())
            .collect::<BTreeSet<_>>();
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        for id in ids {
            detect_schedule_cycle(id, &systems, &mut visiting, &mut visited, phase_index)?;
        }
    }
    Ok(())
}

fn detect_schedule_cycle(
    id: &str,
    systems: &[(&ScheduleSystem, SchedulePlacement)],
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    phase_index: usize,
) -> Result<(), ProductModelError> {
    if visited.contains(id) {
        return Ok(());
    }
    if !visiting.insert(id.to_owned()) {
        return Err(failure(
            "COMPOSITION_SCHEDULE_DEPENDENCY_CYCLE",
            SOURCE,
            format!("schedule[{phase_index}]"),
            format!("schedule dependency graph contains a cycle involving `{id}`"),
        ));
    }
    let Some((system, _)) = systems.iter().find(|(system, _)| system.id == id) else {
        return Ok(());
    };
    for dependency in &system.after {
        detect_schedule_cycle(dependency, systems, visiting, visited, phase_index)?;
    }
    visiting.remove(id);
    visited.insert(id.to_owned());
    Ok(())
}

fn validate_schedule_conflicts(
    phases: &[SchedulePhaseDeclaration],
    _locations: &BTreeMap<String, (usize, SchedulePlacement)>,
) -> Result<(), ProductModelError> {
    for (phase_index, declaration) in phases.iter().enumerate() {
        let systems = declaration.composition.systems().collect::<Vec<_>>();
        for left_index in 0..systems.len() {
            for right_index in left_index + 1..systems.len() {
                let (left, left_placement) = systems[left_index];
                let (right, right_placement) = systems[right_index];
                if left_placement != right_placement {
                    continue;
                }
                let conflict = left.writes.iter().any(|write| {
                    right.writes.iter().any(|other| other == write)
                        || right.reads.iter().any(|read| read == write)
                }) || right
                    .writes
                    .iter()
                    .any(|write| left.reads.iter().any(|read| read == write));
                if !conflict {
                    continue;
                }
                let ordered = reaches(left.id.as_str(), right.id.as_str(), &systems)
                    || reaches(right.id.as_str(), left.id.as_str(), &systems);
                if !ordered {
                    return Err(failure(
                        "COMPOSITION_SCHEDULE_ACCESS_AMBIGUITY",
                        SOURCE,
                        format!("schedule[{phase_index}]"),
                        format!(
                            "schedule systems `{}` and `{}` have conflicting read/write access without an explicit dependency",
                            left.id, right.id
                        ),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn reaches(before: &str, target: &str, systems: &[(&ScheduleSystem, SchedulePlacement)]) -> bool {
    let mut pending = vec![target];
    let mut seen = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !seen.insert(current) {
            continue;
        }
        let Some((system, _)) = systems.iter().find(|(system, _)| system.id == current) else {
            continue;
        };
        if system.after.iter().any(|dependency| dependency == before) {
            return true;
        }
        pending.extend(system.after.iter().map(String::as_str));
    }
    false
}

/// Validates declarative schedule access names before execution. A name present
/// in both lists is intentionally valid: it denotes read/modify/write behavior
/// within one explicitly ordered system.
fn validate_schedule_accesses(
    declarations: &[String],
    path: &str,
    maximum: usize,
    count_code: &str,
    duplicate_code: &str,
) -> Result<(), ProductModelError> {
    validate_bounded_count(declarations.len(), maximum, path, count_code)?;
    let mut known = BTreeSet::new();
    for (index, declaration) in declarations.iter().enumerate() {
        let declaration_path = format!("{path}[{index}]");
        validate_identity(declaration, SOURCE, &declaration_path)?;
        if !known.insert(declaration) {
            return Err(failure(
                duplicate_code,
                SOURCE,
                declaration_path,
                format!("schedule access `{declaration}` is declared more than once"),
            ));
        }
    }
    Ok(())
}

fn validate_timelines(
    timelines: &[Timeline],
    capabilities: &BTreeSet<String>,
    json_nodes: &mut usize,
) -> Result<(), ProductModelError> {
    let mut timeline_ids = BTreeSet::new();
    for (timeline_index, timeline) in timelines.iter().enumerate() {
        let prefix = format!("timelines[{timeline_index}]");
        validate_identity(&timeline.id, SOURCE, &format!("{prefix}.id"))?;
        if !timeline_ids.insert(timeline.id.clone()) {
            return Err(failure(
                "COMPOSITION_DUPLICATE_TIMELINE",
                SOURCE,
                format!("{prefix}.id"),
                format!("timeline `{}` is declared more than once", timeline.id),
            ));
        }
        validate_bounded_count(
            timeline.steps.len(),
            MAX_TIMELINE_STEPS,
            &format!("{prefix}.steps"),
            "COMPOSITION_TIMELINE_STEP_COUNT",
        )?;
        let mut step_ids = BTreeSet::new();
        for (step_index, step) in timeline.steps.iter().enumerate() {
            let step_prefix = format!("{prefix}.steps[{step_index}]");
            validate_identity(&step.id, SOURCE, &format!("{step_prefix}.id"))?;
            require_capability(
                &step.capability,
                capabilities,
                &format!("{step_prefix}.capability"),
            )?;
            validate_opaque_json(&step.payload, &format!("{step_prefix}.payload"), json_nodes)?;
            if !step_ids.insert(step.id.clone()) {
                return Err(failure(
                    "COMPOSITION_DUPLICATE_TIMELINE_STEP",
                    SOURCE,
                    format!("{step_prefix}.id"),
                    format!("timeline step `{}` is declared more than once", step.id),
                ));
            }
        }
    }
    Ok(())
}

fn require_capability(
    capability: &str,
    known: &BTreeSet<String>,
    path: &str,
) -> Result<(), ProductModelError> {
    validate_identity(capability, SOURCE, path)?;
    if !known.contains(capability) {
        return Err(failure(
            "COMPOSITION_UNKNOWN_CAPABILITY",
            SOURCE,
            path,
            format!("reference to undeclared capability `{capability}`"),
        ));
    }
    Ok(())
}

fn validate_bounded_count(
    actual: usize,
    maximum: usize,
    path: &str,
    code: &str,
) -> Result<(), ProductModelError> {
    if actual > maximum {
        return Err(failure(
            code,
            SOURCE,
            path,
            format!("contains {actual} entries; maximum is {maximum}"),
        ));
    }
    Ok(())
}

fn validate_opaque_json(
    value: &Value,
    path: &str,
    nodes: &mut usize,
) -> Result<(), ProductModelError> {
    visit_json(value, path, 1, nodes)
}

fn visit_json(
    value: &Value,
    path: &str,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), ProductModelError> {
    if depth > MAX_OPAQUE_JSON_DEPTH {
        return Err(failure(
            "COMPOSITION_OPAQUE_JSON_DEPTH",
            SOURCE,
            path,
            format!("opaque payload depth exceeds {MAX_OPAQUE_JSON_DEPTH}"),
        ));
    }
    *nodes = nodes.checked_add(1).ok_or_else(|| {
        failure(
            "COMPOSITION_OPAQUE_JSON_NODE_COUNT",
            SOURCE,
            path,
            "opaque payload node count overflowed",
        )
    })?;
    if *nodes > MAX_OPAQUE_JSON_NODES {
        return Err(failure(
            "COMPOSITION_OPAQUE_JSON_NODE_COUNT",
            SOURCE,
            path,
            format!("opaque payload exceeds {MAX_OPAQUE_JSON_NODES} JSON nodes"),
        ));
    }
    match value {
        Value::Null | Value::Bool(_) => Ok(()),
        Value::Number(number) => validate_json_number(number, path),
        Value::String(value) => validate_json_string(value, path),
        Value::Array(values) => {
            if values.len() > MAX_OPAQUE_JSON_ARRAY_ENTRIES {
                return Err(failure(
                    "COMPOSITION_OPAQUE_JSON_ARRAY_ENTRIES",
                    SOURCE,
                    path,
                    format!("opaque arrays are limited to {MAX_OPAQUE_JSON_ARRAY_ENTRIES} entries"),
                ));
            }
            for (index, child) in values.iter().enumerate() {
                visit_json(child, &format!("{path}[{index}]"), depth + 1, nodes)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            if values.len() > MAX_OPAQUE_JSON_OBJECT_ENTRIES {
                return Err(failure(
                    "COMPOSITION_OPAQUE_JSON_OBJECT_ENTRIES",
                    SOURCE,
                    path,
                    format!(
                        "opaque objects are limited to {MAX_OPAQUE_JSON_OBJECT_ENTRIES} entries"
                    ),
                ));
            }
            for (key, child) in values {
                validate_json_string(key, &format!("{path}.<key>"))?;
                visit_json(child, &format!("{path}.{key}"), depth + 1, nodes)?;
            }
            Ok(())
        }
    }
}

fn validate_json_number(number: &serde_json::Number, path: &str) -> Result<(), ProductModelError> {
    match (number.as_i64(), number.as_u64()) {
        (Some(value), _) if value.unsigned_abs() <= MAX_SAFE_JSON_INTEGER => Ok(()),
        (_, Some(value)) if value <= MAX_SAFE_JSON_INTEGER => Ok(()),
        (Some(_), _) | (_, Some(_)) => Err(failure(
            "COMPOSITION_OPAQUE_JSON_NUMBER",
            SOURCE,
            path,
            "opaque payload integer values must remain within the IEEE-754 safe integer range",
        )),
        (None, None) => {
            let value = number.as_f64().ok_or_else(|| {
                failure(
                    "COMPOSITION_OPAQUE_JSON_NUMBER",
                    SOURCE,
                    path,
                    "opaque payload numbers must be finite JSON numbers",
                )
            })?;
            if !value.is_finite() {
                return Err(failure(
                    "COMPOSITION_OPAQUE_JSON_NUMBER",
                    SOURCE,
                    path,
                    "opaque payload numbers must be finite JSON numbers",
                ));
            }
            if value.fract() == 0.0 && value.abs() > MAX_SAFE_JSON_INTEGER as f64 {
                return Err(failure(
                    "COMPOSITION_OPAQUE_JSON_NUMBER",
                    SOURCE,
                    path,
                    "opaque payload integer values must remain within the IEEE-754 safe integer range",
                ));
            }
            Ok(())
        }
    }
}

fn validate_capability_target(value: &str, path: &str) -> Result<(), ProductModelError> {
    let (namespace, local) = value.split_once('.').ok_or_else(|| {
        failure(
            "COMPOSITION_CAPABILITY_TARGET_NAMESPACE",
            SOURCE,
            path,
            "capability targets must use the `engine.<id>` or `kernel.<id>` namespace",
        )
    })?;
    if !matches!(namespace, "engine" | "kernel") {
        return Err(failure(
            "COMPOSITION_CAPABILITY_TARGET_NAMESPACE",
            SOURCE,
            path,
            "capability targets must use the `engine.<id>` or `kernel.<id>` namespace",
        ));
    }
    validate_identity(local, SOURCE, path)
}

fn validate_json_string(value: &str, path: &str) -> Result<(), ProductModelError> {
    if value.len() > MAX_OPAQUE_JSON_STRING_BYTES {
        return Err(failure(
            "COMPOSITION_OPAQUE_JSON_STRING_BYTES",
            SOURCE,
            path,
            format!(
                "opaque JSON strings are limited to {MAX_OPAQUE_JSON_STRING_BYTES} UTF-8 bytes"
            ),
        ));
    }
    Ok(())
}

fn canonicalize_composition(
    candidate: &CompiledCompositionCandidate,
) -> CompiledCompositionCandidate {
    let mut canonical = candidate.clone();
    for descriptor in &mut canonical.intent_descriptors {
        descriptor.payload = canonicalize_json_value(&descriptor.payload);
    }
    for phase in &mut canonical.schedule {
        for (system, _) in phase.composition.systems_mut() {
            system.payload = canonicalize_json_value(&system.payload);
        }
    }
    for definition in &mut canonical.gameplay_definitions {
        definition.payload = canonicalize_json_value(&definition.payload);
    }
    for timeline in &mut canonical.timelines {
        for step in &mut timeline.steps {
            step.payload = canonicalize_json_value(&step.payload);
        }
    }
    canonical
}

/// Sorts only object keys recursively. Array order remains authored and is
/// intentionally never reordered.
fn canonicalize_json_value(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(canonicalize_json_value).collect()),
        Value::Object(values) => {
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
            let mut canonical = serde_json::Map::new();
            for key in keys {
                canonical.insert(key.clone(), canonicalize_json_value(&values[key]));
            }
            Value::Object(canonical)
        }
        _ => value.clone(),
    }
}

/// Writes the exact Compiled Composition canonical bytes. Typed object fields
/// retain their declared current-schema order; opaque object keys sort by raw
/// UTF-8 bytes. Numbers use ECMAScript Number::toString through `ryu-js`, with
/// negative zero normalized to `0`, so TypeScript and Rust have one explicit
/// cross-language policy rather than serializer-dependent output.
fn encode_canonical_composition(candidate: &CompiledCompositionCandidate) -> Vec<u8> {
    let mut output = Vec::new();
    output.push(b'{');
    let mut first = true;
    write_field_name(&mut output, &mut first, "product");
    write_json_string(&mut output, &candidate.product);
    write_field_name(&mut output, &mut first, "intentDescriptors");
    output.push(b'[');
    for (index, descriptor) in candidate.intent_descriptors.iter().enumerate() {
        if index != 0 {
            output.push(b',');
        }
        output.push(b'{');
        let mut descriptor_first = true;
        write_field_name(&mut output, &mut descriptor_first, "id");
        write_json_string(&mut output, &descriptor.id);
        write_field_name(&mut output, &mut descriptor_first, "valueKind");
        write_json_string(
            &mut output,
            match descriptor.value_kind {
                IntentValueKind::Digital => "digital",
                IntentValueKind::Axis => "axis",
                IntentValueKind::ProductPayload => "product-payload",
            },
        );
        if let Some(contract) = &descriptor.payload_contract {
            write_field_name(&mut output, &mut descriptor_first, "payloadContract");
            write_json_string(&mut output, contract);
        }
        if let Some(capability) = &descriptor.capability {
            write_field_name(&mut output, &mut descriptor_first, "capability");
            write_json_string(&mut output, capability);
        }
        write_field_name(&mut output, &mut descriptor_first, "payload");
        write_canonical_json(&mut output, &descriptor.payload);
        output.push(b'}');
    }
    output.push(b']');
    write_field_name(&mut output, &mut first, "inputMap");
    output.push(b'[');
    for (index, entry) in candidate.input_map.iter().enumerate() {
        if index != 0 {
            output.push(b',');
        }
        output.push(b'{');
        let mut entry_first = true;
        write_field_name(&mut output, &mut entry_first, "id");
        write_json_string(&mut output, &entry.id);
        write_field_name(&mut output, &mut entry_first, "intent");
        write_json_string(&mut output, &entry.intent);
        write_field_name(&mut output, &mut entry_first, "trigger");
        write_input_trigger(&mut output, &entry.trigger);
        output.push(b'}');
    }
    output.push(b']');
    write_field_name(&mut output, &mut first, "schedule");
    output.push(b'[');
    for (index, phase) in candidate.schedule.iter().enumerate() {
        if index != 0 {
            output.push(b',');
        }
        output.push(b'{');
        let mut phase_first = true;
        write_field_name(&mut output, &mut phase_first, "phase");
        write_json_string(&mut output, phase.phase.as_str());
        match &phase.composition {
            ScheduleComposition::Append { systems } => {
                write_field_name(&mut output, &mut phase_first, "mode");
                write_json_string(&mut output, "append");
                write_field_name(&mut output, &mut phase_first, "systems");
                write_schedule_system_array(&mut output, systems);
            }
            ScheduleComposition::Prepend { systems } => {
                write_field_name(&mut output, &mut phase_first, "mode");
                write_json_string(&mut output, "prepend");
                write_field_name(&mut output, &mut phase_first, "systems");
                write_schedule_system_array(&mut output, systems);
            }
            ScheduleComposition::Extend { before, after } => {
                write_field_name(&mut output, &mut phase_first, "mode");
                write_json_string(&mut output, "extend");
                write_field_name(&mut output, &mut phase_first, "before");
                write_schedule_system_array(&mut output, before);
                write_field_name(&mut output, &mut phase_first, "after");
                write_schedule_system_array(&mut output, after);
            }
            ScheduleComposition::Replace { systems } => {
                write_field_name(&mut output, &mut phase_first, "mode");
                write_json_string(&mut output, "replace");
                write_field_name(&mut output, &mut phase_first, "systems");
                write_schedule_system_array(&mut output, systems);
            }
        }
        output.push(b'}');
    }
    output.push(b']');
    write_field_name(&mut output, &mut first, "gameplayDefinitions");
    output.push(b'[');
    for (index, definition) in candidate.gameplay_definitions.iter().enumerate() {
        if index != 0 {
            output.push(b',');
        }
        output.push(b'{');
        let mut definition_first = true;
        write_field_name(&mut output, &mut definition_first, "id");
        write_json_string(&mut output, &definition.id);
        write_field_name(&mut output, &mut definition_first, "payload");
        write_canonical_json(&mut output, &definition.payload);
        output.push(b'}');
    }
    output.push(b']');
    write_field_name(&mut output, &mut first, "timelines");
    output.push(b'[');
    for (timeline_index, timeline) in candidate.timelines.iter().enumerate() {
        if timeline_index != 0 {
            output.push(b',');
        }
        output.push(b'{');
        let mut timeline_first = true;
        write_field_name(&mut output, &mut timeline_first, "id");
        write_json_string(&mut output, &timeline.id);
        write_field_name(&mut output, &mut timeline_first, "steps");
        output.push(b'[');
        for (step_index, step) in timeline.steps.iter().enumerate() {
            if step_index != 0 {
                output.push(b',');
            }
            output.push(b'{');
            let mut step_first = true;
            write_field_name(&mut output, &mut step_first, "id");
            write_json_string(&mut output, &step.id);
            write_field_name(&mut output, &mut step_first, "capability");
            write_json_string(&mut output, &step.capability);
            write_field_name(&mut output, &mut step_first, "payload");
            write_canonical_json(&mut output, &step.payload);
            output.push(b'}');
        }
        output.push(b']');
        output.push(b'}');
    }
    output.push(b']');
    write_field_name(&mut output, &mut first, "capabilityBindings");
    output.push(b'[');
    for (index, binding) in candidate.capability_bindings.iter().enumerate() {
        if index != 0 {
            output.push(b',');
        }
        output.push(b'{');
        let mut binding_first = true;
        write_field_name(&mut output, &mut binding_first, "id");
        write_json_string(&mut output, &binding.id);
        write_field_name(&mut output, &mut binding_first, "target");
        write_json_string(&mut output, &binding.target);
        output.push(b'}');
    }
    output.push(b']');
    output.push(b'}');
    output
}

fn write_input_trigger(output: &mut Vec<u8>, trigger: &InputTrigger) {
    let value = serde_json::to_value(trigger)
        .expect("the closed input trigger schema is always serializable");
    write_canonical_json(output, &value);
}

fn write_schedule_system_array(output: &mut Vec<u8>, systems: &[ScheduleSystem]) {
    output.push(b'[');
    for (index, system) in systems.iter().enumerate() {
        if index != 0 {
            output.push(b',');
        }
        write_schedule_system(output, system);
    }
    output.push(b']');
}

fn write_schedule_system(output: &mut Vec<u8>, system: &ScheduleSystem) {
    output.push(b'{');
    let mut first = true;
    write_field_name(output, &mut first, "id");
    write_json_string(output, &system.id);
    write_field_name(output, &mut first, "capability");
    write_json_string(output, &system.capability);
    if let Some(definition) = &system.definition {
        write_field_name(output, &mut first, "definition");
        write_json_string(output, definition);
    }
    write_field_name(output, &mut first, "after");
    write_json_string_array(output, &system.after);
    write_field_name(output, &mut first, "reads");
    write_json_string_array(output, &system.reads);
    write_field_name(output, &mut first, "writes");
    write_json_string_array(output, &system.writes);
    write_field_name(output, &mut first, "cadence");
    output.push(b'{');
    let mut cadence_first = true;
    write_field_name(output, &mut cadence_first, "everySteps");
    output.extend_from_slice(system.cadence.every_steps.to_string().as_bytes());
    write_field_name(output, &mut cadence_first, "offsetSteps");
    output.extend_from_slice(system.cadence.offset_steps.to_string().as_bytes());
    output.push(b'}');
    write_field_name(output, &mut first, "payload");
    write_canonical_json(output, &system.payload);
    output.push(b'}');
}

fn write_field_name(output: &mut Vec<u8>, first: &mut bool, name: &str) {
    if !*first {
        output.push(b',');
    }
    *first = false;
    write_json_string(output, name);
    output.push(b':');
}

fn write_json_string(output: &mut Vec<u8>, value: &str) {
    serde_json::to_writer(output, value).expect("writing JSON to an in-memory buffer cannot fail");
}

fn write_json_string_array(output: &mut Vec<u8>, values: &[String]) {
    output.push(b'[');
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            output.push(b',');
        }
        write_json_string(output, value);
    }
    output.push(b']');
}

fn write_canonical_json(output: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(value) => output.extend_from_slice(if *value { b"true" } else { b"false" }),
        Value::Number(value) => write_canonical_number(output, value),
        Value::String(value) => write_json_string(output, value),
        Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                write_canonical_json(output, value);
            }
            output.push(b']');
        }
        Value::Object(values) => {
            output.push(b'{');
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
            for (index, key) in keys.into_iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                write_json_string(output, key);
                output.push(b':');
                write_canonical_json(output, &values[key]);
            }
            output.push(b'}');
        }
    }
}

fn write_canonical_number(output: &mut Vec<u8>, value: &serde_json::Number) {
    if let Some(value) = value.as_i64() {
        output.extend_from_slice(value.to_string().as_bytes());
        return;
    }
    if let Some(value) = value.as_u64() {
        output.extend_from_slice(value.to_string().as_bytes());
        return;
    }
    let value = value
        .as_f64()
        .expect("validated JSON numbers are finite binary64 values");
    if value == 0.0 {
        output.push(b'0');
        return;
    }
    let mut buffer = ryu_js::Buffer::new();
    output.extend_from_slice(buffer.format(value).as_bytes());
}

/// Parses the raw JSON once before typed decoding so opaque payload objects
/// cannot silently normalize duplicate keys with last-wins semantics.
fn reject_duplicate_json_keys(bytes: &[u8]) -> Result<(), ProductModelError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    DuplicateKeyCheck::deserialize(&mut deserializer).map_err(|error| {
        let message = error.to_string();
        let code = if message.starts_with("duplicate JSON key ") {
            "COMPOSITION_DUPLICATE_JSON_KEY"
        } else {
            "COMPOSITION_DECODE"
        };
        failure(code, SOURCE, "$", format!("invalid JSON: {message}"))
    })?;
    deserializer.end().map_err(|error| {
        failure(
            "COMPOSITION_TRAILING_DATA",
            SOURCE,
            "$",
            format!("trailing data after Compiled Composition: {error}"),
        )
    })?;
    Ok(())
}

struct DuplicateKeyCheck;

impl<'de> Deserialize<'de> for DuplicateKeyCheck {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(DuplicateKeyVisitor)
    }
}

struct DuplicateKeyVisitor;

impl<'de> Visitor<'de> for DuplicateKeyVisitor {
    type Value = DuplicateKeyCheck;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(DuplicateKeyCheck)
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(DuplicateKeyCheck)
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(DuplicateKeyCheck)
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(DuplicateKeyCheck)
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(DuplicateKeyCheck)
    }

    fn visit_borrowed_str<E>(self, _value: &'de str) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(DuplicateKeyCheck)
    }

    fn visit_string<E>(self, _value: String) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(DuplicateKeyCheck)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(DuplicateKeyCheck)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(DuplicateKeyCheck)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element::<DuplicateKeyCheck>()?.is_some() {}
        Ok(DuplicateKeyCheck)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(A::Error::custom(format!("duplicate JSON key `{key}`")));
            }
            map.next_value::<DuplicateKeyCheck>()?;
        }
        Ok(DuplicateKeyCheck)
    }
}
