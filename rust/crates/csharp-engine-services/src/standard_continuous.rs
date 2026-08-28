//! NativeAOT adapter for canonical `gameplay-standard` continuous semantics.

use std::{collections::BTreeMap, ffi::c_void};

use csharp_engine_abi::*;
use gameplay_rules::{
    RuleDomainId, RulePackageId, RulePackageSchemaVersion, RuleProvenance, RuleSource,
    RuleSourceId, RuleSubjectId, RuleVersion,
};
use gameplay_standard::{
    admit_continuous_definition, AdmittedContinuousDefinition, CapabilityRequirementId,
    CapabilityRoleId, ContinuousComparison, ContinuousDefinition, ContinuousEvaluationError,
    ContinuousEvaluator, ContinuousExpr, ContinuousExprLimits, ContinuousExprRequirements,
    ContinuousInputBundle, ContinuousInputBundleError, ContinuousInputReference, ContinuousValue,
    ContinuousValueError, RoleRequirement, StandardDefinitionError, StandardPackageContext,
};

use crate::composition::{borrowed_slice, borrowed_utf8, ABI_OK};

const SERVICE: &[u8] = b"StandardContinuous";
const ADMIT: &[u8] = b"Admit";
const EVALUATE: &[u8] = b"Evaluate";

pub(crate) struct RuntimeStandardContinuousBridge {
    definitions: BTreeMap<u64, AdmittedContinuousDefinition>,
    next_definition: u64,
    predicates: BTreeMap<u64, ContinuousComparison>,
    next_predicate: u64,
    readout_leases: BTreeMap<u64, ReadoutBacking>,
    next_readout_lease: u64,
    evaluation_leases: BTreeMap<u64, EvaluationBacking>,
    next_evaluation_lease: u64,
    predicate_readout_leases: BTreeMap<u64, PredicateReadoutBacking>,
    next_predicate_readout_lease: u64,
    predicate_evaluation_leases: BTreeMap<u64, PredicateEvaluationBacking>,
    next_predicate_evaluation_lease: u64,
    diagnostics: BTreeMap<u64, DiagnosticBacking>,
    next_diagnostic: u64,
}
struct ReadoutBacking {
    _text: Vec<String>,
    definitions: Vec<NativeStandardContinuousDefinitionReadoutRow>,
    roles: Vec<NativeStandardContinuousRoleRequirementRow>,
    capabilities: Vec<NativeStandardContinuousCapabilityRequirementRow>,
    inputs: Vec<NativeStandardContinuousInputRequirementRow>,
}
struct EvaluationBacking {
    results: Vec<NativeStandardContinuousEvaluationRow>,
}
struct PredicateReadoutBacking {
    _text: Vec<String>,
    predicates: Vec<NativeStandardContinuousPredicateReadoutRow>,
    inputs: Vec<NativeStandardContinuousInputRequirementRow>,
}
struct PredicateEvaluationBacking {
    results: Vec<NativeStandardContinuousPredicateEvaluationRow>,
}
struct DiagnosticBacking {
    _values: Vec<DiagnosticValue>,
    rows: Vec<NativeEngineDiagnostic>,
}
struct DiagnosticValue {
    code: String,
    message: String,
    source: String,
}
struct Text {
    values: Vec<String>,
}
#[derive(Debug)]
enum Error {
    Request(&'static str, String),
    Definition(StandardDefinitionError),
    Role(gameplay_standard::RoleRequirementError),
    Evidence(ContinuousInputBundleError),
    Evaluation(ContinuousEvaluationError),
    Value(ContinuousValueError),
    UnknownDefinition(u64),
    UnknownPredicate(u64),
    Lease(&'static str),
}

impl Text {
    fn copy(&mut self, value: &str) -> NativeUtf8Slice {
        self.values.push(value.to_owned());
        let value = self.values.last().expect("text");
        NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        }
    }
}
impl RuntimeStandardContinuousBridge {
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
            diagnostics: BTreeMap::new(),
            next_diagnostic: 1,
        }
    }
    fn admit(
        &mut self,
        request: NativeStandardContinuousAdmitRequest,
    ) -> Result<NativeStandardContinuousDefinitionHandle, Error> {
        let definition = parse_definition(request)?;
        let value = take_next(&mut self.next_definition, "standardContinuous.definition")?;
        self.definitions.insert(value, definition);
        Ok(NativeStandardContinuousDefinitionHandle { value })
    }
    fn destroy(&mut self, handle: NativeStandardContinuousDefinitionHandle) -> bool {
        handle.value != 0 && self.definitions.remove(&handle.value).is_some()
    }
    fn read(
        &mut self,
        handle: NativeStandardContinuousDefinitionHandle,
    ) -> Option<NativeStandardContinuousReadoutLease> {
        let definition = self.definitions.get(&handle.value)?;
        let lease_value =
            take_next(&mut self.next_readout_lease, "standardContinuous.readout").ok()?;
        let package = definition.package();
        let identity = package.identity();
        let details = definition.definition();
        let requirements = details.requirements().ok()?;
        let limits = ContinuousExprLimits::default();
        let mut text = Text { values: vec![] };
        let definition_row = NativeStandardContinuousDefinitionReadoutRow {
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
        let mut roles = vec![];
        let mut capabilities = vec![];
        for role in requirements.roles() {
            let start = narrow(capabilities.len())?;
            for capability in role.capabilities() {
                capabilities.push(NativeStandardContinuousCapabilityRequirementRow {
                    capability: text.copy(capability.as_str()),
                });
            }
            roles.push(NativeStandardContinuousRoleRequirementRow {
                role: text.copy(role.role().as_str()),
                capabilities_start: start,
                capabilities_len: narrow(role.capabilities().len())?,
            });
        }
        let inputs = requirements
            .inputs()
            .iter()
            .map(|input| native_input(&mut text, input))
            .collect();
        let backing = ReadoutBacking {
            _text: text.values,
            definitions: vec![definition_row],
            roles,
            capabilities,
            inputs,
        };
        let lease = NativeStandardContinuousReadoutLease {
            handle: NativeStandardContinuousReadoutLeaseHandle { value: lease_value },
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
    fn destroy_readout(&mut self, handle: NativeStandardContinuousReadoutLeaseHandle) -> bool {
        handle.value != 0 && self.readout_leases.remove(&handle.value).is_some()
    }
    fn evaluate(
        &mut self,
        request: NativeStandardContinuousEvaluateRequest,
    ) -> Result<NativeStandardContinuousEvaluationLease, Error> {
        let definition = self
            .definitions
            .get(&request.definition.value)
            .ok_or(Error::UnknownDefinition(request.definition.value))?;
        let bundle = parse_evidence(request.evidence, request.evidence_len)?;
        let receipt = ContinuousEvaluator::evaluate_with_receipt(
            definition.definition().expression(),
            &bundle,
            Default::default(),
        )
        .map_err(Error::Evaluation)?;
        let value = take_next(
            &mut self.next_evaluation_lease,
            "standardContinuous.evaluation",
        )?;
        let backing = EvaluationBacking {
            results: vec![NativeStandardContinuousEvaluationRow {
                value_bits: receipt.value().bits(),
                work_used: narrow(receipt.work_used())
                    .ok_or(Error::Lease("standardContinuous.work"))?,
            }],
        };
        let lease = NativeStandardContinuousEvaluationLease {
            handle: NativeStandardContinuousEvaluationLeaseHandle { value },
            results: backing.results.as_ptr(),
            results_len: backing.results.len(),
        };
        self.evaluation_leases.insert(value, backing);
        Ok(lease)
    }
    fn destroy_evaluation(
        &mut self,
        handle: NativeStandardContinuousEvaluationLeaseHandle,
    ) -> bool {
        handle.value != 0 && self.evaluation_leases.remove(&handle.value).is_some()
    }
    fn admit_predicate(
        &mut self,
        request_value: NativeStandardContinuousPredicateAdmitRequest,
    ) -> Result<NativeStandardContinuousPredicateHandle, Error> {
        let nodes = unsafe {
            borrowed_slice(
                request_value.nodes,
                request_value.nodes_len,
                "standard continuous predicate nodes",
            )
        }
        .map_err(|_| request("STANDARD_CONTINUOUS_NODE_POINTER", "nodes"))?;
        let children = unsafe {
            borrowed_slice(
                request_value.child_indices,
                request_value.child_indices_len,
                "standard continuous predicate child indices",
            )
        }
        .map_err(|_| request("STANDARD_CONTINUOUS_CHILD_POINTER", "child_indices"))?;
        validate_limits(nodes, children)?;
        let left = usize::try_from(request_value.left_node_index)
            .map_err(|_| request("STANDARD_CONTINUOUS_ROOT", "left_node_index"))?;
        let right = usize::try_from(request_value.right_node_index)
            .map_err(|_| request("STANDARD_CONTINUOUS_ROOT", "right_node_index"))?;
        validate_shape(nodes, children, &[left, right])?;
        let left = build(nodes, children, request_value.left_node_index)?;
        let right = build(nodes, children, request_value.right_node_index)?;
        let predicate = match request_value.comparison {
            NativeStandardContinuousComparisonKind::Equal => {
                ContinuousComparison::Equal(left, right)
            }
            NativeStandardContinuousComparisonKind::LessThan => {
                ContinuousComparison::LessThan(left, right)
            }
            NativeStandardContinuousComparisonKind::LessOrEqual => {
                ContinuousComparison::LessOrEqual(left, right)
            }
            NativeStandardContinuousComparisonKind::GreaterThan => {
                ContinuousComparison::GreaterThan(left, right)
            }
            NativeStandardContinuousComparisonKind::GreaterOrEqual => {
                ContinuousComparison::GreaterOrEqual(left, right)
            }
        };
        ContinuousEvaluator::validate_comparison_structure(&predicate, Default::default())
            .map_err(Error::Evaluation)?;
        let value = take_next(&mut self.next_predicate, "standardContinuous.predicate")?;
        self.predicates.insert(value, predicate);
        Ok(NativeStandardContinuousPredicateHandle { value })
    }
    fn destroy_predicate(&mut self, handle: NativeStandardContinuousPredicateHandle) -> bool {
        handle.value != 0 && self.predicates.remove(&handle.value).is_some()
    }
    fn read_predicate(
        &mut self,
        handle: NativeStandardContinuousPredicateHandle,
    ) -> Option<NativeStandardContinuousPredicateReadoutLease> {
        let predicate = self.predicates.get(&handle.value)?;
        let requirements = ContinuousExprRequirements::inspect_comparison(predicate).ok()?;
        let limits = ContinuousExprLimits::default();
        let value = take_next(
            &mut self.next_predicate_readout_lease,
            "standardContinuous.predicateReadout",
        )
        .ok()?;
        let mut text = Text { values: vec![] };
        let inputs = requirements
            .inputs()
            .iter()
            .map(|input| native_input(&mut text, input))
            .collect();
        let backing = PredicateReadoutBacking {
            _text: text.values,
            predicates: vec![NativeStandardContinuousPredicateReadoutRow {
                comparison: native_comparison(predicate),
                maximum_depth: narrow(limits.maximum_depth)?,
                maximum_nodes: narrow(limits.maximum_nodes)?,
                maximum_inputs: narrow(limits.maximum_inputs)?,
                maximum_arity: narrow(limits.maximum_arity)?,
                maximum_work: narrow(limits.maximum_work)?,
            }],
            inputs,
        };
        let lease = NativeStandardContinuousPredicateReadoutLease {
            handle: NativeStandardContinuousPredicateReadoutLeaseHandle { value },
            predicates: backing.predicates.as_ptr(),
            predicates_len: backing.predicates.len(),
            inputs: backing.inputs.as_ptr(),
            inputs_len: backing.inputs.len(),
        };
        self.predicate_readout_leases.insert(value, backing);
        Some(lease)
    }
    fn destroy_predicate_readout(
        &mut self,
        handle: NativeStandardContinuousPredicateReadoutLeaseHandle,
    ) -> bool {
        handle.value != 0
            && self
                .predicate_readout_leases
                .remove(&handle.value)
                .is_some()
    }
    fn evaluate_predicate(
        &mut self,
        request: NativeStandardContinuousEvaluatePredicateRequest,
    ) -> Result<NativeStandardContinuousPredicateEvaluationLease, Error> {
        let predicate = self
            .predicates
            .get(&request.predicate.value)
            .ok_or(Error::UnknownPredicate(request.predicate.value))?;
        let bundle = parse_evidence(request.evidence, request.evidence_len)?;
        let receipt = ContinuousEvaluator::evaluate_predicate_with_receipt(
            predicate,
            &bundle,
            Default::default(),
        )
        .map_err(Error::Evaluation)?;
        let value = take_next(
            &mut self.next_predicate_evaluation_lease,
            "standardContinuous.predicateEvaluation",
        )?;
        let backing = PredicateEvaluationBacking {
            results: vec![NativeStandardContinuousPredicateEvaluationRow {
                value: receipt.value(),
                work_used: narrow(receipt.work_used())
                    .ok_or(Error::Lease("standardContinuous.predicateWork"))?,
            }],
        };
        let lease = NativeStandardContinuousPredicateEvaluationLease {
            handle: NativeStandardContinuousPredicateEvaluationLeaseHandle { value },
            results: backing.results.as_ptr(),
            results_len: backing.results.len(),
        };
        self.predicate_evaluation_leases.insert(value, backing);
        Ok(lease)
    }
    fn destroy_predicate_evaluation(
        &mut self,
        handle: NativeStandardContinuousPredicateEvaluationLeaseHandle,
    ) -> bool {
        handle.value != 0
            && self
                .predicate_evaluation_leases
                .remove(&handle.value)
                .is_some()
    }
    fn diagnostic(&mut self, error: &Error) -> Option<NativeEngineDiagnosticLease> {
        let value = DiagnosticValue::from(error);
        let handle = take_next(&mut self.next_diagnostic, "standardContinuous.diagnostic").ok()?;
        let values = vec![value];
        let rows = values
            .iter()
            .map(|value| NativeEngineDiagnostic {
                code: slice(&value.code),
                message: slice(&value.message),
                source: slice(&value.source),
            })
            .collect();
        let backing = DiagnosticBacking {
            _values: values,
            rows,
        };
        let lease = NativeEngineDiagnosticLease {
            handle: NativeEngineDiagnosticLeaseHandle { value: handle },
            diagnostics: backing.rows.as_ptr(),
            diagnostics_len: backing.rows.len(),
        };
        self.diagnostics.insert(handle, backing);
        Some(lease)
    }
    fn destroy_diagnostic(&mut self, handle: NativeEngineDiagnosticLeaseHandle) -> bool {
        handle.value != 0 && self.diagnostics.remove(&handle.value).is_some()
    }
}

impl DiagnosticValue {
    fn fixed(code: &'static str, message: &'static str, source: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.to_owned(),
            source: source.into(),
        }
    }
    fn from(error: &Error) -> Self {
        match error {
            Error::Request(code, source) => Self::fixed(
                code,
                "Typed StandardContinuous request was invalid.",
                source,
            ),
            Error::UnknownDefinition(value) => Self::fixed(
                "STANDARD_CONTINUOUS_DEFINITION_HANDLE",
                "Continuous definition handle was not retained.",
                value.to_string(),
            ),
            Error::UnknownPredicate(value) => Self::fixed(
                "STANDARD_CONTINUOUS_PREDICATE_HANDLE",
                "Continuous predicate handle was not retained.",
                value.to_string(),
            ),
            Error::Lease(field) => Self::fixed(
                "STANDARD_CONTINUOUS_LEASE",
                "Continuous service handle allocation overflowed.",
                *field,
            ),
            Error::Value(ContinuousValueError::NonFinite { .. }) => Self::fixed(
                "STANDARD_CONTINUOUS_NONFINITE",
                "Continuous binary64 value must be finite.",
                "value_bits",
            ),
            Error::Value(ContinuousValueError::DivisionByZero) => Self::fixed(
                "STANDARD_CONTINUOUS_DIVIDE_ZERO",
                "Continuous division denominator was zero.",
                "evaluation",
            ),
            Error::Evidence(ContinuousInputBundleError::DuplicateInput { .. }) => Self::fixed(
                "STANDARD_CONTINUOUS_DUPLICATE_INPUT",
                "Every duplicate continuous input observation is rejected.",
                "evidence",
            ),
            Error::Evaluation(error) => evaluation_diagnostic(error),
            Error::Role(error) => role_diagnostic(error),
            Error::Definition(error) => definition_diagnostic(error),
        }
    }
}
fn role_diagnostic(error: &gameplay_standard::RoleRequirementError) -> DiagnosticValue {
    match error {
        gameplay_standard::RoleRequirementError::InvalidRoleId { .. } => DiagnosticValue::fixed(
            "STANDARD_CONTINUOUS_ROLE",
            "Role identity was invalid.",
            "roles",
        ),
        gameplay_standard::RoleRequirementError::InvalidCapabilityId { .. } => {
            DiagnosticValue::fixed(
                "STANDARD_CONTINUOUS_CAPABILITY",
                "Capability identity was invalid.",
                "roles",
            )
        }
        gameplay_standard::RoleRequirementError::InvalidInputId { .. } => DiagnosticValue::fixed(
            "STANDARD_CONTINUOUS_INPUT_ID",
            "Input identity was invalid.",
            "nodes",
        ),
        gameplay_standard::RoleRequirementError::CapabilityQuotaExceeded { .. } => {
            DiagnosticValue::fixed(
                "STANDARD_CONTINUOUS_CAPABILITY_QUOTA",
                "Role capability quota was exceeded.",
                "roles",
            )
        }
        gameplay_standard::RoleRequirementError::NonCanonicalCapabilities => {
            DiagnosticValue::fixed(
                "STANDARD_CONTINUOUS_CAPABILITY_ORDER",
                "Capabilities must be canonical.",
                "roles",
            )
        }
    }
}
fn definition_diagnostic(error: &StandardDefinitionError) -> DiagnosticValue {
    match error {
        StandardDefinitionError::Package(_) => DiagnosticValue::fixed(
            "STANDARD_CONTINUOUS_PACKAGE",
            "Canonical continuous package admission failed.",
            "package",
        ),
        StandardDefinitionError::Role(error) => role_diagnostic(error),
        StandardDefinitionError::ContinuousLiteral(error) => match error {
            ContinuousValueError::NonFinite { .. } => DiagnosticValue::fixed(
                "STANDARD_CONTINUOUS_NONFINITE",
                "Continuous literal bits were nonfinite.",
                "nodes",
            ),
            ContinuousValueError::DivisionByZero => DiagnosticValue::fixed(
                "STANDARD_CONTINUOUS_DIVIDE_ZERO",
                "Continuous literal was invalid.",
                "nodes",
            ),
        },
        StandardDefinitionError::ContinuousStructure(error) => evaluation_diagnostic(error),
        StandardDefinitionError::UndeclaredInputRole { role } => DiagnosticValue::fixed(
            "STANDARD_CONTINUOUS_UNDECLARED_ROLE",
            "Input role was not declared.",
            role.as_str(),
        ),
        StandardDefinitionError::MissingCorrelation { .. }
        | StandardDefinitionError::SourceMismatch { .. } => DiagnosticValue::fixed(
            "STANDARD_CONTINUOUS_CORRELATION",
            "Source/provenance correlation failed.",
            "provenance",
        ),
        StandardDefinitionError::WrongSchema { .. } => DiagnosticValue::fixed(
            "STANDARD_CONTINUOUS_SCHEMA",
            "Continuous definitions require Binary64V2.",
            "package",
        ),
        StandardDefinitionError::WrongFamily { .. }
        | StandardDefinitionError::UnsupportedSemanticsVersion { .. }
        | StandardDefinitionError::MalformedPayload { .. }
        | StandardDefinitionError::NonConvergentPayload => DiagnosticValue::fixed(
            "STANDARD_CONTINUOUS_CANONICAL",
            "Canonical continuous definition admission failed.",
            "package",
        ),
        StandardDefinitionError::ExactLiteral { .. }
        | StandardDefinitionError::ExactStructure(_) => DiagnosticValue::fixed(
            "STANDARD_CONTINUOUS_OWNER",
            "Unexpected exact owner error.",
            "owner",
        ),
    }
}
fn evaluation_diagnostic(error: &ContinuousEvaluationError) -> DiagnosticValue {
    match error {
        ContinuousEvaluationError::Value(ContinuousValueError::NonFinite { .. }) => {
            DiagnosticValue::fixed(
                "STANDARD_CONTINUOUS_NONFINITE",
                "Continuous operation produced a nonfinite value.",
                "evaluation",
            )
        }
        ContinuousEvaluationError::Value(ContinuousValueError::DivisionByZero) => {
            DiagnosticValue::fixed(
                "STANDARD_CONTINUOUS_DIVIDE_ZERO",
                "Continuous division denominator was zero.",
                "evaluation",
            )
        }
        ContinuousEvaluationError::MissingInput { .. } => DiagnosticValue::fixed(
            "STANDARD_CONTINUOUS_MISSING_INPUT",
            "Continuous evaluation lacked a required input.",
            "evidence",
        ),
        ContinuousEvaluationError::DepthExceeded { .. } => DiagnosticValue::fixed(
            "STANDARD_CONTINUOUS_DEPTH_QUOTA",
            "Continuous depth quota exceeded.",
            "nodes",
        ),
        ContinuousEvaluationError::NodeQuotaExceeded { .. } => DiagnosticValue::fixed(
            "STANDARD_CONTINUOUS_NODE_QUOTA",
            "Continuous node quota exceeded.",
            "nodes",
        ),
        ContinuousEvaluationError::InputQuotaExceeded { .. } => DiagnosticValue::fixed(
            "STANDARD_CONTINUOUS_INPUT_QUOTA",
            "Continuous input quota exceeded.",
            "nodes",
        ),
        ContinuousEvaluationError::ArityExceeded { .. } => DiagnosticValue::fixed(
            "STANDARD_CONTINUOUS_ARITY_QUOTA",
            "Continuous aggregate arity quota exceeded.",
            "nodes",
        ),
        ContinuousEvaluationError::EmptyAggregate => DiagnosticValue::fixed(
            "STANDARD_CONTINUOUS_EMPTY_AGGREGATE",
            "Continuous aggregate must not be empty.",
            "nodes",
        ),
        ContinuousEvaluationError::WorkQuotaExceeded { .. } => DiagnosticValue::fixed(
            "STANDARD_CONTINUOUS_WORK_QUOTA",
            "Continuous work quota exceeded.",
            "evaluation",
        ),
        ContinuousEvaluationError::Role(error) => role_diagnostic(error),
    }
}

fn parse_definition(
    request_value: NativeStandardContinuousAdmitRequest,
) -> Result<AdmittedContinuousDefinition, Error> {
    let domain = rule_id(request_value.domain, "domain", RuleDomainId::parse)?;
    let package = rule_id(request_value.package, "package", RulePackageId::parse)?;
    let version = RuleVersion::new(request_value.package_version)
        .map_err(|_| request("STANDARD_CONTINUOUS_PACKAGE_VERSION", "package_version"))?;
    let subject = rule_id(request_value.subject, "subject", RuleSubjectId::parse)?;
    let source = rule_id(request_value.source, "source", RuleSourceId::parse)?;
    let source_path = text(request_value.source_path, "source_path")?;
    let roles = unsafe {
        borrowed_slice(
            request_value.roles,
            request_value.roles_len,
            "standard continuous roles",
        )
    }
    .map_err(|_| request("STANDARD_CONTINUOUS_ROLE_POINTER", "roles"))?;
    let capabilities = unsafe {
        borrowed_slice(
            request_value.capabilities,
            request_value.capabilities_len,
            "standard continuous capabilities",
        )
    }
    .map_err(|_| request("STANDARD_CONTINUOUS_CAPABILITY_POINTER", "capabilities"))?;
    let nodes = unsafe {
        borrowed_slice(
            request_value.nodes,
            request_value.nodes_len,
            "standard continuous nodes",
        )
    }
    .map_err(|_| request("STANDARD_CONTINUOUS_NODE_POINTER", "nodes"))?;
    let children = unsafe {
        borrowed_slice(
            request_value.child_indices,
            request_value.child_indices_len,
            "standard continuous child indices",
        )
    }
    .map_err(|_| request("STANDARD_CONTINUOUS_CHILD_POINTER", "child_indices"))?;
    validate_limits(nodes, children)?;
    let root = usize::try_from(request_value.root_node_index)
        .map_err(|_| request("STANDARD_CONTINUOUS_ROOT", "root_node_index"))?;
    validate_shape(nodes, children, &[root])?;
    let expression = build(nodes, children, request_value.root_node_index)?;
    let roles = roles
        .iter()
        .map(|role| parse_role(role, capabilities))
        .collect::<Result<Vec<_>, _>>()?;
    let definition = ContinuousDefinition::new(subject.clone(), source.clone(), expression, roles)
        .map_err(Error::Definition)?;
    let context = StandardPackageContext::new(
        RulePackageSchemaVersion::Binary64V2,
        domain,
        package,
        version,
        vec![],
        vec![RuleSource::new(source.clone(), source_path)
            .map_err(|_| request("STANDARD_CONTINUOUS_SOURCE_PATH", "source_path"))?],
        vec![RuleProvenance::new(
            subject,
            source,
            request_value
                .has_provenance_line
                .then_some(request_value.provenance_line),
            request_value
                .has_provenance_column
                .then_some(request_value.provenance_column),
        )
        .map_err(|_| request("STANDARD_CONTINUOUS_PROVENANCE", "provenance"))?],
    );
    admit_continuous_definition(&context, definition).map_err(Error::Definition)
}
fn parse_evidence(
    pointer: *const NativeStandardContinuousEvidence,
    len: usize,
) -> Result<ContinuousInputBundle, Error> {
    let rows = unsafe { borrowed_slice(pointer, len, "standard continuous evidence") }
        .map_err(|_| request("STANDARD_CONTINUOUS_EVIDENCE_POINTER", "evidence"))?;
    if rows.len() > ContinuousExprLimits::default().maximum_inputs {
        return Err(request("STANDARD_CONTINUOUS_EVIDENCE_QUOTA", "evidence"));
    }
    ContinuousInputBundle::new(
        rows.iter()
            .map(|row| {
                Ok((
                    parse_input(row.kind, row.role, row.input_id)?,
                    ContinuousValue::from_bits(row.value_bits).map_err(Error::Value)?,
                ))
            })
            .collect::<Result<Vec<_>, Error>>()?,
    )
    .map_err(Error::Evidence)
}
fn parse_role(
    row: &NativeStandardContinuousRole,
    capabilities: &[NativeStandardContinuousCapability],
) -> Result<RoleRequirement, Error> {
    let role = CapabilityRoleId::parse(text(row.role, "role")?)
        .map_err(|_| request("STANDARD_CONTINUOUS_ROLE", "roles"))?;
    let start = usize::try_from(row.capabilities_start)
        .map_err(|_| request("STANDARD_CONTINUOUS_CAPABILITY_RANGE", "roles"))?;
    let end = start
        .checked_add(
            usize::try_from(row.capabilities_len)
                .map_err(|_| request("STANDARD_CONTINUOUS_CAPABILITY_RANGE", "roles"))?,
        )
        .ok_or_else(|| request("STANDARD_CONTINUOUS_CAPABILITY_RANGE", "roles"))?;
    let capabilities = capabilities
        .get(start..end)
        .ok_or_else(|| request("STANDARD_CONTINUOUS_CAPABILITY_RANGE", "roles"))?
        .iter()
        .map(|value| {
            CapabilityRequirementId::parse(text(value.capability, "capability")?)
                .map_err(|_| request("STANDARD_CONTINUOUS_CAPABILITY", "roles"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    RoleRequirement::new(role, capabilities).map_err(Error::Role)
}
fn build(
    nodes: &[NativeStandardContinuousNode],
    children: &[u32],
    root: u32,
) -> Result<ContinuousExpr, Error> {
    let root = usize::try_from(root)
        .map_err(|_| request("STANDARD_CONTINUOUS_ROOT", "root_node_index"))?;
    if root >= nodes.len() {
        return Err(request("STANDARD_CONTINUOUS_ROOT", "root_node_index"));
    }
    let mut built = Vec::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        let child = |value: u32| -> Result<ContinuousExpr, Error> {
            let value = usize::try_from(value)
                .map_err(|_| request("STANDARD_CONTINUOUS_NODE_INDEX", "nodes"))?;
            if value >= index {
                return Err(request("STANDARD_CONTINUOUS_NODE_ORDER", "nodes"));
            }
            built
                .get(value)
                .cloned()
                .ok_or_else(|| request("STANDARD_CONTINUOUS_NODE_INDEX", "nodes"))
        };
        let expr = match node.kind {
            NativeStandardContinuousNodeKind::Literal => ContinuousExpr::Literal(
                ContinuousValue::from_bits(node.literal_bits).map_err(Error::Value)?,
            ),
            NativeStandardContinuousNodeKind::Input => {
                ContinuousExpr::Input(parse_input(node.input_kind, node.role, node.input_id)?)
            }
            NativeStandardContinuousNodeKind::Add => {
                ContinuousExpr::Add(Box::new(child(node.left)?), Box::new(child(node.right)?))
            }
            NativeStandardContinuousNodeKind::Subtract => {
                ContinuousExpr::Subtract(Box::new(child(node.left)?), Box::new(child(node.right)?))
            }
            NativeStandardContinuousNodeKind::Multiply => {
                ContinuousExpr::Multiply(Box::new(child(node.left)?), Box::new(child(node.right)?))
            }
            NativeStandardContinuousNodeKind::Divide => {
                ContinuousExpr::Divide(Box::new(child(node.left)?), Box::new(child(node.right)?))
            }
            NativeStandardContinuousNodeKind::Min | NativeStandardContinuousNodeKind::Max => {
                let start = usize::try_from(node.children_start)
                    .map_err(|_| request("STANDARD_CONTINUOUS_CHILD_RANGE", "nodes"))?;
                let end = start
                    .checked_add(
                        usize::try_from(node.children_len)
                            .map_err(|_| request("STANDARD_CONTINUOUS_CHILD_RANGE", "nodes"))?,
                    )
                    .ok_or_else(|| request("STANDARD_CONTINUOUS_CHILD_RANGE", "nodes"))?;
                let values = children
                    .get(start..end)
                    .ok_or_else(|| request("STANDARD_CONTINUOUS_CHILD_RANGE", "nodes"))?
                    .iter()
                    .map(|value| child(*value))
                    .collect::<Result<Vec<_>, _>>()?;
                if node.kind == NativeStandardContinuousNodeKind::Min {
                    ContinuousExpr::Min(values)
                } else {
                    ContinuousExpr::Max(values)
                }
            }
        };
        built.push(expr);
    }
    Ok(built.swap_remove(root))
}
fn validate_limits(nodes: &[NativeStandardContinuousNode], children: &[u32]) -> Result<(), Error> {
    let limits = ContinuousExprLimits::default();
    if nodes.len() > limits.maximum_nodes {
        return Err(request("STANDARD_CONTINUOUS_NODE_QUOTA", "nodes"));
    }
    if children.len()
        > nodes
            .len()
            .checked_mul(limits.maximum_arity)
            .ok_or_else(|| request("STANDARD_CONTINUOUS_CHILD_QUOTA", "child_indices"))?
    {
        return Err(request("STANDARD_CONTINUOUS_CHILD_QUOTA", "child_indices"));
    }
    Ok(())
}
fn validate_shape(
    nodes: &[NativeStandardContinuousNode],
    children: &[u32],
    roots: &[usize],
) -> Result<(), Error> {
    let mut reached = vec![false; nodes.len()];
    let mut used = vec![false; children.len()];
    for root in roots {
        visit(*root, nodes, children, &mut reached, &mut used)?;
    }
    if reached.iter().any(|value| !value) {
        return Err(request("STANDARD_CONTINUOUS_UNUSED_NODE", "nodes"));
    }
    if used.iter().any(|value| !value) {
        return Err(request(
            "STANDARD_CONTINUOUS_UNUSED_CHILD_INDEX",
            "child_indices",
        ));
    }
    Ok(())
}
fn visit(
    index: usize,
    nodes: &[NativeStandardContinuousNode],
    children: &[u32],
    reached: &mut [bool],
    used: &mut [bool],
) -> Result<(), Error> {
    let node = nodes
        .get(index)
        .ok_or_else(|| request("STANDARD_CONTINUOUS_NODE_INDEX", "nodes"))?;
    if reached[index] {
        return Ok(());
    }
    reached[index] = true;
    let visit_child = |child: u32, reached: &mut [bool], used: &mut [bool]| -> Result<(), Error> {
        let child = usize::try_from(child)
            .map_err(|_| request("STANDARD_CONTINUOUS_NODE_INDEX", "nodes"))?;
        if child >= index {
            return Err(request("STANDARD_CONTINUOUS_NODE_ORDER", "nodes"));
        }
        visit(child, nodes, children, reached, used)
    };
    match node.kind {
        NativeStandardContinuousNodeKind::Literal | NativeStandardContinuousNodeKind::Input => {
            Ok(())
        }
        NativeStandardContinuousNodeKind::Add
        | NativeStandardContinuousNodeKind::Subtract
        | NativeStandardContinuousNodeKind::Multiply
        | NativeStandardContinuousNodeKind::Divide => {
            visit_child(node.left, reached, used)?;
            visit_child(node.right, reached, used)
        }
        NativeStandardContinuousNodeKind::Min | NativeStandardContinuousNodeKind::Max => {
            let start = usize::try_from(node.children_start)
                .map_err(|_| request("STANDARD_CONTINUOUS_CHILD_RANGE", "nodes"))?;
            let end = start
                .checked_add(
                    usize::try_from(node.children_len)
                        .map_err(|_| request("STANDARD_CONTINUOUS_CHILD_RANGE", "nodes"))?,
                )
                .ok_or_else(|| request("STANDARD_CONTINUOUS_CHILD_RANGE", "nodes"))?;
            for (offset, child) in children
                .get(start..end)
                .ok_or_else(|| request("STANDARD_CONTINUOUS_CHILD_RANGE", "nodes"))?
                .iter()
                .enumerate()
            {
                used[start + offset] = true;
                visit_child(*child, reached, used)?;
            }
            Ok(())
        }
    }
}
fn parse_input(
    kind: NativeStandardContinuousInputKind,
    role: NativeUtf8Slice,
    id: NativeUtf8Slice,
) -> Result<ContinuousInputReference, Error> {
    let role = CapabilityRoleId::parse(text(role, "role")?)
        .map_err(|_| request("STANDARD_CONTINUOUS_ROLE", "nodes"))?;
    let id = gameplay_standard::InputId::parse(text(id, "input_id")?)
        .map_err(|_| request("STANDARD_CONTINUOUS_INPUT_ID", "nodes"))?;
    Ok(match kind {
        NativeStandardContinuousInputKind::Parameter => {
            ContinuousInputReference::Parameter { role, id }
        }
        NativeStandardContinuousInputKind::Fact => ContinuousInputReference::Fact { role, id },
        NativeStandardContinuousInputKind::Roll => ContinuousInputReference::Roll { role, id },
        NativeStandardContinuousInputKind::Choice => ContinuousInputReference::Choice { role, id },
    })
}
fn native_input(
    text: &mut Text,
    input: &ContinuousInputReference,
) -> NativeStandardContinuousInputRequirementRow {
    let (kind, role, id) = match input {
        ContinuousInputReference::Parameter { role, id } => (
            NativeStandardContinuousInputKind::Parameter,
            role.as_str(),
            id.as_str(),
        ),
        ContinuousInputReference::Fact { role, id } => (
            NativeStandardContinuousInputKind::Fact,
            role.as_str(),
            id.as_str(),
        ),
        ContinuousInputReference::Roll { role, id } => (
            NativeStandardContinuousInputKind::Roll,
            role.as_str(),
            id.as_str(),
        ),
        ContinuousInputReference::Choice { role, id } => (
            NativeStandardContinuousInputKind::Choice,
            role.as_str(),
            id.as_str(),
        ),
    };
    NativeStandardContinuousInputRequirementRow {
        kind,
        role: text.copy(role),
        input_id: text.copy(id),
    }
}
fn native_comparison(value: &ContinuousComparison) -> NativeStandardContinuousComparisonKind {
    match value {
        ContinuousComparison::Equal(_, _) => NativeStandardContinuousComparisonKind::Equal,
        ContinuousComparison::LessThan(_, _) => NativeStandardContinuousComparisonKind::LessThan,
        ContinuousComparison::LessOrEqual(_, _) => {
            NativeStandardContinuousComparisonKind::LessOrEqual
        }
        ContinuousComparison::GreaterThan(_, _) => {
            NativeStandardContinuousComparisonKind::GreaterThan
        }
        ContinuousComparison::GreaterOrEqual(_, _) => {
            NativeStandardContinuousComparisonKind::GreaterOrEqual
        }
    }
}
fn text(value: NativeUtf8Slice, field: &'static str) -> Result<String, Error> {
    unsafe { borrowed_utf8(value.bytes, value.len, field) }
        .map(|value| value.to_owned())
        .map_err(|_| request("STANDARD_CONTINUOUS_UTF8", field))
}
fn rule_id<T>(
    value: NativeUtf8Slice,
    field: &'static str,
    parse: impl FnOnce(String) -> Result<T, gameplay_rules::RulePackageError>,
) -> Result<T, Error> {
    parse(text(value, field)?).map_err(|_| request("STANDARD_CONTINUOUS_PACKAGE_IDENTITY", field))
}
fn request(code: &'static str, source: impl Into<String>) -> Error {
    Error::Request(code, source.into())
}
fn take_next(value: &mut u64, field: &'static str) -> Result<u64, Error> {
    let result = *value;
    *value = result.checked_add(1).ok_or(Error::Lease(field))?;
    Ok(result)
}
fn narrow(value: usize) -> Option<u32> {
    u32::try_from(value).ok()
}
fn slice(value: &str) -> NativeUtf8Slice {
    NativeUtf8Slice {
        bytes: value.as_ptr(),
        len: value.len(),
    }
}

pub(crate) fn api(bridge: &mut RuntimeStandardContinuousBridge) -> NativeStandardContinuousApi {
    NativeStandardContinuousApi {
        context: (bridge as *mut RuntimeStandardContinuousBridge).cast(),
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
    request_value: *const NativeStandardContinuousAdmitRequest,
    result: *mut NativeStandardContinuousDefinitionHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || request_value.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeStandardContinuousBridge>() };
    match bridge.admit(unsafe { *request_value }) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            write_error(bridge, receipt, ADMIT, &error);
            0
        }
    }
}
unsafe extern "C" fn destroy_definition(
    context: *mut c_void,
    handle: NativeStandardContinuousDefinitionHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    i32::from(unsafe { (&mut *context.cast::<RuntimeStandardContinuousBridge>()).destroy(handle) })
}
unsafe extern "C" fn read_definition(
    context: *mut c_void,
    handle: NativeStandardContinuousDefinitionHandle,
    result: *mut NativeStandardContinuousReadoutLease,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    match unsafe { (&mut *context.cast::<RuntimeStandardContinuousBridge>()).read(handle) } {
        Some(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        None => 0,
    }
}
unsafe extern "C" fn destroy_readout_lease(
    context: *mut c_void,
    handle: NativeStandardContinuousReadoutLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    i32::from(unsafe {
        (&mut *context.cast::<RuntimeStandardContinuousBridge>()).destroy_readout(handle)
    })
}
unsafe extern "C" fn evaluate(
    context: *mut c_void,
    request_value: *const NativeStandardContinuousEvaluateRequest,
    result: *mut NativeStandardContinuousEvaluationLease,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || request_value.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeStandardContinuousBridge>() };
    match bridge.evaluate(unsafe { *request_value }) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            write_error(bridge, receipt, EVALUATE, &error);
            0
        }
    }
}
unsafe extern "C" fn destroy_evaluation_lease(
    context: *mut c_void,
    handle: NativeStandardContinuousEvaluationLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    i32::from(unsafe {
        (&mut *context.cast::<RuntimeStandardContinuousBridge>()).destroy_evaluation(handle)
    })
}
unsafe extern "C" fn destroy_operation_diagnostic_lease(
    context: *mut c_void,
    handle: NativeEngineDiagnosticLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    i32::from(unsafe {
        (&mut *context.cast::<RuntimeStandardContinuousBridge>()).destroy_diagnostic(handle)
    })
}
unsafe extern "C" fn admit_predicate(
    context: *mut c_void,
    request_value: *const NativeStandardContinuousPredicateAdmitRequest,
    result: *mut NativeStandardContinuousPredicateHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || request_value.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeStandardContinuousBridge>() };
    match bridge.admit_predicate(unsafe { *request_value }) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            write_error(bridge, receipt, b"AdmitPredicate", &error);
            0
        }
    }
}
unsafe extern "C" fn destroy_predicate(
    context: *mut c_void,
    handle: NativeStandardContinuousPredicateHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    i32::from(unsafe {
        (&mut *context.cast::<RuntimeStandardContinuousBridge>()).destroy_predicate(handle)
    })
}
unsafe extern "C" fn read_predicate(
    context: *mut c_void,
    handle: NativeStandardContinuousPredicateHandle,
    result: *mut NativeStandardContinuousPredicateReadoutLease,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    match unsafe {
        (&mut *context.cast::<RuntimeStandardContinuousBridge>()).read_predicate(handle)
    } {
        Some(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        None => 0,
    }
}
unsafe extern "C" fn destroy_predicate_readout_lease(
    context: *mut c_void,
    handle: NativeStandardContinuousPredicateReadoutLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    i32::from(unsafe {
        (&mut *context.cast::<RuntimeStandardContinuousBridge>()).destroy_predicate_readout(handle)
    })
}
unsafe extern "C" fn evaluate_predicate(
    context: *mut c_void,
    request_value: *const NativeStandardContinuousEvaluatePredicateRequest,
    result: *mut NativeStandardContinuousPredicateEvaluationLease,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || request_value.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeStandardContinuousBridge>() };
    match bridge.evaluate_predicate(unsafe { *request_value }) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            write_error(bridge, receipt, b"EvaluatePredicate", &error);
            0
        }
    }
}
unsafe extern "C" fn destroy_predicate_evaluation_lease(
    context: *mut c_void,
    handle: NativeStandardContinuousPredicateEvaluationLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    i32::from(unsafe {
        (&mut *context.cast::<RuntimeStandardContinuousBridge>())
            .destroy_predicate_evaluation(handle)
    })
}
fn write_error(
    bridge: &mut RuntimeStandardContinuousBridge,
    receipt: *mut NativeOperationErrorReceipt,
    operation: &[u8],
    error: &Error,
) {
    if let Some(diagnostics) = bridge.diagnostic(error) {
        unsafe {
            *receipt = NativeOperationErrorReceipt {
                service: slice_bytes(SERVICE),
                operation: slice_bytes(operation),
                status: 0,
                diagnostics,
            }
        }
    }
}
fn slice_bytes(value: &[u8]) -> NativeUtf8Slice {
    NativeUtf8Slice {
        bytes: value.as_ptr(),
        len: value.len(),
    }
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
    fn literal(bits: u64) -> NativeStandardContinuousNode {
        NativeStandardContinuousNode {
            kind: NativeStandardContinuousNodeKind::Literal,
            literal_bits: bits,
            input_kind: NativeStandardContinuousInputKind::Parameter,
            role: utf8(""),
            input_id: utf8(""),
            left: 0,
            right: 0,
            children_start: 0,
            children_len: 0,
        }
    }

    #[test]
    fn preserves_subnormal_and_normalizes_negative_zero_through_owner() {
        let subnormal = build(&[literal(1)], &[], 0).expect("subnormal literal");
        let receipt = ContinuousEvaluator::evaluate_with_receipt(
            &subnormal,
            &ContinuousInputBundle::new(vec![]).unwrap(),
            Default::default(),
        )
        .unwrap();
        assert_eq!(receipt.value().bits(), 1);
        let negative_zero =
            build(&[literal((-0.0f64).to_bits())], &[], 0).expect("negative zero literal");
        let receipt = ContinuousEvaluator::evaluate_with_receipt(
            &negative_zero,
            &ContinuousInputBundle::new(vec![]).unwrap(),
            Default::default(),
        )
        .unwrap();
        assert_eq!(receipt.value().bits(), 0.0f64.to_bits());
    }

    #[test]
    fn rejects_identical_duplicate_evidence_without_bridge_deduplication() {
        let rows = [
            NativeStandardContinuousEvidence {
                kind: NativeStandardContinuousInputKind::Parameter,
                role: utf8("self"),
                input_id: utf8("rate"),
                value_bits: 0x3ff0_0000_0000_0000,
            },
            NativeStandardContinuousEvidence {
                kind: NativeStandardContinuousInputKind::Parameter,
                role: utf8("self"),
                input_id: utf8("rate"),
                value_bits: 0x3ff0_0000_0000_0000,
            },
        ];
        assert!(matches!(
            parse_evidence(rows.as_ptr(), rows.len()),
            Err(Error::Evidence(
                ContinuousInputBundleError::DuplicateInput { .. }
            ))
        ));
    }

    #[test]
    fn accepts_nonzero_aggregate_child_span_without_unreachable_rows() {
        let mut nodes = vec![
            literal(0x4000_0000_0000_0000),
            literal(0x4008_0000_0000_0000),
            literal(0),
            literal(0x4010_0000_0000_0000),
            literal(0),
            literal(0),
        ];
        nodes[2].kind = NativeStandardContinuousNodeKind::Add;
        nodes[2].left = 0;
        nodes[2].right = 1;
        nodes[4].kind = NativeStandardContinuousNodeKind::Min;
        nodes[4].children_len = 1;
        nodes[5].kind = NativeStandardContinuousNodeKind::Max;
        nodes[5].children_start = 1;
        nodes[5].children_len = 2;
        let children = [2, 4, 3];
        validate_shape(&nodes, &children, &[5]).expect("fully reachable nonzero span");
        let expression = build(&nodes, &children, 5).unwrap();
        assert_eq!(
            ContinuousEvaluator::evaluate(
                &expression,
                &ContinuousInputBundle::new(vec![]).unwrap(),
                Default::default()
            )
            .unwrap()
            .get(),
            5.0
        );
    }

    #[test]
    fn diagnostic_codes_are_exhaustively_owner_mapped_for_nonfinite_and_division() {
        assert_eq!(
            DiagnosticValue::from(&Error::Value(ContinuousValueError::NonFinite {
                bits: u64::MAX
            }))
            .code,
            "STANDARD_CONTINUOUS_NONFINITE"
        );
        assert_eq!(
            DiagnosticValue::from(&Error::Evaluation(ContinuousEvaluationError::Value(
                ContinuousValueError::DivisionByZero
            )))
            .code,
            "STANDARD_CONTINUOUS_DIVIDE_ZERO"
        );
    }
}
