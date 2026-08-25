use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{Shutdown, TcpStream},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use content_store::{encode_manifest, ArtifactRole, ContentArtifact, ContentManifest};
use product_model::{
    decode_product_manifest, CapabilityAccess, CapabilityAvailability, CapabilityBudget,
    CapabilityKind, CapabilityMetadata, CapabilityProvenance, CapabilityUses,
    ProductKernelCapabilityDescriptor,
};

use crate::{
    decode_assembly_receipt, plan_product_assembly, verify_product_assembly, AssemblyEntryKind,
    AssemblyGenerationInputs, AssemblyPublication, BrowserBundleInputs, PublicationFile,
    PublicationOutput,
};

const MANIFEST: &str = r#"[product]
id = "rusty.test"

[runtime_composition]
entrypoints = ["rules/main.ts"]

[lifecycle]
mode = "demand"

[ui]
entry = "ui/main.ts"

[content]
root = "content"

[outputs]
compiled_composition = "generated/compiled-composition.json"
admitted_runtime_content = "generated/runtime-content"
product_assembly = "generated/product-assembly"
product_bundle = "generated/product-bundle"
"#;

const EMPTY_COMPOSITION: &str = r#"{
  "product": "rusty.test",
  "intentDescriptors": [],
  "inputMap": [],
  "schedule": [
    {"phase": "input", "mode": "append", "systems": []},
    {"phase": "simulation", "mode": "append", "systems": []},
    {"phase": "consequences", "mode": "append", "systems": []},
    {"phase": "commit", "mode": "append", "systems": []},
    {"phase": "projection", "mode": "append", "systems": []}
  ],
  "gameplayDefinitions": [],
  "timelines": [],
  "capabilityBindings": []
}
"#;

const COUNTER_COMPOSITION: &str = r#"{
  "product": "rusty.test",
  "intentDescriptors": [
    {
      "id": "increment",
      "valueKind": "digital",
      "capability": "counter.increment",
      "payload": {"amount": 1}
    }
  ],
  "inputMap": [],
  "schedule": [
    {"phase": "input", "mode": "append", "systems": []},
    {"phase": "simulation", "mode": "append", "systems": []},
    {"phase": "consequences", "mode": "append", "systems": []},
    {"phase": "commit", "mode": "append", "systems": []},
    {"phase": "projection", "mode": "append", "systems": []}
  ],
  "gameplayDefinitions": [],
  "timelines": [],
  "capabilityBindings": [
    {"id": "counter.increment", "target": "kernel.counter-increment"}
  ]
}
"#;

const COUNTER_KERNEL_SOURCE: &str = r#"use rusty_engine::{
    product_kernel::{
        ProductKernelRuntimeDefinition, ProductKernelRuntimeMutationDescriptor,
        ProductKernelRuntimeSelection, ProductRuntimeResources,
    },
    product_model::{
        CapabilityAccess, CapabilityAvailability, CapabilityBudget, CapabilityKind,
        CapabilityMetadata, CapabilityProvenance, CapabilityUses,
        ProductKernelCapabilityDescriptor,
    },
    runtime_composition::{ProductRuntimeAdapter, ProductRuntimeOutputs, ProductRuntimeUi},
    runtime_input, runtime_lifecycle, runtime_mutation, runtime_schedule, runtime_timeline,
};

pub struct CounterAuthority {
    value: u64,
    revision: u64,
}

impl runtime_mutation::MutationAuthority for CounterAuthority {
    type Guard = (u64, u64);

    fn guard(&self) -> Self::Guard {
        (self.value, self.revision)
    }

    fn publication_domain(&self) -> &str {
        "counter"
    }
}

#[derive(Default)]
pub struct CounterPlanner;

impl runtime_mutation::MutationPlanner<CounterAuthority, u32> for CounterPlanner {
    type Error = String;

    fn stage(
        &mut self,
        authority: &CounterAuthority,
        batch: &runtime_mutation::MutationResolvedBatch,
    ) -> Result<runtime_mutation::MutationStage<CounterAuthority, u32>, Self::Error> {
        let mut candidate = CounterAuthority {
            value: authority.value,
            revision: authority.revision,
        };
        for operation in batch.operations() {
            let amount = operation
                .payload()
                .get("amount")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| "counter mutation payload has no amount".to_owned())?;
            candidate.value = candidate
                .value
                .checked_add(amount)
                .ok_or_else(|| "counter value overflowed".to_owned())?;
        }
        candidate.revision = candidate
            .revision
            .checked_add(1)
            .ok_or_else(|| "counter revision overflowed".to_owned())?;
        let evidence = batch
            .operations()
            .iter()
            .enumerate()
            .map(|(index, operation)| {
                runtime_mutation::MutationOwnerEvidence::for_operation(operation, index as u32)
            })
            .collect();
        Ok(runtime_mutation::MutationStage::new(candidate, evidence))
    }
}

pub struct CounterAdapter {
    authority: CounterAuthority,
    planner: CounterPlanner,
    pending: Option<runtime_mutation::MutationBatch>,
}

impl ProductRuntimeAdapter for CounterAdapter {
    type Authority = CounterAuthority;
    type Guard = (u64, u64);
    type Planner = CounterPlanner;
    type Evidence = u32;
    type Error = String;
    type ScheduleOutput = String;
    type UiOutput = String;

    fn on_input(
        &mut self,
        _frame: &runtime_input::InputFrame,
        intents: &[runtime_input::RuntimeIntentEnvelope],
    ) -> Result<(), Self::Error> {
        for intent in intents {
            if intent.intent() != "increment" {
                continue;
            }
            let runtime_input::RuntimeIntentValue::Digital { active } = intent.value() else {
                return Err("counter increment requires a digital intent".to_owned());
            };
            if !active {
                continue;
            }
            let operation = runtime_mutation::MutationOperation::new(
                runtime_mutation::MutationOperationId::new(intent.sequence()),
                "counter.increment",
                "kernel.counter-increment",
                serde_json::json!({"amount": 1}),
            )
            .map_err(|error| format!("counter operation: {error}"))?;
            self.pending = Some(
                runtime_mutation::MutationBatch::new(
                    runtime_mutation::MutationBatchId::new(
                        format!("counter-step-{}", intent.sequence()),
                    )
                    .map_err(|error| format!("counter batch: {error}"))?,
                    runtime_mutation::MutationCausation::new("input.increment")
                        .map_err(|error| format!("counter causation: {error}"))?,
                    runtime_mutation::MutationProvenance::new("counter.runtime")
                        .map_err(|error| format!("counter provenance: {error}"))?,
                    vec![operation],
                )
                .map_err(|error| format!("counter batch: {error}"))?,
            );
        }
        Ok(())
    }

    fn dispatch_schedule(
        &mut self,
        invocation: runtime_schedule::ScheduleSystemInvocation<'_>,
        _lifecycle: &runtime_lifecycle::RuntimeLifecycle,
        _token: runtime_lifecycle::RuntimePhaseToken,
    ) -> Result<Self::ScheduleOutput, Self::Error> {
        Err(format!("unexpected counter system {}", invocation.system_id()))
    }

    fn on_timeline_releases(
        &mut self,
        _releases: &runtime_timeline::TimelineRelease,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn prepare_mutation(
        &mut self,
        _step: runtime_lifecycle::SimulationStep,
    ) -> Result<Option<runtime_mutation::MutationBatch>, Self::Error> {
        Ok(self.pending.take())
    }

    fn mutation_parts(&mut self) -> (&mut Self::Authority, &mut Self::Planner) {
        (&mut self.authority, &mut self.planner)
    }

    fn project(
        &mut self,
        _lifecycle: &runtime_lifecycle::RuntimeLifecycle,
        _token: runtime_lifecycle::RuntimePhaseToken,
    ) -> Result<ProductRuntimeOutputs<Self::UiOutput>, Self::Error> {
        ProductRuntimeOutputs::new(
            vec![ProductRuntimeUi::new(
                "counter",
                "counter.v1",
                self.authority.value.to_string(),
            )],
            None,
            None,
        )
        .map_err(|error| error.to_string())
    }
}

pub struct RustyProductRuntime;

const CAPABILITIES: &[ProductKernelCapabilityDescriptor] = &[ProductKernelCapabilityDescriptor::new(
    "counter-increment",
    CapabilityMetadata::new(
        CapabilityKind::Operation,
        CapabilityUses::INPUT_MAP,
        CapabilityAvailability::Linkable,
        CapabilityAccess::new(&[], &["counter.value"]),
        CapabilityBudget::new(4096),
        CapabilityProvenance::new(
            "rusty.test.kernel",
            "kernel/entry.rs",
            "counter_increment",
        ),
    ),
)];

const SELECTIONS: &[ProductKernelRuntimeSelection] = &[ProductKernelRuntimeSelection::new(
    "counter-increment",
    "kernel.counter-increment",
    "counter.increment.v1",
    CapabilityKind::Operation,
)];

const MUTATIONS: &[ProductKernelRuntimeMutationDescriptor] =
    &[ProductKernelRuntimeMutationDescriptor::new(
        "counter.increment",
        "kernel.counter-increment",
        "counter",
        "rusty.test.counter",
        "counter.increment.v1",
    )];

impl ProductKernelRuntimeDefinition for RustyProductRuntime {
    type Adapter = CounterAdapter;
    type Error = String;
    type ProductState = CounterAuthority;
    type ObserverComponent = ();
    type TargetComponent = ();

    fn capabilities() -> &'static [ProductKernelCapabilityDescriptor] {
        CAPABILITIES
    }

    fn selections() -> &'static [ProductKernelRuntimeSelection] {
        SELECTIONS
    }

    fn mutation_descriptors() -> &'static [ProductKernelRuntimeMutationDescriptor] {
        MUTATIONS
    }

    fn build(resources: ProductRuntimeResources<'_>) -> Result<Self::Adapter, Self::Error> {
        if resources.resource("content/manifest.json").is_none() {
            return Err("generated content resources are not available".to_owned());
        }
        Ok(CounterAdapter {
            authority: CounterAuthority {
                value: 0,
                revision: 0,
            },
            planner: CounterPlanner,
            pending: None,
        })
    }
}
"#;

struct Fixture {
    root: PathBuf,
    manifest: product_model::ProductManifest,
}

fn counter_capabilities() -> [ProductKernelCapabilityDescriptor; 1] {
    [ProductKernelCapabilityDescriptor::new(
        "counter-increment",
        CapabilityMetadata::new(
            CapabilityKind::Operation,
            CapabilityUses::INPUT_MAP,
            CapabilityAvailability::Linkable,
            CapabilityAccess::new(&[], &["counter.value"]),
            CapabilityBudget::new(4096),
            CapabilityProvenance::new("rusty.test.kernel", "kernel/entry.rs", "counter_increment"),
        ),
    )]
}

impl Fixture {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("rusty-product-assembly-{label}-{nonce}"));
        fs::create_dir_all(root.join("rules")).expect("rules");
        fs::create_dir_all(root.join("ui")).expect("ui");
        fs::create_dir_all(root.join("content")).expect("content");
        fs::create_dir_all(root.join("generated")).expect("generated");
        fs::write(root.join("rusty.toml"), MANIFEST).expect("manifest");
        fs::write(root.join("rules/main.ts"), "export const main = () => 1;\n").expect("rules");
        fs::write(root.join("rules/helper.ts"), "export const helper = 1;\n").expect("helper");
        fs::write(root.join("ui/main.ts"), "export const ui = {};\n").expect("ui");
        fs::write(root.join("content/.keep"), []).expect("keep");
        fs::write(
            root.join("generated/compiled-composition.json"),
            EMPTY_COMPOSITION,
        )
        .expect("composition");
        let manifest = decode_product_manifest(MANIFEST).expect("manifest admission");
        Self { root, manifest }
    }

    fn cleanup(self) {
        let _ = fs::remove_dir_all(self.root);
    }

    fn inputs(&self) -> AssemblyGenerationInputs {
        let browser = BrowserBundleInputs::new(
            "ui/main.js",
            vec![
                PublicationFile::new(
                    "index.html",
                    b"<!doctype html><script type=\"module\" src=\"./main.js\"></script>\n"
                        .to_vec(),
                )
                .expect("html"),
                PublicationFile::new(
                    "main.js",
                    b"import { mountProductUi } from './ui/main.js';\n".to_vec(),
                )
                .expect("main"),
                PublicationFile::new("bridge.js", b"import './runtime-adapter.js';\n".to_vec())
                    .expect("bridge"),
                PublicationFile::new(
                    "engine/product-browser-host.js",
                    b"export const host = true;\n".to_vec(),
                )
                .expect("engine host"),
                PublicationFile::new(
                    "runtime-adapter.js",
                    b"export const PRODUCT_RUNTIME_HTTP_BASE_PATH = '/';\n".to_vec(),
                )
                .expect("runtime adapter"),
                PublicationFile::new(
                    "ui/main.js",
                    b"export function mountProductUi() {}\n".to_vec(),
                )
                .expect("ui"),
            ],
        )
        .expect("browser bundle");
        AssemblyGenerationInputs::new(EMPTY_COMPOSITION.as_bytes().to_vec())
            .expect("composition input")
            .with_browser_bundle(browser)
            .expect("browser bundle input")
            .with_engine_dependency_path("../../rusty-engine")
            .expect("engine dependency path")
    }
}

#[test]
fn empty_assembly_is_deterministic_and_verifiable() {
    let fixture = Fixture::new("deterministic");
    let inputs = fixture.inputs();
    let plan = plan_product_assembly(&fixture.root, &fixture.manifest, &inputs).expect("plan");
    assert!(plan
        .receipt()
        .entries()
        .iter()
        .any(|entry| entry.kind() == AssemblyEntryKind::AuthoredSource
            && entry.path() == "rules/helper.ts"));
    assert!(plan
        .source_plan()
        .product_rs()
        .windows(b"RuntimeLifecycle".len())
        .any(|window| window == b"RuntimeLifecycle"));
    assert!(plan
        .source_plan()
        .product_rs()
        .windows(b"while !diagnostic.is_char_boundary(end)".len())
        .any(|window| window == b"while !diagnostic.is_char_boundary(end)"));
    assert!(plan
        .source_plan()
        .product_rs()
        .windows(b"diagnostic.truncate(end)".len())
        .any(|window| window == b"diagnostic.truncate(end)"));
    assert!(std::str::from_utf8(plan.source_plan().cargo_toml())
        .expect("generated Cargo")
        .contains("name = \"rusty_product\""));
    assert_eq!(
        std::str::from_utf8(plan.source_plan().lib_rs()).expect("generated library"),
        "#![forbid(unsafe_code)]\n\npub mod product;\n"
    );
    assert!(std::str::from_utf8(plan.source_plan().main_rs())
        .expect("generated binary")
        .contains("rusty_product::product::run(port)"));
    assert!(plan.receipt().entries().iter().any(|entry| {
        entry.kind() == AssemblyEntryKind::ExecutableWorkspace
            && entry.path() == "generated/product-assembly/src/lib.rs"
    }));
    let receipt_bytes = plan.receipt_bytes().expect("receipt");
    plan.publish(&fixture.root).expect("publish");
    let verified =
        verify_product_assembly(&fixture.root, &fixture.manifest, &inputs).expect("verify");
    let existing_verified =
        crate::verify_existing_product_assembly(&fixture.root, &fixture.manifest)
            .expect("existing-output verify");
    assert_eq!(existing_verified, verified);
    assert_eq!(verified, *plan.receipt());
    assert_eq!(receipt_bytes, plan.receipt_bytes().expect("receipt stable"));
    let receipt = decode_assembly_receipt(
        &fs::read(
            fixture
                .root
                .join("generated/product-assembly/assembly.json"),
        )
        .expect("receipt file"),
    )
    .expect("strict receipt");
    assert_eq!(receipt, verified);
    assert!(!verified.entries().iter().any(|entry| {
        entry.path().ends_with("/assembly.json")
            && matches!(
                entry.kind(),
                AssemblyEntryKind::ExecutableWorkspace | AssemblyEntryKind::BrowserBundle
            )
    }));
    fixture.cleanup();
}

#[test]
fn generated_package_compiles_with_direct_relocatable_engine_path() {
    let fixture = Fixture::new("generated-cargo");
    let generated_package = fixture.root.join("generated/product-assembly");
    let engine_package = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rusty-engine");
    let engine_path = relative_path(&generated_package, &engine_package);
    let inputs = fixture
        .inputs()
        .with_engine_dependency_path(engine_path)
        .expect("engine dependency path");
    let plan = plan_product_assembly(&fixture.root, &fixture.manifest, &inputs).expect("plan");
    plan.publish(&fixture.root).expect("publish");

    let target = fixture.root.join("generated/standalone-target");
    let status = std::process::Command::new("cargo")
        .args([
            "check",
            "--quiet",
            "--manifest-path",
            generated_package
                .join("Cargo.toml")
                .to_str()
                .expect("manifest path"),
            "--offline",
        ])
        .env("CARGO_TARGET_DIR", &target)
        .status()
        .expect("run generated cargo check");
    assert!(status.success(), "generated Cargo check failed: {status}");

    let consumer = fixture.root.join("generated/library-consumer");
    fs::create_dir_all(consumer.join("src")).expect("consumer source directory");
    fs::write(
        consumer.join("Cargo.toml"),
        "[package]\nname = \"library-consumer\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[dependencies]\nrusty-product = { package = \"rusty-product-rusty-test\", path = \"../product-assembly\" }\n\n[workspace]\n",
    )
    .expect("consumer manifest");
    fs::write(
        consumer.join("src/main.rs"),
        "fn main() { let _runtime = rusty_product::product::GeneratedProductDevRuntime::new().expect(\"runtime constructor\"); }\n",
    )
    .expect("consumer source");
    let consumer_status = std::process::Command::new("cargo")
        .args([
            "check",
            "--quiet",
            "--manifest-path",
            consumer
                .join("Cargo.toml")
                .to_str()
                .expect("consumer manifest path"),
            "--offline",
        ])
        .env("CARGO_TARGET_DIR", &target)
        .status()
        .expect("run generated library consumer check");
    assert!(
        consumer_status.success(),
        "generated library consumer check failed: {consumer_status}"
    );
    fixture.cleanup();
}

#[test]
fn generated_package_compiles_with_fixed_kernel_runtime_definition() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("rusty-product-assembly-kernel-{nonce}"));
    fs::create_dir_all(root.join("rules")).expect("rules");
    fs::create_dir_all(root.join("ui")).expect("ui");
    fs::create_dir_all(root.join("kernel")).expect("kernel");
    fs::create_dir_all(root.join("content")).expect("content");
    fs::create_dir_all(root.join("generated")).expect("generated");
    let manifest_text = MANIFEST.replace(
        "[ui]\nentry = \"ui/main.ts\"",
        "[kernel]\nentry = \"kernel/entry.rs\"\n\n[ui]\nentry = \"ui/main.ts\"",
    );
    fs::write(root.join("rusty.toml"), &manifest_text).expect("manifest");
    fs::write(root.join("rules/main.ts"), "export const main = () => 1;\n").expect("rules");
    fs::write(root.join("ui/main.ts"), "export const ui = {};\n").expect("ui");
    fs::write(root.join("content/.keep"), []).expect("keep");
    fs::write(
        root.join("generated/compiled-composition.json"),
        EMPTY_COMPOSITION,
    )
    .expect("composition");
    fs::write(
        root.join("kernel/entry.rs"),
        r#"use rusty_engine::{
    product_kernel::{ProductKernelRuntimeDefinition, ProductRuntimeResources},
    runtime_composition::{ProductRuntimeAdapter, ProductRuntimeOutputs},
    runtime_input, runtime_lifecycle, runtime_mutation, runtime_schedule, runtime_timeline,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authority;

impl runtime_mutation::MutationAuthority for Authority {
    type Guard = u64;

    fn guard(&self) -> Self::Guard { 0 }
    fn publication_domain(&self) -> &str { "kernel.test" }
}

#[derive(Debug, Default)]
pub struct Planner;

impl runtime_mutation::MutationPlanner<Authority, ()> for Planner {
    type Error = String;

    fn stage(
        &mut self,
        _authority: &Authority,
        _batch: &runtime_mutation::MutationResolvedBatch,
    ) -> Result<runtime_mutation::MutationStage<Authority, ()>, Self::Error> {
        Err("kernel test has no mutation batch".to_owned())
    }
}

pub struct KernelAdapter {
    authority: Authority,
    planner: Planner,
}

impl ProductRuntimeAdapter for KernelAdapter {
    type Authority = Authority;
    type Guard = u64;
    type Planner = Planner;
    type Evidence = ();
    type Error = String;
    type ScheduleOutput = String;
    type UiOutput = String;

    fn on_input(
        &mut self,
        _frame: &runtime_input::InputFrame,
        _intents: &[runtime_input::RuntimeIntentEnvelope],
    ) -> Result<(), Self::Error> { Ok(()) }

    fn dispatch_schedule(
        &mut self,
        invocation: runtime_schedule::ScheduleSystemInvocation<'_>,
        _lifecycle: &runtime_lifecycle::RuntimeLifecycle,
        _token: runtime_lifecycle::RuntimePhaseToken,
    ) -> Result<Self::ScheduleOutput, Self::Error> {
        Err(format!("unexpected system {}", invocation.system_id()))
    }

    fn on_timeline_releases(
        &mut self,
        _releases: &runtime_timeline::TimelineRelease,
    ) -> Result<(), Self::Error> { Ok(()) }

    fn prepare_mutation(
        &mut self,
        _step: runtime_lifecycle::SimulationStep,
    ) -> Result<Option<runtime_mutation::MutationBatch>, Self::Error> { Ok(None) }

    fn mutation_parts(&mut self) -> (&mut Self::Authority, &mut Self::Planner) {
        (&mut self.authority, &mut self.planner)
    }

    fn project(
        &mut self,
        _lifecycle: &runtime_lifecycle::RuntimeLifecycle,
        _token: runtime_lifecycle::RuntimePhaseToken,
    ) -> Result<ProductRuntimeOutputs<Self::UiOutput>, Self::Error> {
        ProductRuntimeOutputs::new(Vec::new(), None, None).map_err(|error| error.to_string())
    }
}

pub struct RustyProductRuntime;

impl ProductKernelRuntimeDefinition for RustyProductRuntime {
    type Adapter = KernelAdapter;
    type Error = String;
    type ProductState = ();
    type ObserverComponent = ();
    type TargetComponent = ();

    fn capabilities() -> &'static [rusty_engine::product_model::ProductKernelCapabilityDescriptor] { &[] }
    fn selections() -> &'static [rusty_engine::product_kernel::ProductKernelRuntimeSelection] { &[] }
    fn mutation_descriptors() -> &'static [rusty_engine::product_kernel::ProductKernelRuntimeMutationDescriptor] { &[] }
    fn build(_resources: ProductRuntimeResources<'_>) -> Result<Self::Adapter, Self::Error> {
        Ok(KernelAdapter { authority: Authority, planner: Planner })
    }
}
"#,
    )
    .expect("kernel source");
    let manifest = decode_product_manifest(&manifest_text).expect("manifest admission");
    let browser = BrowserBundleInputs::new(
        "ui/main.js",
        vec![
            PublicationFile::new(
                "index.html",
                b"<!doctype html><script type=\"module\" src=\"./main.js\"></script>\n".to_vec(),
            )
            .expect("html"),
            PublicationFile::new(
                "main.js",
                b"import { mountProductUi } from './ui/main.js';\n".to_vec(),
            )
            .expect("main"),
            PublicationFile::new("bridge.js", b"import './runtime-adapter.js';\n".to_vec())
                .expect("bridge"),
            PublicationFile::new(
                "engine/product-browser-host.js",
                b"export const host = true;\n".to_vec(),
            )
            .expect("engine host"),
            PublicationFile::new(
                "runtime-adapter.js",
                b"export const PRODUCT_RUNTIME_HTTP_BASE_PATH = '/';\n".to_vec(),
            )
            .expect("runtime adapter"),
            PublicationFile::new(
                "ui/main.js",
                b"export function mountProductUi() {}\n".to_vec(),
            )
            .expect("ui"),
        ],
    )
    .expect("browser bundle");
    let generated_package = root.join("generated/product-assembly");
    let engine_package = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rusty-engine");
    let kernel_package = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../product-kernel");
    let inputs = AssemblyGenerationInputs::new(EMPTY_COMPOSITION.as_bytes().to_vec())
        .expect("composition input")
        .with_browser_bundle(browser)
        .expect("browser bundle input")
        .with_engine_dependency_path(relative_path(&generated_package, &engine_package))
        .expect("engine dependency path")
        .with_kernel_dependency_path(relative_path(&generated_package, &kernel_package))
        .expect("kernel dependency path");
    let plan =
        crate::plan_product_assembly_with_kernel_capabilities(&root, &manifest, &inputs, &[])
            .expect("kernel plan");
    plan.publish(&root).expect("kernel publish");
    let target = root.join("target");
    let status = std::process::Command::new("cargo")
        .args([
            "check",
            "--quiet",
            "--manifest-path",
            generated_package
                .join("Cargo.toml")
                .to_str()
                .expect("manifest"),
            "--offline",
        ])
        .env("CARGO_TARGET_DIR", &target)
        .status()
        .expect("run generated kernel cargo check");
    assert!(
        status.success(),
        "generated kernel Cargo check failed: {status}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn generated_counter_host_publishes_mutation_and_ui_projection() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("rusty-product-assembly-counter-{nonce}"));
    fs::create_dir_all(root.join("rules")).expect("rules");
    fs::create_dir_all(root.join("ui")).expect("ui");
    fs::create_dir_all(root.join("kernel")).expect("kernel");
    fs::create_dir_all(root.join("content")).expect("content");
    fs::create_dir_all(root.join("generated")).expect("generated");
    let manifest_text = MANIFEST.replace(
        "[ui]\nentry = \"ui/main.ts\"",
        "[kernel]\nentry = \"kernel/entry.rs\"\n\n[ui]\nentry = \"ui/main.ts\"",
    );
    fs::write(root.join("rusty.toml"), &manifest_text).expect("manifest");
    fs::write(root.join("rules/main.ts"), "export const main = () => 1;\n").expect("rules");
    fs::write(root.join("ui/main.ts"), "export const ui = {};\n").expect("ui");
    fs::write(root.join("kernel/entry.rs"), COUNTER_KERNEL_SOURCE).expect("kernel");
    fs::write(root.join("content/.keep"), []).expect("keep");
    fs::write(
        root.join("generated/compiled-composition.json"),
        COUNTER_COMPOSITION,
    )
    .expect("composition");
    let manifest = decode_product_manifest(&manifest_text).expect("manifest admission");
    let browser = BrowserBundleInputs::new(
        "ui/main.js",
        vec![
            PublicationFile::new(
                "index.html",
                b"<!doctype html><script type=\"module\" src=\"./main.js\"></script>\n".to_vec(),
            )
            .expect("html"),
            PublicationFile::new(
                "main.js",
                b"import { mountProductUi } from './ui/main.js';\n".to_vec(),
            )
            .expect("main"),
            PublicationFile::new("bridge.js", b"import './runtime-adapter.js';\n".to_vec())
                .expect("bridge"),
            PublicationFile::new(
                "engine/product-browser-host.js",
                b"export const host = true;\n".to_vec(),
            )
            .expect("engine host"),
            PublicationFile::new(
                "runtime-adapter.js",
                b"export const PRODUCT_RUNTIME_HTTP_BASE_PATH = '/';\n".to_vec(),
            )
            .expect("runtime adapter"),
            PublicationFile::new(
                "ui/main.js",
                b"export function mountProductUi() {}\n".to_vec(),
            )
            .expect("ui"),
        ],
    )
    .expect("browser bundle");
    let generated_package = root.join("generated/product-assembly");
    let engine_package = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rusty-engine");
    let kernel_package = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../product-kernel");
    let inputs = AssemblyGenerationInputs::new(COUNTER_COMPOSITION.as_bytes().to_vec())
        .expect("composition input")
        .with_browser_bundle(browser)
        .expect("browser bundle input")
        .with_engine_dependency_path(relative_path(&generated_package, &engine_package))
        .expect("engine dependency path")
        .with_kernel_dependency_path(relative_path(&generated_package, &kernel_package))
        .expect("kernel dependency path");
    let capabilities = counter_capabilities();
    let plan = crate::plan_product_assembly_with_kernel_capabilities(
        &root,
        &manifest,
        &inputs,
        &capabilities,
    )
    .expect("counter plan");
    assert!(std::str::from_utf8(plan.source_plan().lib_rs())
        .expect("generated kernel library")
        .contains("mod product_kernel_source;"));
    assert!(!std::str::from_utf8(plan.source_plan().main_rs())
        .expect("generated kernel binary")
        .contains("product_kernel_source"));
    assert!(!std::str::from_utf8(plan.source_plan().product_rs())
        .expect("generated source")
        .contains("struct EmptyProductAdapter"));
    plan.publish(&root).expect("counter publish");

    let target = root.join("target");
    let status = std::process::Command::new("cargo")
        .args([
            "build",
            "--quiet",
            "--manifest-path",
            generated_package
                .join("Cargo.toml")
                .to_str()
                .expect("manifest"),
            "--offline",
        ])
        .env("CARGO_TARGET_DIR", &target)
        .status()
        .expect("build generated counter host");
    assert!(
        status.success(),
        "generated counter host build failed: {status}"
    );

    let binary = target.join("debug/rusty-product-rusty-test");
    let mut child = std::process::Command::new(binary)
        .arg("--port")
        .arg("0")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("start generated counter host");
    let stdout = child.stdout.take().expect("host stdout");
    let mut stdout = BufReader::new(stdout);
    let mut origin = String::new();
    stdout.read_line(&mut origin).expect("host origin");
    let origin = origin.trim().to_owned();
    let authority = origin
        .strip_prefix("http://")
        .expect("loopback origin authority")
        .to_owned();
    let mut sse = TcpStream::connect(&authority).expect("connect output stream");
    sse.set_read_timeout(Some(std::time::Duration::from_secs(3)))
        .expect("output timeout");
    let sse_request = format!(
        "GET /__rusty/product/runtime/outputs HTTP/1.1\r\nHost: {authority}\r\nAccept: text/event-stream\r\nConnection: keep-alive\r\n\r\n"
    );
    sse.write_all(sse_request.as_bytes())
        .expect("subscribe outputs");
    std::thread::sleep(std::time::Duration::from_millis(60));

    let start_request = format!(
        "POST /__rusty/product/runtime/lifecycle/start HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{{}}"
    );
    let start = http_request(&origin, &start_request);
    assert!(
        start.starts_with("HTTP/1.1 200 OK\r\n") && start.contains("\"accepted\":true"),
        "start response: {start}"
    );
    let input_body = r#"{"batch":[{"runtime":{"instanceId":"1","generation":"1","controlRevision":"1"},"sequence":"0","context":"gameplay.default","intent":"increment","value":{"kind":"digital","active":true}}]}"#;
    let input_request = format!(
        "POST /__rusty/product/runtime/input HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        input_body.len(),
        input_body
    );
    let input = http_request(&origin, &input_request);
    assert!(
        input.starts_with("HTTP/1.1 200 OK\r\n") && input.contains("\"accepted\":true"),
        "input response: {input}"
    );
    let demand_request = format!(
        "POST /__rusty/product/runtime/admit-demand-step HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{{}}"
    );
    let demand = http_request(&origin, &demand_request);
    assert!(
        demand.starts_with("HTTP/1.1 200 OK\r\n") && demand.contains("\"accepted\":true"),
        "demand response: {demand}"
    );
    let mut output_bytes = Vec::new();
    for _ in 0..8 {
        let mut chunk = [0_u8; 2_048];
        let count = sse.read(&mut chunk).expect("read output stream");
        output_bytes.extend_from_slice(&chunk[..count]);
        let output = String::from_utf8_lossy(&output_bytes);
        if output.contains("\"kind\":\"ui-projection\"") && output.contains("\"value\":\"1\"") {
            break;
        }
    }
    let output = String::from_utf8_lossy(&output_bytes);
    assert!(
        output.contains("\"kind\":\"ui-projection\""),
        "counter UI projection missing: {output}"
    );
    assert!(
        output.contains("\"value\":\"1\""),
        "counter did not publish value one: {output}"
    );
    child
        .stdin
        .take()
        .expect("host stdin")
        .write_all(b"\n")
        .expect("request host shutdown");
    let status = child.wait().expect("wait generated counter host");
    assert!(
        status.success(),
        "generated counter host exited unsuccessfully: {status}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn relocation_has_the_same_receipt_bytes() {
    let first = Fixture::new("relocation-a");
    let second = Fixture::new("relocation-b");
    let first_inputs = first.inputs();
    let second_inputs = second.inputs();
    let first_plan =
        plan_product_assembly(&first.root, &first.manifest, &first_inputs).expect("first plan");
    let second_plan =
        plan_product_assembly(&second.root, &second.manifest, &second_inputs).expect("second plan");
    assert_eq!(
        first_plan.receipt_bytes().expect("first receipt"),
        second_plan.receipt_bytes().expect("second receipt")
    );
    first.cleanup();
    second.cleanup();
}

#[test]
fn relocated_generated_host_runs_without_authored_source() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let temporary_root = std::env::temp_dir().join(format!("rusty-product-host-{nonce}"));
    let original_root = temporary_root.join("original");
    let relocated_root = temporary_root.join("relocated");
    let mut fixture = Fixture::new("host-source");
    copy_tree(&fixture.root, &original_root);
    fs::remove_dir_all(&fixture.root).expect("remove copied temporary fixture");
    fixture.root = original_root.clone();

    let generated_package = fixture.root.join("generated/product-assembly");
    let engine_package =
        fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rusty-engine"))
            .expect("engine package");
    let inputs = fixture
        .inputs()
        .with_engine_dependency_path(relative_path(&generated_package, &engine_package))
        .expect("engine dependency path");
    let plan = plan_product_assembly(&fixture.root, &fixture.manifest, &inputs).expect("plan");
    plan.publish(&fixture.root).expect("publish");

    fs::create_dir_all(&relocated_root).expect("relocated root");
    copy_tree(
        &fixture.root.join("generated"),
        &relocated_root.join("generated"),
    );
    for path in ["rules", "ui", "content", "rusty.toml"] {
        let path = fixture.root.join(path);
        if path.is_dir() {
            fs::remove_dir_all(path).expect("remove authored lane");
        } else if path.exists() {
            fs::remove_file(path).expect("remove authored manifest");
        }
    }

    let relocated_package = relocated_root.join("generated/product-assembly");
    let target = relocated_root.join("target");
    let status = std::process::Command::new("cargo")
        .args([
            "build",
            "--quiet",
            "--manifest-path",
            relocated_package
                .join("Cargo.toml")
                .to_str()
                .expect("manifest"),
            "--offline",
        ])
        .env("CARGO_TARGET_DIR", &target)
        .status()
        .expect("build relocated generated package");
    assert!(
        status.success(),
        "relocated generated Cargo build failed: {status}"
    );

    let binary = target.join("debug/rusty-product-rusty-test");
    let mut child = std::process::Command::new(binary)
        .arg("--port")
        .arg("0")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("start relocated generated host");
    let stdout = child.stdout.take().expect("host stdout");
    let mut stdout = BufReader::new(stdout);
    let mut origin = String::new();
    stdout.read_line(&mut origin).expect("host origin");
    let origin = origin.trim().to_owned();
    assert!(
        origin.starts_with("http://127.0.0.1:"),
        "unexpected host origin: {origin}"
    );
    let authority = origin
        .strip_prefix("http://")
        .expect("loopback origin authority");

    let start_request = format!(
        "POST /__rusty/product/runtime/lifecycle/start HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{{}}"
    );
    let start = http_request(&origin, &start_request);
    assert!(
        start.starts_with("HTTP/1.1 200 OK\r\n"),
        "start response: {start}"
    );
    assert!(
        start.contains("\"accepted\":true"),
        "start response: {start}"
    );
    let demand_request = format!(
        "POST /__rusty/product/runtime/admit-demand-step HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{{}}"
    );
    let demand = http_request(&origin, &demand_request);
    assert!(
        demand.starts_with("HTTP/1.1 200 OK\r\n"),
        "demand response: {demand}"
    );
    assert!(
        demand.contains("\"accepted\":true"),
        "demand response: {demand}"
    );
    let index_request = format!("GET / HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n");
    let index = http_request(&origin, &index_request);
    assert!(
        index.starts_with("HTTP/1.1 200 OK\r\n"),
        "index response: {index}"
    );
    assert!(index.ends_with("<!doctype html><script type=\"module\" src=\"./main.js\"></script>\n"));

    child
        .stdin
        .take()
        .expect("host stdin")
        .write_all(b"\n")
        .expect("request host shutdown");
    let status = child.wait().expect("wait relocated generated host");
    assert!(
        status.success(),
        "generated host exited unsuccessfully: {status}"
    );
    let _ = fs::remove_dir_all(&relocated_root);
    fixture.cleanup();
    let _ = fs::remove_dir_all(&temporary_root);
}

#[test]
fn stale_source_and_tampered_output_are_rejected() {
    let fixture = Fixture::new("stale");
    let inputs = fixture.inputs();
    let plan = plan_product_assembly(&fixture.root, &fixture.manifest, &inputs).expect("plan");
    plan.publish(&fixture.root).expect("publish");
    fs::write(
        fixture.root.join("rules/helper.ts"),
        "export const helper = 2;\n",
    )
    .expect("mutate source");
    let error = verify_product_assembly(&fixture.root, &fixture.manifest, &inputs)
        .expect_err("stale source");
    assert_eq!(error.diagnostic().code(), "ASSEMBLY_RECEIPT_STALE");

    fs::write(
        fixture.root.join("rules/helper.ts"),
        "export const helper = 1;\n",
    )
    .expect("restore source");
    fs::write(
        fixture
            .root
            .join("generated/product-assembly/src/product.rs"),
        "tampered\n",
    )
    .expect("tamper output");
    let error = verify_product_assembly(&fixture.root, &fixture.manifest, &inputs)
        .expect_err("tampered output");
    assert_eq!(error.diagnostic().code(), "ASSEMBLY_PUBLICATION_READBACK");
    fixture.cleanup();
}

#[test]
fn cache_bodies_are_rejected_explicitly() {
    let fixture = Fixture::new("cache");
    let manifest = ContentManifest::new(vec![ContentArtifact::cache("build/cache.bin")]);
    fs::create_dir_all(fixture.root.join("content/build")).expect("cache dir");
    fs::write(
        fixture.root.join("content/manifest.json"),
        encode_manifest(&manifest).expect("manifest encoding"),
    )
    .expect("content manifest");
    fs::write(fixture.root.join("content/build/cache.bin"), b"cache").expect("cache body");
    let inputs = fixture.inputs();
    let error =
        plan_product_assembly(&fixture.root, &fixture.manifest, &inputs).expect_err("cache body");
    assert_eq!(error.diagnostic().code(), "ASSEMBLY_CONTENT_CACHE_BODY");
    fixture.cleanup();
}

#[test]
fn renderer_resource_roles_generate_a_hashed_browser_preload_descriptor() {
    let fixture = Fixture::new("renderer-preload");
    let png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut wav = vec![0_u8; 44];
    wav[..4].copy_from_slice(b"RIFF");
    wav[8..12].copy_from_slice(b"WAVE");
    fs::create_dir_all(fixture.root.join("content/renderer")).expect("renderer content");
    let manifest = ContentManifest::new(vec![
        ContentArtifact::durable(
            "renderer/sky.png",
            ArtifactRole::Resource("resource:renderer-texture".to_owned()),
            &png,
        ),
        ContentArtifact::durable(
            "renderer/theme.wav",
            ArtifactRole::Resource("resource:renderer-audio".to_owned()),
            &wav,
        ),
        ContentArtifact::durable(
            "renderer/ordinary.png",
            ArtifactRole::Resource("resource:dagger-render".to_owned()),
            &png,
        ),
    ]);
    fs::write(
        fixture.root.join("content/manifest.json"),
        encode_manifest(&manifest).expect("manifest"),
    )
    .expect("write manifest");
    fs::write(fixture.root.join("content/renderer/sky.png"), &png).expect("texture");
    fs::write(fixture.root.join("content/renderer/theme.wav"), &wav).expect("audio");
    fs::write(fixture.root.join("content/renderer/ordinary.png"), &png).expect("ordinary");

    let inputs = fixture.inputs();
    let plan = plan_product_assembly(&fixture.root, &fixture.manifest, &inputs).expect("plan");
    plan.publish(&fixture.root).expect("publish");
    let preload: serde_json::Value = serde_json::from_slice(
        &fs::read(
            fixture
                .root
                .join("generated/product-bundle/renderer-preload.json"),
        )
        .expect("preload"),
    )
    .expect("preload JSON");
    assert_eq!(preload["artifact"], "rusty.product.renderer-preload.v1");
    let resources = preload["resources"].as_array().expect("resources");
    assert_eq!(resources.len(), 2);
    assert!(resources.iter().any(|resource| {
        resource["identity"]
            .as_str()
            .is_some_and(|value| value.starts_with("texture-resource/"))
            && resource["mediaType"] == "image/png"
            && resource["path"] == "content/renderer/sky.png"
    }));
    assert!(resources.iter().any(|resource| {
        resource["identity"]
            .as_str()
            .is_some_and(|value| value.starts_with("audio-resource/"))
            && resource["mediaType"] == "audio/wav"
            && resource["path"] == "content/renderer/theme.wav"
    }));
    fixture.cleanup();
}

#[test]
fn renderer_resource_roles_fail_closed_on_wrong_media_or_hash() {
    let fixture = Fixture::new("renderer-preload-invalid");
    fs::create_dir_all(fixture.root.join("content/renderer")).expect("renderer content");
    let declared = b"not a png";
    let manifest = ContentManifest::new(vec![ContentArtifact::durable(
        "renderer/texture.png",
        ArtifactRole::Resource("resource:renderer-texture".to_owned()),
        declared,
    )]);
    fs::write(
        fixture.root.join("content/manifest.json"),
        encode_manifest(&manifest).expect("manifest"),
    )
    .expect("write manifest");
    fs::write(fixture.root.join("content/renderer/texture.png"), declared).expect("texture");
    let error = plan_product_assembly(&fixture.root, &fixture.manifest, &fixture.inputs())
        .expect_err("invalid PNG must fail");
    assert_eq!(
        error.diagnostic().code(),
        "ASSEMBLY_RENDERER_RESOURCE_MEDIA"
    );

    let png = b"\x89PNG\r\n\x1a\n";
    fs::write(fixture.root.join("content/renderer/texture.png"), png).expect("changed texture");
    let error = plan_product_assembly(&fixture.root, &fixture.manifest, &fixture.inputs())
        .expect_err("changed hash must fail");
    assert_eq!(error.diagnostic().code(), "ASSEMBLY_CONTENT_HASH_MISMATCH");
    fixture.cleanup();
}

#[test]
fn renderer_preload_validation_rejects_escaping_paths_and_oversized_media() {
    let png = b"\x89PNG\r\n\x1a\n";
    let error = crate::source::validate_renderer_preload_resource("texture", "../escape.png", png)
        .expect_err("renderer resource path must not escape content");
    assert_eq!(error.diagnostic().code(), "ASSEMBLY_RENDERER_RESOURCE_PATH");

    let error = crate::source::validate_renderer_preload_resource("texture", "%2e%2e.png", png)
        .expect_err("percent traversal alias must fail");
    assert_eq!(error.diagnostic().code(), "ASSEMBLY_RENDERER_RESOURCE_PATH");

    let mut oversized = vec![0_u8; 16 * 1024 * 1024 + 1];
    oversized[..8].copy_from_slice(png);
    let error =
        crate::source::validate_renderer_preload_resource("texture", "texture.png", &oversized)
            .expect_err("renderer texture byte bound");
    assert_eq!(
        error.diagnostic().code(),
        "ASSEMBLY_RENDERER_RESOURCE_MEDIA"
    );
}

#[cfg(unix)]
#[test]
fn symlinked_source_is_rejected() {
    let fixture = Fixture::new("symlink");
    std::os::unix::fs::symlink("../ui/main.ts", fixture.root.join("rules/link.ts"))
        .expect("symlink");
    let inputs = fixture.inputs();
    let error =
        plan_product_assembly(&fixture.root, &fixture.manifest, &inputs).expect_err("symlink");
    assert_eq!(error.diagnostic().code(), "ASSEMBLY_SYMLINK_REJECTED");
    fixture.cleanup();
}

#[cfg(unix)]
#[test]
fn symlinked_source_lane_is_not_followed() {
    let fixture = Fixture::new("symlink-lane");
    let outside = fixture
        .root
        .with_file_name("rusty-product-assembly-outside");
    fs::create_dir_all(&outside).expect("outside");
    fs::write(outside.join("main.ts"), "export const escaped = true;\n").expect("outside source");
    fs::remove_dir_all(fixture.root.join("rules")).expect("remove rules");
    std::os::unix::fs::symlink(&outside, fixture.root.join("rules")).expect("rules symlink");
    let inputs = fixture.inputs();
    let error = plan_product_assembly(&fixture.root, &fixture.manifest, &inputs)
        .expect_err("symlinked source lane");
    assert_eq!(error.diagnostic().code(), "ASSEMBLY_INPUT_PARENT");
    let _ = fs::remove_dir_all(outside);
    fixture.cleanup();
}

#[test]
fn browser_bundle_requires_fixed_relative_closure() {
    let required = vec![
        PublicationFile::new(
            "index.html",
            b"<!doctype html><script src=\"./main.js\"></script>\n".to_vec(),
        )
        .expect("html"),
        PublicationFile::new("main.js", b"import './ui/main.js';\n".to_vec()).expect("main"),
        PublicationFile::new("bridge.js", b"import './runtime-adapter.js';\n".to_vec())
            .expect("bridge"),
        PublicationFile::new("engine/product-browser-host.js", b"export {};\n".to_vec())
            .expect("engine"),
        PublicationFile::new("runtime-adapter.js", b"export {};\n".to_vec()).expect("adapter"),
    ];
    let error = BrowserBundleInputs::new("ui/main.js", required.clone())
        .expect_err("missing compiled UI entry");
    assert_eq!(error.diagnostic().code(), "ASSEMBLY_BROWSER_REQUIRED_FILE");

    let mut with_ui = required;
    with_ui.push(PublicationFile::new("ui/main.js", b"export {};\n".to_vec()).expect("ui"));
    with_ui.push(PublicationFile::new("assets/main.js.map", b"{}".to_vec()).expect("map"));
    let error = BrowserBundleInputs::new("ui/main.js", with_ui).expect_err("source map");
    assert_eq!(error.diagnostic().code(), "ASSEMBLY_BROWSER_FORBIDDEN_FILE");

    let mut with_ui = vec![
        PublicationFile::new("index.html", b"<script src=\"/main.js\"></script>".to_vec())
            .expect("html"),
        PublicationFile::new("main.js", b"import './ui/main.js';\n".to_vec()).expect("main"),
        PublicationFile::new("bridge.js", b"import './runtime-adapter.js';\n".to_vec())
            .expect("bridge"),
        PublicationFile::new("engine/product-browser-host.js", b"export {};\n".to_vec())
            .expect("engine"),
        PublicationFile::new("runtime-adapter.js", b"export {};\n".to_vec()).expect("adapter"),
        PublicationFile::new("ui/main.js", b"export {};\n".to_vec()).expect("ui"),
    ];
    let error = BrowserBundleInputs::new("ui/main.js", std::mem::take(&mut with_ui))
        .expect_err("absolute resource");
    assert_eq!(
        error.diagnostic().code(),
        "ASSEMBLY_BROWSER_ABSOLUTE_IMPORT"
    );

    for absolute in ["body{background:url('/asset.png')}", "fetch(\"/state\")"] {
        let files = vec![
            PublicationFile::new(
                "index.html",
                b"<script type=\"module\" src=\"./main.js\"></script>".to_vec(),
            )
            .expect("html"),
            PublicationFile::new("main.js", b"import './ui/main.js';\n".to_vec()).expect("main"),
            PublicationFile::new("bridge.js", b"import './runtime-adapter.js';\n".to_vec())
                .expect("bridge"),
            PublicationFile::new("engine/product-browser-host.js", b"export {};\n".to_vec())
                .expect("engine"),
            PublicationFile::new("runtime-adapter.js", b"export {};\n".to_vec()).expect("adapter"),
            PublicationFile::new("ui/main.js", absolute.as_bytes().to_vec()).expect("ui"),
        ];
        let error =
            BrowserBundleInputs::new("ui/main.js", files).expect_err("absolute browser resource");
        assert_eq!(
            error.diagnostic().code(),
            "ASSEMBLY_BROWSER_ABSOLUTE_IMPORT"
        );
    }
}

#[test]
fn complete_tree_swap_rolls_back_prior_generated_outputs() {
    let fixture = Fixture::new("rollback");
    fs::write(fixture.root.join("generated/sentinel.txt"), b"old").expect("sentinel");
    let first =
        PublicationOutput::file("generated/first.txt", b"new-first".to_vec()).expect("first");
    let second = PublicationOutput::directory(
        "generated/second",
        vec![PublicationFile::new("nested.txt", b"new-second".to_vec()).expect("nested")],
    )
    .expect("second");
    let publication = AssemblyPublication::new(vec![first, second]).expect("publication");
    let error = super::publish::publish_outputs_fail_after_swap(&fixture.root, &publication)
        .expect_err("injected failure");
    assert_eq!(
        error.diagnostic().code(),
        "ASSEMBLY_PUBLICATION_INJECTED_FAILURE"
    );
    assert_eq!(
        fs::read(fixture.root.join("generated/sentinel.txt")).expect("sentinel"),
        b"old"
    );
    assert!(!fixture.root.join("generated/first.txt").exists());
    assert!(!fixture.root.join("generated/second").exists());
    let stages = fs::read_dir(&fixture.root)
        .expect("root entries")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".rusty-assembly-stage-")
        })
        .count();
    assert_eq!(stages, 0, "successful rollback cleans its private stage");
    fixture.cleanup();
}

#[test]
fn receipt_decode_rejects_unknown_fields_and_unsorted_entries() {
    let fixture = Fixture::new("receipt");
    let inputs = fixture.inputs();
    let plan = plan_product_assembly(&fixture.root, &fixture.manifest, &inputs).expect("plan");
    let mut json = plan.receipt().json().expect("receipt");
    json = json.replacen("{\n", "{\n  \"unknown\": true,\n", 1);
    assert!(decode_assembly_receipt(json.as_bytes()).is_err());

    let mut entries = plan.receipt().entries().to_vec();
    entries.reverse();
    let value = serde_json::json!({
        "artifact": plan.receipt().artifact(),
        "product": plan.receipt().product(),
        "entries": entries,
    });
    let bytes = serde_json::to_vec(&value).expect("json");
    assert!(decode_assembly_receipt(&bytes).is_err());
    fixture.cleanup();
}

fn relative_path(from_directory: &Path, to: &Path) -> String {
    let from = from_directory.components().collect::<Vec<_>>();
    let to = to.components().collect::<Vec<_>>();
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    let mut components = Vec::new();
    for component in &from[common..] {
        if !matches!(component, Component::RootDir | Component::CurDir) {
            components.push("..");
        }
    }
    for component in &to[common..] {
        match component {
            Component::Normal(value) => components.push(value.to_str().expect("UTF-8 path")),
            Component::ParentDir => components.push(".."),
            Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
        }
    }
    components.join("/")
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("copy destination");
    for entry in fs::read_dir(source).expect("copy source") {
        let entry = entry.expect("copy entry");
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_tree(&source_path, &destination_path);
        } else {
            fs::copy(source_path, destination_path).expect("copy file");
        }
    }
}

fn http_request(origin: &str, request: &str) -> String {
    let address = origin.strip_prefix("http://").expect("loopback origin");
    let mut stream = TcpStream::connect(address).expect("connect generated host");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(3)))
        .expect("host read timeout");
    stream.write_all(request.as_bytes()).expect("write request");
    stream.shutdown(Shutdown::Write).expect("close request");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");
    response
}
