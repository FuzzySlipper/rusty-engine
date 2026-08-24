use std::collections::BTreeSet;

use serde::{
    de::{Error as DeError, MapAccess, SeqAccess, Visitor},
    Deserialize, Serialize,
};
use serde_json::Value;

use crate::{diagnostic::failure, manifest::validate_identity, ProductModelError};

const SOURCE: &str = "compiled-composition.json";
pub const MAX_COMPILED_COMPOSITION_BYTES: usize = 1_048_576;
pub const MAX_INPUT_MAP_ENTRIES: usize = 256;
pub const MAX_SCHEDULE_ENTRIES: usize = 512;
pub const MAX_SCHEDULE_ACCESS_DECLARATIONS: usize = 64;
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
    pub input_map: Vec<InputMapEntry>,
    pub schedule: Vec<ScheduleEntry>,
    pub gameplay_definitions: Vec<GameplayDefinition>,
    pub timelines: Vec<Timeline>,
    pub capability_bindings: Vec<CapabilityBinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputMapEntry {
    pub id: String,
    pub intent: String,
    pub capability: String,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleEntry {
    pub id: String,
    pub phase: String,
    pub capability: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition: Option<String>,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
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
        candidate.schedule.len(),
        MAX_SCHEDULE_ENTRIES,
        "schedule",
        "COMPOSITION_SCHEDULE_COUNT",
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
    let definitions =
        validate_gameplay_definitions(&candidate.gameplay_definitions, &mut json_nodes)?;
    validate_input_map(&candidate.input_map, &capabilities, &mut json_nodes)?;
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

fn validate_input_map(
    entries: &[InputMapEntry],
    capabilities: &BTreeSet<String>,
    json_nodes: &mut usize,
) -> Result<(), ProductModelError> {
    let mut ids = BTreeSet::new();
    for (index, entry) in entries.iter().enumerate() {
        let prefix = format!("inputMap[{index}]");
        validate_identity(&entry.id, SOURCE, &format!("{prefix}.id"))?;
        validate_identity(&entry.intent, SOURCE, &format!("{prefix}.intent"))?;
        require_capability(
            &entry.capability,
            capabilities,
            &format!("{prefix}.capability"),
        )?;
        validate_opaque_json(&entry.payload, &format!("{prefix}.payload"), json_nodes)?;
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

fn validate_schedule(
    entries: &[ScheduleEntry],
    capabilities: &BTreeSet<String>,
    definitions: &BTreeSet<String>,
    json_nodes: &mut usize,
) -> Result<(), ProductModelError> {
    let mut ids = BTreeSet::new();
    for (index, entry) in entries.iter().enumerate() {
        let prefix = format!("schedule[{index}]");
        validate_identity(&entry.id, SOURCE, &format!("{prefix}.id"))?;
        validate_identity(&entry.phase, SOURCE, &format!("{prefix}.phase"))?;
        require_capability(
            &entry.capability,
            capabilities,
            &format!("{prefix}.capability"),
        )?;
        if let Some(definition) = &entry.definition {
            validate_identity(definition, SOURCE, &format!("{prefix}.definition"))?;
            if !definitions.contains(definition) {
                return Err(failure(
                    "COMPOSITION_UNKNOWN_DEFINITION",
                    SOURCE,
                    format!("{prefix}.definition"),
                    format!("schedule entry references unknown gameplay definition `{definition}`"),
                ));
            }
        }
        validate_schedule_accesses(
            &entry.reads,
            &format!("{prefix}.reads"),
            "COMPOSITION_SCHEDULE_READ_COUNT",
            "COMPOSITION_DUPLICATE_SCHEDULE_READ",
        )?;
        validate_schedule_accesses(
            &entry.writes,
            &format!("{prefix}.writes"),
            "COMPOSITION_SCHEDULE_WRITE_COUNT",
            "COMPOSITION_DUPLICATE_SCHEDULE_WRITE",
        )?;
        validate_opaque_json(&entry.payload, &format!("{prefix}.payload"), json_nodes)?;
        if !ids.insert(entry.id.clone()) {
            return Err(failure(
                "COMPOSITION_DUPLICATE_SCHEDULE_ENTRY",
                SOURCE,
                format!("{prefix}.id"),
                format!("schedule entry `{}` is declared more than once", entry.id),
            ));
        }
    }
    Ok(())
}

/// Validates declarative schedule access names without assigning any execution
/// or conflict semantics. A name present in both lists is intentionally valid:
/// read/modify/write behavior is a later scheduler concern.
fn validate_schedule_accesses(
    declarations: &[String],
    path: &str,
    count_code: &str,
    duplicate_code: &str,
) -> Result<(), ProductModelError> {
    validate_bounded_count(
        declarations.len(),
        MAX_SCHEDULE_ACCESS_DECLARATIONS,
        path,
        count_code,
    )?;
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
    for entry in &mut canonical.input_map {
        entry.payload = canonicalize_json_value(&entry.payload);
    }
    for entry in &mut canonical.schedule {
        entry.payload = canonicalize_json_value(&entry.payload);
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
        write_field_name(&mut output, &mut entry_first, "capability");
        write_json_string(&mut output, &entry.capability);
        write_field_name(&mut output, &mut entry_first, "payload");
        write_canonical_json(&mut output, &entry.payload);
        output.push(b'}');
    }
    output.push(b']');
    write_field_name(&mut output, &mut first, "schedule");
    output.push(b'[');
    for (index, entry) in candidate.schedule.iter().enumerate() {
        if index != 0 {
            output.push(b',');
        }
        output.push(b'{');
        let mut entry_first = true;
        write_field_name(&mut output, &mut entry_first, "id");
        write_json_string(&mut output, &entry.id);
        write_field_name(&mut output, &mut entry_first, "phase");
        write_json_string(&mut output, &entry.phase);
        write_field_name(&mut output, &mut entry_first, "capability");
        write_json_string(&mut output, &entry.capability);
        if let Some(definition) = &entry.definition {
            write_field_name(&mut output, &mut entry_first, "definition");
            write_json_string(&mut output, definition);
        }
        write_field_name(&mut output, &mut entry_first, "reads");
        write_json_string_array(&mut output, &entry.reads);
        write_field_name(&mut output, &mut entry_first, "writes");
        write_json_string_array(&mut output, &entry.writes);
        write_field_name(&mut output, &mut entry_first, "payload");
        write_canonical_json(&mut output, &entry.payload);
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
