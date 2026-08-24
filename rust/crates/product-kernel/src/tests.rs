use super::*;
use product_model::{
    admit_checked_product_composition, decode_compiled_composition, decode_product_manifest,
    CapabilityAvailability, CapabilityKind, CapabilityUses,
};
use runtime_lifecycle::{
    RuntimeInstanceId, RuntimeLifecycle, RuntimeLifecycleConfig, RuntimePhase, RuntimePhaseToken,
};
use runtime_mutation::{
    CompiledMutationCatalog, MutationAuthority, MutationBatch, MutationBatchId,
    MutationCapabilityDescriptor, MutationCausation, MutationOperation, MutationOperationId,
    MutationOwnerEvidence, MutationPlanner, MutationProvenance, MutationResolvedBatch,
    MutationStage, RuntimeMutation,
};
use runtime_schedule::CompiledRuntimeSchedule;
use runtime_standard_capabilities::{ObservePairsPlan, OBSERVE_PAIRS_RESULT_KIND};
use serde_json::{json, Value};
use std::cell::Cell;

struct StealthSystem;
struct StealthOperation;
struct StealthSchemaV1;
struct StealthSchemaV2;
struct StealthMigrationV1ToV2;

pub struct MixedSystem;
pub struct MixedOperation;
pub struct MixedProjection;

#[derive(Debug)]
pub struct ExecutionSnapshot {
    value: Cell<u32>,
}

#[derive(Debug, Clone, Copy)]
pub struct ExecutionRequest {
    value: u32,
}

impl ProductKernelCapabilityContract for MixedSystem {
    type Snapshot = ExecutionSnapshot;
    type Request = ExecutionRequest;
    type Result = u16;
    type Error = &'static str;
    const TYPE_ID: &'static str = "mixed.system.v1";
    const TARGET: &'static str = "kernel.mixed.system";
    const KIND: CapabilityKind = CapabilityKind::System;
}

impl ProductKernelCapabilityContract for MixedOperation {
    type Snapshot = ExecutionSnapshot;
    type Request = ExecutionRequest;
    type Result = String;
    type Error = &'static str;
    const TYPE_ID: &'static str = "mixed.operation.v1";
    const TARGET: &'static str = "kernel.mixed.operation";
    const KIND: CapabilityKind = CapabilityKind::Operation;
}

impl ProductKernelCapabilityContract for MixedProjection {
    type Snapshot = ExecutionSnapshot;
    type Request = ();
    type Result = bool;
    type Error = &'static str;
    const TYPE_ID: &'static str = "mixed.projection.v1";
    const TARGET: &'static str = "kernel.mixed.projection";
    const KIND: CapabilityKind = CapabilityKind::Projection;
}

fn mixed_system(
    context: ProductSystemContext<'_, ExecutionSnapshot, ExecutionRequest>,
) -> Result<u16, &'static str> {
    let next = context
        .snapshot()
        .value
        .get()
        .checked_add(context.request().value)
        .ok_or("system overflow")?;
    context.snapshot().value.set(next);
    u16::try_from(next).map_err(|_| "system result overflow")
}

fn mixed_operation(
    context: ProductOperationContext<'_, ExecutionSnapshot, ExecutionRequest>,
) -> Result<String, &'static str> {
    let next = context
        .snapshot()
        .value
        .get()
        .checked_mul(context.request().value)
        .ok_or("operation overflow")?;
    context.snapshot().value.set(next);
    Ok(format!("alert:{next}"))
}

fn mixed_projection(
    context: ProductProjectionContext<'_, ExecutionSnapshot>,
) -> Result<bool, &'static str> {
    Ok(context.snapshot().value.get() > 0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Snapshot {
    value: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Request {
    value: u32,
}

impl ProductKernelCapabilityContract for StealthSystem {
    type Snapshot = Snapshot;
    type Request = Request;
    type Result = u32;
    type Error = &'static str;
    const TYPE_ID: &'static str = "stealth.system.v1";
    const TARGET: &'static str = "kernel.stealth.detect";
    const KIND: CapabilityKind = CapabilityKind::System;
}

impl ProductKernelCapabilityContract for StealthOperation {
    type Snapshot = Snapshot;
    type Request = Request;
    type Result = u32;
    type Error = &'static str;
    const TYPE_ID: &'static str = "stealth.operation.v1";
    const TARGET: &'static str = "kernel.stealth.advance-alert";
    const KIND: CapabilityKind = CapabilityKind::Operation;
}

impl ProductKernelSchemaContract for StealthSchemaV1 {
    const TYPE_ID: &'static str = "stealth.schema.v1";
}

impl ProductKernelSchemaContract for StealthSchemaV2 {
    const TYPE_ID: &'static str = "stealth.schema.v2";
}

impl ProductKernelMigrationContract for StealthMigrationV1ToV2 {
    const TYPE_ID: &'static str = "stealth.migration.v1-to-v2";
    const FROM_SCHEMA: &'static str = "stealth.schema.v1";
    const TO_SCHEMA: &'static str = "stealth.schema.v2";
}

struct MissingSchemaMigration;
impl ProductKernelMigrationContract for MissingSchemaMigration {
    const TYPE_ID: &'static str = "stealth.migration.missing";
    const FROM_SCHEMA: &'static str = "stealth.schema.missing";
    const TO_SCHEMA: &'static str = "stealth.schema.v1";
}

struct MismatchedMigration;
impl ProductKernelMigrationContract for MismatchedMigration {
    const TYPE_ID: &'static str = "stealth.migration.mismatch";
    const FROM_SCHEMA: &'static str = "stealth.schema.v2";
    const TO_SCHEMA: &'static str = "stealth.schema.v1";
}

struct BadTarget;
impl ProductKernelCapabilityContract for BadTarget {
    type Snapshot = Snapshot;
    type Request = Request;
    type Result = u32;
    type Error = &'static str;
    const TYPE_ID: &'static str = "bad.target.v1";
    const TARGET: &'static str = "kernel.bad.other";
    const KIND: CapabilityKind = CapabilityKind::System;
}

struct BadKind;
impl ProductKernelCapabilityContract for BadKind {
    type Snapshot = Snapshot;
    type Request = Request;
    type Result = u32;
    type Error = &'static str;
    const TYPE_ID: &'static str = "bad.kind.v1";
    const TARGET: &'static str = "kernel.bad.kind";
    const KIND: CapabilityKind = CapabilityKind::Operation;
}

struct BadMigrationCapability;
impl ProductKernelCapabilityContract for BadMigrationCapability {
    type Snapshot = Snapshot;
    type Request = Request;
    type Result = u32;
    type Error = &'static str;
    const TYPE_ID: &'static str = "bad.migration-capability.v1";
    const TARGET: &'static str = "kernel.bad.migration-capability";
    const KIND: CapabilityKind = CapabilityKind::Migration;
}

const OVERLONG_CONTRACT_TYPE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

struct BadSchemaIdentityDeclarationContract;
impl ProductKernelCapabilityContract for BadSchemaIdentityDeclarationContract {
    type Snapshot = Snapshot;
    type Request = Request;
    type Result = u32;
    type Error = &'static str;
    const TYPE_ID: &'static str = "bad.schema-identity.v1";
    const TARGET: &'static str = "kernel.bad.schema-identity";
    const KIND: CapabilityKind = CapabilityKind::System;
}

struct BadMigrationEndpointDeclarationContract;
impl ProductKernelCapabilityContract for BadMigrationEndpointDeclarationContract {
    type Snapshot = Snapshot;
    type Request = Request;
    type Result = u32;
    type Error = &'static str;
    const TYPE_ID: &'static str = "bad.migration-endpoint.v1";
    const TARGET: &'static str = "kernel.bad.migration-endpoint";
    const KIND: CapabilityKind = CapabilityKind::System;
}

struct OverlongSchemaContract;
impl ProductKernelSchemaContract for OverlongSchemaContract {
    const TYPE_ID: &'static str = OVERLONG_CONTRACT_TYPE;
}

struct OverlongMigrationContract;
impl ProductKernelMigrationContract for OverlongMigrationContract {
    const TYPE_ID: &'static str = OVERLONG_CONTRACT_TYPE;
    const FROM_SCHEMA: &'static str = "stealth.schema.v1";
    const TO_SCHEMA: &'static str = "stealth.schema.v2";
}

struct OverlongCapabilityContract;
impl ProductKernelCapabilityContract for OverlongCapabilityContract {
    type Snapshot = Snapshot;
    type Request = Request;
    type Result = u32;
    type Error = &'static str;
    const TYPE_ID: &'static str = OVERLONG_CONTRACT_TYPE;
    const TARGET: &'static str = "kernel.overlong-capability";
    const KIND: CapabilityKind = CapabilityKind::System;
}

product_kernel_declaration! {
    declaration: MissingSchemaDeclaration,
    owner: MissingSchemaOwner,
    capabilities: [
        System => StealthSystem {
            identity: "stealth.detect",
            kind: CapabilityKind::System,
            uses: CapabilityUses::SCHEDULE,
            availability: CapabilityAvailability::Linkable,
            reads: &[], writes: &[], maximum_compact_json_payload_bytes: 4096,
            owner: "tests", source: "tests/product-kernel.rs", logical_path: "system"
        }
    ],
    schemas: [SchemaV1 => StealthSchemaV1 { identity: "stealth.schema.v1" }],
    migrations: [
        Missing => MissingSchemaMigration {
            identity: "stealth.migration.missing",
            from: "stealth.schema.missing",
            to: "stealth.schema.v1"
        }
    ]
}

product_kernel_declaration! {
    declaration: DuplicateSchemaDeclaration,
    owner: DuplicateSchemaOwner,
    capabilities: [
        System => StealthSystem {
            identity: "stealth.detect",
            kind: CapabilityKind::System,
            uses: CapabilityUses::SCHEDULE,
            availability: CapabilityAvailability::Linkable,
            reads: &[], writes: &[], maximum_compact_json_payload_bytes: 4096,
            owner: "tests", source: "tests/product-kernel.rs", logical_path: "system"
        }
    ],
    schemas: [
        First => StealthSchemaV1 { identity: "stealth.schema.v1" },
        Duplicate => StealthSchemaV1 { identity: "stealth.schema.v1" }
    ],
    migrations: []
}

struct BadExecutionDeclaration;

static BAD_TYPE_LINKS: &[ProductKernelExecutionLink<StealthOwner>] =
    &[ProductKernelExecutionLink::new(
        StealthOwner::System,
        "stealth.detect",
        "kernel.stealth.detect",
        "wrong.contract.v1",
        "StealthSystem",
        "stealth_system",
        CapabilityKind::System,
    )];

static BAD_TARGET_LINKS: &[ProductKernelExecutionLink<StealthOwner>] =
    &[ProductKernelExecutionLink::new(
        StealthOwner::System,
        "stealth.detect",
        "kernel.wrong-target",
        "stealth.system.v1",
        "StealthSystem",
        "stealth_system",
        CapabilityKind::System,
    )];

static BAD_SOURCE_LINKS: &[ProductKernelExecutionLink<StealthOwner>] =
    &[ProductKernelExecutionLink::new(
        StealthOwner::System,
        "stealth.detect",
        "kernel.stealth.detect",
        "stealth.system.v1",
        "crate::StealthSystem",
        "crate::stealth_system",
        CapabilityKind::System,
    )];

impl ProductKernelDeclaration for BadExecutionDeclaration {
    type Owner = StealthOwner;

    fn entries() -> &'static [ProductKernelCapabilityEntry<Self::Owner>] {
        StealthDeclaration::entries()
    }

    fn descriptors() -> &'static [ProductKernelCapabilityDescriptor] {
        StealthDeclaration::descriptors()
    }

    fn schemas() -> &'static [ProductKernelSchemaDescriptor] {
        StealthDeclaration::schemas()
    }

    fn migrations() -> &'static [ProductKernelMigrationDescriptor] {
        StealthDeclaration::migrations()
    }

    fn execution_links() -> &'static [ProductKernelExecutionLink<Self::Owner>] {
        BAD_TYPE_LINKS
    }

    fn contract_json() -> Result<String, ProductAssemblyError> {
        StealthDeclaration::contract_json()
    }
}

struct BadTargetExecutionDeclaration;

impl ProductKernelDeclaration for BadTargetExecutionDeclaration {
    type Owner = StealthOwner;

    fn entries() -> &'static [ProductKernelCapabilityEntry<Self::Owner>] {
        StealthDeclaration::entries()
    }

    fn descriptors() -> &'static [ProductKernelCapabilityDescriptor] {
        StealthDeclaration::descriptors()
    }

    fn schemas() -> &'static [ProductKernelSchemaDescriptor] {
        StealthDeclaration::schemas()
    }

    fn migrations() -> &'static [ProductKernelMigrationDescriptor] {
        StealthDeclaration::migrations()
    }

    fn execution_links() -> &'static [ProductKernelExecutionLink<Self::Owner>] {
        BAD_TARGET_LINKS
    }

    fn contract_json() -> Result<String, ProductAssemblyError> {
        StealthDeclaration::contract_json()
    }
}

struct BadSourceExecutionDeclaration;

impl ProductKernelDeclaration for BadSourceExecutionDeclaration {
    type Owner = StealthOwner;

    fn entries() -> &'static [ProductKernelCapabilityEntry<Self::Owner>] {
        StealthDeclaration::entries()
    }

    fn descriptors() -> &'static [ProductKernelCapabilityDescriptor] {
        StealthDeclaration::descriptors()
    }

    fn schemas() -> &'static [ProductKernelSchemaDescriptor] {
        StealthDeclaration::schemas()
    }

    fn migrations() -> &'static [ProductKernelMigrationDescriptor] {
        StealthDeclaration::migrations()
    }

    fn execution_links() -> &'static [ProductKernelExecutionLink<Self::Owner>] {
        BAD_SOURCE_LINKS
    }

    fn contract_json() -> Result<String, ProductAssemblyError> {
        StealthDeclaration::contract_json()
    }
}

product_kernel_declaration! {
    declaration: DuplicateMigrationDeclaration,
    owner: DuplicateMigrationOwner,
    capabilities: [
        System => StealthSystem {
            identity: "stealth.detect",
            kind: CapabilityKind::System,
            uses: CapabilityUses::SCHEDULE,
            availability: CapabilityAvailability::Linkable,
            reads: &[], writes: &[], maximum_compact_json_payload_bytes: 4096,
            owner: "tests", source: "tests/product-kernel.rs", logical_path: "system"
        }
    ],
    schemas: [
        SchemaV1 => StealthSchemaV1 { identity: "stealth.schema.v1" },
        SchemaV2 => StealthSchemaV2 { identity: "stealth.schema.v2" }
    ],
    migrations: [
        First => StealthMigrationV1ToV2 {
            identity: "stealth.migration.v1-to-v2", from: "stealth.schema.v1", to: "stealth.schema.v2"
        },
        Duplicate => StealthMigrationV1ToV2 {
            identity: "stealth.migration.v1-to-v2", from: "stealth.schema.v1", to: "stealth.schema.v2"
        }
    ]
}

product_kernel_declaration! {
    declaration: MismatchedMigrationDeclaration,
    owner: MismatchedMigrationOwner,
    capabilities: [
        System => StealthSystem {
            identity: "stealth.detect",
            kind: CapabilityKind::System,
            uses: CapabilityUses::SCHEDULE,
            availability: CapabilityAvailability::Linkable,
            reads: &[], writes: &[], maximum_compact_json_payload_bytes: 4096,
            owner: "tests", source: "tests/product-kernel.rs", logical_path: "system"
        }
    ],
    schemas: [
        SchemaV1 => StealthSchemaV1 { identity: "stealth.schema.v1" },
        SchemaV2 => StealthSchemaV2 { identity: "stealth.schema.v2" }
    ],
    migrations: [
        Mismatch => MismatchedMigration {
            identity: "stealth.migration.mismatch", from: "stealth.schema.v1", to: "stealth.schema.v2"
        }
    ]
}

product_kernel_declaration! {
    declaration: BadTargetDeclaration,
    owner: BadTargetOwner,
    capabilities: [
        Bad => BadTarget {
            identity: "bad.target",
            kind: CapabilityKind::System,
            uses: CapabilityUses::SCHEDULE,
            availability: CapabilityAvailability::Linkable,
            reads: &[], writes: &[], maximum_compact_json_payload_bytes: 4096,
            owner: "tests", source: "tests/product-kernel.rs", logical_path: "bad-target"
        }
    ], schemas: [], migrations: []
}

product_kernel_declaration! {
    declaration: BadKindDeclaration,
    owner: BadKindOwner,
    capabilities: [
        Bad => BadKind {
            identity: "bad.kind",
            kind: CapabilityKind::System,
            uses: CapabilityUses::SCHEDULE,
            availability: CapabilityAvailability::Linkable,
            reads: &[], writes: &[], maximum_compact_json_payload_bytes: 4096,
            owner: "tests", source: "tests/product-kernel.rs", logical_path: "bad-kind"
        }
    ], schemas: [], migrations: []
}

product_kernel_declaration! {
    declaration: BadMetadataDeclaration,
    owner: BadMetadataOwner,
    capabilities: [
        Bad => StealthSystem {
            identity: "bad.metadata",
            kind: CapabilityKind::System,
            uses: CapabilityUses::SCHEDULE,
            availability: CapabilityAvailability::Linkable,
            reads: &["Bad.Read"], writes: &[], maximum_compact_json_payload_bytes: 4096,
            owner: "", source: "tests/product-kernel.rs", logical_path: "bad-metadata"
        }
    ], schemas: [], migrations: []
}

product_kernel_declaration! {
    declaration: BadMigrationCapabilityDeclaration,
    owner: BadMigrationCapabilityOwner,
    capabilities: [
        Bad => BadMigrationCapability {
            identity: "bad.migration-capability",
            kind: CapabilityKind::Migration,
            uses: CapabilityUses::SCHEDULE,
            availability: CapabilityAvailability::Linkable,
            reads: &[], writes: &[], maximum_compact_json_payload_bytes: 4096,
            owner: "tests", source: "tests/product-kernel.rs", logical_path: "bad-migration"
        }
    ], schemas: [], migrations: []
}

product_kernel_declaration! {
    declaration: BadSchemaIdentityDeclaration,
    owner: BadSchemaIdentityOwner,
    capabilities: [
        Bad => BadSchemaIdentityDeclarationContract {
            identity: "bad.schema-identity",
            kind: CapabilityKind::System,
            uses: CapabilityUses::SCHEDULE,
            availability: CapabilityAvailability::Linkable,
            reads: &[], writes: &[], maximum_compact_json_payload_bytes: 4096,
            owner: "tests", source: "tests/product-kernel.rs", logical_path: "bad-schema-identity"
        }
    ],
    schemas: [Bad => StealthSchemaV1 { identity: "Bad.Schema" }],
    migrations: []
}

product_kernel_declaration! {
    declaration: BadMigrationEndpointDeclaration,
    owner: BadMigrationEndpointOwner,
    capabilities: [
        Bad => BadMigrationEndpointDeclarationContract {
            identity: "bad.migration-endpoint",
            kind: CapabilityKind::System,
            uses: CapabilityUses::SCHEDULE,
            availability: CapabilityAvailability::Linkable,
            reads: &[], writes: &[], maximum_compact_json_payload_bytes: 4096,
            owner: "tests", source: "tests/product-kernel.rs", logical_path: "bad-migration-endpoint"
        }
    ],
    schemas: [
        SchemaV1 => StealthSchemaV1 { identity: "stealth.schema.v1" },
        SchemaV2 => StealthSchemaV2 { identity: "stealth.schema.v2" }
    ],
    migrations: [
        Bad => StealthMigrationV1ToV2 {
            identity: "stealth.migration.endpoint",
            from: "Bad.Schema",
            to: "stealth.schema.v2"
        }
    ]
}

product_kernel_declaration! {
    declaration: OverlongCapabilityDeclaration,
    owner: OverlongCapabilityOwner,
    capabilities: [
        Bad => OverlongCapabilityContract {
            identity: "overlong-capability",
            kind: CapabilityKind::System,
            uses: CapabilityUses::SCHEDULE,
            availability: CapabilityAvailability::Linkable,
            reads: &[], writes: &[], maximum_compact_json_payload_bytes: 4096,
            owner: "tests", source: "tests/product-kernel.rs", logical_path: "overlong-capability"
        }
    ], schemas: [], migrations: []
}

product_kernel_declaration! {
    declaration: OverlongSchemaDeclaration,
    owner: OverlongSchemaOwner,
    capabilities: [
        Bad => StealthSystem {
            identity: "stealth.detect",
            kind: CapabilityKind::System,
            uses: CapabilityUses::SCHEDULE,
            availability: CapabilityAvailability::Linkable,
            reads: &[], writes: &[], maximum_compact_json_payload_bytes: 4096,
            owner: "tests", source: "tests/product-kernel.rs", logical_path: "overlong-schema"
        }
    ],
    schemas: [Bad => OverlongSchemaContract { identity: "overlong.schema.v1" }],
    migrations: []
}

product_kernel_declaration! {
    declaration: OverlongMigrationDeclaration,
    owner: OverlongMigrationOwner,
    capabilities: [
        Bad => StealthSystem {
            identity: "stealth.detect",
            kind: CapabilityKind::System,
            uses: CapabilityUses::SCHEDULE,
            availability: CapabilityAvailability::Linkable,
            reads: &[], writes: &[], maximum_compact_json_payload_bytes: 4096,
            owner: "tests", source: "tests/product-kernel.rs", logical_path: "overlong-migration"
        }
    ],
    schemas: [
        SchemaV1 => StealthSchemaV1 { identity: "stealth.schema.v1" },
        SchemaV2 => StealthSchemaV2 { identity: "stealth.schema.v2" }
    ],
    migrations: [
        Bad => OverlongMigrationContract {
            identity: "overlong.migration.v1-to-v2",
            from: "stealth.schema.v1",
            to: "stealth.schema.v2"
        }
    ]
}

product_kernel_declaration! {
    declaration: StealthDeclaration,
    owner: StealthOwner,
    capabilities: [
        System => StealthSystem {
            identity: "stealth.detect",
            kind: CapabilityKind::System,
            uses: CapabilityUses::SCHEDULE,
            availability: CapabilityAvailability::Linkable,
            reads: &["stealth.snapshot"],
            writes: &["stealth.observations"],
            maximum_compact_json_payload_bytes: 4096,
            owner: "stealth.product.detection",
            source: "src/detection.ts",
            logical_path: "detect",
            execution: system => stealth_system
        },
        Operation => StealthOperation {
            identity: "stealth.advance-alert",
            kind: CapabilityKind::Operation,
            uses: CapabilityUses::SCHEDULE,
            availability: CapabilityAvailability::Linkable,
            reads: &["stealth.observations"],
            writes: &["stealth.alerts"],
            maximum_compact_json_payload_bytes: 4096,
            owner: "stealth.product.alerts",
            source: "src/alerts.ts",
            logical_path: "advanceAlert",
            execution: operation => stealth_operation
        },
    ],
    schemas: [
        SchemaV1 => StealthSchemaV1 { identity: "stealth.schema.v1" },
        SchemaV2 => StealthSchemaV2 { identity: "stealth.schema.v2" }
    ],
    migrations: [
        V1ToV2 => StealthMigrationV1ToV2 {
            identity: "stealth.migration.v1-to-v2",
            from: "stealth.schema.v1",
            to: "stealth.schema.v2"
        }
    ]
}

product_kernel_declaration! {
    declaration: MixedExecutionDeclaration,
    owner: MixedExecutionOwner,
    capabilities: [
        System => MixedSystem {
            identity: "mixed.system",
            kind: CapabilityKind::System,
            uses: CapabilityUses::SCHEDULE,
            availability: CapabilityAvailability::Linkable,
            reads: &[], writes: &[], maximum_compact_json_payload_bytes: 4096,
            owner: "tests.mixed", source: "tests/product-kernel.rs", logical_path: "system",
            execution: system => mixed_system
        },
        Operation => MixedOperation {
            identity: "mixed.operation",
            kind: CapabilityKind::Operation,
            uses: CapabilityUses::TIMELINE,
            availability: CapabilityAvailability::Linkable,
            reads: &[], writes: &[], maximum_compact_json_payload_bytes: 4096,
            owner: "tests.mixed", source: "tests/product-kernel.rs", logical_path: "operation",
            execution: operation => mixed_operation
        },
        Projection => MixedProjection {
            identity: "mixed.projection",
            kind: CapabilityKind::Projection,
            uses: CapabilityUses::SCHEDULE,
            availability: CapabilityAvailability::Linkable,
            reads: &[], writes: &[], maximum_compact_json_payload_bytes: 4096,
            owner: "tests.mixed", source: "tests/product-kernel.rs", logical_path: "projection",
            execution: projection => mixed_projection
        }
    ],
    schemas: [],
    migrations: []
}

product_kernel_execution_facade! {
    declaration: MixedExecutionDeclaration,
    owner: MixedExecutionOwner,
    context: MixedExecutionContext,
    result: MixedExecutionResult,
    error: MixedExecutionError,
    capabilities: [
        System => MixedSystem {
            execution: system,
            context: System,
            result: System,
            error: System
        },
        Operation => MixedOperation {
            execution: operation,
            context: Operation,
            result: Operation,
            error: Operation
        },
        Projection => MixedProjection {
            execution: projection,
            context: Projection,
            result: Projection,
            error: Projection
        }
    ]
}

#[allow(dead_code)]
enum MixedAdapterError {
    Context(ProductKernelContextError),
    Execution(MixedExecutionError),
}

struct MixedRuntimeAdapter {
    snapshot: ExecutionSnapshot,
    request: ExecutionRequest,
}

impl MixedRuntimeAdapter {
    fn new(snapshot: u32, request: u32) -> Self {
        Self {
            snapshot: ExecutionSnapshot {
                value: Cell::new(snapshot),
            },
            request: ExecutionRequest { value: request },
        }
    }
}

impl ProductKernelRuntimeAdapter for MixedRuntimeAdapter {
    type Owner = MixedExecutionOwner;
    type Output = MixedExecutionResult;
    type Error = MixedAdapterError;

    fn dispatch_system(
        &mut self,
        owner: Self::Owner,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
    ) -> Result<Self::Output, Self::Error> {
        let context = ProductSystemContext::new(lifecycle, token, &self.snapshot, &self.request)
            .map_err(MixedAdapterError::Context)?;
        execute_system(owner, MixedExecutionContext::System(context))
            .map_err(MixedAdapterError::Execution)
    }

    fn dispatch_operation(
        &mut self,
        owner: Self::Owner,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
    ) -> Result<Self::Output, Self::Error> {
        let context = ProductOperationContext::new(lifecycle, token, &self.snapshot, &self.request)
            .map_err(MixedAdapterError::Context)?;
        execute_operation(owner, MixedExecutionContext::Operation(context))
            .map_err(MixedAdapterError::Execution)
    }

    fn dispatch_projection(
        &mut self,
        owner: Self::Owner,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
    ) -> Result<Self::Output, Self::Error> {
        let context = ProductProjectionContext::new(lifecycle, token, &self.snapshot)
            .map_err(MixedAdapterError::Context)?;
        execute_projection(owner, MixedExecutionContext::Projection(context))
            .map_err(MixedAdapterError::Execution)
    }
}

/// This is the shape a generated composition root can call: one concrete
/// adapter owns state and receives only closed owners plus lifecycle tokens.
fn generated_mixed_composition_root(
    adapter: &mut MixedRuntimeAdapter,
    lifecycle: &RuntimeLifecycle,
    phases: runtime_lifecycle::RuntimePhasePlan,
) -> Result<
    (
        MixedExecutionResult,
        MixedExecutionResult,
        MixedExecutionResult,
    ),
    MixedAdapterError,
> {
    let system =
        adapter.dispatch_system(MixedExecutionOwner::System, lifecycle, phases.schedule())?;
    let operation =
        adapter.dispatch_operation(MixedExecutionOwner::Operation, lifecycle, phases.mutation())?;
    let projection = adapter.dispatch_projection(
        MixedExecutionOwner::Projection,
        lifecycle,
        phases.projection(),
    )?;
    Ok((system, operation, projection))
}

fn manifest() -> product_model::ProductManifest {
    decode_product_manifest(
        r#"[product]
id = "stealth"

[runtime_composition]
entrypoints = ["rules/main.ts"]

[lifecycle]
mode = "demand"

[kernel]
entry = "kernel/lib.rs"

[ui]
entry = "ui/main.ts"

[content]
root = "content"

[outputs]
compiled_composition = "generated/composition.json"
admitted_runtime_content = "generated/runtime-content"
product_assembly = "generated/product-assembly"
product_bundle = "generated/product-bundle"
"#,
    )
    .expect("manifest")
}

fn composition(system_kind: CapabilityKind) -> product_model::CompiledComposition {
    let system_kind = system_kind.as_str();
    let json = r#"{
            "product":"stealth",
            "intentDescriptors":[],
            "inputMap":[],
            "schedule":[
                {"phase":"input","mode":"append","systems":[]},
                {"phase":"simulation","mode":"append","systems":[
                    {"id":"observe-pairs","capability":"observe-pairs","after":[],"reads":["entity-state.components","entity-state.transforms","engine-spatial.occlusion"],"writes":["runtime-mutation.operations"],"cadence":{"everySteps":1,"offsetSteps":0},"payload":{"kind":"engine.runtime.observe-pairs.v1","observerRole":"stealth.observer","targetRole":"stealth.target","operationBinding":"apply-operation","operationType":"engine.runtime.observe-pairs.result.v1","quotas":{"observers":64,"targets":256,"pairs":1024,"aggregates":256}}},
                    {"id":"sense","capability":"sense-system","after":["observe-pairs"],"reads":["stealth.snapshot"],"writes":["stealth.observations"],"cadence":{"everySteps":1,"offsetSteps":0},"payload":{}}
                ]},
                {"phase":"consequences","mode":"append","systems":[]},
                {"phase":"commit","mode":"append","systems":[]},
                {"phase":"projection","mode":"append","systems":[]}
            ],
            "gameplayDefinitions":[],
            "timelines":[],
            "capabilityBindings":[
                {"id":"observe-pairs","target":"engine.runtime.observe-pairs"},
                {"id":"sense-system","target":"kernel.stealth.detect"},
                {"id":"apply-operation","target":"kernel.stealth.advance-alert"}
            ]
        }"#
        .to_owned();
    let mut value: Value = serde_json::from_str(&json).expect("composition json");
    if system_kind == "migration" {
        value["schedule"][1]["systems"][0]["capability"] = Value::String("migrate".to_owned());
        value["capabilityBindings"]
            .as_array_mut()
            .expect("capability bindings")
            .push(json!({"id":"migrate","target":"kernel.stealth.migration"}));
    }
    decode_compiled_composition(serde_json::to_vec(&value).unwrap().as_slice())
        .expect("composition")
}

fn mixed_linked() -> product_model::LinkedProductComposition {
    mixed_linked_at(product_model::SchedulePhase::Simulation)
}

fn mixed_linked_at(
    system_phase: product_model::SchedulePhase,
) -> product_model::LinkedProductComposition {
    let mut value = json!({
        "product": "stealth",
        "intentDescriptors": [],
        "inputMap": [],
        "schedule": [
            {"phase": "input", "mode": "append", "systems": []},
            {"phase": "simulation", "mode": "append", "systems": []},
            {"phase": "consequences", "mode": "append", "systems": []},
            {"phase": "commit", "mode": "append", "systems": []},
            {"phase": "projection", "mode": "append", "systems": [
                {"id": "mixed-projection", "capability": "mixed-projection", "after": [], "reads": [], "writes": [], "cadence": {"everySteps": 1, "offsetSteps": 0}, "payload": null}
            ]}
        ],
        "gameplayDefinitions": [],
        "timelines": [{"id": "mixed-timeline", "steps": [
            {"id": "mixed-operation", "capability": "mixed-operation", "payload": null}
        ]}],
        "capabilityBindings": [
            {"id": "mixed-system", "target": "kernel.mixed.system"},
            {"id": "mixed-operation", "target": "kernel.mixed.operation"},
            {"id": "mixed-projection", "target": "kernel.mixed.projection"}
        ]
    });
    value["schedule"][system_phase.index()]["systems"] = json!([
        {"id": "mixed-system", "capability": "mixed-system", "after": [], "reads": [], "writes": [], "cadence": {"everySteps": 1, "offsetSteps": 0}, "payload": null}
    ]);
    let composition =
        decode_compiled_composition(&serde_json::to_vec(&value).expect("mixed composition JSON"))
            .expect("mixed composition");
    let admitted = admit_checked_product_composition(&manifest(), composition).expect("admit");
    product_model::link_admitted_product_composition(
        admitted,
        MixedExecutionDeclaration::descriptors(),
    )
    .expect("mixed Product Kernel linkage")
}

fn assembly(
    composition: product_model::CompiledComposition,
    selections: &[ProductKernelSelection<StealthOwner>],
) -> Result<ProductAssembly<StealthDeclaration>, ProductAssemblyError> {
    let admitted = admit_checked_product_composition(&manifest(), composition).expect("admission");
    ProductAssembly::<StealthDeclaration>::link(admitted, selections)
}

fn stealth_system(
    context: ProductSystemContext<'_, Snapshot, Request>,
) -> Result<u32, &'static str> {
    Ok(context.snapshot().value + context.request().value)
}

fn stealth_operation(
    context: ProductOperationContext<'_, Snapshot, Request>,
) -> Result<u32, &'static str> {
    Ok(context.snapshot().value * context.request().value)
}

fn invoke_system_owner(
    owner: StealthOwner,
    lifecycle: &RuntimeLifecycle,
    token: RuntimePhaseToken,
    snapshot: &Snapshot,
    request: &Request,
) -> Result<u32, &'static str> {
    match owner {
        StealthOwner::System => stealth_system(
            ProductSystemContext::new(lifecycle, token, snapshot, request)
                .map_err(|_| "invalid system context")?,
        ),
        StealthOwner::Operation => Err("operation owner cannot run as a system"),
    }
}

fn invoke_operation_owner(
    owner: StealthOwner,
    lifecycle: &RuntimeLifecycle,
    token: RuntimePhaseToken,
    snapshot: &Snapshot,
    request: &Request,
) -> Result<u32, &'static str> {
    match owner {
        StealthOwner::System => Err("system owner cannot publish as an operation"),
        StealthOwner::Operation => stealth_operation(
            ProductOperationContext::new(lifecycle, token, snapshot, request)
                .map_err(|_| "invalid operation context")?,
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StealthAuthority {
    value: u32,
}

impl MutationAuthority for StealthAuthority {
    type Guard = u32;

    fn guard(&self) -> Self::Guard {
        self.value
    }

    fn publication_domain(&self) -> &str {
        "stealth.world"
    }
}

struct StealthMutationPlanner {
    fail: bool,
}

impl MutationPlanner<StealthAuthority, u32> for StealthMutationPlanner {
    type Error = &'static str;

    fn stage(
        &mut self,
        authority: &StealthAuthority,
        batch: &MutationResolvedBatch,
    ) -> Result<MutationStage<StealthAuthority, u32>, Self::Error> {
        if self.fail {
            return Err("typed planner rejected operation");
        }
        let candidate = StealthAuthority {
            value: authority
                .value
                .checked_add(batch.operations().len() as u32)
                .ok_or("typed planner overflow")?,
        };
        let evidence = batch
            .operations()
            .iter()
            .map(|operation| MutationOwnerEvidence::for_operation(operation, 7))
            .collect();
        Ok(MutationStage::new(candidate, evidence))
    }
}

#[test]
fn declaration_is_typed_closed_and_reorder_stable() {
    assert_eq!(StealthOwner::System.target(), "kernel.stealth.detect");
    assert_eq!(StealthOwner::Operation.kind(), CapabilityKind::Operation);
    assert_eq!(StealthOwner::all().len(), 2);
    assert_eq!(StealthOwner::System.entry().identity(), "stealth.detect");
    assert_eq!(StealthDeclaration::descriptors().len(), 2);
    assert_eq!(StealthDeclaration::schemas().len(), 2);
    assert_eq!(StealthDeclaration::migrations().len(), 1);
    let first = StealthDeclaration::contract_json().expect("validated contract JSON");
    let mut reordered = StealthDeclaration::entries().to_vec();
    reordered.reverse();
    assert_eq!(
        first,
        render_contract_json_unchecked(
            &reordered,
            StealthDeclaration::schemas(),
            StealthDeclaration::migrations(),
        )
    );
    let typescript = StealthDeclaration::contract_typescript().expect("validated contract TS");
    assert_eq!(
        typescript,
        include_str!("../../../../rules/packages/runtime-composition-authoring/src/product-kernel-rendered.fixture.ts")
    );
    assert!(!typescript.contains("version"));
    let mut schemas = StealthDeclaration::schemas().to_vec();
    schemas.reverse();
    let mut migrations = StealthDeclaration::migrations().to_vec();
    migrations.reverse();
    assert_eq!(
        typescript,
        render_contract_typescript_unchecked(&reordered, &schemas, &migrations)
    );
}

#[test]
fn standard_and_product_lanes_share_one_typed_assembly_and_mutation_boundary() {
    let product_assembly = assembly(
        composition(CapabilityKind::System),
        &[
            StealthOwner::System.selection("sense-system"),
            StealthOwner::Operation.selection("apply-operation"),
        ],
    )
    .expect("standard and Product Kernel bindings link together");
    let linked = product_assembly.linked();
    assert!(linked
        .capability_bindings()
        .iter()
        .any(|binding| binding.target() == "engine.runtime.observe-pairs"));
    assert!(linked
        .capability_bindings()
        .iter()
        .any(|binding| binding.target() == "kernel.stealth.detect"));
    assert!(linked
        .capability_bindings()
        .iter()
        .any(|binding| binding.target() == "kernel.stealth.advance-alert"));

    let schedule = CompiledRuntimeSchedule::compile(linked).expect("shared schedule compiles");
    let mutations = CompiledMutationCatalog::compile(
        linked,
        &[MutationCapabilityDescriptor::new(
            "apply-operation",
            "kernel.stealth.advance-alert",
            "stealth.world",
            "stealth.product.operation",
            OBSERVE_PAIRS_RESULT_KIND,
        )],
    )
    .expect("shared mutation catalog compiles");
    let simulation = schedule
        .phase(product_model::SchedulePhase::Simulation)
        .systems();
    assert_eq!(simulation.len(), 2);
    let observe = simulation
        .iter()
        .find(|system| system.id() == "observe-pairs")
        .expect("standard system retained");
    let observe_plan = ObservePairsPlan::compile(linked, observe, &mutations)
        .expect("standard system selects Product Kernel operation");
    assert_eq!(observe_plan.operation_binding(), "apply-operation");
    let product_system = simulation
        .iter()
        .find(|system| system.id() == "sense")
        .expect("novel Product Kernel system retained");
    assert_eq!(
        product_system.capability().target(),
        "kernel.stealth.detect"
    );

    let mut lifecycle =
        RuntimeLifecycle::new(RuntimeInstanceId::new(33), RuntimeLifecycleConfig::Demand);
    lifecycle.start().expect("start lifecycle");
    let mut mutation = RuntimeMutation::<StealthAuthority, u32>::bind(mutations, &lifecycle)
        .expect("bind mutation lane before first step");
    let admission = lifecycle.admit_demand_step().expect("admit one step");
    let phases = admission.step_at(0).expect("step phases").phases();
    let snapshot = Snapshot { value: 3 };
    let request = Request { value: 4 };
    let system_value = invoke_system_owner(
        StealthOwner::System,
        &lifecycle,
        phases.schedule(),
        &snapshot,
        &request,
    )
    .expect("closed Product Kernel system call");
    let operation_value = invoke_operation_owner(
        StealthOwner::Operation,
        &lifecycle,
        phases.mutation(),
        &snapshot,
        &request,
    )
    .expect("closed Product Kernel operation call");
    assert_eq!(system_value, 7);
    assert_eq!(operation_value, 12);

    let operation = MutationOperation::new(
        MutationOperationId::new(1),
        "apply-operation",
        "kernel.stealth.advance-alert",
        json!({
            "kind": OBSERVE_PAIRS_RESULT_KIND,
            "value": operation_value,
        }),
    )
    .expect("typed operation payload");
    let batch = MutationBatch::new(
        MutationBatchId::new("stealth-operation-step-0").expect("batch id"),
        MutationCausation::new("stealth.system").expect("causation"),
        MutationProvenance::new("stealth.product.operation").expect("provenance"),
        vec![operation],
    )
    .expect("mutation batch");
    let mut authority = StealthAuthority { value: 10 };
    let before = authority.clone();
    let mut failing_planner = StealthMutationPlanner { fail: true };
    assert!(matches!(
        mutation.apply_batch(
            &lifecycle,
            phases.mutation(),
            &mut authority,
            &mut failing_planner,
            batch.clone(),
        ),
        Err(runtime_mutation::RuntimeMutationError::Planner(
            "typed planner rejected operation"
        ))
    ));
    assert_eq!(authority, before);
    assert_eq!(mutation.readout().last_completed_step(), None);

    let mut planner = StealthMutationPlanner { fail: false };
    let receipt = mutation
        .apply_batch(
            &lifecycle,
            phases.mutation(),
            &mut authority,
            &mut planner,
            batch.clone(),
        )
        .expect("operation publishes through runtime mutation");
    assert_eq!(authority.value, 11);
    assert_eq!(receipt.batch_id().as_str(), "stealth-operation-step-0");
    assert_eq!(receipt.operations()[0].binding_id(), "apply-operation");
    let replay = mutation
        .apply_batch(
            &lifecycle,
            phases.mutation(),
            &mut authority,
            &mut planner,
            batch,
        )
        .expect("same batch is an exact receipt replay");
    assert_eq!(replay.batch_id(), receipt.batch_id());
    assert_eq!(replay.batch_fingerprint(), receipt.batch_fingerprint());
    assert_eq!(authority.value, 11);
}

#[test]
fn assembly_rejects_missing_and_wrong_contract_type_before_lifecycle() {
    let missing = assembly(
        composition(CapabilityKind::System),
        &[StealthOwner::System.selection("sense-system")],
    );
    assert!(matches!(
        missing,
        Err(ProductAssemblyError::MissingSelection { ref binding_id, .. })
            if binding_id == "apply-operation"
    ));

    let wrong_type = assembly(
        composition(CapabilityKind::System),
        &[
            StealthOwner::System.selection("sense-system"),
            ProductKernelSelection::new(
                "apply-operation",
                StealthOwner::Operation,
                "stale.operation.v0",
            ),
        ],
    );
    assert!(matches!(
        wrong_type,
        Err(ProductAssemblyError::SelectionTypeMismatch { ref binding_id, .. })
            if binding_id == "apply-operation"
    ));
}

#[test]
fn assembly_rejects_kind_and_migration_schedule_selection_before_lifecycle() {
    let wrong_kind = assembly(
        composition(CapabilityKind::System),
        &[
            ProductKernelSelection::new(
                "sense-system",
                StealthOwner::Operation,
                StealthOwner::Operation.contract_type(),
            ),
            StealthOwner::Operation.selection("apply-operation"),
        ],
    );
    assert!(matches!(
        wrong_kind,
        Err(ProductAssemblyError::SelectionTargetMismatch { ref binding_id, .. })
            if binding_id == "sense-system"
    ));

    let migration = assembly(
        composition(CapabilityKind::Migration),
        &[
            StealthOwner::System.selection("sense-system"),
            StealthOwner::Operation.selection("apply-operation"),
        ],
    );
    assert!(matches!(
        migration,
        Err(ProductAssemblyError::ProductModel(ref error))
            if error.diagnostic().code() == "RUNTIME_CAPABILITY_UNKNOWN_KERNEL_TARGET"
    ));
}

#[test]
fn declaration_rejects_schema_migration_and_live_kind_drift() {
    assert!(matches!(
        validate_declaration::<MissingSchemaDeclaration>(),
        Err(ProductAssemblyError::Declaration(
            DeclarationError::MissingMigrationSchema { .. }
        ))
    ));
    assert!(matches!(
        validate_declaration::<DuplicateSchemaDeclaration>(),
        Err(ProductAssemblyError::Declaration(
            DeclarationError::DuplicateSchema { .. }
        ))
    ));
    assert!(matches!(
        validate_declaration::<DuplicateMigrationDeclaration>(),
        Err(ProductAssemblyError::Declaration(
            DeclarationError::DuplicateMigration { .. }
        ))
    ));
    assert!(matches!(
        validate_declaration::<MismatchedMigrationDeclaration>(),
        Err(ProductAssemblyError::Declaration(
            DeclarationError::MigrationFromMismatch { .. }
        ))
    ));
    assert!(matches!(
        validate_declaration::<BadTargetDeclaration>(),
        Err(ProductAssemblyError::Declaration(
            DeclarationError::ContractTargetMismatch { .. }
        ))
    ));
    assert!(matches!(
        validate_declaration::<BadKindDeclaration>(),
        Err(ProductAssemblyError::Declaration(
            DeclarationError::ContractKindMismatch { .. }
        ))
    ));
    assert!(matches!(
        validate_declaration::<BadMetadataDeclaration>(),
        Err(ProductAssemblyError::Declaration(
            DeclarationError::InvalidIdentity { .. }
                | DeclarationError::InvalidContractText { .. }
                | DeclarationError::InvalidMetadata { .. }
        ))
    ));
    assert!(matches!(
        validate_declaration::<BadMigrationCapabilityDeclaration>(),
        Err(ProductAssemblyError::Declaration(
            DeclarationError::MigrationCapability { .. }
        ))
    ));
    assert!(matches!(
        validate_declaration::<BadSchemaIdentityDeclaration>(),
        Err(ProductAssemblyError::Declaration(
            DeclarationError::InvalidIdentity { .. }
        ))
    ));
    assert!(matches!(
        validate_declaration::<BadMigrationEndpointDeclaration>(),
        Err(ProductAssemblyError::Declaration(
            DeclarationError::InvalidIdentity { .. }
        ))
    ));
    assert!(matches!(
        validate_declaration::<OverlongCapabilityDeclaration>(),
        Err(ProductAssemblyError::Declaration(
            DeclarationError::InvalidContractText { .. }
        ))
    ));
    assert!(matches!(
        validate_declaration::<OverlongSchemaDeclaration>(),
        Err(ProductAssemblyError::Declaration(
            DeclarationError::InvalidContractText { .. }
        ))
    ));
    assert!(matches!(
        validate_declaration::<OverlongMigrationDeclaration>(),
        Err(ProductAssemblyError::Declaration(
            DeclarationError::InvalidContractText { .. }
        ))
    ));
    for result in [
        BadTargetDeclaration::contract_json(),
        BadKindDeclaration::contract_json(),
        BadMetadataDeclaration::contract_json(),
        BadSchemaIdentityDeclaration::contract_json(),
        BadMigrationEndpointDeclaration::contract_json(),
        OverlongCapabilityDeclaration::contract_json(),
        OverlongSchemaDeclaration::contract_json(),
        OverlongMigrationDeclaration::contract_json(),
    ] {
        assert!(result.is_err(), "invalid declaration rendered JSON");
    }
    for result in [
        BadTargetDeclaration::contract_typescript(),
        BadKindDeclaration::contract_typescript(),
        BadMetadataDeclaration::contract_typescript(),
        BadSchemaIdentityDeclaration::contract_typescript(),
        BadMigrationEndpointDeclaration::contract_typescript(),
        OverlongCapabilityDeclaration::contract_typescript(),
        OverlongSchemaDeclaration::contract_typescript(),
        OverlongMigrationDeclaration::contract_typescript(),
    ] {
        assert!(result.is_err(), "invalid declaration rendered TypeScript");
    }
}

#[test]
fn contexts_validate_their_own_phase_tokens_and_keep_closed_types() {
    let mut lifecycle =
        RuntimeLifecycle::new(RuntimeInstanceId::new(9), RuntimeLifecycleConfig::Demand);
    lifecycle.start().expect("start");
    let admission = lifecycle.admit_demand_step().expect("step");
    let token = admission.step_at(0).expect("phase").phases();
    let snapshot = Snapshot { value: 3 };
    let request = Request { value: 4 };
    let system = ProductSystemContext::new(&lifecycle, token.schedule(), &snapshot, &request)
        .expect("schedule context");
    assert_eq!(system.snapshot().value + system.request().value, 7);
    let operation = ProductOperationContext::new(&lifecycle, token.mutation(), &snapshot, &request)
        .expect("mutation context");
    assert_eq!(operation.step().value(), 0);
    let projection = ProductProjectionContext::new(&lifecycle, token.projection(), &snapshot)
        .expect("projection context");
    assert_eq!(projection.snapshot().value, 3);
    assert_eq!(projection.step().value(), 0);
    assert!(matches!(
        ProductOperationContext::new(&lifecycle, token.schedule(), &snapshot, &request),
        Err(ProductKernelContextError::WrongPhase {
            expected: RuntimePhase::Mutation,
            received: RuntimePhase::Schedule
        })
    ));
    assert!(matches!(
        ProductProjectionContext::new(&lifecycle, token.mutation(), &snapshot),
        Err(ProductKernelContextError::WrongPhase {
            expected: RuntimePhase::Projection,
            received: RuntimePhase::Mutation
        })
    ));
}

#[test]
fn mixed_execution_links_call_concrete_functions_with_typed_contexts() {
    validate_product_kernel_execution_declaration::<MixedExecutionDeclaration>()
        .expect("mixed declaration has one typed link per executable kind");
    assert_eq!(MixedExecutionDeclaration::execution_links().len(), 3);
    assert_eq!(
        MixedExecutionDeclaration::execution_links()[1].function_path(),
        "mixed_operation"
    );

    let mut lifecycle =
        RuntimeLifecycle::new(RuntimeInstanceId::new(44), RuntimeLifecycleConfig::Demand);
    lifecycle.start().expect("start");
    let admission = lifecycle.admit_demand_step().expect("step");
    let phases = admission.step_at(0).expect("phase").phases();
    let snapshot = ExecutionSnapshot {
        value: Cell::new(3),
    };
    let request = ExecutionRequest { value: 4 };

    let system = <MixedSystem as ProductKernelSystemExecutor>::execute_system(
        ProductSystemContext::new(&lifecycle, phases.schedule(), &snapshot, &request)
            .expect("typed system context"),
    )
    .expect("system result");
    assert_eq!(system, 7);
    assert_eq!(snapshot.value.get(), 7);

    let operation = <MixedOperation as ProductKernelOperationExecutor>::execute_operation(
        ProductOperationContext::new(&lifecycle, phases.mutation(), &snapshot, &request)
            .expect("typed operation context"),
    )
    .expect("operation result");
    assert_eq!(operation, "alert:28");
    assert_eq!(snapshot.value.get(), 28);

    let projection = <MixedProjection as ProductKernelProjectionExecutor>::execute_projection(
        ProductProjectionContext::new(&lifecycle, phases.projection(), &snapshot)
            .expect("typed projection context"),
    )
    .expect("projection result");
    assert!(projection);
}

#[test]
fn generated_execution_facade_routes_mixed_contexts_results_and_effects() {
    validate().expect("generated facade is backed by validated static links");

    let mut lifecycle =
        RuntimeLifecycle::new(RuntimeInstanceId::new(45), RuntimeLifecycleConfig::Demand);
    lifecycle.start().expect("start");
    let admission = lifecycle.admit_demand_step().expect("step");
    let phases = admission.step_at(0).expect("phase").phases();
    let snapshot = ExecutionSnapshot {
        value: Cell::new(2),
    };
    let request = ExecutionRequest { value: 5 };

    let system = execute_system(
        MixedExecutionOwner::System,
        MixedExecutionContext::System(
            ProductSystemContext::new(&lifecycle, phases.schedule(), &snapshot, &request)
                .expect("system context"),
        ),
    )
    .unwrap_or_else(|_| panic!("system route"));
    assert!(matches!(system, MixedExecutionResult::System(7)));
    assert_eq!(snapshot.value.get(), 7);

    let operation = execute_operation(
        MixedExecutionOwner::Operation,
        MixedExecutionContext::Operation(
            ProductOperationContext::new(&lifecycle, phases.mutation(), &snapshot, &request)
                .expect("operation context"),
        ),
    )
    .unwrap_or_else(|_| panic!("operation route"));
    assert!(matches!(operation, MixedExecutionResult::Operation(value) if value == "alert:35"));
    assert_eq!(snapshot.value.get(), 35);

    let projection = execute_projection(
        MixedExecutionOwner::Projection,
        MixedExecutionContext::Projection(
            ProductProjectionContext::new(&lifecycle, phases.projection(), &snapshot)
                .expect("projection context"),
        ),
    )
    .unwrap_or_else(|_| panic!("projection route"));
    assert!(matches!(projection, MixedExecutionResult::Projection(true)));

    let wrong_context = execute_system(
        MixedExecutionOwner::System,
        MixedExecutionContext::Operation(
            ProductOperationContext::new(&lifecycle, phases.mutation(), &snapshot, &request)
                .expect("operation context"),
        ),
    );
    assert!(matches!(
        wrong_context,
        Err(MixedExecutionError::WrongContextKind {
            expected: CapabilityKind::System,
            received: CapabilityKind::Operation
        })
    ));

    let wrong_owner = execute_system(
        MixedExecutionOwner::Operation,
        MixedExecutionContext::System(
            ProductSystemContext::new(&lifecycle, phases.schedule(), &snapshot, &request)
                .expect("system context"),
        ),
    );
    assert!(matches!(
        wrong_owner,
        Err(MixedExecutionError::WrongOwnerKind {
            expected: CapabilityKind::System,
            received: CapabilityKind::Operation
        })
    ));
}

#[test]
fn generated_composition_root_calls_one_concrete_runtime_adapter() {
    let mut lifecycle =
        RuntimeLifecycle::new(RuntimeInstanceId::new(46), RuntimeLifecycleConfig::Demand);
    lifecycle.start().expect("start");
    let admission = lifecycle.admit_demand_step().expect("step");
    let phases = admission.step_at(0).expect("phase").phases();
    let mut adapter = MixedRuntimeAdapter::new(2, 5);

    let (system, operation, projection) =
        generated_mixed_composition_root(&mut adapter, &lifecycle, phases)
            .unwrap_or_else(|_| panic!("generated adapter dispatch"));
    assert!(matches!(system, MixedExecutionResult::System(7)));
    assert!(matches!(
        operation,
        MixedExecutionResult::Operation(value) if value == "alert:35"
    ));
    assert!(matches!(projection, MixedExecutionResult::Projection(true)));
    assert_eq!(adapter.snapshot.value.get(), 35);

    let wrong_phase =
        adapter.dispatch_system(MixedExecutionOwner::System, &lifecycle, phases.mutation());
    assert!(matches!(
        wrong_phase,
        Err(MixedAdapterError::Context(
            ProductKernelContextError::WrongPhase {
                expected: RuntimePhase::Schedule,
                received: RuntimePhase::Mutation
            }
        ))
    ));
}

#[test]
fn execution_validation_covers_live_schedule_and_renders_direct_arms() {
    let linked = mixed_linked();
    validate_product_kernel_execution::<MixedExecutionDeclaration>(&linked)
        .expect("every live mixed capability has a matching executor");
    let arms = render_product_kernel_execution_arms::<MixedExecutionDeclaration>(&linked)
        .expect("closed direct source arms");
    assert!(
        arms.contains("\"kernel.mixed.system\" => product_kernel::mixed_system(system_context)")
    );
    assert!(arms.contains(
        "\"kernel.mixed.operation\" => product_kernel::mixed_operation(operation_context)"
    ));
    assert!(arms.contains(
        "\"kernel.mixed.projection\" => product_kernel::mixed_projection(projection_context)"
    ));
}

#[test]
fn execution_validation_rejects_systems_without_a_typed_schedule_context() {
    let linked = mixed_linked_at(product_model::SchedulePhase::Input);
    assert!(matches!(
        validate_product_kernel_execution::<MixedExecutionDeclaration>(&linked),
        Err(ProductKernelExecutionError::UnsupportedSystemPhase {
            target,
            phase: product_model::SchedulePhase::Input,
            path
        }) if target == "kernel.mixed.system" && path == "schedule[0].systems[0]"
    ));
}

#[test]
fn execution_validation_rejects_missing_type_and_target_links() {
    let stealth_admitted =
        admit_checked_product_composition(&manifest(), composition(CapabilityKind::System))
            .expect("admit stealth composition");
    let stealth_linked = product_model::link_admitted_product_composition(
        stealth_admitted,
        StealthDeclaration::descriptors(),
    )
    .expect("link stealth composition");
    assert!(matches!(
        validate_product_kernel_execution::<MissingSchemaDeclaration>(&stealth_linked),
        Err(ProductKernelExecutionError::MissingExecutableLink { target, .. })
            if target == "kernel.stealth.detect"
    ));
    assert!(matches!(
        validate_product_kernel_execution_declaration::<BadExecutionDeclaration>(),
        Err(ProductKernelExecutionError::ContractTypeMismatch { target, .. })
            if target == "kernel.stealth.detect"
    ));
    assert!(matches!(
        validate_product_kernel_execution_declaration::<BadTargetExecutionDeclaration>(),
        Err(ProductKernelExecutionError::InvalidTarget { target })
            if target == "kernel.wrong-target"
    ));
    assert!(matches!(
        validate_product_kernel_execution_declaration::<BadSourceExecutionDeclaration>(),
        Err(ProductKernelExecutionError::InvalidSourceFragment {
            field: "function_path"
        })
    ));
}
