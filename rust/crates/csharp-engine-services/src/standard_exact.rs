//! NativeAOT adapter for the canonical `gameplay-standard` exact owner.

use std::{collections::BTreeMap, ffi::c_void};

use csharp_engine_abi::*;
use gameplay_mechanics::{MechanicsArithmeticError, MechanicsScalar, StatId, TrackId};
use gameplay_rules::{
    RuleDomainId, RulePackageId, RulePackageSchemaVersion, RuleProvenance, RuleSource,
    RuleSourceId, RuleSubjectId, RuleVersion,
};
use gameplay_standard::{
    admit_exact_definition, AdmittedExactDefinition, BoundedRollDescriptor,
    CapabilityRequirementId, CapabilityRoleId, ExactComparison, ExactDefinition,
    ExactEvaluationError, ExactEvaluator, ExactExpr, ExactExprLimits, ExactExprRequirements,
    ExactInputBundle, ExactInputBundleError, ExactInputReference, RoleRequirement,
    StandardDefinitionError, StandardExactFactReference, StandardPackageContext,
};

use crate::composition::{borrowed_slice, borrowed_utf8, ABI_OK};

const SERVICE: &[u8] = b"StandardExact";
const ADMIT: &[u8] = b"Admit";
const EVALUATE: &[u8] = b"Evaluate";
const MAX_DIAGNOSTIC_SOURCE_BYTES: usize = 512;

pub(crate) struct RuntimeStandardExactBridge {
    definitions: BTreeMap<u64, AdmittedExactDefinition>,
    next_definition: u64,
    predicates: BTreeMap<u64, ExactComparison>,
    next_predicate: u64,
    readout_leases: BTreeMap<u64, StandardExactReadoutBacking>,
    next_readout_lease: u64,
    evaluation_leases: BTreeMap<u64, StandardExactEvaluationBacking>,
    next_evaluation_lease: u64,
    predicate_readout_leases: BTreeMap<u64, StandardExactPredicateReadoutBacking>,
    next_predicate_readout_lease: u64,
    predicate_evaluation_leases: BTreeMap<u64, StandardExactPredicateEvaluationBacking>,
    next_predicate_evaluation_lease: u64,
    diagnostic_leases: BTreeMap<u64, StandardExactDiagnosticLease>,
    next_diagnostic_lease: u64,
}

struct StandardExactReadoutBacking {
    _text: Vec<String>,
    definitions: Vec<NativeStandardExactDefinitionReadoutRow>,
    roles: Vec<NativeStandardExactRoleRequirementRow>,
    capabilities: Vec<NativeStandardExactCapabilityRequirementRow>,
    inputs: Vec<NativeStandardExactInputRequirementRow>,
}

struct StandardExactEvaluationBacking {
    results: Vec<NativeStandardExactEvaluationRow>,
}
struct StandardExactPredicateReadoutBacking {
    _text: Vec<String>,
    predicates: Vec<NativeStandardExactPredicateReadoutRow>,
    inputs: Vec<NativeStandardExactInputRequirementRow>,
}
struct StandardExactPredicateEvaluationBacking {
    results: Vec<NativeStandardExactPredicateEvaluationRow>,
}

struct StandardExactDiagnosticLease {
    _values: Vec<StandardExactDiagnosticValue>,
    readout: Vec<NativeEngineDiagnostic>,
}

struct StandardExactDiagnosticValue {
    code: String,
    message: String,
    source: String,
}

#[derive(Debug)]
enum StandardExactOperationError {
    Request { code: &'static str, source: String },
    Definition(StandardDefinitionError),
    Role(gameplay_standard::RoleRequirementError),
    Evidence(ExactInputBundleError),
    Evaluation(ExactEvaluationError),
    UnknownDefinition { value: u64 },
    UnknownPredicate { value: u64 },
    LeaseExhausted { field: &'static str },
}

struct ReadoutText {
    values: Vec<String>,
}

impl ReadoutText {
    fn copy(&mut self, value: &str) -> NativeUtf8Slice {
        self.values.push(value.to_owned());
        let copied = self.values.last().expect("pushed exact readout text");
        NativeUtf8Slice {
            bytes: copied.as_ptr(),
            len: copied.len(),
        }
    }
}

impl RuntimeStandardExactBridge {
    pub(crate) fn new() -> Self {
        Self {
            definitions: BTreeMap::new(),
            next_definition: 1,
            predicates: BTreeMap::new(),
            next_predicate: 1,
            readout_leases: BTreeMap::new(),
            next_readout_lease: 1,
            evaluation_leases: BTreeMap::new(),
            next_evaluation_lease: 1,
            predicate_readout_leases: BTreeMap::new(),
            next_predicate_readout_lease: 1,
            predicate_evaluation_leases: BTreeMap::new(),
            next_predicate_evaluation_lease: 1,
            diagnostic_leases: BTreeMap::new(),
            next_diagnostic_lease: 1,
        }
    }

    fn admit(
        &mut self,
        request: NativeStandardExactAdmitRequest,
    ) -> Result<NativeStandardExactDefinitionHandle, StandardExactOperationError> {
        let definition = parse_definition(request)?;
        let value = self.next_definition;
        self.next_definition =
            value
                .checked_add(1)
                .ok_or(StandardExactOperationError::LeaseExhausted {
                    field: "standardExact.definition",
                })?;
        self.definitions.insert(value, definition);
        Ok(NativeStandardExactDefinitionHandle { value })
    }

    fn destroy(&mut self, handle: NativeStandardExactDefinitionHandle) -> bool {
        handle.value != 0 && self.definitions.remove(&handle.value).is_some()
    }

    fn read(
        &mut self,
        handle: NativeStandardExactDefinitionHandle,
    ) -> Option<NativeStandardExactReadoutLease> {
        let definition = self.definitions.get(&handle.value)?;
        let lease_value = self.next_readout_lease;
        self.next_readout_lease = lease_value.checked_add(1)?;
        let mut text = ReadoutText { values: Vec::new() };
        let package = definition.package();
        let identity = package.identity();
        let details = definition.definition();
        let requirements = details.requirements().ok()?;
        let limits = ExactExprLimits::default();
        let definition_row = NativeStandardExactDefinitionReadoutRow {
            domain: text.copy(identity.domain().as_str()),
            package: text.copy(identity.package().as_str()),
            package_version: identity.version().get(),
            fingerprint: text.copy(package.fingerprint().as_str()),
            subject: text.copy(details.subject().as_str()),
            source: text.copy(details.source().as_str()),
            family: text.copy(definition.identity().family()),
            semantics_version: definition.identity().semantics_version(),
            maximum_depth: narrow(limits.maximum_depth)?,
            maximum_nodes: narrow(limits.maximum_nodes)?,
            maximum_inputs: narrow(limits.maximum_inputs)?,
            maximum_arity: narrow(limits.maximum_arity)?,
            maximum_work: narrow(limits.maximum_work)?,
        };
        let mut roles = Vec::new();
        let mut capabilities = Vec::new();
        for role in requirements.roles() {
            let start = narrow(capabilities.len())?;
            for capability in role.capabilities() {
                capabilities.push(NativeStandardExactCapabilityRequirementRow {
                    capability: text.copy(capability.as_str()),
                });
            }
            roles.push(NativeStandardExactRoleRequirementRow {
                role: text.copy(role.role().as_str()),
                capabilities_start: start,
                capabilities_len: narrow(role.capabilities().len())?,
            });
        }
        let inputs = requirements
            .inputs()
            .iter()
            .map(|input| native_requirement(&mut text, input))
            .collect::<Vec<_>>();
        let backing = StandardExactReadoutBacking {
            _text: text.values,
            definitions: vec![definition_row],
            roles,
            capabilities,
            inputs,
        };
        let lease = NativeStandardExactReadoutLease {
            handle: NativeStandardExactReadoutLeaseHandle { value: lease_value },
            definitions: backing.definitions.as_ptr(),
            definitions_len: backing.definitions.len(),
            roles: backing.roles.as_ptr(),
            roles_len: backing.roles.len(),
            capabilities: backing.capabilities.as_ptr(),
            capabilities_len: backing.capabilities.len(),
            inputs: backing.inputs.as_ptr(),
            inputs_len: backing.inputs.len(),
        };
        self.readout_leases.insert(lease_value, backing);
        Some(lease)
    }

    fn destroy_readout_lease(&mut self, handle: NativeStandardExactReadoutLeaseHandle) -> bool {
        handle.value != 0 && self.readout_leases.remove(&handle.value).is_some()
    }

    fn evaluate(
        &mut self,
        request: NativeStandardExactEvaluateRequest,
    ) -> Result<NativeStandardExactEvaluationLease, StandardExactOperationError> {
        let definition = self.definitions.get(&request.definition.value).ok_or(
            StandardExactOperationError::UnknownDefinition {
                value: request.definition.value,
            },
        )?;
        let evidence = unsafe {
            borrowed_slice(
                request.evidence,
                request.evidence_len,
                "standard exact evidence",
            )
        }
        .map_err(|_| StandardExactOperationError::Request {
            code: "STANDARD_EXACT_EVIDENCE_POINTER",
            source: "evidence".to_owned(),
        })?;
        if evidence.len() > ExactExprLimits::default().maximum_inputs {
            return Err(request_error("STANDARD_EXACT_EVIDENCE_QUOTA", "evidence"));
        }
        let bundle = ExactInputBundle::new(
            evidence
                .iter()
                .map(parse_evidence)
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(StandardExactOperationError::Evidence)?;
        let receipt = ExactEvaluator::evaluate_with_receipt(
            definition.definition().expression(),
            &bundle,
            Default::default(),
        )
        .map_err(StandardExactOperationError::Evaluation)?;
        let lease_value = self.next_evaluation_lease;
        self.next_evaluation_lease =
            lease_value
                .checked_add(1)
                .ok_or(StandardExactOperationError::LeaseExhausted {
                    field: "standardExact.evaluationLease",
                })?;
        let work_used =
            narrow(receipt.work_used()).ok_or(StandardExactOperationError::LeaseExhausted {
                field: "standardExact.workUsed",
            })?;
        let backing = StandardExactEvaluationBacking {
            results: vec![NativeStandardExactEvaluationRow {
                value: receipt.value().get(),
                work_used,
            }],
        };
        let lease = NativeStandardExactEvaluationLease {
            handle: NativeStandardExactEvaluationLeaseHandle { value: lease_value },
            results: backing.results.as_ptr(),
            results_len: backing.results.len(),
        };
        self.evaluation_leases.insert(lease_value, backing);
        Ok(lease)
    }

    fn destroy_evaluation_lease(
        &mut self,
        handle: NativeStandardExactEvaluationLeaseHandle,
    ) -> bool {
        handle.value != 0 && self.evaluation_leases.remove(&handle.value).is_some()
    }

    fn admit_predicate(
        &mut self,
        request: NativeStandardExactPredicateAdmitRequest,
    ) -> Result<NativeStandardExactPredicateHandle, StandardExactOperationError> {
        let nodes = unsafe {
            borrowed_slice(
                request.nodes,
                request.nodes_len,
                "standard exact predicate nodes",
            )
        }
        .map_err(|_| request_error("STANDARD_EXACT_NODE_POINTER", "nodes"))?;
        let child_indices = unsafe {
            borrowed_slice(
                request.child_indices,
                request.child_indices_len,
                "standard exact predicate child indices",
            )
        }
        .map_err(|_| request_error("STANDARD_EXACT_CHILD_POINTER", "child_indices"))?;
        validate_flat_request_limits(nodes, child_indices)?;
        let left_root = usize::try_from(request.left_node_index)
            .map_err(|_| request_error("STANDARD_EXACT_ROOT", "left_node_index"))?;
        let right_root = usize::try_from(request.right_node_index)
            .map_err(|_| request_error("STANDARD_EXACT_ROOT", "right_node_index"))?;
        validate_flat_shape(nodes, child_indices, &[left_root, right_root])?;
        let left = build_expression(nodes, child_indices, request.left_node_index)?;
        let right = build_expression(nodes, child_indices, request.right_node_index)?;
        let predicate = match request.comparison {
            NativeStandardExactComparisonKind::Equal => ExactComparison::Equal(left, right),
            NativeStandardExactComparisonKind::LessThan => ExactComparison::LessThan(left, right),
            NativeStandardExactComparisonKind::LessOrEqual => {
                ExactComparison::LessOrEqual(left, right)
            }
            NativeStandardExactComparisonKind::GreaterThan => {
                ExactComparison::GreaterThan(left, right)
            }
            NativeStandardExactComparisonKind::GreaterOrEqual => {
                ExactComparison::GreaterOrEqual(left, right)
            }
        };
        ExactEvaluator::validate_comparison_structure(&predicate, Default::default())
            .map_err(StandardExactOperationError::Evaluation)?;
        let value = self.next_predicate;
        self.next_predicate =
            value
                .checked_add(1)
                .ok_or(StandardExactOperationError::LeaseExhausted {
                    field: "standardExact.predicate",
                })?;
        self.predicates.insert(value, predicate);
        Ok(NativeStandardExactPredicateHandle { value })
    }

    fn destroy_predicate(&mut self, handle: NativeStandardExactPredicateHandle) -> bool {
        handle.value != 0 && self.predicates.remove(&handle.value).is_some()
    }

    fn read_predicate(
        &mut self,
        handle: NativeStandardExactPredicateHandle,
    ) -> Option<NativeStandardExactPredicateReadoutLease> {
        let predicate = self.predicates.get(&handle.value)?;
        let requirements = ExactExprRequirements::inspect_comparison(predicate).ok()?;
        let comparison = native_comparison_kind(predicate);
        let lease_value = self.next_predicate_readout_lease;
        self.next_predicate_readout_lease = lease_value.checked_add(1)?;
        let mut text = ReadoutText { values: Vec::new() };
        let inputs = requirements
            .inputs()
            .iter()
            .map(|input| native_requirement(&mut text, input))
            .collect();
        let limits = ExactExprLimits::default();
        let backing = StandardExactPredicateReadoutBacking {
            _text: text.values,
            predicates: vec![NativeStandardExactPredicateReadoutRow {
                comparison,
                maximum_depth: narrow(limits.maximum_depth)?,
                maximum_nodes: narrow(limits.maximum_nodes)?,
                maximum_inputs: narrow(limits.maximum_inputs)?,
                maximum_arity: narrow(limits.maximum_arity)?,
                maximum_work: narrow(limits.maximum_work)?,
            }],
            inputs,
        };
        let lease = NativeStandardExactPredicateReadoutLease {
            handle: NativeStandardExactPredicateReadoutLeaseHandle { value: lease_value },
            predicates: backing.predicates.as_ptr(),
            predicates_len: backing.predicates.len(),
            inputs: backing.inputs.as_ptr(),
            inputs_len: backing.inputs.len(),
        };
        self.predicate_readout_leases.insert(lease_value, backing);
        Some(lease)
    }

    fn destroy_predicate_readout_lease(
        &mut self,
        handle: NativeStandardExactPredicateReadoutLeaseHandle,
    ) -> bool {
        handle.value != 0
            && self
                .predicate_readout_leases
                .remove(&handle.value)
                .is_some()
    }

    fn evaluate_predicate(
        &mut self,
        request: NativeStandardExactEvaluatePredicateRequest,
    ) -> Result<NativeStandardExactPredicateEvaluationLease, StandardExactOperationError> {
        let predicate = self.predicates.get(&request.predicate.value).ok_or(
            StandardExactOperationError::UnknownPredicate {
                value: request.predicate.value,
            },
        )?;
        let evidence = unsafe {
            borrowed_slice(
                request.evidence,
                request.evidence_len,
                "standard exact predicate evidence",
            )
        }
        .map_err(|_| request_error("STANDARD_EXACT_EVIDENCE_POINTER", "evidence"))?;
        if evidence.len() > ExactExprLimits::default().maximum_inputs {
            return Err(request_error("STANDARD_EXACT_EVIDENCE_QUOTA", "evidence"));
        }
        let bundle = ExactInputBundle::new(
            evidence
                .iter()
                .map(parse_evidence)
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(StandardExactOperationError::Evidence)?;
        let receipt =
            ExactEvaluator::evaluate_predicate_with_receipt(predicate, &bundle, Default::default())
                .map_err(StandardExactOperationError::Evaluation)?;
        let lease_value = self.next_predicate_evaluation_lease;
        self.next_predicate_evaluation_lease =
            lease_value
                .checked_add(1)
                .ok_or(StandardExactOperationError::LeaseExhausted {
                    field: "standardExact.predicateEvaluationLease",
                })?;
        let work_used =
            narrow(receipt.work_used()).ok_or(StandardExactOperationError::LeaseExhausted {
                field: "standardExact.predicateWorkUsed",
            })?;
        let backing = StandardExactPredicateEvaluationBacking {
            results: vec![NativeStandardExactPredicateEvaluationRow {
                value: receipt.value(),
                work_used,
            }],
        };
        let lease = NativeStandardExactPredicateEvaluationLease {
            handle: NativeStandardExactPredicateEvaluationLeaseHandle { value: lease_value },
            results: backing.results.as_ptr(),
            results_len: backing.results.len(),
        };
        self.predicate_evaluation_leases
            .insert(lease_value, backing);
        Ok(lease)
    }

    fn destroy_predicate_evaluation_lease(
        &mut self,
        handle: NativeStandardExactPredicateEvaluationLeaseHandle,
    ) -> bool {
        handle.value != 0
            && self
                .predicate_evaluation_leases
                .remove(&handle.value)
                .is_some()
    }

    fn retain_diagnostic(
        &mut self,
        error: &StandardExactOperationError,
    ) -> Option<NativeEngineDiagnosticLease> {
        let value = StandardExactDiagnosticValue::from_error(error);
        let lease_value = self.next_diagnostic_lease;
        self.next_diagnostic_lease = lease_value.checked_add(1)?;
        let values = vec![value];
        let readout = values
            .iter()
            .map(|value| NativeEngineDiagnostic {
                code: NativeUtf8Slice {
                    bytes: value.code.as_ptr(),
                    len: value.code.len(),
                },
                message: NativeUtf8Slice {
                    bytes: value.message.as_ptr(),
                    len: value.message.len(),
                },
                source: NativeUtf8Slice {
                    bytes: value.source.as_ptr(),
                    len: value.source.len(),
                },
            })
            .collect::<Vec<_>>();
        let backing = StandardExactDiagnosticLease {
            _values: values,
            readout,
        };
        let lease = NativeEngineDiagnosticLease {
            handle: NativeEngineDiagnosticLeaseHandle { value: lease_value },
            diagnostics: backing.readout.as_ptr(),
            diagnostics_len: backing.readout.len(),
        };
        self.diagnostic_leases.insert(lease_value, backing);
        Some(lease)
    }

    fn destroy_diagnostic_lease(&mut self, handle: NativeEngineDiagnosticLeaseHandle) -> bool {
        handle.value != 0 && self.diagnostic_leases.remove(&handle.value).is_some()
    }
}

impl StandardExactDiagnosticValue {
    fn fixed(code: &'static str, message: &'static str, source: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.to_owned(),
            source: bounded_source(&source.into()),
        }
    }

    fn from_error(error: &StandardExactOperationError) -> Self {
        match error {
            StandardExactOperationError::Request { code, source } => Self::fixed(code, "Typed StandardExact request was invalid.", source),
            StandardExactOperationError::UnknownDefinition { value } => Self::fixed("STANDARD_EXACT_DEFINITION_HANDLE", "Exact definition handle was not retained.", value.to_string()),
            StandardExactOperationError::UnknownPredicate { value } => Self::fixed("STANDARD_EXACT_PREDICATE_HANDLE", "Exact predicate handle was not retained.", value.to_string()),
            StandardExactOperationError::LeaseExhausted { field } => Self::fixed("STANDARD_EXACT_LEASE", "Exact service handle allocation overflowed.", *field),
            StandardExactOperationError::Evidence(error) => match error {
                ExactInputBundleError::ConflictingDescriptor { .. } => Self::fixed("STANDARD_EXACT_INPUT_DESCRIPTOR_CONFLICT", "Evidence repeated one input identity with incompatible bounded-roll descriptors.", "evidence"),
                ExactInputBundleError::ConflictingValue { .. } => Self::fixed("STANDARD_EXACT_INPUT_VALUE_CONFLICT", "Evidence repeated one exact input with conflicting values.", "evidence"),
                ExactInputBundleError::InvalidBoundedRollDescriptor { .. } => Self::fixed("STANDARD_EXACT_BOUNDED_ROLL_DESCRIPTOR", "Evidence bounded-roll minimum exceeded maximum.", "evidence"),
            },
            StandardExactOperationError::Evaluation(error) => exact_evaluation_diagnostic(error),
            StandardExactOperationError::Role(error) => match error {
                gameplay_standard::RoleRequirementError::InvalidRoleId { .. } => Self::fixed("STANDARD_EXACT_ROLE", "Role identity was invalid.", "roles"),
                gameplay_standard::RoleRequirementError::InvalidCapabilityId { .. } => Self::fixed("STANDARD_EXACT_CAPABILITY", "Capability identity was invalid.", "roles"),
                gameplay_standard::RoleRequirementError::InvalidInputId { .. } => Self::fixed("STANDARD_EXACT_INPUT_ID", "Input identity was invalid.", "nodes"),
                gameplay_standard::RoleRequirementError::CapabilityQuotaExceeded { .. } => Self::fixed("STANDARD_EXACT_CAPABILITY_QUOTA", "A role exceeded its capability quota.", "roles"),
                gameplay_standard::RoleRequirementError::NonCanonicalCapabilities => Self::fixed("STANDARD_EXACT_CAPABILITY_ORDER", "Role capability requirements were not canonical.", "roles"),
            },
            StandardExactOperationError::Definition(error) => standard_definition_diagnostic(error),
        }
    }
}

fn standard_definition_diagnostic(error: &StandardDefinitionError) -> StandardExactDiagnosticValue {
    match error {
        StandardDefinitionError::Package(error) => rule_package_diagnostic(error),
        StandardDefinitionError::Role(error) => match error {
            gameplay_standard::RoleRequirementError::InvalidRoleId { .. } => {
                StandardExactDiagnosticValue::fixed(
                    "STANDARD_EXACT_ROLE",
                    "Role identity was invalid.",
                    "roles",
                )
            }
            gameplay_standard::RoleRequirementError::InvalidCapabilityId { .. } => {
                StandardExactDiagnosticValue::fixed(
                    "STANDARD_EXACT_CAPABILITY",
                    "Capability identity was invalid.",
                    "roles",
                )
            }
            gameplay_standard::RoleRequirementError::InvalidInputId { .. } => {
                StandardExactDiagnosticValue::fixed(
                    "STANDARD_EXACT_INPUT_ID",
                    "Input identity was invalid.",
                    "nodes",
                )
            }
            gameplay_standard::RoleRequirementError::CapabilityQuotaExceeded { .. } => {
                StandardExactDiagnosticValue::fixed(
                    "STANDARD_EXACT_CAPABILITY_QUOTA",
                    "A role exceeded its capability quota.",
                    "roles",
                )
            }
            gameplay_standard::RoleRequirementError::NonCanonicalCapabilities => {
                StandardExactDiagnosticValue::fixed(
                    "STANDARD_EXACT_CAPABILITY_ORDER",
                    "Role capability requirements were not canonical.",
                    "roles",
                )
            }
        },
        StandardDefinitionError::ExactLiteral { .. } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_SCALAR",
            "Exact scalar was outside the Mechanics scalar range.",
            "nodes",
        ),
        StandardDefinitionError::ExactStructure(error) => exact_evaluation_diagnostic(error),
        StandardDefinitionError::MalformedPayload { path, .. } => {
            StandardExactDiagnosticValue::fixed(
                "STANDARD_EXACT_CANONICAL_PAYLOAD",
                "Canonical exact package payload was malformed after admission.",
                path,
            )
        }
        StandardDefinitionError::WrongSchema { .. } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_SCHEMA",
            "Exact definitions require IntegerOnlyV1 package schema.",
            "package",
        ),
        StandardDefinitionError::WrongFamily { .. } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_FAMILY",
            "Canonical package did not retain the exact family.",
            "package",
        ),
        StandardDefinitionError::UnsupportedSemanticsVersion { .. } => {
            StandardExactDiagnosticValue::fixed(
                "STANDARD_EXACT_SEMANTICS",
                "Exact package semantics version is unsupported.",
                "package",
            )
        }
        StandardDefinitionError::MissingCorrelation { .. } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_CORRELATION",
            "Package provenance did not correlate definition subject and source.",
            "provenance",
        ),
        StandardDefinitionError::SourceMismatch { .. } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_SOURCE_MISMATCH",
            "Package provenance selected a different source for the definition subject.",
            "provenance",
        ),
        StandardDefinitionError::NonConvergentPayload => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_NONCONVERGENT",
            "Canonical owner rehydration did not reproduce the submitted exact definition.",
            "package",
        ),
        StandardDefinitionError::UndeclaredInputRole { role } => {
            StandardExactDiagnosticValue::fixed(
                "STANDARD_EXACT_UNDECLARED_ROLE",
                "Expression input role was not declared by the definition.",
                role.as_str(),
            )
        }
        // Continuous variants are part of the public non-exhaustive definition error but cannot be produced by this exact bridge.
        StandardDefinitionError::ContinuousLiteral(_)
        | StandardDefinitionError::ContinuousStructure(_) => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_OWNER",
            "Unexpected continuous owner error while admitting exact definition.",
            "owner",
        ),
    }
}

fn exact_evaluation_diagnostic(error: &ExactEvaluationError) -> StandardExactDiagnosticValue {
    match error {
        ExactEvaluationError::Arithmetic(error) => arithmetic_diagnostic(error),
        ExactEvaluationError::MissingInput { .. } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_MISSING_INPUT",
            "Exact evaluation lacked a required input observation.",
            "evidence",
        ),
        ExactEvaluationError::MissingBoundedRoll { .. } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_MISSING_BOUNDED_ROLL",
            "Exact evaluation lacked a required bounded-roll observation.",
            "evidence",
        ),
        ExactEvaluationError::BoundedRollOutOfRange { .. } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_BOUNDED_ROLL_RANGE",
            "Bounded-roll evidence was outside its authored inclusive range.",
            "evidence",
        ),
        ExactEvaluationError::BoundedRollInvalidBounds { .. } => {
            StandardExactDiagnosticValue::fixed(
                "STANDARD_EXACT_BOUNDED_ROLL_DESCRIPTOR",
                "Bounded-roll minimum exceeded maximum.",
                "nodes",
            )
        }
        ExactEvaluationError::ConflictingInputDescriptor { .. } => {
            StandardExactDiagnosticValue::fixed(
                "STANDARD_EXACT_INPUT_DESCRIPTOR_CONFLICT",
                "Definition supplied incompatible descriptors for one input identity.",
                "nodes",
            )
        }
        ExactEvaluationError::FixedPowerScaleOutOfRange { .. } => {
            StandardExactDiagnosticValue::fixed(
                "STANDARD_EXACT_POWER_SCALE",
                "Fixed-power scale was outside the owner range.",
                "nodes",
            )
        }
        ExactEvaluationError::FixedPowerNegativeBase { .. } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_POWER_NEGATIVE_BASE",
            "Fixed-power base must be nonnegative.",
            "nodes",
        ),
        ExactEvaluationError::FixedPowerExponentOutOfRange { .. } => {
            StandardExactDiagnosticValue::fixed(
                "STANDARD_EXACT_POWER_EXPONENT",
                "Fixed-power exponent was outside the owner range.",
                "nodes",
            )
        }
        ExactEvaluationError::FixedPowerMultiplicationOverflow
        | ExactEvaluationError::FixedPowerScalarRange { .. } => {
            StandardExactDiagnosticValue::fixed(
                "STANDARD_EXACT_POWER_ARITHMETIC",
                "Fixed-power arithmetic exceeded the Mechanics scalar range.",
                "nodes",
            )
        }
        ExactEvaluationError::DepthExceeded { .. } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_DEPTH_QUOTA",
            "Exact expression exceeded its depth quota.",
            "nodes",
        ),
        ExactEvaluationError::NodeQuotaExceeded { .. } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_NODE_QUOTA",
            "Exact expression exceeded its node quota.",
            "nodes",
        ),
        ExactEvaluationError::InputQuotaExceeded { .. } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_INPUT_QUOTA",
            "Exact expression exceeded its input quota.",
            "nodes",
        ),
        ExactEvaluationError::ArityExceeded { .. } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_ARITY_QUOTA",
            "Exact min/max expression exceeded its arity quota.",
            "nodes",
        ),
        ExactEvaluationError::EmptyAggregate => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_EMPTY_AGGREGATE",
            "Exact min/max expression must have at least one child.",
            "nodes",
        ),
        ExactEvaluationError::WorkQuotaExceeded { .. } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_WORK_QUOTA",
            "Exact evaluation exceeded its deterministic work quota.",
            "evaluation",
        ),
    }
}

fn arithmetic_diagnostic(error: &MechanicsArithmeticError) -> StandardExactDiagnosticValue {
    match error {
        MechanicsArithmeticError::ScalarOutOfRange { .. } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_SCALAR",
            "Exact arithmetic result was outside the Mechanics scalar range.",
            "evaluation",
        ),
        MechanicsArithmeticError::RatioComponentOutOfRange { .. } => {
            StandardExactDiagnosticValue::fixed(
                "STANDARD_EXACT_RATIO",
                "Exact arithmetic ratio component was outside the Mechanics range.",
                "evaluation",
            )
        }
        MechanicsArithmeticError::ZeroDenominator => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_DIVIDE_ZERO",
            "Exact division denominator was zero.",
            "evaluation",
        ),
        MechanicsArithmeticError::NegativeAmount { .. } | MechanicsArithmeticError::Overflow => {
            StandardExactDiagnosticValue::fixed(
                "STANDARD_EXACT_ARITHMETIC",
                "Exact arithmetic overflowed or was invalid.",
                "evaluation",
            )
        }
    }
}

fn rule_package_diagnostic(
    error: &gameplay_rules::RulePackageError,
) -> StandardExactDiagnosticValue {
    use gameplay_rules::RulePackageError::*;
    match error {
        ArtifactQuotaExceeded { .. } | QuotaExceeded { .. } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_PACKAGE_QUOTA",
            "Exact canonical package exceeded an owner quota.",
            "package",
        ),
        InvalidIdentity { path, .. } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_PACKAGE_IDENTITY",
            "Exact package identity was invalid.",
            path,
        ),
        InvalidVersion { path, .. } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_PACKAGE_VERSION",
            "Exact package version was invalid.",
            path,
        ),
        InvalidSourcePath { path, .. } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_SOURCE_PATH",
            "Exact package source path was invalid.",
            path,
        ),
        InvalidSourceLocation { path, .. } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_PROVENANCE",
            "Exact package provenance location was invalid.",
            path,
        ),
        ArithmeticOverflow { path } => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_PACKAGE_ARITHMETIC",
            "Exact package admission arithmetic overflowed.",
            path,
        ),
        _ => StandardExactDiagnosticValue::fixed(
            "STANDARD_EXACT_PACKAGE",
            "Exact canonical package was rejected by the owner.",
            "package",
        ),
    }
}

fn parse_definition(
    request: NativeStandardExactAdmitRequest,
) -> Result<AdmittedExactDefinition, StandardExactOperationError> {
    let domain = parse_rule_id(request.domain, "domain", RuleDomainId::parse)?;
    let package = parse_rule_id(request.package, "package", RulePackageId::parse)?;
    let version = RuleVersion::new(request.package_version)
        .map_err(|_| request_error("STANDARD_EXACT_PACKAGE_VERSION", "package_version"))?;
    let subject = parse_rule_id(request.subject, "subject", RuleSubjectId::parse)?;
    let source = parse_rule_id(request.source, "source", RuleSourceId::parse)?;
    let source_path = copied_utf8(request.source_path, "source_path")?;
    let roles = unsafe { borrowed_slice(request.roles, request.roles_len, "standard exact roles") }
        .map_err(|_| request_error("STANDARD_EXACT_ROLE_POINTER", "roles"))?;
    let capabilities = unsafe {
        borrowed_slice(
            request.capabilities,
            request.capabilities_len,
            "standard exact capabilities",
        )
    }
    .map_err(|_| request_error("STANDARD_EXACT_CAPABILITY_POINTER", "capabilities"))?;
    let nodes = unsafe { borrowed_slice(request.nodes, request.nodes_len, "standard exact nodes") }
        .map_err(|_| request_error("STANDARD_EXACT_NODE_POINTER", "nodes"))?;
    let child_indices = unsafe {
        borrowed_slice(
            request.child_indices,
            request.child_indices_len,
            "standard exact child indices",
        )
    }
    .map_err(|_| request_error("STANDARD_EXACT_CHILD_POINTER", "child_indices"))?;
    validate_flat_request_limits(nodes, child_indices)?;
    let root = usize::try_from(request.root_node_index)
        .map_err(|_| request_error("STANDARD_EXACT_ROOT", "root_node_index"))?;
    validate_flat_shape(nodes, child_indices, &[root])?;
    let expression = build_expression(nodes, child_indices, request.root_node_index)?;
    let roles = roles
        .iter()
        .map(|role| parse_role(role, capabilities))
        .collect::<Result<Vec<_>, _>>()?;
    let definition = ExactDefinition::new(subject.clone(), source.clone(), expression, roles)
        .map_err(StandardExactOperationError::Definition)?;
    let context = StandardPackageContext::new(
        RulePackageSchemaVersion::IntegerOnlyV1,
        domain,
        package,
        version,
        vec![],
        vec![RuleSource::new(source.clone(), source_path)
            .map_err(|_| request_error("STANDARD_EXACT_SOURCE_PATH", "source_path"))?],
        vec![RuleProvenance::new(
            subject,
            source,
            request
                .has_provenance_line
                .then_some(request.provenance_line),
            request
                .has_provenance_column
                .then_some(request.provenance_column),
        )
        .map_err(|_| request_error("STANDARD_EXACT_PROVENANCE", "provenance"))?],
    );
    admit_exact_definition(&context, definition).map_err(StandardExactOperationError::Definition)
}

fn parse_rule_id<T>(
    value: NativeUtf8Slice,
    field: &'static str,
    parse: impl FnOnce(String) -> Result<T, gameplay_rules::RulePackageError>,
) -> Result<T, StandardExactOperationError> {
    parse(copied_utf8(value, field)?)
        .map_err(|_| request_error("STANDARD_EXACT_PACKAGE_IDENTITY", field))
}

fn parse_role(
    value: &NativeStandardExactRole,
    capabilities: &[NativeStandardExactCapability],
) -> Result<RoleRequirement, StandardExactOperationError> {
    let role = CapabilityRoleId::parse(copied_utf8(value.role, "role")?)
        .map_err(|_| request_error("STANDARD_EXACT_ROLE", "roles"))?;
    let start = usize::try_from(value.capabilities_start)
        .map_err(|_| request_error("STANDARD_EXACT_CAPABILITY_RANGE", "roles"))?;
    let len = usize::try_from(value.capabilities_len)
        .map_err(|_| request_error("STANDARD_EXACT_CAPABILITY_RANGE", "roles"))?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| request_error("STANDARD_EXACT_CAPABILITY_RANGE", "roles"))?;
    let capabilities = capabilities
        .get(start..end)
        .ok_or_else(|| request_error("STANDARD_EXACT_CAPABILITY_RANGE", "roles"))?
        .iter()
        .map(|capability| {
            CapabilityRequirementId::parse(copied_utf8(capability.capability, "capability")?)
                .map_err(|_| request_error("STANDARD_EXACT_CAPABILITY", "roles"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    RoleRequirement::new(role, capabilities).map_err(StandardExactOperationError::Role)
}

fn build_expression(
    nodes: &[NativeStandardExactNode],
    child_indices: &[u32],
    root: u32,
) -> Result<ExactExpr, StandardExactOperationError> {
    let root = usize::try_from(root)
        .map_err(|_| request_error("STANDARD_EXACT_ROOT", "root_node_index"))?;
    if root >= nodes.len() {
        return Err(request_error("STANDARD_EXACT_ROOT", "root_node_index"));
    }
    let mut built = Vec::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        built.push(build_node(index, node, child_indices, &built)?);
    }
    Ok(built.swap_remove(root))
}

fn validate_flat_request_limits(
    nodes: &[NativeStandardExactNode],
    child_indices: &[u32],
) -> Result<(), StandardExactOperationError> {
    let limits = ExactExprLimits::default();
    if nodes.len() > limits.maximum_nodes {
        return Err(request_error("STANDARD_EXACT_NODE_QUOTA", "nodes"));
    }
    let maximum_child_indices = nodes
        .len()
        .checked_mul(limits.maximum_arity)
        .ok_or_else(|| request_error("STANDARD_EXACT_CHILD_QUOTA", "child_indices"))?;
    if child_indices.len() > maximum_child_indices {
        return Err(request_error("STANDARD_EXACT_CHILD_QUOTA", "child_indices"));
    }
    Ok(())
}

fn build_node(
    index: usize,
    node: &NativeStandardExactNode,
    child_indices: &[u32],
    built: &[ExactExpr],
) -> Result<ExactExpr, StandardExactOperationError> {
    let child = |value: u32| -> Result<ExactExpr, StandardExactOperationError> {
        let value = usize::try_from(value)
            .map_err(|_| request_error("STANDARD_EXACT_NODE_INDEX", "nodes"))?;
        if value >= index {
            return Err(request_error("STANDARD_EXACT_NODE_ORDER", "nodes"));
        }
        built
            .get(value)
            .cloned()
            .ok_or_else(|| request_error("STANDARD_EXACT_NODE_INDEX", "nodes"))
    };
    let scalar = |value| {
        MechanicsScalar::new(value).map_err(|_| request_error("STANDARD_EXACT_SCALAR", "nodes"))
    };
    match node.kind {
        NativeStandardExactNodeKind::Literal => Ok(ExactExpr::Literal(scalar(node.literal)?)),
        NativeStandardExactNodeKind::Input => Ok(ExactExpr::Input(parse_input(
            node.input_kind,
            node.role,
            node.input_id,
            node.minimum,
            node.maximum,
        )?)),
        NativeStandardExactNodeKind::Add => Ok(ExactExpr::Add(
            Box::new(child(node.left)?),
            Box::new(child(node.right)?),
        )),
        NativeStandardExactNodeKind::Subtract => Ok(ExactExpr::Subtract(
            Box::new(child(node.left)?),
            Box::new(child(node.right)?),
        )),
        NativeStandardExactNodeKind::Multiply => Ok(ExactExpr::Multiply(
            Box::new(child(node.left)?),
            Box::new(child(node.right)?),
        )),
        NativeStandardExactNodeKind::FloorDivide => Ok(ExactExpr::FloorDivide(
            Box::new(child(node.left)?),
            Box::new(child(node.right)?),
        )),
        NativeStandardExactNodeKind::TruncatingDivide => Ok(ExactExpr::TruncatingDivide(
            Box::new(child(node.left)?),
            Box::new(child(node.right)?),
        )),
        NativeStandardExactNodeKind::FixedPower => Ok(ExactExpr::fixed_power(
            child(node.left)?,
            child(node.right)?,
            scalar(node.fixed_power_scale)?,
        )),
        NativeStandardExactNodeKind::Min | NativeStandardExactNodeKind::Max => {
            let start = usize::try_from(node.children_start)
                .map_err(|_| request_error("STANDARD_EXACT_CHILD_RANGE", "nodes"))?;
            let len = usize::try_from(node.children_len)
                .map_err(|_| request_error("STANDARD_EXACT_CHILD_RANGE", "nodes"))?;
            let end = start
                .checked_add(len)
                .ok_or_else(|| request_error("STANDARD_EXACT_CHILD_RANGE", "nodes"))?;
            let values = child_indices
                .get(start..end)
                .ok_or_else(|| request_error("STANDARD_EXACT_CHILD_RANGE", "nodes"))?
                .iter()
                .map(|child_index| child(*child_index))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(if node.kind == NativeStandardExactNodeKind::Min {
                ExactExpr::Min(values)
            } else {
                ExactExpr::Max(values)
            })
        }
    }
}

/// Validates connectivity separately from expression ordering. Node indices and
/// child-index offsets are different domains: the latter are only bounded by
/// the request's child-index span. Every selected node still has to precede
/// its parent, and all supplied rows must contribute to the submitted root.
fn validate_flat_shape(
    nodes: &[NativeStandardExactNode],
    child_indices: &[u32],
    roots: &[usize],
) -> Result<(), StandardExactOperationError> {
    let mut reached_nodes = vec![false; nodes.len()];
    let mut used_child_indices = vec![false; child_indices.len()];
    for root in roots {
        visit_flat_node(
            *root,
            nodes,
            child_indices,
            &mut reached_nodes,
            &mut used_child_indices,
        )?;
    }
    if reached_nodes.iter().any(|reached| !reached) {
        return Err(request_error("STANDARD_EXACT_UNUSED_NODE", "nodes"));
    }
    if used_child_indices.iter().any(|used| !used) {
        return Err(request_error(
            "STANDARD_EXACT_UNUSED_CHILD_INDEX",
            "child_indices",
        ));
    }
    Ok(())
}

fn visit_flat_node(
    index: usize,
    nodes: &[NativeStandardExactNode],
    child_indices: &[u32],
    reached_nodes: &mut [bool],
    used_child_indices: &mut [bool],
) -> Result<(), StandardExactOperationError> {
    let node = nodes
        .get(index)
        .ok_or_else(|| request_error("STANDARD_EXACT_NODE_INDEX", "nodes"))?;
    if reached_nodes[index] {
        return Ok(());
    }
    reached_nodes[index] = true;
    match node.kind {
        NativeStandardExactNodeKind::Literal | NativeStandardExactNodeKind::Input => Ok(()),
        NativeStandardExactNodeKind::Add
        | NativeStandardExactNodeKind::Subtract
        | NativeStandardExactNodeKind::Multiply
        | NativeStandardExactNodeKind::FloorDivide
        | NativeStandardExactNodeKind::TruncatingDivide
        | NativeStandardExactNodeKind::FixedPower => {
            visit_flat_child(
                node.left,
                index,
                nodes,
                child_indices,
                reached_nodes,
                used_child_indices,
            )?;
            visit_flat_child(
                node.right,
                index,
                nodes,
                child_indices,
                reached_nodes,
                used_child_indices,
            )
        }
        NativeStandardExactNodeKind::Min | NativeStandardExactNodeKind::Max => {
            let start = usize::try_from(node.children_start)
                .map_err(|_| request_error("STANDARD_EXACT_CHILD_RANGE", "nodes"))?;
            let len = usize::try_from(node.children_len)
                .map_err(|_| request_error("STANDARD_EXACT_CHILD_RANGE", "nodes"))?;
            let end = start
                .checked_add(len)
                .ok_or_else(|| request_error("STANDARD_EXACT_CHILD_RANGE", "nodes"))?;
            let values = child_indices
                .get(start..end)
                .ok_or_else(|| request_error("STANDARD_EXACT_CHILD_RANGE", "nodes"))?;
            for (offset, child) in values.iter().enumerate() {
                used_child_indices[start + offset] = true;
                visit_flat_child(
                    *child,
                    index,
                    nodes,
                    child_indices,
                    reached_nodes,
                    used_child_indices,
                )?;
            }
            Ok(())
        }
    }
}

fn visit_flat_child(
    child: u32,
    parent: usize,
    nodes: &[NativeStandardExactNode],
    child_indices: &[u32],
    reached_nodes: &mut [bool],
    used_child_indices: &mut [bool],
) -> Result<(), StandardExactOperationError> {
    let child =
        usize::try_from(child).map_err(|_| request_error("STANDARD_EXACT_NODE_INDEX", "nodes"))?;
    if child >= parent {
        return Err(request_error("STANDARD_EXACT_NODE_ORDER", "nodes"));
    }
    visit_flat_node(
        child,
        nodes,
        child_indices,
        reached_nodes,
        used_child_indices,
    )
}

fn parse_input(
    kind: NativeStandardExactInputKind,
    role: NativeUtf8Slice,
    input_id: NativeUtf8Slice,
    minimum: i64,
    maximum: i64,
) -> Result<ExactInputReference, StandardExactOperationError> {
    let role = CapabilityRoleId::parse(copied_utf8(role, "role")?)
        .map_err(|_| request_error("STANDARD_EXACT_ROLE", "nodes"))?;
    let id = copied_utf8(input_id, "input_id")?;
    match kind {
        NativeStandardExactInputKind::Parameter => Ok(ExactInputReference::Parameter {
            role,
            id: parse_input_id(id)?,
        }),
        NativeStandardExactInputKind::Fact => Ok(ExactInputReference::Fact {
            role,
            id: parse_input_id(id)?,
        }),
        NativeStandardExactInputKind::Roll => Ok(ExactInputReference::Roll {
            role,
            id: parse_input_id(id)?,
        }),
        NativeStandardExactInputKind::BoundedRoll => Ok(ExactInputReference::BoundedRoll {
            descriptor: Box::new(BoundedRollDescriptor::new(
                role,
                parse_input_id(id)?,
                MechanicsScalar::new(minimum)
                    .map_err(|_| request_error("STANDARD_EXACT_SCALAR", "nodes"))?,
                MechanicsScalar::new(maximum)
                    .map_err(|_| request_error("STANDARD_EXACT_SCALAR", "nodes"))?,
            )),
        }),
        NativeStandardExactInputKind::Choice => Ok(ExactInputReference::Choice {
            role,
            id: parse_input_id(id)?,
        }),
        NativeStandardExactInputKind::StandardStat => Ok(ExactInputReference::StandardFact(
            StandardExactFactReference::Stat {
                role,
                stat: StatId::parse(id)
                    .map_err(|_| request_error("STANDARD_EXACT_STAT", "nodes"))?,
            },
        )),
        NativeStandardExactInputKind::StandardTrackCurrent => Ok(
            ExactInputReference::StandardFact(StandardExactFactReference::TrackCurrent {
                role,
                track: TrackId::parse(id)
                    .map_err(|_| request_error("STANDARD_EXACT_TRACK", "nodes"))?,
            }),
        ),
        NativeStandardExactInputKind::StandardTrackMaximum => Ok(
            ExactInputReference::StandardFact(StandardExactFactReference::TrackMaximum {
                role,
                track: TrackId::parse(id)
                    .map_err(|_| request_error("STANDARD_EXACT_TRACK", "nodes"))?,
            }),
        ),
    }
}

fn parse_input_id(
    value: String,
) -> Result<gameplay_standard::InputId, StandardExactOperationError> {
    gameplay_standard::InputId::parse(value)
        .map_err(|_| request_error("STANDARD_EXACT_INPUT_ID", "nodes"))
}

fn parse_evidence(
    value: &NativeStandardExactEvidence,
) -> Result<(ExactInputReference, MechanicsScalar), StandardExactOperationError> {
    Ok((
        parse_input(
            value.kind,
            value.role,
            value.input_id,
            value.minimum,
            value.maximum,
        )?,
        MechanicsScalar::new(value.value)
            .map_err(|_| request_error("STANDARD_EXACT_SCALAR", "evidence"))?,
    ))
}

fn native_requirement(
    text: &mut ReadoutText,
    input: &ExactInputReference,
) -> NativeStandardExactInputRequirementRow {
    let (kind, role, id, minimum, maximum) = match input {
        ExactInputReference::Parameter { role, id } => (
            NativeStandardExactInputKind::Parameter,
            role.as_str(),
            id.as_str(),
            0,
            0,
        ),
        ExactInputReference::Fact { role, id } => (
            NativeStandardExactInputKind::Fact,
            role.as_str(),
            id.as_str(),
            0,
            0,
        ),
        ExactInputReference::Roll { role, id } => (
            NativeStandardExactInputKind::Roll,
            role.as_str(),
            id.as_str(),
            0,
            0,
        ),
        ExactInputReference::BoundedRoll { descriptor } => (
            NativeStandardExactInputKind::BoundedRoll,
            descriptor.role().as_str(),
            descriptor.id().as_str(),
            descriptor.minimum().get(),
            descriptor.maximum().get(),
        ),
        ExactInputReference::Choice { role, id } => (
            NativeStandardExactInputKind::Choice,
            role.as_str(),
            id.as_str(),
            0,
            0,
        ),
        ExactInputReference::StandardFact(StandardExactFactReference::Stat { role, stat }) => (
            NativeStandardExactInputKind::StandardStat,
            role.as_str(),
            stat.as_str(),
            0,
            0,
        ),
        ExactInputReference::StandardFact(StandardExactFactReference::TrackCurrent {
            role,
            track,
        }) => (
            NativeStandardExactInputKind::StandardTrackCurrent,
            role.as_str(),
            track.as_str(),
            0,
            0,
        ),
        ExactInputReference::StandardFact(StandardExactFactReference::TrackMaximum {
            role,
            track,
        }) => (
            NativeStandardExactInputKind::StandardTrackMaximum,
            role.as_str(),
            track.as_str(),
            0,
            0,
        ),
    };
    NativeStandardExactInputRequirementRow {
        kind,
        role: text.copy(role),
        input_id: text.copy(id),
        minimum,
        maximum,
    }
}

fn native_comparison_kind(value: &ExactComparison) -> NativeStandardExactComparisonKind {
    match value {
        ExactComparison::Equal(_, _) => NativeStandardExactComparisonKind::Equal,
        ExactComparison::LessThan(_, _) => NativeStandardExactComparisonKind::LessThan,
        ExactComparison::LessOrEqual(_, _) => NativeStandardExactComparisonKind::LessOrEqual,
        ExactComparison::GreaterThan(_, _) => NativeStandardExactComparisonKind::GreaterThan,
        ExactComparison::GreaterOrEqual(_, _) => NativeStandardExactComparisonKind::GreaterOrEqual,
    }
}

fn copied_utf8(
    value: NativeUtf8Slice,
    field: &'static str,
) -> Result<String, StandardExactOperationError> {
    unsafe { borrowed_utf8(value.bytes, value.len, field) }
        .map(|value| value.to_owned())
        .map_err(|_| request_error("STANDARD_EXACT_UTF8", field))
}

fn request_error(code: &'static str, source: impl Into<String>) -> StandardExactOperationError {
    StandardExactOperationError::Request {
        code,
        source: source.into(),
    }
}
fn narrow(value: usize) -> Option<u32> {
    u32::try_from(value).ok()
}
fn bounded_source(value: &str) -> String {
    value.chars().take(MAX_DIAGNOSTIC_SOURCE_BYTES).collect()
}
fn native_utf8(bytes: &[u8]) -> NativeUtf8Slice {
    NativeUtf8Slice {
        bytes: bytes.as_ptr(),
        len: bytes.len(),
    }
}

pub(crate) fn api(bridge: &mut RuntimeStandardExactBridge) -> NativeStandardExactApi {
    NativeStandardExactApi {
        context: (bridge as *mut RuntimeStandardExactBridge).cast(),
        admit,
        destroy_definition,
        read_definition,
        destroy_readout_lease,
        evaluate,
        destroy_evaluation_lease,
        destroy_operation_diagnostic_lease,
        admit_predicate,
        destroy_predicate,
        read_predicate,
        destroy_predicate_readout_lease,
        evaluate_predicate,
        destroy_predicate_evaluation_lease,
    }
}

unsafe extern "C" fn admit(
    context: *mut c_void,
    request: *const NativeStandardExactAdmitRequest,
    result: *mut NativeStandardExactDefinitionHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { *request };
    let bridge = unsafe { &mut *context.cast::<RuntimeStandardExactBridge>() };
    match bridge.admit(request) {
        Ok(handle) => {
            unsafe { *result = handle };
            ABI_OK
        }
        Err(error) => {
            if let Some(diagnostics) = bridge.retain_diagnostic(&error) {
                unsafe {
                    *receipt = NativeOperationErrorReceipt {
                        service: native_utf8(SERVICE),
                        operation: native_utf8(ADMIT),
                        status: 0,
                        diagnostics,
                    }
                };
            }
            0
        }
    }
}

unsafe extern "C" fn destroy_definition(
    context: *mut c_void,
    handle: NativeStandardExactDefinitionHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeStandardExactBridge>() };
    i32::from(bridge.destroy(handle))
}
unsafe extern "C" fn read_definition(
    context: *mut c_void,
    handle: NativeStandardExactDefinitionHandle,
    result: *mut NativeStandardExactReadoutLease,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeStandardExactBridge>() };
    match bridge.read(handle) {
        Some(lease) => {
            unsafe { *result = lease };
            ABI_OK
        }
        None => 0,
    }
}
unsafe extern "C" fn destroy_readout_lease(
    context: *mut c_void,
    handle: NativeStandardExactReadoutLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeStandardExactBridge>() };
    i32::from(bridge.destroy_readout_lease(handle))
}
unsafe extern "C" fn evaluate(
    context: *mut c_void,
    request: *const NativeStandardExactEvaluateRequest,
    result: *mut NativeStandardExactEvaluationLease,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { *request };
    let bridge = unsafe { &mut *context.cast::<RuntimeStandardExactBridge>() };
    match bridge.evaluate(request) {
        Ok(lease) => {
            unsafe { *result = lease };
            ABI_OK
        }
        Err(error) => {
            if let Some(diagnostics) = bridge.retain_diagnostic(&error) {
                unsafe {
                    *receipt = NativeOperationErrorReceipt {
                        service: native_utf8(SERVICE),
                        operation: native_utf8(EVALUATE),
                        status: 0,
                        diagnostics,
                    }
                };
            }
            0
        }
    }
}
unsafe extern "C" fn destroy_evaluation_lease(
    context: *mut c_void,
    handle: NativeStandardExactEvaluationLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeStandardExactBridge>() };
    i32::from(bridge.destroy_evaluation_lease(handle))
}
unsafe extern "C" fn destroy_operation_diagnostic_lease(
    context: *mut c_void,
    handle: NativeEngineDiagnosticLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeStandardExactBridge>() };
    i32::from(bridge.destroy_diagnostic_lease(handle))
}

unsafe extern "C" fn admit_predicate(
    context: *mut c_void,
    request: *const NativeStandardExactPredicateAdmitRequest,
    result: *mut NativeStandardExactPredicateHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { *request };
    let bridge = unsafe { &mut *context.cast::<RuntimeStandardExactBridge>() };
    match bridge.admit_predicate(request) {
        Ok(handle) => {
            unsafe { *result = handle };
            ABI_OK
        }
        Err(error) => {
            if let Some(diagnostics) = bridge.retain_diagnostic(&error) {
                unsafe {
                    *receipt = NativeOperationErrorReceipt {
                        service: native_utf8(SERVICE),
                        operation: native_utf8(b"AdmitPredicate"),
                        status: 0,
                        diagnostics,
                    }
                };
            }
            0
        }
    }
}
unsafe extern "C" fn destroy_predicate(
    context: *mut c_void,
    handle: NativeStandardExactPredicateHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeStandardExactBridge>() };
    i32::from(bridge.destroy_predicate(handle))
}
unsafe extern "C" fn read_predicate(
    context: *mut c_void,
    handle: NativeStandardExactPredicateHandle,
    result: *mut NativeStandardExactPredicateReadoutLease,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeStandardExactBridge>() };
    match bridge.read_predicate(handle) {
        Some(lease) => {
            unsafe { *result = lease };
            ABI_OK
        }
        None => 0,
    }
}
unsafe extern "C" fn destroy_predicate_readout_lease(
    context: *mut c_void,
    handle: NativeStandardExactPredicateReadoutLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeStandardExactBridge>() };
    i32::from(bridge.destroy_predicate_readout_lease(handle))
}
unsafe extern "C" fn evaluate_predicate(
    context: *mut c_void,
    request: *const NativeStandardExactEvaluatePredicateRequest,
    result: *mut NativeStandardExactPredicateEvaluationLease,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { *request };
    let bridge = unsafe { &mut *context.cast::<RuntimeStandardExactBridge>() };
    match bridge.evaluate_predicate(request) {
        Ok(lease) => {
            unsafe { *result = lease };
            ABI_OK
        }
        Err(error) => {
            if let Some(diagnostics) = bridge.retain_diagnostic(&error) {
                unsafe {
                    *receipt = NativeOperationErrorReceipt {
                        service: native_utf8(SERVICE),
                        operation: native_utf8(b"EvaluatePredicate"),
                        status: 0,
                        diagnostics,
                    }
                };
            }
            0
        }
    }
}
unsafe extern "C" fn destroy_predicate_evaluation_lease(
    context: *mut c_void,
    handle: NativeStandardExactPredicateEvaluationLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeStandardExactBridge>() };
    i32::from(bridge.destroy_predicate_evaluation_lease(handle))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf8(value: &'static str) -> NativeUtf8Slice {
        NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        }
    }

    fn literal(value: i64) -> NativeStandardExactNode {
        NativeStandardExactNode {
            kind: NativeStandardExactNodeKind::Literal,
            literal: value,
            input_kind: NativeStandardExactInputKind::Parameter,
            role: utf8(""),
            input_id: utf8(""),
            minimum: 0,
            maximum: 0,
            left: 0,
            right: 0,
            children_start: 0,
            children_len: 0,
            fixed_power_scale: 0,
        }
    }

    #[test]
    fn exact_definition_retains_empty_role_and_measured_work() {
        let mut bridge = RuntimeStandardExactBridge::new();
        let roles = [NativeStandardExactRole {
            role: utf8("self"),
            capabilities_start: 0,
            capabilities_len: 0,
        }];
        let nodes = [literal(7)];
        let handle = bridge
            .admit(NativeStandardExactAdmitRequest {
                domain: utf8("fixture"),
                package: utf8("exact-bridge"),
                package_version: 1,
                subject: utf8("fixture.exact"),
                source: utf8("fixture-source"),
                source_path: utf8("rules/exact"),
                has_provenance_line: false,
                provenance_line: 0,
                has_provenance_column: false,
                provenance_column: 0,
                roles: roles.as_ptr(),
                roles_len: roles.len(),
                capabilities: std::ptr::null(),
                capabilities_len: 0,
                nodes: nodes.as_ptr(),
                nodes_len: nodes.len(),
                child_indices: std::ptr::null(),
                child_indices_len: 0,
                root_node_index: 0,
            })
            .expect("canonical exact admission");
        let readout = bridge.read(handle).expect("exact readout");
        assert_eq!(readout.roles_len, 1);
        let role = unsafe { &*readout.roles };
        assert_eq!(role.capabilities_len, 0);
        assert!(bridge.destroy_readout_lease(readout.handle));
        let evaluation = bridge
            .evaluate(NativeStandardExactEvaluateRequest {
                definition: handle,
                evidence: std::ptr::null(),
                evidence_len: 0,
            })
            .expect("exact evaluation");
        let result = unsafe { &*evaluation.results };
        assert_eq!((result.value, result.work_used), (7, 1));
        assert!(bridge.destroy_evaluation_lease(evaluation.handle));
        assert!(bridge.destroy(handle));
    }

    #[test]
    fn aggregate_child_indices_allow_a_nonzero_span_for_nonconsecutive_subtree_roots() {
        let mut nodes = vec![
            literal(2),
            literal(3),
            literal(0),
            literal(4),
            literal(0),
            literal(0),
        ];
        nodes[2].kind = NativeStandardExactNodeKind::Add;
        nodes[2].left = 0;
        nodes[2].right = 1;
        nodes[4].kind = NativeStandardExactNodeKind::Min;
        nodes[4].children_start = 0;
        nodes[4].children_len = 1;
        nodes[5].kind = NativeStandardExactNodeKind::Max;
        nodes[5].children_start = 1;
        nodes[5].children_len = 2;
        let child_indices = [2, 4, 3];
        validate_flat_shape(&nodes, &child_indices, &[5]).expect("fully connected flat tree");
        let expression = build_expression(&nodes, &child_indices, 5).expect("flat aggregate tree");
        let receipt = ExactEvaluator::evaluate_with_receipt(
            &expression,
            &ExactInputBundle::empty(),
            Default::default(),
        )
        .expect("evaluate aggregate");
        assert_eq!(receipt.value().get(), 5);
    }

    #[test]
    fn flat_shape_rejects_unused_nodes_and_child_indices() {
        let nodes = [literal(1), literal(2)];
        let error = validate_flat_shape(&nodes, &[], &[0]).expect_err("unused node rejected");
        assert!(matches!(
            error,
            StandardExactOperationError::Request {
                code: "STANDARD_EXACT_UNUSED_NODE",
                ..
            }
        ));
        let mut aggregate = [literal(1), literal(0)];
        aggregate[1].kind = NativeStandardExactNodeKind::Min;
        aggregate[1].children_len = 1;
        let error = validate_flat_shape(&aggregate, &[0, 0], &[1])
            .expect_err("unused child index rejected");
        assert!(matches!(
            error,
            StandardExactOperationError::Request {
                code: "STANDARD_EXACT_UNUSED_CHILD_INDEX",
                ..
            }
        ));
    }
}
