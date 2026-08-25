//! A bounded, disposable JavaScript execution lane for product runtime code.
//!
//! The Engine owns the revisioned state root and commits it only after a turn
//! and projection both decode within their configured bounds. The bundled
//! runtime program is evaluated in a fresh QuickJS realm for every ABI call,
//! so runtime-local globals and closures cannot retain authoritative state.

#![forbid(unsafe_code)]

use std::{
    collections::BTreeSet,
    fmt,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use rquickjs::{context::intrinsic, Context, Runtime, Value as JsValue};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// The one global property that a bundled runtime program may install.
pub const RUNTIME_EXPORT_NAME: &str = "__rustyEngineRuntime";

/// Maximum depth accepted for product-state and projection values by default.
pub const DEFAULT_MAX_JSON_DEPTH: usize = 32;
/// Maximum JSON nodes accepted for product-state and projection values by default.
pub const DEFAULT_MAX_JSON_NODES: usize = 16_384;
/// Maximum UTF-8 bytes in one JSON string by default.
pub const DEFAULT_MAX_JSON_STRING_BYTES: usize = 64 * 1024;
/// Maximum bundled runtime program size by default.
pub const DEFAULT_MAX_PROGRAM_BYTES: usize = 256 * 1024;
/// Maximum serialized product state by default.
pub const DEFAULT_MAX_STATE_BYTES: usize = 256 * 1024;
/// Maximum serialized projection by default.
pub const DEFAULT_MAX_PROJECTION_BYTES: usize = 256 * 1024;
/// Maximum serialized initialization facts by default.
///
/// Initialization facts are admitted product inputs, not persistent product
/// state. They may include a compiled composition and a large static navgrid.
pub const DEFAULT_MAX_INITIALIZE_FACTS_BYTES: usize = 4 * 1024 * 1024;
/// Maximum JSON depth accepted for one initialization facts value by default.
pub const DEFAULT_MAX_INITIALIZE_FACTS_JSON_DEPTH: usize = 64;
/// Maximum JSON nodes accepted for one initialization facts value by default.
pub const DEFAULT_MAX_INITIALIZE_FACTS_JSON_NODES: usize = 512 * 1024;
/// Maximum UTF-8 bytes in one initialization facts JSON string by default.
pub const DEFAULT_MAX_INITIALIZE_FACTS_JSON_STRING_BYTES: usize = 4 * 1024 * 1024;
/// Maximum QuickJS heap allocation per disposable realm by default.
pub const DEFAULT_MAX_VM_MEMORY_BYTES: usize = 32 * 1024 * 1024;
/// Maximum QuickJS stack allocation per disposable realm by default.
pub const DEFAULT_MAX_VM_STACK_BYTES: usize = 512 * 1024;
/// Maximum interrupt polls allowed while one ABI call executes by default.
pub const DEFAULT_MAX_INTERRUPT_POLLS: usize = 100_000;

/// Bounds applied to every disposable VM realm and JSON edge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeVmConfig {
    pub max_program_bytes: usize,
    pub max_state_bytes: usize,
    pub max_projection_bytes: usize,
    pub max_initialize_facts_bytes: usize,
    pub max_initialize_facts_json_depth: usize,
    pub max_initialize_facts_json_nodes: usize,
    pub max_initialize_facts_json_string_bytes: usize,
    pub max_json_depth: usize,
    pub max_json_nodes: usize,
    pub max_json_string_bytes: usize,
    pub max_vm_memory_bytes: usize,
    pub max_vm_stack_bytes: usize,
    pub max_interrupt_polls: usize,
}

impl Default for RuntimeVmConfig {
    fn default() -> Self {
        Self {
            max_program_bytes: DEFAULT_MAX_PROGRAM_BYTES,
            max_state_bytes: DEFAULT_MAX_STATE_BYTES,
            max_projection_bytes: DEFAULT_MAX_PROJECTION_BYTES,
            max_initialize_facts_bytes: DEFAULT_MAX_INITIALIZE_FACTS_BYTES,
            max_initialize_facts_json_depth: DEFAULT_MAX_INITIALIZE_FACTS_JSON_DEPTH,
            max_initialize_facts_json_nodes: DEFAULT_MAX_INITIALIZE_FACTS_JSON_NODES,
            max_initialize_facts_json_string_bytes: DEFAULT_MAX_INITIALIZE_FACTS_JSON_STRING_BYTES,
            max_json_depth: DEFAULT_MAX_JSON_DEPTH,
            max_json_nodes: DEFAULT_MAX_JSON_NODES,
            max_json_string_bytes: DEFAULT_MAX_JSON_STRING_BYTES,
            max_vm_memory_bytes: DEFAULT_MAX_VM_MEMORY_BYTES,
            max_vm_stack_bytes: DEFAULT_MAX_VM_STACK_BYTES,
            max_interrupt_polls: DEFAULT_MAX_INTERRUPT_POLLS,
        }
    }
}

/// SHA-256 of the canonical JSON bytes of an Engine-owned state root.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProductStateFingerprint([u8; 32]);

impl ProductStateFingerprint {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ProductStateFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// A read-only snapshot of the Engine-owned product-state root.
#[derive(Clone, Debug, PartialEq)]
pub struct ProductStateReadout {
    revision: u64,
    fingerprint: ProductStateFingerprint,
    state: Value,
}

impl ProductStateReadout {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn fingerprint(&self) -> ProductStateFingerprint {
        self.fingerprint
    }

    pub fn state(&self) -> &Value {
        &self.state
    }
}

/// Successful initialize or turn receipt.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeVmReceipt {
    state: ProductStateReadout,
    projection: RuntimeVmProjection,
}

impl RuntimeVmReceipt {
    pub fn state(&self) -> &ProductStateReadout {
        &self.state
    }

    pub fn projection(&self) -> &RuntimeVmProjection {
        &self.projection
    }
}

/// One fully validated VM candidate that has not yet replaced the state root.
///
/// Product runtime owners use this when another named Engine owner must admit
/// the candidate's projections before the product state is published.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeVmCandidate {
    base_revision: Option<u64>,
    receipt: RuntimeVmReceipt,
}

impl RuntimeVmCandidate {
    pub fn receipt(&self) -> &RuntimeVmReceipt {
        &self.receipt
    }
}

/// The fixed renderer-neutral observation returned by a runtime projection.
///
/// `ui` remains product-owned observation data. `render`, when present, is an
/// opaque bounded value for the named Engine render projection owner to decode.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeVmProjection {
    ui: Value,
    render: Option<Value>,
}

impl RuntimeVmProjection {
    pub fn ui(&self) -> &Value {
        &self.ui
    }

    pub fn render(&self) -> Option<&Value> {
        self.render.as_ref()
    }
}

/// The fixed DTO supplied to the runtime's `turn` export.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeTurn {
    pub step: Value,
    pub input: Value,
}

/// Errors at the narrow VM and JSON boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeVmError {
    ProgramTooLarge {
        actual: usize,
        maximum: usize,
    },
    InvalidConfiguration(&'static str),
    NotInitialized,
    RevisionExhausted,
    Vm {
        stage: &'static str,
        message: String,
    },
    ExportContract(String),
    JsonTooLarge {
        stage: &'static str,
        actual: usize,
        maximum: usize,
    },
    JsonTooDeep {
        stage: &'static str,
        maximum: usize,
    },
    TooManyJsonNodes {
        stage: &'static str,
        maximum: usize,
    },
    JsonStringTooLarge {
        stage: &'static str,
        maximum: usize,
    },
    MissingJsonResult {
        stage: &'static str,
    },
    InvalidJson {
        stage: &'static str,
        message: String,
    },
}

impl fmt::Display for RuntimeVmError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProgramTooLarge { actual, maximum } => {
                write!(
                    formatter,
                    "runtime program is {actual} bytes; limit is {maximum}"
                )
            }
            Self::InvalidConfiguration(reason) => {
                write!(formatter, "invalid runtime VM configuration: {reason}")
            }
            Self::NotInitialized => write!(formatter, "runtime VM has not been initialized"),
            Self::RevisionExhausted => write!(formatter, "product-state revision is exhausted"),
            Self::Vm { stage, message } => write!(formatter, "VM {stage} failed: {message}"),
            Self::ExportContract(message) => write!(formatter, "invalid runtime export: {message}"),
            Self::JsonTooLarge {
                stage,
                actual,
                maximum,
            } => {
                write!(
                    formatter,
                    "{stage} JSON is {actual} bytes; limit is {maximum}"
                )
            }
            Self::JsonTooDeep { stage, maximum } => {
                write!(formatter, "{stage} JSON exceeds depth limit {maximum}")
            }
            Self::TooManyJsonNodes { stage, maximum } => {
                write!(formatter, "{stage} JSON exceeds node limit {maximum}")
            }
            Self::JsonStringTooLarge { stage, maximum } => {
                write!(
                    formatter,
                    "{stage} JSON has a string exceeding {maximum} bytes"
                )
            }
            Self::MissingJsonResult { stage } => {
                write!(formatter, "VM {stage} returned no JSON value")
            }
            Self::InvalidJson { stage, message } => {
                write!(formatter, "VM {stage} returned invalid JSON: {message}")
            }
        }
    }
}

impl std::error::Error for RuntimeVmError {}

/// A thin Engine owner for one bundled runtime program and one state root.
///
/// The program must install exactly this fixed export:
///
/// ```text
/// globalThis.__rustyEngineRuntime = {
///   initialize({ facts }) => state,
///   turn({ state, step, input }) => nextState,
///   project({ state }) => ({ ui, render? }),
/// };
/// ```
pub struct RuntimeVm {
    program: String,
    config: RuntimeVmConfig,
    root: Option<ProductStateReadout>,
}

impl RuntimeVm {
    pub fn new(
        program: impl Into<String>,
        config: RuntimeVmConfig,
    ) -> Result<Self, RuntimeVmError> {
        validate_config(&config)?;
        let program = program.into();
        if program.len() > config.max_program_bytes {
            return Err(RuntimeVmError::ProgramTooLarge {
                actual: program.len(),
                maximum: config.max_program_bytes,
            });
        }

        Ok(Self {
            program,
            config,
            root: None,
        })
    }

    pub fn state(&self) -> Option<&ProductStateReadout> {
        self.root.as_ref()
    }

    /// Initializes and publishes a first state/projection pair atomically.
    pub fn initialize(&mut self, facts: Value) -> Result<RuntimeVmReceipt, RuntimeVmError> {
        let candidate = self.prepare_initialize(facts)?;
        self.commit_prepared(candidate)
    }

    /// Produces the first validated state/projection pair without publishing it.
    pub fn prepare_initialize(&self, facts: Value) -> Result<RuntimeVmCandidate, RuntimeVmError> {
        if self.root.is_some() {
            return Err(RuntimeVmError::ExportContract(
                "initialize may only be called once for a VM instance".into(),
            ));
        }
        let facts = canonicalize_initialize_facts(facts, &self.config)?;
        let candidate = self.call(
            AbiMethod::Initialize,
            serde_json::json!({ "facts": facts }),
            initialize_input_bounds(&self.config),
            self.config.max_state_bytes,
        )?;
        self.prepare_candidate(candidate)
    }

    /// Runs one fixed Engine-admitted turn and publishes its projection atomically.
    pub fn turn(&mut self, turn: RuntimeTurn) -> Result<RuntimeVmReceipt, RuntimeVmError> {
        let candidate = self.prepare_turn(turn)?;
        self.commit_prepared(candidate)
    }

    /// Runs one fixed Engine-admitted turn without publishing its candidate.
    pub fn prepare_turn(&self, turn: RuntimeTurn) -> Result<RuntimeVmCandidate, RuntimeVmError> {
        let current = self.root.clone().ok_or(RuntimeVmError::NotInitialized)?;
        validate_json(
            "turn step",
            &turn.step,
            &self.config,
            self.config.max_state_bytes,
        )?;
        validate_json(
            "turn input",
            &turn.input,
            &self.config,
            self.config.max_state_bytes,
        )?;
        let candidate = self.call(
            AbiMethod::Turn,
            serde_json::json!({
                "state": current.state,
                "step": turn.step,
                "input": turn.input,
            }),
            ordinary_input_bounds(&self.config, self.config.max_state_bytes),
            self.config.max_state_bytes,
        )?;
        self.prepare_candidate(candidate)
    }

    /// Publishes a candidate produced by this VM at its current state revision.
    pub fn commit_prepared(
        &mut self,
        candidate: RuntimeVmCandidate,
    ) -> Result<RuntimeVmReceipt, RuntimeVmError> {
        let current_revision = self.root.as_ref().map(ProductStateReadout::revision);
        if current_revision != candidate.base_revision {
            return Err(RuntimeVmError::ExportContract(
                "prepared candidate no longer matches the current state root".into(),
            ));
        }
        self.root = Some(candidate.receipt.state.clone());
        Ok(candidate.receipt)
    }

    fn prepare_candidate(&self, candidate: Value) -> Result<RuntimeVmCandidate, RuntimeVmError> {
        validate_json(
            "next state",
            &candidate,
            &self.config,
            self.config.max_state_bytes,
        )?;
        let projection = self.call(
            AbiMethod::Project,
            serde_json::json!({ "state": candidate }),
            ordinary_input_bounds(&self.config, self.config.max_state_bytes),
            self.config.max_projection_bytes,
        )?;
        let projection = decode_projection(projection, &self.config)?;

        let revision = match self.root.as_ref() {
            Some(current) => current
                .revision
                .checked_add(1)
                .ok_or(RuntimeVmError::RevisionExhausted)?,
            None => 1,
        };
        let state = canonicalize_json(
            "next state",
            candidate,
            &self.config,
            self.config.max_state_bytes,
        )?;
        let root = ProductStateReadout {
            revision,
            fingerprint: fingerprint(&state),
            state,
        };
        Ok(RuntimeVmCandidate {
            base_revision: self.root.as_ref().map(ProductStateReadout::revision),
            receipt: RuntimeVmReceipt {
                state: root,
                projection,
            },
        })
    }

    fn call(
        &self,
        method: AbiMethod,
        input: Value,
        input_bounds: JsonValidationBounds,
        maximum_result_bytes: usize,
    ) -> Result<Value, RuntimeVmError> {
        let input = canonicalize_json_with_bounds("runtime input", input, input_bounds)?;
        let input_json = serde_json::to_string(&input).expect("JSON value must serialize");
        let counter = Arc::new(AtomicUsize::new(0));
        let runtime = Runtime::new().map_err(|error| vm_error("runtime creation", error))?;
        runtime.set_memory_limit(self.config.max_vm_memory_bytes);
        runtime.set_max_stack_size(self.config.max_vm_stack_bytes);
        let interrupt_counter = Arc::clone(&counter);
        let interrupt_limit = self.config.max_interrupt_polls;
        runtime.set_interrupt_handler(Some(Box::new(move || {
            interrupt_counter.fetch_add(1, Ordering::Relaxed) >= interrupt_limit
        })));

        let context = Context::builder()
            .with::<intrinsic::Eval>()
            .build(&runtime)
            .map_err(|error| vm_error("context creation", error))?;

        context.with(|ctx| {
            ctx.eval::<(), _>(HARDEN_GLOBALS)
                .map_err(|error| vm_error("global hardening", error))?;
            let baseline = global_properties(&ctx)?;
            let install = format!("(function(){{'use strict';\n{}\n}})()", self.program);
            ctx.eval::<(), _>(install)
                .map_err(|error| vm_error("program installation", error))?;
            validate_export(&ctx, &baseline)?;
            let call = format!(
                "(function(){{'use strict'; const freeze = (value) => {{ if (value && typeof value === 'object') {{ Object.keys(value).forEach((key) => freeze(value[key])); Object.freeze(value); }} return value; }}; return globalThis.{RUNTIME_EXPORT_NAME}.{}(freeze({input_json})); }})()",
                method.name(),
            );
            let value: JsValue<'_> = ctx
                .eval(call)
                .map_err(|error| vm_error(method.stage(), error))?;
            let json = ctx
                .json_stringify(value)
                .map_err(|error| vm_error(method.stage(), error))?
                .ok_or(RuntimeVmError::MissingJsonResult {
                    stage: method.stage(),
                })?
                .to_string()
                .map_err(|error| vm_error(method.stage(), error))?;
            decode_json(method.stage(), &json, &self.config, maximum_result_bytes)
        })
    }
}

#[derive(Clone, Copy)]
enum AbiMethod {
    Initialize,
    Turn,
    Project,
}

impl AbiMethod {
    fn name(self) -> &'static str {
        match self {
            Self::Initialize => "initialize",
            Self::Turn => "turn",
            Self::Project => "project",
        }
    }

    fn stage(self) -> &'static str {
        match self {
            Self::Initialize => "initialize",
            Self::Turn => "turn",
            Self::Project => "project",
        }
    }
}

const HARDEN_GLOBALS: &str = r#"
(function () {
  'use strict';
  const global = globalThis;
  const disableConstructor = (value) => {
    if (value && value.prototype) {
      Object.defineProperty(value.prototype, 'constructor', {
        value: undefined, writable: false, configurable: false,
      });
    }
  };
  [Object, Function, Array, String, Boolean, Number, Error, EvalError, RangeError,
   ReferenceError, SyntaxError, TypeError, URIError].forEach(disableConstructor);
  for (const name of ['eval', 'Function', 'Promise', 'Date', 'fetch', 'setTimeout',
    'setInterval', 'clearTimeout', 'clearInterval', 'queueMicrotask', 'WebAssembly',
    'window', 'document', 'process', 'require', 'module', 'exports', 'global', 'Deno', 'Bun']) {
    Object.defineProperty(global, name, {
      value: undefined, writable: false, configurable: false,
    });
  }
  if (global.Math) {
    Object.defineProperty(global.Math, 'random', {
      value: undefined, writable: false, configurable: false,
    });
  }
})();
"#;

fn validate_export(
    ctx: &rquickjs::Ctx<'_>,
    baseline: &BTreeSet<String>,
) -> Result<(), RuntimeVmError> {
    let after = global_properties(ctx)?;
    let additions = after.difference(baseline).cloned().collect::<BTreeSet<_>>();
    let expected = BTreeSet::from([RUNTIME_EXPORT_NAME.to_owned()]);
    if additions != expected {
        return Err(RuntimeVmError::ExportContract(format!(
            "program must add only {RUNTIME_EXPORT_NAME}; added {:?}",
            additions
        )));
    }
    let valid: bool = ctx
        .eval(format!(
            "(function(){{ const runtime = globalThis.{RUNTIME_EXPORT_NAME}; if (!runtime || typeof runtime !== 'object') return false; const names = Object.getOwnPropertyNames(runtime).sort().join(','); if (names !== 'initialize,project,turn') return false; if (typeof runtime.initialize !== 'function' || typeof runtime.turn !== 'function' || typeof runtime.project !== 'function') return false; Object.freeze(runtime); return typeof eval === 'undefined' && typeof Function === 'undefined' && typeof Promise === 'undefined' && typeof Date === 'undefined' && typeof fetch === 'undefined' && typeof setTimeout === 'undefined' && typeof WebAssembly === 'undefined' && typeof Math.random === 'undefined'; }})()"
        ))
        .map_err(|error| vm_error("runtime export validation", error))?;
    if !valid {
        return Err(RuntimeVmError::ExportContract(
            "expected exactly initialize, turn, and project functions with hardened ambient globals".into(),
        ));
    }
    Ok(())
}

fn global_properties(ctx: &rquickjs::Ctx<'_>) -> Result<BTreeSet<String>, RuntimeVmError> {
    let names: String = ctx
        .eval("Object.getOwnPropertyNames(globalThis).sort().join('\\u0000')")
        .map_err(|error| vm_error("global inspection", error))?;
    Ok(names.split('\0').map(str::to_owned).collect())
}

fn validate_config(config: &RuntimeVmConfig) -> Result<(), RuntimeVmError> {
    if config.max_program_bytes == 0
        || config.max_state_bytes == 0
        || config.max_projection_bytes == 0
        || config.max_initialize_facts_bytes == 0
        || config.max_initialize_facts_json_depth == 0
        || config.max_initialize_facts_json_nodes == 0
        || config.max_initialize_facts_json_string_bytes == 0
        || config.max_json_depth == 0
        || config.max_json_nodes == 0
        || config.max_json_string_bytes == 0
        || config.max_vm_memory_bytes == 0
        || config.max_vm_stack_bytes == 0
        || config.max_interrupt_polls == 0
    {
        return Err(RuntimeVmError::InvalidConfiguration(
            "all VM and JSON limits must be non-zero",
        ));
    }
    Ok(())
}

fn canonicalize_json(
    stage: &'static str,
    value: Value,
    config: &RuntimeVmConfig,
    maximum_bytes: usize,
) -> Result<Value, RuntimeVmError> {
    canonicalize_json_with_bounds(stage, value, ordinary_input_bounds(config, maximum_bytes))
}

fn canonicalize_initialize_facts(
    value: Value,
    config: &RuntimeVmConfig,
) -> Result<Value, RuntimeVmError> {
    canonicalize_json_with_bounds(
        "initialize facts",
        value,
        JsonValidationBounds {
            maximum_bytes: config.max_initialize_facts_bytes,
            maximum_depth: config.max_initialize_facts_json_depth,
            maximum_nodes: config.max_initialize_facts_json_nodes,
            maximum_string_bytes: config.max_initialize_facts_json_string_bytes,
        },
    )
}

fn canonicalize_json_with_bounds(
    stage: &'static str,
    value: Value,
    bounds: JsonValidationBounds,
) -> Result<Value, RuntimeVmError> {
    validate_json_with_bounds(stage, &value, bounds)?;
    let bytes = serde_json::to_vec(&value).expect("JSON value must serialize");
    serde_json::from_slice(&bytes).map_err(|error| RuntimeVmError::InvalidJson {
        stage,
        message: error.to_string(),
    })
}

fn decode_json(
    stage: &'static str,
    bytes: &str,
    config: &RuntimeVmConfig,
    maximum_bytes: usize,
) -> Result<Value, RuntimeVmError> {
    if bytes.len() > maximum_bytes {
        return Err(RuntimeVmError::JsonTooLarge {
            stage,
            actual: bytes.len(),
            maximum: maximum_bytes,
        });
    }
    let value = serde_json::from_str(bytes).map_err(|error| RuntimeVmError::InvalidJson {
        stage,
        message: error.to_string(),
    })?;
    canonicalize_json(stage, value, config, maximum_bytes)
}

fn decode_projection(
    projection: Value,
    config: &RuntimeVmConfig,
) -> Result<RuntimeVmProjection, RuntimeVmError> {
    validate_json(
        "projection",
        &projection,
        config,
        config.max_projection_bytes,
    )?;
    let entries = projection
        .as_object()
        .ok_or_else(|| RuntimeVmError::InvalidJson {
            stage: "projection",
            message: "expected an object with ui and optional render fields".into(),
        })?;
    if !entries.contains_key("ui") {
        return Err(RuntimeVmError::InvalidJson {
            stage: "projection",
            message: "missing required ui field".into(),
        });
    }
    if let Some(field) = entries
        .keys()
        .find(|field| field.as_str() != "ui" && field.as_str() != "render")
    {
        return Err(RuntimeVmError::InvalidJson {
            stage: "projection",
            message: format!("unknown field {field:?}"),
        });
    }
    let ui = entries["ui"].clone();
    validate_json("projection ui", &ui, config, config.max_projection_bytes)?;
    let render = entries.get("render").cloned();
    if let Some(render) = &render {
        validate_json(
            "projection render",
            render,
            config,
            config.max_projection_bytes,
        )?;
    }
    Ok(RuntimeVmProjection { ui, render })
}

fn validate_json(
    stage: &'static str,
    value: &Value,
    config: &RuntimeVmConfig,
    maximum_bytes: usize,
) -> Result<(), RuntimeVmError> {
    validate_json_with_bounds(
        stage,
        value,
        JsonValidationBounds {
            maximum_bytes,
            maximum_depth: config.max_json_depth,
            maximum_nodes: config.max_json_nodes,
            maximum_string_bytes: config.max_json_string_bytes,
        },
    )
}

#[derive(Clone, Copy)]
struct JsonValidationBounds {
    maximum_bytes: usize,
    maximum_depth: usize,
    maximum_nodes: usize,
    maximum_string_bytes: usize,
}

fn ordinary_input_bounds(config: &RuntimeVmConfig, maximum_bytes: usize) -> JsonValidationBounds {
    JsonValidationBounds {
        maximum_bytes,
        maximum_depth: config.max_json_depth,
        maximum_nodes: config.max_json_nodes,
        maximum_string_bytes: config.max_json_string_bytes,
    }
}

fn initialize_input_bounds(config: &RuntimeVmConfig) -> JsonValidationBounds {
    JsonValidationBounds {
        // The ABI wrapper contributes only the "facts" key and object braces;
        // the separately admitted facts value remains at its exact bound.
        maximum_bytes: config.max_initialize_facts_bytes.saturating_add(10),
        maximum_depth: config.max_initialize_facts_json_depth.saturating_add(1),
        maximum_nodes: config.max_initialize_facts_json_nodes.saturating_add(1),
        maximum_string_bytes: config.max_initialize_facts_json_string_bytes,
    }
}

fn validate_json_with_bounds(
    stage: &'static str,
    value: &Value,
    bounds: JsonValidationBounds,
) -> Result<(), RuntimeVmError> {
    let encoded = serde_json::to_vec(value).expect("JSON value must serialize");
    if encoded.len() > bounds.maximum_bytes {
        return Err(RuntimeVmError::JsonTooLarge {
            stage,
            actual: encoded.len(),
            maximum: bounds.maximum_bytes,
        });
    }
    let mut nodes = 0;
    validate_json_value(stage, value, bounds, 0, &mut nodes)
}

fn validate_json_value(
    stage: &'static str,
    value: &Value,
    bounds: JsonValidationBounds,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), RuntimeVmError> {
    if depth > bounds.maximum_depth {
        return Err(RuntimeVmError::JsonTooDeep {
            stage,
            maximum: bounds.maximum_depth,
        });
    }
    *nodes += 1;
    if *nodes > bounds.maximum_nodes {
        return Err(RuntimeVmError::TooManyJsonNodes {
            stage,
            maximum: bounds.maximum_nodes,
        });
    }
    match value {
        Value::String(string) => {
            if string.len() > bounds.maximum_string_bytes {
                return Err(RuntimeVmError::JsonStringTooLarge {
                    stage,
                    maximum: bounds.maximum_string_bytes,
                });
            }
        }
        Value::Array(values) => {
            for value in values {
                validate_json_value(stage, value, bounds, depth + 1, nodes)?;
            }
        }
        Value::Object(entries) => {
            for (key, value) in entries {
                if key.len() > bounds.maximum_string_bytes {
                    return Err(RuntimeVmError::JsonStringTooLarge {
                        stage,
                        maximum: bounds.maximum_string_bytes,
                    });
                }
                validate_json_value(stage, value, bounds, depth + 1, nodes)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(())
}

fn fingerprint(state: &Value) -> ProductStateFingerprint {
    let mut digest = Sha256::new();
    digest.update(serde_json::to_vec(state).expect("JSON value must serialize"));
    ProductStateFingerprint(digest.finalize().into())
}

fn vm_error(stage: &'static str, error: rquickjs::Error) -> RuntimeVmError {
    RuntimeVmError::Vm {
        stage,
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commits_only_completed_turns_and_exposes_no_ambient_runtime_helpers() {
        let program = r#"
          globalThis.__rustyEngineRuntime = {
            initialize({ facts }) {
              return {
                count: facts.content[0].value.start,
                composition: facts.composition.id,
                contentPath: facts.content[0].path,
                last: 'initialize',
              };
            },
            turn({ state, step, input }) {
              if (input.failTurn) throw new Error('rejected turn');
              if (input.failProjection) return { count: state.count + 1, failProjection: true };
              if (input.badProjection) return { count: state.count + 1, badProjection: input.badProjection };
              return { count: state.count + step.delta, last: input.label };
            },
            project({ state }) {
              if (state.failProjection) throw new Error('rejected projection');
              if (state.badProjection === 'unknown') return { ui: {}, unexpected: true };
              if (state.badProjection === 'missing') return { render: {} };
              return {
                ui: {
                  text: state.last + ':' + state.count,
                  ambient: {
                    date: typeof Date,
                    random: typeof Math.random,
                    evaluate: typeof eval,
                    function: typeof Function,
                    promise: typeof Promise,
                    fetch: typeof fetch,
                    timer: typeof setTimeout,
                    wasm: typeof WebAssembly,
                  },
                },
                render: { count: state.count },
              };
            },
          };
        "#;
        let mut vm = RuntimeVm::new(program, RuntimeVmConfig::default()).unwrap();

        let initialized = vm
            .initialize(serde_json::json!({
                "composition": { "id": "test-composition" },
                "content": [{
                    "path": "content/runtime.json",
                    "value": { "start": 1 },
                }],
            }))
            .unwrap();
        assert_eq!(initialized.state().revision(), 1);
        assert_eq!(initialized.projection().ui()["text"], "initialize:1");
        assert_eq!(
            initialized.state().state()["composition"],
            "test-composition"
        );
        assert_eq!(
            initialized.state().state()["contentPath"],
            "content/runtime.json"
        );
        assert!(initialized.projection().ui()["ambient"]
            .as_object()
            .unwrap()
            .values()
            .all(|value| value == "undefined"));
        assert_eq!(initialized.projection().render().unwrap()["count"], 1);

        let first = vm
            .turn(RuntimeTurn {
                step: serde_json::json!({ "delta": 2 }),
                input: serde_json::json!({ "label": "first" }),
            })
            .unwrap();
        let first_fingerprint = first.state().fingerprint();
        assert_eq!(first.state().revision(), 2);
        assert_eq!(first.projection().ui()["text"], "first:3");

        let second = vm
            .turn(RuntimeTurn {
                step: serde_json::json!({ "delta": 4 }),
                input: serde_json::json!({ "label": "second" }),
            })
            .unwrap();
        assert_eq!(second.state().revision(), 3);
        assert_eq!(second.projection().ui()["text"], "second:7");
        assert_ne!(second.state().fingerprint(), first_fingerprint);

        let prepared = vm
            .prepare_turn(RuntimeTurn {
                step: serde_json::json!({ "delta": 1 }),
                input: serde_json::json!({ "label": "prepared" }),
            })
            .unwrap();
        assert_eq!(vm.state(), Some(second.state()));
        let committed = vm.commit_prepared(prepared).unwrap();
        assert_eq!(committed.state().revision(), 4);

        let before_failure = vm.state().cloned().unwrap();
        assert!(vm
            .turn(RuntimeTurn {
                step: serde_json::json!({ "delta": 1 }),
                input: serde_json::json!({ "failTurn": true }),
            })
            .is_err());
        assert_eq!(vm.state(), Some(&before_failure));

        assert!(vm
            .turn(RuntimeTurn {
                step: serde_json::json!({ "delta": 1 }),
                input: serde_json::json!({ "failProjection": true }),
            })
            .is_err());
        assert_eq!(vm.state(), Some(&before_failure));

        for bad_projection in ["unknown", "missing"] {
            assert!(vm
                .turn(RuntimeTurn {
                    step: serde_json::json!({ "delta": 1 }),
                    input: serde_json::json!({ "badProjection": bad_projection }),
                })
                .is_err());
            assert_eq!(vm.state(), Some(&before_failure));
        }
    }
}
