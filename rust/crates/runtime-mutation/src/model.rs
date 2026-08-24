use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt;

/// Maximum number of operations in one admitted Mutation batch.
pub const MAX_MUTATION_BATCH_OPERATIONS: usize = product_model::MAX_SCHEDULE_ENTRIES;
/// Maximum bytes in a batch identity.
pub const MAX_MUTATION_BATCH_ID_BYTES: usize = product_model::MAX_IDENTITY_BYTES;
/// Maximum bytes in a causation identity.
pub const MAX_MUTATION_CAUSATION_BYTES: usize = product_model::MAX_IDENTITY_BYTES;
/// Maximum bytes in a provenance identity.
pub const MAX_MUTATION_PROVENANCE_BYTES: usize = product_model::MAX_IDENTITY_BYTES;
/// A defensive upper bound before a selected capability's smaller budget is
/// applied. This is a runtime envelope bound, not a generic write protocol.
pub const MAX_MUTATION_PAYLOAD_BYTES: usize = 16 * 1024;
/// Maximum retained applied receipts in one instance-owned lane.
pub const MAX_MUTATION_RECEIPTS: usize = 32;

/// Deterministic SHA-256 fingerprint over the compact Rust JSON batch
/// envelope. This is a local identity/readback aid, not a cross-language
/// canonicalization protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MutationFingerprint([u8; 32]);

impl MutationFingerprint {
    pub(crate) fn from_batch(
        id: &MutationBatchId,
        causation: &MutationCausation,
        provenance: &MutationProvenance,
        operations: &[MutationOperation],
    ) -> Result<Self, MutationDataError> {
        let envelope = MutationFingerprintEnvelope {
            id: id.as_str(),
            causation: causation.as_str(),
            provenance: provenance.as_str(),
            operations: operations
                .iter()
                .map(MutationFingerprintOperation::from_operation)
                .collect(),
        };
        let bytes =
            serde_json::to_vec(&envelope).map_err(|_| MutationDataError::FingerprintEncoding)?;
        let digest = Sha256::digest(bytes);
        Ok(Self(digest.into()))
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Identity of one compiled Product Assembly mutation catalog.
///
/// Like [`MutationFingerprint`], this is a local compact-Rust-JSON identity.
/// It is useful for receipt correlation and conflict diagnostics; it is not a
/// cross-language canonical schema or a version protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MutationCatalogIdentity([u8; 32]);

impl MutationCatalogIdentity {
    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MutationFingerprintEnvelope<'a> {
    id: &'a str,
    causation: &'a str,
    provenance: &'a str,
    operations: Vec<MutationFingerprintOperation<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MutationFingerprintOperation<'a> {
    id: u64,
    binding_id: &'a str,
    target: &'a str,
    payload: &'a Value,
}

impl<'a> MutationFingerprintOperation<'a> {
    fn from_operation(operation: &'a MutationOperation) -> Self {
        Self {
            id: operation.id().value(),
            binding_id: operation.binding_id(),
            target: operation.target(),
            payload: operation.payload(),
        }
    }
}

/// Caller-chosen identity for one admitted batch.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MutationBatchId(String);

impl MutationBatchId {
    pub fn new(value: impl Into<String>) -> Result<Self, MutationDataError> {
        let value = value.into();
        validate_identity(&value, MAX_MUTATION_BATCH_ID_BYTES, "batch id")?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Causation identity retained as data in the applied receipt.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MutationCausation(String);

impl MutationCausation {
    pub fn new(value: impl Into<String>) -> Result<Self, MutationDataError> {
        let value = value.into();
        validate_identity(&value, MAX_MUTATION_CAUSATION_BYTES, "causation")?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Product-owned provenance identity retained as data in the applied receipt.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MutationProvenance(String);

impl MutationProvenance {
    pub fn new(value: impl Into<String>) -> Result<Self, MutationDataError> {
        let value = value.into();
        validate_identity(&value, MAX_MUTATION_PROVENANCE_BYTES, "provenance")?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable caller operation identity within one batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MutationOperationId(u64);

impl MutationOperationId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// One capability-specific operation request.
///
/// `binding_id`, `target`, and `payload` must agree with the closed Product
/// Assembly selection. The mutation lane treats the payload as bounded data;
/// only the caller's closed planner interprets it.
#[derive(Debug, Clone, PartialEq)]
pub struct MutationOperation {
    id: MutationOperationId,
    binding_id: String,
    target: String,
    payload: Value,
}

impl MutationOperation {
    pub fn new(
        id: MutationOperationId,
        binding_id: impl Into<String>,
        target: impl Into<String>,
        payload: Value,
    ) -> Result<Self, MutationDataError> {
        let binding_id = binding_id.into();
        let target = target.into();
        validate_identity(&binding_id, product_model::MAX_IDENTITY_BYTES, "binding id")?;
        validate_identity(&target, product_model::MAX_IDENTITY_BYTES, "target")?;
        validate_payload(&payload)?;
        Ok(Self {
            id,
            binding_id,
            target,
            payload,
        })
    }

    pub const fn id(&self) -> MutationOperationId {
        self.id
    }

    pub fn binding_id(&self) -> &str {
        &self.binding_id
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

/// One ordered, nonempty batch admitted to the Mutation lane.
#[derive(Debug, Clone, PartialEq)]
pub struct MutationBatch {
    id: MutationBatchId,
    causation: MutationCausation,
    provenance: MutationProvenance,
    operations: Vec<MutationOperation>,
    fingerprint: MutationFingerprint,
}

impl MutationBatch {
    pub fn new(
        id: MutationBatchId,
        causation: MutationCausation,
        provenance: MutationProvenance,
        operations: Vec<MutationOperation>,
    ) -> Result<Self, MutationDataError> {
        if operations.is_empty() {
            return Err(MutationDataError::EmptyOperations);
        }
        if operations.len() > MAX_MUTATION_BATCH_OPERATIONS {
            return Err(MutationDataError::TooManyOperations {
                received: operations.len(),
                maximum: MAX_MUTATION_BATCH_OPERATIONS,
            });
        }
        for operation in &operations {
            validate_payload(operation.payload())?;
        }
        let fingerprint =
            MutationFingerprint::from_batch(&id, &causation, &provenance, &operations)?;
        Ok(Self {
            id,
            causation,
            provenance,
            operations,
            fingerprint,
        })
    }

    pub fn id(&self) -> &MutationBatchId {
        &self.id
    }

    pub fn causation(&self) -> &MutationCausation {
        &self.causation
    }

    pub fn provenance(&self) -> &MutationProvenance {
        &self.provenance
    }

    pub fn operations(&self) -> &[MutationOperation] {
        &self.operations
    }

    pub const fn fingerprint(&self) -> MutationFingerprint {
        self.fingerprint
    }
}

/// A resolved operation supplied to the closed Product Assembly planner.
///
/// All identity, linkage, kind, budget, owner, and publication facts have
/// already been validated. The type is constructed only by the static catalog
/// and mutation preflight path.
#[derive(Debug, Clone, PartialEq)]
pub struct MutationResolvedOperation {
    index: usize,
    id: MutationOperationId,
    binding_index: usize,
    binding_id: String,
    target: String,
    resolved_target: String,
    kind: String,
    publication_domain: String,
    owner: String,
    provenance_source: String,
    provenance_path: String,
    payload: Value,
}

pub(crate) struct MutationResolvedMetadata {
    pub(crate) binding_index: usize,
    pub(crate) resolved_target: String,
    pub(crate) kind: String,
    pub(crate) publication_domain: String,
    pub(crate) owner: String,
    pub(crate) provenance_source: String,
    pub(crate) provenance_path: String,
}

impl MutationResolvedOperation {
    pub(crate) fn new(
        index: usize,
        operation: &MutationOperation,
        metadata: MutationResolvedMetadata,
    ) -> Self {
        Self {
            index,
            id: operation.id(),
            binding_index: metadata.binding_index,
            binding_id: operation.binding_id().to_owned(),
            target: operation.target().to_owned(),
            resolved_target: metadata.resolved_target,
            kind: metadata.kind,
            publication_domain: metadata.publication_domain,
            owner: metadata.owner,
            provenance_source: metadata.provenance_source,
            provenance_path: metadata.provenance_path,
            payload: operation.payload().clone(),
        }
    }

    pub const fn index(&self) -> usize {
        self.index
    }

    pub const fn id(&self) -> MutationOperationId {
        self.id
    }

    pub const fn binding_index(&self) -> usize {
        self.binding_index
    }

    pub fn binding_id(&self) -> &str {
        &self.binding_id
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn resolved_target(&self) -> &str {
        &self.resolved_target
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn publication_domain(&self) -> &str {
        &self.publication_domain
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn provenance_source(&self) -> &str {
        &self.provenance_source
    }

    pub fn provenance_path(&self) -> &str {
        &self.provenance_path
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

/// A fully linked, ordered batch handed to a Product Assembly planner.
#[derive(Debug, Clone, PartialEq)]
pub struct MutationResolvedBatch {
    id: MutationBatchId,
    causation: MutationCausation,
    provenance: MutationProvenance,
    operations: Vec<MutationResolvedOperation>,
    fingerprint: MutationFingerprint,
}

impl MutationResolvedBatch {
    pub(crate) fn new(batch: &MutationBatch, operations: Vec<MutationResolvedOperation>) -> Self {
        Self {
            id: batch.id.clone(),
            causation: batch.causation.clone(),
            provenance: batch.provenance.clone(),
            operations,
            fingerprint: batch.fingerprint,
        }
    }

    pub fn id(&self) -> &MutationBatchId {
        &self.id
    }

    pub fn causation(&self) -> &MutationCausation {
        &self.causation
    }

    pub fn provenance(&self) -> &MutationProvenance {
        &self.provenance
    }

    pub fn operations(&self) -> &[MutationResolvedOperation] {
        &self.operations
    }

    pub const fn fingerprint(&self) -> MutationFingerprint {
        self.fingerprint
    }
}

/// Named-owner evidence for one staged operation. The pipeline checks the
/// operation identity, binding target, resolved target, publication domain, and
/// owner against the resolved batch in exact order before publication.
#[derive(Debug, Clone, PartialEq)]
pub struct MutationOwnerEvidence<E> {
    operation_id: MutationOperationId,
    binding_id: String,
    target: String,
    resolved_target: String,
    publication_domain: String,
    owner: String,
    evidence: E,
}

impl<E> MutationOwnerEvidence<E> {
    pub fn new(
        operation_id: MutationOperationId,
        binding_id: impl Into<String>,
        target: impl Into<String>,
        resolved_target: impl Into<String>,
        publication_domain: impl Into<String>,
        owner: impl Into<String>,
        evidence: E,
    ) -> Self {
        Self {
            operation_id,
            binding_id: binding_id.into(),
            target: target.into(),
            resolved_target: resolved_target.into(),
            publication_domain: publication_domain.into(),
            owner: owner.into(),
            evidence,
        }
    }

    pub fn for_operation(operation: &MutationResolvedOperation, evidence: E) -> Self {
        Self::new(
            operation.id(),
            operation.binding_id(),
            operation.target(),
            operation.resolved_target(),
            operation.publication_domain(),
            operation.owner(),
            evidence,
        )
    }

    pub const fn operation_id(&self) -> MutationOperationId {
        self.operation_id
    }

    pub fn binding_id(&self) -> &str {
        &self.binding_id
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn resolved_target(&self) -> &str {
        &self.resolved_target
    }

    pub fn publication_domain(&self) -> &str {
        &self.publication_domain
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn evidence(&self) -> &E {
        &self.evidence
    }

    pub fn into_evidence(self) -> E {
        self.evidence
    }
}

/// Planner output. The candidate is wholly owned by the caller until the
/// runtime lane performs its final infallible publication assignment.
#[derive(Debug, Clone, PartialEq)]
pub struct MutationStage<A, E> {
    candidate: A,
    owner_evidence: Vec<MutationOwnerEvidence<E>>,
}

impl<A, E> MutationStage<A, E> {
    pub fn new(candidate: A, owner_evidence: Vec<MutationOwnerEvidence<E>>) -> Self {
        Self {
            candidate,
            owner_evidence,
        }
    }

    pub fn candidate(&self) -> &A {
        &self.candidate
    }

    pub fn owner_evidence(&self) -> &[MutationOwnerEvidence<E>] {
        &self.owner_evidence
    }

    pub fn into_parts(self) -> (A, Vec<MutationOwnerEvidence<E>>) {
        (self.candidate, self.owner_evidence)
    }
}

/// Construction/data errors caught before a runtime lane exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationDataError {
    EmptyIdentity(&'static str),
    IdentityTooLong {
        field: &'static str,
        maximum: usize,
        received: usize,
    },
    EmptyOperations,
    TooManyOperations {
        received: usize,
        maximum: usize,
    },
    PayloadNotJson,
    FingerprintEncoding,
    PayloadStructureOutOfBounds(&'static str),
    PayloadTooLarge {
        actual: usize,
        maximum: usize,
    },
}

impl fmt::Display for MutationDataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid mutation data: {self:?}")
    }
}

impl std::error::Error for MutationDataError {}

pub(crate) fn validate_identity(
    value: &str,
    maximum: usize,
    field: &'static str,
) -> Result<(), MutationDataError> {
    if value.is_empty() {
        return Err(MutationDataError::EmptyIdentity(field));
    }
    if value.len() > maximum {
        return Err(MutationDataError::IdentityTooLong {
            field,
            maximum,
            received: value.len(),
        });
    }
    Ok(())
}

pub(crate) fn validate_payload(value: &Value) -> Result<usize, MutationDataError> {
    let bytes = serde_json::to_vec(value).map_err(|_| MutationDataError::PayloadNotJson)?;
    if bytes.len() > MAX_MUTATION_PAYLOAD_BYTES {
        return Err(MutationDataError::PayloadTooLarge {
            actual: bytes.len(),
            maximum: MAX_MUTATION_PAYLOAD_BYTES,
        });
    }
    let mut nodes = 0;
    validate_value_shape(value, 0, &mut nodes)?;
    Ok(bytes.len())
}

fn validate_value_shape(
    value: &Value,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), MutationDataError> {
    *nodes = nodes
        .checked_add(1)
        .ok_or(MutationDataError::PayloadStructureOutOfBounds("nodes"))?;
    if *nodes > product_model::MAX_OPAQUE_JSON_NODES {
        return Err(MutationDataError::PayloadStructureOutOfBounds("nodes"));
    }
    if depth > product_model::MAX_OPAQUE_JSON_DEPTH {
        return Err(MutationDataError::PayloadStructureOutOfBounds("depth"));
    }
    match value {
        Value::Null | Value::Bool(_) => {}
        Value::Number(number) => {
            let integer = number
                .as_u64()
                .or_else(|| number.as_i64().map(|value| value.unsigned_abs()));
            if integer.is_some_and(|value| value > product_model::MAX_SAFE_JSON_INTEGER) {
                return Err(MutationDataError::PayloadStructureOutOfBounds("integer"));
            }
        }
        Value::String(string) => {
            if string.len() > product_model::MAX_OPAQUE_JSON_STRING_BYTES {
                return Err(MutationDataError::PayloadStructureOutOfBounds("string"));
            }
        }
        Value::Array(values) => {
            if values.len() > product_model::MAX_OPAQUE_JSON_ARRAY_ENTRIES {
                return Err(MutationDataError::PayloadStructureOutOfBounds("array"));
            }
            for value in values {
                validate_value_shape(value, depth + 1, nodes)?;
            }
        }
        Value::Object(values) => {
            if values.len() > product_model::MAX_OPAQUE_JSON_OBJECT_ENTRIES {
                return Err(MutationDataError::PayloadStructureOutOfBounds("object"));
            }
            for (key, value) in values {
                if key.len() > product_model::MAX_OPAQUE_JSON_STRING_BYTES {
                    return Err(MutationDataError::PayloadStructureOutOfBounds("object-key"));
                }
                validate_value_shape(value, depth + 1, nodes)?;
            }
        }
    }
    Ok(())
}
